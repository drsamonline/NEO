//! The NOVA debugger as a library.
//!
//! It was a binary-only crate, so nothing could drive it: no test outside `main.rs` could start a
//! program and read a variable, which is why the live path of `print x` (ADR 0023) went unverified
//! while its parts were each tested. This exposes the modules so a test can do what a user does.
//!
//! The binary is a thin shell over these: `main.rs` parses arguments and calls in.

pub mod commands;
pub mod dwarf;
pub mod session;
pub mod target;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DebuggerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Target process error: {0}")]
    Target(String),

    #[error("Breakpoint error: {0}")]
    Breakpoint(String),

    #[error("Invalid command: {0}")]
    InvalidCommand(String),

    #[error("Process not running")]
    ProcessNotRunning,

    #[error("Failed to attach to process: {0}")]
    AttachFailed(String),

    #[error("Readline error: {0}")]
    Readline(#[from] rustyline::error::ReadlineError),
}

/// A frame, as `backtrace` prints it.
///
/// The `impl` lives here because a `Display` for a type from another crate cannot be written in a
/// binary that depends on it — which is the same reason this file exists.
impl std::fmt::Display for session::StackFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.file, self.line) {
            (Some(file), Some(line)) => {
                write!(f, "{} () at {}:{} (0x{:x})", self.function, file, line, self.address)
            }
            _ => write!(f, "{} () at 0x{:x}", self.function, self.address),
        }
    }
}
