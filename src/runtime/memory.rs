// NOVA Runtime System - Memory Management Module
// Provides garbage collection, allocation, and memory safety primitives

use std::alloc::{self, Layout};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Global memory statistics
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
static GC_RUNS: AtomicUsize = AtomicUsize::new(0);

/// Memory arena for efficient object allocation
pub struct MemoryArena {
    buffer: *mut u8,
    capacity: usize,
    offset: usize,
    layout: Layout,
}

impl MemoryArena {
    /// Create a new memory arena with specified capacity
    pub fn new(capacity: usize) -> Result<Self, &'static str> {
        if capacity == 0 {
            return Err("Arena capacity must be greater than zero");
        }
        
        let layout = Layout::from_size_align(capacity, 8)
            .map_err(|_| "Invalid layout")?;
        
        unsafe {
            let ptr = alloc::alloc(layout);
            if ptr.is_null() {
                alloc::handle_alloc_error(layout);
            }
            
            Ok(MemoryArena {
                buffer: ptr,
                capacity,
                offset: 0,
                layout,
            })
        }
    }
    
    /// Allocate memory from the arena
    pub fn allocate<T>(&mut self, value: T) -> Result<*mut T, &'static str> {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        
        // Align the offset
        let aligned_offset = (self.offset + align - 1) & !(align - 1);
        
        if aligned_offset + size > self.capacity {
            return Err("Arena out of memory");
        }
        
        unsafe {
            let ptr = self.buffer.add(aligned_offset) as *mut T;
            ptr::write(ptr, value);
            self.offset = aligned_offset + size;
            
            ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
            
            Ok(ptr)
        }
    }
    
    /// Reset the arena (mark all memory as free without deallocating)
    pub fn reset(&mut self) {
        self.offset = 0;
    }
    
    /// Get current allocation statistics
    pub fn stats(&self) -> ArenaStats {
        ArenaStats {
            capacity: self.capacity,
            used: self.offset,
            allocated_bytes: ALLOCATED_BYTES.load(Ordering::Relaxed),
            allocation_count: ALLOCATION_COUNT.load(Ordering::Relaxed),
            gc_runs: GC_RUNS.load(Ordering::Relaxed),
        }
    }
}

impl Drop for MemoryArena {
    fn drop(&mut self) {
        unsafe {
            alloc::dealloc(self.buffer, self.layout);
        }
    }
}

/// Statistics about memory arena usage
#[derive(Debug, Clone)]
pub struct ArenaStats {
    pub capacity: usize,
    pub used: usize,
    pub allocated_bytes: usize,
    pub allocation_count: usize,
    pub gc_runs: usize,
}

/// Simple mark-and-sweep garbage collector placeholder
pub struct GarbageCollector {
    arenas: Vec<MemoryArena>,
}

impl GarbageCollector {
    pub fn new() -> Self {
        GarbageCollector { arenas: Vec::new() }
    }
    
    /// Run garbage collection cycle
    pub fn collect(&mut self) -> usize {
        let collected = 0; // Placeholder for actual GC logic
        GC_RUNS.fetch_add(1, Ordering::Relaxed);
        collected
    }
    
    /// Add an arena to be managed by GC
    pub fn add_arena(&mut self, arena: MemoryArena) {
        self.arenas.push(arena);
    }
}

impl Default for GarbageCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Safe wrapper for runtime-managed pointers
pub struct ManagedPtr<T> {
    ptr: *mut T,
    tracked: bool,
}

impl<T> ManagedPtr<T> {
    pub fn new(ptr: *mut T) -> Self {
        ManagedPtr {
            ptr,
            tracked: true,
        }
    }
    
    pub fn as_ptr(&self) -> *mut T {
        self.ptr
    }
    
    pub unsafe fn as_ref(&self) -> Option<&T> {
        if self.ptr.is_null() {
            None
        } else {
            Some(&*self.ptr)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_arena_allocation() {
        let mut arena = MemoryArena::new(1024).unwrap();
        let ptr = arena.allocate(42i32).unwrap();
        
        unsafe {
            assert_eq!(*ptr, 42);
        }
        
        let stats = arena.stats();
        assert!(stats.allocation_count > 0);
        assert!(stats.allocated_bytes >= std::mem::size_of::<i32>());
    }
    
    #[test]
    fn test_arena_reset() {
        let mut arena = MemoryArena::new(1024).unwrap();
        arena.allocate(1i32).unwrap();
        arena.allocate(2i32).unwrap();
        
        let before = arena.stats().used;
        arena.reset();
        let after = arena.stats().used;
        
        assert!(before > 0);
        assert_eq!(after, 0);
    }
}
