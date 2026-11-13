//! Semantic Analyzer Module - Simplified for v1

use nova_common::{AstNode, Diagnostic, Program, Item, Function, UseDecl, ImplBlock, Diagnostics, Symbol, SymbolKind, Span};
use std::collections::HashMap;

pub mod types;
pub mod checker;
pub mod resolver;

use self::types::Type;

/// The function whose body is currently being checked, so `return` can be
/// validated against its declared return type.
#[derive(Clone)]
struct ReturnContext {
    name: String,
    typ: Option<Type>,
    span: Span,
    /// The function's generic parameters; a return type naming one of them is
    /// not compared before instantiation.
    generics: Vec<String>,
}

/// Declared shape of a user function: what calls are checked against.
#[derive(Clone)]
struct FunctionSig {
    params: Vec<Type>,
    return_type: Option<Type>,
    generics: Vec<String>,
}

/// Semantic analyzer context
pub struct SemanticAnalyzer {
    pub diagnostics: Diagnostics,
    pub symbol_table: nova_common::SymbolTable,
    /// Declared functions, used to type call results and check arguments.
    signatures: HashMap<String, FunctionSig>,
    /// Stack of enclosing functions during body checking.
    return_context: Vec<ReturnContext>,
    /// Local variable types (parameters and annotated `let`s), innermost last.
    type_scopes: Vec<HashMap<String, Type>>,
    /// Struct name -> field name -> declared type.
    struct_fields: HashMap<String, HashMap<String, Type>>,
    /// Span of the expression currently being checked, so nested identifiers that
    /// are not individually spanned still report a useful location.
    current_span: Option<Span>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        let mut analyzer = Self {
            diagnostics: Diagnostics::new(),
            symbol_table: nova_common::SymbolTable::new(),
            signatures: HashMap::new(),
            return_context: Vec::new(),
            type_scopes: Vec::new(),
            struct_fields: HashMap::new(),
            current_span: None,
        };
        
        // Register built-in functions
        analyzer.register_builtins();
        
        analyzer
    }
    
    /// Register built-in functions and types
    fn register_builtins(&mut self) {
        // Register print function
        let print_symbol = Symbol::new(
            "print".to_string(),
            SymbolKind::Function,
            Span::default(),
        );
        let _ = self.symbol_table.insert(print_symbol);
        
        // Register println function
        let println_symbol = Symbol::new(
            "println".to_string(),
            SymbolKind::Function,
            Span::default(),
        );
        let _ = self.symbol_table.insert(println_symbol);
        
        // Register panic function
        let panic_symbol = Symbol::new(
            "panic".to_string(),
            SymbolKind::Function,
            Span::default(),
        );
        let _ = self.symbol_table.insert(panic_symbol);
    }

    /// Analyze a complete program
    pub fn analyze(&mut self, program: &Program) -> Result<(), ()> {
        self.symbol_table.enter_scope(); // Global scope
        
        // First pass: collect declarations
        for item in &program.items {
            self.collect_declaration(item);
        }
        
        // Second pass: check bodies
        for item in &program.items {
            self.check_item(item);
        }
        
        if let Some(_) = self.symbol_table.exit_scope() {}
        
        if self.diagnostics.has_errors() {
            Err(())
        } else {
            Ok(())
        }
    }

    /// Collect declarations (first pass)
    fn collect_declaration(&mut self, item: &Item) {
        match item {
            Item::Function(func) => {
                // Don't set type yet - function types are complex
                let symbol = Symbol::new(func.name.clone(), SymbolKind::Function, func.span);
                let _ = self.symbol_table.insert(symbol);
                // Remember the declared return type so `return` can be checked
                // against it and calls can be typed.
                self.signatures.insert(
                    func.name.clone(),
                    FunctionSig {
                        params: func
                            .params
                            .iter()
                            .map(|param| type_from_name(&param.typ.name))
                            .collect(),
                        return_type: func.return_type.as_ref().map(|t| type_from_name(&t.name)),
                        generics: func.generics.clone(),
                    },
                );
            }
            Item::Struct(struct_def) => {
                let symbol = Symbol::new(struct_def.name.clone(), SymbolKind::Struct, struct_def.span);
                let _ = self.symbol_table.insert(symbol);
                self.struct_fields.insert(
                    struct_def.name.clone(),
                    struct_def
                        .fields
                        .iter()
                        .map(|field| (field.name.clone(), type_from_name(&field.typ.name)))
                        .collect(),
                );
            }
            Item::Enum(enum_def) => {
                let symbol = Symbol::new(enum_def.name.clone(), SymbolKind::Enum, enum_def.span);
                let _ = self.symbol_table.insert(symbol);
            }
            Item::Const(const_def) => {
                // Don't set type yet - will be inferred or checked later
                let symbol = Symbol::new(const_def.name.clone(), SymbolKind::Const, const_def.span);
                let _ = self.symbol_table.insert(symbol);
            }
            Item::Use(use_decl) => {
                self.resolve_use_decl(use_decl);
            }
            Item::Mod(module_decl) => {
                let symbol = Symbol::new(module_decl.name.clone(), SymbolKind::Module, module_decl.span);
                let _ = self.symbol_table.insert(symbol);
            }
            Item::Impl(_impl_block) => {
                // Handle impl blocks in second pass
            }
            Item::TypeAlias(_type_alias) => {
                // Handle type aliases
            }
            Item::Trait(_trait_def) => {
                // Handle traits
            }
        }
    }

    /// Check item bodies (second pass)
    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Function(func) => {
                self.check_function(func);
            }
            Item::Const(const_def) => {
                // Check const value expression
                self.check_expr(&const_def.value);
            }
            Item::Impl(impl_block) => {
                self.check_impl_block(impl_block);
            }
            _ => {}
        }
    }

    /// Check function body
    fn check_function(&mut self, func: &Function) {
        self.symbol_table.enter_scope(); // Function scope
        
        // Add parameters to symbol table
        for param in &func.params {
            let symbol = Symbol::new(param.name.clone(), SymbolKind::Parameter, param.span);
            let _ = self.symbol_table.insert(symbol);
        }

        self.return_context.push(ReturnContext {
            name: func.name.clone(),
            typ: func.return_type.as_ref().map(|t| type_from_name(&t.name)),
            span: func.span,
            generics: func.generics.clone(),
        });
        self.type_scopes.push(
            func.params
                .iter()
                .map(|param| (param.name.clone(), type_from_name(&param.typ.name)))
                .collect(),
        );
        
        // Check function body
        if let Some(block) = &func.body {
            self.check_block(block);
            if let Some(final_expr) = &block.final_expr {
                self.check_tail_value(func, final_expr);
            }
            self.check_missing_return(func, block);
        }

        self.type_scopes.pop();
        self.return_context.pop();
        if let Some(_) = self.symbol_table.exit_scope() {}
    }

    /// Check a block statement
    fn check_block(&mut self, block: &nova_common::Block) {
        for stmt in &block.statements {
            self.check_statement(stmt);
        }
        
        // Check final expression (tail expr)
        if let Some(expr) = &block.final_expr {
            self.check_expr(expr);
        }
    }

    /// Check a statement
    fn check_statement(&mut self, stmt: &nova_common::Statement) {
        match stmt {
            // A local `const` is visible to the statements after it.
            nova_common::Statement::Item(nova_common::Item::Const(const_def)) => {
                let symbol =
                    Symbol::new(const_def.name.clone(), SymbolKind::Const, const_def.span);
                let _ = self.symbol_table.insert(symbol);
                if let Some(scope) = self.type_scopes.last_mut() {
                    if let Some(typ) = const_def.typ.as_ref().map(|t| type_from_name(&t.name)) {
                        scope.insert(const_def.name.clone(), typ);
                    }
                }
                self.check_expr(&const_def.value);
            }
            nova_common::Statement::Let(let_stmt) => {
                // Add every name the pattern binds to the symbol table.
                let mut names = Vec::new();
                collect_pattern_names(&let_stmt.pattern, &mut names);
                for ident_name in &names {
                    let symbol = Symbol::new(ident_name.clone(), SymbolKind::Variable, let_stmt.span);
                    let _ = self.symbol_table.insert(symbol);
                }
                
                // Check initialization expression
                if let Some(init_expr) = &let_stmt.init {
                    self.check_expr(init_expr);
                }

                // Record the binding's type so later expressions can be inferred.
                let declared = let_stmt.typ.as_ref().map(|t| type_from_name(&t.name));
                let binding = declared.or_else(|| {
                    let_stmt
                        .init
                        .as_ref()
                        .and_then(|expr| self.infer_expr_type(expr))
                });
                match &let_stmt.pattern {
                    nova_common::Pattern::Tuple(patterns) => {
                        // Element types come from the initialiser's tuple type.
                        let element_types = match &binding {
                            Some(Type::Tuple(types)) => types.clone(),
                            _ => Vec::new(),
                        };
                        for (index, pattern) in patterns.iter().enumerate() {
                            let nova_common::Pattern::Identifier(ident_name) = pattern else {
                                continue;
                            };
                            if let (Some(scope), Some(typ)) =
                                (self.type_scopes.last_mut(), element_types.get(index).cloned())
                            {
                                scope.insert(ident_name.clone(), typ);
                            }
                        }
                    }
                    nova_common::Pattern::Identifier(ident_name) => {
                        if let (Some(scope), Some(typ)) = (self.type_scopes.last_mut(), binding) {
                            scope.insert(ident_name.clone(), typ);
                        }
                    }
                    _ => {}
                }
            }
            nova_common::Statement::Expr(expr) => {
                self.check_expr(expr);
            }
            nova_common::Statement::Item(item) => {
                self.check_item(item);
            }
        }
    }

    /// Check an expression
    fn check_expr(&mut self, expr: &nova_common::Expr) {
        match expr {
            nova_common::Expr::Spanned(inner, span) => {
                // An identifier under a span reports with that span so editors can
                // point at the exact name instead of the enclosing expression.
                if let nova_common::Expr::Identifier(name) = inner.as_ref() {
                    self.check_identifier(name, Some(*span));
                } else if let nova_common::Expr::Call(func, args) = inner.as_ref() {
                    let outer = self.current_span.replace(*span);
                    self.check_expr(func);
                    for arg in args {
                        self.check_expr(arg);
                    }
                    self.current_span = outer;
                    self.check_call_arguments(func, args, Some(*span));
                } else {
                    let outer = self.current_span.replace(*span);
                    self.check_expr(inner);
                    self.current_span = outer;
                }
            }
            nova_common::Expr::Literal(_lit) => {
                // Literals are always valid
            }
            nova_common::Expr::Binary(left, _op, right) => {
                self.check_expr(left);
                self.check_expr(right);
            }
            nova_common::Expr::Unary(_op, expr_inner) => {
                self.check_expr(expr_inner);
            }
            nova_common::Expr::Identifier(name) => {
                self.check_identifier(name, None);
            }
            nova_common::Expr::Call(func, args) => {
                self.check_expr(func);
                for arg in args {
                    self.check_expr(arg);
                }
                self.check_call_arguments(func, args, None);
            }
            nova_common::Expr::Field(object, _field) => {
                self.check_expr(object);
            }
            nova_common::Expr::Index(array, index) => {
                self.check_expr(array);
                self.check_expr(index);
            }
            nova_common::Expr::Range(start, end) => {
                self.check_expr(start);
                self.check_expr(end);
            }
            nova_common::Expr::If(condition, then_branch, else_branch) => {
                self.check_expr(condition);
                self.check_block(then_branch);
                if let Some(else_block) = else_branch {
                    self.check_block(else_block);
                }
            }
            nova_common::Expr::Match(scrutinee, arms) => {
                self.check_expr(scrutinee);
                for arm in arms {
                    // A payload pattern binds its variant's value for the arm.
                    let binding = match &arm.pattern {
                        nova_common::Pattern::EnumVariant(_, _, Some(inner)) => {
                            match inner.as_ref() {
                                nova_common::Pattern::Identifier(name) if name != "_" => {
                                    Some(name.clone())
                                }
                                _ => None,
                            }
                        }
                        // `x if x > 10 => …` binds the scrutinee for the arm.
                        nova_common::Pattern::Identifier(name) if name != "_" => {
                            Some(name.clone())
                        }
                        _ => None,
                    };
                    if let Some(name) = &binding {
                        let symbol = Symbol::new(
                            name.clone(),
                            SymbolKind::Variable,
                            nova_common::Span::default(),
                        );
                        let _ = self.symbol_table.insert(symbol);
                        let mut scope = HashMap::new();
                        let binding_type = match &arm.pattern {
                            nova_common::Pattern::Identifier(_) => self
                                .infer_expr_type(scrutinee)
                                .unwrap_or(Type::Int),
                            _ => Type::Int,
                        };
                        scope.insert(name.clone(), binding_type);
                        self.type_scopes.push(scope);
                    }

                    self.check_expr(&arm.body);
                    if let Some(guard) = &arm.guard {
                        self.check_expr(guard);
                    }
                    if binding.is_some() {
                        self.type_scopes.pop();
                    }
                }
            }
            nova_common::Expr::While(condition, body) => {
                self.check_expr(condition);
                self.check_block(body);
            }
            nova_common::Expr::Loop(body) => {
                self.check_block(body);
            }
            nova_common::Expr::For(pattern, iterable, body) => {
                self.check_expr(iterable);
                // Add loop variable to scope
                if let nova_common::Pattern::Identifier(ident_name) = pattern {
                    let symbol = Symbol::new(ident_name.clone(), SymbolKind::Variable, nova_common::Span::default());
                    let _ = self.symbol_table.insert(symbol);
                }
                
                // Enter a new scope for the loop variable
                self.symbol_table.enter_scope();
                
                // Add loop variable(s) to symbol table
                self.add_pattern_to_scope(pattern);
                
                // Check loop body
                self.check_block(body);
                
                // Exit the loop scope
                if let Some(_) = self.symbol_table.exit_scope() {}
            }
            nova_common::Expr::Block(block) => {
                self.check_block(block);
            }
            nova_common::Expr::Return(expr_opt) => {
                if let Some(expr_inner) = expr_opt {
                    self.check_expr(expr_inner);
                }
                self.check_return_value(expr_opt.as_deref());
            }
            nova_common::Expr::Break => {}
            nova_common::Expr::Continue => {}
            nova_common::Expr::Assign(target, value) => {
                self.check_expr(target);
                self.check_expr(value);
            }
            nova_common::Expr::Cast(expr_inner, _type_expr) => {
                self.check_expr(expr_inner);
            }
            nova_common::Expr::Path(_path_parts) => {
                // Check path resolution
            }
            nova_common::Expr::StructLiteral(_name, fields) => {
                for (_field, value) in fields {
                    if let Some(value) = value {
                        self.check_expr(value);
                    }
                }
            }
            nova_common::Expr::Tuple(items) => {
                for item in items {
                    self.check_expr(item);
                }
            }
            nova_common::Expr::Array(items) => {
                for item in items {
                    self.check_expr(item);
                }
            }
            nova_common::Expr::Closure(params, body) => {
                self.symbol_table.enter_scope();
                for (name, _typ) in params {
                    let symbol = Symbol::new(name.clone(), SymbolKind::Parameter, Span::default());
                    let _ = self.symbol_table.insert(symbol);
                }
                self.check_expr(body);
                if let Some(_) = self.symbol_table.exit_scope() {}
            }
        }
    }

    /// Check that an identifier resolves, reporting the name's span when known.
    fn check_identifier(&mut self, name: &str, span: Option<Span>) {
        // `__builtin_*` names and the built-in `Option` constructors are
        // declared by the compiler, not the program.
        if name.starts_with("__builtin_") || name == "Some" || name == "None" {
            return;
        }
        if self.symbol_table.lookup(name).is_some() {
            return;
        }

        let mut diag = Diagnostic::error(format!("undefined name `{name}`"))
            .with_code("E0002")
            .with_help(format!(
                "declare `{name}` first (`let {name} = ...` or `fn {name}(...)`), or check the spelling"
            ));
        if let Some(span) = span.or(self.current_span) {
            diag = diag.with_span(span);
        }
        self.diagnostics.push(diag);
    }

    /// Check a call's arity and argument types against the callee's signature.
    ///
    /// Only user functions have signatures - builtins (`print`, `println`,
    /// `panic`) do not - and an argument whose type is unknown is not guessed at.
    fn check_call_arguments(
        &mut self,
        callee: &nova_common::Expr,
        args: &[nova_common::Expr],
        call_span: Option<Span>,
    ) {
        let nova_common::Expr::Identifier(name) = callee.unspanned() else {
            return;
        };
        // Builtins are declared by the compiler with no user-visible signature;
        // checking them against one would only produce false errors.
        if name.starts_with("__builtin_") || name == "Some" {
            return;
        }
        let Some(signature) = self.signatures.get(name).cloned() else {
            return;
        };
        let span = call_span.or_else(|| self.return_context.last().map(|context| context.span));

        if args.len() != signature.params.len() {
            let mut diagnostic = Diagnostic::error(format!(
                "function `{}` takes {} argument{}, but {} {} given",
                name,
                signature.params.len(),
                if signature.params.len() == 1 { "" } else { "s" },
                args.len(),
                if args.len() == 1 { "was" } else { "were" },
            ))
            .with_code("E0061")
            .with_help(format!(
                "`{}` expects ({})",
                name,
                signature
                    .params
                    .iter()
                    .map(|typ| typ.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if let Some(span) = span {
                diagnostic = diagnostic.with_span(span);
            }
            self.diagnostics.push(diagnostic);
            return;
        }

        // Generic parameters are substituted when the function is instantiated,
        // which sema does not do yet; only the arity is enforced here.
        if !signature.generics.is_empty() {
            return;
        }

        for (index, (arg, expected)) in args.iter().zip(signature.params.iter()).enumerate() {
            let Some(found) = self.infer_expr_type(arg) else {
                continue;
            };
            if !self.types_compatible(&found, expected) {
                let mut diagnostic = Diagnostic::error(format!(
                    "argument {} of `{}` is `{}`, but the parameter is `{}`",
                    index + 1,
                    name,
                    found,
                    expected
                ))
                .with_code("E0308");
                if let Some(span) = span_of(arg).or(call_span) {
                    diagnostic = diagnostic.with_span(span);
                }
                self.diagnostics.push(diagnostic);
            }
        }
    }

    /// The type recorded for a local variable or parameter, if any.
    fn lookup_variable_type(&self, name: &str) -> Option<Type> {
        self.type_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    /// Check a `return` expression against the enclosing function's declared
    /// return type.
    fn check_return_value(&mut self, value: Option<&nova_common::Expr>) {
        let Some(context) = self.return_context.last().cloned() else {
            return;
        };

        // `-> T` cannot be compared until T is instantiated.
        if let Some(Type::Struct(name)) = &context.typ {
            if context.generics.iter().any(|generic| generic == name) {
                return;
            }
        }

        match (&context.typ, value) {
            // A `!` function cannot return, so no value can be wrong here.
            (Some(Type::Never), _) => {}
            (Some(expected), Some(expr)) => {
                if matches!(expected, Type::Unit) {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "function `{}` returns nothing, but this `return` gives a value",
                            context.name
                        ))
                        .with_span(context.span),
                    );
                    return;
                }
                // Unknown expression types are not guessed at: silence beats a
                // false error while inference is partial.
                if let Some(found) = self.infer_expr_type(expr) {
                    if !self.types_compatible(&found, expected) {
                        self.diagnostics.push(
                            Diagnostic::error(format!(
                                "function `{}` returns `{}`, but this return value is `{}`",
                                context.name, expected, found
                            ))
                            .with_code("E0308")
                            .with_span(span_of(expr).unwrap_or(context.span))
                            .with_help(format!(
                                "return a `{}` value, or change the declared return type",
                                expected
                            )),
                        );
                    }
                }
            }
            (Some(expected), None) => {
                if !matches!(expected, Type::Unit) {
                    self.diagnostics.push(
                        Diagnostic::error(format!(
                            "function `{}` returns `{}`, but this `return` has no value",
                            context.name, expected
                        ))
                        .with_code("E0308")
                        .with_span(context.span),
                    );
                }
            }
            (None, Some(_)) => {
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "function `{}` has no return type, but this `return` gives a value",
                        context.name
                    ))
                    .with_span(context.span),
                );
            }
            (None, None) => {}
        }
    }

    /// Check a function whose value is produced by a trailing expression
    /// (`fn f() -> int { 1 }`).
    fn check_tail_value(&mut self, func: &Function, expr: &nova_common::Expr) {
        let Some(expected) = func.return_type.as_ref().map(|t| type_from_name(&t.name)) else {
            return;
        };
        if matches!(expected, Type::Unit | Type::Never) {
            return;
        }
        if let Some(found) = self.infer_expr_type(expr) {
            if !self.types_compatible(&found, &expected) {
                self.diagnostics.push(
                    Diagnostic::error(format!(
                        "function `{}` returns `{}`, but its final expression is `{}`",
                        func.name, expected, found
                    ))
                    .with_code("E0308")
                    .with_span(span_of(expr).unwrap_or(func.span)),
                );
            }
        }
    }

    /// Flag a function that must produce a value but cannot: it declares a
    /// non-unit return type and has no `return`, no tail expression and no
    /// diverging loop at the end. Conservative: a conditional `return` counts.
    fn check_missing_return(&mut self, func: &Function, block: &nova_common::Block) {
        // A generic function's body is checked per instantiation, which sema
        // does not do; do not claim it must return every path.
        if !func.generics.is_empty() {
            return;
        }
        let Some(expected) = func.return_type.as_ref().map(|t| type_from_name(&t.name)) else {
            return;
        };
        if matches!(expected, Type::Unit | Type::Never) || block.final_expr.is_some() {
            return;
        }
        if block_contains_return(block) || ends_with_loop(block) {
            return;
        }
        self.diagnostics.push(
            Diagnostic::error(format!(
                "function `{}` returns `{}`, but its body has no `return` and no final expression",
                func.name, expected
            ))
            .with_code("E0308")
            .with_span(func.span),
        );
    }

    /// Types are compatible when they match structurally or the found type may
    /// be coerced (the only coercion today is `int` -> `float`).
    fn types_compatible(&self, found: &Type, expected: &Type) -> bool {
        // A reference parameter accepts its target type; mutability is not
        // enforced yet. Array-ish spellings (`Array`, `[T]`, `&mut [T]`) are one
        // family, because arrays carry their own runtime representation.
        if let Type::Ref { target, .. } = expected {
            return self.types_compatible(found, target);
        }
        if is_array_like(found) && is_array_like(expected) {
            return true;
        }
        let checker = checker::TypeChecker::new();
        checker.structural_eq(found, expected) || checker.can_coerce(found, expected)
    }

    /// The type of an expression, where it is cheap to know. `None` means
    /// "not known", never "no type".
    fn infer_expr_type(&self, expr: &nova_common::Expr) -> Option<Type> {
        use nova_common::{BinOp, Expr, Literal, UnOp};

        match expr {
            Expr::Spanned(inner, _) => self.infer_expr_type(inner),
            Expr::Literal(lit) => Some(match lit {
                Literal::Integer(_) => Type::Int,
                Literal::Float(_) => Type::Float,
                Literal::Bool(_) => Type::Bool,
                Literal::String(_) => Type::String,
                Literal::Char(_) => Type::Char,
            }),
            Expr::Identifier(name) => self.lookup_variable_type(name),
            Expr::Binary(left, op, right) => match op {
                BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
                | BinOp::And | BinOp::Or => Some(Type::Bool),
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                    match (self.infer_expr_type(left), self.infer_expr_type(right)) {
                        (Some(Type::Float), _) | (_, Some(Type::Float)) => Some(Type::Float),
                        (Some(Type::Int), Some(Type::Int)) => Some(Type::Int),
                        _ => None,
                    }
                }
                _ => None,
            },
            Expr::Unary(op, inner) => match op {
                UnOp::Not => Some(Type::Bool),
                UnOp::Neg => self.infer_expr_type(inner),
                _ => None,
            },
            Expr::Call(callee, _args) => match callee.as_ref() {
                Expr::Identifier(name) if name == "Some" => None,
                Expr::Identifier(name) => {
                    if let Some(builtin) = builtin_return_type(name) {
                        Some(builtin)
                    } else {
                        self.signatures.get(name).and_then(|signature| {
                            if signature.generics.is_empty() {
                                signature.return_type.clone()
                            } else {
                                None
                            }
                        })
                    }
                }
                _ => None,
            },
            Expr::Tuple(items) => {
                let types: Option<Vec<Type>> =
                    items.iter().map(|item| self.infer_expr_type(item)).collect();
                types.map(Type::Tuple)
            }
            Expr::If(_, then_block, _else_block) => then_block
                .final_expr
                .as_ref()
                .and_then(|e| self.infer_expr_type(e)),
            Expr::Block(block) => block
                .final_expr
                .as_ref()
                .and_then(|e| self.infer_expr_type(e)),
            Expr::Cast(_inner, ty) => Some(type_from_name(&ty.name)),
            Expr::StructLiteral(name, _) => Some(Type::Struct(name.clone())),
            Expr::Field(object, field) => match self.infer_expr_type(object) {
                Some(Type::Struct(name)) => self
                    .struct_fields
                    .get(&name)
                    .and_then(|fields| fields.get(field).cloned()),
                _ => None,
            },
            _ => None,
        }
    }

    /// Resolve use declaration
    fn resolve_use_decl(&mut self, _use_decl: &UseDecl) {
        // Simplified import resolution
    }

    /// Check impl block
    fn check_impl_block(&mut self, impl_block: &ImplBlock) {
        // Validate that the type being implemented exists
        if self.symbol_table.lookup(&impl_block.type_name).is_none() {
            self.diagnostics.push(nova_common::Diagnostic::error(
                format!("Cannot implement methods on undefined type '{}'", impl_block.type_name),
            ).with_span(impl_block.span));
            return;
        }
        
        // Check each method in the impl block
        for method in &impl_block.methods {
            // Add 'self' parameter to scope for instance methods
            if !method.params.is_empty() && method.params[0].name == "self" {
                // Create a symbol for self with the type being implemented
                let self_symbol = Symbol::new(
                    "self".to_string(),
                    SymbolKind::Variable,
                    method.params[0].span,
                );
                let _ = self.symbol_table.insert(self_symbol);
            }
            
            // Check the method body
            self.check_function(method);
            
            // Remove self from scope after checking the method (exit function scope handles this)
        }
    }
    
    /// Add pattern bindings to current scope
    fn add_pattern_to_scope(&mut self, pattern: &nova_common::Pattern) {
        match pattern {
            nova_common::Pattern::Identifier(name) => {
                let symbol = Symbol::new(name.clone(), SymbolKind::Variable, Span::default());
                let _ = self.symbol_table.insert(symbol);
            }
            nova_common::Pattern::Tuple(patterns) => {
                for p in patterns {
                    self.add_pattern_to_scope(p);
                }
            }
            nova_common::Pattern::Struct(_, field_patterns) => {
                for (_, p) in field_patterns {
                    self.add_pattern_to_scope(p);
                }
            }
            nova_common::Pattern::EnumVariant(_, _, pattern_opt) => {
                if let Some(p) = pattern_opt {
                    self.add_pattern_to_scope(p);
                }
            }
            nova_common::Pattern::Path(_) => {
                // A qualified variant path binds nothing.
            }
            nova_common::Pattern::Wildcard | 
            nova_common::Pattern::Literal(_) => {
                // These patterns don't introduce new bindings
            }
        }
    }
}

/// A declared type name as the analyzer's `Type`. Unknown names become
/// `Struct(name)`, so two mentions of the same name still compare equal.
/// Return type of a compiler-declared `__builtin_*` function, when it is one.
/// `None` also covers builtins whose result type depends on the argument.
/// True for the spellings that name an array: `Array`, `[T]` and references to
/// either.
/// Names bound by a pattern, including tuple elements.
fn collect_pattern_names(pattern: &nova_common::Pattern, names: &mut Vec<String>) {
    match pattern {
        nova_common::Pattern::Identifier(name) if name != "_" => names.push(name.clone()),
        nova_common::Pattern::Tuple(patterns) => {
            for pattern in patterns {
                collect_pattern_names(pattern, names);
            }
        }
        _ => {}
    }
}

fn is_array_like(typ: &Type) -> bool {
    match typ {
        Type::Array(_) | Type::Slice(_) => true,
        Type::Ref { target, .. } => is_array_like(target),
        Type::Struct(name) => {
            name.eq_ignore_ascii_case("array") || name.contains('[') || name.starts_with("Array")
        }
        _ => false,
    }
}

fn builtin_return_type(name: &str) -> Option<Type> {
    Some(match name {
        "__builtin_panic" | "__builtin_exit" => Type::Never,
        "__builtin_print" | "__builtin_println" => Type::Unit,
        "__builtin_strlen" | "__builtin_strcmp" | "__builtin_array_len" => Type::Int,
        "__builtin_typeof" | "__builtin_tostring" | "__builtin_strcat" | "__builtin_substr" => {
            Type::String
        }
        _ => return None,
    })
}

fn type_from_name(name: &str) -> Type {
    let name = name.trim();
    if name == "!" {
        return Type::Never;
    }
    // `(A, B)` is a tuple type, not a struct whose name happens to start with `(`.
    if let Some(elements) = nova_common::sir::tuple_element_types(name) {
        if elements.is_empty() {
            return Type::Unit;
        }
        return Type::Tuple(elements.iter().map(|element| type_from_name(element)).collect());
    }
    // `fn(A, B) -> C` is a function type.
    if let Some(rest) = name.strip_prefix("fn(") {
        if let Some((params, return_type)) = rest.split_once(") -> ") {
            let params = if params.trim().is_empty() {
                Vec::new()
            } else {
                params.split(',').map(|param| type_from_name(param)).collect()
            };
            return Type::Function {
                params,
                return_type: Box::new(type_from_name(return_type)),
            };
        }
    }
    match name.to_lowercase().as_str() {
        "int" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" => Type::Int,
        "float" | "f32" | "f64" => Type::Float,
        "bool" => Type::Bool,
        "string" | "str" => Type::String,
        "char" => Type::Char,
        "unit" | "void" | "()" => Type::Unit,
        _ => Type::Struct(name.to_string()),
    }
}

fn block_contains_return(block: &nova_common::Block) -> bool {
    block.statements.iter().any(stmt_contains_return)
        || block
            .final_expr
            .as_ref()
            .map(|expr| expr_contains_return(expr))
            .unwrap_or(false)
}

fn stmt_contains_return(stmt: &nova_common::Statement) -> bool {
    use nova_common::Statement;
    match stmt {
        Statement::Let(let_stmt) => let_stmt
            .init
            .as_ref()
            .map(|expr| expr_contains_return(expr))
            .unwrap_or(false),
        Statement::Expr(expr) => expr_contains_return(expr),
        Statement::Item(nova_common::Item::Function(func)) => func
            .body
            .as_ref()
            .map(|block| block_contains_return(block))
            .unwrap_or(false),
        Statement::Item(_) => false,
    }
}

fn expr_contains_return(expr: &nova_common::Expr) -> bool {
    use nova_common::Expr;
    match expr {
        Expr::Return(_) => true,
        Expr::Spanned(inner, _) => expr_contains_return(inner),
        Expr::Tuple(items) => items.iter().any(expr_contains_return),
        Expr::StructLiteral(_, fields) => fields
            .iter()
            .any(|(_, value)| value.as_ref().map(expr_contains_return).unwrap_or(false)),
        Expr::Closure(_, body) => expr_contains_return(body),
        Expr::If(_, then_block, else_block) => {
            block_contains_return(then_block)
                || else_block
                    .as_ref()
                    .map(|block| block_contains_return(block))
                    .unwrap_or(false)
        }
        Expr::While(_, body) | Expr::Loop(body) => block_contains_return(body),
        Expr::For(_, _, body) => block_contains_return(body),
        Expr::Block(block) => block_contains_return(block),
        Expr::Match(_, arms) => arms.iter().any(|arm| expr_contains_return(&arm.body)),
        Expr::Binary(left, _, right) => {
            expr_contains_return(left) || expr_contains_return(right)
        }
        Expr::Unary(_, inner) => expr_contains_return(inner),
        Expr::Call(callee, args) => {
            expr_contains_return(callee) || args.iter().any(expr_contains_return)
        }
        Expr::Assign(target, value) => {
            expr_contains_return(target) || expr_contains_return(value)
        }
        Expr::Field(object, _) => expr_contains_return(object),
        Expr::Index(array, index) => {
            expr_contains_return(array) || expr_contains_return(index)
        }
        Expr::Range(start, end) => {
            expr_contains_return(start) || expr_contains_return(end)
        }
        Expr::Cast(inner, _) => expr_contains_return(inner),
        _ => false,
    }
}

/// A trailing `loop` or `while true` diverges, so the body need not return.
fn ends_with_loop(block: &nova_common::Block) -> bool {
    use nova_common::{Expr, Literal, Statement};
    match block.statements.last() {
        Some(Statement::Expr(expr)) => match expr.unspanned() {
            Expr::Loop(_) => true,
            Expr::While(condition, _) => {
                matches!(condition.unspanned(), Expr::Literal(Literal::Bool(true)))
            }
            _ => false,
        },
        _ => false,
    }
}

/// The span of an expression when it has a real one (not the zero span).
fn span_of(expr: &nova_common::Expr) -> Option<Span> {
    let span = expr.span();
    if span == Span::zero() {
        None
    } else {
        Some(span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyze(source: &str) -> SemanticAnalyzer {
        let mut lexer = crate::Lexer::new(source);
        let (tokens, diagnostics) = lexer.tokenize();
        assert!(!diagnostics.has_errors(), "unexpected lexer errors");
        let program = crate::Parser::new(tokens)
            .parse()
            .expect("program should parse");
        let mut analyzer = SemanticAnalyzer::new();
        let _ = analyzer.analyze(&program);
        analyzer
    }

    fn messages(analyzer: &SemanticAnalyzer) -> Vec<String> {
        analyzer.diagnostics.iter().map(|d| d.message.clone()).collect()
    }

    #[test]
    fn test_basic_type_checking() {
        let mut analyzer = SemanticAnalyzer::new();
        assert!(!analyzer.diagnostics.has_errors());
    }

    #[test]
    fn matching_return_types_are_accepted() {
        let analyzer = analyze("fn one() -> int {\n    return 1;\n}\n");
        assert!(!analyzer.diagnostics.has_errors(), "{:?}", messages(&analyzer));
    }

    #[test]
    fn integer_returns_promote_to_float() {
        let analyzer = analyze("fn ratio() -> float {\n    return 1;\n}\n");
        assert!(!analyzer.diagnostics.has_errors(), "{:?}", messages(&analyzer));
    }

    #[test]
    fn mismatched_return_literal_is_reported() {
        let analyzer = analyze("fn flag() -> bool {\n    return 1;\n}\n");
        let msgs = messages(&analyzer);
        assert!(analyzer.diagnostics.has_errors(), "expected an error");
        assert!(
            msgs.iter()
                .any(|m| m.contains("returns `bool`") && m.contains("`int`")),
            "unexpected diagnostics: {msgs:?}"
        );
    }

    #[test]
    fn mismatched_return_from_call_is_reported() {
        let analyzer = analyze(
            "fn count() -> int {\n    return 1;\n}\n\nfn flag() -> bool {\n    return count();\n}\n",
        );
        let msgs = messages(&analyzer);
        assert!(
            msgs.iter()
                .any(|m| m.contains("function `flag` returns `bool`")),
            "unexpected diagnostics: {msgs:?}"
        );
    }

    #[test]
    fn value_returned_from_void_function_is_reported() {
        let analyzer = analyze("fn nothing() {\n    return 1;\n}\n");
        let msgs = messages(&analyzer);
        assert!(
            msgs.iter().any(|m| m.contains("has no return type")),
            "unexpected diagnostics: {msgs:?}"
        );
    }

    #[test]
    fn missing_return_value_is_reported() {
        let analyzer = analyze("fn needed() -> int {\n    return;\n}\n");
        let msgs = messages(&analyzer);
        assert!(
            msgs.iter().any(|m| m.contains("has no value")),
            "unexpected diagnostics: {msgs:?}"
        );
    }

    #[test]
    fn function_without_any_return_is_reported() {
        let analyzer = analyze("fn needed() -> int {\n    print(1);\n}\n");
        let msgs = messages(&analyzer);
        assert!(
            msgs.iter().any(|m| m.contains("no `return`")),
            "unexpected diagnostics: {msgs:?}"
        );
    }

    #[test]
    fn unknown_expression_types_are_not_guessed() {
        let analyzer = analyze("fn echo(x: int) -> int {\n    return x;\n}\n");
        assert!(
            !analyzer.diagnostics.has_errors(),
            "a known parameter type must satisfy the return type: {:?}",
            messages(&analyzer)
        );
    }

    #[test]
    fn correct_arguments_are_accepted() {
        let analyzer = analyze(
            "fn add(a: int, b: int) -> int {\n    return a + b;\n}\n\nfn main() {\n    print(add(1, 2));\n}\n",
        );
        assert!(!analyzer.diagnostics.has_errors(), "{:?}", messages(&analyzer));
    }

    #[test]
    fn wrong_arity_is_reported() {
        let analyzer = analyze(
            "fn add(a: int, b: int) -> int {\n    return a + b;\n}\n\nfn main() {\n    print(add(1));\n}\n",
        );
        let msgs = messages(&analyzer);
        assert!(
            msgs.iter()
                .any(|m| m.contains("takes 2 arguments, but 1 was given")),
            "unexpected diagnostics: {msgs:?}"
        );
    }

    #[test]
    fn wrong_argument_type_is_reported() {
        let analyzer = analyze(
            "fn takes_flag(flag: bool) {\n    print(flag);\n}\n\nfn main() {\n    takes_flag(1);\n}\n",
        );
        let msgs = messages(&analyzer);
        assert!(
            msgs.iter().any(|m| m.contains("argument 1 of `takes_flag` is `int`")),
            "unexpected diagnostics: {msgs:?}"
        );
    }

    #[test]
    fn integer_argument_promotes_to_float_parameter() {
        let analyzer = analyze(
            "fn scale(factor: float) -> float {\n    return factor;\n}\n\nfn main() {\n    print(scale(2));\n}\n",
        );
        assert!(!analyzer.diagnostics.has_errors(), "{:?}", messages(&analyzer));
    }

    #[test]
    fn return_type_error_points_at_the_return_value() {
        let analyzer = analyze("fn flag() -> bool {\n    return 1;\n}\n");
        let diagnostic = analyzer
            .diagnostics
            .iter()
            .find(|d| d.message.contains("return value is `int`"))
            .expect("expected a return type error");
        let span = diagnostic.span.expect("the diagnostic should carry a span");
        assert_eq!(
            (span.start.line, span.start.column),
            (2, 12),
            "the error should point at the returned expression, span was {span:?}"
        );
    }

    #[test]
    fn argument_type_error_points_at_the_argument() {
        let analyzer = analyze(
            "fn takes_flag(flag: bool) {\n    print(flag);\n}\n\nfn main() {\n    takes_flag(1);\n}\n",
        );
        let diagnostic = analyzer
            .diagnostics
            .iter()
            .find(|d| d.message.contains("argument 1 of `takes_flag`"))
            .expect("expected an argument type error");
        let span = diagnostic.span.expect("the diagnostic should carry a span");
        assert_eq!(
            (span.start.line, span.start.column),
            (6, 16),
            "the error should point at the argument, span was {span:?}"
        );
    }

    #[test]
    fn arity_error_points_at_the_call() {
        let analyzer = analyze(
            "fn add(a: int, b: int) -> int {\n    return a + b;\n}\n\nfn main() {\n    print(add(1));\n}\n",
        );
        let diagnostic = analyzer
            .diagnostics
            .iter()
            .find(|d| d.message.contains("takes 2 arguments"))
            .expect("expected an arity error");
        let span = diagnostic.span.expect("the diagnostic should carry a span");
        assert_eq!(
            (span.start.line, span.start.column),
            (6, 11),
            "the error should point at the call, span was {span:?}"
        );
    }

    #[test]
    fn struct_field_types_satisfy_return_types() {
        let analyzer = analyze(
            "struct Point {\n    x: int,\n    y: int,\n}\n\nfn get_x() -> int {\n    let p = Point { x: 1, y: 2 };\n    p.x\n}\n",
        );
        assert!(!analyzer.diagnostics.has_errors(), "{:?}", messages(&analyzer));
    }

    #[test]
    fn builtin_calls_are_declared_by_the_compiler() {
        let analyzer = analyze(
            "fn loud(text: String) {\n    __builtin_print(__builtin_strcat(text, \"!\"));\n}\n\nfn main() {\n    let n: int = __builtin_strlen(\"hi\");\n    print(n);\n}\n", 
        );
        assert!(
            !analyzer.diagnostics.has_errors(),
            "builtins must resolve without user declarations: {:?}",
            messages(&analyzer)
        );
    }

    #[test]
    fn generic_calls_are_accepted() {
        let analyzer = analyze(
            "fn id<T>(x: T) -> T {\n    x\n}\n\nfn main() {\n    print(id(1));\n    print(id(\"hi\"));\n}\n",
        );
        assert!(
            !analyzer.diagnostics.has_errors(),
            "generic calls must not be reported as type mismatches: {:?}",
            messages(&analyzer)
        );
    }

    #[test]
    fn unknown_argument_types_are_not_guessed() {
        let analyzer = analyze(
            "fn echo(value: bool) {\n    print(value);\n}\n\nfn main() {\n    echo(items[0]);\n}\n",
        );
        let msgs = messages(&analyzer);
        assert!(
            !msgs.iter().any(|m| m.contains("argument 1 of `echo`")),
            "an unknown argument type must not be reported as a mismatch: {msgs:?}"
        );
    }
}
