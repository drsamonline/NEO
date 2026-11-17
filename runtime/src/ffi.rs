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
///
/// # Safety
///
/// `c_str` must be either null or a valid pointer to a NUL-terminated C string
/// whose bytes stay readable for the whole call. The string is copied out, so it
/// may be freed as soon as this returns.
pub unsafe fn c_to_rust_string(c_str: *const c_char) -> Option<String> {
    if c_str.is_null() {
        return None;
    }
    // SAFETY: the caller guarantees a NUL-terminated, readable C string, which
    // is the only precondition `CStr::from_ptr` has.
    let c_str = unsafe { CStr::from_ptr(c_str) };
    c_str.to_str().ok().map(|s| s.to_string())
}

/// Free a C string created by rust_to_c_string
///
/// # Safety
///
/// `ptr` must be null, or a pointer returned by `rust_to_c_string` that has not
/// already been freed. Any other pointer, or a second free of the same pointer,
/// is undefined behaviour — `CString::from_raw` reclaims ownership and runs the
/// allocator on whatever it is given.
pub unsafe fn free_c_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        // SAFETY: the caller guarantees this pointer came from
        // `CString::into_raw` and is not yet freed, which is exactly the
        // contract `CString::from_raw` requires.
        drop(unsafe { CString::from_raw(ptr) });
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
///
/// # Safety
///
/// If `ptr` is not null it must point to a `T` that is valid for reads for the
/// whole of `'a`, and nothing may mutate it during `'a`. `'a` is chosen by the
/// caller and is not tied to the pointer's real lifetime, so the caller is the
/// only thing asserting it.
pub unsafe fn ptr_to_opt<'a, T>(ptr: *const T) -> Option<&'a T> {
    // SAFETY: the caller guarantees validity and the `'a` lifetime above.
    unsafe { ptr.as_ref() }
}

/// Convert nullable mutable pointer to option
///
/// # Safety
///
/// If `ptr` is not null it must point to a `T` that is valid for reads and writes
/// for the whole of `'a`, nothing else may access it during `'a`, and `'a` must
/// not outlive the real allocation. The caller is asserting the lifetime; the
/// compiler cannot.
pub unsafe fn ptr_to_opt_mut<'a, T>(ptr: *mut T) -> Option<&'a mut T> {
    // SAFETY: the caller guarantees exclusive, valid access for `'a`.
    unsafe { ptr.as_mut() }
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
/// Call a C function that takes no arguments.
///
/// Not `unsafe`: `extern "C" fn` — as opposed to `unsafe extern "C" fn` — is a
/// *safe* function pointer, so calling it is a safe operation. This used to be
/// marked `unsafe`, which told callers they had obligations they did not have.
pub fn call_c_func_0<R>(func: extern "C" fn() -> R) -> R {
    func()
}

/// Call an external C function with one argument
/// Call a C function with one argument. Safe for the reason given on
/// [`call_c_func_0`].
pub fn call_c_func_1<A, R>(func: extern "C" fn(A) -> R, arg: A) -> R {
    func(arg)
}

/// Call an external C function with two arguments
/// Call a C function with two arguments. Safe for the reason given on
/// [`call_c_func_0`].
pub fn call_c_func_2<A, B, R>(func: extern "C" fn(A, B) -> R, arg1: A, arg2: B) -> R {
    func(arg1, arg2)
}

// ============================================================================
// Memory Interop
// ============================================================================

/// Allocate memory visible to C code
///
/// Returns null on failure or when `size` is zero, matching `malloc`. The caller
/// owns the block and must release it with [`c_free`].
pub fn c_alloc(size: usize) -> *mut c_void {
    // SAFETY: `malloc` is safe to call with any `usize`; it returns null rather
    // than failing. The returned pointer is not dereferenced here.
    unsafe { libc::malloc(size) }
}

/// Free memory allocated by c_alloc
///
/// # Safety
///
/// `ptr` must be null or a block returned by [`c_alloc`] or [`c_calloc`] that has
/// not already been freed. Freeing a pointer from anywhere else, or freeing
/// twice, is undefined behaviour.
pub unsafe fn c_free(ptr: *mut c_void) {
    // SAFETY: the caller guarantees the pointer came from this allocator and is
    // still live. `free` accepts null.
    unsafe { libc::free(ptr) }
}

/// Zero-allocate memory
///
/// `count * size` is not checked for overflow here; `calloc` is required to fail
/// rather than return a short block, but a caller that can see attacker-controlled
/// sizes should multiply with `checked_mul` first and treat `None` as an error.
pub fn c_calloc(count: usize, size: usize) -> *mut c_void {
    // SAFETY: `calloc` is safe to call with any pair of `usize`; it returns null
    // on overflow or failure.
    unsafe { libc::calloc(count, size) }
}

/// Reallocate memory
///
/// # Safety
///
/// `ptr` must be null or a block returned by [`c_alloc`], [`c_calloc`] or
/// [`c_realloc`] that has not been freed. On success the old pointer is
/// invalidated even when the address is unchanged, so the caller must not use it
/// again.
pub unsafe fn c_realloc(ptr: *mut c_void, new_size: usize) -> *mut c_void {
    // SAFETY: the caller guarantees a live allocation from this allocator, and
    // gives up the old pointer by calling this.
    unsafe { libc::realloc(ptr, new_size) }
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
