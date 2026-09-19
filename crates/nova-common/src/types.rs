//! Type system definitions

use std::fmt;
use std::rc::Rc;

/// Primitive types in NOVA
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    // Integers
    I8, I16, I32, I64, I128,
    U8, U16, U32, U64, U128,
    // Floats
    F32, F64,
    // Other primitives
    Bool, Char,
    // Unit type
    Unit,
    // Never type (bottom)
    Never,
}

impl fmt::Display for PrimitiveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrimitiveType::I8 => write!(f, "i8"),
            PrimitiveType::I16 => write!(f, "i16"),
            PrimitiveType::I32 => write!(f, "i32"),
            PrimitiveType::I64 => write!(f, "i64"),
            PrimitiveType::I128 => write!(f, "i128"),
            PrimitiveType::U8 => write!(f, "u8"),
            PrimitiveType::U16 => write!(f, "u16"),
            PrimitiveType::U32 => write!(f, "u32"),
            PrimitiveType::U64 => write!(f, "u64"),
            PrimitiveType::U128 => write!(f, "u128"),
            PrimitiveType::F32 => write!(f, "f32"),
            PrimitiveType::F64 => write!(f, "f64"),
            PrimitiveType::Bool => write!(f, "bool"),
            PrimitiveType::Char => write!(f, "char"),
            PrimitiveType::Unit => write!(f, "()"),
            PrimitiveType::Never => write!(f, "!"),
        }
    }
}

/// Type kind enumeration
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeKind {
    Primitive(PrimitiveType),
    Array(Rc<Type>, usize),
    Tuple(Vec<Type>),
    Function(Vec<Type>, Rc<Type>),
    Struct(String),
    Enum(String),
    Reference(Rc<Type>, bool), // mutable flag
    Pointer(Rc<Type>, bool),   // mutable flag
    Generic(String),
    Error,
}

/// A complete type representation
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Type {
    pub kind: TypeKind,
}

impl Type {
    pub fn new(kind: TypeKind) -> Self {
        Self { kind }
    }

    pub fn primitive(p: PrimitiveType) -> Self {
        Self { kind: TypeKind::Primitive(p) }
    }

    pub fn i32() -> Self {
        Self::primitive(PrimitiveType::I32)
    }

    pub fn i64() -> Self {
        Self::primitive(PrimitiveType::I64)
    }

    pub fn f64() -> Self {
        Self::primitive(PrimitiveType::F64)
    }

    pub fn bool() -> Self {
        Self::primitive(PrimitiveType::Bool)
    }

    pub fn unit() -> Self {
        Self::primitive(PrimitiveType::Unit)
    }

    pub fn string() -> Self {
        Self::struct_type("String".to_string())
    }

    pub fn str() -> Self {
        Self::primitive(PrimitiveType::I8) // Simplified for now
    }

    pub fn array(element: Type, size: usize) -> Self {
        Self { kind: TypeKind::Array(Rc::new(element), size) }
    }

    pub fn tuple(elements: Vec<Type>) -> Self {
        Self { kind: TypeKind::Tuple(elements) }
    }

    pub fn function(params: Vec<Type>, ret: Type) -> Self {
        Self { kind: TypeKind::Function(params, Rc::new(ret)) }
    }

    pub fn struct_type(name: String) -> Self {
        Self { kind: TypeKind::Struct(name) }
    }

    pub fn enum_type(name: String) -> Self {
        Self { kind: TypeKind::Enum(name) }
    }

    pub fn reference(inner: Type, mutable: bool) -> Self {
        Self { kind: TypeKind::Reference(Rc::new(inner), mutable) }
    }

    pub fn pointer(inner: Type, mutable: bool) -> Self {
        Self { kind: TypeKind::Pointer(Rc::new(inner), mutable) }
    }

    pub fn generic(name: String) -> Self {
        Self { kind: TypeKind::Generic(name) }
    }

    pub fn error() -> Self {
        Self { kind: TypeKind::Error }
    }

    pub fn is_error(&self) -> bool {
        matches!(self.kind, TypeKind::Error)
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            TypeKind::Primitive(p) => write!(f, "{}", p),
            TypeKind::Array(elem, size) => write!(f, "[{}; {}]", elem, size),
            TypeKind::Tuple(elems) => {
                let elems_str: Vec<String> = elems.iter().map(|e| format!("{}", e)).collect();
                write!(f, "({})", elems_str.join(", "))
            }
            TypeKind::Function(params, ret) => {
                let params_str: Vec<String> = params.iter().map(|p| format!("{}", p)).collect();
                write!(f, "fn({}) -> {}", params_str.join(", "), ret)
            }
            TypeKind::Struct(name) => write!(f, "{}", name),
            TypeKind::Enum(name) => write!(f, "{}", name),
            TypeKind::Reference(inner, mutability) => {
                if *mutability {
                    write!(f, "&mut {}", inner)
                } else {
                    write!(f, "&{}", inner)
                }
            }
            TypeKind::Pointer(inner, mutability) => {
                if *mutability {
                    write!(f, "*mut {}", inner)
                } else {
                    write!(f, "*const {}", inner)
                }
            }
            TypeKind::Generic(name) => write!(f, "{}", name),
            TypeKind::Error => write!(f, "<error>"),
        }
    }
}
