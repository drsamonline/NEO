//! Debug target management
//! 
//! Handles loading binaries, attaching to processes, and remote debugging connections.

use crate::DebuggerError;
use std::path::Path;

/// Represents a debug target
#[derive(Debug)]
pub enum DebugTarget {
    /// Local binary to run
    Binary {
        path: String,
        symbols_loaded: bool,
    },
    /// Attached to running process
    Attached {
        pid: u32,
        process_name: String,
    },
    /// Remote GDB stub connection
    Remote {
        host: String,
        port: u16,
        connected: bool,
    },
}

impl DebugTarget {
    pub fn is_running(&self) -> bool {
        match self {
            DebugTarget::Binary { .. } => false,
            DebugTarget::Attached { .. } => true,
            DebugTarget::Remote { connected, .. } => *connected,
        }
    }
    
    pub fn pid(&self) -> Option<u32> {
        match self {
            DebugTarget::Attached { pid, .. } => Some(*pid),
            _ => None,
        }
    }
}

/// Load a binary for debugging
pub fn load_binary(path: &Path) -> Result<DebugTarget, DebuggerError> {
    if !path.exists() {
        return Err(DebuggerError::Target(
            format!("Binary not found: {:?}", path)
        ));
    }
    
    // Verify it's an executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(path)?;
        if !metadata.is_file() || (metadata.permissions().mode() & 0o111) == 0 {
            return Err(DebuggerError::Target(
                format!("Not an executable: {:?}", path)
            ));
        }
    }
    
    println!("Binary loaded: {:?}", path);
    
    Ok(DebugTarget::Binary {
        path: path.to_string_lossy().to_string(),
        symbols_loaded: false,
    })
}

/// Attach to a running process
pub fn attach(pid: u32) -> Result<DebugTarget, DebuggerError> {
    // `target_os = "linux"`, not `unix`: macOS is unix and has no `PTRACE_ATTACH`, so the `unix`
    // gate compiled a `ptrace` call that `libc` does not define there. Every other function in
    // this file was already gated on Linux; this one was the exception, and it is the one the
    // v0.8.0 release failed on.
    #[cfg(target_os = "linux")]
    {
        // Check if process exists
        let proc_path = std::path::Path::new("/proc").join(pid.to_string());
        if !proc_path.exists() {
            return Err(DebuggerError::AttachFailed(format!(
                "process {pid} does not exist. Nothing was attached"
            )));
        }
        
        let comm_path = proc_path.join("comm");
        let process_name = std::fs::read_to_string(&comm_path)
            .unwrap_or_else(|_| "unknown".to_string())
            .trim()
            .to_string();

        // SAFETY: `ptrace` is variadic and takes pointers whose meaning depends on the request.
        // For `PTRACE_ATTACH` both are ignored, so passing null is correct; the kernel only
        // reads `pid`, and `pid` came from the caller as a `u32`, which is a valid `pid_t`.
        let attached = unsafe {
            libc::ptrace(
                libc::PTRACE_ATTACH,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            )
        };
        if attached == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::AttachFailed(format!(
                "cannot attach to process {pid} ({process_name}): {error}. Nothing was attached"
            )));
        }

        // `PTRACE_ATTACH` sends `SIGSTOP`; the process is not ours to read until that stop is
        // reaped. Without this wait, a register read races the kernel and returns garbage.
        let mut status: libc::c_int = 0;
        // SAFETY: `waitpid` writes one `c_int` through the pointer, which points at a live
        // local of exactly that type, and `pid` is a valid `pid_t`.
        let waited = unsafe { libc::waitpid(pid as libc::pid_t, &mut status, 0) };
        if waited == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::AttachFailed(format!(
                "attached to {pid} but could not wait for it to stop: {error}"
            )));
        }
        if !libc::WIFSTOPPED(status) {
            return Err(DebuggerError::AttachFailed(format!(
                "process {pid} did not stop after attach (status {status})"
            )));
        }

        Ok(DebugTarget::Attached { pid, process_name })
    }
    
    // The same refusal for every other unix: macOS has `ptrace` but not the Linux request
    // constants, and a debugger that cannot attach should say so rather than fail to build.
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let _ = pid;
        Err(DebuggerError::AttachFailed(
            "attaching is implemented with `ptrace` and is Linux-only in this debugger; \
             nothing was attached"
                .to_string(),
        ))
    }

    #[cfg(windows)]
    {
        // Windows would need `DebugActiveProcess`. Refused rather than reported as done.
        Err(DebuggerError::AttachFailed(format!(
            "process {pid}: attaching is not implemented on Windows in this debugger; \
             nothing was attached"
        )))
    }
}

/// Detach from a process this debugger attached to, letting it run again.
pub fn detach(pid: u32) -> Result<(), DebuggerError> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: as in `attach`, `PTRACE_DETACH` ignores both pointers; `pid` is the process
        // this debugger attached to, and detaching a process it did not attach to fails with
        // `EPERM` rather than doing something undefined.
        let detached = unsafe {
            libc::ptrace(
                libc::PTRACE_DETACH,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            )
        };
        if detached == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::Target(format!(
                "cannot detach from {pid}: {error}"
            )));
        }
        return Ok(());
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        Err(DebuggerError::Target(
            "detaching is implemented on Linux only".to_string(),
        ))
    }
}

/// Start a program with tracing enabled, so it stops before its first instruction.
///
/// `PTRACE_TRACEME` is called in the child between `fork` and `exec`, which is the only place it
/// is meaningful; `pre_exec` is the hook for exactly that. The child then stops on the `exec`
/// with `SIGTRAP`, and this function reaps that stop so the caller gets a process it can read.
///
/// Running the program under trace is what makes `step` and `continue` possible at all: a
/// process nobody traced cannot be stopped inside itself.
pub fn spawn_traced(path: &str, args: &[String]) -> Result<u32, DebuggerError> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt;

        let mut command = std::process::Command::new(path);
        command.args(args);
        // SAFETY: `pre_exec` runs in the child after `fork` and before `exec`, where only
        // async-signal-safe work is permitted. This closure calls `ptrace`, which is
        // async-signal-safe, and returns an `io::Error` built from `errno` — no allocation, no
        // locks, no Rust runtime. Returning `Err` aborts the exec and the parent sees a failed
        // spawn rather than a child that was never traced.
        unsafe {
            command.pre_exec(|| {
                if libc::ptrace(
                    libc::PTRACE_TRACEME,
                    0,
                    std::ptr::null_mut::<libc::c_void>(),
                    std::ptr::null_mut::<libc::c_void>(),
                ) == -1
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }

        let child = command.spawn().map_err(|e| {
            DebuggerError::Target(format!("cannot start {path}: {e}"))
        })?;
        let pid = child.id();
        // The `exec` stop. Without reaping it, the first `PTRACE_CONT` would race it and the
        // program would appear to start twice.
        let mut status: libc::c_int = 0;
        // SAFETY: writes one `c_int` through a pointer to a live local of that type; `pid` is a
        // valid `pid_t` for a child this process just created.
        let waited = unsafe { libc::waitpid(pid as libc::pid_t, &mut status, 0) };
        if waited == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::Target(format!(
                "started {path} but could not wait for its first stop: {error}"
            )));
        }
        if !libc::WIFSTOPPED(status) {
            return Err(DebuggerError::Target(format!(
                "{path} did not stop on start (status {status}); it may have exited immediately"
            )));
        }
        Ok(pid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (path, args);
        Err(DebuggerError::Target(
            "running under trace is implemented on Linux only".to_string(),
        ))
    }
}

/// The address of a function in a binary, from its symbol table.
///
/// `addr2line` maps an address to a line and cannot go the other way, so a breakpoint on a
/// function is resolved with `nm`. NOVA mangles its own functions as `nova_fn_<name>` (see the
/// naming note in the lowerer), and `main` stays `main`, so both spellings are tried.
pub fn resolve_function(binary: &str, name: &str) -> Result<u64, DebuggerError> {
    resolve_function_in(binary, name, None)
}

/// As `resolve_function`, with the load base applied for a position-independent binary.
pub fn resolve_function_in(
    binary: &str,
    name: &str,
    pid: Option<u32>,
) -> Result<u64, DebuggerError> {
    // A position-independent executable loads at an address the loader chooses, so a symbol
    // value from `nm` is an offset and not an address. Writing `int3` at an offset would
    // corrupt whatever happens to live there — so this refuses rather than guessing a base.
    // NOVA's own builds are non-PIE (`ELF ... executable`), which is why the simple case works.
    // A PIE's symbols are offsets from a load base the kernel chose, so the base has to be
    // added — and it can only be read from a running process. Without a process there is no
    // base, and writing at an offset would corrupt whatever lives there.
    let bias = match elf_type(binary)? {
        3 => match pid {
            Some(pid) => load_bias(pid, binary)?,
            None => {
                return Err(DebuggerError::Target(format!(
                    "{binary} is position-independent, so its symbols are offsets from a load \
                     base the kernel chooses. Start the program first — `run`, then `break` — \
                     and the base can be read from /proc/<pid>/maps"
                )))
            }
        },
        _ => 0,
    };

    let output = std::process::Command::new("nm")
        .arg(binary)
        .output()
        .map_err(|e| {
            DebuggerError::Target(format!("cannot run nm: {e} (install binutils)"))
        })?;
    if !output.status.success() {
        return Err(DebuggerError::Target(format!(
            "nm could not read {binary}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let wanted = [name.to_string(), format!("nova_fn_{name}")];
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        // "0000000001026330 T main"
        let mut parts = line.split_whitespace();
        let (Some(address), Some(kind), Some(symbol)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        // `T`/`t` are text symbols; anything else is not code we can break on.
        if !kind.eq_ignore_ascii_case("t") {
            continue;
        }
        if wanted.iter().any(|w| w == symbol) {
            let offset = u64::from_str_radix(address, 16).map_err(|_| {
                DebuggerError::Target(format!("unreadable symbol address `{address}` for {symbol}"))
            })?;
            return Ok(offset + bias);
        }
    }
    Err(DebuggerError::Target(format!(
        "no function `{name}` in {binary}: tried `{name}` and `nova_fn_{name}`. `nm {binary}` \
         lists what is there"
    )))
}

/// The ELF `e_type`: 2 is `ET_EXEC` (fixed addresses), 3 is `ET_DYN` (position-independent).
fn elf_type(path: &str) -> Result<u16, DebuggerError> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .map_err(|e| DebuggerError::Target(format!("cannot open {path}: {e}")))?;
    let mut header = [0u8; 18];
    file.read_exact(&mut header)
        .map_err(|e| DebuggerError::Target(format!("cannot read {path}: {e}")))?;
    if &header[0..4] != b"\x7fELF" {
        return Err(DebuggerError::Target(format!("{path} is not an ELF file")));
    }
    // `e_type` is a 16-bit field at offset 16, in the file's own endianness.
    let little = header[5] == 1;
    Ok(if little {
        u16::from_le_bytes([header[16], header[17]])
    } else {
        u16::from_be_bytes([header[16], header[17]])
    })
}

/// Where a position-independent binary was actually loaded.
///
/// A PIE's symbol values are offsets from the load base the kernel chose. `/proc/<pid>/maps`
/// gives the mappings; the one whose file offset is 0 is the base. Without this, a symbol value
/// is an offset and writing a breakpoint there corrupts whatever happens to live at it.
pub fn load_bias(pid: u32, binary: &str) -> Result<u64, DebuggerError> {
    let maps = std::fs::read_to_string(format!("/proc/{pid}/maps")).map_err(|e| {
        DebuggerError::Target(format!("cannot read /proc/{pid}/maps: {e}"))
    })?;
    let canonical = std::fs::canonicalize(binary)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| binary.to_string());
    let mut lowest: Option<u64> = None;
    for line in maps.lines() {
        // "5615e8776000-5615e8778000 r--p 00000000 00:2a 1714   /usr/bin/head"
        let mut fields = line.split_whitespace();
        let (Some(range), Some(_perms), Some(offset)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let Some(path) = line.split_whitespace().nth(5) else {
            continue;
        };
        if path != canonical && path != binary {
            continue;
        }
        if offset != "00000000" {
            continue;
        }
        let Some((start, _end)) = range.split_once('-') else {
            continue;
        };
        if let Ok(start) = u64::from_str_radix(start, 16) {
            lowest = Some(lowest.map_or(start, |current: u64| current.min(start)));
        }
    }
    lowest.ok_or_else(|| {
        DebuggerError::Target(format!(
            "{binary} is not mapped in process {pid}: it may have exited, or the path differs"
        ))
    })
}

/// The address of a source line, from the binary's line table.
///
/// `addr2line` goes address → line; this goes the other way by **scanning** the addresses the
/// line table covers, which is how `gdb` resolves a `file:line` breakpoint too. The scan is one
/// subprocess, not one per address: `addr2line` takes a list.
///
/// `readelf --debug-dump=decodedline` lists the addresses but reports the *compilation unit's*
/// file for every row, so a `#line`-remapped name never appears there — measured: a binary whose
/// `addr2line` answers `/tmp/nv/fn.nova:3` has zero `.nova` rows in `decodedline`. The addresses
/// are still right, so they are the input and `addr2line` is the judge.
pub fn resolve_line(
    binary: &str,
    file: &str,
    line: usize,
    pid: Option<u32>,
) -> Result<u64, DebuggerError> {
    let bias = match elf_type(binary)? {
        3 => match pid {
            Some(pid) => load_bias(pid, binary)?,
            None => {
                return Err(DebuggerError::Target(format!(
                    "{binary} is position-independent, so a line resolves to an offset. Start \
                     the program first — `run`, then `break` — and the base can be read from \
                     /proc/<pid>/maps"
                )))
            }
        },
        _ => 0,
    };

    let dump = std::process::Command::new("readelf")
        .arg("--debug-dump=decodedline")
        .arg(binary)
        .output()
        .map_err(|e| DebuggerError::Target(format!("cannot run readelf: {e} (install binutils)")))?;
    if !dump.status.success() {
        return Err(DebuggerError::Target(format!(
            "readelf could not read {binary}: {}",
            String::from_utf8_lossy(&dump.stderr).trim()
        )));
    }

    // Every address the line table covers, in order and without repeats.
    let text = String::from_utf8_lossy(&dump.stdout);
    let mut addresses: Vec<u64> = Vec::new();
    for row in text.lines() {
        let mut fields = row.split_whitespace();
        let (Some(_name), Some(_number), Some(address)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if !address.starts_with("0x") {
            continue;
        }
        if let Ok(value) = u64::from_str_radix(&address[2..], 16) {
            if addresses.last() != Some(&value) {
                addresses.push(value);
            }
        }
    }
    if addresses.is_empty() {
        return Err(DebuggerError::Target(format!(
            "{binary} has no line table: it may have been built without `-g`"
        )));
    }

    let mut command = std::process::Command::new("addr2line");
    command.arg("-e").arg(binary).arg("-f").arg("-C");
    for address in &addresses {
        command.arg(format!("0x{address:x}"));
    }
    let resolved = command
        .output()
        .map_err(|e| DebuggerError::Target(format!("cannot run addr2line: {e}")))?;
    let resolved = String::from_utf8_lossy(&resolved.stdout);
    let mut lines = resolved.lines();

    let wanted = std::path::Path::new(file)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| file.to_string());
    for address in &addresses {
        // Two lines per address: the function, then `file:line`.
        let _function = lines.next();
        let Some(location) = lines.next() else {
            break;
        };
        let Some((found_file, found_line)) = location.rsplit_once(':') else {
            continue;
        };
        if found_line.parse::<usize>() != Ok(line) {
            continue;
        }
        let found_name = std::path::Path::new(found_file)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| found_file.to_string());
        if found_name == wanted {
            return Ok(address + bias);
        }
    }
    Err(DebuggerError::Target(format!(
        "no code for {file}:{line} in {binary}. The line may be a comment, a blank line, or \
         outside the program; `list` shows what is there"
    )))
}

/// Write an `int3` at `address`, returning the byte that was there.
///
/// A breakpoint on x86-64 is the one-byte trap instruction `0xCC`. The original byte must be
/// kept, because leaving `int3` behind changes the program: the next execution of that
/// instruction would trap again, forever.
pub fn insert_breakpoint(pid: u32, address: u64) -> Result<u8, DebuggerError> {
    #[cfg(target_os = "linux")]
    {
        let word = peek_word(pid, address)?;
        let original = (word & 0xff) as u8;
        let patched = (word & !0xff) | libc::c_long::from(libc::c_uchar::from(0xccu8));
        poke_word(pid, address, patched)?;
        Ok(original)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (pid, address);
        Err(DebuggerError::Target(
            "breakpoints are implemented on Linux only".to_string(),
        ))
    }
}

/// Put the original byte back, so the program runs as it was compiled.
pub fn remove_breakpoint(pid: u32, address: u64, original: u8) -> Result<(), DebuggerError> {
    #[cfg(target_os = "linux")]
    {
        let word = peek_word(pid, address)?;
        let restored = (word & !0xff) | libc::c_long::from(original);
        poke_word(pid, address, restored)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (pid, address, original);
        Err(DebuggerError::Target(
            "breakpoints are implemented on Linux only".to_string(),
        ))
    }
}

#[cfg(target_os = "linux")]
fn peek_word(pid: u32, address: u64) -> Result<libc::c_long, DebuggerError> {
    // SAFETY: `PTRACE_PEEKTEXT` reads one word from the tracee's address space and returns it as
    // the `ptrace` return value; the data pointer is ignored. `pid` is a process this debugger
    // is tracing. A word that cannot be read is reported through `errno`, not silently as -1.
    let word = unsafe {
        libc::ptrace(
            libc::PTRACE_PEEKTEXT,
            pid as libc::pid_t,
            address as *mut libc::c_void,
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    if word == -1 {
        let error = std::io::Error::last_os_error();
        // `-1` is also a legitimate word, so `errno` is what distinguishes them.
        if error.raw_os_error() != Some(0) {
            return Err(DebuggerError::Target(format!(
                "cannot read memory at 0x{address:x} of {pid}: {error}"
            )));
        }
    }
    Ok(word)
}

#[cfg(target_os = "linux")]
fn poke_word(pid: u32, address: u64, word: libc::c_long) -> Result<(), DebuggerError> {
    // SAFETY: `PTRACE_POKETEXT` writes the word given as `data` at `address` in the tracee;
    // `address` came from that tracee's own symbol table and `pid` is a process this debugger
    // is tracing.
    let written = unsafe {
        libc::ptrace(
            libc::PTRACE_POKETEXT,
            pid as libc::pid_t,
            address as *mut libc::c_void,
            word as *mut libc::c_void,
        )
    };
    if written == -1 {
        let error = std::io::Error::last_os_error();
        return Err(DebuggerError::Target(format!(
            "cannot write memory at 0x{address:x} of {pid}: {error}"
        )));
    }
    Ok(())
}

/// Resume a stopped process: to the next stop, or one instruction.
///
/// Returns `(stopped, signal, exited_code)`. `stopped` is false when the process ended, and then
/// `exited_code` is its status — the caller must report that rather than a location.
pub fn resume(pid: u32, single_step: bool) -> Result<Stop, DebuggerError> {
    #[cfg(target_os = "linux")]
    {
        let request = if single_step {
            libc::PTRACE_SINGLESTEP
        } else {
            libc::PTRACE_CONT
        };
        // SAFETY: as in `attach`, both data pointers are ignored by these requests; `pid` is a
        // process this debugger is tracing.
        let resumed = unsafe {
            libc::ptrace(
                request,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                std::ptr::null_mut::<libc::c_void>(),
            )
        };
        if resumed == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::Target(format!(
                "cannot resume {pid}: {error}. Is it still traced?"
            )));
        }
        let mut status: libc::c_int = 0;
        // SAFETY: as in `attach`, writes one `c_int` through a pointer to a live local.
        let waited = unsafe { libc::waitpid(pid as libc::pid_t, &mut status, 0) };
        if waited == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::Target(format!("cannot wait for {pid}: {error}")));
        }
        if libc::WIFEXITED(status) {
            return Ok(Stop::Exited(libc::WEXITSTATUS(status)));
        }
        if libc::WIFSIGNALED(status) {
            return Ok(Stop::Signalled(libc::WTERMSIG(status)));
        }
        if libc::WIFSTOPPED(status) {
            return Ok(Stop::Stopped(libc::WSTOPSIG(status)));
        }
        Err(DebuggerError::Target(format!(
            "unexpected wait status {status} for {pid}"
        )))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (pid, single_step);
        Err(DebuggerError::Target(
            "resuming is implemented on Linux only".to_string(),
        ))
    }
}

/// The frame pointer of a stopped process, and the two words a stack walk needs.
///
/// x86-64 frame layout: `[rbp]` is the caller's `rbp`, `[rbp+8]` is the return address. Walking
/// that chain is the simplest backtrace there is, and it works only because the compiler was
/// told to keep a frame pointer (`-fno-omit-frame-pointer`).
pub fn frame_pointer(pid: u32) -> Result<u64, DebuggerError> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        let registers = read_registers(pid)?;
        Ok(registers.rbp)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = pid;
        Err(DebuggerError::Target(
            "reading the frame pointer is implemented for x86-64 Linux only".to_string(),
        ))
    }
}

/// One word of a stopped process's memory.
pub fn read_word(pid: u32, address: u64) -> Result<u64, DebuggerError> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: `PTRACE_PEEKTEXT` reads one word from the tracee and returns it; the data
        // pointer is ignored. `pid` is a process this debugger is tracing, and a failed read is
        // distinguished from a word that happens to be -1 by `errno`.
        let word = unsafe {
            libc::ptrace(
                libc::PTRACE_PEEKTEXT,
                pid as libc::pid_t,
                address as *mut libc::c_void,
                std::ptr::null_mut::<libc::c_void>(),
            )
        };
        if word == -1 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(0) {
                return Err(DebuggerError::Target(format!(
                    "cannot read 0x{address:x} of {pid}: {error}"
                )));
            }
        }
        Ok(word as u64)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (pid, address);
        Err(DebuggerError::Target(
            "reading memory is implemented on Linux only".to_string(),
        ))
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn read_registers(pid: u32) -> Result<libc::user_regs_struct, DebuggerError> {
    // SAFETY: writes the register block into a live local of exactly the type the kernel
    // writes; `pid` is a process this debugger is tracing.
    let mut registers: libc::user_regs_struct = unsafe { std::mem::zeroed() };
    let read = unsafe {
        libc::ptrace(
            libc::PTRACE_GETREGS,
            pid as libc::pid_t,
            std::ptr::null_mut::<libc::c_void>(),
            &mut registers as *mut libc::user_regs_struct as *mut libc::c_void,
        )
    };
    if read == -1 {
        let error = std::io::Error::last_os_error();
        return Err(DebuggerError::Target(format!(
            "cannot read registers of {pid}: {error}"
        )));
    }
    Ok(registers)
}

/// Move a stopped process's instruction pointer, so a trap can be undone.
///
/// After an `int3` the process is stopped **one byte past** the instruction it should execute
/// next. Putting `rip` back is what makes the breakpoint invisible to the program.
pub fn set_instruction_pointer(pid: u32, address: u64) -> Result<(), DebuggerError> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        // SAFETY: reads the register block into a live local, modifies one field, and writes it
        // back through a pointer of the same type. `pid` is a process this debugger traces.
        let mut registers: libc::user_regs_struct = unsafe { std::mem::zeroed() };
        let read = unsafe {
            libc::ptrace(
                libc::PTRACE_GETREGS,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                &mut registers as *mut libc::user_regs_struct as *mut libc::c_void,
            )
        };
        if read == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::Target(format!("cannot read registers of {pid}: {error}")));
        }
        registers.rip = address;
        let written = unsafe {
            libc::ptrace(
                libc::PTRACE_SETREGS,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                &registers as *const libc::user_regs_struct as *mut libc::c_void,
            )
        };
        if written == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::Target(format!("cannot write registers of {pid}: {error}")));
        }
        Ok(())
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (pid, address);
        Err(DebuggerError::Target(
            "moving the instruction pointer is implemented for x86-64 Linux only".to_string(),
        ))
    }
}

/// How a traced process came to rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// Stopped by a signal; `SIGTRAP` is a breakpoint or a completed single step.
    Stopped(i32),
    /// Exited with this code.
    Exited(i32),
    /// Killed by this signal.
    Signalled(i32),
}

/// The instruction pointer of a stopped process — where it is right now.
///
/// x86-64 only. A stopped process's `rip` is the address of the instruction it will execute
/// next, which is exactly what `addr2line` needs to name a source line.
pub fn instruction_pointer(pid: u32) -> Result<u64, DebuggerError> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        // SAFETY: `user_regs_struct` is plain data; zeroing it is a valid initial value and
        // `PTRACE_GETREGS` overwrites it. The pointer is to a live local of the exact type the
        // kernel writes, and `pid` is a process this debugger attached to.
        let mut registers: libc::user_regs_struct = unsafe { std::mem::zeroed() };
        let read = unsafe {
            libc::ptrace(
                libc::PTRACE_GETREGS,
                pid as libc::pid_t,
                std::ptr::null_mut::<libc::c_void>(),
                &mut registers as *mut libc::user_regs_struct as *mut libc::c_void,
            )
        };
        if read == -1 {
            let error = std::io::Error::last_os_error();
            return Err(DebuggerError::Target(format!(
                "cannot read registers of {pid}: {error}. Is it attached and stopped?"
            )));
        }
        Ok(registers.rip)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = pid;
        Err(DebuggerError::Target(
            "reading the instruction pointer is implemented for x86-64 Linux only".to_string(),
        ))
    }
}

/// Connect to a remote GDB stub
pub fn connect_remote(address: &str) -> Result<DebugTarget, DebuggerError> {
    let parts: Vec<&str> = address.split(':').collect();
    
    if parts.len() != 2 {
        return Err(DebuggerError::Target(
            "Remote address must be in format host:port".to_string()
        ));
    }
    
    let host = parts[0].to_string();
    let port: u16 = parts[1].parse().map_err(|_| {
        DebuggerError::Target("Invalid port number".to_string())
    })?;
    
    // No socket is opened here. This used to print "Connected to remote target host:port" and
    // return `connected: true`, so the debugger reported a connection that did not exist and
    // then had no way to use one.
    Err(DebuggerError::Target(format!(
        "cannot connect to {host}:{port}: the remote GDB stub protocol is not implemented. \
         Nothing was connected"
    )))
}

/// Get process memory map (platform-specific)
pub fn get_memory_map(pid: u32) -> Result<Vec<MemoryRegion>, DebuggerError> {
    #[cfg(unix)]
    {
        let maps_path = std::path::Path::new("/proc")
            .join(pid.to_string())
            .join("maps");
        
        let content = std::fs::read_to_string(&maps_path)?;
        let mut regions = Vec::new();
        
        for line in content.lines() {
            // Parse /proc/pid/maps format
            // Example: 00400000-00452000 r-xp 00000000 08:02 123456 /path/to/binary
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 6 {
                let addr_range: Vec<&str> = parts[0].split('-').collect();
                if addr_range.len() == 2 {
                    let start = u64::from_str_radix(addr_range[0], 16).unwrap_or(0);
                    let end = u64::from_str_radix(addr_range[1], 16).unwrap_or(0);
                    let perms = parts[1].to_string();
                    let path = parts.get(5).map(|s| s.to_string()).unwrap_or_default();
                    
                    regions.push(MemoryRegion {
                        start,
                        end,
                        permissions: perms,
                        path,
                    });
                }
            }
        }
        
        Ok(regions)
    }
    
    #[cfg(not(unix))]
    {
        Ok(vec![])
    }
}

/// Memory region information
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    pub start: u64,
    pub end: u64,
    pub permissions: String,
    pub path: String,
}

impl MemoryRegion {
    pub fn size(&self) -> u64 {
        self.end - self.start
    }
    
    pub fn is_readable(&self) -> bool {
        self.permissions.contains('r')
    }
    
    pub fn is_writable(&self) -> bool {
        self.permissions.contains('w')
    }
    
    pub fn is_executable(&self) -> bool {
        self.permissions.contains('x')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_target_is_running() {
        let binary = DebugTarget::Binary {
            path: "/tmp/test".to_string(),
            symbols_loaded: false,
        };
        assert!(!binary.is_running());
        
        let attached = DebugTarget::Attached {
            pid: 1234,
            process_name: "test".to_string(),
        };
        assert!(attached.is_running());
    }
    
    #[test]
    fn test_target_pid() {
        let binary = DebugTarget::Binary {
            path: "/tmp/test".to_string(),
            symbols_loaded: false,
        };
        assert_eq!(binary.pid(), None);
        
        let attached = DebugTarget::Attached {
            pid: 1234,
            process_name: "test".to_string(),
        };
        assert_eq!(attached.pid(), Some(1234));
    }
    
    #[test]
    fn test_memory_region_permissions() {
        let region = MemoryRegion {
            start: 0x400000,
            end: 0x452000,
            permissions: "r-xp".to_string(),
            path: "/bin/test".to_string(),
        };
        
        assert!(region.is_readable());
        assert!(!region.is_writable());
        assert!(region.is_executable());
        assert_eq!(region.size(), 0x52000);
    }
}
