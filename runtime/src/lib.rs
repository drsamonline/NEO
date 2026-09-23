//! NOVA Runtime System
//! 
//! Provides core runtime support for compiled NOVA programs.
//! 
//! # Features
//! - Memory management utilities
//! - Basic type implementations
//! - Runtime error handling
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
