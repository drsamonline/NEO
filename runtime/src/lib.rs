//! **Nothing depends on this crate.**
//!
//! `nova-runtime` is a workspace member that no other crate in the workspace uses: it appears in
//! `[workspace.dependencies]` and in no `[dependencies]` table. The runtime a compiled NOVA
//! program actually runs on is the **C shim** in `compiler/src/runtime.rs` — that is where
//! `nova_strdup`, `nova_array_push`, the file table and every function the backends emit live.
//!
//! So read the modules below as a sketch, not as the implementation:
//!
//! | This crate has | A compiled program gets |
//! |---|---|
//! | `gc.rs` — 23 functions of collector infrastructure | **no collector**; `nova_strdup` leaks by design |
//! | `threading/` — mutexes, RW locks, condition variables | **one thread**; see `docs/language/13_concurrency.md` |
//! | `async_rt/` — futures and channels | **no async**; nothing in the language schedules anything |
//! | `ffi.rs` — `malloc`/`free` wrappers | the C shim calls `malloc` directly |
//! | `io/` — files, streams, directories | `nova_file_*` in the C shim |
//!
//! It is kept because deleting it is a decision rather than a fix, and because the FFI module
//! records how to wrap C safely in this workspace. Until someone decides between wiring it up and
//! removing it, this header is the truth about it — the modules below were written for a design
//! that the compiler does not use.
//!
//! # `unsafe` in this crate
//!
//! Every other crate in the workspace is `#![forbid(unsafe_code)]`. This one
//! is not, because it is the only crate that talks to C: `ffi.rs` wraps
//! `libc::malloc`/`free`/`realloc` and turns raw pointers into references, and
//! none of that can be expressed without `unsafe`. Each block and each
//! `unsafe fn` carries a `# Safety` section or a `// SAFETY:` note stating the
//! obligation it relies on. If a change here can be written without `unsafe`,
//! it should be — `memory.rs` and `error.rs` used `static mut` and now use an
//! `AtomicBool` and a `Mutex` instead.
//! - FFI support foundations
//! - Garbage collection infrastructure
//! - I/O operations (files, streams, directories)
//! - Threading primitives (mutexes, RW locks, condition variables)
//! - Async runtime (futures, channels)
//! 
//! # Status: Phase 2 - Active Development
//! Core runtime modules implemented with GC foundation, I/O, threading, and async support.

// ============================================================================
// Module Declarations
// ============================================================================

pub mod memory;
pub mod types;
pub mod error;
pub mod ffi;
pub mod gc;
pub mod io;
pub mod threading;
pub mod async_rt;

// ============================================================================
// Re-exports
// ============================================================================

pub use memory::*;
pub use types::*;
pub use error::*;
pub use ffi::*;
pub use gc::*;
pub use io::*;
pub use threading::*;
pub use async_rt::*;

// ============================================================================
// Runtime Version
// ============================================================================

/// Runtime version string
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Runtime initialization
/// 
/// Called at program startup to initialize runtime systems.
pub fn init() {
    memory::init();
    error::init();
}

/// Runtime cleanup
/// 
/// Called at program exit to clean up resources.
pub fn shutdown() {
    memory::shutdown();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_init() {
        init();
        // Should not panic
        shutdown();
    }

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
