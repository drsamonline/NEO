// NOVA Runtime System - FFI Module
// Foreign Function Interface for C interoperability

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;

/// NOVA value representation for FFI
#[repr(C)]
pub struct NovaValue {
    pub tag: u32,
    pub data: NovaValueData,
}

#[repr(C)]
union NovaValueData {
    pub int_val: i64,
    pub float_val: f64,
    pub bool_val: bool,
    pub ptr_val: *mut c_void,
}

impl NovaValue {
    pub fn new_int(val: i64) -> Self {
        NovaValue {
            tag: 0, // INT tag
            data: NovaValueData { int_val: val },
        }
    }
    
    pub fn new_float(val: f64) -> Self {
        NovaValue {
            tag: 1, // FLOAT tag
            data: NovaValueData { float_val: val },
        }
    }
    
    pub fn new_bool(val: bool) -> Self {
        NovaValue {
            tag: 2, // BOOL tag
            data: NovaValueData { bool_val: val },
        }
    }
    
    pub fn new_ptr(ptr: *mut c_void) -> Self {
        NovaValue {
            tag: 3, // PTR tag
            data: NovaValueData { ptr_val: ptr },
        }
    }
    
    pub unsafe fn as_int(&self) -> i64 {
        self.data.int_val
    }
    
    pub unsafe fn as_float(&self) -> f64 {
        self.data.float_val
    }
    
    pub unsafe fn as_bool(&self) -> bool {
        self.data.bool_val
    }
    
    pub unsafe fn as_ptr(&self) -> *mut c_void {
        self.data.ptr_val
    }
}

/// String utilities for FFI
pub struct FfiString;

impl FfiString {
    /// Convert Rust string to C string (caller must free)
    pub fn to_c_string(s: &str) -> *mut c_char {
        match CString::new(s) {
            Ok(c_string) => c_string.into_raw(),
            Err(_) => ptr::null_mut(),
        }
    }
    
    /// Convert C string to Rust string (does not free the C string)
    pub unsafe fn from_c_string(ptr: *const c_char) -> Option<String> {
        if ptr.is_null() {
            return None;
        }
        
        CStr::from_ptr(ptr)
            .to_str()
            .ok()
            .map(|s| s.to_string())
    }
    
    /// Free a C string allocated by this module
    pub unsafe fn free(ptr: *mut c_char) {
        if !ptr.is_null() {
            let _ = CString::from_raw(ptr);
        }
    }
}

/// Array utilities for FFI
pub struct FfiArray<T> {
    data: *mut T,
    length: usize,
    capacity: usize,
}

impl<T: Clone> FfiArray<T> {
    pub fn new(capacity: usize) -> Self {
        let layout = std::alloc::Layout::array::<T>(capacity)
            .expect("Invalid array layout");
        
        unsafe {
            let ptr = std::alloc::alloc(layout) as *mut T;
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            
            FfiArray {
                data: ptr,
                length: 0,
                capacity,
            }
        }
    }
    
    pub fn push(&mut self, value: T) -> bool {
        if self.length >= self.capacity {
            return false;
        }
        
        unsafe {
            ptr::write(self.data.add(self.length), value);
            self.length += 1;
        }
        true
    }
    
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.length {
            return None;
        }
        unsafe { Some(&*self.data.add(index)) }
    }
    
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.length {
            return None;
        }
        unsafe { Some(&mut *self.data.add(index)) }
    }
    
    pub fn len(&self) -> usize {
        self.length
    }
    
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
    
    pub fn as_ptr(&self) -> *const T {
        self.data
    }
    
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.data
    }
}

impl<T> Drop for FfiArray<T> {
    fn drop(&mut self) {
        unsafe {
            let layout = std::alloc::Layout::array::<T>(self.capacity)
                .expect("Invalid array layout");
            std::alloc::dealloc(self.data as *mut u8, layout);
        }
    }
}

/// Callback function type for FFI
pub type NovaCallback = extern "C" fn(*mut c_void) -> NovaValue;

/// Opaque handle for managing external resources
pub struct FfiHandle {
    id: u64,
    resource_type: String,
    closed: bool,
}

impl FfiHandle {
    pub fn new(id: u64, resource_type: &str) -> Self {
        FfiHandle {
            id,
            resource_type: resource_type.to_string(),
            closed: false,
        }
    }
    
    pub fn id(&self) -> u64 {
        self.id
    }
    
    pub fn resource_type(&self) -> &str {
        &self.resource_type
    }
    
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    
    pub fn close(&mut self) {
        self.closed = true;
    }
}

/// C-compatible function signature wrapper
pub trait CallableFromC: Sized {
    unsafe extern "C" fn call(ptr: *mut c_void, args: *const NovaValue, nargs: c_int) -> NovaValue;
    
    fn into_ffi(self) -> FfiFunction {
        FfiFunction {
            ptr: Box::into_raw(Box::new(self)) as *mut c_void,
            caller: Self::call,
            destructor: |ptr| {
                unsafe {
                    let _ = Box::from_raw(ptr as *mut Self);
                }
            },
        }
    }
}

/// FFI function wrapper
pub struct FfiFunction {
    ptr: *mut c_void,
    caller: unsafe extern "C" fn(*mut c_void, *const NovaValue, c_int) -> NovaValue,
    destructor: unsafe fn(*mut c_void),
}

impl FfiFunction {
    pub unsafe fn call(&self, args: *const NovaValue, nargs: c_int) -> NovaValue {
        (self.caller)(self.ptr, args, nargs)
    }
    
    pub fn ptr(&self) -> *mut c_void {
        self.ptr
    }
}

impl Drop for FfiFunction {
    fn drop(&mut self) {
        unsafe {
            (self.destructor)(self.ptr);
        }
    }
}

/// Initialize FFI subsystem
pub fn init_ffi() {
    // Placeholder for FFI initialization
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_nova_value_int() {
        let val = NovaValue::new_int(42);
        unsafe {
            assert_eq!(val.as_int(), 42);
        }
    }
    
    #[test]
    fn test_nova_value_float() {
        let val = NovaValue::new_float(3.14);
        unsafe {
            assert!((val.as_float() - 3.14).abs() < f64::EPSILON);
        }
    }
    
    #[test]
    fn test_nova_value_bool() {
        let val_true = NovaValue::new_bool(true);
        let val_false = NovaValue::new_bool(false);
        unsafe {
            assert_eq!(val_true.as_bool(), true);
            assert_eq!(val_false.as_bool(), false);
        }
    }
    
    #[test]
    fn test_ffi_string_conversion() {
        let rust_str = "Hello, World!";
        let c_str = FfiString::to_c_string(rust_str);
        
        unsafe {
            let recovered = FfiString::from_c_string(c_str).unwrap();
            assert_eq!(recovered, rust_str);
            FfiString::free(c_str);
        }
    }
    
    #[test]
    fn test_ffi_array() {
        let mut array = FfiArray::<i32>::new(10);
        
        assert!(array.push(1));
        assert!(array.push(2));
        assert!(array.push(3));
        
        assert_eq!(array.len(), 3);
        assert_eq!(*array.get(0).unwrap(), 1);
        assert_eq!(*array.get(1).unwrap(), 2);
        assert_eq!(*array.get(2).unwrap(), 3);
        
        assert!(array.get(10).is_none());
    }
    
    #[test]
    fn test_ffi_handle() {
        let mut handle = FfiHandle::new(123, "File");
        
        assert_eq!(handle.id(), 123);
        assert_eq!(handle.resource_type(), "File");
        assert!(!handle.is_closed());
        
        handle.close();
        assert!(handle.is_closed());
    }
}
