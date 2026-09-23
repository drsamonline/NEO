// NOVA Runtime System - Error Handling Module
// Runtime error types, panic handling, and error propagation

use std::fmt;
use std::panic::{self, PanicInfo};
use std::sync::atomic::{AtomicBool, Ordering};
use std::backtrace::Backtrace;

/// Global panic flag
static PANIC_OCCURRED: AtomicBool = AtomicBool::new(false);

/// NOVA runtime error types
#[derive(Debug, Clone)]
pub enum RuntimeError {
    /// Memory allocation failure
    OutOfMemory { requested: usize, available: usize },
    
    /// Type mismatch at runtime
    TypeMismatch { expected: String, found: String },
    
    /// Index out of bounds
    IndexOutOfBounds { index: usize, length: usize },
    
    /// Division by zero
    DivisionByZero,
    
    /// Integer overflow
    IntegerOverflow { operation: String, value: i128 },
    
    /// Null pointer dereference
    NullPointerDereference,
    
    /// Invalid function argument
    InvalidArgument { param: String, reason: String },
    
    /// Resource not found
    NotFound { resource: String, name: String },
    
    /// I/O error
    IoError { message: String },
    
    /// Custom user-defined error
    Custom { code: i32, message: String },
}

impl RuntimeError {
    pub fn out_of_memory(requested: usize, available: usize) -> Self {
        RuntimeError::OutOfMemory { requested, available }
    }
    
    pub fn type_mismatch(expected: &str, found: &str) -> Self {
        RuntimeError::TypeMismatch {
            expected: expected.to_string(),
            found: found.to_string(),
        }
    }
    
    pub fn index_out_of_bounds(index: usize, length: usize) -> Self {
        RuntimeError::IndexOutOfBounds { index, length }
    }
    
    pub fn division_by_zero() -> Self {
        RuntimeError::DivisionByZero
    }
    
    pub fn integer_overflow(operation: &str, value: i128) -> Self {
        RuntimeError::IntegerOverflow {
            operation: operation.to_string(),
            value,
        }
    }
    
    pub fn null_pointer() -> Self {
        RuntimeError::NullPointerDereference
    }
    
    pub fn invalid_argument(param: &str, reason: &str) -> Self {
        RuntimeError::InvalidArgument {
            param: param.to_string(),
            reason: reason.to_string(),
        }
    }
    
    pub fn not_found(resource: &str, name: &str) -> Self {
        RuntimeError::NotFound {
            resource: resource.to_string(),
            name: name.to_string(),
        }
    }
    
    pub fn io_error(message: &str) -> Self {
        RuntimeError::IoError {
            message: message.to_string(),
        }
    }
    
    pub fn custom(code: i32, message: &str) -> Self {
        RuntimeError::Custom {
            code,
            message: message.to_string(),
        }
    }
    
    pub fn code(&self) -> i32 {
        match self {
            RuntimeError::OutOfMemory { .. } => 1,
            RuntimeError::TypeMismatch { .. } => 2,
            RuntimeError::IndexOutOfBounds { .. } => 3,
            RuntimeError::DivisionByZero => 4,
            RuntimeError::IntegerOverflow { .. } => 5,
            RuntimeError::NullPointerDereference => 6,
            RuntimeError::InvalidArgument { .. } => 7,
            RuntimeError::NotFound { .. } => 8,
            RuntimeError::IoError { .. } => 9,
            RuntimeError::Custom { code, .. } => *code,
        }
    }
    
    pub fn category(&self) -> &'static str {
        match self {
            RuntimeError::OutOfMemory { .. } => "Memory",
            RuntimeError::TypeMismatch { .. } => "Type",
            RuntimeError::IndexOutOfBounds { .. } => "Bounds",
            RuntimeError::DivisionByZero => "Arithmetic",
            RuntimeError::IntegerOverflow { .. } => "Arithmetic",
            RuntimeError::NullPointerDereference => "Memory",
            RuntimeError::InvalidArgument { .. } => "Argument",
            RuntimeError::NotFound { .. } => "Resource",
            RuntimeError::IoError { .. } => "IO",
            RuntimeError::Custom { .. } => "User",
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::OutOfMemory { requested, available } => {
                write!(f, "Out of memory: requested {} bytes, only {} available", 
                       requested, available)
            }
            RuntimeError::TypeMismatch { expected, found } => {
                write!(f, "Type mismatch: expected {}, found {}", expected, found)
            }
            RuntimeError::IndexOutOfBounds { index, length } => {
                write!(f, "Index out of bounds: index {} in collection of length {}", 
                       index, length)
            }
            RuntimeError::DivisionByZero => {
                write!(f, "Division by zero")
            }
            RuntimeError::IntegerOverflow { operation, value } => {
                write!(f, "Integer overflow in {}: {}", operation, value)
            }
            RuntimeError::NullPointerDereference => {
                write!(f, "Null pointer dereference")
            }
            RuntimeError::InvalidArgument { param, reason } => {
                write!(f, "Invalid argument '{}': {}", param, reason)
            }
            RuntimeError::NotFound { resource, name } => {
                write!(f, "{} not found: {}", resource, name)
            }
            RuntimeError::IoError { message } => {
                write!(f, "I/O error: {}", message)
            }
            RuntimeError::Custom { code, message } => {
                write!(f, "Error {}: {}", code, message)
            }
        }
    }
}

impl std::error::Error for RuntimeError {}

/// Enhanced error with backtrace information
#[derive(Debug)]
pub struct RuntimeErrorWithTrace {
    pub error: RuntimeError,
    pub backtrace: Backtrace,
    pub message: Option<String>,
}

impl RuntimeErrorWithTrace {
    pub fn new(error: RuntimeError) -> Self {
        RuntimeErrorWithTrace {
            error,
            backtrace: Backtrace::capture(),
            message: None,
        }
    }
    
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }
}

impl fmt::Display for RuntimeErrorWithTrace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(msg) = &self.message {
            writeln!(f, "{}", msg)?;
        }
        writeln!(f, "Error: {}", self.error)?;
        writeln!(f, "Backtrace:\n{}", self.backtrace)
    }
}

/// Initialize runtime error handling
pub fn init_runtime() {
    panic::set_hook(Box::new(|panic_info| {
        PANIC_OCCURRED.store(true, Ordering::SeqCst);
        
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };
        
        let location = panic_info.location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        
        eprintln!("\n=== NOVA Runtime Panic ===");
        eprintln!("Message: {}", message);
        eprintln!("Location: {}", location);
        eprintln!("Backtrace:\n{}", Backtrace::capture());
        eprintln!("=========================\n");
    }));
}

/// Check if a panic has occurred
pub fn has_panicked() -> bool {
    PANIC_OCCURRED.load(Ordering::SeqCst)
}

/// Reset panic flag
pub fn reset_panic_flag() {
    PANIC_OCCURRED.store(false, Ordering::SeqCst);
}

/// Result type alias for NOVA runtime operations
pub type RuntimeResult<T> = Result<T, RuntimeError>;

/// Catch panics and convert to RuntimeResult
pub fn catch_unwind<F, R>(f: F) -> RuntimeResult<R>
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    panic::catch_unwind(f)
        .map_err(|_| RuntimeError::Custom {
            code: 100,
            message: "Panic caught during execution".to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_creation() {
        let err = RuntimeError::type_mismatch("i32", "f64");
        assert_eq!(err.code(), 2);
        assert_eq!(err.category(), "Type");
        
        let display = format!("{}", err);
        assert!(display.contains("Type mismatch"));
        assert!(display.contains("i32"));
        assert!(display.contains("f64"));
    }
    
    #[test]
    fn test_error_categories() {
        assert_eq!(RuntimeError::division_by_zero().category(), "Arithmetic");
        assert_eq!(RuntimeError::null_pointer().category(), "Memory");
        assert_eq!(RuntimeError::io_error("test").category(), "IO");
    }
    
    #[test]
    fn test_error_with_trace() {
        let err = RuntimeError::division_by_zero();
        let traced = RuntimeErrorWithTrace::new(err);
        
        assert_eq!(traced.error.code(), 4);
        assert!(!format!("{}", traced).is_empty());
    }
    
    #[test]
    fn test_catch_unwind_success() {
        let result = catch_unwind(|| 42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
}
