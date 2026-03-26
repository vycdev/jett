use std::collections::{HashMap, HashSet};

use jett_common::Span;
use jett_diagnostics::{Diagnostic, DiagnosticSink};
use jett_parser::ast::{
    self, BinOp, Block, Expr, FunctionDef, Item, Module, Stmt, StringPart, TypeExpr, UnaryOp,
    VerifyBlock,
};
use jett_resolve::resolver::ResolveResult;
use jett_resolve::scope::{DefId, DefKind};
use jett_types::{
    EnumDef as TypeEnumDef, FunctionSig, InterfaceDef as TypeInterfaceDef,
    StructDef as TypeStructDef, StructId, Type, TypeId, TypeInterner, VariantDef,
};

use crate::capability;
use crate::errors;

/// The result of type checking.
#[derive(Debug)]
pub struct CheckResult {
    /// Diagnostics (errors and warnings) emitted during type checking.
    pub diagnostics: Vec<Diagnostic>,
    /// Map from expression spans to their inferred type.
    pub type_map: HashMap<Span, TypeId>,
    /// The type interner, containing all types encountered during checking.
    pub interner: TypeInterner,
}

/// Type-check a resolved module.
pub fn check(module: &Module, resolve: &ResolveResult) -> CheckResult {
    let mut checker = TypeChecker::new(resolve);
    checker.check_module(module);

    // Run ownership analysis (linear type checking) after type checking.
    let ownership_diagnostics = crate::ownership::check_ownership(module, &checker.interner);

    let mut diagnostics = checker.sink.into_diagnostics();
    diagnostics.extend(ownership_diagnostics);

    CheckResult {
        diagnostics,
        type_map: checker.type_map,
        interner: checker.interner,
    }
}

// ---------------------------------------------------------------------------
// Internal type checker
// ---------------------------------------------------------------------------

struct TypeChecker<'a> {
    interner: TypeInterner,
    resolve: &'a ResolveResult,
    sink: DiagnosticSink,
    /// DefId → TypeId for variables, parameters, and functions.
    type_env: HashMap<DefId, TypeId>,
    /// Declaration span → DefId for locally declared names.
    decl_defs: HashMap<Span, DefId>,
    /// User-defined type name → TypeId.
    named_types: HashMap<String, TypeId>,
    /// Expression span → TypeId (the output type map).
    type_map: HashMap<Span, TypeId>,
    /// The expected return type for the function currently being checked.
    current_return_type: Option<TypeId>,
    /// (interface, concrete type) -> implemented method signatures.
    interface_impls: HashMap<(TypeId, TypeId), HashMap<String, FunctionSig>>,
    /// concrete type -> all methods contributed by implement blocks.
    impl_methods_by_type: HashMap<TypeId, HashMap<String, FunctionSig>>,

    // -- Capability / purity tracking --
    /// Function name → is_pure.  Built during the first pass over the module.
    purity_map: HashMap<String, bool>,
    /// Name of the function currently being type-checked (None outside functions).
    current_function_name: Option<String>,
    /// Whether the function currently being type-checked is pure.
    current_function_pure: bool,
    /// Whether we are inside a verify block.
    in_verify_block: bool,
    /// The name of the verify block currently being checked (for error messages).
    current_verify_name: Option<String>,
    /// Nesting depth inside a handle-block body. Used to validate `default`.
    handle_body_depth: usize,
}

impl<'a> TypeChecker<'a> {
    fn new(resolve: &'a ResolveResult) -> Self {
        let decl_defs = resolve
            .scope_table
            .definitions
            .iter()
            .map(|def| (def.span, def.id))
            .collect();

        Self {
            interner: TypeInterner::new(),
            resolve,
            sink: DiagnosticSink::new(),
            type_env: HashMap::new(),
            decl_defs,
            named_types: HashMap::new(),
            type_map: HashMap::new(),
            current_return_type: None,
            interface_impls: HashMap::new(),
            impl_methods_by_type: HashMap::new(),
            purity_map: HashMap::new(),
            current_function_name: None,
            current_function_pure: false,
            in_verify_block: false,
            current_verify_name: None,
            handle_body_depth: 0,
        }
    }

    // ------------------------------------------------------------------
    // Utility: human-readable type name
    // ------------------------------------------------------------------

    fn type_name(&self, id: TypeId) -> String {
        match self.interner.resolve(id) {
            Type::Int8 => "int8".to_string(),
            Type::Int16 => "int16".to_string(),
            Type::Int32 => "int32".to_string(),
            Type::Int64 => "int64".to_string(),
            Type::Uint8 => "uint8".to_string(),
            Type::Uint16 => "uint16".to_string(),
            Type::Uint32 => "uint32".to_string(),
            Type::Uint64 => "uint64".to_string(),
            Type::Float32 => "float32".to_string(),
            Type::Float64 => "float64".to_string(),
            Type::String => "string".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Bytes => "bytes".to_string(),
            Type::Nothing => "nothing".to_string(),
            Type::List(inner) => format!("list[{}]", self.type_name(*inner)),
            Type::Map(k, v) => format!("map[{}, {}]", self.type_name(*k), self.type_name(*v)),
            Type::Set(inner) => format!("set[{}]", self.type_name(*inner)),
            Type::Optional(inner) => format!("optional[{}]", self.type_name(*inner)),
            Type::Result(ok, err) => {
                format!("result[{}, {}]", self.type_name(*ok), self.type_name(*err))
            }
            Type::Secret(inner) => format!("secret[{}]", self.type_name(*inner)),
            Type::Struct(sid) => self.interner.resolve_struct(*sid).name.clone(),
            Type::Enum(eid) => self.interner.resolve_enum(*eid).name.clone(),
            Type::Interface(iid) => self.interner.resolve_interface(*iid).name.clone(),
            Type::Function {
                params,
                return_type,
            } => {
                let params: Vec<String> = params.iter().map(|p| self.type_name(*p)).collect();
                format!(
                    "function({}) returns {}",
                    params.join(", "),
                    self.type_name(*return_type)
                )
            }
            Type::Refinement { name, .. } => name.clone(),
            Type::Error => "<error>".to_string(),
        }
    }

    fn declaration_def_id(&self, span: Span) -> Option<DefId> {
        self.resolve
            .resolutions
            .get(&span)
            .copied()
            .or_else(|| self.decl_defs.get(&span).copied())
    }

    fn ident_def_kind(&self, ident: &ast::Ident) -> Option<DefKind> {
        let def_id = self
            .resolve
            .resolutions
            .get(&ident.span)
            .copied()
            .or_else(|| self.decl_defs.get(&ident.span).copied())?;
        Some(self.resolve.scope_table.def(def_id).kind)
    }

    fn is_struct_type_name_expr(&self, expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::Ident(ident) if self.ident_def_kind(ident) == Some(DefKind::Struct)
        )
    }

    /// Returns true if the type is numeric (any integer or float type).
    fn is_numeric(&self, id: TypeId) -> bool {
        matches!(
            self.interner.resolve(id),
            Type::Int8
                | Type::Int16
                | Type::Int32
                | Type::Int64
                | Type::Uint8
                | Type::Uint16
                | Type::Uint32
                | Type::Uint64
                | Type::Float32
                | Type::Float64
        )
    }

    fn is_secret_type(&self, id: TypeId) -> bool {
        matches!(self.interner.resolve(id), Type::Secret(_))
    }

    fn secret_inner_type(&self, id: TypeId) -> Option<TypeId> {
        match self.interner.resolve(id) {
            Type::Secret(inner) => Some(*inner),
            _ => None,
        }
    }

    fn is_secret_output_boundary(name: &str) -> bool {
        matches!(name, "Stdout.write")
    }

    fn types_compatible(&self, expected: TypeId, got: TypeId) -> bool {
        if expected == got || expected == TypeInterner::ERROR || got == TypeInterner::ERROR {
            return true;
        }

        match (self.interner.resolve(expected), self.interner.resolve(got)) {
            (Type::Interface(_), Type::Interface(_)) => expected == got,
            (Type::Interface(_), _) => self.interface_impls.contains_key(&(expected, got)),
            (Type::Secret(expected_inner), Type::Secret(got_inner)) => {
                self.types_compatible(*expected_inner, *got_inner)
            }
            (Type::Secret(expected_inner), _) => self.types_compatible(*expected_inner, got),
            (Type::List(expected_inner), Type::List(got_inner))
            | (Type::Optional(expected_inner), Type::Optional(got_inner)) => {
                self.types_compatible(*expected_inner, *got_inner)
            }
            (Type::Set(expected_inner), Type::Set(got_inner)) => {
                self.types_compatible(*expected_inner, *got_inner)
            }
            (Type::Map(expected_key, expected_val), Type::Map(got_key, got_val))
            | (Type::Result(expected_key, expected_val), Type::Result(got_key, got_val)) => {
                self.types_compatible(*expected_key, *got_key)
                    && self.types_compatible(*expected_val, *got_val)
            }
            _ => false,
        }
    }

    fn extract_dotted_name(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(ident) => Some(ident.name.clone()),
            Expr::FieldAccess(inner, field, _) => {
                let prefix = Self::extract_dotted_name(inner)?;
                Some(format!("{prefix}.{}", field.name))
            }
            _ => None,
        }
    }

    fn builtin_signature(
        &mut self,
        callee: &Expr,
        type_args: &[TypeExpr],
        span: Span,
    ) -> Option<(Vec<TypeId>, TypeId)> {
        let name = Self::extract_dotted_name(callee)?;
        match name.as_str() {
            "int64.from_string" => Some((
                vec![TypeInterner::STRING],
                self.interner
                    .intern(Type::Result(TypeInterner::INT64, TypeInterner::STRING)),
            )),
            "string.from_int64" => Some((vec![TypeInterner::INT64], TypeInterner::STRING)),
            "string.from_float64" => Some((vec![TypeInterner::FLOAT64], TypeInterner::STRING)),
            "Environment.args" => Some((
                vec![TypeInterner::ERROR],
                self.interner.intern(Type::List(TypeInterner::STRING)),
            )),
            "Stdout.write" => Some((
                vec![TypeInterner::ERROR, TypeInterner::STRING],
                TypeInterner::NOTHING,
            )),
            "list.new" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "list.new (expected 1 type argument, got {})",
                            type_args.len()
                        ),
                        span,
                    ));
                    return Some((vec![], TypeInterner::ERROR));
                }
                let inner = self.resolve_type_expr(&type_args[0]);
                Some((vec![], self.interner.intern(Type::List(inner))))
            }
            "list.append" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "list.append (expected 1 type argument, got {})",
                            type_args.len()
                        ),
                        span,
                    ));
                    return Some((
                        vec![TypeInterner::ERROR, TypeInterner::ERROR],
                        TypeInterner::ERROR,
                    ));
                }
                let inner = self.resolve_type_expr(&type_args[0]);
                let list_ty = self.interner.intern(Type::List(inner));
                Some((vec![list_ty, inner], list_ty))
            }
            "list.length" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "list.length (expected 1 type argument, got {})",
                            type_args.len()
                        ),
                        span,
                    ));
                    return Some((vec![TypeInterner::ERROR], TypeInterner::ERROR));
                }
                let inner = self.resolve_type_expr(&type_args[0]);
                Some((
                    vec![self.interner.intern(Type::List(inner))],
                    TypeInterner::INT64,
                ))
            }
            "list.get" => {
                if type_args.len() != 1 {
                    self.sink.emit(errors::unknown_type(
                        &format!(
                            "list.get (expected 1 type argument, got {})",
                            type_args.len()
                        ),
                        span,
                    ));
                    return Some((
                        vec![TypeInterner::ERROR, TypeInterner::INT64],
                        TypeInterner::ERROR,
                    ));
                }
                let inner = self.resolve_type_expr(&type_args[0]);
                Some((
                    vec![self.interner.intern(Type::List(inner)), TypeInterner::INT64],
                    self.interner.intern(Type::Optional(inner)),
                ))
            }
            _ => None,
        }
    }

    // ------------------------------------------------------------------
    // Module
    // ------------------------------------------------------------------

    fn check_module(&mut self, module: &Module) {
        // First pass: predeclare all user-defined types so function signatures,
        // fields, and methods can refer to them by name.
        for item in &module.items {
            match item {
                Item::Interface(def) => self.predeclare_interface(def),
                Item::Struct(def) => self.predeclare_struct(def),
                Item::Enum(def) => self.predeclare_enum(def),
                _ => {}
            }
        }

        // Second pass: fill in the struct/enum contents now that all names exist.
        for item in &module.items {
            match item {
                Item::Interface(def) => self.finish_interface(def),
                Item::Struct(def) => self.finish_struct(def),
                Item::Enum(def) => self.finish_enum(def),
                _ => {}
            }
        }

        // Third pass: register all top-level function signatures into the type env
        // and build the purity map.
        for item in &module.items {
            match item {
                Item::Mutual(block) => {
                    for decl in &block.declarations {
                        self.register_function_decl_sig(decl);
                        let is_pure = Self::params_are_pure(&decl.params);
                        self.purity_map.insert(decl.name.name.clone(), is_pure);
                    }
                }
                Item::Function(func) => {
                    self.register_function_sig(func);
                    let is_pure = Self::function_is_pure(func);
                    self.purity_map.insert(func.name.name.clone(), is_pure);
                }
                Item::Interface(def) => {
                    for method in &def.methods {
                        let dotted = format!("{}.{}", def.name.name, method.name.name);
                        let is_pure = Self::params_are_pure(&method.params);
                        self.purity_map.insert(dotted, is_pure);
                    }
                }
                Item::Implement(block) => self.register_implement_block(block),
                Item::Struct(def) => {
                    for method in &def.methods {
                        let dotted = format!("{}.{}", def.name.name, method.name.name);
                        let is_pure = Self::function_is_pure(method);
                        self.purity_map.insert(dotted, is_pure);
                    }
                }
                _ => {}
            }
        }

        self.validate_mutual_blocks(module);
        self.validate_implement_blocks(module);

        // Fourth pass: type-check function bodies, methods, and verify blocks.
        for item in &module.items {
            match item {
                Item::Function(func) => self.check_function(func),
                Item::Implement(block) => self.check_implement_block(block),
                Item::Struct(def) => {
                    for method in &def.methods {
                        self.check_method(&def.name.name, method);
                    }
                }
                Item::VarDecl(decl) => self.check_var_decl(decl),
                Item::Verify(verify) => self.check_verify_block(verify),
                _ => {}
            }
        }
    }

    /// Returns true if a function has no capability-type parameters (i.e. is pure).
    fn function_is_pure(func: &FunctionDef) -> bool {
        Self::params_are_pure(&func.params)
    }

    fn params_are_pure(params: &[ast::Param]) -> bool {
        !params
            .iter()
            .any(|p| capability::type_expr_is_capability(&p.ty))
    }

    fn predeclare_struct(&mut self, def: &ast::StructDef) {
        let sid = self.interner.add_struct(TypeStructDef {
            name: def.name.name.clone(),
            fields: Vec::new(),
            methods: Vec::new(),
        });
        let ty = self.interner.intern(Type::Struct(sid));
        self.named_types.insert(def.name.name.clone(), ty);

        if let Some(def_id) = self.declaration_def_id(def.name.span) {
            self.type_env.insert(def_id, ty);
        }
    }

    fn predeclare_interface(&mut self, def: &ast::InterfaceDecl) {
        let iid = self.interner.add_interface(TypeInterfaceDef {
            name: def.name.name.clone(),
            methods: Vec::new(),
        });
        let ty = self.interner.intern(Type::Interface(iid));
        self.named_types.insert(def.name.name.clone(), ty);

        if let Some(def_id) = self.declaration_def_id(def.name.span) {
            self.type_env.insert(def_id, ty);
        }
    }

    fn finish_struct(&mut self, def: &ast::StructDef) {
        let Some(&ty) = self.named_types.get(&def.name.name) else {
            return;
        };
        let Type::Struct(sid) = *self.interner.resolve(ty) else {
            return;
        };

        let fields = def
            .fields
            .iter()
            .map(|field| (field.name.name.clone(), self.resolve_type_expr(&field.ty)))
            .collect();
        let methods = def
            .methods
            .iter()
            .map(|method| self.method_signature(method))
            .collect();

        self.interner.update_struct(
            sid,
            TypeStructDef {
                name: def.name.name.clone(),
                fields,
                methods,
            },
        );
    }

    fn finish_interface(&mut self, def: &ast::InterfaceDecl) {
        let Some(&ty) = self.named_types.get(&def.name.name) else {
            return;
        };
        let Type::Interface(iid) = *self.interner.resolve(ty) else {
            return;
        };

        let methods = def
            .methods
            .iter()
            .map(|method| self.function_decl_method_signature(method))
            .collect();

        self.interner.update_interface(
            iid,
            TypeInterfaceDef {
                name: def.name.name.clone(),
                methods,
            },
        );
    }

    fn predeclare_enum(&mut self, def: &ast::EnumDef) {
        let eid = self.interner.add_enum(TypeEnumDef {
            name: def.name.name.clone(),
            variants: Vec::new(),
        });
        let ty = self.interner.intern(Type::Enum(eid));
        self.named_types.insert(def.name.name.clone(), ty);

        if let Some(def_id) = self.declaration_def_id(def.name.span) {
            self.type_env.insert(def_id, ty);
        }
    }

    fn finish_enum(&mut self, def: &ast::EnumDef) {
        let Some(&ty) = self.named_types.get(&def.name.name) else {
            return;
        };
        let Type::Enum(eid) = *self.interner.resolve(ty) else {
            return;
        };

        let variants = def
            .variants
            .iter()
            .map(|variant| VariantDef {
                name: variant.name.name.clone(),
                fields: variant
                    .fields
                    .iter()
                    .map(|field| (field.name.name.clone(), self.resolve_type_expr(&field.ty)))
                    .collect(),
            })
            .collect();

        self.interner.update_enum(
            eid,
            TypeEnumDef {
                name: def.name.name.clone(),
                variants,
            },
        );
    }

    fn method_signature(&mut self, func: &FunctionDef) -> FunctionSig {
        let params = func
            .params
            .iter()
            .map(|param| {
                (
                    param.name.name.clone(),
                    self.resolve_type_expr(&param.ty),
                    param.view,
                )
            })
            .collect();
        let return_type = func
            .return_type
            .as_ref()
            .map(|ty| self.resolve_type_expr(ty))
            .unwrap_or(TypeInterner::NOTHING);

        FunctionSig {
            name: func.name.name.clone(),
            params,
            return_type,
            is_pure: Self::function_is_pure(func),
        }
    }

    fn function_decl_method_signature(&mut self, decl: &ast::FunctionDecl) -> FunctionSig {
        let params = decl
            .params
            .iter()
            .map(|param| {
                (
                    param.name.name.clone(),
                    self.resolve_type_expr(&param.ty),
                    param.view,
                )
            })
            .collect();
        let return_type = decl
            .return_type
            .as_ref()
            .map(|ty| self.resolve_type_expr(ty))
            .unwrap_or(TypeInterner::NOTHING);

        FunctionSig {
            name: decl.name.name.clone(),
            params,
            return_type,
            is_pure: Self::params_are_pure(&decl.params),
        }
    }

    fn function_decl_signature(&mut self, decl: &ast::FunctionDecl) -> (Vec<TypeId>, TypeId) {
        let params = decl
            .params
            .iter()
            .map(|p| self.resolve_type_expr(&p.ty))
            .collect();
        let return_type = decl
            .return_type
            .as_ref()
            .map(|t| self.resolve_type_expr(t))
            .unwrap_or(TypeInterner::NOTHING);
        (params, return_type)
    }

    // ------------------------------------------------------------------
    // Function registration (builds FunctionType + binds to DefId)
    // ------------------------------------------------------------------

    fn register_function_decl_sig(&mut self, decl: &ast::FunctionDecl) {
        let (param_types, return_type) = self.function_decl_signature(decl);
        let fn_type = self.interner.intern(Type::Function {
            params: param_types,
            return_type,
        });

        if let Some(def_id) = self.declaration_def_id(decl.name.span) {
            self.type_env.insert(def_id, fn_type);
        }
    }

    fn register_function_sig(&mut self, func: &FunctionDef) {
        let param_types: Vec<TypeId> = func
            .params
            .iter()
            .map(|p| self.resolve_type_expr(&p.ty))
            .collect();

        let return_type = func
            .return_type
            .as_ref()
            .map(|t| self.resolve_type_expr(t))
            .unwrap_or(TypeInterner::NOTHING);

        let fn_type = self.interner.intern(Type::Function {
            params: param_types,
            return_type,
        });

        // Bind the function name's DefId to this function type.
        if let Some(def_id) = self.declaration_def_id(func.name.span) {
            self.type_env.insert(def_id, fn_type);
        }
    }

    fn register_implement_block(&mut self, block: &ast::ImplementBlock) {
        let interface_ty = self.resolve_type_expr(&TypeExpr::Named(block.interface_name.clone()));
        let owner_ty = self.resolve_type_expr(&block.for_type);
        if interface_ty == TypeInterner::ERROR || owner_ty == TypeInterner::ERROR {
            return;
        }

        let owner_name = self.type_name(owner_ty);
        let interface_name = self.type_name(interface_ty);
        let method_sigs: Vec<_> = block
            .methods
            .iter()
            .map(|method| {
                let sig = self.method_signature(method);
                (method.name.name.clone(), sig)
            })
            .collect();

        let impl_methods = self.impl_methods_by_type.entry(owner_ty).or_default();
        let interface_methods = self
            .interface_impls
            .entry((interface_ty, owner_ty))
            .or_default();

        for (method_name, sig) in method_sigs {
            self.purity_map
                .insert(format!("{owner_name}.{method_name}"), sig.is_pure);
            self.purity_map
                .insert(format!("{interface_name}.{method_name}"), sig.is_pure);
            impl_methods.insert(method_name.clone(), sig.clone());
            interface_methods.insert(method_name, sig);
        }
    }

    fn validate_implement_blocks(&mut self, module: &Module) {
        for item in &module.items {
            let Item::Implement(block) = item else {
                continue;
            };

            let interface_ty =
                self.resolve_type_expr(&TypeExpr::Named(block.interface_name.clone()));
            let owner_ty = self.resolve_type_expr(&block.for_type);
            if interface_ty == TypeInterner::ERROR || owner_ty == TypeInterner::ERROR {
                continue;
            }

            let Type::Interface(iid) = *self.interner.resolve(interface_ty) else {
                self.sink.emit(errors::expected_interface(
                    &block.interface_name.name,
                    block.interface_name.span,
                ));
                continue;
            };
            let interface_def = self.interner.resolve_interface(iid).clone();

            let Some(impl_methods) = self.interface_impls.get(&(interface_ty, owner_ty)).cloned()
            else {
                continue;
            };

            let mut seen = HashSet::new();
            for method in &block.methods {
                if !seen.insert(method.name.name.clone()) {
                    self.sink.emit(errors::duplicate_implemented_method(
                        &self.type_name(owner_ty),
                        &method.name.name,
                        method.name.span,
                    ));
                    continue;
                }

                let Some(interface_method) = interface_def
                    .methods
                    .iter()
                    .find(|candidate| candidate.name == method.name.name)
                    .cloned()
                else {
                    self.sink.emit(errors::interface_has_no_member(
                        &interface_def.name,
                        &method.name.name,
                        method.name.span,
                    ));
                    continue;
                };

                let impl_sig = impl_methods
                    .get(&method.name.name)
                    .expect("impl method must exist");
                if !self.implementation_matches_interface(owner_ty, impl_sig, &interface_method) {
                    self.sink
                        .emit(errors::implemented_method_signature_mismatch(
                            &interface_def.name,
                            &self.type_name(owner_ty),
                            &method.name.name,
                            method.name.span,
                        ));
                }
            }

            for interface_method in &interface_def.methods {
                if !seen.contains(&interface_method.name) {
                    self.sink.emit(errors::missing_implemented_method(
                        &interface_def.name,
                        &self.type_name(owner_ty),
                        &interface_method.name,
                        block.span,
                    ));
                }
            }
        }
    }

    fn implementation_matches_interface(
        &mut self,
        owner_ty: TypeId,
        impl_sig: &FunctionSig,
        interface_sig: &FunctionSig,
    ) -> bool {
        if impl_sig.params.len() != interface_sig.params.len() {
            return false;
        }

        for (index, (impl_param, interface_param)) in impl_sig
            .params
            .iter()
            .zip(interface_sig.params.iter())
            .enumerate()
        {
            if impl_param.0 != interface_param.0 || impl_param.2 != interface_param.2 {
                return false;
            }

            let expected_ty = if index == 0 {
                match self.interner.resolve(interface_param.1) {
                    Type::Interface(_) => owner_ty,
                    _ => interface_param.1,
                }
            } else {
                interface_param.1
            };

            if !self.types_compatible(expected_ty, impl_param.1)
                || !self.types_compatible(impl_param.1, expected_ty)
            {
                return false;
            }
        }

        self.types_compatible(interface_sig.return_type, impl_sig.return_type)
            && self.types_compatible(impl_sig.return_type, interface_sig.return_type)
    }

    fn validate_mutual_blocks(&mut self, module: &Module) {
        let function_defs: HashMap<&str, &FunctionDef> = module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Function(func) => Some((func.name.name.as_str(), func)),
                _ => None,
            })
            .collect();

        for item in &module.items {
            let Item::Mutual(block) = item else {
                continue;
            };

            for decl in &block.declarations {
                let Some(func) = function_defs.get(decl.name.name.as_str()).copied() else {
                    self.sink.emit(errors::mutual_function_missing_definition(
                        &decl.name.name,
                        decl.name.span,
                    ));
                    continue;
                };

                if !self.function_matches_decl(func, decl) {
                    self.sink.emit(errors::mutual_signature_mismatch(
                        &decl.name.name,
                        func.name.span,
                    ));
                }
            }
        }
    }

    fn function_matches_decl(&mut self, func: &FunctionDef, decl: &ast::FunctionDecl) -> bool {
        if func.params.len() != decl.params.len() {
            return false;
        }

        for (func_param, decl_param) in func.params.iter().zip(decl.params.iter()) {
            if func_param.name.name != decl_param.name.name
                || func_param.view != decl_param.view
                || func_param.mutable != decl_param.mutable
            {
                return false;
            }

            let func_ty = self.resolve_type_expr(&func_param.ty);
            let decl_ty = self.resolve_type_expr(&decl_param.ty);
            if !self.types_compatible(decl_ty, func_ty) || !self.types_compatible(func_ty, decl_ty)
            {
                return false;
            }
        }

        let func_return = func
            .return_type
            .as_ref()
            .map(|ty| self.resolve_type_expr(ty))
            .unwrap_or(TypeInterner::NOTHING);
        let decl_return = decl
            .return_type
            .as_ref()
            .map(|ty| self.resolve_type_expr(ty))
            .unwrap_or(TypeInterner::NOTHING);

        self.types_compatible(decl_return, func_return)
            && self.types_compatible(func_return, decl_return)
    }

    // ------------------------------------------------------------------
    // Function body
    // ------------------------------------------------------------------

    fn check_function(&mut self, func: &FunctionDef) {
        self.check_function_impl(func, func.name.name.clone());
    }

    fn check_method(&mut self, owner: &str, func: &FunctionDef) {
        self.check_function_impl(func, format!("{owner}.{}", func.name.name));
    }

    fn check_implement_block(&mut self, block: &ast::ImplementBlock) {
        let owner_ty = self.resolve_type_expr(&block.for_type);
        let owner_name = self.type_name(owner_ty);
        for method in &block.methods {
            self.check_method(&owner_name, method);
        }
    }

    fn check_function_impl(&mut self, func: &FunctionDef, function_name: String) {
        let return_type = func
            .return_type
            .as_ref()
            .map(|t| self.resolve_type_expr(t))
            .unwrap_or(TypeInterner::NOTHING);

        self.current_return_type = Some(return_type);

        // Set the purity context for this function.
        let is_pure = Self::function_is_pure(func);
        self.current_function_name = Some(function_name);
        self.current_function_pure = is_pure;

        // Bind parameter types into the type environment.
        for param in &func.params {
            let param_type = self.resolve_type_expr(&param.ty);
            if let Some(def_id) = self.declaration_def_id(param.name.span) {
                self.type_env.insert(def_id, param_type);
            }
        }

        self.check_block(&func.body);

        self.current_return_type = None;
        self.current_function_name = None;
        self.current_function_pure = false;
    }

    fn check_verify_block(&mut self, verify: &VerifyBlock) {
        self.in_verify_block = true;
        self.current_verify_name = Some(verify.name.name.clone());
        self.check_block(&verify.body);
        self.in_verify_block = false;
        self.current_verify_name = None;
    }

    // ------------------------------------------------------------------
    // Block
    // ------------------------------------------------------------------

    fn check_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
    }

    // ------------------------------------------------------------------
    // Statements
    // ------------------------------------------------------------------

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl(decl) => self.check_var_decl(decl),
            Stmt::Assign(assign) => self.check_assign(assign),
            Stmt::Return(ret) => self.check_return(ret),
            Stmt::If(if_stmt) => self.check_if(if_stmt),
            Stmt::For(for_stmt) => self.check_for(for_stmt),
            Stmt::While(while_stmt) => self.check_while(while_stmt),
            Stmt::Expr(expr_stmt) => {
                self.check_expr(&expr_stmt.expr);
            }
            Stmt::Assert(assert_stmt) => self.check_assert(assert_stmt),
            Stmt::Match(match_stmt) => self.check_match(match_stmt),
            Stmt::Use(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn check_var_decl(&mut self, decl: &ast::VarDecl) {
        let declared_type = self.resolve_type_expr(&decl.ty);
        let init_type = self.check_expr(&decl.value);

        // Bind the variable's DefId to its declared type.
        if let Some(def_id) = self.declaration_def_id(decl.name.span) {
            self.type_env.insert(def_id, declared_type);
        }

        // Check that the initializer type matches the declared type (skip if Error).
        if !self.types_compatible(declared_type, init_type) {
            self.sink.emit(errors::var_decl_type_mismatch(
                &decl.name.name,
                &self.type_name(declared_type),
                &self.type_name(init_type),
                decl.span,
            ));
        }
    }

    fn check_assign(&mut self, assign: &ast::AssignStmt) {
        let target_type = self.check_expr(&assign.target);
        let value_type = self.check_expr(&assign.value);

        if !self.types_compatible(target_type, value_type) {
            self.sink.emit(errors::assign_type_mismatch(
                &self.type_name(target_type),
                &self.type_name(value_type),
                assign.span,
            ));
        }
    }

    fn check_return(&mut self, ret: &ast::ReturnStmt) {
        let ret_type = match &ret.value {
            Some(expr) => self.check_expr(expr),
            None => TypeInterner::NOTHING,
        };

        if let Some(expected) = self.current_return_type {
            if !self.types_compatible(expected, ret_type) {
                self.sink.emit(errors::return_type_mismatch(
                    &self.type_name(expected),
                    &self.type_name(ret_type),
                    ret.span,
                ));
            }
        }
    }

    fn check_if(&mut self, if_stmt: &ast::IfStmt) {
        let cond_type = self.check_expr(&if_stmt.condition);
        if cond_type != TypeInterner::ERROR && cond_type != TypeInterner::BOOL {
            self.sink.emit(errors::condition_not_bool(
                &self.type_name(cond_type),
                if_stmt.condition.span(),
            ));
        }

        self.check_block(&if_stmt.then_block);

        for (else_if_cond, else_if_block) in &if_stmt.else_ifs {
            let ei_type = self.check_expr(else_if_cond);
            if ei_type != TypeInterner::ERROR && ei_type != TypeInterner::BOOL {
                self.sink.emit(errors::condition_not_bool(
                    &self.type_name(ei_type),
                    else_if_cond.span(),
                ));
            }
            self.check_block(else_if_block);
        }

        if let Some(else_block) = &if_stmt.else_block {
            self.check_block(else_block);
        }
    }

    fn check_for(&mut self, for_stmt: &ast::ForStmt) {
        let iterable_type = self.check_expr(&for_stmt.iterable);

        // The iterable must be list[T]; the loop variable gets type T.
        let elem_type = if iterable_type == TypeInterner::ERROR {
            TypeInterner::ERROR
        } else if let Type::List(inner) = self.interner.resolve(iterable_type) {
            *inner
        } else {
            self.sink.emit(errors::not_iterable(
                &self.type_name(iterable_type),
                for_stmt.iterable.span(),
            ));
            TypeInterner::ERROR
        };

        // Bind the loop variable.
        if let Some(def_id) = self.declaration_def_id(for_stmt.variable.span) {
            self.type_env.insert(def_id, elem_type);
        }

        self.check_block(&for_stmt.body);
    }

    fn check_while(&mut self, while_stmt: &ast::WhileStmt) {
        let cond_type = self.check_expr(&while_stmt.condition);
        if cond_type != TypeInterner::ERROR && cond_type != TypeInterner::BOOL {
            self.sink.emit(errors::condition_not_bool(
                &self.type_name(cond_type),
                while_stmt.condition.span(),
            ));
        }
        self.check_block(&while_stmt.body);
    }

    fn check_assert(&mut self, assert_stmt: &ast::AssertStmt) {
        let cond_type = self.check_expr(&assert_stmt.condition);
        if cond_type != TypeInterner::ERROR && cond_type != TypeInterner::BOOL {
            self.sink.emit(errors::assert_condition_not_bool(
                &self.type_name(cond_type),
                assert_stmt.condition.span(),
            ));
        }
        if let Some(msg) = &assert_stmt.message {
            self.check_expr(msg);
        }
    }

    fn check_match(&mut self, match_stmt: &ast::MatchStmt) {
        let expr_ty = self.check_expr(&match_stmt.expr);
        if expr_ty == TypeInterner::ERROR {
            for arm in &match_stmt.arms {
                self.check_block(&arm.body);
            }
            return;
        }

        let Type::Enum(enum_id) = *self.interner.resolve(expr_ty) else {
            self.sink.emit(errors::match_requires_enum(
                &self.type_name(expr_ty),
                match_stmt.expr.span(),
            ));
            for arm in &match_stmt.arms {
                self.check_block(&arm.body);
            }
            return;
        };

        let enum_def = self.interner.resolve_enum(enum_id).clone();
        let mut covered = HashSet::new();
        let mut has_other = false;

        for arm in &match_stmt.arms {
            match &arm.pattern {
                ast::Pattern::Ident(name) => {
                    if enum_def
                        .variants
                        .iter()
                        .any(|variant| variant.name == name.name)
                    {
                        covered.insert(name.name.clone());
                    } else {
                        self.sink.emit(errors::type_has_no_member(
                            &enum_def.name,
                            &name.name,
                            name.span,
                        ));
                    }
                }
                ast::Pattern::Variant(name, bindings) => {
                    if let Some(variant) = enum_def
                        .variants
                        .iter()
                        .find(|variant| variant.name == name.name)
                    {
                        covered.insert(name.name.clone());
                        if bindings.len() != variant.fields.len() {
                            self.sink.emit(errors::variant_binding_count_mismatch(
                                &name.name,
                                variant.fields.len(),
                                bindings.len(),
                                name.span,
                            ));
                        }

                        for (binding, (_, field_ty)) in bindings.iter().zip(variant.fields.iter()) {
                            if let Some(def_id) = self.declaration_def_id(binding.span) {
                                self.type_env.insert(def_id, *field_ty);
                            }
                        }
                    } else {
                        self.sink.emit(errors::type_has_no_member(
                            &enum_def.name,
                            &name.name,
                            name.span,
                        ));
                    }
                }
                ast::Pattern::Other(_) => {
                    has_other = true;
                }
            }

            self.check_block(&arm.body);
        }

        if !has_other {
            for variant in &enum_def.variants {
                if !covered.contains(&variant.name) {
                    self.sink.emit(errors::non_exhaustive_match(
                        &enum_def.name,
                        &variant.name,
                        match_stmt.span,
                    ));
                    break;
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    fn check_expr(&mut self, expr: &Expr) -> TypeId {
        let ty = match expr {
            Expr::IntLiteral(_, _) => TypeInterner::INT64,
            Expr::FloatLiteral(_, _) => TypeInterner::FLOAT64,
            Expr::StringLiteral(_, _) => TypeInterner::STRING,
            Expr::BoolLiteral(_, _) => TypeInterner::BOOL,
            Expr::Nothing(_) => TypeInterner::NOTHING,

            Expr::Ident(ident) => self.check_ident(ident),
            Expr::Binary(lhs, op, rhs, span) => self.check_binary(lhs, *op, rhs, *span),
            Expr::Unary(op, operand, span) => self.check_unary(*op, operand, *span),
            Expr::Call(callee, args, span) => self.check_call(callee, &[], args, *span),
            Expr::GenericCall(callee, type_args, args, span) => {
                self.check_call(callee, type_args, args, *span)
            }
            Expr::Paren(inner, _) => self.check_expr(inner),
            Expr::FieldAccess(base, field, span) => self.check_field_access(base, field, *span),
            Expr::View(inner, _) => self.check_expr(inner),

            Expr::ListConstruct(elems, _span) => self.check_list_construct(elems),
            Expr::MapConstruct(_entries, _span) => {
                // Map construction type checking is deferred.
                TypeInterner::ERROR
            }

            Expr::Handle(target, bind_name, body, span) => {
                self.check_handle(target, bind_name.as_ref(), body, *span)
            }

            Expr::Ok(inner, _span) => {
                let inner_ty = self.check_expr(inner);
                // ok(T) → result[T, <error>] — the error type is unknown without context.
                // For now, produce result[T, nothing].
                self.interner
                    .intern(Type::Result(inner_ty, TypeInterner::ERROR))
            }
            Expr::Fail(inner, _span) => {
                let inner_ty = self.check_expr(inner);
                // fail(E) → result[<error>, E]
                self.interner
                    .intern(Type::Result(TypeInterner::ERROR, inner_ty))
            }
            Expr::Some(inner, _span) => {
                let inner_ty = self.check_expr(inner);
                self.interner.intern(Type::Optional(inner_ty))
            }
            Expr::None(_) => {
                // none → optional[<error>] (unknown inner type without context)
                self.interner.intern(Type::Optional(TypeInterner::ERROR))
            }
            Expr::Default(inner, span) => {
                if self.handle_body_depth == 0 {
                    self.sink.emit(errors::default_outside_handle(*span));
                }
                self.check_expr(inner)
            }

            Expr::StringInterpolation(parts, _) => {
                // Each interpolated expression must be displayable; the overall result is string.
                for part in parts {
                    if let StringPart::Expr(expr) = part {
                        let expr_ty = self.check_expr(expr);
                        if !self.is_displayable_type(expr_ty) {
                            self.sink.emit(errors::type_does_not_implement_interface(
                                &self.type_name(expr_ty),
                                "Displayable",
                                expr.span(),
                            ));
                        }
                    }
                }
                TypeInterner::STRING
            }
            Expr::Declassify(inner, span) => {
                let inner_ty = self.check_expr(inner);
                if let Some(unwrapped) = self.secret_inner_type(inner_ty) {
                    unwrapped
                } else {
                    self.sink.emit(errors::declassify_requires_secret(
                        &self.type_name(inner_ty),
                        *span,
                    ));
                    TypeInterner::ERROR
                }
            }
            Expr::Coarsen(inner, _) => {
                // coarsen strips a refinement type back to its base type.
                // For now just check the inner expression.
                self.check_expr(inner)
            }
            Expr::Pipeline(initial, steps, _) => {
                // Check the initial expression and each step; return the type
                // of the last step (or the initial expression if there are no steps).
                let mut current_ty = self.check_expr(initial);
                for step in steps {
                    // Check the function and extra args but return Error for now
                    // since full pipeline type inference is not yet implemented.
                    self.check_expr(&step.function);
                    for arg in &step.extra_args {
                        self.check_expr(&arg.value);
                    }
                    current_ty = TypeInterner::ERROR;
                }
                current_ty
            }
            Expr::At(inner, _state, _) => {
                // `expr at state` returns a bool.
                self.check_expr(inner);
                TypeInterner::BOOL
            }
            Expr::Error(_) => TypeInterner::ERROR,
            Expr::EnumVariant(type_name, variant, span) => {
                self.check_enum_variant(type_name, variant, &[], *span)
            }
        };

        // Record the type for this expression span.
        self.type_map.insert(expr.span(), ty);
        ty
    }

    fn check_ident(&mut self, ident: &ast::Ident) -> TypeId {
        if let Some(&def_id) = self
            .resolve
            .resolutions
            .get(&ident.span)
            .or_else(|| self.decl_defs.get(&ident.span))
        {
            if let Some(&type_id) = self.type_env.get(&def_id) {
                return type_id;
            }
        }
        // If name resolution didn't find this ident, the resolver already
        // emitted an error. We return Error to avoid cascading type errors.
        TypeInterner::ERROR
    }

    fn is_displayable_type(&self, ty: TypeId) -> bool {
        matches!(
            self.interner.resolve(ty),
            Type::Int8
                | Type::Int16
                | Type::Int32
                | Type::Int64
                | Type::Uint8
                | Type::Uint16
                | Type::Uint32
                | Type::Uint64
                | Type::Float32
                | Type::Float64
                | Type::String
                | Type::Bool
        ) || self.type_implements_named_interface(ty, "Displayable")
    }

    fn type_implements_named_interface(&self, ty: TypeId, interface_name: &str) -> bool {
        let Some(&interface_ty) = self.named_types.get(interface_name) else {
            return false;
        };
        matches!(self.interner.resolve(interface_ty), Type::Interface(_))
            && self.interface_impls.contains_key(&(interface_ty, ty))
    }

    fn check_binary(&mut self, lhs: &Expr, op: BinOp, rhs: &Expr, span: Span) -> TypeId {
        let lhs_ty = self.check_expr(lhs);
        let rhs_ty = self.check_expr(rhs);

        // If either side is an error, propagate.
        if lhs_ty == TypeInterner::ERROR || rhs_ty == TypeInterner::ERROR {
            return TypeInterner::ERROR;
        }

        match op {
            // Arithmetic operators: both sides must be the same numeric type.
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Modulo => {
                if !self.is_numeric(lhs_ty) || !self.is_numeric(rhs_ty) || lhs_ty != rhs_ty {
                    self.sink.emit(errors::binary_op_mismatch(
                        Self::binop_str(op),
                        &self.type_name(lhs_ty),
                        &self.type_name(rhs_ty),
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
                lhs_ty
            }

            // Comparison operators: both sides must be the same type, returns bool.
            BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
                if lhs_ty != rhs_ty {
                    self.sink.emit(errors::binary_op_mismatch(
                        Self::binop_str(op),
                        &self.type_name(lhs_ty),
                        &self.type_name(rhs_ty),
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
                TypeInterner::BOOL
            }

            // Logical operators: both sides must be bool.
            BinOp::And | BinOp::Or => {
                if lhs_ty != TypeInterner::BOOL || rhs_ty != TypeInterner::BOOL {
                    self.sink.emit(errors::binary_op_mismatch(
                        Self::binop_str(op),
                        &self.type_name(lhs_ty),
                        &self.type_name(rhs_ty),
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
                TypeInterner::BOOL
            }
        }
    }

    fn check_unary(&mut self, op: UnaryOp, operand: &Expr, span: Span) -> TypeId {
        let operand_ty = self.check_expr(operand);

        if operand_ty == TypeInterner::ERROR {
            return TypeInterner::ERROR;
        }

        match op {
            UnaryOp::Not => {
                if operand_ty != TypeInterner::BOOL {
                    self.sink.emit(errors::unary_op_mismatch(
                        "not",
                        &self.type_name(operand_ty),
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
                TypeInterner::BOOL
            }
            UnaryOp::Neg => {
                if !self.is_numeric(operand_ty) {
                    self.sink.emit(errors::unary_op_mismatch(
                        "-",
                        &self.type_name(operand_ty),
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
                operand_ty
            }
        }
    }

    fn check_call(
        &mut self,
        callee: &Expr,
        type_args: &[TypeExpr],
        args: &[ast::CallArg],
        span: Span,
    ) -> TypeId {
        let callee_name = Self::extract_dotted_name(callee);

        // -- Capability / purity check --
        // Extract the callee name so we can look it up in the purity map.
        if let Some(callee_name) = callee_name.as_deref() {
            let callee_is_pure = self.purity_map.get(callee_name).copied().unwrap_or(true);

            if !callee_is_pure {
                // E0500: pure function calls impure function
                if self.current_function_pure {
                    if let Some(caller_name) = &self.current_function_name {
                        self.sink
                            .emit(errors::pure_calls_impure(caller_name, &callee_name, span));
                    }
                }
                // E0501: verify block calls impure function
                if self.in_verify_block {
                    if let Some(verify_name) = &self.current_verify_name {
                        self.sink.emit(errors::verify_calls_impure(
                            verify_name,
                            &callee_name,
                            span,
                        ));
                    }
                }
            }
        }

        let builtin_signature = self.builtin_signature(callee, type_args, span);

        let (param_types, return_type) = if let Some(signature) = builtin_signature {
            signature
        } else {
            let callee_ty = self.check_expr(callee);

            if callee_ty == TypeInterner::ERROR {
                // Still check argument expressions so we populate the type map.
                for arg in args {
                    self.check_expr(&arg.value);
                }
                return TypeInterner::ERROR;
            }

            // The callee must be a function type.
            match self.interner.resolve(callee_ty).clone() {
                Type::Function {
                    params,
                    return_type,
                } => (params, return_type),
                Type::Struct(sid) if self.is_struct_type_name_expr(callee) => {
                    return self.check_struct_constructor(sid, args, span);
                }
                _ => {
                    self.sink
                        .emit(errors::not_callable(&self.type_name(callee_ty), span));
                    for arg in args {
                        self.check_expr(&arg.value);
                    }
                    return TypeInterner::ERROR;
                }
            }
        };

        // Check argument count.
        if args.len() != param_types.len() {
            let func_name = callee_name
                .clone()
                .unwrap_or_else(|| "<anonymous>".to_string());
            self.sink.emit(errors::argument_count_mismatch(
                &func_name,
                param_types.len(),
                args.len(),
                span,
            ));
            // Still type-check the provided arguments.
            for arg in args {
                self.check_expr(&arg.value);
            }
            return return_type;
        }

        // Check each argument type.
        for (i, arg) in args.iter().enumerate() {
            let arg_ty = self.check_expr(&arg.value);
            let param_ty = param_types[i];

            if let Some(callee_name) = callee_name.as_deref() {
                if Self::is_secret_output_boundary(callee_name) && self.is_secret_type(arg_ty) {
                    self.sink.emit(errors::secret_exposure(
                        callee_name,
                        &self.type_name(arg_ty),
                        arg.value.span(),
                    ));
                    continue;
                }
            }

            if !self.types_compatible(param_ty, arg_ty) {
                let param_name = format!("#{}", i + 1);
                self.sink.emit(errors::argument_type_mismatch(
                    &param_name,
                    &self.type_name(param_ty),
                    &self.type_name(arg_ty),
                    arg.value.span(),
                ));
            }
        }

        return_type
    }

    fn check_field_access(&mut self, base: &Expr, field: &ast::Ident, span: Span) -> TypeId {
        if let Expr::Ident(base_ident) = base {
            if self.ident_def_kind(base_ident) == Some(DefKind::Enum) {
                return self.check_enum_variant(base_ident, field, &[], span);
            }
            if self.ident_def_kind(base_ident) == Some(DefKind::Interface) {
                let base_ty = self.check_ident(base_ident);
                if let Type::Interface(iid) = *self.interner.resolve(base_ty) {
                    let interface_def = self.interner.resolve_interface(iid);
                    if let Some(method) = interface_def
                        .methods
                        .iter()
                        .find(|m| m.name == field.name)
                        .cloned()
                    {
                        let params = method.params.iter().map(|(_, ty, _)| *ty).collect();
                        return self.interner.intern(Type::Function {
                            params,
                            return_type: method.return_type,
                        });
                    }

                    self.sink.emit(errors::interface_has_no_member(
                        &interface_def.name,
                        &field.name,
                        span,
                    ));
                    return TypeInterner::ERROR;
                }
            }
            if self.ident_def_kind(base_ident) == Some(DefKind::Struct) {
                let base_ty = self.check_ident(base_ident);
                if let Some(method_ty) = self.check_type_module_method(base_ty, field, span) {
                    return method_ty;
                }
            }
            if let Some(type_id) = self.named_types.get(&base_ident.name).copied() {
                if let Some(method_ty) = self.check_type_module_method(type_id, field, span) {
                    return method_ty;
                }
            }
        }

        let base_ty = self.check_expr(base);
        if base_ty == TypeInterner::ERROR {
            return TypeInterner::ERROR;
        }

        match self.interner.resolve(base_ty) {
            Type::Struct(sid) => {
                let struct_def = self.interner.resolve_struct(*sid);
                if let Some((_, field_ty)) = struct_def
                    .fields
                    .iter()
                    .find(|(name, _)| name == &field.name)
                {
                    *field_ty
                } else {
                    self.sink.emit(errors::type_has_no_member(
                        &struct_def.name,
                        &field.name,
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            _ => {
                self.sink.emit(errors::type_has_no_member(
                    &self.type_name(base_ty),
                    &field.name,
                    span,
                ));
                TypeInterner::ERROR
            }
        }
    }

    fn check_type_module_method(
        &mut self,
        type_id: TypeId,
        field: &ast::Ident,
        span: Span,
    ) -> Option<TypeId> {
        if let Type::Struct(sid) = *self.interner.resolve(type_id) {
            let struct_def = self.interner.resolve_struct(sid);
            if let Some(method) = struct_def
                .methods
                .iter()
                .find(|m| m.name == field.name)
                .cloned()
            {
                let params = method.params.iter().map(|(_, ty, _)| *ty).collect();
                return Some(self.interner.intern(Type::Function {
                    params,
                    return_type: method.return_type,
                }));
            }
        }

        if let Some(method) = self
            .impl_methods_by_type
            .get(&type_id)
            .and_then(|methods| methods.get(&field.name))
            .cloned()
        {
            let params = method.params.iter().map(|(_, ty, _)| *ty).collect();
            return Some(self.interner.intern(Type::Function {
                params,
                return_type: method.return_type,
            }));
        }

        self.sink.emit(errors::type_has_no_member(
            &self.type_name(type_id),
            &field.name,
            span,
        ));
        None
    }

    fn check_enum_variant(
        &mut self,
        type_name: &ast::Ident,
        variant: &ast::Ident,
        args: &[ast::CallArg],
        span: Span,
    ) -> TypeId {
        let enum_ty = self.check_ident(type_name);
        if enum_ty == TypeInterner::ERROR {
            return TypeInterner::ERROR;
        }

        let Type::Enum(eid) = *self.interner.resolve(enum_ty) else {
            self.sink.emit(errors::type_has_no_member(
                &self.type_name(enum_ty),
                &variant.name,
                span,
            ));
            return TypeInterner::ERROR;
        };

        let enum_def = self.interner.resolve_enum(eid).clone();
        let Some(variant_def) = enum_def
            .variants
            .iter()
            .find(|candidate| candidate.name == variant.name)
            .cloned()
        else {
            self.sink.emit(errors::type_has_no_member(
                &enum_def.name,
                &variant.name,
                span,
            ));
            return TypeInterner::ERROR;
        };

        if args.is_empty() {
            if variant_def.fields.is_empty() {
                return enum_ty;
            }

            let params = variant_def.fields.iter().map(|(_, ty)| *ty).collect();
            return self.interner.intern(Type::Function {
                params,
                return_type: enum_ty,
            });
        }

        if args.len() != variant_def.fields.len() {
            self.sink.emit(errors::argument_count_mismatch(
                &format!("{}.{}", enum_def.name, variant_def.name),
                variant_def.fields.len(),
                args.len(),
                span,
            ));
        }

        for (index, arg) in args.iter().enumerate() {
            let arg_ty = self.check_expr(&arg.value);
            if let Some((field_name, expected_ty)) = variant_def.fields.get(index) {
                if !self.types_compatible(*expected_ty, arg_ty) {
                    self.sink.emit(errors::argument_type_mismatch(
                        field_name,
                        &self.type_name(*expected_ty),
                        &self.type_name(arg_ty),
                        arg.value.span(),
                    ));
                }
            }
        }

        enum_ty
    }

    fn check_struct_constructor(
        &mut self,
        sid: StructId,
        args: &[ast::CallArg],
        span: Span,
    ) -> TypeId {
        let struct_def = self.interner.resolve_struct(sid).clone();
        let mut assigned = vec![false; struct_def.fields.len()];

        for arg in args {
            let Some(field_index) = (match &arg.name {
                Some(name) => struct_def
                    .fields
                    .iter()
                    .position(|(field_name, _)| field_name == &name.name),
                None => assigned.iter().position(|filled| !filled),
            }) else {
                if let Some(name) = &arg.name {
                    self.sink.emit(errors::type_has_no_member(
                        &struct_def.name,
                        &name.name,
                        arg.span,
                    ));
                } else {
                    self.sink.emit(errors::argument_count_mismatch(
                        &struct_def.name,
                        struct_def.fields.len(),
                        args.len(),
                        span,
                    ));
                }
                self.check_expr(&arg.value);
                continue;
            };

            if assigned[field_index] {
                self.sink.emit(errors::duplicate_constructor_field(
                    &struct_def.name,
                    &struct_def.fields[field_index].0,
                    arg.span,
                ));
                self.check_expr(&arg.value);
                continue;
            }

            assigned[field_index] = true;
            let arg_ty = self.check_expr(&arg.value);
            let expected_ty = struct_def.fields[field_index].1;
            if !self.types_compatible(expected_ty, arg_ty) {
                self.sink.emit(errors::argument_type_mismatch(
                    &struct_def.fields[field_index].0,
                    &self.type_name(expected_ty),
                    &self.type_name(arg_ty),
                    arg.value.span(),
                ));
            }
        }

        for (index, (field_name, _)) in struct_def.fields.iter().enumerate() {
            if !assigned[index] {
                self.sink.emit(errors::missing_constructor_field(
                    &struct_def.name,
                    field_name,
                    span,
                ));
            }
        }

        self.interner.intern(Type::Struct(sid))
    }

    fn check_list_construct(&mut self, elems: &[Expr]) -> TypeId {
        if elems.is_empty() {
            // Empty list: list[<error>] since we can't infer the element type.
            return self.interner.intern(Type::List(TypeInterner::ERROR));
        }

        let first_ty = self.check_expr(&elems[0]);
        for elem in &elems[1..] {
            let elem_ty = self.check_expr(elem);
            if !self.types_compatible(first_ty, elem_ty) {
                self.sink.emit(errors::type_mismatch(
                    &self.type_name(first_ty),
                    &self.type_name(elem_ty),
                    elem.span(),
                ));
            }
        }

        self.interner.intern(Type::List(first_ty))
    }

    fn check_handle(
        &mut self,
        target: &Expr,
        bind_name: Option<&ast::Ident>,
        body: &Block,
        span: Span,
    ) -> TypeId {
        let target_ty = self.check_expr(target);

        if target_ty == TypeInterner::ERROR {
            self.check_handle_body(body);
            self.validate_handle_terminator(body, TypeInterner::ERROR);
            return TypeInterner::ERROR;
        }

        match self.interner.resolve(target_ty).clone() {
            Type::Result(ok_ty, err_ty) => {
                if bind_name.is_none() {
                    self.sink.emit(errors::result_requires_handle_error(span));
                }
                if let Some(name) = bind_name {
                    if let Some(def_id) = self.declaration_def_id(name.span) {
                        self.type_env.insert(def_id, err_ty);
                    }
                }
                self.check_handle_body(body);
                self.validate_handle_terminator(body, ok_ty);
                ok_ty
            }
            Type::Optional(inner_ty) => {
                if bind_name.is_some() {
                    self.sink.emit(errors::optional_requires_bare_handle(span));
                }
                self.check_handle_body(body);
                self.validate_handle_terminator(body, inner_ty);
                inner_ty
            }
            _ => {
                self.sink.emit(errors::handle_requires_result_or_optional(
                    &self.type_name(target_ty),
                    span,
                ));
                self.check_handle_body(body);
                self.validate_handle_terminator(body, TypeInterner::ERROR);
                TypeInterner::ERROR
            }
        }
    }

    fn check_handle_body(&mut self, body: &Block) {
        self.handle_body_depth += 1;
        self.check_block(body);
        self.handle_body_depth -= 1;
    }

    fn validate_handle_terminator(&mut self, body: &Block, success_ty: TypeId) {
        let Some(last_stmt) = body.stmts.last() else {
            self.sink
                .emit(errors::handle_block_requires_return_or_default(body.span));
            return;
        };

        match last_stmt {
            Stmt::Return(_) => {}
            Stmt::Expr(expr_stmt) => {
                if matches!(expr_stmt.expr, Expr::Default(_, _)) {
                    if success_ty != TypeInterner::ERROR {
                        let default_ty = self
                            .type_map
                            .get(&expr_stmt.expr.span())
                            .copied()
                            .unwrap_or(TypeInterner::ERROR);
                        if !self.types_compatible(success_ty, default_ty) {
                            self.sink.emit(errors::type_mismatch(
                                &self.type_name(success_ty),
                                &self.type_name(default_ty),
                                expr_stmt.expr.span(),
                            ));
                        }
                    }
                } else {
                    self.sink
                        .emit(errors::handle_block_requires_return_or_default(
                            expr_stmt.span,
                        ));
                }
            }
            _ => self
                .sink
                .emit(errors::handle_block_requires_return_or_default(stmt_span(
                    last_stmt,
                ))),
        }
    }

    // ------------------------------------------------------------------
    // Type expression resolution (AST TypeExpr → TypeId)
    // ------------------------------------------------------------------

    pub fn resolve_type_expr(&mut self, type_expr: &TypeExpr) -> TypeId {
        match type_expr {
            TypeExpr::Named(ident) => self.resolve_named_type(&ident.name, ident.span),
            TypeExpr::Generic(name, args, span) => {
                self.resolve_generic_type(&name.name, args, *span)
            }
            TypeExpr::View(inner, _span) => {
                // View types are transparent for type checking purposes.
                self.resolve_type_expr(inner)
            }
        }
    }

    fn resolve_named_type(&mut self, name: &str, span: Span) -> TypeId {
        match name {
            "int8" => TypeInterner::INT8,
            "int16" => TypeInterner::INT16,
            "int32" => TypeInterner::INT32,
            "int64" => TypeInterner::INT64,
            "uint8" => TypeInterner::UINT8,
            "uint16" => TypeInterner::UINT16,
            "uint32" => TypeInterner::UINT32,
            "uint64" => TypeInterner::UINT64,
            "float32" => TypeInterner::FLOAT32,
            "float64" => TypeInterner::FLOAT64,
            "string" => TypeInterner::STRING,
            "bool" => TypeInterner::BOOL,
            "bytes" => TypeInterner::BYTES,
            "nothing" => TypeInterner::NOTHING,
            _ if self.named_types.contains_key(name) => self.named_types[name],
            // Capability types are recognised but opaque — no further type
            // checking is performed on values of these types.
            _ if capability::is_capability_type(name) => TypeInterner::ERROR,
            _ => {
                self.sink.emit(errors::unknown_type(name, span));
                TypeInterner::ERROR
            }
        }
    }

    fn resolve_generic_type(&mut self, name: &str, args: &[TypeExpr], span: Span) -> TypeId {
        match name {
            "list" => {
                if args.len() == 1 {
                    let inner = self.resolve_type_expr(&args[0]);
                    self.interner.intern(Type::List(inner))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("list (expected 1 type argument, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            "map" => {
                if args.len() == 2 {
                    let key = self.resolve_type_expr(&args[0]);
                    let val = self.resolve_type_expr(&args[1]);
                    self.interner.intern(Type::Map(key, val))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("map (expected 2 type arguments, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            "set" => {
                if args.len() == 1 {
                    let inner = self.resolve_type_expr(&args[0]);
                    self.interner.intern(Type::Set(inner))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("set (expected 1 type argument, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            "optional" => {
                if args.len() == 1 {
                    let inner = self.resolve_type_expr(&args[0]);
                    self.interner.intern(Type::Optional(inner))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("optional (expected 1 type argument, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            "result" => {
                if args.len() == 2 {
                    let ok = self.resolve_type_expr(&args[0]);
                    let err = self.resolve_type_expr(&args[1]);
                    self.interner.intern(Type::Result(ok, err))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("result (expected 2 type arguments, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            "secret" => {
                if args.len() == 1 {
                    let inner = self.resolve_type_expr(&args[0]);
                    self.interner.intern(Type::Secret(inner))
                } else {
                    self.sink.emit(errors::unknown_type(
                        &format!("secret (expected 1 type argument, got {})", args.len()),
                        span,
                    ));
                    TypeInterner::ERROR
                }
            }
            _ => {
                self.sink.emit(errors::unknown_type(name, span));
                TypeInterner::ERROR
            }
        }
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn binop_str(op: BinOp) -> &'static str {
        match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Modulo => "modulo",
            BinOp::Eq => "==",
            BinOp::NotEq => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::LtEq => "<=",
            BinOp::GtEq => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }
}

fn stmt_span(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::VarDecl(v) => v.span,
        Stmt::Assign(a) => a.span,
        Stmt::Return(r) => r.span,
        Stmt::If(i) => i.span,
        Stmt::For(f) => f.span,
        Stmt::While(w) => w.span,
        Stmt::Match(m) => m.span,
        Stmt::Expr(e) => e.span,
        Stmt::Use(u) => u.span,
        Stmt::Assert(a) => a.span,
        Stmt::Break(span) | Stmt::Continue(span) => *span,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::{FileId, Span};
    use jett_parser::{ast::*, parse};
    use jett_resolve::resolver::ResolveResult;
    use jett_resolve::scope::{DefKind, ScopeTable};

    /// Helper to create a span for tests.
    fn sp(start: u32, end: u32) -> Span {
        Span::new(FileId::new(0), start, end)
    }

    /// Helper: build a resolve result manually for testing.
    struct TestEnv {
        scope_table: ScopeTable,
        resolutions: HashMap<Span, DefId>,
    }

    impl TestEnv {
        fn new() -> Self {
            Self {
                scope_table: ScopeTable::new(),
                resolutions: HashMap::new(),
            }
        }

        fn def_var(&mut self, name: &str, span: Span) -> DefId {
            let def_id = self
                .scope_table
                .new_def(name.to_string(), DefKind::Variable, span);
            self.resolutions.insert(span, def_id);
            def_id
        }

        fn def_param(&mut self, name: &str, span: Span) -> DefId {
            let def_id = self
                .scope_table
                .new_def(name.to_string(), DefKind::Param, span);
            self.resolutions.insert(span, def_id);
            def_id
        }

        fn def_func(&mut self, name: &str, span: Span) -> DefId {
            let def_id = self
                .scope_table
                .new_def(name.to_string(), DefKind::Function, span);
            self.resolutions.insert(span, def_id);
            def_id
        }

        /// Also map an identifier reference span to a DefId.
        fn reference(&mut self, span: Span, def_id: DefId) {
            self.resolutions.insert(span, def_id);
        }

        fn into_resolve_result(self) -> ResolveResult {
            ResolveResult {
                scope_table: self.scope_table,
                resolutions: self.resolutions,
                diagnostics: Vec::new(),
            }
        }
    }

    fn ident(name: &str, span: Span) -> Ident {
        Ident {
            name: name.to_string(),
            span,
        }
    }

    fn check_source_result(source: &str) -> CheckResult {
        let file_id = FileId::new(0);
        let parse_result = parse(source, file_id);
        assert!(
            parse_result.errors.is_empty(),
            "unexpected parse errors: {:?}",
            parse_result.errors
        );

        let resolve_result = jett_resolve::resolve(&parse_result.module);
        let resolve_errors: Vec<_> = resolve_result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(
            resolve_errors.is_empty(),
            "unexpected resolve errors: {:?}",
            resolve_result.diagnostics
        );

        check(&parse_result.module, &resolve_result)
    }

    fn check_source_errors(source: &str) -> Vec<Diagnostic> {
        check_source_result(source)
            .diagnostics
            .into_iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect()
    }

    // ---------------------------------------------------------------
    // Test: simple function with parameters and return type
    // ---------------------------------------------------------------

    #[test]
    fn simple_function_params_and_return() {
        // function add(a: int64, b: int64) returns int64:
        //     return a + b
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);
        let ref_a_span = sp(10, 11);
        let ref_b_span = sp(12, 13);
        let binop_span = sp(10, 13);
        let ret_span = sp(8, 13);
        let body_span = sp(8, 14);
        let func_span = sp(0, 14);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("add", fn_name_span);
        let a_def_id = env.def_param("a", param_a_span);
        let b_def_id = env.def_param("b", param_b_span);
        env.reference(ref_a_span, a_def_id);
        env.reference(ref_b_span, b_def_id);
        // Also reference fn name for self-registration
        env.reference(fn_name_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("add", fn_name_span),
                params: vec![
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("a", param_a_span),
                        ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                        span: param_a_span,
                    },
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("b", param_b_span),
                        ty: TypeExpr::Named(ident("int64", sp(106, 111))),
                        span: param_b_span,
                    },
                ],
                return_type: Some(TypeExpr::Named(ident("int64", sp(112, 117)))),
                body: Block {
                    stmts: vec![Stmt::Return(ReturnStmt {
                        value: Some(Expr::Binary(
                            Box::new(Expr::Ident(ident("a", ref_a_span))),
                            BinOp::Add,
                            Box::new(Expr::Ident(ident("b", ref_b_span))),
                            binop_span,
                        )),
                        span: ret_span,
                    })],
                    span: body_span,
                },
                span: func_span,
            })],
            span: sp(0, 14),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        // No errors expected.
        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);

        // The binary expression should be typed as int64.
        assert_eq!(result.type_map[&binop_span], TypeInterner::INT64);
    }

    // ---------------------------------------------------------------
    // Test: type mismatch error (int64 + string)
    // ---------------------------------------------------------------

    #[test]
    fn type_mismatch_int_plus_string() {
        // a: int64, b: string  →  a + b  should emit an error
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);
        let ref_a_span = sp(10, 11);
        let ref_b_span = sp(12, 13);
        let binop_span = sp(10, 13);
        let body_span = sp(8, 14);
        let func_span = sp(0, 14);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("bad", fn_name_span);
        let a_def_id = env.def_param("a", param_a_span);
        let b_def_id = env.def_param("b", param_b_span);
        env.reference(ref_a_span, a_def_id);
        env.reference(ref_b_span, b_def_id);
        env.reference(fn_name_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("bad", fn_name_span),
                params: vec![
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("a", param_a_span),
                        ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                        span: param_a_span,
                    },
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("b", param_b_span),
                        ty: TypeExpr::Named(ident("string", sp(106, 112))),
                        span: param_b_span,
                    },
                ],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(113, 120)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Binary(
                            Box::new(Expr::Ident(ident("a", ref_a_span))),
                            BinOp::Add,
                            Box::new(Expr::Ident(ident("b", ref_b_span))),
                            binop_span,
                        ),
                        span: binop_span,
                    })],
                    span: body_span,
                },
                span: func_span,
            })],
            span: sp(0, 14),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 301); // binary_op_mismatch
        assert!(errors[0].message.contains("int64"));
        assert!(errors[0].message.contains("string"));
    }

    // ---------------------------------------------------------------
    // Test: binary operator type checking (arithmetic, comparison, logic)
    // ---------------------------------------------------------------

    #[test]
    fn binary_operators_arithmetic_returns_same_type() {
        // 10 + 20 → int64
        let binop_span = sp(0, 5);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Binary(
                            Box::new(Expr::IntLiteral(10, sp(0, 2))),
                            BinOp::Add,
                            Box::new(Expr::IntLiteral(20, sp(3, 5))),
                            binop_span,
                        ),
                        span: binop_span,
                    })],
                    span: sp(0, 5),
                },
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "unexpected errors: {:?}",
            result.diagnostics
        );
        assert_eq!(result.type_map[&binop_span], TypeInterner::INT64);
    }

    #[test]
    fn binary_operators_comparison_returns_bool() {
        // 10 < 20 → bool
        let binop_span = sp(0, 5);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Binary(
                            Box::new(Expr::IntLiteral(10, sp(0, 2))),
                            BinOp::Lt,
                            Box::new(Expr::IntLiteral(20, sp(3, 5))),
                            binop_span,
                        ),
                        span: binop_span,
                    })],
                    span: sp(0, 5),
                },
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "unexpected errors: {:?}",
            result.diagnostics
        );
        assert_eq!(result.type_map[&binop_span], TypeInterner::BOOL);
    }

    #[test]
    fn binary_operators_logic_requires_bool() {
        // true && false → bool (ok)
        let binop_span = sp(0, 10);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Binary(
                            Box::new(Expr::BoolLiteral(true, sp(0, 4))),
                            BinOp::And,
                            Box::new(Expr::BoolLiteral(false, sp(5, 10))),
                            binop_span,
                        ),
                        span: binop_span,
                    })],
                    span: sp(0, 10),
                },
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        assert!(
            result
                .diagnostics
                .iter()
                .all(|d| d.severity != jett_diagnostics::Severity::Error),
            "unexpected errors: {:?}",
            result.diagnostics
        );
        assert_eq!(result.type_map[&binop_span], TypeInterner::BOOL);
    }

    #[test]
    fn binary_operators_logic_error_on_non_bool() {
        // 42 && true → error
        let binop_span = sp(0, 10);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Binary(
                            Box::new(Expr::IntLiteral(42, sp(0, 2))),
                            BinOp::And,
                            Box::new(Expr::BoolLiteral(true, sp(5, 10))),
                            binop_span,
                        ),
                        span: binop_span,
                    })],
                    span: sp(0, 10),
                },
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code.code(), 301);
    }

    // ---------------------------------------------------------------
    // Test: variable declaration type matching
    // ---------------------------------------------------------------

    #[test]
    fn var_decl_type_match_ok() {
        // int64 x = 42
        let var_name_span = sp(6, 7);
        let var_span = sp(0, 10);

        let mut env = TestEnv::new();
        env.def_var("x", var_name_span);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::VarDecl(VarDecl {
                        mutable: false,
                        ty: TypeExpr::Named(ident("int64", sp(0, 5))),
                        name: ident("x", var_name_span),
                        value: Expr::IntLiteral(42, sp(8, 10)),
                        span: var_span,
                    })],
                    span: sp(0, 10),
                },
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn var_decl_type_mismatch() {
        // string x = 42   →  error E0311
        let var_name_span = sp(7, 8);
        let var_span = sp(0, 12);

        let mut env = TestEnv::new();
        env.def_var("x", var_name_span);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::VarDecl(VarDecl {
                        mutable: false,
                        ty: TypeExpr::Named(ident("string", sp(0, 6))),
                        name: ident("x", var_name_span),
                        value: Expr::IntLiteral(42, sp(9, 11)),
                        span: var_span,
                    })],
                    span: sp(0, 12),
                },
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 311);
        assert!(errors[0].message.contains("string"));
        assert!(errors[0].message.contains("int64"));
    }

    // ---------------------------------------------------------------
    // Test: function call argument type checking
    // ---------------------------------------------------------------

    #[test]
    fn function_call_correct_args() {
        // function add(a: int64, b: int64) returns int64
        // add(1, 2)  →  no error, result is int64
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);
        let call_ref_span = sp(20, 23);
        let call_span = sp(20, 30);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("add", fn_name_span);
        env.def_param("a", param_a_span);
        env.def_param("b", param_b_span);
        env.reference(call_ref_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("add", fn_name_span),
                params: vec![
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("a", param_a_span),
                        ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                        span: param_a_span,
                    },
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("b", param_b_span),
                        ty: TypeExpr::Named(ident("int64", sp(106, 111))),
                        span: param_b_span,
                    },
                ],
                return_type: Some(TypeExpr::Named(ident("int64", sp(112, 117)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Call(
                            Box::new(Expr::Ident(ident("add", call_ref_span))),
                            vec![
                                CallArg {
                                    name: None,
                                    value: Expr::IntLiteral(1, sp(24, 25)),
                                    span: sp(24, 25),
                                },
                                CallArg {
                                    name: None,
                                    value: Expr::IntLiteral(2, sp(27, 28)),
                                    span: sp(27, 28),
                                },
                            ],
                            call_span,
                        ),
                        span: call_span,
                    })],
                    span: sp(20, 30),
                },
                span: sp(0, 30),
            })],
            span: sp(0, 30),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
        assert_eq!(result.type_map[&call_span], TypeInterner::INT64);
    }

    #[test]
    fn function_call_wrong_arg_type() {
        // function add(a: int64, b: int64) returns int64
        // add(1, "hello")  →  error E0304
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);
        let call_ref_span = sp(20, 23);
        let call_span = sp(20, 35);
        let bad_arg_span = sp(27, 34);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("add", fn_name_span);
        env.def_param("a", param_a_span);
        env.def_param("b", param_b_span);
        env.reference(call_ref_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("add", fn_name_span),
                params: vec![
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("a", param_a_span),
                        ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                        span: param_a_span,
                    },
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("b", param_b_span),
                        ty: TypeExpr::Named(ident("int64", sp(106, 111))),
                        span: param_b_span,
                    },
                ],
                return_type: Some(TypeExpr::Named(ident("int64", sp(112, 117)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Call(
                            Box::new(Expr::Ident(ident("add", call_ref_span))),
                            vec![
                                CallArg {
                                    name: None,
                                    value: Expr::IntLiteral(1, sp(24, 25)),
                                    span: sp(24, 25),
                                },
                                CallArg {
                                    name: None,
                                    value: Expr::StringLiteral("hello".to_string(), bad_arg_span),
                                    span: bad_arg_span,
                                },
                            ],
                            call_span,
                        ),
                        span: call_span,
                    })],
                    span: sp(20, 35),
                },
                span: sp(0, 35),
            })],
            span: sp(0, 35),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 304); // argument_type_mismatch
        assert!(errors[0].message.contains("int64"));
        assert!(errors[0].message.contains("string"));
    }

    #[test]
    fn function_call_wrong_arg_count() {
        // function add(a: int64, b: int64) returns int64
        // add(1)  →  error E0303
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);
        let call_ref_span = sp(20, 23);
        let call_span = sp(20, 28);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("add", fn_name_span);
        env.def_param("a", param_a_span);
        env.def_param("b", param_b_span);
        env.reference(call_ref_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("add", fn_name_span),
                params: vec![
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("a", param_a_span),
                        ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                        span: param_a_span,
                    },
                    Param {
                        view: false,
                        mutable: false,
                        name: ident("b", param_b_span),
                        ty: TypeExpr::Named(ident("int64", sp(106, 111))),
                        span: param_b_span,
                    },
                ],
                return_type: Some(TypeExpr::Named(ident("int64", sp(112, 117)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Call(
                            Box::new(Expr::Ident(ident("add", call_ref_span))),
                            vec![CallArg {
                                name: None,
                                value: Expr::IntLiteral(1, sp(24, 25)),
                                span: sp(24, 25),
                            }],
                            call_span,
                        ),
                        span: call_span,
                    })],
                    span: sp(20, 28),
                },
                span: sp(0, 28),
            })],
            span: sp(0, 28),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 303); // argument_count_mismatch
    }

    // ---------------------------------------------------------------
    // Test: if condition must be bool
    // ---------------------------------------------------------------

    #[test]
    fn if_condition_must_be_bool() {
        // if 42:   →  error E0306
        let cond_span = sp(3, 5);
        let if_span = sp(0, 10);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::If(IfStmt {
                        condition: Expr::IntLiteral(42, cond_span),
                        then_block: Block {
                            stmts: vec![],
                            span: sp(6, 10),
                        },
                        else_ifs: vec![],
                        else_block: None,
                        span: if_span,
                    })],
                    span: sp(0, 10),
                },
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 306); // condition_not_bool
        assert!(errors[0].message.contains("int64"));
    }

    #[test]
    fn if_condition_bool_ok() {
        // if true:   →  no error
        let cond_span = sp(3, 7);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::If(IfStmt {
                        condition: Expr::BoolLiteral(true, cond_span),
                        then_block: Block {
                            stmts: vec![],
                            span: sp(8, 10),
                        },
                        else_ifs: vec![],
                        else_block: None,
                        span: sp(0, 10),
                    })],
                    span: sp(0, 10),
                },
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    // ---------------------------------------------------------------
    // Test: return type checking
    // ---------------------------------------------------------------

    #[test]
    fn return_type_mismatch() {
        // function foo() returns int64:
        //     return "hello"
        // → error E0305
        let fn_name_span = sp(0, 3);
        let ret_span = sp(10, 25);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("foo", fn_name_span);
        env.reference(fn_name_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("foo", fn_name_span),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("int64", sp(100, 105)))),
                body: Block {
                    stmts: vec![Stmt::Return(ReturnStmt {
                        value: Some(Expr::StringLiteral("hello".to_string(), sp(17, 24))),
                        span: ret_span,
                    })],
                    span: sp(10, 25),
                },
                span: sp(0, 25),
            })],
            span: sp(0, 25),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 305); // return_type_mismatch
        assert!(errors[0].message.contains("int64"));
        assert!(errors[0].message.contains("string"));
    }

    #[test]
    fn return_type_correct() {
        // function foo() returns int64:
        //     return 42
        let fn_name_span = sp(0, 3);

        let mut env = TestEnv::new();
        let fn_def_id = env.def_func("foo", fn_name_span);
        env.reference(fn_name_span, fn_def_id);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("foo", fn_name_span),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("int64", sp(100, 105)))),
                body: Block {
                    stmts: vec![Stmt::Return(ReturnStmt {
                        value: Some(Expr::IntLiteral(42, sp(17, 19))),
                        span: sp(10, 19),
                    })],
                    span: sp(10, 19),
                },
                span: sp(0, 19),
            })],
            span: sp(0, 19),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    // ---------------------------------------------------------------
    // Test: unary operators
    // ---------------------------------------------------------------

    #[test]
    fn unary_not_requires_bool() {
        // not 42  →  error
        let unary_span = sp(0, 6);

        let env = TestEnv::new();
        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::Expr(ExprStmt {
                        expr: Expr::Unary(
                            UnaryOp::Not,
                            Box::new(Expr::IntLiteral(42, sp(4, 6))),
                            unary_span,
                        ),
                        span: unary_span,
                    })],
                    span: sp(0, 6),
                },
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code.code(), 302); // unary_op_mismatch
    }

    // ---------------------------------------------------------------
    // Test: for loop iterable check
    // ---------------------------------------------------------------

    #[test]
    fn for_loop_requires_list() {
        // for x in 42:  →  error E0307
        let var_span = sp(4, 5);
        let iterable_span = sp(9, 11);
        let for_span = sp(0, 15);

        let mut env = TestEnv::new();
        env.def_var("x", var_span);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![Stmt::For(ForStmt {
                        variable: ident("x", var_span),
                        view: false,
                        iterable: Expr::IntLiteral(42, iterable_span),
                        body: Block {
                            stmts: vec![],
                            span: sp(12, 15),
                        },
                        span: for_span,
                    })],
                    span: sp(0, 15),
                },
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 307);
    }

    // ---------------------------------------------------------------
    // Test: assignment type mismatch
    // ---------------------------------------------------------------

    #[test]
    fn assignment_type_mismatch() {
        // int64 x = 42
        // x = "hello"  →  error E0312
        let var_name_span = sp(6, 7);
        let var_span = sp(0, 10);
        let ref_x_span = sp(15, 16);
        let assign_span = sp(15, 26);

        let mut env = TestEnv::new();
        let x_def = env.def_var("x", var_name_span);
        env.reference(ref_x_span, x_def);

        let module = Module {
            items: vec![Item::Function(FunctionDef {
                name: ident("test", sp(50, 54)),
                params: vec![],
                return_type: Some(TypeExpr::Named(ident("nothing", sp(55, 62)))),
                body: Block {
                    stmts: vec![
                        Stmt::VarDecl(VarDecl {
                            mutable: true,
                            ty: TypeExpr::Named(ident("int64", sp(0, 5))),
                            name: ident("x", var_name_span),
                            value: Expr::IntLiteral(42, sp(8, 10)),
                            span: var_span,
                        }),
                        Stmt::Assign(AssignStmt {
                            target: Expr::Ident(ident("x", ref_x_span)),
                            value: Expr::StringLiteral("hello".to_string(), sp(19, 26)),
                            span: assign_span,
                        }),
                    ],
                    span: sp(0, 26),
                },
                span: sp(50, 62),
            })],
            span: sp(0, 62),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "expected 1 error, got: {:?}", errors);
        assert_eq!(errors[0].code.code(), 312); // assign_type_mismatch
    }

    // ---------------------------------------------------------------
    // Test: resolve_type_expr for generic types
    // ---------------------------------------------------------------

    #[test]
    fn resolve_generic_types() {
        // Verify that type expressions like list[int64], result[string, int64] resolve correctly.
        let env = TestEnv::new();
        let resolve = env.into_resolve_result();
        let mut checker = TypeChecker::new(&resolve);

        let list_int = checker.resolve_type_expr(&TypeExpr::Generic(
            ident("list", sp(0, 4)),
            vec![TypeExpr::Named(ident("int64", sp(5, 10)))],
            sp(0, 11),
        ));
        assert_eq!(
            *checker.interner.resolve(list_int),
            Type::List(TypeInterner::INT64)
        );

        let result_type = checker.resolve_type_expr(&TypeExpr::Generic(
            ident("result", sp(0, 6)),
            vec![
                TypeExpr::Named(ident("string", sp(7, 13))),
                TypeExpr::Named(ident("int64", sp(15, 20))),
            ],
            sp(0, 21),
        ));
        assert_eq!(
            *checker.interner.resolve(result_type),
            Type::Result(TypeInterner::STRING, TypeInterner::INT64)
        );
    }

    // ---------------------------------------------------------------
    // Test: capability-based purity enforcement
    // ---------------------------------------------------------------

    /// Helper: create a minimal function definition.
    fn make_function(name: &str, name_span: Span, params: Vec<Param>, body: Block) -> FunctionDef {
        FunctionDef {
            name: ident(name, name_span),
            params,
            return_type: Some(TypeExpr::Named(ident("nothing", sp(200, 207)))),
            body,
            span: Span::new(name_span.file, name_span.start, name_span.start + 50),
        }
    }

    #[test]
    fn pure_function_calling_pure_function_ok() {
        // function helper() returns nothing:
        //     return nothing
        // function caller() returns nothing:
        //     helper()
        let helper_name_span = sp(0, 6);
        let caller_name_span = sp(100, 106);
        let call_ref_span = sp(110, 116);
        let call_span = sp(110, 118);

        let mut env = TestEnv::new();
        let helper_def = env.def_func("helper", helper_name_span);
        let _caller_def = env.def_func("caller", caller_name_span);
        env.reference(helper_name_span, helper_def);
        env.reference(caller_name_span, _caller_def);
        env.reference(call_ref_span, helper_def);

        let module = Module {
            items: vec![
                Item::Function(make_function(
                    "helper",
                    helper_name_span,
                    vec![],
                    Block {
                        stmts: vec![],
                        span: sp(10, 20),
                    },
                )),
                Item::Function(make_function(
                    "caller",
                    caller_name_span,
                    vec![],
                    Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(
                                Box::new(Expr::Ident(ident("helper", call_ref_span))),
                                vec![],
                                call_span,
                            ),
                            span: call_span,
                        })],
                        span: sp(107, 120),
                    },
                )),
            ],
            span: sp(0, 200),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn pure_function_calling_impure_function_error() {
        // function writer(view out: Stdout) returns nothing:
        //     return nothing
        // function caller() returns nothing:
        //     writer()          ← E0500
        let writer_name_span = sp(0, 6);
        let writer_param_span = sp(7, 10);
        let caller_name_span = sp(100, 106);
        let call_ref_span = sp(110, 116);
        let call_span = sp(110, 118);

        let mut env = TestEnv::new();
        let writer_def = env.def_func("writer", writer_name_span);
        let _caller_def = env.def_func("caller", caller_name_span);
        env.def_param("out", writer_param_span);
        env.reference(writer_name_span, writer_def);
        env.reference(caller_name_span, _caller_def);
        env.reference(call_ref_span, writer_def);

        let module = Module {
            items: vec![
                Item::Function(make_function(
                    "writer",
                    writer_name_span,
                    vec![Param {
                        view: true,
                        mutable: false,
                        name: ident("out", writer_param_span),
                        ty: TypeExpr::View(
                            Box::new(TypeExpr::Named(ident("Stdout", sp(11, 17)))),
                            sp(7, 17),
                        ),
                        span: writer_param_span,
                    }],
                    Block {
                        stmts: vec![],
                        span: sp(20, 30),
                    },
                )),
                Item::Function(make_function(
                    "caller",
                    caller_name_span,
                    vec![],
                    Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(
                                Box::new(Expr::Ident(ident("writer", call_ref_span))),
                                vec![],
                                call_span,
                            ),
                            span: call_span,
                        })],
                        span: sp(107, 120),
                    },
                )),
            ],
            span: sp(0, 200),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        // Expect E0500 (pure calls impure) and E0303 (arg count mismatch — caller
        // passes 0 args but writer expects 1).  We only assert E0500 exists.
        let purity_errors: Vec<_> = errors.iter().filter(|d| d.code.code() == 500).collect();
        assert_eq!(
            purity_errors.len(),
            1,
            "expected 1 purity error (E0500), got: {:?}",
            purity_errors
        );
        assert!(purity_errors[0].message.contains("caller"));
        assert!(purity_errors[0].message.contains("writer"));
    }

    #[test]
    fn impure_function_calling_impure_function_ok() {
        // function writer(view out: Stdout) returns nothing:
        //     return nothing
        // function caller(view out: Stdout) returns nothing:
        //     writer()          ← ok, caller is also impure
        let writer_name_span = sp(0, 6);
        let writer_param_span = sp(7, 10);
        let caller_name_span = sp(100, 106);
        let caller_param_span = sp(107, 110);
        let call_ref_span = sp(150, 156);
        let call_span = sp(150, 158);

        let mut env = TestEnv::new();
        let writer_def = env.def_func("writer", writer_name_span);
        let _caller_def = env.def_func("caller", caller_name_span);
        env.def_param("out", writer_param_span);
        env.def_param("out2", caller_param_span);
        env.reference(writer_name_span, writer_def);
        env.reference(caller_name_span, _caller_def);
        env.reference(call_ref_span, writer_def);

        let module = Module {
            items: vec![
                Item::Function(make_function(
                    "writer",
                    writer_name_span,
                    vec![Param {
                        view: true,
                        mutable: false,
                        name: ident("out", writer_param_span),
                        ty: TypeExpr::View(
                            Box::new(TypeExpr::Named(ident("Stdout", sp(11, 17)))),
                            sp(7, 17),
                        ),
                        span: writer_param_span,
                    }],
                    Block {
                        stmts: vec![],
                        span: sp(20, 30),
                    },
                )),
                Item::Function(make_function(
                    "caller",
                    caller_name_span,
                    vec![Param {
                        view: true,
                        mutable: false,
                        name: ident("out2", caller_param_span),
                        ty: TypeExpr::View(
                            Box::new(TypeExpr::Named(ident("Stdout", sp(111, 117)))),
                            sp(107, 117),
                        ),
                        span: caller_param_span,
                    }],
                    Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(
                                Box::new(Expr::Ident(ident("writer", call_ref_span))),
                                vec![],
                                call_span,
                            ),
                            span: call_span,
                        })],
                        span: sp(140, 160),
                    },
                )),
            ],
            span: sp(0, 300),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        // No purity errors expected (E0500).
        let purity_errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code.code() == 500)
            .collect();
        assert!(
            purity_errors.is_empty(),
            "unexpected purity errors: {:?}",
            purity_errors
        );
    }

    #[test]
    fn function_with_stdout_param_is_impure() {
        // function printer(view out: Stdout) returns nothing
        // This is a unit-level check: the purity map marks this function as impure.
        let fn_name_span = sp(0, 7);
        let param_span = sp(8, 11);

        let mut env = TestEnv::new();
        let fn_def = env.def_func("printer", fn_name_span);
        env.def_param("out", param_span);
        env.reference(fn_name_span, fn_def);

        let func = make_function(
            "printer",
            fn_name_span,
            vec![Param {
                view: true,
                mutable: false,
                name: ident("out", param_span),
                ty: TypeExpr::View(
                    Box::new(TypeExpr::Named(ident("Stdout", sp(12, 18)))),
                    sp(8, 18),
                ),
                span: param_span,
            }],
            Block {
                stmts: vec![],
                span: sp(20, 30),
            },
        );

        let module = Module {
            items: vec![Item::Function(func)],
            span: sp(0, 100),
        };

        let resolve = env.into_resolve_result();
        let mut checker = TypeChecker::new(&resolve);
        checker.check_module(&module);

        // The purity map should mark "printer" as impure.
        assert_eq!(
            checker.purity_map.get("printer").copied(),
            Some(false),
            "function with Stdout param should be impure"
        );
    }

    #[test]
    fn function_without_capability_params_is_pure() {
        // function add(a: int64, b: int64) returns nothing
        let fn_name_span = sp(0, 3);
        let param_a_span = sp(4, 5);
        let param_b_span = sp(6, 7);

        let mut env = TestEnv::new();
        let fn_def = env.def_func("add", fn_name_span);
        env.def_param("a", param_a_span);
        env.def_param("b", param_b_span);
        env.reference(fn_name_span, fn_def);

        let func = make_function(
            "add",
            fn_name_span,
            vec![
                Param {
                    view: false,
                    mutable: false,
                    name: ident("a", param_a_span),
                    ty: TypeExpr::Named(ident("int64", sp(100, 105))),
                    span: param_a_span,
                },
                Param {
                    view: false,
                    mutable: false,
                    name: ident("b", param_b_span),
                    ty: TypeExpr::Named(ident("int64", sp(106, 111))),
                    span: param_b_span,
                },
            ],
            Block {
                stmts: vec![],
                span: sp(10, 20),
            },
        );

        let module = Module {
            items: vec![Item::Function(func)],
            span: sp(0, 100),
        };

        let resolve = env.into_resolve_result();
        let mut checker = TypeChecker::new(&resolve);
        checker.check_module(&module);

        assert_eq!(
            checker.purity_map.get("add").copied(),
            Some(true),
            "function without capability params should be pure"
        );
    }

    #[test]
    fn verify_block_calling_impure_function_error() {
        // function writer(view out: Stdout) returns nothing:
        //     return nothing
        // verify test_writer:
        //     writer()          ← E0501
        let writer_name_span = sp(0, 6);
        let writer_param_span = sp(7, 10);
        let call_ref_span = sp(110, 116);
        let call_span = sp(110, 118);
        let verify_span = sp(100, 130);

        let mut env = TestEnv::new();
        let writer_def = env.def_func("writer", writer_name_span);
        env.def_param("out", writer_param_span);
        env.reference(writer_name_span, writer_def);
        env.reference(call_ref_span, writer_def);

        let module = Module {
            items: vec![
                Item::Function(make_function(
                    "writer",
                    writer_name_span,
                    vec![Param {
                        view: true,
                        mutable: false,
                        name: ident("out", writer_param_span),
                        ty: TypeExpr::View(
                            Box::new(TypeExpr::Named(ident("Stdout", sp(11, 17)))),
                            sp(7, 17),
                        ),
                        span: writer_param_span,
                    }],
                    Block {
                        stmts: vec![],
                        span: sp(20, 30),
                    },
                )),
                Item::Verify(VerifyBlock {
                    name: ident("test_writer", sp(100, 111)),
                    body: Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(
                                Box::new(Expr::Ident(ident("writer", call_ref_span))),
                                vec![],
                                call_span,
                            ),
                            span: call_span,
                        })],
                        span: sp(112, 130),
                    },
                    span: verify_span,
                }),
            ],
            span: sp(0, 200),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let verify_errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code.code() == 501)
            .collect();
        assert_eq!(
            verify_errors.len(),
            1,
            "expected 1 verify purity error (E0501), got: {:?}",
            verify_errors
        );
        assert!(verify_errors[0].message.contains("test_writer"));
        assert!(verify_errors[0].message.contains("writer"));
    }

    #[test]
    fn verify_block_calling_pure_function_ok() {
        // function helper() returns nothing:
        //     return nothing
        // verify test_helper:
        //     helper()          ← ok
        let helper_name_span = sp(0, 6);
        let call_ref_span = sp(110, 116);
        let call_span = sp(110, 118);
        let verify_span = sp(100, 130);

        let mut env = TestEnv::new();
        let helper_def = env.def_func("helper", helper_name_span);
        env.reference(helper_name_span, helper_def);
        env.reference(call_ref_span, helper_def);

        let module = Module {
            items: vec![
                Item::Function(make_function(
                    "helper",
                    helper_name_span,
                    vec![],
                    Block {
                        stmts: vec![],
                        span: sp(10, 20),
                    },
                )),
                Item::Verify(VerifyBlock {
                    name: ident("test_helper", sp(100, 111)),
                    body: Block {
                        stmts: vec![Stmt::Expr(ExprStmt {
                            expr: Expr::Call(
                                Box::new(Expr::Ident(ident("helper", call_ref_span))),
                                vec![],
                                call_span,
                            ),
                            span: call_span,
                        })],
                        span: sp(112, 130),
                    },
                    span: verify_span,
                }),
            ],
            span: sp(0, 200),
        };

        let resolve = env.into_resolve_result();
        let result = check(&module, &resolve);

        let purity_errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code.code() == 500 || d.code.code() == 501)
            .collect();
        assert!(
            purity_errors.is_empty(),
            "unexpected purity errors: {:?}",
            purity_errors
        );
    }

    #[test]
    fn mutual_block_allows_mutual_recursion() {
        let result = check_source_result(
            "\
mutual:
    function is_even(n: int64) returns bool
    function is_odd(n: int64) returns bool

function is_even(n: int64) returns bool:
    if n == 0:
        return true
    return is_odd(n - 1)

function is_odd(n: int64) returns bool:
    if n == 0:
        return false
    return is_even(n - 1)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn mutual_block_missing_definition_reports_error() {
        let errors = check_source_errors(
            "\
mutual:
    function is_even(n: int64) returns bool

function main() returns nothing:
    return nothing
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 325),
            "expected E0325, got: {:?}",
            errors
        );
    }

    #[test]
    fn mutual_block_signature_mismatch_reports_error() {
        let errors = check_source_errors(
            "\
mutual:
    function is_even(n: int64) returns bool

function is_even(value: string) returns bool:
    return true
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 326),
            "expected E0326, got: {:?}",
            errors
        );
    }

    #[test]
    fn user_defined_struct_constructor_and_field_access_typecheck_cleanly() {
        let result = check_source_result(
            "\
struct Point:
    x: int64
    y: int64

function sum(view point: Point) returns int64:
    return point.x + point.y

function main() returns int64:
    Point point = Point(x: 1, y: 2)
    return sum(view point)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn struct_method_call_typechecks_cleanly() {
        let result = check_source_result(
            "\
struct Point:
    x: int64
    y: int64

    function total(view self: Point) returns int64:
        return self.x + self.y

function main() returns int64:
    Point point = Point(x: 1, y: 2)
    return Point.total(view point)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn struct_constructor_missing_field_reports_error() {
        let errors = check_source_errors(
            "\
struct Point:
    x: int64
    y: int64

function main() returns Point:
    return Point(x: 1)
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 321),
            "expected E0321, got: {:?}",
            errors
        );
    }

    #[test]
    fn unknown_struct_field_reports_error() {
        let errors = check_source_errors(
            "\
struct Point:
    x: int64
    y: int64

function main(view point: Point) returns int64:
    return point.z
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 319),
            "expected E0319, got: {:?}",
            errors
        );
    }

    #[test]
    fn enum_variant_construction_and_match_typecheck_cleanly() {
        let result = check_source_result(
            "\
enum Shape:
    circle(radius: int64)
    rect(width: int64, height: int64)

function area(shape: Shape) returns int64:
    match shape:
        circle(radius):
            return radius * radius
        rect(width, height):
            return width * height

function main() returns int64:
    Shape shape = Shape.circle(3)
    return area(shape)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn non_exhaustive_enum_match_reports_error() {
        let errors = check_source_errors(
            "\
enum Color:
    red
    blue

function describe(color: Color) returns int64:
    match color:
        red:
            return 1
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 324),
            "expected E0324, got: {:?}",
            errors
        );
    }

    #[test]
    fn enum_pattern_binding_count_mismatch_reports_error() {
        let errors = check_source_errors(
            "\
enum Shape:
    rect(width: int64, height: int64)

function area(shape: Shape) returns int64:
    match shape:
        rect(width):
            return width
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 323),
            "expected E0323, got: {:?}",
            errors
        );
    }

    #[test]
    fn result_handle_requires_error_keyword() {
        let errors = check_source_errors(
            "\
function main() returns int64:
    int64 parsed = int64.from_string(\"42\") handle:
        default 0
    return parsed
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 316),
            "expected E0316, got: {:?}",
            errors
        );
    }

    #[test]
    fn optional_handle_rejects_error_keyword() {
        let errors = check_source_errors(
            "\
function main() returns int64:
    int64 value = some(1) handle error:
        default 0
    return value
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 317),
            "expected E0317, got: {:?}",
            errors
        );
    }

    #[test]
    fn handle_block_requires_explicit_terminator() {
        let errors = check_source_errors(
            "\
function main() returns int64:
    int64 parsed = int64.from_string(\"oops\") handle error:
        int64 fallback = 0
    return parsed
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 318),
            "expected E0318, got: {:?}",
            errors
        );
    }

    #[test]
    fn handle_default_must_match_unwrapped_type() {
        let errors = check_source_errors(
            "\
function main() returns int64:
    int64 parsed = int64.from_string(\"oops\") handle error:
        default \"bad\"
    return parsed
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 300),
            "expected E0300, got: {:?}",
            errors
        );
    }

    #[test]
    fn handle_with_builtin_result_type_checks_cleanly() {
        let errors = check_source_errors(
            "\
function main() returns int64:
    int64 parsed = int64.from_string(\"42\") handle error:
        default 0
    return parsed
",
        );

        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn interface_implement_call_typechecks_cleanly() {
        let result = check_source_result(
            "\
interface Speaker:
    function speak(view self: Speaker) returns string

struct Dog:
    name: string

implement Speaker for Dog:
    function speak(view self: Dog) returns string:
        return self.name

function main() returns string:
    Dog dog = Dog(name: \"woof\")
    return Speaker.speak(view dog)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn implement_block_missing_method_reports_error() {
        let errors = check_source_errors(
            "\
interface Speaker:
    function speak(view self: Speaker) returns string
    function growl(view self: Speaker) returns string

struct Dog:
    name: string

implement Speaker for Dog:
    function speak(view self: Dog) returns string:
        return self.name
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 331),
            "expected E0331, got: {:?}",
            errors
        );
    }

    #[test]
    fn implement_block_signature_mismatch_reports_error() {
        let errors = check_source_errors(
            "\
interface Speaker:
    function speak(view self: Speaker) returns string

struct Dog:
    name: string

implement Speaker for Dog:
    function speak(self: Dog) returns int64:
        return 1
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 330),
            "expected E0330, got: {:?}",
            errors
        );
    }

    #[test]
    fn string_interpolation_accepts_user_defined_displayable_types() {
        let result = check_source_result(
            "\
interface Displayable:
    function display(view self: Displayable) returns string

struct User:
    name: string

implement Displayable for User:
    function display(view self: User) returns string:
        return self.name

function main() returns string:
    User user = User(name: \"Ada\")
    return \"user: {user}\"
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn secret_value_can_be_declassified() {
        let result = check_source_result(
            "\
function reveal(key: secret[string]) returns string:
    return declassify key

function main() returns string:
    secret[string] api_key = \"abc\"
    return reveal(api_key)
",
        );

        let errors: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.severity == jett_diagnostics::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn declassify_requires_secret_type() {
        let errors = check_source_errors(
            "\
function main() returns string:
    return declassify \"abc\"
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 601),
            "expected E0601, got: {:?}",
            errors
        );
    }

    #[test]
    fn stdout_write_rejects_secret_values() {
        let errors = check_source_errors(
            "\
function main(view stdout: Stdout) returns nothing:
    secret[string] api_key = \"abc\"
    Stdout.write(view stdout, api_key)
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 600),
            "expected E0600, got: {:?}",
            errors
        );
    }

    #[test]
    fn secret_values_are_not_displayable() {
        let errors = check_source_errors(
            "\
function main() returns string:
    secret[string] api_key = \"abc\"
    return \"api key: {api_key}\"
",
        );

        assert!(
            errors.iter().any(|d| d.code.code() == 332),
            "expected E0332, got: {:?}",
            errors
        );
    }
}
