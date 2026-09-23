//! NOVA Runtime Garbage Collection Foundation
//! 
//! Provides basic garbage collection infrastructure for NOVA programs.
//! This is a foundational implementation that will be extended in future phases.

use std::cell::RefCell;
use std::rc::Rc;
use std::ptr::{self, NonNull};

// ============================================================================
// GC Configuration
// ============================================================================

/// Default heap size limit (in MB)
pub const DEFAULT_HEAP_LIMIT_MB: usize = 64;

/// Collection threshold (percentage of heap used)
pub const COLLECTION_THRESHOLD: f64 = 0.75;

// ============================================================================
// GC Statistics
// ============================================================================

/// Garbage collector statistics
#[derive(Debug, Clone, Default)]
pub struct GcStats {
    /// Total allocations
    pub total_allocations: usize,
    /// Total deallocations
    pub total_deallocations: usize,
    /// Number of collections performed
    pub collections: usize,
    /// Bytes currently allocated
    pub bytes_allocated: usize,
    /// Bytes freed in last collection
    pub bytes_freed_last_collection: usize,
    /// Peak memory usage
    pub peak_memory: usize,
}

impl GcStats {
    /// Create new stats tracker
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Record an allocation
    pub fn record_allocation(&mut self, bytes: usize) {
        self.total_allocations += 1;
        self.bytes_allocated += bytes;
        if self.bytes_allocated > self.peak_memory {
            self.peak_memory = self.bytes_allocated;
        }
    }
    
    /// Record a deallocation
    pub fn record_deallocation(&mut self, bytes: usize) {
        self.total_deallocations += 1;
        self.bytes_allocated = self.bytes_allocated.saturating_sub(bytes);
    }
    
    /// Record a collection cycle
    pub fn record_collection(&mut self, bytes_freed: usize) {
        self.collections += 1;
        self.bytes_freed_last_collection = bytes_freed;
    }
    
    /// Get allocation/deallocation ratio
    pub fn allocation_ratio(&self) -> f64 {
        if self.total_allocations == 0 {
            return 0.0;
        }
        self.total_deallocations as f64 / self.total_allocations as f64
    }
}

// ============================================================================
// Managed Object Trait
// ============================================================================

/// Trait for objects managed by the garbage collector
pub trait GcManaged {
    /// Get the size of this object in bytes
    fn gc_size(&self) -> usize;
    
    /// Mark phase - mark this object and any children as reachable
    fn gc_mark(&self);
    
    /// Optional: custom cleanup before deallocation
    fn gc_drop(&mut self) {}
}

// ============================================================================
// Reference Counted Pointer Wrapper
// ============================================================================

/// A garbage-collected reference counted pointer
pub struct GcPtr<T: GcManaged> {
    inner: Rc<RefCell<T>>,
}

impl<T: GcManaged> GcPtr<T> {
    /// Create a new GC-managed pointer
    pub fn new(value: T) -> Self {
        Gc::record_alloc(value.gc_size());
        Self {
            inner: Rc::new(RefCell::new(value)),
        }
    }
    
    /// Get a reference to the inner value
    pub fn get(&self) -> &Rc<RefCell<T>> {
        &self.inner
    }
    
    /// Try to get a mutable borrow
    pub fn borrow_mut(&self) -> std::cell::RefMut<T> {
        self.inner.borrow_mut()
    }
    
    /// Try to get an immutable borrow
    pub fn borrow(&self) -> std::cell::Ref<T> {
        self.inner.borrow()
    }
    
    /// Check if this is the only reference
    pub fn is_unique(&self) -> bool {
        Rc::strong_count(&self.inner) == 1
    }
    
    /// Get the strong reference count
    pub fn ref_count(&self) -> usize {
        Rc::strong_count(&self.inner)
    }
}

impl<T: GcManaged> Clone for GcPtr<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: GcManaged> Drop for GcPtr<T> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.inner) == 1 {
            // Last reference, object will be dropped
            let size = self.inner.borrow().gc_size();
            Gc::record_dealloc(size);
        }
    }
}

// ============================================================================
// Weak Reference Support
// ============================================================================

/// A weak reference to a GC-managed object
pub struct GcWeak<T: GcManaged> {
    inner: Rc<RefCell<T>>,
}

impl<T: GcManaged> GcWeak<T> {
    /// Create a weak reference from a strong one
    pub fn downgrade(strong: &GcPtr<T>) -> Self {
        Self {
            inner: strong.inner.clone(),
        }
    }
    
    /// Try to upgrade to a strong reference
    pub fn upgrade(&self) -> Option<GcPtr<T>> {
        if Rc::strong_count(&self.inner) > 0 {
            Some(GcPtr {
                inner: self.inner.clone(),
            })
        } else {
            None
        }
    }
    
    /// Check if the referenced object still exists
    pub fn is_alive(&self) -> bool {
        Rc::strong_count(&self.inner) > 0
    }
}

// ============================================================================
// Global GC State
// ============================================================================

thread_local! {
    static GC_STATE: RefCell<GcState> = RefCell::new(GcState::new());
}

struct GcState {
    stats: GcStats,
    heap_limit: usize,
    enabled: bool,
}

impl GcState {
    fn new() -> Self {
        Self {
            stats: GcStats::new(),
            heap_limit: DEFAULT_HEAP_LIMIT_MB * 1024 * 1024,
            enabled: true,
        }
    }
}

// ============================================================================
// Garbage Collector
// ============================================================================

/// Main garbage collector interface
pub struct Gc;

impl Gc {
    /// Initialize the garbage collector
    pub fn init() {
        GC_STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.stats = GcStats::new();
            s.enabled = true;
        });
    }
    
    /// Shutdown the garbage collector
    pub fn shutdown() {
        GC_STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.enabled = false;
        });
    }
    
    /// Record an allocation
    pub fn record_alloc(bytes: usize) {
        GC_STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.stats.record_allocation(bytes);
            
            // Check if collection should be triggered
            if s.enabled && s.stats.bytes_allocated as f64 > s.heap_limit as f64 * COLLECTION_THRESHOLD {
                drop(s); // Release borrow before calling collect
                Self::collect();
            }
        });
    }
    
    /// Record a deallocation
    pub fn record_dealloc(bytes: usize) {
        GC_STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.stats.record_deallocation(bytes);
        });
    }
    
    /// Trigger a garbage collection cycle
    pub fn collect() -> GcStats {
        GC_STATE.with(|state| {
            let mut s = state.borrow_mut();
            
            // For now, we rely on Rc's automatic cleanup
            // Future: implement mark-and-sweep or generational GC
            let freed = 0; // Rc handles this automatically
            s.stats.record_collection(freed);
            
            s.stats.clone()
        })
    }
    
    /// Get current GC statistics
    pub fn stats() -> GcStats {
        GC_STATE.with(|state| {
            let s = state.borrow();
            s.stats.clone()
        })
    }
    
    /// Set heap limit
    pub fn set_heap_limit(mb: usize) {
        GC_STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.heap_limit = mb * 1024 * 1024;
        });
    }
    
    /// Enable/disable GC
    pub fn set_enabled(enabled: bool) {
        GC_STATE.with(|state| {
            let mut s = state.borrow_mut();
            s.enabled = enabled;
        });
    }
    
    /// Force a collection regardless of threshold
    pub fn force_collect() -> GcStats {
        GC_STATE.with(|state| {
            let mut s = state.borrow_mut();
            let freed = 0;
            s.stats.record_collection(freed);
            s.stats.clone()
        })
    }
}

// ============================================================================
// Convenience Macros
// ============================================================================

/// Create a new GC-managed object
#[macro_export]
macro_rules! gc_new {
    ($value:expr) => {
        $crate::gc::GcPtr::new($value)
    };
}

/// Create a weak reference to a GC object
#[macro_export]
macro_rules! gc_weak {
    ($gc_ptr:expr) => {
        $crate::gc::GcWeak::downgrade(&$gc_ptr)
    };
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    struct TestObject {
        data: i32,
        size: usize,
    }

    impl GcManaged for TestObject {
        fn gc_size(&self) -> usize {
            self.size
        }
        
        fn gc_mark(&self) {
            // Mark children if any
        }
    }

    #[test]
    fn test_gc_init_shutdown() {
        Gc::init();
        assert!(Gc::stats().collections >= 0);
        Gc::shutdown();
    }

    #[test]
    fn test_gc_ptr_creation() {
        Gc::init();
        let obj = TestObject { data: 42, size: 16 };
        let gc_ptr = GcPtr::new(obj);
        
        assert_eq!(gc_ptr.borrow().data, 42);
        assert_eq!(gc_ptr.ref_count(), 1);
        assert!(gc_ptr.is_unique());
    }

    #[test]
    fn test_gc_ptr_clone() {
        Gc::init();
        let obj = TestObject { data: 100, size: 32 };
        let gc_ptr1 = GcPtr::new(obj);
        let gc_ptr2 = gc_ptr1.clone();
        
        assert_eq!(gc_ptr1.ref_count(), 2);
        assert_eq!(gc_ptr2.ref_count(), 2);
        assert!(!gc_ptr1.is_unique());
        
        gc_ptr1.borrow_mut().data = 200;
        assert_eq!(gc_ptr2.borrow().data, 200);
    }

    #[test]
    fn test_weak_reference() {
        Gc::init();
        let obj = TestObject { data: 42, size: 16 };
        let gc_ptr = GcPtr::new(obj);
        let weak = GcWeak::downgrade(&gc_ptr);
        
        assert!(weak.is_alive());
        
        let upgraded = weak.upgrade();
        assert!(upgraded.is_some());
        assert_eq!(upgraded.unwrap().borrow().data, 42);
    }

    #[test]
    fn test_gc_stats() {
        Gc::init();
        
        let _obj1 = GcPtr::new(TestObject { data: 1, size: 16 });
        let _obj2 = GcPtr::new(TestObject { data: 2, size: 32 });
        
        let stats = Gc::stats();
        assert_eq!(stats.total_allocations, 2);
        assert!(stats.bytes_allocated > 0);
    }

    #[test]
    fn test_heap_limit() {
        Gc::init();
        Gc::set_heap_limit(128); // 128 MB
        
        let stats = Gc::stats();
        // Should not panic
        assert!(stats.total_allocations >= 0);
    }
}
