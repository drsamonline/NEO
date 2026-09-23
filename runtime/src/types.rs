//! NOVA Runtime Type System
//! 
//! Core runtime type implementations for NOVA programs.

// ============================================================================
// String Type
// ============================================================================

/// NOVA string type wrapper
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NovaString {
    data: String,
}

impl NovaString {
    /// Create a new NovaString
    pub fn new(s: &str) -> Self {
        Self {
            data: s.to_string(),
        }
    }
    
    /// Create an empty NovaString
    pub fn empty() -> Self {
        Self {
            data: String::new(),
        }
    }
    
    /// Get the length
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    
    /// Convert to Rust string slice
    pub fn as_str(&self) -> &str {
        &self.data
    }
    
    /// Concatenate two strings
    pub fn concat(&self, other: &Self) -> Self {
        Self {
            data: format!("{}{}", self.data, other.data),
        }
    }
}

impl From<&str> for NovaString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for NovaString {
    fn from(s: String) -> Self {
        Self { data: s }
    }
}

impl std::fmt::Display for NovaString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.data)
    }
}

impl Default for NovaString {
    fn default() -> Self {
        Self::empty()
    }
}

// ============================================================================
// Array Type
// ============================================================================

/// Dynamic array type for NOVA
#[derive(Debug, Clone)]
pub struct NovaArray<T> {
    data: Vec<T>,
}

impl<T: Clone> NovaArray<T> {
    /// Create a new empty array
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
        }
    }
    
    /// Create with capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }
    
    /// Push an element
    pub fn push(&mut self, value: T) {
        self.data.push(value);
    }
    
    /// Pop an element
    pub fn pop(&mut self) -> Option<T> {
        self.data.pop()
    }
    
    /// Get length
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    
    /// Get element at index
    pub fn get(&self, index: usize) -> Option<&T> {
        self.data.get(index)
    }
    
    /// Get mutable element at index
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.data.get_mut(index)
    }
    
    /// Set element at index
    pub fn set(&mut self, index: usize, value: T) -> Result<(), &'static str> {
        if index >= self.data.len() {
            return Err("Index out of bounds");
        }
        self.data[index] = value;
        Ok(())
    }
}

impl<T: Clone + Default> NovaArray<T> {
    /// Create array with repeated value
    pub fn repeat(value: T, count: usize) -> Self {
        Self {
            data: vec![value; count],
        }
    }
}

impl<T: Clone> Default for NovaArray<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Optional Type
// ============================================================================

/// NOVA optional type (like Option but runtime-visible)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NovaOption<T> {
    Some(T),
    None,
}

impl<T> NovaOption<T> {
    /// Create a Some variant
    pub fn some(value: T) -> Self {
        Self::Some(value)
    }
    
    /// Create a None variant
    pub fn none() -> Self {
        Self::None
    }
    
    /// Check if Some
    pub fn is_some(&self) -> bool {
        matches!(self, Self::Some(_))
    }
    
    /// Check if None
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
    
    /// Unwrap or provide default
    pub fn unwrap_or(self, default: T) -> T {
        match self {
            Self::Some(v) => v,
            Self::None => default,
        }
    }
}

impl<T> Default for NovaOption<T> {
    fn default() -> Self {
        Self::None
    }
}

impl<T> From<Option<T>> for NovaOption<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => Self::Some(v),
            None => Self::None,
        }
    }
}

impl<T> From<NovaOption<T>> for Option<T> {
    fn from(opt: NovaOption<T>) -> Self {
        match opt {
            NovaOption::Some(v) => Some(v),
            NovaOption::None => None,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nova_string() {
        let s = NovaString::new("hello");
        assert_eq!(s.len(), 5);
        assert_eq!(s.as_str(), "hello");
        
        let empty = NovaString::empty();
        assert!(empty.is_empty());
        
        let s2 = NovaString::new(" world");
        let concatenated = s.concat(&s2);
        assert_eq!(concatenated.as_str(), "hello world");
    }

    #[test]
    fn test_nova_array() {
        let mut arr = NovaArray::<i32>::new();
        assert!(arr.is_empty());
        
        arr.push(1);
        arr.push(2);
        arr.push(3);
        
        assert_eq!(arr.len(), 3);
        assert_eq!(arr.get(0), Some(&1));
        assert_eq!(arr.get(1), Some(&2));
        
        arr.set(1, 20).unwrap();
        assert_eq!(arr.get(1), Some(&20));
        
        assert_eq!(arr.pop(), Some(3));
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_nova_option() {
        let some_val: NovaOption<i32> = NovaOption::some(42);
        assert!(some_val.is_some());
        assert!(!some_val.is_none());
        
        let none_val: NovaOption<i32> = NovaOption::none();
        assert!(none_val.is_none());
        
        assert_eq!(some_val.unwrap_or(0), 42);
        assert_eq!(none_val.unwrap_or(0), 0);
    }
}
