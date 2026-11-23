//! Does the standard library compute what it promises?
//!
//! `stdlib/not-compiling.txt` is empty, which means every module's emitted C compiles.
//! That gate answers one question — does `cc` accept it — and this file answers the other
//! one. `text` compiled for a while with `split` as an infinite loop and `==` on two
//! `String`s comparing pointers; nothing in CI said a word, because nothing ran the
//! functions.
//!
//! **This is a start, not a claim of coverage.** 249 functions are declared and 189 are
//! exercised here. `math` is complete — every function it exports has an assertion. What is
//! asserted is asserted by value, checked against arithmetic or a published standard rather
//! than against whatever the compiler happened to print, except where no value is knowable:
//! `rand` is asserted deterministic under a seed, `cwd` non-empty, `now` positive.
//!
//! Functions that call a `__builtin_*` the runtime does not implement are deliberately
//! absent: `tools/stdlib-check.py` counts 11 of them, and a test that traps on an
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
    build_and_run(&scratch(name), name, source)
}

/// Build a program in `dir` and return what it printed.
fn build_and_run(dir: &Path, name: &str, source: &str) -> String {
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

/// Build a program against a directory tree with known contents, and return what it printed.
///
/// `files` maps a relative path to its contents; a path ending in `/` makes a directory.
/// Every `__FIXTURE__` in the source is replaced with the fixture's absolute path, so the
/// program names the files the harness created and neither side has to guess.
///
/// This exists because `io`'s functions want a filesystem rather than a value, and a test
/// that invents its own paths leaves debris behind and behaves differently on a second run.
fn run_with_fixture(name: &str, files: &[(&str, &str)], source: &str) -> String {
    let dir = scratch(name);
    let fixture = dir.join("fixture");
    fs::create_dir_all(&fixture).expect("create fixture directory");
    for (relative, contents) in files {
        let path = fixture.join(relative);
        if relative.ends_with('/') {
            fs::create_dir_all(&path).expect("create fixture directory");
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create fixture parent");
            }
            fs::write(&path, contents).expect("write fixture file");
        }
    }
    let source = source.replace("__FIXTURE__", &fixture.to_string_lossy());
    build_and_run(&dir, name, &source)
}

/// Build a program and expect it to be rejected *at run time*, returning what it printed
/// to stderr. Used for constructs the backend refuses rather than emits.
fn run_expecting_rejection(name: &str, source: &str) -> String {
    let dir = scratch(name);
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
        "build failed, so the construct was rejected at compile time instead: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let run = Command::new(&binary).output().expect("run built binary");
    assert!(
        !run.status.success(),
        "the program ran to completion, so the construct was silently accepted"
    );
    String::from_utf8_lossy(&run.stderr).to_string()
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
    match array_pop(a) {
        Some(v) => print(v),
        None => print(0 - 1)
    }
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

/// `core`'s predicates, `typeof`, and `for` over a range.
///
/// The loop is here because it was broken and nothing noticed: no example, no parity
/// program and no stdlib function contains a `for` at all, so `range_bounds` could miss
/// the `Typed` wrapper around `range(0, 4)` and every such loop silently iterated zero
/// times. `total` would have been 0 rather than 6.
#[test]
fn core_predicates_and_range_iteration() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "core_predicates",
        r#"
use core;

fn main() -> int {
    print(is_even(4));
    print(is_odd(4));
    print(fibonacci(10));
    print(typeof(1));
    print(typeof("a"));
    print(typeof(true));
    let mut total = 0;
    for i in range(0, 4) {
        total = total + i;
    }
    print(total);
    return 0;
}
"#,
    );
    let expected = [
        "true",   // 4 is even
        "false",  // 4 is not odd
        "55",     // fibonacci(10)
        "int",
        "string",
        "bool",
        "6",      // 0 + 1 + 2 + 3
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `text`'s search, edit and formatting helpers.
///
/// `hash_string` is asserted for *determinism* rather than for a value: a recorded digest
/// is a change detector, and the function promises a stable hash, not a particular one.
#[test]
fn text_search_and_edit_functions() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "text_search_edit",
        r#"
use core;
use text;

fn main() -> int {
    print(find_string("hello world", "o"));
    print(rfind_string("hello world", "o"));
    print(count_substring("banana", "an"));
    print(remove_char_at("abc", 1));
    print(insert_at("ac", 1, "b"));
    print(indent("a", 2));
    print(dedent("  a"));
    print(is_alphanumeric("a1"));
    print(center("ab", 6, '-'));
    print(zero_pad(7, 3));
    print(format_number(1234567));
    print(strings_similar("abc", "abd", 2));
    print(url_decode("a%20b"));
    print(hash_string("abc") == hash_string("abc"));
    return 0;
}
"#,
    );
    let expected = [
        "4",           // first 'o'
        "7",           // last 'o'
        "2",           // "an" twice in "banana"
        "ac",          // removed index 1
        "abc",         // inserted at 1
        "  a",         // indented by 2
        "a",           // dedented
        "true",
        "--ab--",      // centred in 6
        "007",
        "1,234,567",   // thousands separators
        "true",        // distance 1 is within 2
        "a b",         // percent-decoded
        "true",        // the hash is stable
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `for` over an array is not implemented, and says so instead of doing nothing.
///
/// The backend emitted a comment and an empty loop body, so the loop compiled, ran, and
/// iterated zero times — a program that looks like it sums an array and returns 0. It
/// traps now, and this asserts the trap rather than the wrong answer.
#[test]
fn for_over_an_array_is_rejected() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stderr = run_expecting_rejection(
        "for_array",
        r#"
use core;

fn main() -> int {
    let mut n = 0;
    for x in [10, 20, 30] {
        n = n + x;
    }
    print(n);
    return 0;
}
"#,
    );
    assert!(
        stderr.contains("for over this iterable"),
        "the rejection must name the construct, not just fail: {stderr}"
    );
}

/// `core`'s higher-order functions, called with closures.
///
/// The closure syntax is `|x| body`, not `fn(x) { ... }` — the latter does not parse, and
/// nothing in `examples/`, `tests/parity/` or the standard library uses a closure, so
/// `map`, `filter` and `reduce` had never been called by anything.
///
/// `reduce`'s parameter order is `(arr, initial, f)`, which is easy to get wrong and was
/// wrong in the first draft of this test.
#[test]
fn core_higher_order_functions_take_closures() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "core_higher_order",
        r#"
use core;

fn main() -> int {
    let doubled = map([1, 2, 3], |x| x * 2);
    print(doubled[0]);
    print(doubled[2]);
    let evens = filter([1, 2, 3, 4], |x| is_even(x));
    print(array_len(evens));
    print(evens[0]);
    print(reduce([1, 2, 3, 4], 0, |a, b| a + b));
    print(reduce([1, 2, 3, 4], 1, |a, b| a * b));
    print(id(7));
    return 0;
}
"#,
    );
    let expected = [
        "2",   // 1 * 2
        "6",   // 3 * 2
        "2",   // two evens
        "2",   // the first of them
        "10",  // 1 + 2 + 3 + 4
        "24",  // 1 * 2 * 3 * 4
        "7",   // id
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `text`'s escaping, wrapping and hex helpers.
#[test]
fn text_escaping_and_wrapping() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "text_escaping",
        r#"
use core;
use text;

fn main() -> int {
    print(escape_string("a\"b"));
    print(escape_string("a\nb"));
    print(wrap_text("one two three", 7));
    print(to_hex(255));
    print(url_encode("a b"));
    return 0;
}
"#,
    );
    let expected = [
        "a\\\"b",       // the quote is escaped
        "a\\nb",        // so is the newline
        "one two\nthree", // wrapped at 7
        "FF",           // to_hex is upper case
        "a+b",          // form encoding: a space becomes '+', not %20
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// A generic function returning `Option<T>`, and a `match` that evaluates its scrutinee once.
///
/// Two defects met here. `find<T>` is declared `-> Option<int>`, but the instantiation's
/// return type was computed from the type's *name* — `Option`, which `c_type` maps to
/// `int` — so `find_int` was declared to return an `int` while returning an `Option_int`.
/// And `match` read its scrutinee twice, once for the tag and once for the payload, so
/// `match array_pop(a)` popped two elements.
#[test]
fn option_returning_generics_and_single_evaluation() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "option_generics",
        r#"
use core;

fn main() -> int {
    match find([1, 2, 3], |x| x > 1) {
        Some(i) => print(i),
        None => print(0 - 1)
    }
    match find([1, 2, 3], |x| x > 9) {
        Some(i) => print(i),
        None => print(0 - 1)
    }
    let mut a: [int] = [];
    array_push(a, 10);
    array_push(a, 20);
    match array_pop(a) {
        Some(v) => print(v),
        None => print(0 - 1)
    }
    print(array_len(a));
    return 0;
}
"#,
    );
    let expected = [
        "1",   // the first element greater than 1 is at index 1
        "-1",  // nothing is greater than 9
        "20",  // the last push — popped once, not twice
        "1",   // one element left
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `io`'s file API: open, write, read back, close.
///
/// Every function in `io.nova` that touched a file used to trap, because the ten
/// `__builtin_f*` calls fell through to `nova_unsupported`. Wiring them up exposed two
/// defects of their own:
///
/// - `match fopen(path, "w") { .. }` opened the file **twice** — once for the payload
///   binding and once for the tag test — so `write_file` truncated its own output and
///   returned `true` over an empty file. `lower_match_chain` hoists the scrutinee now.
/// - `fread_all` concatenated lines without their newlines and stopped at a blank line,
///   because `fread_line` returns "" for both. It reads the whole file in one builtin now.
///
/// This round-trip is the guard for both: either defect makes it return the wrong string.
#[test]
fn io_reads_back_what_it_writes() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "io_roundtrip",
        r#"
use core;
use io;

fn main() -> int {
    print(write_file("/tmp/nova_stdlib_io_test.txt", "alpha\nbeta\n"));
    match read_file("/tmp/nova_stdlib_io_test.txt") {
        Some(text) => print(text),
        None => print("missing"),
    }
    match fopen("/tmp/nova_stdlib_io_test.txt", "r") {
        Some(mut f) => {
            print(fread_line(&mut f));
            print(ftell(&mut f));
            print(fclose(&mut f));
        },
        None => print("missing"),
    }
    match fopen("/tmp/nova_no_such_file_at_all.txt", "r") {
        Some(mut f) => print("opened"),
        None => print("absent"),
    }
    return 0;
}
"#,
    );
    let expected = [
        "true",              // write_file
        "alpha",             // read_file keeps the newlines
        "beta",
        "",                  // trailing newline in the file
        "alpha",             // fread_line
        "6",                 // ftell: five characters and the newline
        "true",              // fclose
        "absent",            // a missing path is None, not a trap
    ];
    assert_eq!(stdout.trim_end().lines().collect::<Vec<_>>().join("\n"), expected.join("\n"));
}

/// `text`'s checksum and encoding helpers, against published vectors.
///
/// These are the first stdlib functions in this file whose answer is a *standard* rather
/// than a value read off a run: the CRC-32 of `"123456789"` is `0xCBF43926`, and base64 is
/// RFC 4648. That makes them checkable by anyone, which is the point of asserting them.
///
/// `crc32` is why `int` had to become 64-bit: `0xCBF43926` does not fit in a signed 32-bit
/// integer, so a correct checksum came back negative. See ADR 0011.
#[test]
fn text_checksums_and_encodings_match_the_standard() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "text_digest",
        r#"
use core;
use text;

fn main() -> int {
    print(crc32("123456789"));
    print(base64_encode(""));
    print(base64_encode("f"));
    print(base64_encode("fo"));
    print(base64_encode("foo"));
    print(base64_encode("hello world"));
    print(base64_decode("Zm9v"));
    print(base64_decode("aGVsbG8gd29ybGQ="));
    print(base64_decode(""));
    return 0;
}
"#,
    );
    let expected = [
        "3421780262",        // CRC-32 of "123456789", the standard check value
        "",                  // RFC 4648, empty input
        "Zg==",              // one byte, two pad characters
        "Zm8=",              // two bytes, one pad character
        "Zm9v",              // three bytes, no padding
        "aGVsbG8gd29ybGQ=",  // eleven bytes
        "foo",               // round trip
        "hello world",
        // The final `base64_decode("")` prints an empty line, which `trim_end` absorbs.
    ];
    assert_eq!(stdout.trim_end().lines().collect::<Vec<_>>().join("\n"), expected.join("\n"));
}

/// `text.md5` and `text.sha256` against the published vectors.
///
/// Both are RFC/FIPS standards with canonical check values, so these assertions can be
/// checked against a standard rather than against this implementation. The empty string is
/// included deliberately: it is the case where padding is entirely synthetic, and the one
/// an off-by-one in the length field gets wrong.
#[test]
fn text_digests_match_published_vectors() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "text_digests",
        r#"
use core;
use text;

fn main() -> int {
    print(md5(""));
    print(md5("abc"));
    print(md5("The quick brown fox jumps over the lazy dog"));
    print(sha256(""));
    print(sha256("abc"));
    print(sha256("The quick brown fox jumps over the lazy dog"));
    return 0;
}
"#,
    );
    let expected = [
        "d41d8cd98f00b204e9800998ecf8427e",                                  // RFC 1321, empty
        "900150983cd24fb0d6963f7d28e17f72",                                  // RFC 1321, "abc"
        "9e107d9d372bb6826bd81d3542a419d6",                                  // the fox
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",  // FIPS 180-4, empty
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",  // FIPS 180-4, "abc"
        "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592",  // the fox
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `io`'s path helpers: split, join, and list.
///
/// These are pure string operations over a path, so every expectation is arithmetic rather
/// than a value read off a run. The awkward cases are the ones worth asserting: a trailing
/// separator is not part of the name, a leading dot is a hidden file and not an extension,
/// and joining onto an absolute path discards the base.
#[test]
fn io_paths_split_and_join() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "io_paths",
        r#"
use core;
use io;

fn main() -> int {
    print(basename("/a/b/c.txt"));
    print(basename("/a/b/"));
    print(extension("/a/b/c.txt"));
    print(extension("/a/b/.hidden"));
    print(parent_dir("/a/b/c.txt"));
    print(parent_dir("/c.txt"));
    print(join_path("/a", "b"));
    print(join_path("/a/", "b"));
    print(join_path("/a", "/b"));
    print(array_len(list_dir("/definitely/not/a/directory")));
    return 0;
}
"#,
    );
    let expected = [
        "c.txt",  // the name, not the path
        "b",      // a trailing separator is not part of the name
        "txt",    // after the last dot
        "",       // a leading dot is a hidden file, not an extension
        "/a/b",   // the parent
        "/",      // one level deep
        "/a/b",   // one separator, not two
        "/a/b",   // a trailing separator on the base is not doubled
        "/b",     // an absolute path discards the base
        "0",      // a directory that does not exist lists nothing, and does not trap
    ];
    assert_eq!(stdout.trim_end().lines().collect::<Vec<_>>().join("\n"), expected.join("\n"));
}

/// `io.stat` returns an `Option` of a **struct** — the first payload that is not a scalar.
///
/// The runtime deliberately does not build the `FileMetadata`: it is C, and the struct is a
/// layout the language defines. `stat` asks for the scalar facts and composes the struct in
/// NOVA, which is also why `Option<FileMetadata>` exercises the option machinery's struct
/// path rather than its scalar one.
///
/// `size` is the interesting field: it is the one a wrong `stat` buffer or a bytes-vs-
/// characters mistake gets wrong.
#[test]
fn io_stat_carries_a_struct_in_an_option() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "io_stat",
        r#"
use core;
use io;

fn main() -> int {
    write_file("/tmp/nova_stdlib_stat.txt", "hello\n");
    match stat("/tmp/nova_stdlib_stat.txt") {
        Some(m) => {
            print(m.name);
            print(m.size);
            print(m.is_dir);
            print(m.modified > 0);
        },
        None => print("missing"),
    }
    match stat("/tmp") {
        Some(m) => print(m.is_dir),
        None => print("missing"),
    }
    match stat("/no/such/path/at/all") {
        Some(m) => print("found"),
        None => print("absent"),
    }
    return 0;
}
"#,
    );
    let expected = [
        "nova_stdlib_stat.txt",  // the name, not the path
        "6",                     // "hello" and its newline, in bytes
        "false",                 // a regular file
        "true",                  // it has a modification time
        "true",                  // /tmp is a directory
        "absent",                // a missing path is None, not a trap
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// The bulk of `math`: trigonometry, interpolation, vectors, combinatorics.
///
/// Every expectation here is arithmetic a reader can check without running anything —
/// `hypot(3, 4)` is 5, `dot2((1,2),(3,4))` is 11, `factorialf(5)` is 120. That is the point
/// of asserting them: a wrong formula fails a reader, not just a run.
///
/// `factorialf` was the last function blocked by a missing builtin. Its non-integer branch
/// calls the gamma function, which nothing implemented, so `factorialf(5.0)` trapped even
/// though the integer branch never reaches it.
#[test]
fn math_functions_compute_standard_values() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "math_bulk",
        r#"
use core;
use math;

fn main() -> int {
    print(sin(0.0));
    print(cos(0.0));
    print(tan(0.0));
    print(degrees(3.141592653589793));
    print(radians(180.0));
    print(hypot(3.0, 4.0));
    print(trunc(3.7));
    print(fract(3.25));
    print(minf([2.0, 5.0, 1.5]));
    print(maxf([2.0, 5.0, 1.5]));
    print(clampf(10.0, 0.0, 1.0));
    print(clampi(10, 0, 5));
    print(signf(0.0 - 3.0));
    print(lerp(0.0, 10.0, 0.5));
    print(inverse_lerp(0.0, 10.0, 5.0));
    print(dot2(1.0, 2.0, 3.0, 4.0));
    print(dot3(1.0, 2.0, 3.0, 4.0, 5.0, 6.0));
    print(length2(3.0, 4.0));
    print(distance(0.0, 0.0, 3.0, 4.0));
    print(factorialf(5.0));
    print(binomial(5, 2));
    print(combinations(5, 2));
    print(permutations(5, 2));
    print(gcdf(12.0, 18.0));
    print(lcmf(4.0, 6.0));
    print(copysign(3.0, 0.0 - 1.0));
    print(is_nan(nan()));
    print(is_infinite(infinity()));
    print(is_finite(1.0));
    print(remap(5, 0, 10, 0, 100));
    print(map_range(5, 0, 10, 0, 100));
    print(smoothstep(0.0, 1.0, 0.5));
    print(smootherstep(0.0, 1.0, 0.5));
    print(approx_equal(1.0, 1.0001, 0.001));
    print(nearly_equal(1.0, 1.0));
    return 0;
}
"#,
    );
    let expected = [
        "0",         // sin 0
        "1",         // cos 0
        "0",         // tan 0
        "180",       // pi radians is 180 degrees
        "3.14159",   // 180 degrees is pi radians
        "5",         // the 3-4-5 triangle
        "3",         // truncation toward zero
        "0.25",      // the fractional part
        "1.5",       // smallest
        "5",         // largest
        "1",         // clamped to the upper bound
        "5",         // clamped to the upper bound
        "-1",        // the sign of a negative
        "5",         // halfway between 0 and 10
        "0.5",       // 5 is halfway between 0 and 10
        "11",        // 1*3 + 2*4
        "32",        // 1*4 + 2*5 + 3*6
        "5",         // the 3-4-5 triangle again
        "5",         // the same, as a distance
        "120",       // 5!
        "10",        // C(5,2)
        "10",        // C(5,2)
        "20",        // P(5,2)
        "6",         // gcd(12, 18)
        "12",        // lcm(4, 6)
        "-3",        // the sign of y is applied to x
        "true",      // nan is nan
        "true",      // infinity is infinite
        "true",      // 1 is finite
        "50",        // 5 of the way from 0..10 into 0..100
        "50",        // the integer version of the same
        "0.5",       // smoothstep at the midpoint
        "0.5",       // smootherstep at the midpoint
        "true",      // within 0.001
        "true",      // identical values are nearly equal
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `core`'s collection, string and higher-order helpers.
///
/// `array_get` answers `Option<T>` and `array_set` answers `bool`, which is the difference
/// between a checked index and a trap — both are asserted at an index that exists and one
/// that does not.
#[test]
fn core_collections_and_strings_compute_standard_values() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "core_collections",
        r#"
use core;

fn main() -> int {
    let a = [10, 20, 30];
    match array_get(a, 1) {
        Some(v) => print(v),
        None => print(0 - 1),
    }
    match array_get(a, 9) {
        Some(v) => print(v),
        None => print(0 - 1),
    }
    print(array_set(a, 1, 99));
    print(a[1]);
    match binary_search([1, 3, 5, 7], 5) {
        Some(i) => print(i),
        None => print(0 - 1),
    }
    match binary_search([1, 3, 5, 7], 4) {
        Some(i) => print(i),
        None => print(0 - 1),
    }
    match linear_search([1, 3, 5, 7], 7) {
        Some(i) => print(i),
        None => print(0 - 1),
    }
    print(pow(2, 10));
    print(strcat("ab", "cd"));
    print(strlen("abc"));
    print(pipe(3, |x| x + 1));
    let doubled_after_plus_one = compose(|x| x * 2, |x| x + 1);
    print(doubled_after_plus_one(3));
    assert(true, "never shown");
    println("printed");
    sleep(0);
    return 0;
}
"#,
    );
    let expected = [
        "20",    // array_get finds the element
        "-1",    // and reports absence rather than trapping
        "true",  // array_set succeeds
        "99",    // and the element changed
        "2",     // binary_search finds 5 at index 2
        "-1",    // and reports absence for 4
        "3",     // linear_search finds 7 at index 3
        "1024",  // 2^10
        "abcd",  // concatenation
        "3",     // the length of "abc"
        "4",     // pipe applies the function
        "8",     // compose(f, g) is f(g(x)): (3 + 1) * 2
        "printed",
    ];
    assert_eq!(stdout.trim_end().lines().collect::<Vec<_>>().join("\n"), expected.join("\n"));
}

/// `core`'s environment and random helpers.
///
/// Nothing here asserts a value that could differ between runs: `cwd` is asserted non-empty,
/// `now` is asserted positive, and `rand_range` is asserted to stay inside its bounds. The
/// random generator is asserted *deterministic under a seed* instead, which is the only
/// property a test can honestly hold it to.
#[test]
fn core_environment_and_random_are_deterministic() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "core_env",
        r#"
use core;

fn main() -> int {
    print(strlen(cwd()) > 0);
    print(getenv("NO_SUCH_VARIABLE_XYZ_123"));
    print(now() > 0);
    srand(1);
    let first = rand();
    srand(1);
    let second = rand();
    print(first == second);
    let low = rand_range(5, 6);
    print(low >= 5 && low <= 6);
    return 0;
}
"#,
    );
    let expected = [
        "true",  // the working directory has a name
        "",      // a variable that does not exist is empty, not a trap
        "true",  // the clock is after the epoch
        "true",  // the same seed produces the same first number
        "true",  // the range is inclusive at both ends, and stays inside them
    ];
    assert_eq!(stdout.trim_end().lines().collect::<Vec<_>>().join("\n"), expected.join("\n"));
}

/// `panic` stops the program and says why.
///
/// The message is the whole contract: a trap that does not name its cause is barely better
/// than a segfault.
#[test]
fn panic_reports_its_message() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stderr = run_expecting_rejection(
        "core_panic",
        r#"
use core;

fn main() -> int {
    panic("deliberate");
    return 0;
}
"#,
    );
    assert!(
        stderr.contains("deliberate"),
        "the trap must carry the message it was given: {stderr}"
    );
}

/// `math`'s remaining deterministic functions: transcendentals, 2D and 3D vectors, angles.
///
/// The tuple-returning ones — `normalize2`, `cross3`, `rotate2` — are reachable through
/// destructuring, `let (x, y) = normalize2(3.0, 4.0)`, which is what makes them testable at
/// all. Indexing a tuple with `t.0` does not parse.
#[test]
fn math_transcendentals_and_vectors_compute_standard_values() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "math_vectors",
        r#"
use core;
use math;

fn main() -> int {
    print(acos(1.0));
    print(asin(0.0));
    print(atan(1.0));
    print(atan2(1.0, 1.0));
    print(cosh(0.0));
    print(sinh(0.0));
    print(tanh(0.0));
    print(exp(0.0));
    print(log(1.0));
    print(log10(100.0));
    print(log2(8.0));
    print(fmod(7.0, 3.0));
    print(remainder(7.0, 3.0));
    print(cross2(1.0, 0.0, 0.0, 1.0));
    print(length3(1.0, 2.0, 2.0));
    print(distance3d(0.0, 0.0, 0.0, 1.0, 2.0, 2.0));
    let (nx, ny) = normalize2(3.0, 4.0);
    print(nx);
    print(ny);
    let (rx, ry) = rotate2(1.0, 0.0, PI / 2.0);
    print(approx_equal(rx, 0.0, 0.0001));
    print(approx_equal(ry, 1.0, 0.0001));
    print(lerp_progress(0.0, 10.0, 5, 10));
    print(lerpi(0, 10, 0.5));
    print(normalize_angle(0.0 - PI));
    print(normalize_angle_signed(PI + 1.0));
    let (cx, cy, cz) = cross3(1.0, 0.0, 0.0, 0.0, 1.0, 0.0);
    print(cx);
    print(cy);
    print(cz);
    let (ux, uy, uz) = normalize3(1.0, 2.0, 2.0);
    print(ux);
    print(uy);
    print(uz);
    return 0;
}
"#,
    );
    let expected = [
        "0",          // acos 1
        "0",          // asin 0
        "0.785398",   // atan 1 is pi/4
        "0.785398",   // atan2(1,1) is pi/4
        "1",          // cosh 0
        "0",          // sinh 0
        "0",          // tanh 0
        "1",          // e^0
        "0",          // ln 1
        "2",          // log10 100
        "3",          // log2 8
        "1",          // 7 mod 3
        "1",          // IEEE remainder, same here
        "1",          // the 2D cross product of x-hat and y-hat
        "3",          // |(1,2,2)|
        "3",          // the distance between the origin and (1,2,2)
        "0.6",        // (3,4) normalised
        "0.8",
        "true",       // rotating x-hat by a quarter turn lands on the y axis
        "true",
        "5",          // halfway through the range
        "5",          // the integer version
        "3.14159",    // -pi normalises to pi
        "-2.14159",   // pi + 1 normalises to 1 - pi
        "0",          // x-hat cross y-hat is z-hat
        "0",
        "1",
        "0.333333",   // (1,2,2) normalised, each divided by its length of 3
        "0.666667",
        "0.666667",
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `math`'s random family, asserted for the properties a test can honestly hold them to.
///
/// All four are built on `rand()`, so `srand` makes them reproducible. Nothing here pins a
/// value: it asserts that a seed reproduces a sequence, and that the outputs stay in their
/// documented ranges.
#[test]
fn math_random_is_seeded_and_bounded() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "math_random",
        r#"
use core;
use math;

fn main() -> int {
    srand(7);
    let a = randomf();
    srand(7);
    let b = randomf();
    print(a == b);
    print(a >= 0.0 && a < 1.0);
    let r = random_range(2.0, 3.0);
    print(r >= 2.0 && r < 3.0);
    srand(9);
    let n1 = random_normal(0.0, 1.0);
    srand(9);
    let n2 = random_normal(0.0, 1.0);
    print(n1 == n2);
    srand(11);
    let e1 = random_exponential(2.0);
    srand(11);
    let e2 = random_exponential(2.0);
    print(e1 == e2);
    print(e1 >= 0.0);
    return 0;
}
"#,
    );
    let expected = [
        "true",  // the same seed reproduces randomf
        "true",  // and it stays in [0, 1)
        "true",  // random_range stays in [min, max)
        "true",  // the same seed reproduces the normal
        "true",  // and the exponential
        "true",  // which is never negative
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `io`'s filesystem functions, against a directory tree the harness builds.
///
/// This is the test `run_with_fixture` exists for. Before it, `io`'s 45 untested functions
/// had no way to reach a real filesystem: a program that invents its own paths leaves debris
/// and behaves differently on a second run.
///
/// The fixture is two entries — a three-byte file and an empty directory — and every
/// assertion is a count or a boolean a reader can predict from that.
#[test]
fn io_manipulates_a_real_directory_tree() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_fixture(
        "io_filesystem",
        &[("a.txt", "abc"), ("sub/", "")],
        r#"
use core;
use io;

fn main() -> int {
    let root = "__FIXTURE__";
    print(file_exists(root));
    print(is_directory(root));
    print(array_len(list_dir(root)));
    print(file_exists(join_path(root, "a.txt")));
    print(is_file(join_path(root, "a.txt")));
    print(file_size(join_path(root, "a.txt")));
    print(is_directory(join_path(root, "sub")));
    print(append_file(join_path(root, "a.txt"), "de"));
    print(file_size(join_path(root, "a.txt")));
    print(copy_file(join_path(root, "a.txt"), join_path(root, "b.txt")));
    print(file_exists(join_path(root, "b.txt")));
    print(rename_file(join_path(root, "b.txt"), join_path(root, "c.txt")));
    print(file_exists(join_path(root, "c.txt")));
    print(delete_file(join_path(root, "c.txt")));
    print(file_exists(join_path(root, "c.txt")));
    print(mkdir(join_path(root, "newdir")));
    print(is_directory(join_path(root, "newdir")));
    print(rmdir(join_path(root, "newdir")));
    print(is_directory(join_path(root, "newdir")));
    print(strlen(getcwd()) > 0);
    return 0;
}
"#,
    );
    let expected = [
        "true",   // the fixture root exists
        "true",   // and it is a directory
        "2",      // one file and one subdirectory
        "true",   // the file exists
        "true",   // and it is a file, not a directory
        "3",      // "abc"
        "true",   // the subdirectory exists
        "true",   // append succeeds
        "5",      // "abcde"
        "true",   // copy succeeds
        "true",   // the copy exists
        "true",   // rename succeeds
        "true",   // under its new name
        "true",   // delete succeeds
        "false",  // and the file is gone
        "true",   // mkdir succeeds
        "true",   // the new directory is a directory
        "true",   // rmdir succeeds
        "false",  // and it is gone
        "true",   // the working directory has a name
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `io`'s string parsers, which used to answer `Some` for everything.
///
/// `parse_int` wrapped `__builtin_parse_int` in `Some` unconditionally, and the builtin
/// answers 0 for anything it cannot read — so `parse_int("nope")` was `Some(0)`, quietly
/// turning a typo into a zero. `parse_float` had the same defect. Both validate the input
/// now, so the `Option` means what it says.
#[test]
fn io_parsers_reject_what_they_cannot_read() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "io_parsers",
        r#"
use core;
use io;

fn main() -> int {
    match parse_int("42") { Some(v) => print(v), None => print(0 - 1), }
    match parse_int("nope") { Some(v) => print(v), None => print(0 - 1), }
    match parse_int("") { Some(v) => print(v), None => print(0 - 1), }
    match parse_int("-7") { Some(v) => print(v), None => print(0 - 1), }
    match parse_int("12x") { Some(v) => print(v), None => print(0 - 1), }
    match parse_float("1.5") { Some(v) => print(v), None => print(0.0 - 1.0), }
    match parse_float("x") { Some(v) => print(v), None => print(0.0 - 1.0), }
    match parse_float("2e3") { Some(v) => print(v), None => print(0.0 - 1.0), }
    match parse_float("1.2.3") { Some(v) => print(v), None => print(0.0 - 1.0), }
    match parse_bool("yes") { Some(v) => print(v), None => print(false), }
    match parse_bool("maybe") { Some(v) => print(v), None => print(false), }
    return 0;
}
"#,
    );
    let expected = [
        "42",     // a valid integer
        "-1",     // a word is not one
        "-1",     // neither is the empty string
        "-7",     // a negative one
        "-1",     // trailing junk makes it invalid
        "1.5",    // a valid float
        "-1",     // a letter is not one
        "2000",   // an exponent is accepted
        "-1",     // two decimal points are not
        "true",   // a word the parser knows
        "false",  // and one it does not
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `io`'s file operations that the fixture makes reachable: write, seek, read back.
///
/// `fread_bytes` is asserted by the byte it returns rather than by the array it builds —
/// `"hello"` starts with `h`, which is 104.
#[test]
fn io_reads_and_writes_through_a_file_handle() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_fixture(
        "io_handles",
        &[],
        r#"
use core;
use io;

fn main() -> int {
    let root = "__FIXTURE__";
    match fopen(join_path(root, "w.txt"), "w") {
        Some(mut f) => {
            print(fwrite(&mut f, "hello"));
            print(fflush(&mut f));
            print(fclose(&mut f));
        },
        None => print("no open"),
    }
    match fopen(join_path(root, "w.txt"), "r") {
        Some(mut f) => {
            print(fread_all(&mut f));
            print(feof(&mut f));
            print(fseek(&mut f, 0));
            let bytes = fread_bytes(&mut f, 2);
            print(array_len(bytes));
            print(bytes[0]);
            print(fclose(&mut f));
        },
        None => print("no open"),
    }
    match fopen(join_path(root, "bin.dat"), "w") {
        Some(mut f) => {
            print(fwrite_bytes(&mut f, [65, 66, 67]));
            print(fclose(&mut f));
        },
        None => print("no open"),
    }
    print(file_size(join_path(root, "bin.dat")));
    print(resolve_path("/tmp"));
    return 0;
}
"#,
    );
    let expected = [
        "true",   // fwrite succeeded
        "true",   // fflush succeeded
        "true",   // fclose succeeded
        "hello",  // fread_all returns the whole file
        "true",   // feof is set after reading to the end
        "true",   // fseek rewinds
        "2",      // two bytes requested and returned
        "104",    // 'h'
        "true",   // fclose
        "true",   // fwrite_bytes answers whether the whole array went out, not a count
        "true",   // fclose
        "3",      // and the file is three bytes
        "/tmp",   // an absolute path resolves to itself
    ];
    assert_eq!(stdout.trim(), expected.join("\n"));
}

/// `io`'s terminal and logging functions.
///
/// These cannot be asserted as whole strings: `info`, `warn`, `error` and `debug` prefix
/// every line with a **timestamp**, so the output differs between runs. The test asserts the
/// parts that do not change — the level tag and the message — against the captured stdout.
///
/// All of them were blocked until `__builtin_write` existed; `io.writeln` and the whole
/// logging family go through `io.write`.
#[test]
fn io_writes_to_the_terminal() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "io_terminal",
        r#"
use core;
use io;

fn main() -> int {
    writeln("plain");
    info("an info");
    warn("a warning");
    error("an error");
    debug("a debug");
    print_color("in red", "red");
    print_colored("in both", "green", "blue");
    progress_bar(1, 4, 8);
    spinner(0);
    clear_screen();
    set_cursor(1, 1);
    print(terminal_width() > 0);
    print(terminal_height() > 0);
    return 0;
}
"#,
    );
    assert!(stdout.starts_with("plain\n"), "writeln must print the text and a newline");
    for (tag, message) in [
        ("[INFO]", "an info"),
        ("[WARN]", "a warning"),
        ("[ERROR]", "an error"),
        ("[DEBUG]", "a debug"),
    ] {
        assert!(stdout.contains(tag), "the output must carry the level {tag}");
        assert!(stdout.contains(message), "the output must carry the message {message}");
    }
    assert!(stdout.contains("in red"), "print_color must print its text");
    assert!(stdout.contains("in both"), "print_colored must print its text");
    assert!(stdout.contains('\u{1b}'), "the colour helpers must emit an escape sequence");
    assert!(stdout.contains("true\ntrue"), "a terminal with no tty must still report a size");
}

/// `io.format` substitutes `{}` in order, and leaves a placeholder it cannot fill.
///
/// Leaving `{}` rather than dropping it is deliberate: a formatter that silently deletes
/// text is the failure this file keeps finding. The empty array has to be declared —
/// `format("x", [])` types the literal as `[int]` and does not compile — which is the
/// untyped-empty-literal limitation, not a defect in `format`.
#[test]
fn io_format_substitutes_and_leaves_the_rest() {
    if !c_compiler_available() {
        eprintln!("skipping: no host C compiler available (set CC)");
        return;
    }
    let stdout = run_with_stdlib(
        "io_format",
        r#"
use core;
use io;

fn main() -> int {
    let none: [String] = [];
    print(format("{} and {}", ["a", "b"]));
    print(format("no placeholders", none));
    print(format("{}", none));
    print(format("{}-{}-{}", ["x", "y"]));
    print(format("", none));
    print(format("{}{}", ["p", "q"]));
    return 0;
}
"#,
    );
    let expected = [
        "a and b",          // two placeholders, two arguments
        "no placeholders",  // a template with none is copied through
        "{}",               // an argument that is missing is left as written
        "x-y-{}",           // the third is missing, the first two are not
        "",                 // an empty template is an empty string
        "pq",               // adjacent placeholders
    ];
    assert_eq!(stdout.trim_end().lines().collect::<Vec<_>>().join("\n"), expected.join("\n"));
}
