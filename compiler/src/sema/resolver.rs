//! Name resolver module - simplified version using nova_common types

use nova_common::{Span, Diagnostics, SymbolTable, Symbol, SymbolKind};

/// Name resolver for resolving identifiers to their declarations
pub struct NameResolver {
    pub diagnostics: Diagnostics,
    pub symbol_table: SymbolTable,
}

impl NameResolver {
    pub fn new() -> Self {
        Self {
            diagnostics: Diagnostics::new(),
            symbol_table: SymbolTable::new(),
        }
    }

    /// Enter a new scope
    pub fn enter_scope(&mut self) {
        self.symbol_table.enter_scope();
    }

    /// Exit the current scope
    pub fn exit_scope(&mut self) {
        self.symbol_table.exit_scope();
    }

    /// Insert a name into the current scope
    pub fn insert(&mut self, name: String, type_: nova_common::Type, span: Span) {
        let kind = SymbolKind::Variable;
        let symbol = Symbol::new(name.clone(), kind, span).with_type(type_);
        
        if self.symbol_table.lookup_local(&name).is_some() {
            self.diagnostics.error(format!("Duplicate definition of '{}'", name));
        } else {
            let _ = self.symbol_table.insert(symbol);
        }
    }

    /// Resolve a name to its type
    pub fn resolve(&mut self, name: &str, span: Span) -> Option<nova_common::Type> {
        match self.symbol_table.lookup(name) {
            Some(symbol) => symbol.typ.clone(),
            None => {
                self.diagnostics.error(format!("Undefined name: {} (at {:?})", name, span));
                None
            }
        }
    }

    /// Check if a name is defined in any scope
    pub fn is_defined(&self, name: &str) -> bool {
        self.symbol_table.lookup(name).is_some()
    }

    /// Report an error
    pub fn report_error(&mut self, span: Span, message: String) {
        self.diagnostics.error(format!("{} (at {:?})", message, span));
    }

    /// Take diagnostics
    pub fn take_diagnostics(&mut self) -> Diagnostics {
        std::mem::replace(&mut self.diagnostics, Diagnostics::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nova_common::{Type, Position};

    fn make_span(start: usize, end: usize) -> Span {
        Span { 
            start: Position::new(0, start, start), 
            end: Position::new(0, end, end) 
        }
    }

    #[test]
    fn test_basic_resolution() {
        let mut resolver = NameResolver::new();
        
        resolver.enter_scope();
        resolver.insert("x".to_string(), Type::i32(), make_span(0, 1));
        
        assert!(resolver.is_defined("x"));
        assert!(!resolver.is_defined("y"));
        
        let resolved = resolver.resolve("x", make_span(5, 6));
        assert_eq!(resolved, Some(Type::i32()));
        
        resolver.exit_scope();
    }

    #[test]
    fn test_scope_nesting() {
        let mut resolver = NameResolver::new();
        
        resolver.enter_scope(); // Global scope
        resolver.insert("global".to_string(), Type::i32(), make_span(0, 6));
        
        resolver.enter_scope(); // Inner scope
        resolver.insert("local".to_string(), Type::bool(), make_span(10, 15));
        
        // Can see both global and local
        assert!(resolver.is_defined("global"));
        assert!(resolver.is_defined("local"));
        
        resolver.exit_scope(); // Back to global
        
        // Can only see global
        assert!(resolver.is_defined("global"));
        assert!(!resolver.is_defined("local"));
        
        resolver.exit_scope();
    }

    #[test]
    fn test_duplicate_detection() {
        let mut resolver = NameResolver::new();
        
        resolver.enter_scope();
        resolver.insert("x".to_string(), Type::i32(), make_span(0, 1));
        resolver.insert("x".to_string(), Type::bool(), make_span(5, 6));
        
        // Should have detected duplicate
        assert!(resolver.diagnostics.has_errors());
    }
}
