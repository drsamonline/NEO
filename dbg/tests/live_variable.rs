//! The live path of `print x`, end to end.
//!
//! ADR 0023. The reader, the evaluator and the formatter were each tested in `src/dwarf.rs` and
//! `src/session.rs`; the join between them needs a *process* to stop and a *binary* to read, so it
//! could not be tested from inside the crate. This test does what a user does: start a program under
//! the debugger, stop at `main`, and print a local.
//!
//! The fixture is C rather than NOVA on purpose. What is under test is the debugger's reading of
//! DWARF, and a C compiler produces the same `.debug_info` a NOVA program's generated C does — with
//! no dependency on a working toolchain in the test environment. If the debugger can read a C
//! binary's local, it can read the one it was built for.

use std::path::{Path, PathBuf};
use std::process::Command;

use nova_dbg::session::DebugSession;
use nova_dbg::target;

/// A program with one local in `main`, built with `-g`.
///
/// Returns `None` when the host cannot build it, so the test skips loudly instead of failing on a
/// machine without a C compiler — the same rule the project's other end-to-end tests follow.
fn fixture(name: &str) -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(format!("nova-live-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).ok()?;
    let source = dir.join("fixture.c");
    std::fs::write(
        &source,
        "#include <stdio.h>\n\
         int main(void) {\n\
             int counter = 7;\n\
             printf(\"%d\\n\", counter);\n\
             return 0;\n\
         }\n",
    )
    .ok()?;
    let binary = dir.join("fixture");
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let output = Command::new(cc)
        // `-g` for the DWARF, and no frame-pointer omission so the debugger can walk the chain.
        .arg("-g")
        .arg("-O0")
        .arg("-fno-omit-frame-pointer")
        .arg("-o")
        .arg(&binary)
        .arg(&source)
        .output()
        .ok()?;
    if !output.status.success() {
        eprintln!(
            "skipping: the host C compiler could not build the fixture: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }
    Some(binary)
}

#[test]
fn print_reads_a_live_variable_from_a_stopped_program() {
    let Some(binary) = fixture("live") else {
        return;
    };

    let target = target::load_binary(&binary).expect("the binary loads");
    let mut session = DebugSession::new(target).expect("a session starts");
    // `run` **before** `break`, in this order on purpose: the fixture is position-independent, so
    // `main`'s symbol is an offset from a load base the kernel chooses. The debugger says exactly
    // this when a breakpoint is set first, and reads the base from `/proc/<pid>/maps` once there is
    // a process to ask.
    session.run(&[]).expect("the program starts under trace");
    // Line 4 is the `printf`, one line after `int counter = 7;`. Stopping at `main` itself would
    // read the slot *before* the assignment, and 0 is the honest answer there — the first version
    // of this test asserted 7 at entry and was wrong about the program, not about the debugger.
    session
        .set_breakpoint("fixture.c:4")
        .expect("the printf line is a breakpoint location");
    session
        .continue_execution()
        .expect("it runs to the breakpoint after the assignment");

    // The compiler's own numbers, printed before the assertion. A wrong slot produces a number, so
    // the number alone cannot say *why* it is wrong; these four lines can. The location expression
    // says what gcc thinks the offset is, and the two addresses say where this debugger looked.
    let pid = session.traced_pid().expect("a traced pid");
    let rbp = nova_dbg::target::frame_pointer(pid).expect("rbp");
    let locals = nova_dbg::dwarf::locals_of_function(&binary, "main").expect("locals");
    if let Some(counter) = locals.iter().find(|local| local.name == "counter") {
        eprintln!("DIAG location = {}", counter.location);
        eprintln!("DIAG ops      = {:?}", counter.ops);
    }
    eprintln!("DIAG rbp      = 0x{rbp:x}");
    eprintln!("DIAG cfa      = 0x{:x}", rbp.wrapping_add(16));
    for delta in [-24i64, -20, -16, -12, -8, -4] {
        let address = (rbp as i64 + delta) as u64;
        let word = nova_dbg::target::read_word(pid, address).unwrap_or(0);
        eprintln!(
            "DIAG rbp{delta:+} = 0x{address:x} holds {}",
            word as u32 as i32
        );
    }

    // The assertion is on the value the program was written to hold: `counter = 7`. A wrong slot, a
    // wrong width or a wrong frame base all produce a number, which is why the number matters.
    let printed = session.evaluate("counter").expect("counter is readable");
    assert!(
        printed.contains("= 7"),
        "`print counter` should report 7; got {printed:?}"
    );
    assert!(
        printed.contains("int"),
        "the type should be reported; got {printed:?}"
    );
}

#[test]
fn a_name_that_is_not_a_local_is_refused_with_the_ones_that_are() {
    let Some(binary) = fixture("missing") else {
        return;
    };

    let target = target::load_binary(&binary).expect("the binary loads");
    let mut session = DebugSession::new(target).expect("a session starts");
    // Same order and the same stopping point: `run`, then `break` on the line after the
    // assignment, so the frame is a real one with a known function.
    session.run(&[]).expect("starts");
    session
        .set_breakpoint("fixture.c:4")
        .expect("the printf line");
    session.continue_execution().expect("stops there");

    let error = session
        .evaluate("nonexistent")
        .expect_err("there is no such local")
        .to_string();
    assert!(error.contains("no variable named"), "{error}");
    // It should say what *is* there, so a typo is a one-line fix rather than a hunt.
    assert!(error.contains("counter"), "{error}");
}

/// The binary path the test built, for a failure message worth reading.
#[allow(dead_code)]
fn describe(path: &Path) -> String {
    path.display().to_string()
}
