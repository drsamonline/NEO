//! NOVA Runtime Error Handling
//! 
//! Provides error types and handling mechanisms for NOVA programs.

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

static mut ERROR_HANDLER: Option<fn(&NovaError)> = None;

/// Initialize error handling system
pub fn init() {
    unsafe {
        ERROR_HANDLER = None;
    }
}

/// Set custom error handler
pub fn set_handler(handler: fn(&NovaError)) {
    unsafe {
        ERROR_HANDLER = Some(handler);
    }
}

/// Report an error (calls handler if set)
pub fn report_error(error: &NovaError) {
    eprintln!("NOVA Error: {}", error);
    unsafe {
        if let Some(handler) = ERROR_HANDLER {
            handler(error);
        }
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
    
    pub fn io_error(msg: &str) -> Self {
        Self::new(ErrorKind::IoError, msg)
    }
    
    pub fn argument_error(msg: &str) -> Self {
        Self::new(ErrorKind::ArgumentError, msg)
    }
    
    pub fn not_found(msg: &str) -> Self {
        Self::new(ErrorKind::NotFound, msg)
    }
    
    pub fn custom(msg: &str) -> Self {
        Self::new(ErrorKind::Custom, msg)
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
