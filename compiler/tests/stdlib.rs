//! Does the standard library compute what it promises?
//!
//! `stdlib/not-compiling.txt` is empty, which means every module's emitted C compiles.
//! That gate answers one question — does `cc` accept it — and this file answers the other
//! one. `text` compiled for a while with `split` as an infinite loop and `==` on two
//! `String`s comparing pointers; nothing in CI said a word, because nothing ran the
//! functions.
//!
//! **This is a start, not a claim of coverage.** 249 functions are declared and about
//! ninety are exercised here. The rest are still read, not run. What is asserted is
//! asserted by value, checked against arithmetic rather than against whatever the compiler
//! happened to print.
//!
//! Functions that call a `__builtin_*` the runtime does not implement are deliberately
//! absent: `tools/stdlib-check.py` counts 38 of them, and a test that traps on an
//! unimplemented builtin is testing the trap.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn compiler() -> &'static str {
    env!("CARGO_BIN_EXE_nova-compiler")
}

fn cc() -> String {
    std::env::var("CC").unwrap_or_else(|_| "cc".to_string())
}

fn c_compiler_available() -> bool {
    Command::new(cc())
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("compiler crate has a parent directory")
        .to_path_buf()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nova-stdlib-{}-{}", std::process::id(), name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch directory");
    dir
}

/// Build a program against the real standard library and return what it printed.
///
/// Duplicated from `end_to_end.rs` on purpose: integration tests are separate crates, so
/// sharing these would mean a third crate whose only job is to hold six small functions.
fn run_with_stdlib(name: &str, source: &str) -> String {
    let dir = scratch(name);
    // Not `<name>.nova`: `find_module` looks beside the importing file first, so a
    // program called `math.nova` that says `use math;` imports itself.
    let file = dir.join(format!("{name}_program.nova"));
    let binary = dir.join(name);
    fs::write(&file, source).expect("write source");

    let build = Command::new(compiler())
        .arg("build")
        .arg(&file)
        .arg("-o")
        .arg(&binary)
        .arg("--module-root")
        .arg(workspace_root().join("stdlib"))
        .output()
        .expect("run nova-compiler build");
    assert!(
        build.status.success(),
        "build failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run built binary");
    assert!(
        run.status.success(),
        "program exited with {}:\nstderr: {}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).to_string()
}

/// `core`'s arithmetic, array and string helpers.
///
/// Every expected value is checkable by hand: 5! is 120, gcd(12, 18) is 6, and reversing
/// `[1, 2, 3]` puts 3 first. Nothing here was recorded from a run and blessed afterwards.
#[test]
fn core_functions_compute_what_they_promise() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "core_functions",
        r#"
use core;

fn main() -> int {
    print(factorial(5));
    print(gcd(12, 18));
    print(lcm(4, 6));
    print(is_prime(7));
    print(is_prime(8));
    print(sum([1, 2, 3]));
    print(product([2, 3, 4]));
    print(min(3, 5));
    print(max(3, 5));
    print(abs(0 - 4));
    print(sign(0 - 9));
    print(clamp(15, 0, 10));
    print(substr("abcdef", 1, 3));
    print(strcmp("a", "b") < 0);
    print(tostring(42));
    print(array_len([7, 8, 9]));
    let mut r = [1, 2, 3];
    reverse(r);
    print(r[0]);
    print(r[2]);
    let mut s = [3, 1, 2];
    sort(s);
    print(s[0]);
    print(s[2]);
    let mut a: [int] = [];
    array_push(a, 10);
    array_push(a, 20);
    print(array_pop(a));
    print(array_len(a));
    return 0;
}
"#,
    );
    let expected = [
        "120",  // 5!
        "6",    // gcd(12, 18)
        "12",   // lcm(4, 6)
        "true", // 7 is prime
        "false",// 8 is not
        "6",    // 1 + 2 + 3
        "24",   // 2 * 3 * 4
        "3",    // min
        "5",    // max
        "4",    // abs(-4)
        "-1",   // sign(-9)
        "10",   // clamp(15, 0, 10)
        "bcd",  // substr("abcdef", 1, 3)
        "true", // "a" < "b"
        "42",   // tostring
        "3",    // array_len
        "3",    // reversed: first is the old last
        "1",    // reversed: last is the old first
        "1",    // sorted: first is the smallest
        "3",    // sorted: last is the largest
        "20",   // popped the last push
        "1",    // one left
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `text`'s string helpers, including the two that were broken while it compiled.
///
/// `split_whitespace` and `title_case` returned nothing until `lower_if_statement` stopped
/// lowering its branch's trailing expression as an expression — a nested `if` with no
/// value became `;` and its assignment vanished. `split` was an infinite loop for a
/// different reason. None of that showed up in a compile gate.
#[test]
fn text_functions_compute_what_they_promise() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "text_functions",
        r#"
use core;
use text;

fn main() -> int {
    print(capitalize("hello"));
    print(title_case("hello world"));
    print(contains("hello", "ell"));
    print(contains("hello", "xyz"));
    print(replace_all("aaa", "a", "b"));
    print(replace_first("aaa", "a", "b"));
    print(pad_left("7", 3, '0'));
    print(pad_right("7", 3, '.'));
    print(count_char("banana", 'a'));
    print(reverse_string("abc"));
    print(is_palindrome("racecar"));
    print(is_palindrome("abc"));
    print(strip_prefix("foobar", "foo"));
    print(strip_suffix("foobar", "bar"));
    print(truncate("abcdef", 3));
    print(from_hex("ff"));
    print(is_blank("   "));
    print(is_digit_string("123"));
    print(is_alpha("abc"));
    print(ends_with("hello", "lo"));
    print(starts_with("hello", "he"));
    let w = split_whitespace("a b c");
    print(array_len(w));
    print(join(w, "-"));
    let p = split("a,b,c", ",");
    print(array_len(p));
    print(join(p, "-"));
    print(levenshtein("kitten", "sitting"));
    return 0;
}
"#,
    );
    let expected = [
        "Hello",
        "Hello World",
        "true",
        "false",
        "bbb",  // every 'a' replaced
        "baa",  // only the first
        "007",  // padded on the left to width 3
        "7..",  // padded on the right
        "3",    // 'a' in "banana"
        "cba",
        "true",
        "false",
        "bar",
        "foo",
        "abc",
        "255",  // from_hex("ff")
        "true", // is_blank("   ")
        "true",
        "true",
        "true",
        "true",
        "3",     // split_whitespace("a b c")
        "a-b-c",
        "3",     // split("a,b,c", ",")
        "a-b-c",
        "3",     // kitten -> sitting
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `math`'s scalar and float-array functions.
///
/// The float-array half is here rather than only in `end_to_end.rs` because it is the
/// standard library's, not the compiler's: `sum_float([1.5, 2.5])` returning 3 was a
/// library bug that survived because the module compiled.
#[test]
fn math_functions_compute_what_they_promise() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "math_functions",
        r#"
use core;
use math;

fn main() -> int {
    print(sqrt(16.0));
    print(cbrt(27.0));
    print(floor(3.7));
    print(ceil(3.2));
    print(round(2.5));
    print(powf(2.0, 10.0));
    print(fabs(0.0 - 2.5));
    print(fmin(3.0, 5.0));
    print(fmax(3.0, 5.0));
    print(sum_float([1.5, 2.5]));
    print(mean([1.5, 2.5]));
    print(median([1.5, 2.5]));
    print(variance([1.5, 2.5]));
    print(stddev([1.5, 2.5]));
    return 0;
}
"#,
    );
    let expected = [
        "4", "3", "3", "4", "3", "1024", "2.5", "3", "5", "4", "2", "2", "0.25", "0.5",
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}
