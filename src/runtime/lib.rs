// NOVA Runtime System
// Core runtime library for NOVA language

pub mod memory;
pub mod types;
pub mod error;
pub mod ffi;

pub use memory::{MemoryArena, GarbageCollector, ManagedPtr, ArenaStats};
pub use types::{TypeId, TypeInfo, TypeKind, FieldInfo, VariantInfo, TypeRegistry, DynamicValue};
pub use error::{RuntimeError, RuntimeErrorWithTrace, RuntimeResult, init_runtime, has_panicked, reset_panic_flag, catch_unwind};
pub use ffi::{NovaValue, FfiString, FfiArray, FfiHandle, FfiFunction, init_ffi};

/// Runtime version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const VERSION_MAJOR: u32 = 0;
pub const VERSION_MINOR: u32 = 1;
pub const VERSION_PATCH: u32 = 0;

/// Initialize the NOVA runtime system
pub fn init() {
    init_runtime();
    init_ffi();
}

/// Get runtime statistics
pub fn get_stats() -> RuntimeStats {
    RuntimeStats {
        version: VERSION.to_string(),
        memory_allocated: memory::ALLOCATED_BYTES.load(std::sync::atomic::Ordering::Relaxed),
        allocation_count: memory::ALLOCATION_COUNT.load(std::sync::atomic::Ordering::Relaxed),
        gc_runs: memory::GC_RUNS.load(std::sync::atomic::Ordering::Relaxed),
        type_count: types::TypeRegistry::new().count(),
        has_panicked: has_panicked(),
    }
}

/// Runtime statistics structure
#[derive(Debug, Clone)]
pub struct RuntimeStats {
    pub version: String,
    pub memory_allocated: usize,
    pub allocation_count: usize,
    pub gc_runs: usize,
    pub type_count: usize,
    pub has_panicked: bool,
}

impl std::fmt::Display for RuntimeStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "NOVA Runtime Statistics")?;
        writeln!(f, "=======================")?;
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f, "Memory Allocated: {} bytes", self.memory_allocated)?;
        writeln!(f, "Allocation Count: {}", self.allocation_count)?;
        writeln!(f, "GC Runs: {}", self.gc_runs)?;
        writeln!(f, "Registered Types: {}", self.type_count)?;
        writeln!(f, "Panics Occurred: {}", self.has_panicked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_runtime_init() {
        init();
        assert!(!has_panicked());
    }
    
    #[test]
    fn test_runtime_stats() {
        let stats = get_stats();
        assert_eq!(stats.version, VERSION);
        assert!(stats.version.starts_with("0."));
    }
    
    #[test]
    fn test_memory_arena_integration() {
        let mut arena = MemoryArena::new(2048).unwrap();
        
        let ptr1 = arena.allocate(100i32).unwrap();
        let ptr2 = arena.allocate(200i32).unwrap();
        
        unsafe {
            assert_eq!(*ptr1, 100);
            assert_eq!(*ptr2, 200);
        }
        
        let stats = arena.stats();
        assert!(stats.allocation_count >= 2);
    }
    
    #[test]
    fn test_type_registry_integration() {
        let registry = TypeRegistry::new();
        
        // Verify primitives are registered
        assert!(registry.get_by_name("i32").is_some());
        assert!(registry.get_by_name("f64").is_some());
        
        // Register custom type
        let custom_type = TypeInfo::new("Custom", TypeKind::Struct, 32, 8);
        registry.register(custom_type).unwrap();
        
        assert!(registry.get_by_name("Custom").is_some());
    }
    
    #[test]
    fn test_error_handling_integration() {
        init();
        
        let result = catch_unwind(|| {
            42
        });
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
    
    #[test]
    fn test_ffi_value_roundtrip() {
        let int_val = ffi::NovaValue::new_int(42);
        unsafe {
            assert_eq!(int_val.as_int(), 42);
        }
        
        let float_val = ffi::NovaValue::new_float(3.14159);
        unsafe {
            assert!((float_val.as_float() - 3.14159).abs() < f64::EPSILON);
        }
        
        let bool_val = ffi::NovaValue::new_bool(true);
        unsafe {
            assert_eq!(bool_val.as_bool(), true);
        }
    }
}
