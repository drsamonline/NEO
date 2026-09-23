//! NOVA Runtime Memory Management
//! 
//! Provides basic memory allocation and management utilities
//! for NOVA programs at runtime.

// ============================================================================
// Module State
// ============================================================================

static mut INITIALIZED: bool = false;

/// Initialize the memory management system
pub fn init() {
    unsafe {
        if !INITIALIZED {
            // Future: Initialize custom allocator, GC, etc.
            INITIALIZED = true;
        }
    }
}

/// Shutdown the memory management system
pub fn shutdown() {
    unsafe {
        if INITIALIZED {
            // Future: Cleanup allocations, run finalizers, etc.
            INITIALIZED = false;
        }
    }
}

/// Check if memory system is initialized
pub fn is_initialized() -> bool {
    unsafe { INITIALIZED }
}

// ============================================================================
// Allocation Traits
// ============================================================================

/// Trait for types that can be allocated on the heap
pub trait Allocatable {
    /// Allocate this value on the heap
    fn allocate(self) -> Box<Self> where Self: Sized {
        Box::new(self)
    }
    
    /// Allocate an array of this type
    fn allocate_array(size: usize) -> Vec<Self> where Self: Sized + Default + Clone {
        vec![Self::default(); size]
    }
}

// Implement for common types
impl Allocatable for i8 {}
impl Allocatable for i16 {}
impl Allocatable for i32 {}
impl Allocatable for i64 {}
impl Allocatable for u8 {}
impl Allocatable for u16 {}
impl Allocatable for u32 {}
impl Allocatable for u64 {}
impl Allocatable for f32 {}
impl Allocatable for f64 {}
impl Allocatable for bool {}
impl Allocatable for char {}

// ============================================================================
// Memory Utilities
// ============================================================================

/// Zero-initialize a memory region
pub fn zero_initialize<T: Default>(value: &mut T) {
    *value = T::default();
}

/// Copy memory from source to destination
pub fn copy_memory<T: Clone>(src: &T, dst: &mut T) {
    *dst = src.clone();
}

/// Swap two values
pub fn swap_values<T>(a: &mut T, b: &mut T) {
    std::mem::swap(a, b);
}

// ============================================================================
// Stack Utilities
// ============================================================================

/// Simple stack-allocated buffer
pub struct StackBuffer<T, const N: usize> {
    data: [T; N],
    len: usize,
}

impl<T: Default + Clone, const N: usize> StackBuffer<T, N> {
    /// Create a new empty stack buffer
    pub fn new() -> Self {
        Self {
            data: std::array::from_fn(|_| T::default()),
            len: 0,
        }
    }
    
    /// Push a value onto the buffer
    pub fn push(&mut self, value: T) -> Result<(), &'static str> {
        if self.len >= N {
            return Err("Buffer overflow");
        }
        self.data[self.len] = value;
        self.len += 1;
        Ok(())
    }
    
    /// Pop a value from the buffer
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.data[self.len].clone())
    }
    
    /// Get the current length
    pub fn len(&self) -> usize {
        self.len
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T: Default + Clone, const N: usize> Default for StackBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_init() {
        assert!(!is_initialized());
        init();
        assert!(is_initialized());
        shutdown();
        assert!(!is_initialized());
    }

    #[test]
    fn test_allocate() {
        let val = 42i32;
        let boxed = val.allocate();
        assert_eq!(*boxed, 42);
    }

    #[test]
    fn test_allocate_array() {
        let arr: Vec<i32> = i32::allocate_array(5);
        assert_eq!(arr.len(), 5);
        assert!(arr.iter().all(|&x| x == 0));
    }

    #[test]
    fn test_stack_buffer() {
        let mut buf = StackBuffer::<i32, 10>::new();
        assert!(buf.is_empty());
        
        buf.push(1).unwrap();
        buf.push(2).unwrap();
        assert_eq!(buf.len(), 2);
        
        assert_eq!(buf.pop(), Some(2));
        assert_eq!(buf.pop(), Some(1));
        assert_eq!(buf.pop(), None);
    }

    #[test]
    fn test_swap() {
        let mut a = 1;
        let mut b = 2;
        swap_values(&mut a, &mut b);
        assert_eq!(a, 2);
        assert_eq!(b, 1);
    }
}
