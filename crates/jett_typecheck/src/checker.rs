use std::collections::HashMap;

use jett_common::Span;
use jett_diagnostics::{Diagnostic, DiagnosticSink};
use jett_parser::ast::{
    self, BinOp, Block, Expr, FunctionDef, Item, Module, Stmt, StringPart, TypeExpr, UnaryOp,
    VerifyBlock,
};
use jett_resolve::resolver::ResolveResult;
use jett_resolve::scope::DefId;
use jett_types::{Type, TypeId, TypeInterner};

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
    let ownership_diagnostics =
        crate::ownership::check_ownership(module, &checker.interner);

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
    /// Expression span → TypeId (the output type map).
    type_map: HashMap<Span, TypeId>,
    /// The expected return type for the function currently being checked.
    current_return_type: Option<TypeId>,

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
        Self {
            interner: TypeInterner::new(),
            resolve,
            sink: DiagnosticSink::new(),
            type_env: HashMap::new(),
            type_map: HashMap::new(),
            current_return_type: None,
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
            Type::Function { params, return_type } => {
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

    fn builtin_signature(&mut self, callee: &Expr) -> Option<(Vec<TypeId>, TypeId)> {
        let name = Self::extract_dotted_name(callee)?;
        match name.as_str() {
            "int64.from_string" => Some((
                vec![TypeInterner::STRING],
                self.interner
                    .intern(Type::Result(TypeInterner::INT64, TypeInterner::STRING)),
            )),
            "string.from_int64" => Some((vec![TypeInterner::INT64], TypeInterner::STRING)),
            "string.from_float64" => Some((vec![TypeInterner::FLOAT64], TypeInterner::STRING)),
            _ => None,
        }
    }

    // ------------------------------------------------------------------
    // Module
    // ------------------------------------------------------------------

    fn check_module(&mut self, module: &Module) {
        // First pass: register all top-level function signatures into the type env
        // and build the purity map.
        for item in &module.items {
            if let Item::Function(func) = item {
                self.register_function_sig(func);
                let is_pure = Self::function_is_pure(func);
                self.purity_map.insert(func.name.name.clone(), is_pure);
            }
        }

        // Second pass: type-check function bodies and verify blocks.
        for item in &module.items {
            match item {
                Item::Function(func) => self.check_function(func),
                Item::VarDecl(decl) => self.check_var_decl(decl),
                Item::Verify(verify) => self.check_verify_block(verify),
                // Struct/Enum/Namespace declarations are handled during
                // registration; nothing to type-check in their bodies yet.
                _ => {}
            }
        }
    }

    /// Returns true if a function has no capability-type parameters (i.e. is pure).
    fn function_is_pure(func: &FunctionDef) -> bool {
        !func.params.iter().any(|p| capability::type_expr_is_capability(&p.ty))
    }

    // ------------------------------------------------------------------
    // Function registration (builds FunctionType + binds to DefId)
    // ------------------------------------------------------------------

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
        if let Some(&def_id) = self.resolve.resolutions.get(&func.name.span) {
            self.type_env.insert(def_id, fn_type);
        }
    }

    // ------------------------------------------------------------------
    // Function body
    // ------------------------------------------------------------------

    fn check_function(&mut self, func: &FunctionDef) {
        let return_type = func
            .return_type
            .as_ref()
            .map(|t| self.resolve_type_expr(t))
            .unwrap_or(TypeInterner::NOTHING);

        self.current_return_type = Some(return_type);

        // Set the purity context for this function.
        let is_pure = Self::function_is_pure(func);
        self.current_function_name = Some(func.name.name.clone());
        self.current_function_pure = is_pure;

        // Bind parameter types into the type environment.
        for param in &func.params {
            let param_type = self.resolve_type_expr(&param.ty);
            if let Some(&def_id) = self.resolve.resolutions.get(&param.name.span) {
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
            Stmt::Match(_) | Stmt::Use(_) | Stmt::Break(_) | Stmt::Continue(_) => {
                // Nothing to type-check for these (match type-checking is TODO).
            }
        }
    }

    fn check_var_decl(&mut self, decl: &ast::VarDecl) {
        let declared_type = self.resolve_type_expr(&decl.ty);
        let init_type = self.check_expr(&decl.value);

        // Bind the variable's DefId to its declared type.
        if let Some(&def_id) = self.resolve.resolutions.get(&decl.name.span) {
            self.type_env.insert(def_id, declared_type);
        }

        // Check that the initializer type matches the declared type (skip if Error).
        if declared_type != TypeInterner::ERROR
            && init_type != TypeInterner::ERROR
            && declared_type != init_type
        {
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

        if target_type != TypeInterner::ERROR
            && value_type != TypeInterner::ERROR
            && target_type != value_type
        {
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
            if expected != TypeInterner::ERROR
                && ret_type != TypeInterner::ERROR
                && expected != ret_type
            {
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
        if let Some(&def_id) = self.resolve.resolutions.get(&for_stmt.variable.span) {
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
            Expr::Call(callee, args, span) => self.check_call(callee, args, *span),
            Expr::GenericCall(callee, _type_args, args, span) => {
                // For now, treat generic calls the same as normal calls.
                self.check_call(callee, args, *span)
            }
            Expr::Paren(inner, _) => self.check_expr(inner),
            Expr::FieldAccess(_, _, _) => {
                // Field access type checking requires struct resolution;
                // return Error for now.
                TypeInterner::ERROR
            }
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
                    .intern(Type::Result(inner_ty, TypeInterner::NOTHING))
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
                // Each interpolated expression is checked; the overall result is string.
                for part in parts {
                    if let StringPart::Expr(expr) = part {
                        self.check_expr(expr);
                    }
                }
                TypeInterner::STRING
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
            Expr::EnumVariant(_, _, _) => {
                // TODO: resolve enum variant type
                TypeInterner::ERROR
            }
        };

        // Record the type for this expression span.
        self.type_map.insert(expr.span(), ty);
        ty
    }

    fn check_ident(&mut self, ident: &ast::Ident) -> TypeId {
        if let Some(&def_id) = self.resolve.resolutions.get(&ident.span) {
            if let Some(&type_id) = self.type_env.get(&def_id) {
                return type_id;
            }
        }
        // If name resolution didn't find this ident, the resolver already
        // emitted an error. We return Error to avoid cascading type errors.
        TypeInterner::ERROR
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

    fn check_call(&mut self, callee: &Expr, args: &[ast::CallArg], span: Span) -> TypeId {
        // -- Capability / purity check --
        // Extract the callee name so we can look it up in the purity map.
        if let Expr::Ident(callee_ident) = callee {
            let callee_name = &callee_ident.name;
            let callee_is_pure = self.purity_map.get(callee_name).copied().unwrap_or(true);

            if !callee_is_pure {
                // E0500: pure function calls impure function
                if self.current_function_pure {
                    if let Some(caller_name) = &self.current_function_name {
                        self.sink.emit(errors::pure_calls_impure(
                            caller_name,
                            callee_name,
                            span,
                        ));
                    }
                }
                // E0501: verify block calls impure function
                if self.in_verify_block {
                    if let Some(verify_name) = &self.current_verify_name {
                        self.sink.emit(errors::verify_calls_impure(
                            verify_name,
                            callee_name,
                            span,
                        ));
                    }
                }
            }
        }

        let builtin_signature = self.builtin_signature(callee);

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
            let func_name =
                Self::extract_dotted_name(callee).unwrap_or_else(|| "<anonymous>".to_string());
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

            if arg_ty != TypeInterner::ERROR
                && param_ty != TypeInterner::ERROR
                && arg_ty != param_ty
            {
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

    fn check_list_construct(&mut self, elems: &[Expr]) -> TypeId {
        if elems.is_empty() {
            // Empty list: list[<error>] since we can't infer the element type.
            return self.interner.intern(Type::List(TypeInterner::ERROR));
        }

        let first_ty = self.check_expr(&elems[0]);
        for elem in &elems[1..] {
            let elem_ty = self.check_expr(elem);
            if first_ty != TypeInterner::ERROR
                && elem_ty != TypeInterner::ERROR
                && first_ty != elem_ty
            {
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
                    if let Some(&def_id) = self.resolve.resolutions.get(&name.span) {
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
                        if default_ty != TypeInterner::ERROR && default_ty != success_ty {
                            self.sink.emit(errors::type_mismatch(
                                &self.type_name(success_ty),
                                &self.type_name(default_ty),
                                expr_stmt.expr.span(),
                            ));
                        }
                    }
                } else {
                    self.sink
                        .emit(errors::handle_block_requires_return_or_default(expr_stmt.span));
                }
            }
            _ => self
                .sink
                .emit(errors::handle_block_requires_return_or_default(stmt_span(last_stmt))),
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
            // Capability types are recognised but opaque — no further type
            // checking is performed on values of these types.
            _ if capability::is_capability_type(name) => TypeInterner::ERROR,
            _ => {
                self.sink.emit(errors::unknown_type(name, span));
                TypeInterner::ERROR
            }
        }
    }

    fn resolve_generic_type(
        &mut self,
        name: &str,
        args: &[TypeExpr],
        span: Span,
    ) -> TypeId {
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
            let def_id = self.scope_table.new_def(name.to_string(), DefKind::Variable, span);
            self.resolutions.insert(span, def_id);
            def_id
        }

        fn def_param(&mut self, name: &str, span: Span) -> DefId {
            let def_id = self.scope_table.new_def(name.to_string(), DefKind::Param, span);
            self.resolutions.insert(span, def_id);
            def_id
        }

        fn def_func(&mut self, name: &str, span: Span) -> DefId {
            let def_id = self.scope_table.new_def(name.to_string(), DefKind::Function, span);
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

    fn check_source_errors(source: &str) -> Vec<Diagnostic> {
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
            result.diagnostics.iter().all(|d| d.severity != jett_diagnostics::Severity::Error),
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
            result.diagnostics.iter().all(|d| d.severity != jett_diagnostics::Severity::Error),
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
            result.diagnostics.iter().all(|d| d.severity != jett_diagnostics::Severity::Error),
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
    fn make_function(
        name: &str,
        name_span: Span,
        params: Vec<Param>,
        body: Block,
    ) -> FunctionDef {
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
}
