//! Type checker module
//! 
//! Provides additional type checking utilities

use super::types::Type;
use nova_common::{Span, Diagnostics};

/// Type checker context for advanced type operations
pub struct TypeChecker {
    pub diagnostics: Diagnostics,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            diagnostics: Diagnostics::new(),
        }
    }

    /// Check if a type is fully resolved (no Unknown types)
    pub fn is_fully_resolved(&self, ty: &Type) -> bool {
        match ty {
            Type::Unknown | Type::Error => false,
            Type::Array(inner) => self.is_fully_resolved(inner),
            Type::Slice(inner) => self.is_fully_resolved(inner),
            Type::Tuple(types) => types.iter().all(|t| self.is_fully_resolved(t)),
            Type::Function { params, return_type } => {
                params.iter().all(|t| self.is_fully_resolved(t)) && 
                self.is_fully_resolved(return_type)
            }
            Type::Ref { target, .. } => self.is_fully_resolved(target),
            Type::Ptr { target, .. } => self.is_fully_resolved(target),
            _ => true,
        }
    }

    /// Get the depth of nested types
    pub fn type_depth(&self, ty: &Type) -> usize {
        match ty {
            Type::Array(inner) | Type::Slice(inner) | 
            Type::Ref { target: inner, .. } | 
            Type::Ptr { target: inner, .. } => 1 + self.type_depth(inner),
            Type::Tuple(types) => {
                types.iter().map(|t| self.type_depth(t)).max().unwrap_or(0)
            }
            Type::Function { params, return_type } => {
                let param_depth = params.iter().map(|t| self.type_depth(t)).max().unwrap_or(0);
                1 + param_depth.max(self.type_depth(return_type))
            }
            _ => 0,
        }
    }

    /// Check if two types are structurally equivalent
    pub fn structural_eq(&self, left: &Type, right: &Type) -> bool {
        match (left, right) {
            (Type::Int, Type::Int) => true,
            (Type::Float, Type::Float) => true,
            (Type::Bool, Type::Bool) => true,
            (Type::String, Type::String) => true,
            (Type::Char, Type::Char) => true,
            (Type::Unit, Type::Unit) => true,
            (Type::Array(l), Type::Array(r)) => self.structural_eq(l, r),
            (Type::Slice(l), Type::Slice(r)) => self.structural_eq(l, r),
            (Type::Tuple(l_types), Type::Tuple(r_types)) => {
                if l_types.len() != r_types.len() {
                    return false;
                }
                l_types.iter().zip(r_types.iter())
                    .all(|(l, r)| self.structural_eq(l, r))
            }
            (Type::Function { params: l_params, return_type: l_ret }, 
             Type::Function { params: r_params, return_type: r_ret }) => {
                if l_params.len() != r_params.len() {
                    return false;
                }
                l_params.iter().zip(r_params.iter())
                    .all(|(l, r)| self.structural_eq(l, r)) &&
                self.structural_eq(l_ret, r_ret)
            }
            (Type::Ref { mutable: l_mut, target: l_target }, 
             Type::Ref { mutable: r_mut, target: r_target }) => {
                l_mut == r_mut && self.structural_eq(l_target, r_target)
            }
            (Type::Ptr { mutable: l_mut, target: l_target }, 
             Type::Ptr { mutable: r_mut, target: r_target }) => {
                l_mut == r_mut && self.structural_eq(l_target, r_target)
            }
            (Type::Struct(l_name), Type::Struct(r_name)) => l_name == r_name,
            (Type::Enum(l_name), Type::Enum(r_name)) => l_name == r_name,
            _ => false,
        }
    }

    /// Unwrap reference types to get the base type
    pub fn unwrap_refs<'a>(&self, ty: &'a Type) -> &'a Type {
        match ty {
            Type::Ref { target, .. } | Type::Ptr { target, .. } => self.unwrap_refs(target),
            _ => ty,
        }
    }

    /// Check if a type can be coerced to another type
    pub fn can_coerce(&self, from: &Type, to: &Type) -> bool {
        // Identity coercion
        if from == to {
            return true;
        }

        // Numeric promotions
        match (from, to) {
            (Type::Int, Type::Float) => true,
            (Type::Int, Type::Int) => true,
            (Type::Float, Type::Float) => true,
            
            // Mutable to immutable reference
            (Type::Ref { mutable: true, target: l }, Type::Ref { mutable: false, target: r }) => {
                self.structural_eq(l, r)
            }
            
            // Array to slice
            (Type::Array(l), Type::Slice(r)) => self.structural_eq(l, r),
            
            _ => false,
        }
    }

    /// Report a type error
    pub fn report_error(&mut self, span: Span, message: String) {
        // For now, just add the message - span handling would require updating Diagnostics API
        self.diagnostics.error(format!("{} (at {:?})", message, span));
    }

    /// Get accumulated diagnostics
    pub fn into_diagnostics(self) -> Diagnostics {
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_fully_resolved() {
        let checker = TypeChecker::new();
        
        assert!(checker.is_fully_resolved(&Type::Int));
        assert!(checker.is_fully_resolved(&Type::Bool));
        assert!(!checker.is_fully_resolved(&Type::Unknown));
        assert!(!checker.is_fully_resolved(&Type::Error));
        assert!(checker.is_fully_resolved(&Type::Array(Box::new(Type::Int))));
        assert!(!checker.is_fully_resolved(&Type::Array(Box::new(Type::Unknown))));
    }

    #[test]
    fn test_structural_eq() {
        let checker = TypeChecker::new();
        
        assert!(checker.structural_eq(&Type::Int, &Type::Int));
        assert!(!checker.structural_eq(&Type::Int, &Type::Float));
        assert!(checker.structural_eq(
            &Type::Array(Box::new(Type::Int)),
            &Type::Array(Box::new(Type::Int))
        ));
    }

    #[test]
    fn test_can_coerce() {
        let checker = TypeChecker::new();
        
        assert!(checker.can_coerce(&Type::Int, &Type::Float));
        assert!(checker.can_coerce(&Type::Int, &Type::Int));
        assert!(!checker.can_coerce(&Type::Float, &Type::Int));
    }
}
