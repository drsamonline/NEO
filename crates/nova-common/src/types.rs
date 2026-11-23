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

/// The C struct name for `Option<payload>`.
///
/// This lives here rather than in the lowerer or the backend because **both need it and they
/// disagreed**. `[int]` and `int` both mangled to `Option_int` — the brackets are not
/// alphanumeric — so an `Option<[int]>` was declared as an `Option<int>` and its array payload
/// was read as a `long long`. Fixing it meant fixing it twice, and two copies of one rule is
/// the defect this project has found more often than any other.
pub fn option_struct_name(payload: &str) -> String {
    let trimmed = payload.trim();
    // Arrays keep a marker of their own, so `[int]` and `int` cannot collide.
    if let Some(inner) = trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        return format!("Option_array_{}", inner.trim());
    }
    let mangled: String = trimmed
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { '_' })
        .collect();
    format!("Option_{}", mangled.trim_matches('_'))
}

#[cfg(test)]
mod tests {
    use super::option_struct_name;

    #[test]
    fn an_array_payload_does_not_collide_with_its_element() {
        // The defect this function exists to prevent: `[int]` and `int` mangled alike, so an
        // `Option<[int]>` was declared as an `Option<int>` and its payload read as a scalar.
        assert_eq!(option_struct_name("int"), "Option_int");
        assert_eq!(option_struct_name("[int]"), "Option_array_int");
        assert_ne!(option_struct_name("int"), option_struct_name("[int]"));
        assert_eq!(option_struct_name("[String]"), "Option_array_String");
    }

    #[test]
    fn a_plain_payload_keeps_its_name() {
        // Names already emitted must not change; this is what the two copies agreed on.
        assert_eq!(option_struct_name("File"), "Option_File");
        assert_eq!(option_struct_name("Point"), "Option_Point");
        assert_eq!(option_struct_name("  int  "), "Option_int");
    }
}
