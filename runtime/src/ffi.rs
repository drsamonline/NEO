//! NOVA Runtime FFI (Foreign Function Interface)
//! 
//! Provides interoperability with C and other languages.

// ============================================================================
// C-Compatible Types
// ============================================================================

use std::os::raw::{c_char, c_int, c_void};
use std::ffi::{CStr, CString};
use std::ptr;

/// Boolean type compatible with C
pub type NovaBool = c_int;

/// True constant for C interop
pub const NOVA_TRUE: NovaBool = 1;

/// False constant for C interop  
pub const NOVA_FALSE: NovaBool = 0;

// ============================================================================
// String Conversion
// ============================================================================

/// Convert Rust string to C string (caller must free)
pub fn rust_to_c_string(s: &str) -> *mut c_char {
    match CString::new(s) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Convert C string to Rust string (does not free the C string)
pub unsafe fn c_to_rust_string(c_str: *const c_char) -> Option<String> {
    if c_str.is_null() {
        return None;
    }
    CStr::from_ptr(c_str)
        .to_str()
        .ok()
        .map(|s| s.to_string())
}

/// Free a C string created by rust_to_c_string
pub unsafe fn free_c_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

// ============================================================================
// Pointer Utilities
// ============================================================================

/// Safe null pointer check
pub fn is_null<T>(ptr: *const T) -> bool {
    ptr.is_null()
}

/// Safe null pointer check for mutable pointers
pub fn is_null_mut<T>(ptr: *mut T) -> bool {
    ptr.is_null()
}

/// Convert option to nullable pointer
pub fn opt_to_ptr<T>(opt: Option<&T>) -> *const T {
    match opt {
        Some(v) => v as *const T,
        None => ptr::null(),
    }
}

/// Convert option to nullable mutable pointer
pub fn opt_to_ptr_mut<T>(opt: Option<&mut T>) -> *mut T {
    match opt {
        Some(v) => v as *mut T,
        None => ptr::null_mut(),
    }
}

/// Convert nullable pointer to option
pub unsafe fn ptr_to_opt<'a, T>(ptr: *const T) -> Option<&'a T> {
    ptr.as_ref()
}

/// Convert nullable mutable pointer to option
pub unsafe fn ptr_to_opt_mut<'a, T>(ptr: *mut T) -> Option<&'a mut T> {
    ptr.as_mut()
}

// ============================================================================
// Callback Type Definitions
// ============================================================================

/// Generic callback function type for C interop
pub type NovaCallback = extern "C" fn(*mut c_void) -> c_int;

/// Callback with integer argument
pub type NovaCallbackI32 = extern "C" fn(i32, *mut c_void) -> c_int;

/// Callback with two integer arguments
pub type NovaCallbackI32I32 = extern "C" fn(i32, i32, *mut c_void) -> c_int;

// ============================================================================
// External Function Calling
// ============================================================================

/// Call an external C function with no arguments
pub unsafe fn call_c_func_0<R>(func: extern "C" fn() -> R) -> R {
    func()
}

/// Call an external C function with one argument
pub unsafe fn call_c_func_1<A, R>(func: extern "C" fn(A) -> R, arg: A) -> R {
    func(arg)
}

/// Call an external C function with two arguments
pub unsafe fn call_c_func_2<A, B, R>(func: extern "C" fn(A, B) -> R, arg1: A, arg2: B) -> R {
    func(arg1, arg2)
}

// ============================================================================
// Memory Interop
// ============================================================================

/// Allocate memory visible to C code
pub fn c_alloc(size: usize) -> *mut c_void {
    unsafe { libc::malloc(size) }
}

/// Free memory allocated by c_alloc
pub unsafe fn c_free(ptr: *mut c_void) {
    libc::free(ptr);
}

/// Zero-allocate memory
pub fn c_calloc(count: usize, size: usize) -> *mut c_void {
    unsafe { libc::calloc(count, size) }
}

/// Reallocate memory
pub unsafe fn c_realloc(ptr: *mut c_void, new_size: usize) -> *mut c_void {
    libc::realloc(ptr, new_size)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bool_constants() {
        assert_eq!(NOVA_TRUE, 1);
        assert_eq!(NOVA_FALSE, 0);
    }

    #[test]
    fn test_string_conversion() {
        let rust_str = "hello";
        let c_str = rust_to_c_string(rust_str);
        
        unsafe {
            let back = c_to_rust_string(c_str).unwrap();
            assert_eq!(back, rust_str);
            free_c_string(c_str);
        }
    }

    #[test]
    fn test_null_pointer_checks() {
        let null_ptr: *const i32 = ptr::null();
        assert!(is_null(null_ptr));
        
        let value = 42i32;
        assert!(!is_null(&value));
    }

    #[test]
    fn test_option_to_ptr() {
        let value = 42i32;
        let ptr = opt_to_ptr(Some(&value));
        assert!(!ptr.is_null());
        
        let null_ptr: Option<&i32> = None;
        let ptr = opt_to_ptr(null_ptr);
        assert!(ptr.is_null());
    }
}
