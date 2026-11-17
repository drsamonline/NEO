//! NOVA Runtime Error Handling
//! 
//! Provides error types and handling mechanisms for NOVA programs.

use std::sync::{Mutex, MutexGuard};

// ============================================================================
// Error Types
// ============================================================================

/// Core NOVA error type
#[derive(Debug, Clone, PartialEq)]
pub struct NovaError {
    pub kind: ErrorKind,
    pub message: String,
    pub location: Option<String>,
}

/// Categories of errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Runtime type error
    TypeError,
    /// Index out of bounds
    IndexError,
    /// Null/None access
    NullError,
    /// Division by zero
    DivisionByZero,
    /// Overflow/underflow
    Overflow,
    /// Memory allocation failure
    OutOfMemory,
    /// I/O operation failed
    IoError,
    /// Invalid argument
    ArgumentError,
    /// Operation not permitted
    PermissionDenied,
    /// Resource not found
    NotFound,
    /// Custom user error
    Custom,
}

impl NovaError {
    /// Create a new error
    pub fn new(kind: ErrorKind, message: &str) -> Self {
        Self {
            kind,
            message: message.to_string(),
            location: None,
        }
    }
    
    /// Create with location info
    pub fn with_location(mut self, location: &str) -> Self {
        self.location = Some(location.to_string());
        self
    }
    
    /// Get error description
    pub fn description(&self) -> &str {
        &self.message
    }
    
    /// Get error kind
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl std::fmt::Display for NovaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)?;
        if let Some(loc) = &self.location {
            write!(f, " (at {})", loc)?;
        }
        Ok(())
    }
}

impl std::error::Error for NovaError {}

// ============================================================================
// Result Type
// ============================================================================

/// Standard result type for NOVA operations
pub type NovaResult<T> = Result<T, NovaError>;

// ============================================================================
// Error Handlers
// ============================================================================

/// The process-wide error handler.
///
/// A `Mutex` rather than `static mut`. `report_error` is callable from any
/// thread, and a `static mut` read there is a data race; the value behind this
/// lock is a single `Copy` function pointer, so the lock is never held across
/// anything that can block for long.
static ERROR_HANDLER: Mutex<Option<fn(&NovaError)>> = Mutex::new(None);

/// The handler slot, with lock poisoning ignored.
///
/// A handler that panics poisons the lock. Poisoning exists to warn that a
/// protected invariant may be half-updated; the only thing protected here is one
/// `Copy` function pointer, which cannot be left half-written. Recovering with
/// `into_inner` keeps error reporting alive after a panic inside a handler,
/// which is exactly when you still want it.
fn handler_slot() -> MutexGuard<'static, Option<fn(&NovaError)>> {
    ERROR_HANDLER
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Initialize error handling system
pub fn init() {
    *handler_slot() = None;
}

/// Set custom error handler
pub fn set_handler(handler: fn(&NovaError)) {
    *handler_slot() = Some(handler);
}

/// Report an error (calls handler if set)
pub fn report_error(error: &NovaError) {
    eprintln!("NOVA Error: {}", error);
    // Copy the pointer out and drop the guard before calling it, so a handler
    // that reports another error cannot deadlock against this lock.
    let handler = *handler_slot();
    if let Some(handler) = handler {
        handler(error);
    }
}

/// Panic on error
pub fn panic_on_error(result: NovaResult<()>) {
    if let Err(e) = result {
        panic!("NOVA runtime error: {}", e);
    }
}

// ============================================================================
// Convenience Error Constructors
// ============================================================================

impl NovaError {
    pub fn type_error(msg: &str) -> Self {
        Self::new(ErrorKind::TypeError, msg)
    }
    
    pub fn index_error(msg: &str) -> Self {
        Self::new(ErrorKind::IndexError, msg)
    }
    
    pub fn null_error(msg: &str) -> Self {
        Self::new(ErrorKind::NullError, msg)
    }
    
    pub fn division_by_zero() -> Self {
        Self::new(ErrorKind::DivisionByZero, "division by zero")
    }
    
    pub fn overflow(msg: &str) -> Self {
        Self::new(ErrorKind::Overflow, msg)
    }
    
    pub fn out_of_memory() -> Self {
        Self::new(ErrorKind::OutOfMemory, "out of memory")
    }
    
    pub fn io_error(msg: impl AsRef<str>) -> Self {
        Self::new(ErrorKind::IoError, msg.as_ref())
    }
    
    pub fn argument_error(msg: impl AsRef<str>) -> Self {
        Self::new(ErrorKind::ArgumentError, msg.as_ref())
    }
    
    pub fn not_found(msg: impl AsRef<str>) -> Self {
        Self::new(ErrorKind::NotFound, msg.as_ref())
    }
    
    pub fn custom(msg: impl AsRef<str>) -> Self {
        Self::new(ErrorKind::Custom, msg.as_ref())
    }
    
    pub fn sync_error(msg: impl AsRef<str>) -> Self {
        Self::new(ErrorKind::Custom, msg.as_ref())
    }
    
    pub fn async_error(msg: impl AsRef<str>) -> Self {
        Self::new(ErrorKind::Custom, msg.as_ref())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = NovaError::type_error("expected integer");
        assert_eq!(err.kind(), ErrorKind::TypeError);
        assert_eq!(err.description(), "expected integer");
    }

    #[test]
    fn test_error_with_location() {
        let err = NovaError::index_error("out of bounds")
            .with_location("main.nova:10:5");
        assert_eq!(err.location, Some("main.nova:10:5".to_string()));
    }

    #[test]
    fn test_error_display() {
        let err = NovaError::division_by_zero();
        let msg = format!("{}", err);
        assert!(msg.contains("DivisionByZero"));
        assert!(msg.contains("division by zero"));
    }

    #[test]
    fn test_result_ok() {
        let result: NovaResult<i32> = Ok(42);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_result_err() {
        let result: NovaResult<i32> = Err(NovaError::null_error("value was null"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), ErrorKind::NullError);
    }

    #[test]
    fn test_convenience_constructors() {
        assert_eq!(NovaError::overflow("too big").kind(), ErrorKind::Overflow);
        assert_eq!(NovaError::out_of_memory().kind(), ErrorKind::OutOfMemory);
        assert_eq!(NovaError::io_error("read failed").kind(), ErrorKind::IoError);
        assert_eq!(NovaError::not_found("file").kind(), ErrorKind::NotFound);
    }
}
