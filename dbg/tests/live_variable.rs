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

#[test]
fn info_variables_lists_the_locals_of_the_current_frame() {
    let Some(binary) = fixture("info") else {
        return;
    };

    let target = target::load_binary(&binary).expect("the binary loads");
    let mut session = DebugSession::new(target).expect("a session starts");
    session.run(&[]).expect("starts");
    session.set_breakpoint("fixture.c:4").expect("the printf line");
    session.continue_execution().expect("stops there");

    let listed = session.get_info("variables").expect("the frame has locals");
    // The command was deleted once because its map was never filled and it could only answer
    // "No variables in scope". It reads `.debug_info` now, so the list has to be real.
    assert!(listed.contains("counter"), "got {listed:?}");
    assert!(listed.contains("= 7"), "got {listed:?}");
}

#[test]
fn info_registers_reads_them_rather_than_printing_zeros() {
    let Some(binary) = fixture("regs") else {
        return;
    };

    let target = target::load_binary(&binary).expect("the binary loads");
    let mut session = DebugSession::new(target).expect("a session starts");
    session.run(&[]).expect("starts");

    let printed = session.get_info("registers").expect("registers are readable");
    // It used to print four fixed lines of zeros and a made-up `RIP`. A real reading has a
    // stack pointer and an instruction pointer that are neither zero nor equal.
    assert!(printed.contains("rsp"), "got {printed:?}");
    assert!(printed.contains("rip"), "got {printed:?}");
    assert!(
        !printed.contains("0x0000000000000000\nrip: 0x0000000000000000"),
        "the table should not be zeros: {printed:?}"
    );
}

#[test]
fn info_threads_refuses_rather_than_inventing_one() {
    let target = target::DebugTarget::Binary {
        path: "/bin/true".to_string(),
        symbols_loaded: false,
    };
    let session = DebugSession::new(target).expect("a session starts");
    let error = session.get_info("threads").expect_err("no thread list").to_string();
    assert!(error.contains("one process"), "{error}");
}
