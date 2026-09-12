//! The initial graphics callbacks retain a stricter, source-provable effect
//! boundary until ordinary function values carry effect metadata themselves.

use super::*;

impl TypeChecker<'_> {
    pub(super) fn graphics_state_contains_authority(&self, initial: TypeId) -> bool {
        let mut pending = vec![initial];
        let mut seen = HashSet::new();
        while let Some(ty) = pending.pop() {
            if !seen.insert(ty) {
                continue;
            }
            match self.interner.resolve(ty) {
                Type::Error
                | Type::TypeConstruction
                | Type::Function { .. }
                | Type::Resource(_)
                | Type::Actor(_)
                | Type::Interface(_) => return true,
                Type::List(inner)
                | Type::Set(inner)
                | Type::Optional(inner)
                | Type::Secret(inner)
                | Type::Refinement { base: inner, .. } => pending.push(*inner),
                Type::Map(key, value) | Type::Result(key, value) => pending.extend([*key, *value]),
                Type::Struct(id) => pending.extend(
                    self.interner
                        .resolve_struct(*id)
                        .fields
                        .iter()
                        .map(|(_, ty)| *ty),
                ),
                Type::Enum(id) => pending.extend(
                    self.interner
                        .resolve_enum(*id)
                        .variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter().map(|(_, ty)| *ty)),
                ),
                Type::Bitfield(id) => pending.extend(
                    self.interner
                        .resolve_bitfield(*id)
                        .fields
                        .iter()
                        .map(|field| field.ty),
                ),
                Type::Machine(id) => pending.extend(
                    self.interner
                        .resolve_machine(*id)
                        .states
                        .iter()
                        .flat_map(|state| state.fields.iter().map(|(_, ty)| *ty)),
                ),
                Type::MachineState { machine, state } => {
                    if let Some(state) = self.interner.resolve_machine(*machine).state(*state) {
                        pending.extend(state.fields.iter().map(|(_, ty)| *ty));
                    }
                }
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
                | Type::Bytes
                | Type::Nothing => {}
            }
        }
        false
    }

    pub(super) fn audit_graphics_function(&mut self, function: &FunctionDef) {
        if !self.graphics_audited_functions.insert(function.span)
            || !Self::params_are_pure(&function.params)
        {
            return;
        }
        self.audit_graphics_block(&function.body, false);
    }

    fn audit_graphics_block(&mut self, block: &Block, inline: bool) {
        for statement in &block.stmts {
            match statement {
                Stmt::VarDecl(declaration) => {
                    self.audit_graphics_expr(&declaration.value, inline);
                }
                Stmt::Assign(assignment) => {
                    self.audit_graphics_expr(&assignment.target, inline);
                    self.audit_graphics_expr(&assignment.value, inline);
                }
                Stmt::Return(statement) => {
                    if let Some(value) = &statement.value {
                        self.audit_graphics_expr(value, inline);
                    }
                }
                Stmt::Respond(statement) => self.audit_graphics_expr(&statement.value, inline),
                Stmt::ComptimeTypeBind(binding) => {
                    self.audit_graphics_expr(&binding.value, inline);
                    self.audit_graphics_block(&binding.body, inline);
                }
                Stmt::If(statement) => {
                    self.audit_graphics_expr(&statement.condition, inline);
                    self.audit_graphics_block(&statement.then_block, inline);
                    for (condition, body) in &statement.else_ifs {
                        self.audit_graphics_expr(condition, inline);
                        self.audit_graphics_block(body, inline);
                    }
                    if let Some(body) = &statement.else_block {
                        self.audit_graphics_block(body, inline);
                    }
                }
                Stmt::For(statement) => {
                    self.audit_graphics_expr(&statement.iterable, inline);
                    self.audit_graphics_block(&statement.body, inline);
                }
                Stmt::While(statement) => {
                    self.audit_graphics_expr(&statement.condition, inline);
                    self.audit_graphics_block(&statement.body, inline);
                }
                Stmt::Match(statement) => {
                    self.audit_graphics_expr(&statement.expr, inline);
                    for arm in &statement.arms {
                        self.audit_graphics_block(&arm.body, inline);
                    }
                }
                Stmt::Expr(statement) => self.audit_graphics_expr(&statement.expr, inline),
                Stmt::Assert(statement) => {
                    self.audit_graphics_expr(&statement.condition, inline);
                    if let Some(message) = &statement.message {
                        self.audit_graphics_expr(message, inline);
                    }
                }
                Stmt::Breakpoint(statement) => {
                    if let Some(condition) = &statement.condition {
                        self.audit_graphics_expr(condition, inline);
                    }
                }
                Stmt::Use(_) | Stmt::Trace(_) | Stmt::Break(_) | Stmt::Continue(_) => {}
            }
        }
    }

    fn audit_graphics_expr(&mut self, expr: &Expr, inline: bool) {
        match expr {
            Expr::Ident(_) | Expr::FieldAccess(_, _, _) => {
                self.audit_graphics_reference(expr, false);
                if inline && self.type_map.get(&expr.span()) == Some(&TypeInterner::ERROR) {
                    self.sink.emit(errors::graphics_contract(
                        "callback closures cannot contain opaque capability values",
                        expr.span(),
                    ));
                }
                if let Expr::FieldAccess(base, _, _) = expr {
                    self.audit_graphics_expr(base, inline);
                }
            }
            Expr::Call(callee, args, span) | Expr::GenericCall(callee, _, args, span) => {
                self.audit_graphics_call(callee, *span, inline);
                for arg in args {
                    self.audit_graphics_expr(&arg.value, inline);
                }
            }
            Expr::InlineFn(params, return_type, body, span) => {
                if params
                    .iter()
                    .any(|param| self.graphics_type_contains_capability(&param.ty))
                    || return_type
                        .as_ref()
                        .is_some_and(|ty| self.graphics_type_contains_capability(ty))
                {
                    self.sink.emit(errors::graphics_contract(
                        "callback closures must be pure and cannot accept or return capabilities",
                        *span,
                    ));
                }
                self.audit_graphics_block(body, true);
            }
            Expr::Binary(left, _, right, _) => {
                self.audit_graphics_expr(left, inline);
                self.audit_graphics_expr(right, inline);
            }
            Expr::Unary(_, inner, _)
            | Expr::Paren(inner, _)
            | Expr::View(inner, _)
            | Expr::Comptime(inner, _)
            | Expr::Ok(inner, _)
            | Expr::Fail(inner, _)
            | Expr::Some(inner, _)
            | Expr::Default(inner, _)
            | Expr::Declassify(inner, _)
            | Expr::Coarsen(inner, _)
            | Expr::At(inner, _, _)
            | Expr::Spawn(inner, _)
            | Expr::Send(inner, _)
            | Expr::Ask(inner, _)
            | Expr::Clone(inner, _)
            | Expr::Run(inner, _)
            | Expr::Join(inner, _)
            | Expr::Cancel(inner, _) => self.audit_graphics_expr(inner, inline),
            Expr::ListConstruct(items, _) => {
                for item in items {
                    self.audit_graphics_expr(item, inline);
                }
            }
            Expr::MapConstruct(items, _) => {
                for (key, value) in items {
                    self.audit_graphics_expr(key, inline);
                    self.audit_graphics_expr(value, inline);
                }
            }
            Expr::Handle(value, _, body, _) => {
                self.audit_graphics_expr(value, inline);
                self.audit_graphics_block(body, inline);
            }
            Expr::StringInterpolation(parts, _) => {
                for part in parts {
                    if let StringPart::Expr(value) = part {
                        self.audit_graphics_expr(value, inline);
                    }
                }
            }
            Expr::Pipeline(initial, steps, _) => {
                self.audit_graphics_expr(initial, inline);
                for step in steps {
                    self.audit_graphics_call(&step.function, step.span, inline);
                    for arg in &step.extra_args {
                        self.audit_graphics_expr(&arg.value, inline);
                    }
                    if let Some(handler) = &step.handle {
                        self.audit_graphics_block(&handler.body, inline);
                    }
                }
            }
            Expr::IntLiteral(_, _)
            | Expr::FloatLiteral(_, _)
            | Expr::StringLiteral(_, _)
            | Expr::BoolLiteral(_, _)
            | Expr::Nothing(_)
            | Expr::None(_)
            | Expr::EnumVariant(_, _, _)
            | Expr::Error(_) => {}
        }
    }

    fn audit_graphics_call(&mut self, callee: &Expr, span: Span, inline: bool) {
        if inline
            && let Some(name) = self.resolved_expr_name(callee)
            && !self.named_call_is_pure(&name)
        {
            self.sink.emit(errors::pure_calls_impure(
                "<graphics callback>",
                &name,
                span,
            ));
        }
        if let Some(ty) = self.type_map.get(&callee.span())
            && let Type::Function { params, .. } = self.interner.resolve(*ty)
            && params.iter().any(|ty| *ty == TypeInterner::ERROR)
        {
            self.sink.emit(errors::graphics_contract(
                "callbacks cannot call function values with opaque capability parameters",
                span,
            ));
        }
        self.audit_graphics_reference(callee, true);
        if let Some(method) = self.method_calls.get(&span)
            && let Some(function) = self
                .graphics_method_definitions
                .get(&method.source_span)
                .cloned()
        {
            self.audit_graphics_function(&function);
        }
        if !matches!(callee, Expr::Ident(_) | Expr::FieldAccess(_, _, _)) {
            self.audit_graphics_expr(callee, inline);
        } else if let Expr::FieldAccess(base, _, _) = callee
            && !self.graphics_callback_is_named(callee)
        {
            self.audit_graphics_expr(base, inline);
        }
    }

    fn audit_graphics_reference(&mut self, expr: &Expr, called: bool) {
        let Some(name) = self.resolved_expr_name(expr) else {
            return;
        };
        if !called && !self.named_call_is_pure(&name) {
            self.sink.emit(errors::graphics_contract(
                "callbacks cannot retain impure function values",
                expr.span(),
            ));
        }
        if self.graphics_callback_is_named(expr)
            && let Some(function) = self.graphics_callback_definitions.get(&name).cloned()
        {
            self.audit_graphics_function(&function);
        }
    }
}
