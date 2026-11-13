//! AST to SIR Lowering

use crate::sir::*;
use nova_common::{Program, Function, Statement, Expr, Literal, BinOp as AstBinOp, UnOp as AstUnOp, Pattern, MatchArm, Block};
use std::collections::{HashMap, HashSet};

pub struct Lowerer {
    pub functions: Vec<SirFunction>,
    pub globals: Vec<SirGlobal>,
    /// Name -> declared type, innermost scope last. Used to type expressions so
    /// codegen knows what it is printing.
    scopes: Vec<HashMap<String, String>>,
    /// Function name -> declared return type, collected before any body is
    /// lowered so a call may precede its callee's definition.
    signatures: HashMap<String, Option<String>>,
    /// Struct name -> field name -> declared type.
    struct_types: HashMap<String, HashMap<String, String>>,
    /// Struct name -> method name -> signature, collected from `impl` blocks.
    methods: HashMap<String, HashMap<String, MethodSig>>,
    /// Enum name -> variant names, in declaration order.
    enums: HashMap<String, Vec<String>>,
    /// `Enum_Variant` -> ordinal, so a `match` arm can be a C `case` constant.
    enum_ordinals: HashMap<String, i64>,
    /// Enum name -> variant name -> payload type, for variants that carry data.
    enum_payloads: HashMap<String, HashMap<String, String>>,
    /// Enum name -> its generic parameters, so payload types can be substituted.
    enum_generics: HashMap<String, Vec<String>>,
    /// Function name -> declared parameter types, so `&mut` arguments can be
    /// passed by address.
    param_types: HashMap<String, Vec<String>>,
    /// Struct name -> its generic parameters.
    struct_generics: HashMap<String, Vec<String>>,
    /// Concrete instantiations of generic structs, one per type-argument list.
    pending_structs: Vec<SirStruct>,
    /// The type a `let` or parameter expects, so a struct literal can pick its
    /// instantiation instead of the erased generic name.
    expected_type: Option<String>,
    /// Methods of generic structs that still need to be lowered per receiver type.
    pending_methods: Vec<PendingMethod>,
    /// Concrete method names already scheduled.
    instantiated_methods: HashSet<String>,
    /// Generic parameters of the `impl` block currently being collected.
    method_generics: HashMap<String, Vec<String>>,
    /// `Type_method` -> the method's AST, so a generic method can be lowered
    /// again for each receiver type.
    method_declarations: HashMap<String, Function>,
    /// True once `Some`/`None` are lowered, so the `Option` struct is emitted.
    uses_option: bool,
    /// Generic functions, keyed by name; lowered once per instantiation.
    generic_decls: HashMap<String, Function>,
    /// Instantiations that still need to be lowered.
    pending: Vec<PendingInstance>,
    /// Concrete names already scheduled, so each instantiation is emitted once.
    instantiated: HashSet<String>,
    /// Generic parameter -> concrete type while an instantiation is lowered.
    substitutions: Vec<HashMap<String, String>>,
    /// True while lowering a body that returns nothing, so a trailing `match`
    /// can become a `switch` instead of a value-producing ternary chain.
    discard_result: bool,
    /// Tuple layouts already emitted, keyed by generated struct name.
    emitted_tuples: HashSet<String>,
    /// Counter for temporaries introduced by tuple destructuring.
    temp_counter: usize,
    /// Functions lifted from closure expressions, emitted with the program.
    lifted_closures: Vec<SirFunction>,
    /// Counter for naming lifted closures and their environments.
    closure_counter: usize,
    /// Environments of the closures currently being lowered, innermost last.
    env_stack: Vec<EnvContext>,
}

/// One closure's captured variables and the struct that holds them.
struct EnvContext {
    struct_name: String,
    captures: HashMap<String, String>,
}

/// A generic function specialised for one set of concrete type arguments.
#[derive(Clone)]
struct PendingInstance {
    function: Function,
    concrete_name: String,
    substitution: HashMap<String, String>,
}

/// What the lowerer needs to know about one `impl` method.
#[derive(Clone)]
struct MethodSig {
    return_type: Option<String>,
}

/// One `impl Box<T> { … }` method specialised for a concrete receiver type.
#[derive(Clone)]
struct PendingMethod {
    method: Function,
    /// The impl's type name, e.g. `Box`.
    base: String,
    /// The C struct name, e.g. `Box_int`.
    concrete_type: String,
    /// The receiver type as written, e.g. `Box<int>`.
    self_type: String,
    concrete_name: String,
    substitution: HashMap<String, String>,
}

impl Lowerer {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            globals: Vec::new(),
            scopes: Vec::new(),
            signatures: HashMap::new(),
            struct_types: HashMap::new(),
            methods: HashMap::new(),
            enums: HashMap::new(),
            enum_ordinals: HashMap::new(),
            enum_payloads: HashMap::new(),
            enum_generics: HashMap::new(),
            param_types: HashMap::new(),
            struct_generics: HashMap::new(),
            pending_structs: Vec::new(),
            expected_type: None,
            pending_methods: Vec::new(),
            instantiated_methods: HashSet::new(),
            method_generics: HashMap::new(),
            method_declarations: HashMap::new(),
            uses_option: false,
            generic_decls: HashMap::new(),
            pending: Vec::new(),
            instantiated: HashSet::new(),
            substitutions: Vec::new(),
            discard_result: false,
            emitted_tuples: HashSet::new(),
            temp_counter: 0,
            lifted_closures: Vec::new(),
            closure_counter: 0,
            env_stack: Vec::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str, typ: Option<String>) {
        if let (Some(scope), Some(typ)) = (self.scopes.last_mut(), typ) {
            scope.insert(name.to_string(), typ);
        }
    }

    fn lookup(&self, name: &str) -> Option<String> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    /// The type of a literal, which is all the inference this compiler has.
    fn literal_type(expr: &Expr) -> Option<String> {
        match expr.unspanned() {
            Expr::Literal(Literal::Integer(_)) => Some("int".to_string()),
            Expr::Literal(Literal::Float(_)) => Some("float".to_string()),
            Expr::Literal(Literal::Bool(_)) => Some("bool".to_string()),
            Expr::Literal(Literal::String(_)) => Some("string".to_string()),
            Expr::Literal(Literal::Char(_)) => Some("char".to_string()),
            _ => None,
        }
    }

    /// The declared type of an expression, where the lowerer can know it.
    /// The type of an expression, with the active generic substitution applied.
    fn expr_type(&self, expr: &Expr) -> Option<String> {
        self.expr_type_raw(expr).map(|typ| self.substitute(&typ))
    }

    fn expr_type_raw(&self, expr: &Expr) -> Option<String> {
        match expr.unspanned() {
            Expr::Literal(_) => Self::literal_type(expr),
            Expr::Identifier(name) if name == "None" => Some("Option<int>".to_string()),
            // A captured value's type lives in its closure environment.
            Expr::Identifier(name) => self
                .lookup(name)
                .or_else(|| self.captured_type(name).map(|(_, typ)| typ)),
            Expr::StructLiteral(name, _) => Some(name.clone()),
            Expr::Tuple(items) => {
                let types: Vec<String> = items
                    .iter()
                    .map(|item| self.expr_type(item).unwrap_or_else(|| "int".to_string()))
                    .collect();
                Some(format!("({})", types.join(", ")))
            }
            Expr::Array(_) => Some("Array".to_string()),
            Expr::Index(_, _) => Some("int".to_string()),
            Expr::Field(object, field) => {
                let object_type = self.expr_type(object)?;
                let base = object_type.split('<').next().unwrap_or(&object_type).to_string();
                let declared = self.struct_types.get(&base)?.get(field)?.clone();
                Some(substitute_generic_payload(
                    &declared,
                    &base,
                    Some(&object_type),
                    self.struct_generics.get(&base),
                ))
            }
            Expr::Call(callee, args) => match callee.unspanned() {
                Expr::Identifier(name) if name == "Some" => {
                    let payload = args
                        .first()
                        .and_then(|arg| self.expr_type_raw(arg))
                        .unwrap_or_else(|| "int".to_string());
                    Some(format!("Option<{}>", payload))
                }
                Expr::Identifier(name) => {
                    if let Some((declaration, bindings)) = self.generic_bindings(name, args) {
                        declaration
                            .return_type
                            .as_ref()
                            .map(|typ| substitute_type_name(&typ.name, &bindings))
                    } else {
                        self.signatures.get(name).cloned().flatten()
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    pub fn lower_program(&mut self, program: &Program) -> SirProgram {
        let mut functions = Vec::new();
        let mut globals = Vec::new();
        let mut structs = Vec::new();

        // Signatures and struct layouts first: a body may call a function or
        // build a struct defined after it.
        for item in &program.items {
            match item {
                nova_common::Item::Function(func) => {
                    if !func.generics.is_empty() {
                        self.generic_decls.insert(func.name.clone(), func.clone());
                    }
                    self.signatures.insert(
                        func.name.clone(),
                        func.return_type.as_ref().map(|t| t.name.clone()),
                    );
                    self.param_types.insert(
                        func.name.clone(),
                        func.params
                            .iter()
                            .map(|param| type_display(&param.typ))
                            .collect(),
                    );
                }
                nova_common::Item::Enum(enum_def) => {
                    self.enums.insert(
                        enum_def.name.clone(),
                        enum_def
                            .variants
                            .iter()
                            .filter(|variant| variant.fields.is_none())
                            .map(|variant| variant.name.clone())
                            .collect(),
                    );
                    let mut payloads = HashMap::new();
                    for (index, variant) in enum_def.variants.iter().enumerate() {
                        self.enum_ordinals.insert(
                            format!("{}_{}", enum_def.name, variant.name),
                            index as i64,
                        );
                        if let Some(fields) = &variant.fields {
                            if let Some(first) = fields.first() {
                                payloads.insert(variant.name.clone(), first.name.clone());
                            }
                        }
                    }
                    if !payloads.is_empty() {
                        structs.push(SirStruct {
                            name: enum_def.name.clone(),
                            fields: payload_struct_fields(),
                        });
                    }
                    self.enum_payloads.insert(enum_def.name.clone(), payloads);
                    self.enum_generics
                        .insert(enum_def.name.clone(), enum_def.generics.clone());
                }
                nova_common::Item::Impl(impl_block) => {
                    if !impl_block.generics.is_empty() {
                        self.method_generics.insert(
                            impl_block.type_name.clone(),
                            impl_block.generics.clone(),
                        );
                    }
                    for method in &impl_block.methods {
                        self.methods
                            .entry(impl_block.type_name.clone())
                            .or_default()
                            .insert(
                                method.name.clone(),
                                MethodSig {
                                    return_type: method
                                        .return_type
                                        .as_ref()
                                        .map(|typ| typ.name.clone()),
                                },
                            );
                        self.method_declarations.insert(
                            format!("{}_{}", impl_block.type_name, method.name),
                            method.clone(),
                        );
                    }
                }
                nova_common::Item::Struct(struct_def) => {
                    self.struct_generics
                        .insert(struct_def.name.clone(), struct_def.generics.clone());
                    self.struct_types.insert(
                        struct_def.name.clone(),
                        struct_def
                            .fields
                            .iter()
                            .map(|field| (field.name.clone(), field.typ.name.clone()))
                            .collect(),
                    );
                    structs.push(SirStruct {
                        name: struct_def.name.clone(),
                        fields: struct_def
                            .fields
                            .iter()
                            .map(|field| SirStructField {
                                name: field.name.clone(),
                                typ: field.typ.name.clone(),
                            })
                            .collect(),
                    });
                }
                _ => {}
            }
        }

        for item in &program.items {
            match item {
                nova_common::Item::Function(func) => {
                    // A generic function is emitted once per instantiation, not
                    // as a template.
                    if func.generics.is_empty() {
                        functions.push(self.lower_function(func));
                    }
                }
                nova_common::Item::Impl(impl_block) => {
                    // Methods become ordinary C functions named `Type_method`.
                    for method in &impl_block.methods {
                        functions.push(self.lower_method(method, &impl_block.type_name));
                    }
                }
                nova_common::Item::Enum(enum_def) => {
                    // Payload-less variants become integer constants, so a match
                    // against `Color.Red` is an ordinary integer comparison.
                    for (index, variant) in enum_def.variants.iter().enumerate() {
                        self.enum_ordinals
                            .insert(format!("{}_{}", enum_def.name, variant.name), index as i64);
                        if variant.fields.is_some() {
                            continue;
                        }
                        globals.push(SirGlobal {
                            name: format!("{}_{}", enum_def.name, variant.name),
                            typ: "int".to_string(),
                            init_value: Some(SirExpr::Literal(SirLiteral::Int(index as i64))),
                        });
                    }
                }
                nova_common::Item::Const(const_def) => {
                    let init_value = self.lower_expr(&const_def.value);
                    globals.push(SirGlobal {
                        name: const_def.name.clone(),
                        typ: const_def.typ.as_ref().map(|t| t.name.clone()).unwrap_or_else(|| "int".to_string()),
                        init_value: Some(init_value),
                    });
                }
                _ => {}
            }
        }

        // Specialise every generic call the bodies above reached. Lowering an
        // instantiation may discover further ones, so drain the queue.
        let mut index = 0;
        while index < self.pending.len() {
            let instance = self.pending[index].clone();
            index += 1;
            functions.push(self.lower_instance(&instance));
        }

        let mut method_index = 0;
        while method_index < self.pending_methods.len() {
            let method = self.pending_methods[method_index].clone();
            method_index += 1;
            functions.push(self.lower_method_instance(&method));
        }

        functions.extend(self.lifted_closures.drain(..));
        structs.extend(self.pending_structs.drain(..));

        if self.uses_option && !structs.iter().any(|struct_def| struct_def.name == "Option") {
            structs.push(SirStruct {
                name: "Option".to_string(),
                fields: payload_struct_fields(),
            });
        }

        SirProgram {
            functions,
            globals,
            structs,
        }
    }

    /// Lower one instantiation of a generic function, with `T` replaced by the
    /// concrete type its call site determined.
    fn lower_instance(&mut self, instance: &PendingInstance) -> SirFunction {
        self.substitutions.push(instance.substitution.clone());
        self.push_scope();

        let mut params = Vec::new();
        for param in &instance.function.params {
            let typ = self.substitute(&type_display(&param.typ));
            self.declare(&param.name, Some(typ.clone()));
            params.push(SirParam {
                name: param.name.clone(),
                typ,
            });
        }

        let return_type = instance
            .function
            .return_type
            .as_ref()
            .map(|typ| self.substitute(&typ.name));
        let previous = self.discard_result;
        self.discard_result = !Self::returns_value(&return_type);
        let body = match &instance.function.body {
            Some(block) => self.lower_block(block),
            None => Vec::new(),
        };
        let body = Self::finish_body(body, &return_type);
        self.discard_result = previous;

        self.pop_scope();
        self.substitutions.pop();

        SirFunction {
            name: instance.concrete_name.clone(),
            params,
            return_type,
            body,
            span: instance.function.span.clone(),
            env_type: None,
        }
    }

    /// The active generic substitution applied to a type name.
    fn substitute(&self, typ: &str) -> String {
        let mut result = typ.to_string();
        for substitution in &self.substitutions {
            result = substitute_type_name(&result, substitution);
        }
        result
    }

    /// Bind a generic declaration's parameters to the types of a call's
    /// arguments. `None` when the callee is not generic.
    fn generic_bindings(
        &self,
        name: &str,
        args: &[Expr],
    ) -> Option<(Function, HashMap<String, String>)> {
        let declaration = self.generic_decls.get(name)?.clone();
        let mut bindings = HashMap::new();
        for (param, arg) in declaration.params.iter().zip(args.iter()) {
            if declaration
                .generics
                .iter()
                .any(|generic| generic == &param.typ.name)
            {
                if let Some(found) = self.expr_type(arg) {
                    bindings.entry(param.typ.name.clone()).or_insert(found);
                }
            }
        }
        Some((declaration, bindings))
    }

    /// Resolve a call to a generic function into a concrete function name and
    /// the instantiated return type, scheduling the instantiation if it is new.
    fn resolve_generic_call(
        &mut self,
        name: &str,
        args: &[Expr],
    ) -> Option<(String, Option<String>)> {
        let (declaration, bindings) = self.generic_bindings(name, args)?;

        let type_args: Vec<String> = declaration
            .generics
            .iter()
            .map(|generic| bindings.get(generic).cloned().unwrap_or_else(|| "int".to_string()))
            .collect();

        let mut substitution = HashMap::new();
        for (generic, concrete) in declaration.generics.iter().zip(type_args.iter()) {
            substitution.insert(generic.clone(), concrete.clone());
        }

        let concrete_name = format!(
            "{}_{}",
            name,
            type_args
                .iter()
                .map(|typ| mangle_type(typ))
                .collect::<Vec<_>>()
                .join("_")
        );

        if !self.instantiated.contains(&concrete_name) {
            self.instantiated.insert(concrete_name.clone());
            self.pending.push(PendingInstance {
                function: declaration.clone(),
                concrete_name: concrete_name.clone(),
                substitution: substitution.clone(),
            });
        }

        let return_type = declaration
            .return_type
            .as_ref()
            .map(|typ| substitute_type_name(&typ.name, &substitution));
        Some((concrete_name, return_type))
    }

    fn lower_function(&mut self, func: &Function) -> SirFunction {
        self.push_scope();
        for param in &func.params {
            self.declare(&param.name, Some(type_display(&param.typ)));
        }

        let return_type = func.return_type.as_ref().map(|typ| typ.name.clone());
        let previous = self.discard_result;
        self.discard_result = !Self::returns_value(&return_type);
        let body_stmts = match &func.body {
            Some(block) => self.lower_block(block),
            None => Vec::new(),
        };
        let body_stmts = Self::finish_body(body_stmts, &return_type);
        self.discard_result = previous;
        self.pop_scope();

        SirFunction {
            name: func.name.clone(),
            params: func.params.iter().map(|p| SirParam {
                name: p.name.clone(),
                typ: type_display(&p.typ),
            }).collect(),
            return_type: func.return_type.as_ref().map(|t| t.name.clone()),
            body: body_stmts,
            span: func.span.clone(),
            env_type: None,
        }
    }

    /// An `impl` method becomes `Type_method`, with `self` passed as a pointer.
    fn lower_method(&mut self, method: &Function, type_name: &str) -> SirFunction {
        self.push_scope();
        let mut params = Vec::new();
        for (index, param) in method.params.iter().enumerate() {
            let is_self = index == 0 && param.name == "self";
            if is_self {
                // Inside the body `self` keeps the struct's type so field types
                // resolve; in C it is a pointer to the receiver.
                self.declare(&param.name, Some(type_name.to_string()));
                let by_reference = param.typ.name.trim_start().starts_with('&');
                params.push(SirParam {
                    name: param.name.clone(),
                    typ: if by_reference {
                        format!("{}*", type_name)
                    } else {
                        type_name.to_string()
                    },
                });
            } else {
                self.declare(&param.name, Some(type_display(&param.typ)));
                params.push(SirParam {
                    name: param.name.clone(),
                    typ: type_display(&param.typ),
                });
            }
        }

        let return_type = method.return_type.as_ref().map(|typ| typ.name.clone());
        let previous = self.discard_result;
        self.discard_result = !Self::returns_value(&return_type);
        let body = match &method.body {
            Some(block) => self.lower_block(block),
            None => Vec::new(),
        };
        let body = Self::finish_body(body, &return_type);
        self.discard_result = previous;
        self.pop_scope();

        SirFunction {
            name: format!("{}_{}", type_name, method.name),
            params,
            return_type: method.return_type.as_ref().map(|typ| typ.name.clone()),
            body,
            span: method.span.clone(),
            env_type: None,
        }
    }

    /// Whether a declared return type produces a value.
    fn returns_value(return_type: &Option<String>) -> bool {
        return_type
            .as_deref()
            // `!` never returns a value, so a diverging body is emitted as
            // statements (the call itself traps) rather than `return expr`.
            .map(|typ| !matches!(typ.to_lowercase().as_str(), "void" | "unit" | "!"))
            .unwrap_or(false)
    }

    /// Lower one `impl Box<T>` method for a concrete receiver type.
    fn lower_method_instance(&mut self, instance: &PendingMethod) -> SirFunction {
        self.substitutions.push(instance.substitution.clone());
        self.push_scope();

        let mut params = Vec::new();
        for (index, param) in instance.method.params.iter().enumerate() {
            let is_self = index == 0 && param.name == "self";
            if is_self {
                // Typing uses `Box<int>`; C uses the monomorphised `Box_int`.
                self.declare(&param.name, Some(instance.self_type.clone()));
                let by_reference = param.typ.name.trim_start().starts_with('&');
                params.push(SirParam {
                    name: param.name.clone(),
                    typ: if by_reference {
                        format!("{}*", instance.concrete_type)
                    } else {
                        instance.concrete_type.clone()
                    },
                });
            } else {
                let typ = self.substitute(&type_display(&param.typ));
                self.declare(&param.name, Some(typ.clone()));
                params.push(SirParam {
                    name: param.name.clone(),
                    typ,
                });
            }
        }

        let return_type = instance
            .method
            .return_type
            .as_ref()
            .map(|typ| self.substitute(&type_display(typ)));
        let previous = self.discard_result;
        self.discard_result = !Self::returns_value(&return_type);
        let body = match &instance.method.body {
            Some(block) => self.lower_block(block),
            None => Vec::new(),
        };
        let body = Self::finish_body(body, &return_type);
        self.discard_result = previous;

        self.pop_scope();
        self.substitutions.pop();

        SirFunction {
            name: instance.concrete_name.clone(),
            params,
            return_type,
            body,
            span: instance.method.span.clone(),
            env_type: None,
        }
    }

    /// A function that declares a value and ends in an expression returns that
    /// expression, rather than leaving it as a discarded statement.
    fn finish_body(mut body: Vec<SirStatement>, return_type: &Option<String>) -> Vec<SirStatement> {
        let returns_value = Self::returns_value(return_type);
        // Check before popping: a trailing `return` must not be dropped.
        if returns_value && matches!(body.last(), Some(SirStatement::Expr(_))) {
            if let Some(SirStatement::Expr(expr)) = body.pop() {
                body.push(SirStatement::Return(Some(expr)));
            }
        }
        body
    }

    /// Emit one concrete C struct for `Box<int>`, substituting the declaration's
    /// generic parameters into its field types. Returns the C type name.
    fn instantiate_struct(&mut self, base: &str, object_type: &str) -> Option<String> {
        let generics = self.struct_generics.get(base)?.clone();
        let arguments = generic_arguments(object_type)?;
        let name = format!(
            "{}_{}",
            base,
            arguments
                .iter()
                .map(|argument| mangle_type(argument))
                .collect::<Vec<_>>()
                .join("_")
        );
        if self.pending_structs.iter().any(|struct_def| struct_def.name == name) {
            return Some(name);
        }
        let fields = self
            .struct_types
            .get(base)?
            .iter()
            .map(|(field, typ)| {
                let substituted = generics
                    .iter()
                    .position(|generic| generic == typ)
                    .and_then(|index| arguments.get(index).cloned())
                    .unwrap_or_else(|| typ.clone());
                SirStructField {
                    name: field.clone(),
                    typ: substituted,
                }
            })
            .collect();
        self.pending_structs.push(SirStruct {
            name: name.clone(),
            fields,
        });
        Some(name)
    }

    /// Lower call arguments, taking a `&`/`&mut` parameter by address unless the
    /// argument is already a reference.
    fn lower_call_args(&mut self, name: &str, args: &[Expr]) -> Vec<SirExpr> {
        let params = self.param_types.get(name).cloned().unwrap_or_default();
        args.iter()
            .enumerate()
            .map(|(index, arg)| {
                let lowered = self.lower_expr(arg);
                let wants_reference = params
                    .get(index)
                    .is_some_and(|typ| typ.trim_start().starts_with('&'));
                let is_reference = self
                    .expr_type(arg)
                    .is_some_and(|typ| typ.trim_start().starts_with('&'));
                if wants_reference && !is_reference {
                    SirExpr::AddressOf(Box::new(lowered))
                } else if !wants_reference && is_reference {
                    // A `&mut [T]` value passed to a by-value parameter is
                    // dereferenced, so `array_len(&mut arr)` sees the header.
                    SirExpr::Deref(Box::new(lowered))
                } else {
                    lowered
                }
            })
            .collect()
    }

    /// An array value for a runtime call: a `&mut` parameter is dereferenced.
    fn lower_array_value(&mut self, expr: &Expr) -> SirExpr {
        let lowered = self.lower_expr(expr);
        if self
            .expr_type(expr)
            .is_some_and(|typ| typ.trim_start().starts_with('&'))
        {
            SirExpr::Deref(Box::new(lowered))
        } else {
            lowered
        }
    }

    /// An array header for a mutating call, and whether it is already a pointer.
    fn lower_array_handle(&mut self, expr: &Expr) -> (SirExpr, bool) {
        let is_reference = self
            .expr_type(expr)
            .is_some_and(|typ| typ.trim_start().starts_with('&'));
        (self.lower_expr(expr), is_reference)
    }

    /// Map a `__builtin_*` call onto the runtime shim. Builtins the shim does not
    /// implement yet become `nova_unsupported`, which reports the name and exits
    /// rather than silently doing nothing.
    fn lower_builtin_call(&mut self, name: &str, args: &[Expr]) -> Option<SirExpr> {
        let lowered = |lowerer: &mut Self| -> Vec<SirExpr> {
            args.iter().map(|arg| lowerer.lower_expr(arg)).collect()
        };
        let typed = |callee: &str, args: Vec<SirExpr>, typ: &str| SirExpr::Typed {
            expr: Box::new(SirExpr::Call {
                callee: callee.to_string(),
                args,
            }),
            typ: typ.to_string(),
        };

        match name {
            "__builtin_print" => Some(SirExpr::Call {
                callee: "print".to_string(),
                args: lowered(self),
            }),
            "__builtin_println" => Some(SirExpr::Call {
                callee: "println".to_string(),
                args: lowered(self),
            }),
            // `typeof` is static: the answer is the argument's type name.
            "__builtin_typeof" => {
                let typ = args
                    .first()
                    .and_then(|arg| self.expr_type(arg))
                    .unwrap_or_else(|| "unknown".to_string());
                Some(SirExpr::Literal(SirLiteral::String(typ)))
            }
            "__builtin_tostring" => {
                let typ = args
                    .first()
                    .and_then(|arg| self.expr_type(arg))
                    .unwrap_or_else(|| "int".to_string());
                let callee = match typ.as_str() {
                    "string" | "str" => "nova_strdup",
                    "bool" => "nova_tostring_bool",
                    _ => "nova_tostring_int",
                };
                Some(typed(callee, lowered(self), "string"))
            }
            "__builtin_strlen" => Some(typed("nova_strlen", lowered(self), "int")),
            "__builtin_strcmp" => Some(typed("nova_strcmp", lowered(self), "int")),
            "__builtin_strcat" => Some(typed("nova_strcat", lowered(self), "string")),
            "__builtin_substr" => Some(typed("nova_substr", lowered(self), "string")),
            "__builtin_array_len" => Some(typed(
                "nova_array_len",
                vec![self.lower_array_value(args.first()?)],
                "int",
            )),
            // Push and pop mutate the header (length, capacity), so they take
            // the array by address unless it is already a `&mut` reference.
            "__builtin_array_push" => {
                let (array, is_reference) = self.lower_array_handle(args.first()?);
                let array = if is_reference {
                    array
                } else {
                    SirExpr::AddressOf(Box::new(array))
                };
                let value = self.lower_expr(args.get(1)?);
                Some(typed("nova_array_push", vec![array, value], "void"))
            }
            "__builtin_array_pop" | "__builtin_array_pop_ref" => {
                let (array, is_reference) = self.lower_array_handle(args.first()?);
                let array = if is_reference {
                    array
                } else {
                    SirExpr::AddressOf(Box::new(array))
                };
                Some(typed("nova_array_pop", vec![array], "int"))
            }
            "__builtin_panic" => Some(typed("nova_panic", lowered(self), "void")),
            "__builtin_exit" => Some(typed("nova_exit", lowered(self), "void")),
            _ => Some(typed(
                "nova_unsupported",
                vec![SirExpr::Literal(SirLiteral::String(name.to_string()))],
                "int",
            )),
        }
    }

    /// `value.method(args)` becomes `Type_method(&value, args)` when the receiver
    /// has a known struct type and the method is declared in an `impl` block.
    fn lower_method_call(&mut self, callee: &Expr, args: &[Expr]) -> Option<SirExpr> {
        let Expr::Field(receiver, method) = callee.unspanned() else {
            return None;
        };
        let type_name = self.expr_type(receiver)?;

        // A method on `Box<int>` is emitted as `Box_int_method`.
        if let Some((base, argument_list)) = type_name.split_once('<') {
            if let Some(signature) = self.methods.get(base).and_then(|m| m.get(method)).cloned()
            {
                let arguments: Vec<String> = argument_list
                    .trim_end_matches('>')
                    .split(',')
                    .map(|argument| argument.trim().to_string())
                    .collect();
                let generics = self
                    .method_generics
                    .get(base)
                    .cloned()
                    .or_else(|| self.struct_generics.get(base).cloned())
                    .unwrap_or_default();
                let substitution: HashMap<String, String> = generics
                    .iter()
                    .zip(arguments.iter())
                    .map(|(generic, argument)| (generic.clone(), argument.clone()))
                    .collect();
                let concrete_type = format!(
                    "{}_{}",
                    base,
                    arguments
                        .iter()
                        .map(|argument| mangle_type(argument))
                        .collect::<Vec<_>>()
                        .join("_")
                );
                let concrete_name = format!("{}_{}", concrete_type, method);

                if self.instantiated_methods.insert(concrete_name.clone()) {
                    if let Some(declaration) = self.methods
                        .get(base)
                        .and_then(|_| self.method_declarations.get(&format!("{}_{}", base, method)))
                        .cloned()
                    {
                        self.pending_methods.push(PendingMethod {
                            method: declaration,
                            base: base.to_string(),
                            concrete_type,
                            self_type: type_name.clone(),
                            concrete_name: concrete_name.clone(),
                            substitution: substitution.clone(),
                        });
                    }
                }

                self.instantiate_struct(base, &type_name);

                let mut lowered_args = vec![SirExpr::AddressOf(Box::new(self.lower_expr(receiver)))];
                lowered_args.extend(args.iter().map(|arg| self.lower_expr(arg)));

                let call = SirExpr::Call {
                    callee: concrete_name,
                    args: lowered_args,
                };
                return Some(match signature.return_type {
                    Some(typ) => SirExpr::Typed {
                        expr: Box::new(call),
                        typ: substitute_type_name(&typ, &substitution),
                    },
                    None => call,
                });
            }
        }

        let signature = self.methods.get(&type_name)?.get(method)?.clone();

        let mut lowered_args = vec![SirExpr::AddressOf(Box::new(self.lower_expr(receiver)))];
        lowered_args.extend(args.iter().map(|arg| self.lower_expr(arg)));

        let call = SirExpr::Call {
            callee: format!("{}_{}", type_name, method),
            args: lowered_args,
        };
        Some(match signature.return_type {
            Some(typ) => SirExpr::Typed {
                expr: Box::new(call),
                typ,
            },
            None => call,
        })
    }

    fn lower_statement(&mut self, stmt: &Statement) -> Vec<SirStatement> {
        match stmt {
            // A local `const` is a read-only binding with an initialiser.
            Statement::Item(nova_common::Item::Const(const_def)) => {
                let typ = const_def
                    .typ
                    .as_ref()
                    .map(|typ| self.substitute(&typ.name))
                    .or_else(|| self.expr_type(&const_def.value));
                self.declare(&const_def.name, typ.clone());
                vec![SirStatement::Let {
                    name: const_def.name.clone(),
                    typ,
                    value: self.lower_expr(&const_def.value),
                }]
            }
            // `let (a, b) = pair;` binds each element of a tuple temporary.
            Statement::Let(let_stmt) if matches!(let_stmt.pattern, Pattern::Tuple(_)) => {
                return self.lower_tuple_let(let_stmt);
            }
            Statement::Let(let_stmt) => {
                let name = match &let_stmt.pattern {
                    Pattern::Identifier(n) => n.clone(),
                    _ => "_".to_string(),
                };
                let declared = let_stmt
                    .typ
                    .as_ref()
                    .map(|typ| self.substitute(&type_display(typ)));
                // Tell a struct literal which instantiation the binding expects,
                // and a closure what parameter types its binding promises.
                let previous = self.expected_type.take();
                if let Some(typ) = &declared {
                    if let Some((base, _)) = typ.split_once('<') {
                        if self.struct_generics.contains_key(base) {
                            self.expected_type = Some(typ.clone());
                        }
                    }
                    if typ.starts_with("fn(") {
                        self.expected_type = Some(typ.clone());
                    }
                }
                let value = let_stmt.init.as_ref()
                    .map(|e| self.lower_expr(e))
                    .unwrap_or_else(|| SirExpr::Literal(SirLiteral::Int(0)));
                self.expected_type = previous;
                // An annotation wins; otherwise the initialiser's type tells.
                // A closure reports its signature only once lowered.
                let typ = declared
                    .or_else(|| let_stmt.init.as_ref().and_then(|init| self.expr_type(init)))
                    .or_else(|| match &value {
                        SirExpr::Typed { typ, .. } => Some(typ.clone()),
                        _ => None,
                    });
                if name != "_" {
                    self.declare(&name, typ.clone());
                }
                vec![SirStatement::Let { name, typ, value }]
            }
            Statement::Expr(expr) => {
                // Control-flow detection looks through the span wrapper.
                let expr = expr.unspanned();
                // Check if the expression is a control flow statement that should be lowered as a statement
                match expr {
                    Expr::If(cond, then_block, else_block) => {
                        return vec![self.lower_if_statement(cond, then_block, else_block)];
                    }
                    Expr::While(cond, body) => {
                        return vec![self.lower_while_statement(cond, body)];
                    }
                    Expr::For(pattern, iterable, body) => {
                        return vec![self.lower_for_statement(pattern, iterable, body)];
                    }
                    Expr::Match(scrutinee, arms) => {
                        // A bare `match` statement compiles to a C `switch`;
                        // patterns with no constant test fall through to the
                        // ternary chain below.
                        if let Some(statements) = self.lower_match_statement(scrutinee, arms) {
                            return statements;
                        }
                        // String patterns cannot be `switch` labels, so the same
                        // arms become an if/else statement chain.
                        if let Some(statements) = self.lower_match_to_if(scrutinee, arms) {
                            return statements;
                        }
                    }
                    Expr::Return(value) => {
                        return vec![SirStatement::Return(value.as_ref().map(|v| self.lower_expr(v)))];
                    }
                    _ => {}
                }
                // Check if the expression is a control flow construct that should be a statement
                match expr {
                    Expr::If(cond, then_block, else_block) => {
                        let then_stmts = self.lower_block(then_block);
                        let else_stmts = else_block.as_ref().map(|b| self.lower_block(b));
                        return vec![SirStatement::If {
                            condition: self.lower_expr(cond),
                            then_block: then_stmts,
                            else_block: else_stmts,
                        }];
                    }
                    Expr::While(cond, body) => {
                        vec![SirStatement::While {
                            condition: self.lower_expr(cond),
                            body: self.lower_block(body),
                        }]
                    }
                    Expr::Loop(body) => {
                        // Lower loop as while(true)
                        vec![SirStatement::While {
                            condition: SirExpr::Literal(SirLiteral::Bool(true)),
                            body: self.lower_block(body),
                        }]
                    }
                    Expr::For(pattern, iterable, body) => {
                        let pattern_str = match pattern {
                            Pattern::Identifier(n) => n.clone(),
                            _ => "_".to_string(),
                        };
                        vec![SirStatement::For {
                            pattern: pattern_str,
                            iterable: self.lower_expr(iterable),
                            body: self.lower_block(body),
                        }]
                    }
                    Expr::Block(block) => {
                        vec![SirStatement::Block(self.lower_block(block))]
                    }
                    Expr::Return(value) => {
                        vec![SirStatement::Return(value.as_ref().map(|v| self.lower_expr(v)))]
                    }
                    _ => {
                        vec![SirStatement::Expr(self.lower_expr(expr))]
                    }
                }
            }
            Statement::Item(_item) => {
                // Skip item statements for now (e.g., nested function definitions)
                Vec::new()
            }
        }
    }

    fn lower_if_statement(&mut self, cond: &Expr, then_block: &nova_common::Block, else_block: &Option<nova_common::Block>) -> SirStatement {
        let condition = self.lower_expr(cond);
        
        let then_stmts: Vec<SirStatement> = then_block.statements.iter()
            .flat_map(|s| self.lower_statement(s))
            .collect();
        let then_final = then_block.final_expr.as_ref()
            .map(|e| self.lower_expr(e));
        
        // Add final expression as a statement if present
        let mut all_then_stmts = then_stmts;
        if let Some(final_expr) = then_final {
            all_then_stmts.push(SirStatement::Expr(final_expr));
        }
        
        let else_stmts = else_block.as_ref().map(|else_blk| {
            let mut stmts: Vec<SirStatement> = else_blk.statements.iter()
                .flat_map(|s| self.lower_statement(s))
                .collect();
            if let Some(final_expr) = &else_blk.final_expr {
                stmts.push(SirStatement::Expr(self.lower_expr(final_expr)));
            }
            stmts
        });
        
        SirStatement::If {
            condition,
            then_block: all_then_stmts,
            else_block: else_stmts,
        }
    }

    fn lower_while_statement(&mut self, cond: &Expr, body: &nova_common::Block) -> SirStatement {
        let condition = self.lower_expr(cond);
        let body_stmts: Vec<SirStatement> = body.statements.iter()
            .flat_map(|s| self.lower_statement(s))
            .collect();
        
        SirStatement::While {
            condition,
            body: body_stmts,
        }
    }

    fn lower_for_statement(&mut self, pattern: &Pattern, iterable: &Expr, body: &nova_common::Block) -> SirStatement {
        let pattern_name = match pattern {
            Pattern::Identifier(n) => n.clone(),
            _ => "_".to_string(),
        };
        let iterable_expr = self.lower_expr(iterable);
        let body_stmts: Vec<SirStatement> = body.statements.iter()
            .flat_map(|s| self.lower_statement(s))
            .collect();
        
        SirStatement::For {
            pattern: pattern_name,
            iterable: iterable_expr,
            body: body_stmts,
        }
    }


    /// Emit (once) and return the struct name for a tuple of `types`.
    fn tuple_struct(&mut self, types: &[String]) -> String {
        let name = tuple_struct_name(types);
        if self.emitted_tuples.insert(name.clone()) {
            let fields = types
                .iter()
                .enumerate()
                .map(|(index, typ)| SirStructField {
                    name: format!("_{index}"),
                    typ: typ.clone(),
                })
                .collect();
            self.pending_structs.push(SirStruct {
                name: name.clone(),
                fields,
            });
        }
        name
    }

    /// A variable reference: a local, a captured value, or a plain name.
    fn lower_variable(&mut self, name: &str) -> SirExpr {
        match self.lookup(name) {
            // A known type rides along so codegen can print the value correctly.
            Some(typ) => SirExpr::Typed {
                expr: Box::new(SirExpr::Var(name.to_string())),
                typ: self.substitute(&typ),
            },
            None => match self.captured_type(name) {
                Some((env_type, typ)) => SirExpr::Typed {
                    expr: Box::new(SirExpr::EnvAccess {
                        env_type,
                        field: name.to_string(),
                    }),
                    typ,
                },
                None => SirExpr::Var(name.to_string()),
            },
        }
    }

    /// A captured variable's environment struct and type, searching outwards.
    fn captured_type(&self, name: &str) -> Option<(String, String)> {
        self.env_stack.iter().rev().find_map(|env| {
            env.captures
                .get(name)
                .map(|typ| (env.struct_name.clone(), typ.clone()))
        })
    }

    /// Lift a closure to a top-level function plus an environment.
    fn lower_closure(
        &mut self,
        params: &[(String, Option<nova_common::TypeExpr>)],
        body: &Expr,
    ) -> SirExpr {
        // An annotation wins; otherwise a `let f: fn(int) -> int = |x| …`
        // annotation supplies the parameter types; the default is `int`.
        let contextual: Vec<String> = self
            .expected_type
            .as_deref()
            .and_then(parse_function_type)
            .map(|(types, _)| types)
            .unwrap_or_default();
        let param_types: Vec<String> = params
            .iter()
            .enumerate()
            .map(|(index, (_, typ))| match typ {
                Some(typ) => self.substitute(&type_display(typ)),
                None => contextual
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| "int".to_string()),
            })
            .collect();
        let params: Vec<String> = params.iter().map(|(name, _)| name.clone()).collect();
        let params = &params[..];
        self.closure_counter += 1;
        let id = self.closure_counter;
        let env_name = format!("__Env_{id}");
        let function_name = format!("__closure_{id}");

        // Capture the free variables that resolve to locals or outer captures.
        let mut bound: HashSet<String> = params.iter().cloned().collect();
        let mut referenced = Vec::new();
        collect_names_expr(body, &mut bound, &mut referenced);
        let mut seen = HashSet::new();
        let mut captures: Vec<(String, String)> = Vec::new();
        for name in referenced {
            if !seen.insert(name.clone()) {
                continue;
            }
            if let Some(typ) = self.lookup(&name) {
                captures.push((name, typ));
            } else if let Some((_, typ)) = self.captured_type(&name) {
                captures.push((name, typ));
            }
        }

        // Lower the capture expressions in the enclosing environment first.
        let capture_args: Vec<SirExpr> = captures
            .iter()
            .map(|(name, _)| self.lower_variable(name))
            .collect();

        // Sort out the body's type with the parameters and captures in scope.
        self.push_scope();
        for (param, typ) in params.iter().zip(param_types.iter()) {
            self.declare(param, Some(typ.clone()));
        }
        for (name, typ) in &captures {
            self.declare(name, Some(typ.clone()));
        }
        let return_type = self
            .expr_type(body)
            .filter(|typ| !matches!(typ.as_str(), "()" | "void"))
            .unwrap_or_else(|| "int".to_string());
        self.pop_scope();

        // The environment struct and its constructor.
        let env_expr = if captures.is_empty() {
            None
        } else {
            let fields = captures
                .iter()
                .map(|(name, typ)| SirStructField {
                    name: name.clone(),
                    typ: typ.clone(),
                })
                .collect();
            self.pending_structs.push(SirStruct {
                name: env_name.clone(),
                fields,
            });

            let constructor = SirFunction {
                name: format!("__env_{id}"),
                params: captures
                    .iter()
                    .map(|(name, typ)| SirParam {
                        name: name.clone(),
                        typ: typ.clone(),
                    })
                    .collect(),
                return_type: Some("void*".to_string()),
                body: vec![
                    SirStatement::Let {
                        name: "__env_value".to_string(),
                        typ: Some(env_name.clone()),
                        value: SirExpr::StructLiteral {
                            name: env_name.clone(),
                            fields: captures
                                .iter()
                                .map(|(name, _)| (name.clone(), SirExpr::Var(name.clone())))
                                .collect(),
                        },
                    },
                    SirStatement::Return(Some(SirExpr::Typed {
                        expr: Box::new(SirExpr::Cast {
                            value: Box::new(SirExpr::Call {
                                callee: "nova_env_new".to_string(),
                                args: vec![
                                    SirExpr::AddressOf(Box::new(SirExpr::Var(
                                        "__env_value".to_string(),
                                    ))),
                                    SirExpr::SizeOf(env_name.clone()),
                                ],
                            }),
                            typ: "void*".to_string(),
                        }),
                        typ: "void*".to_string(),
                    })),
                ],
                span: nova_common::Span::default(),
                env_type: None,
            };
            self.lifted_closures.push(constructor);

            Some(Box::new(SirExpr::Typed {
                expr: Box::new(SirExpr::Call {
                    callee: format!("__env_{id}"),
                    args: capture_args,
                }),
                typ: "void*".to_string(),
            }))
        };

        // Lower the body in isolation from the enclosing function's locals: a
        // capture must become an environment read, not the outer C variable.
        let outer_scopes = std::mem::take(&mut self.scopes);
        self.env_stack.push(EnvContext {
            struct_name: env_name.clone(),
            captures: captures.iter().cloned().collect(),
        });
        self.push_scope();
        for (param, typ) in params.iter().zip(param_types.iter()) {
            self.declare(param, Some(typ.clone()));
        }
        let body_expr = match body.unspanned() {
            Expr::Block(block) => self.lower_block_as_expr(block),
            _ => self.lower_expr(body),
        };
        self.pop_scope();
        self.env_stack.pop();
        self.scopes = outer_scopes;

        let signature = format!(
            "fn({}) -> {}",
            param_types.join(", "),
            return_type
        );
        self.lifted_closures.push(SirFunction {
            name: function_name.clone(),
            params: params
                .iter()
                .zip(param_types.iter())
                .map(|(param, typ)| SirParam {
                    name: param.clone(),
                    typ: typ.clone(),
                })
                .collect(),
            return_type: Some(return_type),
            body: vec![SirStatement::Return(Some(body_expr))],
            span: nova_common::Span::default(),
            // Every lifted closure takes the environment parameter, even when it
            // captures nothing, so call sites need no special case.
            env_type: Some(env_name),
        });

        SirExpr::Typed {
            expr: Box::new(SirExpr::Closure {
                name: function_name,
                env: env_expr,
            }),
            typ: signature,
        }
    }

    /// `let (a, b) = pair;` becomes a tuple temporary plus one `let` per field.
    fn lower_tuple_let(&mut self, let_stmt: &nova_common::LetStmt) -> Vec<SirStatement> {
        let Pattern::Tuple(patterns) = &let_stmt.pattern else {
            return Vec::new();
        };
        let Some(init) = &let_stmt.init else {
            return Vec::new();
        };
        let element_types = self
            .expr_type(init)
            .as_deref()
            .and_then(tuple_element_types)
            .unwrap_or_default();
        let name = self.tuple_struct(&element_types);

        self.temp_counter += 1;
        let temporary = format!("__tuple{}", self.temp_counter);
        let mut statements = vec![SirStatement::Let {
            name: temporary.clone(),
            typ: Some(name.clone()),
            value: self.lower_expr(init),
        }];

        for (index, pattern) in patterns.iter().enumerate() {
            let Pattern::Identifier(binding) = pattern else {
                // Wildcards and nested patterns keep their value unbound.
                continue;
            };
            if binding == "_" {
                continue;
            }
            let element_type = element_types
                .get(index)
                .cloned()
                .unwrap_or_else(|| "int".to_string());
            let access = SirExpr::Typed {
                expr: Box::new(SirExpr::FieldAccess {
                    object: Box::new(SirExpr::Var(temporary.clone())),
                    field: format!("_{index}"),
                }),
                typ: element_type.clone(),
            };
            self.declare(binding, Some(element_type.clone()));
            statements.push(SirStatement::Let {
                name: binding.clone(),
                typ: Some(element_type),
                value: access,
            });
        }

        statements
    }

    fn lower_block(&mut self, block: &nova_common::Block) -> Vec<SirStatement> {
        self.push_scope();
        let mut stmts = Vec::new();
        for stmt in &block.statements {
            stmts.extend(self.lower_statement(stmt));
        }
        if let Some(final_expr) = &block.final_expr {
            // Handle control flow in final expression
            match final_expr.unspanned() {
                Expr::Return(value) => {
                    stmts.push(SirStatement::Return(value.as_ref().map(|v| self.lower_expr(v))));
                }
                // A block's value may be a control-flow construct; lowering it as
                // an expression would lose its loop/if statements, so reuse the
                // statement lowering that knows about them.
                // In a function that returns a value, a trailing `if` is that
                // function's result: `fn sign(x) -> int { if x < 0 { -1 } else { 1 } }`.
                // Lowering it as a statement would drop the value on the floor.
                Expr::If(condition, then_block, else_block) if !self.discard_result => {
                    stmts.push(self.lower_if_as_return(condition, then_block, else_block));
                }
                Expr::If(..) | Expr::While(..) | Expr::For(..) | Expr::Loop(..) | Expr::Block(..) => {
                    stmts.extend(self.lower_statement(&Statement::Expr((**final_expr).clone())));
                }
                // In a function that returns nothing, a trailing `match` is a
                // statement: keep it a `switch` instead of a discarded ternary.
                Expr::Match(..) if self.discard_result => {
                    stmts.extend(self.lower_statement(&Statement::Expr((**final_expr).clone())));
                }
                _ => {
                    stmts.push(SirStatement::Expr(self.lower_expr(final_expr)));
                }
            }
        }
        self.pop_scope();
        stmts
    }

    fn lower_expr(&mut self, expr: &Expr) -> SirExpr {
        match expr {
            Expr::Spanned(inner, _) => self.lower_expr(inner),
            Expr::StructLiteral(name, fields) => {
                // `Box<int> { … }` is emitted as `Box_int`, so the C struct's
                // field types are concrete.
                let expected = self.expected_type.clone();
                let concrete = match expected.as_deref() {
                    Some(expected)
                        if expected
                            .split_once('<')
                            .is_some_and(|(base, _)| base == name) =>
                    {
                        self.instantiate_struct(name, expected)
                            .unwrap_or_else(|| name.clone())
                    }
                    _ => name.clone(),
                };
                let fields = fields
                    .iter()
                    .map(|(field, value)| {
                        // `Point { x }` means `Point { x: x }`.
                        let value = match value {
                            Some(value) => self.lower_expr(value),
                            None => self.lower_expr(&Expr::Identifier(field.clone())),
                        };
                        (field.clone(), value)
                    })
                    .collect();
                SirExpr::Typed {
                    expr: Box::new(SirExpr::StructLiteral {
                        name: concrete.clone(),
                        fields,
                    }),
                    typ: concrete,
                }
            }
            Expr::Tuple(items) => {
                let types: Vec<String> = items
                    .iter()
                    .map(|item| self.expr_type(item).unwrap_or_else(|| "int".to_string()))
                    .collect();
                let name = self.tuple_struct(&types);
                SirExpr::Typed {
                    expr: Box::new(SirExpr::Tuple {
                        name: name.clone(),
                        items: items.iter().map(|item| self.lower_expr(item)).collect(),
                    }),
                    typ: name,
                }
            }
            Expr::Closure(params, body) => self.lower_closure(params, body),
            Expr::Array(items) => SirExpr::Typed {
                expr: Box::new(SirExpr::Array(
                    items.iter().map(|item| self.lower_expr(item)).collect(),
                )),
                typ: "Array".to_string(),
            },
            Expr::Literal(lit) => {
                SirExpr::Literal(match lit {
                    Literal::Integer(i) => SirLiteral::Int(*i),
                    Literal::Float(f) => SirLiteral::Float(*f),
                    Literal::String(s) => SirLiteral::String(s.clone()),
                    Literal::Char(c) => SirLiteral::Char(*c),
                    Literal::Bool(b) => SirLiteral::Bool(*b),
                })
            }
            // `None` is the built-in empty `Option`.
            Expr::Identifier(name) if name == "None" => {
                self.uses_option = true;
                SirExpr::Typed {
                    expr: Box::new(SirExpr::StructLiteral {
                        name: "Option".to_string(),
                        fields: vec![(
                            "tag".to_string(),
                            SirExpr::Literal(SirLiteral::Int(1)),
                        )],
                    }),
                    typ: "Option<int>".to_string(),
                }
            }
            Expr::Identifier(name) => self.lower_variable(name),
            Expr::Binary(left, op, right) => {
                let expr = SirExpr::BinOp {
                    left: Box::new(self.lower_expr(left)),
                    op: self.lower_bin_op(op),
                    right: Box::new(self.lower_expr(right)),
                };
                if matches!(
                    op,
                    AstBinOp::Eq
                        | AstBinOp::Ne
                        | AstBinOp::Lt
                        | AstBinOp::Le
                        | AstBinOp::Gt
                        | AstBinOp::Ge
                        | AstBinOp::And
                        | AstBinOp::Or
                ) {
                    SirExpr::Typed {
                        expr: Box::new(expr),
                        typ: "bool".to_string(),
                    }
                } else {
                    expr
                }
            }
            Expr::Unary(op, operand) => {
                SirExpr::UnaryOp {
                    op: self.lower_un_op(op),
                    operand: Box::new(self.lower_expr(operand)),
                }
            }
            Expr::Call(callee, args) => {
                // Calling a function-typed value goes through the closure ABI.
                if let Some(typ) = self.expr_type(callee) {
                    if typ.starts_with("fn(") {
                        if let Some((params, return_type)) = parse_function_type(&typ) {
                            let callee = self.lower_expr(callee);
                            let args = args.iter().map(|arg| self.lower_expr(arg)).collect();
                            return SirExpr::Typed {
                                expr: Box::new(SirExpr::CallValue {
                                    callee: Box::new(callee),
                                    args,
                                    return_type: return_type.clone(),
                                    params,
                                }),
                                typ: return_type,
                            };
                        }
                    }
                }
                // `Some(value)` and `Enum.Variant(value)` build tagged structs.
                if let Expr::Identifier(name) = callee.unspanned() {
                    if name == "Some" {
                        if let Some(arg) = args.first() {
                            let payload = self
                                .expr_type(arg)
                                .unwrap_or_else(|| "int".to_string());
                            return self.lower_payload_value("Option", 0, arg, &payload);
                        }
                    }
                }
                if let Expr::Field(object, variant) = callee.unspanned() {
                    if let Expr::Identifier(enum_name) = object.unspanned() {
                        if let Some(payload_type) = self
                            .enum_payloads
                            .get(enum_name)
                            .and_then(|variants| variants.get(variant))
                            .cloned()
                        {
                            let tag = self
                                .enum_ordinals
                                .get(&format!("{}_{}", enum_name, variant))
                                .copied()
                                .unwrap_or(0);
                            if let Some(arg) = args.first() {
                                // A variant declared with a generic parameter takes
                                // its payload type from the value at this call.
                                let payload_type = if self
                                    .enum_generics
                                    .get(enum_name)
                                    .is_some_and(|generics| generics.contains(&payload_type))
                                {
                                    self.expr_type(arg)
                                        .unwrap_or_else(|| "int".to_string())
                                } else {
                                    payload_type
                                };
                                return self
                                    .lower_payload_value(enum_name, tag, arg, &payload_type);
                            }
                        }
                    }
                }
                // The implicit `__builtin_*` functions map onto the runtime shim.
                if let Expr::Identifier(name) = callee.unspanned() {
                    if name.starts_with("__builtin_") {
                        if let Some(call) = self.lower_builtin_call(name, args) {
                            return call;
                        }
                    }
                }
                // `value.method(...)` is a call on the method's mangled function.
                if let Some(call) = self.lower_method_call(callee, args) {
                    return call;
                }
                // A generic function is called through its instantiation.
                if let Expr::Identifier(name) = callee.unspanned() {
                    if let Some((concrete_name, return_type)) =
                        self.resolve_generic_call(name, args)
                    {
                        let call = SirExpr::Call {
                            callee: concrete_name,
                            args: self.lower_call_args(name, args),
                        };
                        return match return_type {
                            Some(typ) => SirExpr::Typed {
                                expr: Box::new(call),
                                typ,
                            },
                            None => call,
                        };
                    }
                }
                let callee_name = match callee.unspanned() {
                    Expr::Identifier(name) => Some(name.clone()),
                    _ => None,
                };
                let call = SirExpr::Call {
                    callee: callee_name.clone().unwrap_or_else(|| "unknown".to_string()),
                    args: callee_name
                        .as_deref()
                        .map(|name| self.lower_call_args(name, args))
                        .unwrap_or_else(|| args.iter().map(|arg| self.lower_expr(arg)).collect()),
                };
                // A declared return type rides along, so printing `f()` uses the
                // same runtime function as printing a variable of that type.
                match callee_name.and_then(|name| self.signatures.get(&name).cloned().flatten()) {
                    Some(typ) => SirExpr::Typed {
                        expr: Box::new(call),
                        typ: self.substitute(&typ),
                    },
                    None => call,
                }
            }
            Expr::Assign(target, value) => match target.unspanned() {
                // A field target keeps the object and the field name, so codegen
                // can emit `object.field = value` instead of losing the target.
                Expr::Field(object, field) => SirExpr::FieldAssign {
                    object: Box::new(self.lower_expr(object)),
                    field: field.clone(),
                    value: Box::new(self.lower_expr(value)),
                },
                Expr::Index(array, index) => SirExpr::IndexAssign {
                    array: Box::new(self.lower_array_value(array)),
                    index: Box::new(self.lower_expr(index)),
                    value: Box::new(self.lower_expr(value)),
                },
                target => SirExpr::Assign {
                    target: match target {
                        Expr::Identifier(name) => name.clone(),
                        _ => "unknown".to_string(),
                    },
                    value: Box::new(self.lower_expr(value)),
                },
            },
            Expr::If(cond, then_block, else_block) => {
                // If used in expression context, keep as If expression
                let condition = Box::new(self.lower_expr(cond));
                
                let _then_stmts: Vec<SirStatement> = then_block.statements.iter()
                    .flat_map(|s| self.lower_statement(s))
                    .collect();
                let then_final = then_block.final_expr.as_ref()
                    .map(|e| Box::new(self.lower_expr(e)));
                
                let else_final = else_block.as_ref().and_then(|else_blk| 
                    else_blk.final_expr.as_ref().map(|e| Box::new(self.lower_expr(e)))
                );
                let _else_stmts: Vec<SirStatement> = else_block.as_ref().map(|else_blk| {
                    else_blk.statements.iter()
                        .flat_map(|s| self.lower_statement(s))
                        .collect()
                }).unwrap_or_default();
                
                // If there's a final expression, use If expression, otherwise use If statement
                if then_final.is_some() || else_final.is_some() {
                    SirExpr::If {
                        condition,
                        then_expr: if let Some(final_expr) = then_final {
                            final_expr
                        } else {
                            Box::new(SirExpr::Literal(SirLiteral::Unit))
                        },
                        else_expr: else_final.or(Some(Box::new(SirExpr::Literal(SirLiteral::Unit)))),
                    }
                } else {
                    // Emit as If statement - requires statement context
                    // This case should be handled by lower_statement now
                    SirExpr::Literal(SirLiteral::Unit)
                }
            }
            Expr::While(cond, body) => {
                // While in expression context returns Unit
                // Statement context is handled by lower_statement
                let _condition = self.lower_expr(cond);
                let _body_stmts: Vec<SirStatement> = body.statements.iter()
                    .flat_map(|s| self.lower_statement(s))
                    .collect();
                
                SirExpr::Literal(SirLiteral::Unit)
            }
            Expr::For(pattern, iterable, body) => {
                // For in expression context returns Unit
                // Statement context is handled by lower_statement
                let _pattern_name = match pattern {
                    Pattern::Identifier(n) => n.clone(),
                    _ => "_".to_string(),
                };
                let _iterable_expr = self.lower_expr(iterable);
                let _body_stmts: Vec<SirStatement> = body.statements.iter()
                    .flat_map(|s| self.lower_statement(s))
                    .collect();
                
                SirExpr::Literal(SirLiteral::Unit)
            }
            Expr::Return(value) => {
                // Return in expression context returns Unit
                // Statement context is handled by lower_statement
                let _value_expr = value.as_ref().map(|v| self.lower_expr(v));
                
                            SirExpr::Literal(SirLiteral::Unit)
            }
            Expr::While(cond, body) => {
                // While as expression returns Unit
                SirExpr::Block(
                    vec![SirStatement::While {
                        condition: self.lower_expr(cond),
                        body: self.lower_block(body),
                    }],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::For(pattern, iterable, body) => {
                // For as expression returns Unit
                let pattern_str = match pattern {
                    Pattern::Identifier(n) => n.clone(),
                    _ => "_".to_string(),
                };
                SirExpr::Block(
                    vec![SirStatement::For {
                        pattern: pattern_str,
                        iterable: self.lower_expr(iterable),
                        body: self.lower_block(body),
                    }],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::Return(value) => {
                // Return as expression - wraps in block with Unit return
                SirExpr::Block(
                    vec![SirStatement::Return(value.as_ref().map(|v| self.lower_expr(v)))],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::Break => {
                // Break as expression - wraps in block
                SirExpr::Block(
                    vec![SirStatement::Break],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::Continue => {
                // Continue as expression - wraps in block
                SirExpr::Block(
                    vec![SirStatement::Continue],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::Block(block) => {
                let stmts: Vec<SirStatement> = block.statements.iter()
                    .flat_map(|s| self.lower_statement(s))
                    .collect();
                let final_val = block.final_expr.as_ref()
                    .map(|e| Box::new(self.lower_expr(e)));
                
                self.lower_block_as_expr(block)
            }
            Expr::Loop(body) => {
                // Loop as expression returns Unit
                SirExpr::Block(
                    vec![SirStatement::While {
                        condition: SirExpr::Literal(SirLiteral::Bool(true)),
                        body: self.lower_block(body),
                    }],
                    Some(Box::new(SirExpr::Literal(SirLiteral::Unit))),
                )
            }
            Expr::Cast(inner, _type_expr) => {
                // For now, just pass through the inner expression
                self.lower_expr(inner)
            }
            Expr::Path(path_parts) => {
                // For simple paths, return the last part as variable
                if let Some(last) = path_parts.last() {
                    SirExpr::Var(last.clone())
                } else {
                    SirExpr::Literal(SirLiteral::Unit)
                }
            }
            Expr::Field(obj, field) => {
                // `Color.Red` is an enum variant constant, not a struct field.
                if let Expr::Identifier(enum_name) = obj.unspanned() {
                    let is_variant = self
                        .enums
                        .get(enum_name)
                        .map(|variants| variants.iter().any(|variant| variant == field))
                        .unwrap_or(false);
                    if is_variant {
                        return SirExpr::Typed {
                            expr: Box::new(SirExpr::Var(format!("{}_{}", enum_name, field))),
                            typ: "int".to_string(),
                        };
                    }
                }
                let access = SirExpr::FieldAccess {
                    object: Box::new(self.lower_expr(obj)),
                    field: field.clone(),
                };
                // Knowing the field's declared type lets `print` pick the right
                // runtime function.
                match self.expr_type(expr) {
                    Some(typ) => SirExpr::Typed {
                        expr: Box::new(access),
                        typ,
                    },
                    None => access,
                }
            }
            Expr::Index(obj, index) => {
                // `text[start..end]` is a runtime substring.
                if self
                    .expr_type(obj)
                    .is_some_and(|typ| matches!(typ.to_lowercase().as_str(), "string" | "str"))
                {
                    if let Expr::Range(start, end) = index.unspanned() {
                        return SirExpr::Typed {
                            expr: Box::new(SirExpr::StringSlice {
                                value: Box::new(self.lower_expr(obj)),
                                start: Box::new(self.lower_expr(start)),
                                end: Box::new(self.lower_expr(end)),
                            }),
                            typ: "string".to_string(),
                        };
                    }
                }
                let access = SirExpr::Index {
                    array: Box::new(self.lower_array_value(obj)),
                    index: Box::new(self.lower_expr(index)),
                };
                // Elements are typed `int` for now, so `print(arr[i])` works.
                SirExpr::Typed {
                    expr: Box::new(access),
                    typ: "int".to_string(),
                }
            }
            Expr::Range(start, end) => {
                let start_expr = match start.unspanned() {
                    Expr::Literal(Literal::Integer(0)) => SirExpr::Literal(SirLiteral::Int(0)),
                    _ => self.lower_expr(start),
                };
                SirExpr::BinOp {
                    left: Box::new(start_expr),
                    op: SirBinOp::Range,
                    right: Box::new(self.lower_expr(end)),
                }
            }
            Expr::Match(scrutinee, arms) => {
                // Lower match as a series of if-else expressions
                self.lower_match_chain(scrutinee, arms)
            }
        }
    }

    /// Lower an `if` whose branches are the enclosing function's result, so each
    /// branch ends in a `return` instead of an unused expression.
    fn lower_if_as_return(
        &mut self,
        condition: &Expr,
        then_block: &nova_common::Block,
        else_block: &Option<nova_common::Block>,
    ) -> SirStatement {
        SirStatement::If {
            condition: self.lower_expr(condition),
            then_block: self.lower_value_block(then_block),
            else_block: else_block
                .as_ref()
                .map(|block| self.lower_value_block(block)),
        }
    }

    /// A block used as a value: its statements run, then its final expression is
    /// returned. A nested trailing `if` recurses, so `else if` chains work.
    fn lower_value_block(&mut self, block: &nova_common::Block) -> Vec<SirStatement> {
        self.push_scope();
        let mut stmts = Vec::new();

        // `else if` is parsed as an `else` block whose last *statement* is the
        // nested `if`, not as its final expression. That nested `if` is still the
        // value of the block, so lower it as a returning if instead of a
        // statement whose branch values would be discarded.
        let trailing_if = if block.final_expr.is_none() {
            match block.statements.last() {
                Some(Statement::Expr(expr)) => match expr.unspanned() {
                    Expr::If(condition, then_block, else_block) => {
                        Some((condition, then_block, else_block))
                    }
                    _ => None,
                },
                _ => None,
            }
        } else {
            None
        };

        let statement_count = block.statements.len();
        for (index, statement) in block.statements.iter().enumerate() {
            if trailing_if.is_some() && index + 1 == statement_count {
                break;
            }
            stmts.extend(self.lower_statement(statement));
        }
        if let Some((condition, then_block, else_block)) = trailing_if {
            stmts.push(self.lower_if_as_return(condition, then_block, else_block));
        }

        if let Some(final_expr) = &block.final_expr {
            match final_expr.unspanned() {
                Expr::If(condition, then_block, else_block) => {
                    stmts.push(self.lower_if_as_return(condition, then_block, else_block));
                }
                Expr::Block(inner) => {
                    stmts.extend(self.lower_value_block(inner));
                }
                Expr::Return(value) => {
                    stmts.push(SirStatement::Return(value.as_ref().map(|v| self.lower_expr(v))));
                }
                // A call that never returns (`panic`, `exit`) is a statement, not
                // a value: `return panic(...)` would not compile.
                Expr::Call(callee, _) if diverging_call(callee) => {
                    stmts.extend(self.lower_statement(&Statement::Expr((**final_expr).clone())));
                }
                _ => stmts.push(SirStatement::Return(Some(self.lower_expr(final_expr)))),
            }
        }
        self.pop_scope();
        stmts
    }

    fn lower_block_as_expr(&mut self, block: &nova_common::Block) -> SirExpr {
        let mut stmts = Vec::new();
        for stmt in &block.statements {
            stmts.extend(self.lower_statement(stmt));
        }
        let final_expr = block.final_expr.as_ref().map(|e| Box::new(self.lower_expr(e)));
        SirExpr::Block(stmts, final_expr)
    }

    #[allow(dead_code)]
    fn lower_match_as_if(&mut self, scrutinee: &Expr, arms: &[MatchArm]) -> SirExpr {
        if arms.is_empty() {
            return SirExpr::Literal(SirLiteral::Unit);
        }

        let scrutinee_expr = self.lower_expr(scrutinee);
        
        // Build if-else chain from match arms
        let mut current_if: Option<SirExpr> = None;
        
        for (i, arm) in arms.iter().enumerate().rev() {
            let result_expr = self.lower_expr(&arm.body);
            
            // For now, only handle simple identifier patterns and wildcard
            let is_wildcard = matches!(arm.pattern, Pattern::Wildcard);
            let pattern_name = match &arm.pattern {
                Pattern::Identifier(n) => Some(n.clone()),
                _ => None,
            };
            
            if is_wildcard || i == arms.len() - 1 {
                // Last arm or wildcard - becomes the else branch
                if let Some(prev_if) = current_if.take() {
                    if let SirExpr::If { condition, then_expr, .. } = prev_if {
                        current_if = Some(SirExpr::If {
                            condition,
                            then_expr,
                            else_expr: Some(Box::new(result_expr)),
                        });
                    }
                } else {
                    current_if = Some(result_expr);
                }
            } else if let Some(pattern_name) = pattern_name {
                // Simple equality check
                let condition = SirExpr::BinOp {
                    left: Box::new(scrutinee_expr.clone()),
                    op: SirBinOp::Eq,
                    right: Box::new(SirExpr::Var(pattern_name)),
                };
                
                if let Some(prev_if) = current_if.take() {
                    current_if = Some(SirExpr::If {
                        condition: Box::new(condition),
                        then_expr: Box::new(result_expr),
                        else_expr: Some(Box::new(prev_if)),
                    });
                } else {
                    current_if = Some(SirExpr::If {
                        condition: Box::new(condition),
                        then_expr: Box::new(result_expr),
                        else_expr: None,
                    });
                }
            }
        }
        
        current_if.unwrap_or_else(|| SirExpr::Literal(SirLiteral::Unit))
    }

    /// `Some(value)` and enum payload constructions become tagged struct
    /// literals; the payload goes in the field that matches its type.
    fn lower_payload_value(
        &mut self,
        enum_name: &str,
        tag: i64,
        arg: &Expr,
        payload_type: &str,
    ) -> SirExpr {
        if enum_name == "Option" {
            self.uses_option = true;
        }
        let (field, _) = payload_field(payload_type);
        let fields = vec![
            ("tag".to_string(), SirExpr::Literal(SirLiteral::Int(tag))),
            (field.to_string(), self.lower_expr(arg)),
        ];
        let typ = if enum_name == "Option" {
            format!("Option<{}>", payload_type)
        } else {
            enum_name.to_string()
        };
        SirExpr::Typed {
            expr: Box::new(SirExpr::StructLiteral {
                name: enum_name.to_string(),
                fields,
            }),
            typ,
        }
    }

    /// Lower a statement-position `match` to a C `switch`. Returns `None` when
    /// an arm has no integer constant test (a string literal or a binding), so
    /// the caller keeps the expression-form chain instead.
    fn lower_match_statement(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
    ) -> Option<Vec<SirStatement>> {
        // A guarded arm is not a constant `case`, so use the if/else chain.
        if arms.iter().any(|arm| arm.guard.is_some()) {
            return None;
        }
        let record_expr = self.lower_expr(scrutinee);
        let scrutinee_type = self.expr_type(scrutinee);
        // A payload enum is a struct, so the switch tests its `tag`; a
        // payload-less enum is already an integer value.
        let is_payload_enum = scrutinee_type
            .as_deref()
            .map(|typ| typ.split('<').next().unwrap_or(typ))
            .is_some_and(|base| base == "Option" || self.enum_payloads.contains_key(base));
        let scrutinee_expr = if is_payload_enum {
            SirExpr::FieldAccess {
                object: Box::new(record_expr.clone()),
                field: "tag".to_string(),
            }
        } else {
            record_expr.clone()
        };
        let mut cases: Vec<(SirExpr, Vec<SirStatement>)> = Vec::new();
        let mut default = None;

        for arm in arms {
            let (constant, binding) = match &arm.pattern {
                Pattern::Literal(Literal::Integer(n)) => {
                    (Some(SirExpr::Literal(SirLiteral::Int(*n))), None)
                }
                Pattern::Literal(Literal::Bool(b)) => {
                    (Some(SirExpr::Literal(SirLiteral::Bool(*b))), None)
                }
                Pattern::Literal(Literal::Char(c)) => {
                    (Some(SirExpr::Literal(SirLiteral::Char(*c))), None)
                }
                Pattern::Path(parts) if parts.len() == 2 => {
                    let name = format!("{}_{}", parts[0], parts[1]);
                    (
                        Some(SirExpr::Literal(SirLiteral::Int(
                            *self.enum_ordinals.get(&name)?,
                        ))),
                        None,
                    )
                }
                Pattern::Identifier(name) if name == "None" => {
                    self.uses_option = true;
                    (Some(SirExpr::Literal(SirLiteral::Int(1))), None)
                }
                Pattern::EnumVariant(enum_name, variant, inner) => {
                    let binding_name = match inner.as_deref() {
                        Some(Pattern::Identifier(name)) if name != "_" => Some(name.clone()),
                        _ => None,
                    };
                    let (tag, payload_type) = if enum_name == "Some"
                        || (enum_name == "Option" && variant == "Some")
                    {
                        self.uses_option = true;
                        (
                            0,
                            scrutinee_type
                                .as_deref()
                                .and_then(option_payload_type)
                                .or_else(|| Some("int".to_string())),
                        )
                    } else if enum_name == "None" {
                        (1, None)
                    } else {
                        let key = format!("{}_{}", enum_name, variant);
                        let tag = self.enum_ordinals.get(&key).copied()?;
                        (
                            tag,
                            self.enum_payloads
                                .get(enum_name)
                                .and_then(|variants| variants.get(variant))
                                .map(|payload| {
                                    substitute_generic_payload(
                                        payload,
                                        enum_name,
                                        scrutinee_type.as_deref(),
                                        self.enum_generics.get(enum_name),
                                    )
                                }),
                        )
                    };
                    let binding = binding_name.map(|name| {
                        let typ = payload_type
                            .clone()
                            .unwrap_or_else(|| "int".to_string());
                        (name, typ)
                    });
                    (Some(SirExpr::Literal(SirLiteral::Int(tag))), binding)
                }
                Pattern::Wildcard | Pattern::Identifier(_) => (None, None),
                _ => return None,
            };

            self.push_scope();
            let mut body = Vec::new();
            if let Some((name, typ)) = binding {
                let (field, _) = payload_field(&typ);
                body.push(SirStatement::Let {
                    name: name.clone(),
                    typ: Some(typ.clone()),
                    value: SirExpr::FieldAccess {
                        object: Box::new(record_expr.clone()),
                        field: field.to_string(),
                    },
                });
                self.declare(&name, Some(typ));
            }
            body.extend(self.lower_arm_body(&arm.body));
            self.pop_scope();

            match constant {
                Some(constant) => cases.push((constant, body)),
                None => {
                    if default.is_none() {
                        default = Some(body);
                    }
                }
            }
        }

        Some(vec![SirStatement::Switch {
            scrutinee: scrutinee_expr,
            cases,
            default,
        }])
    }

    /// The statements of one `match` arm body: a block keeps its statements, a
    /// bare `return` stays a return, anything else becomes an expression
    /// statement.
    fn lower_arm_body(&mut self, body: &Expr) -> Vec<SirStatement> {
        match body.unspanned() {
            Expr::Return(value) => {
                vec![SirStatement::Return(
                    value.as_ref().map(|v| self.lower_expr(v)),
                )]
            }
            Expr::Block(block) => self.lower_block(block),
            _ => vec![SirStatement::Expr(self.lower_expr(body))],
        }
    }

    /// A statement-position `match` with string literals becomes an if/else
    /// chain, because C case labels must be integers.
    fn lower_match_to_if(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
    ) -> Option<Vec<SirStatement>> {
        let scrutinee_expr = self.lower_expr(scrutinee);
        let mut fallback: Vec<SirStatement> = Vec::new();
        let mut has_test = false;

        for arm in arms.iter().rev() {
            // A binding pattern (`x if x > 10`) declares its name before the
            // guard and the body are lowered.
            self.push_scope();
            let mut binding: Option<(String, String)> = None;
            if let Pattern::Identifier(name) = &arm.pattern {
                if name != "_" {
                    let typ = self
                        .expr_type(scrutinee)
                        .unwrap_or_else(|| "int".to_string());
                    self.declare(name, Some(typ.clone()));
                    binding = Some((name.clone(), typ));
                }
            }
            let guard = arm.guard.as_ref().map(|guard| self.lower_expr(guard));
            let body = self.lower_arm_body(&arm.body);
            self.pop_scope();
            // The binding must exist before the guard test, so the arm becomes
            // `{ let x = value; if (guard) … }`.
            let with_binding = |statements: Vec<SirStatement>| -> Vec<SirStatement> {
                match &binding {
                    Some((name, typ)) => {
                        let mut wrapped = vec![SirStatement::Let {
                            name: name.clone(),
                            typ: Some(typ.clone()),
                            value: scrutinee_expr.clone(),
                        }];
                        wrapped.extend(statements);
                        wrapped
                    }
                    None => statements,
                }
            };
            let pattern_condition = match &arm.pattern {
                Pattern::Literal(Literal::String(text)) => Some(SirExpr::BinOp {
                    left: Box::new(SirExpr::Typed {
                        expr: Box::new(SirExpr::Call {
                            callee: "nova_strcmp".to_string(),
                            args: vec![
                                scrutinee_expr.clone(),
                                SirExpr::Literal(SirLiteral::String(text.clone())),
                            ],
                        }),
                        typ: "int".to_string(),
                    }),
                    op: SirBinOp::Eq,
                    right: Box::new(SirExpr::Literal(SirLiteral::Int(0))),
                }),
                Pattern::Literal(Literal::Integer(value)) => Some(SirExpr::BinOp {
                    left: Box::new(scrutinee_expr.clone()),
                    op: SirBinOp::Eq,
                    right: Box::new(SirExpr::Literal(SirLiteral::Int(*value))),
                }),
                Pattern::Literal(Literal::Bool(value)) => Some(SirExpr::BinOp {
                    left: Box::new(scrutinee_expr.clone()),
                    op: SirBinOp::Eq,
                    right: Box::new(SirExpr::Literal(SirLiteral::Bool(*value))),
                }),
                Pattern::Literal(Literal::Char(value)) => Some(SirExpr::BinOp {
                    left: Box::new(scrutinee_expr.clone()),
                    op: SirBinOp::Eq,
                    right: Box::new(SirExpr::Literal(SirLiteral::Char(*value))),
                }),
                Pattern::Path(parts) if parts.len() == 2 => Some(SirExpr::BinOp {
                    left: Box::new(scrutinee_expr.clone()),
                    op: SirBinOp::Eq,
                    right: Box::new(SirExpr::Var(format!("{}_{}", parts[0], parts[1]))),
                }),
                Pattern::Wildcard | Pattern::Identifier(_) => None,
                _ => return None,
            };
            // A guard narrows the arm: `pattern && guard`.
            let condition = match (pattern_condition, guard) {
                (Some(pattern), Some(guard)) => Some(SirExpr::BinOp {
                    left: Box::new(pattern),
                    op: SirBinOp::And,
                    right: Box::new(SirExpr::Typed {
                        expr: Box::new(guard),
                        typ: "bool".to_string(),
                    }),
                }),
                (pattern, guard) => pattern.or(guard),
            };

            match condition {
                Some(condition) => {
                    has_test = true;
                    let branch = SirStatement::If {
                        condition,
                        then_block: body,
                        else_block: Some(fallback),
                    };
                    fallback = if binding.is_some() {
                        vec![SirStatement::Block(with_binding(vec![branch]))]
                    } else {
                        vec![branch]
                    };
                }
                None => fallback = with_binding(body),
            }
        }

        if has_test {
            Some(fallback)
        } else {
            None
        }
    }

    /// `match` becomes a chain of equality tests (ternaries in C). Supported
    /// patterns: literals, `Enum.Variant` paths, and the wildcard arm.
    fn lower_match_chain(&mut self, scrutinee: &Expr, arms: &[MatchArm]) -> SirExpr {
        let scrutinee_expr = self.lower_expr(scrutinee);
        let mut result: Option<SirExpr> = None;

        for arm in arms.iter().rev() {
            let guard = arm.guard.as_ref().map(|guard| self.lower_expr(guard));
            let body = self.lower_expr(&arm.body);
            let pattern_condition = match &arm.pattern {
                // Strings compare through the runtime; other literals use `==`.
                Pattern::Literal(Literal::String(text)) => Some(SirExpr::BinOp {
                    left: Box::new(SirExpr::Typed {
                        expr: Box::new(SirExpr::Call {
                            callee: "nova_strcmp".to_string(),
                            args: vec![
                                scrutinee_expr.clone(),
                                SirExpr::Literal(SirLiteral::String(text.clone())),
                            ],
                        }),
                        typ: "int".to_string(),
                    }),
                    op: SirBinOp::Eq,
                    right: Box::new(SirExpr::Literal(SirLiteral::Int(0))),
                }),
                Pattern::Literal(literal) => Some(SirExpr::BinOp {
                    left: Box::new(scrutinee_expr.clone()),
                    op: SirBinOp::Eq,
                    right: Box::new(match literal {
                        Literal::Integer(n) => SirExpr::Literal(SirLiteral::Int(*n)),
                        Literal::Float(f) => SirExpr::Literal(SirLiteral::Float(*f)),
                        Literal::Bool(b) => SirExpr::Literal(SirLiteral::Bool(*b)),
                        Literal::Char(c) => SirExpr::Literal(SirLiteral::Char(*c)),
                        Literal::String(_) => unreachable!("handled above"),
                    }),
                }),
                Pattern::Path(parts) if parts.len() == 2 => Some(SirExpr::BinOp {
                    left: Box::new(scrutinee_expr.clone()),
                    op: SirBinOp::Eq,
                    right: Box::new(SirExpr::Var(format!("{}_{}", parts[0], parts[1]))),
                }),
                // Wildcards and patterns without a test become the fallback.
                _ => None,
            };
            let condition = match (pattern_condition, guard) {
                (Some(pattern), Some(guard)) => Some(SirExpr::BinOp {
                    left: Box::new(pattern),
                    op: SirBinOp::And,
                    right: Box::new(SirExpr::Typed {
                        expr: Box::new(guard),
                        typ: "bool".to_string(),
                    }),
                }),
                (pattern, guard) => pattern.or(guard),
            };

            result = Some(match condition {
                Some(condition) => SirExpr::If {
                    condition: Box::new(condition),
                    then_expr: Box::new(body),
                    else_expr: result.map(Box::new),
                },
                None => body,
            });
        }

        result.unwrap_or(SirExpr::Literal(SirLiteral::Unit))
    }

    fn lower_bin_op(&self, op: &AstBinOp) -> SirBinOp {
        match op {
            AstBinOp::Add => SirBinOp::Add,
            AstBinOp::Sub => SirBinOp::Sub,
            AstBinOp::Mul => SirBinOp::Mul,
            AstBinOp::Div => SirBinOp::Div,
            AstBinOp::Rem => SirBinOp::Mod,
            AstBinOp::Eq => SirBinOp::Eq,
            AstBinOp::Ne => SirBinOp::Ne,
            AstBinOp::Lt => SirBinOp::Lt,
            AstBinOp::Le => SirBinOp::Le,
            AstBinOp::Gt => SirBinOp::Gt,
            AstBinOp::Ge => SirBinOp::Ge,
            AstBinOp::And => SirBinOp::And,
            AstBinOp::Or => SirBinOp::Or,
            _ => SirBinOp::Add, // Handle BitAnd, BitOr, BitXor, Shl, Shr
        }
    }

    fn lower_un_op(&self, op: &AstUnOp) -> SirUnOp {
        match op {
            AstUnOp::Neg => SirUnOp::Neg,
            AstUnOp::Not => SirUnOp::Not,
            AstUnOp::Deref => SirUnOp::Neg, // Placeholder for Deref
            AstUnOp::AddrOf => SirUnOp::Not, // Placeholder for AddrOf
        }
    }
}

/// The struct fields that hold an enum payload: the discriminant plus one slot
/// per scalar payload type.
/// Parse a function type such as `fn(int, string) -> bool`.
fn parse_function_type(typ: &str) -> Option<(Vec<String>, String)> {
    let rest = typ.strip_prefix("fn(")?;
    let (params, return_type) = rest.split_once(") -> ")?;
    let params = if params.trim().is_empty() {
        Vec::new()
    } else {
        params.split(',').map(|param| param.trim().to_string()).collect()
    };
    Some((params, return_type.trim().to_string()))
}

/// Names bound by a pattern.
fn pattern_binding_names(pattern: &Pattern, bound: &mut HashSet<String>) {
    match pattern {
        Pattern::Identifier(name) => {
            bound.insert(name.clone());
        }
        Pattern::Tuple(patterns) => {
            for pattern in patterns {
                pattern_binding_names(pattern, bound);
            }
        }
        Pattern::Struct(_, fields) => {
            for (_, pattern) in fields {
                pattern_binding_names(pattern, bound);
            }
        }
        Pattern::EnumVariant(_, _, Some(pattern)) => pattern_binding_names(pattern, bound),
        _ => {}
    }
}

/// Collect identifiers used by an expression that are not locally bound.
fn collect_names_expr(expr: &Expr, bound: &mut HashSet<String>, out: &mut Vec<String>) {
    match expr.unspanned() {
        Expr::Identifier(name) => {
            if !bound.contains(name) {
                out.push(name.clone());
            }
        }
        Expr::Binary(left, _, right) => {
            collect_names_expr(left, bound, out);
            collect_names_expr(right, bound, out);
        }
        Expr::Unary(_, inner) | Expr::Cast(inner, _) => collect_names_expr(inner, bound, out),
        Expr::Call(callee, args) => {
            collect_names_expr(callee, bound, out);
            for arg in args {
                collect_names_expr(arg, bound, out);
            }
        }
        Expr::Field(object, _) => collect_names_expr(object, bound, out),
        Expr::Index(object, index) => {
            collect_names_expr(object, bound, out);
            collect_names_expr(index, bound, out);
        }
        Expr::Range(start, end) => {
            collect_names_expr(start, bound, out);
            collect_names_expr(end, bound, out);
        }
        Expr::If(condition, then_block, else_block) => {
            collect_names_expr(condition, bound, out);
            collect_names_block(then_block, bound, out);
            if let Some(else_block) = else_block {
                collect_names_block(else_block, bound, out);
            }
        }
        Expr::Match(scrutinee, arms) => {
            collect_names_expr(scrutinee, bound, out);
            for arm in arms {
                collect_names_expr(&arm.body, bound, out);
                if let Some(guard) = &arm.guard {
                    collect_names_expr(guard, bound, out);
                }
            }
        }
        Expr::While(condition, body) => {
            collect_names_expr(condition, bound, out);
            collect_names_block(body, bound, out);
        }
        Expr::For(pattern, iterable, body) => {
            collect_names_expr(iterable, bound, out);
            pattern_binding_names(pattern, bound);
            collect_names_block(body, bound, out);
        }
        Expr::Loop(body) | Expr::Block(body) => collect_names_block(body, bound, out),
        Expr::Return(value) => {
            if let Some(value) = value {
                collect_names_expr(value, bound, out);
            }
        }
        Expr::Assign(target, value) => {
            collect_names_expr(target, bound, out);
            collect_names_expr(value, bound, out);
        }
        Expr::StructLiteral(_, fields) => {
            for (field, value) in fields {
                match value {
                    Some(value) => collect_names_expr(value, bound, out),
                    None => {
                        if !bound.contains(field) {
                            out.push(field.clone());
                        }
                    }
                }
            }
        }
        Expr::Tuple(items) | Expr::Array(items) => {
            for item in items {
                collect_names_expr(item, bound, out);
            }
        }
        Expr::Closure(params, body) => {
            let mut inner = bound.clone();
            inner.extend(params.iter().map(|(name, _)| name.clone()));
            collect_names_expr(body, &mut inner, out);
        }
        _ => {}
    }
}

fn collect_names_block(
    block: &nova_common::Block,
    bound: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    for statement in &block.statements {
        match statement {
            Statement::Let(let_stmt) => {
                if let Some(init) = &let_stmt.init {
                    collect_names_expr(init, bound, out);
                }
                pattern_binding_names(&let_stmt.pattern, bound);
            }
            Statement::Expr(expr) => collect_names_expr(expr, bound, out),
            Statement::Item(_) => {}
        }
    }
    if let Some(final_expr) = &block.final_expr {
        collect_names_expr(final_expr, bound, out);
    }
}

fn payload_struct_fields() -> Vec<SirStructField> {
    vec![
        SirStructField {
            name: "tag".to_string(),
            typ: "int".to_string(),
        },
        SirStructField {
            name: "payload_i".to_string(),
            typ: "int".to_string(),
        },
        SirStructField {
            name: "payload_f".to_string(),
            typ: "float".to_string(),
        },
        SirStructField {
            name: "payload_s".to_string(),
            typ: "string".to_string(),
        },
    ]
}

/// Which payload slot a value of `typ` uses, and the type it is read back as.
fn payload_field(typ: &str) -> (&'static str, &'static str) {
    match typ.to_lowercase().as_str() {
        "float" | "f32" | "f64" => ("payload_f", "float"),
        "string" | "str" => ("payload_s", "string"),
        _ => ("payload_i", "int"),
    }
}

/// A type as written, including generic arguments: `Result<int, String>`.
/// Calls that never return a value because they abort the program.
fn diverging_call(callee: &Expr) -> bool {
    matches!(callee.unspanned(), Expr::Identifier(name) if name == "panic" || name == "exit")
}

fn type_display(typ: &nova_common::TypeExpr) -> String {
    if typ.generics.is_empty() {
        typ.name.clone()
    } else {
        format!(
            "{}<{}>",
            typ.name,
            typ.generics
                .iter()
                .map(type_display)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

/// The arguments in `Name<A, B>`, if the type name has any.
fn generic_arguments(typ: &str) -> Option<Vec<String>> {
    let inner = typ.split_once('<')?.1.strip_suffix('>')?;
    Some(inner.split(',').map(|arg| arg.trim().to_string()).collect())
}

/// Replace a variant payload type that names one of the enum's generic
/// parameters with the concrete argument from the matched type.
fn substitute_generic_payload(
    payload: &str,
    _enum_name: &str,
    scrutinee_type: Option<&str>,
    generics: Option<&Vec<String>>,
) -> String {
    let (Some(generics), Some(scrutinee)) = (generics, scrutinee_type) else {
        return payload.to_string();
    };
    let Some(arguments) = generic_arguments(scrutinee) else {
        return payload.to_string();
    };
    generics
        .iter()
        .position(|generic| generic == payload)
        .and_then(|index| arguments.get(index).cloned())
        .unwrap_or_else(|| payload.to_string())
}

/// The `T` in `Option<T>`, if the type name is an option.
fn option_payload_type(typ: &str) -> Option<String> {
    let inner = typ.strip_prefix("Option<")?.strip_suffix('>')?;
    Some(inner.trim().to_string())
}

/// Replace whole identifiers in a type name: `T` in `[T]`, but not inside
/// `Total`.
fn substitute_type_name(typ: &str, substitution: &HashMap<String, String>) -> String {
    let mut result = typ.to_string();
    for (generic, concrete) in substitution {
        result = replace_identifier(&result, generic, concrete);
    }
    result
}

fn replace_identifier(text: &str, from: &str, to: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            word.push(ch);
        } else {
            push_word(&mut out, &mut word, from, to);
            out.push(ch);
        }
    }
    push_word(&mut out, &mut word, from, to);
    out
}

fn push_word(out: &mut String, word: &mut String, from: &str, to: &str) {
    if word.is_empty() {
        return;
    }
    if word == from {
        out.push_str(to);
    } else {
        out.push_str(word);
    }
    word.clear();
}

/// `int` -> `int`, `&mut [int]` -> `mut_int`: a C-identifier-safe suffix.
fn mangle_type(typ: &str) -> String {
    let mut out = String::new();
    for ch in typ.chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lexer, Parser};

    fn lower(source: &str) -> SirProgram {
        let mut lexer = Lexer::new(source);
        let (tokens, diagnostics) = lexer.tokenize();
        assert!(!diagnostics.has_errors(), "unexpected lexer errors");
        let program = Parser::new(tokens).parse().expect("program should parse");
        let mut lowerer = Lowerer::new();
        lowerer.lower_program(&program)
    }

    fn print_arg(program: &SirProgram, index: usize) -> &SirExpr {
        match &program.functions[0].body[index] {
            SirStatement::Expr(SirExpr::Call { args, .. }) => &args[0],
            other => panic!("expected a print call, got {other:?}"),
        }
    }

    #[test]
    fn variables_and_comparisons_carry_their_type() {
        let program = lower(
            "fn main() {\n    let flag: bool = true;\n    print(flag);\n    print(1 < 2);\n}\n",
        );
        assert!(
            matches!(print_arg(&program, 1), SirExpr::Typed { typ, .. } if typ == "bool"),
            "a declared bool variable should be typed: {:?}",
            print_arg(&program, 1)
        );
        assert!(
            matches!(print_arg(&program, 2), SirExpr::Typed { typ, .. } if typ == "bool"),
            "a comparison should be typed bool: {:?}",
            print_arg(&program, 2)
        );
    }

    #[test]
    fn literal_initialisers_are_typed_without_annotations() {
        let program = lower("fn main() {\n    let s = \"hi\";\n    let f = 1.5;\n    let b = true;\n}\n");
        let body = &program.functions[0].body;
        assert!(matches!(&body[0], SirStatement::Let { typ: Some(t), .. } if t == "string"));
        assert!(matches!(&body[1], SirStatement::Let { typ: Some(t), .. } if t == "float"));
        assert!(matches!(&body[2], SirStatement::Let { typ: Some(t), .. } if t == "bool"));
    }

    #[test]
    fn call_results_carry_the_declared_return_type() {
        let program = lower(
            "fn is_ready() -> bool {\n    return 1 < 2;\n}\n\nfn main() {\n    print(is_ready());\n}\n",
        );
        let main = program.functions.last().expect("main should be lowered");
        match &main.body[0] {
            SirStatement::Expr(SirExpr::Call { args, .. }) => assert!(
                matches!(&args[0], SirExpr::Typed { typ, .. } if typ == "bool"),
                "expected a typed call, got {:?}",
                args[0]
            ),
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn forward_references_are_typed() {
        let program = lower(
            "fn main() {\n    print(greeting());\n}\n\nfn greeting() -> string {\n    return \"hi\";\n}\n",
        );
        match &program.functions[0].body[0] {
            SirStatement::Expr(SirExpr::Call { args, .. }) => assert!(
                matches!(&args[0], SirExpr::Typed { typ, .. } if typ == "string"),
                "expected a typed call, got {:?}",
                args[0]
            ),
            other => panic!("unexpected statement: {other:?}"),
        }
    }

    #[test]
    fn field_assignment_keeps_its_target() {
        let program = lower(
            "struct Point {\n    x: int,\n    y: int,\n}\n\nfn main() {\n    let mut p = Point { x: 1, y: 2 };\n    p.x = 5;\n}\n",
        );
        let main = program.functions.last().expect("main should be lowered");
        match &main.body[1] {
            SirStatement::Expr(SirExpr::FieldAssign { object, field, value }) => {
                // The object is a known variable, so it carries its type.
                let object = match object.as_ref() {
                    SirExpr::Typed { expr, .. } => expr.as_ref(),
                    other => other,
                };
                assert!(matches!(object, SirExpr::Var(name) if name == "p"));
                assert_eq!(field, "x");
                assert!(matches!(**value, SirExpr::Literal(SirLiteral::Int(5))));
            }
            other => panic!("expected a field assignment, got {other:?}"),
        }
    }

    #[test]
    fn methods_lower_to_mangled_functions_with_a_pointer_receiver() {
        let program = lower(
            "struct Counter {\n    value: int,\n}\n\nimpl Counter {\n    fn bump(&mut self) {\n        self.value = self.value + 1;\n    }\n}\n\nfn main() {\n    let mut c = Counter { value: 0 };\n    c.bump();\n}\n",
        );

        let bump = program
            .functions
            .iter()
            .find(|function| function.name == "Counter_bump")
            .expect("the method should be lowered as Counter_bump");
        assert_eq!(bump.params[0].name, "self");
        assert_eq!(bump.params[0].typ, "Counter*");
        assert!(
            bump.body.iter().any(|statement| matches!(
                statement,
                SirStatement::Expr(SirExpr::FieldAssign { field, .. }) if field == "value"
            )),
            "the method body should assign self.value: {:?}",
            bump.body
        );

        let main = program.functions.last().expect("main should be lowered");
        match &main.body[1] {
            SirStatement::Expr(SirExpr::Call { callee, args }) => {
                assert_eq!(callee, "Counter_bump");
                assert!(
                    matches!(args.first(), Some(SirExpr::AddressOf(_))),
                    "the receiver should be passed by address: {args:?}"
                );
            }
            other => panic!("expected a method call, got {other:?}"),
        }
    }

    #[test]
    fn a_trailing_expression_becomes_a_return() {
        let program = lower("fn value() -> int {\n    6 * 7\n}\n");
        let function = program
            .functions
            .iter()
            .find(|function| function.name == "value")
            .expect("value should be lowered");
        assert!(
            matches!(function.body.last(), Some(SirStatement::Return(Some(_)))),
            "the trailing expression should be returned: {:?}",
            function.body
        );
    }

    #[test]
    fn option_values_are_tagged_structs_and_match_binds_payloads() {
        let program = lower(
            "fn show(value: Option<int>) {\n    match value {\n        Some(v) => { print(v); },\n        None => { print(0); },\n    }\n}\n\nfn main() {\n    show(Some(42));\n    show(None);\n}\n",
        );

        let show = program
            .functions
            .iter()
            .find(|function| function.name == "show")
            .expect("show should be lowered");
        let SirStatement::Switch { scrutinee, cases, .. } = show
            .body
            .iter()
            .find(|statement| matches!(statement, SirStatement::Switch { .. }))
            .expect("the option match should become a switch")
        else {
            unreachable!("just matched")
        };
        assert!(
            matches!(scrutinee, SirExpr::FieldAccess { field, .. } if field == "tag"),
            "the switch must test the tag: {scrutinee:?}"
        );
        assert_eq!(cases.len(), 2, "Some and None are both tag cases");
        assert!(
            cases[0].1.iter().any(|statement| matches!(
                statement,
                SirStatement::Let { name, .. } if name == "v"
            )),
            "the payload binding must be declared in the arm"
        );
        assert!(
            program.structs.iter().any(|struct_def| struct_def.name == "Option"),
            "the Option struct must be emitted"
        );
    }

    #[test]
    fn declared_enum_payloads_are_constructed_and_bound() {
        let program = lower(
            "enum Shape {\n    Circle(int),\n    Square(int),\n}\n\nfn main() {\n    let shape: Shape = Shape.Circle(7);\n    match shape {\n        Shape.Circle(r) => { print(r); },\n        Shape.Square(w) => { print(w); },\n    }\n}\n",
        );
        assert!(program.structs.iter().any(|struct_def| struct_def.name == "Shape"));
        let main = program.functions.iter().find(|f| f.name == "main").expect("main");
        let SirStatement::Let { value, .. } = &main.body[0] else {
            panic!("expected a let, got {:?}", main.body[0]);
        };
        let SirExpr::Typed { expr, .. } = value else {
            panic!("payload construction should be typed: {value:?}");
        };
        assert!(
            matches!(expr.as_ref(), SirExpr::StructLiteral { name, fields }
                if name == "Shape" && fields.iter().any(|(f, _)| f == "payload_i")),
            "expected a tagged Shape literal: {expr:?}"
        );
    }

    #[test]
    fn builtin_calls_lower_to_the_runtime() {
        let program = lower(
            "fn main() {\n    let n: int = __builtin_strlen(\"hi\");\n    print(n);\n    print(__builtin_strcat(\"a\", \"b\"));\n}\n",
        );
        let main = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main should be lowered");

        let SirStatement::Let { value, .. } = &main.body[0] else {
            panic!("first statement should be a let: {:?}", main.body[0]);
        };
        let SirExpr::Typed { expr, typ } = value else {
            panic!("builtin result should be typed: {value:?}");
        };
        assert_eq!(typ, "int");
        assert!(
            matches!(expr.as_ref(), SirExpr::Call { callee, .. } if callee == "nova_strlen"),
            "strlen should call the runtime: {expr:?}"
        );
    }

    #[test]
    fn statement_match_is_lowered_to_a_switch() {
        let program = lower(
            "enum Color {\n    Red,\n    Green,\n}\n\nfn main() {\n    let c = Color.Green;\n    match c {\n        Color.Red => { print(\"red\"); },\n        Color.Green => { print(\"green\"); },\n        _ => { print(\"other\"); },\n\n    }\n}\n",
        );
        let main = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .expect("main should be lowered");
        let switch = main
            .body
            .iter()
            .find(|statement| matches!(statement, SirStatement::Switch { .. }))
            .expect("a statement match should become a Switch");

        let SirStatement::Switch { cases, default, .. } = switch else {
            unreachable!("just matched")
        };
        assert_eq!(cases.len(), 2, "one case per enum arm");
        assert!(
            matches!(&cases[0].0, SirExpr::Literal(SirLiteral::Int(0))),
            "Color.Red is ordinal 0"
        );
        assert!(
            matches!(&cases[1].0, SirExpr::Literal(SirLiteral::Int(1))),
            "Color.Green is ordinal 1"
        );
        assert!(default.is_some(), "the wildcard arm becomes default");
    }

    #[test]
    fn generic_functions_are_instantiated_per_call() {
        let program = lower(
            "fn id<T>(x: T) -> T {\n    x\n}\n\nfn show<T>(value: T) {\n    print(value);\n}\n\nfn main() {\n    print(id(7));\n    print(id(\"hi\"));\n    show(42);\n    show(\"hey\");\n}\n",
        );

        let names: Vec<&str> = program
            .functions
            .iter()
            .map(|function| function.name.as_str())
            .collect();
        assert!(names.contains(&"id_int"), "functions: {names:?}");
        assert!(names.contains(&"id_string"), "functions: {names:?}");
        assert!(names.contains(&"show_int"), "functions: {names:?}");
        assert!(names.contains(&"show_string"), "functions: {names:?}");
        assert!(
            !names.contains(&"id") && !names.contains(&"show"),
            "the generic templates should not be emitted: {names:?}"
        );

        let id_string = program
            .functions
            .iter()
            .find(|function| function.name == "id_string")
            .expect("id_string should be lowered");
        assert_eq!(id_string.params[0].typ, "string");
        assert_eq!(id_string.return_type.as_deref(), Some("string"));
    }

    #[test]
    fn enums_become_constants_and_match_becomes_comparisons() {
        let program = lower(
            "enum Color {\n    Red,\n    Green,\n    Blue,\n}\n\nfn code(c: Color) -> int {\n    match c {\n        Color.Red => 1,\n        Color.Green => 2,\n        _ => 3,\n    }\n}\n",
        );

        let names: Vec<&str> = program
            .globals
            .iter()
            .map(|global| global.name.as_str())
            .collect();
        assert_eq!(names, vec!["Color_Red", "Color_Green", "Color_Blue"]);
        assert!(
            matches!(
                program.globals[0].init_value,
                Some(SirExpr::Literal(SirLiteral::Int(0)))
            ),
            "Red should be 0: {:?}",
            program.globals[0]
        );

        let function = program
            .functions
            .iter()
            .find(|function| function.name == "code")
            .expect("code should be lowered");
        let lowered = format!("{:?}", function.body);
        assert!(
            lowered.contains("Color_Red") && lowered.contains("Color_Green"),
            "match should compare against the variant constants: {lowered}"
        );
    }

    #[test]
    fn array_literals_and_indexing_are_lowered() {
        let program = lower(
            "fn main() {\n    let numbers: Array = [5, 2, 8];\n    print(numbers[1]);\n    numbers[1] = 9;\n}\n",
        );
        let body = &program.functions[0].body;
        assert!(
            matches!(&body[0], SirStatement::Let { typ: Some(typ), value, .. }
                if typ == "Array" && matches!(value, SirExpr::Typed { expr, .. } if matches!(**expr, SirExpr::Array(_)))),
            "expected an array literal let, got {:?}",
            body[0]
        );
        match &body[1] {
            SirStatement::Expr(SirExpr::Call { args, .. }) => assert!(
                matches!(&args[0], SirExpr::Typed { typ, expr } if typ == "int" && matches!(**expr, SirExpr::Index { .. })),
                "expected a typed index read, got {:?}",
                args[0]
            ),
            other => panic!("expected a print call, got {other:?}"),
        }
        assert!(
            matches!(&body[2], SirStatement::Expr(SirExpr::IndexAssign { .. })),
            "expected an index assignment, got {:?}",
            body[2]
        );
    }

    #[test]
    fn struct_literals_and_field_access_are_lowered() {
        let program = lower(
            "struct Point {\n    x: int,\n    y: int,\n}\n\nfn main() {\n    let p = Point { x: 3, y: 4 };\n    print(p.x);\n}\n",
        );
        assert_eq!(program.structs.len(), 1);
        assert_eq!(program.structs[0].name, "Point");
        assert_eq!(program.structs[0].fields.len(), 2);

        let main = program.functions.last().expect("main should be lowered");
        match &main.body[0] {
            SirStatement::Let {
                typ: Some(declared),
                value,
                ..
            } => {
                assert_eq!(declared, "Point");
                assert!(
                    matches!(value, SirExpr::Typed { typ, .. } if typ == "Point"),
                    "the struct literal should carry its struct type: {value:?}"
                );
            }
            other => panic!("unexpected statement: {other:?}"),
        }
        match &main.body[1] {
            SirStatement::Expr(SirExpr::Call { args, .. }) => assert!(
                matches!(&args[0], SirExpr::Typed { typ, .. } if typ == "int"),
                "a field access should carry the field's declared type: {:?}",
                args[0]
            ),
            other => panic!("unexpected statement: {other:?}"),
        }
    }
}
