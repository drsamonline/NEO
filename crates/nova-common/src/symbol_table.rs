//! Symbol table for name resolution

use std::collections::HashMap;
use crate::span::Span;
use crate::types::Type;

/// Kind of symbol in the symbol table
#[derive(Debug, Clone)]
pub enum SymbolKind {
    Variable,
    Function,
    Struct,
    Enum,
    EnumVariant,
    Const,
    TypeAlias,
    Module,
    Parameter,
}

/// A symbol representing a named entity
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub typ: Option<Type>,
    pub span: Span,
    pub is_mut: bool,
    pub scope_id: usize,
}

impl Symbol {
    pub fn new(name: String, kind: SymbolKind, span: Span) -> Self {
        Self {
            name,
            kind,
            typ: None,
            span,
            is_mut: false,
            scope_id: 0,
        }
    }

    pub fn with_type(mut self, typ: Type) -> Self {
        self.typ = Some(typ);
        self
    }

    pub fn mutable(mut self, is_mut: bool) -> Self {
        self.is_mut = is_mut;
        self
    }

    pub fn with_scope(mut self, scope_id: usize) -> Self {
        self.scope_id = scope_id;
        self
    }
}

/// Scope level in the symbol table
#[derive(Debug)]
struct Scope {
    #[allow(dead_code)]
    id: usize,
    parent: Option<usize>,
    symbols: HashMap<String, Symbol>,
}

impl Scope {
    fn new(id: usize, parent: Option<usize>) -> Self {
        Self {
            id,
            parent,
            symbols: HashMap::new(),
        }
    }
}

/// Symbol table with scoped lookups
#[derive(Debug)]
pub struct SymbolTable {
    scopes: Vec<Scope>,
    current_scope: usize,
    next_scope_id: usize,
}

impl SymbolTable {
    pub fn new() -> Self {
        let mut table = Self {
            scopes: vec![],
            current_scope: 0,
            next_scope_id: 0,
        };
        // Create global scope
        table.enter_scope();
        table
    }

    /// Enter a new nested scope
    pub fn enter_scope(&mut self) -> usize {
        let parent = if self.scopes.is_empty() {
            None
        } else {
            Some(self.current_scope)
        };
        let id = self.next_scope_id;
        self.next_scope_id += 1;
        self.scopes.push(Scope::new(id, parent));
        self.current_scope = id;
        id
    }

    /// Exit current scope, return to parent
    pub fn exit_scope(&mut self) -> Option<usize> {
        if let Some(scope) = self.scopes.get(self.current_scope) {
            if let Some(parent) = scope.parent {
                self.current_scope = parent;
                return Some(parent);
            }
        }
        None
    }

    /// Insert a symbol into current scope
    pub fn insert(&mut self, mut symbol: Symbol) -> Result<(), Symbol> {
        symbol.scope_id = self.current_scope;
        if let Some(scope) = self.scopes.get_mut(self.current_scope) {
            if let Some(existing) = scope.symbols.insert(symbol.name.clone(), symbol.clone()) {
                return Err(existing);
            }
        }
        Ok(())
    }

    /// Lookup a symbol, searching from current scope outward
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        let mut scope_id = Some(self.current_scope);
        while let Some(id) = scope_id {
            if let Some(scope) = self.scopes.get(id) {
                if let Some(symbol) = scope.symbols.get(name) {
                    return Some(symbol);
                }
                scope_id = scope.parent;
            } else {
                break;
            }
        }
        None
    }

    /// Lookup a symbol only in current scope
    pub fn lookup_local(&self, name: &str) -> Option<&Symbol> {
        self.scopes
            .get(self.current_scope)
            .and_then(|s| s.symbols.get(name))
    }

    /// Get current scope ID
    pub fn current_scope_id(&self) -> usize {
        self.current_scope
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}
