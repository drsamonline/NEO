//! Type definitions for semantic analysis

use std::fmt;

/// Nova type system
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// Integer types (i8, i16, i32, i64, u8, u16, u32, u64)
    Int,
    /// Floating point types (f32, f64)
    Float,
    /// Boolean type
    Bool,
    /// String type
    String,
    /// Character type
    Char,
    /// Unit type ()
    Unit,
    /// Array type [T]
    Array(Box<Type>),
    /// Slice type [T]
    Slice(Box<Type>),
    /// Tuple type (T1, T2, ...)
    Tuple(Vec<Type>),
    /// Function type fn(T1, T2, ...) -> R
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
    },
    /// Reference type &T or &mut T
    Ref {
        mutable: bool,
        target: Box<Type>,
    },
    /// Pointer type *const T or *mut T
    Ptr {
        mutable: bool,
        target: Box<Type>,
    },
    /// User-defined struct type
    Struct(String),
    /// User-defined enum type
    Enum(String),
    /// Module type
    Module,
    /// Range type
    Range,
    /// Unknown type (during inference)
    Unknown,
    /// Error type (for error recovery)
    Error,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::Bool => write!(f, "bool"),
            Type::String => write!(f, "string"),
            Type::Char => write!(f, "char"),
            Type::Unit => write!(f, "()"),
            Type::Array(t) => write!(f, "[{}]", t),
            Type::Slice(t) => write!(f, "[{}]", t),
            Type::Tuple(types) => {
                let strs: Vec<String> = types.iter().map(|t| format!("{}", t)).collect();
                write!(f, "({})", strs.join(", "))
            }
            Type::Function { params, return_type } => {
                let param_strs: Vec<String> = params.iter().map(|t| format!("{}", t)).collect();
                write!(f, "fn({}) -> {}", param_strs.join(", "), return_type)
            }
            Type::Ref { mutable, target } => {
                if *mutable {
                    write!(f, "&mut {}", target)
                } else {
                    write!(f, "&{}", target)
                }
            }
            Type::Ptr { mutable, target } => {
                if *mutable {
                    write!(f, "*mut {}", target)
                } else {
                    write!(f, "*const {}", target)
                }
            }
            Type::Struct(name) => write!(f, "{}", name),
            Type::Enum(name) => write!(f, "{}", name),
            Type::Module => write!(f, "module"),
            Type::Range => write!(f, "range"),
            Type::Unknown => write!(f, "unknown"),
            Type::Error => write!(f, "error"),
        }
    }
}
