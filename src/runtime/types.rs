// NOVA Runtime System - Type System Module
// Runtime type information and dynamic type checking

use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

/// Runtime type identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeId(u64);

impl TypeId {
    pub fn new(hash: u64) -> Self {
        TypeId(hash)
    }
    
    pub fn from_str(name: &str) -> Self {
        TypeId(hash_string(name))
    }
    
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

fn hash_string(s: &str) -> u64 {
    let mut hash = 5381u64;
    for byte in s.bytes() {
        hash = ((hash << 5).wrapping_add(hash)) ^ (byte as u64);
    }
    hash
}

/// Runtime type information
#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub id: TypeId,
    pub name: String,
    pub size: usize,
    pub align: usize,
    pub kind: TypeKind,
    pub fields: Vec<FieldInfo>,
    pub variants: Vec<VariantInfo>,
}

/// Kind of type
#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Primitive,
    Struct,
    Enum,
    Array,
    Pointer,
    Function,
    Generic,
}

/// Field information for structs
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub type_id: TypeId,
    pub offset: usize,
}

/// Variant information for enums
#[derive(Debug, Clone)]
pub struct VariantInfo {
    pub name: String,
    pub discriminant: i64,
    pub fields: Vec<FieldInfo>,
}

impl TypeInfo {
    pub fn new(name: &str, kind: TypeKind, size: usize, align: usize) -> Self {
        TypeInfo {
            id: TypeId::from_str(name),
            name: name.to_string(),
            size,
            align,
            kind,
            fields: Vec::new(),
            variants: Vec::new(),
        }
    }
    
    pub fn with_fields(mut self, fields: Vec<FieldInfo>) -> Self {
        self.fields = fields;
        self
    }
    
    pub fn with_variants(mut self, variants: Vec<VariantInfo>) -> Self {
        self.variants = variants;
        self
    }
    
    pub fn get_field(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.iter().find(|f| f.name == name)
    }
    
    pub fn get_variant(&self, name: &str) -> Option<&VariantInfo> {
        self.variants.iter().find(|v| v.name == name)
    }
}

/// Global type registry for runtime type lookup
pub struct TypeRegistry {
    types: RwLock<HashMap<TypeId, TypeInfo>>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        let registry = TypeRegistry {
            types: RwLock::new(HashMap::new()),
        };
        
        // Register primitive types
        registry.register_primitive("i8", 1, 1);
        registry.register_primitive("i16", 2, 2);
        registry.register_primitive("i32", 4, 4);
        registry.register_primitive("i64", 8, 8);
        registry.register_primitive("u8", 1, 1);
        registry.register_primitive("u16", 2, 2);
        registry.register_primitive("u32", 4, 4);
        registry.register_primitive("u64", 8, 8);
        registry.register_primitive("f32", 4, 4);
        registry.register_primitive("f64", 8, 8);
        registry.register_primitive("bool", 1, 1);
        registry.register_primitive("char", 4, 4);
        registry.register_primitive("()", 0, 1);
        
        registry
    }
    
    fn register_primitive(&self, name: &str, size: usize, align: usize) {
        let info = TypeInfo::new(name, TypeKind::Primitive, size, align);
        self.types.write().unwrap().insert(info.id.clone(), info);
    }
    
    pub fn register(&self, info: TypeInfo) -> Result<(), &'static str> {
        let mut types = self.types.write().unwrap();
        if types.contains_key(&info.id) {
            return Err("Type already registered");
        }
        types.insert(info.id.clone(), info);
        Ok(())
    }
    
    pub fn get(&self, id: &TypeId) -> Option<TypeInfo> {
        self.types.read().unwrap().get(id).cloned()
    }
    
    pub fn get_by_name(&self, name: &str) -> Option<TypeInfo> {
        let id = TypeId::from_str(name);
        self.get(&id)
    }
    
    pub fn count(&self) -> usize {
        self.types.read().unwrap().len()
    }
}

impl Default for TypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Dynamic value wrapper for runtime type checking
pub struct DynamicValue {
    data: Box<dyn Any + Send>,
    type_info: TypeInfo,
}

impl DynamicValue {
    pub fn new<T: Any + Send>(value: T, type_info: TypeInfo) -> Self {
        DynamicValue {
            data: Box::new(value),
            type_info,
        }
    }
    
    pub fn type_info(&self) -> &TypeInfo {
        &self.type_info
    }
    
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.data.downcast_ref::<T>()
    }
    
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.data.downcast_mut::<T>()
    }
    
    pub fn is_type(&self, type_id: &TypeId) -> bool {
        self.type_info.id == *type_id
    }
}

impl fmt::Debug for DynamicValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicValue")
            .field("type", &self.type_info.name)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_type_registry() {
        let registry = TypeRegistry::new();
        
        // Check primitive types are registered
        assert!(registry.get_by_name("i32").is_some());
        assert!(registry.get_by_name("f64").is_some());
        assert!(registry.get_by_name("bool").is_some());
        
        assert_eq!(registry.count(), 13); // 12 primitives + ()
    }
    
    #[test]
    fn test_custom_type_registration() {
        let registry = TypeRegistry::new();
        
        let point_type = TypeInfo::new("Point", TypeKind::Struct, 16, 8)
            .with_fields(vec![
                FieldInfo {
                    name: "x".to_string(),
                    type_id: TypeId::from_str("f64"),
                    offset: 0,
                },
                FieldInfo {
                    name: "y".to_string(),
                    type_id: TypeId::from_str("f64"),
                    offset: 8,
                },
            ]);
        
        registry.register(point_type).unwrap();
        
        let retrieved = registry.get_by_name("Point").unwrap();
        assert_eq!(retrieved.fields.len(), 2);
        assert_eq!(retrieved.size, 16);
    }
    
    #[test]
    fn test_dynamic_value() {
        let registry = TypeRegistry::new();
        let i32_type = registry.get_by_name("i32").unwrap();
        
        let value = DynamicValue::new(42i32, i32_type);
        
        assert!(value.is_type(&TypeId::from_str("i32")));
        assert_eq!(*value.downcast_ref::<i32>().unwrap(), 42);
    }
}
