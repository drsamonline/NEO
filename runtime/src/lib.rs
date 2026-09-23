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
//! 
//! # Status: Phase 2 - Active Development
//! Core runtime modules implemented with GC foundation added.

// ============================================================================
// Module Declarations
// ============================================================================

pub mod memory;
pub mod types;
pub mod error;
pub mod ffi;
pub mod gc;

// ============================================================================
// Re-exports
// ============================================================================

pub use memory::*;
pub use types::*;
pub use error::*;
pub use ffi::*;
pub use gc::*;

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
