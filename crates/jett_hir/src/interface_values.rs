//! Preserve checked interface boundaries before ownership and CFG lowering.
use super::*;

impl Lowerer<'_> {
    pub(super) fn collect_interface_dispatches(&mut self) -> Vec<Function> {
        let mut slots = Vec::new();
        fn calls(slots: &mut Vec<(TypeId, usize)>, facts: &HashMap<Span, CheckedInterfaceCall>) {
            slots.extend(
                facts
                    .values()
                    .map(|call| (call.interface_type, call.method_index)),
            );
        }
        fn values(slots: &mut Vec<(TypeId, usize)>, facts: &HashMap<Span, CheckedMethodValue>) {
            slots.extend(facts.values().filter_map(|value| match value {
                CheckedMethodValue::Source { .. } => None,
                CheckedMethodValue::Interface {
                    interface_type,
                    method_index,
                } => Some((*interface_type, *method_index)),
            }));
        }
        fn bindings(
            slots: &mut Vec<(TypeId, usize)>,
            facts: &HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
        ) {
            for binding in facts.values().flatten() {
                calls(slots, &binding.body.interface_calls);
                values(slots, &binding.body.method_values);
                bindings(slots, &binding.body.comptime_type_bindings);
            }
        }
        calls(&mut slots, &self.check.interface_calls);
        values(&mut slots, &self.check.method_values);
        bindings(&mut slots, &self.check.comptime_type_bindings);
        for body in &self.check.generic_function_instantiations {
            calls(&mut slots, &body.interface_calls);
            values(&mut slots, &body.method_values);
            bindings(&mut slots, &body.comptime_type_bindings);
        }
        let first = self.functions.len()
            + self.actor_constructors.len()
            + self.actor_handlers.len()
            + self.refinement_sources.len();
        let mut functions = Vec::new();
        slots.sort_by_key(|(owner, method)| (owner.index(), *method));
        slots.dedup();
        for (owner, method_index) in slots {
            let Type::Interface(interface) = self.check.interner.resolve(owner) else {
                self.error(
                    self.module.span,
                    "checked interface slot has a non-interface owner",
                );
                continue;
            };
            let interface = self.check.interner.resolve_interface(*interface);
            let Some((&definition, _)) =
                self.check.definition_types.iter().find(|(definition, ty)| {
                    **ty == owner
                        && self.resolve.scope_table.def(**definition).kind == DefKind::Interface
                })
            else {
                self.error(
                    self.module.span,
                    "checked interface slot has no resolved declaration",
                );
                continue;
            };
            let declaration = self.resolve.scope_table.def(definition);
            let span = declaration.span;
            let Some(method) = interface.methods.get(method_index) else {
                self.error(span, "checked interface method slot is out of range");
                continue;
            };
            let Some(origin) = self.origins.get(&span.file).cloned() else {
                self.error(span, "checked interface declaration has no source origin");
                continue;
            };
            let id = FunctionId((first + functions.len()) as u32);
            self.function_ids.insert(
                FunctionKey::Interface {
                    owner,
                    method: method_index,
                },
                id,
            );
            let params = method
                .params
                .iter()
                .enumerate()
                .map(|(index, (name, ty, view))| Param {
                    local: LocalId(index as u32),
                    name: name.clone(),
                    ty: *ty,
                    mode: if *view {
                        ParamMode::View
                    } else {
                        ParamMode::Owned
                    },
                    mutable: false,
                    span,
                })
                .collect::<Vec<_>>();
            let locals = params
                .iter()
                .map(|param| Local {
                    id: param.local,
                    name: param.name.clone(),
                    ty: param.ty,
                    debug_ty: param.ty,
                    debug_type_name: None,
                    mutable: false,
                    view_source: None,
                    span,
                })
                .collect();
            let mut statements = Vec::new();
            for implementation in &self.check.method_definitions {
                if implementation.interface_type != Some(owner)
                    || implementation.method_name != method.name
                {
                    continue;
                }
                let Some(&target) = self.function_ids.get(&FunctionKey::Method {
                    source_span: implementation.source_span,
                }) else {
                    self.error(
                        implementation.source_span,
                        "checked interface implementation has no native function",
                    );
                    continue;
                };
                let receiver = Expression {
                    kind: ExpressionKind::Local(LocalId(0)),
                    ty: owner,
                    span,
                };
                let condition = Expression {
                    kind: ExpressionKind::Binary {
                        left: Box::new(Expression {
                            kind: ExpressionKind::InterfaceType(Box::new(receiver)),
                            ty: TypeInterner::UINT64,
                            span,
                        }),
                        op: BinaryOp::Equal,
                        right: Box::new(Expression {
                            kind: ExpressionKind::Int(implementation.owner_type.index() as i128),
                            ty: TypeInterner::UINT64,
                            span,
                        }),
                    },
                    ty: TypeInterner::BOOL,
                    span,
                };
                let args = params
                    .iter()
                    .zip(&implementation.parameter_types)
                    .map(|(param, ty)| {
                        let mut value = Expression {
                            kind: ExpressionKind::Local(param.local),
                            ty: param.ty,
                            span,
                        };
                        coerce(&mut value, *ty, &self.check.interner);
                        value
                    })
                    .collect::<Vec<_>>();
                let value = Expression {
                    kind: ExpressionKind::Call {
                        function: target,
                        evaluation_order: (0..args.len()).collect(),
                        args,
                    },
                    ty: implementation.return_type,
                    span,
                };
                statements.push(Statement {
                    kind: StatementKind::If {
                        condition,
                        then_block: Block {
                            statements: vec![Statement {
                                kind: StatementKind::Return(Some(value)),
                                span,
                            }],
                            span,
                        },
                        else_block: None,
                    },
                    span,
                });
            }
            let name = format!("{}.{}", interface.name, method.name);
            statements.push(Statement {
                kind: StatementKind::Return(Some(Expression {
                    kind: ExpressionKind::RuntimeFailure(format!("undefined function '{name}'")),
                    ty: method.return_type,
                    span,
                })),
                span,
            });
            functions.push(Function {
                id,
                identity: FunctionIdentity {
                    declaration: DeclarationId {
                        origin,
                        namespace: declaration.namespace.clone().unwrap_or_default(),
                        name: format!("$interface.dispatch.{name}"),
                        kind: DeclarationKind::Method,
                    },
                    scoped_type_bindings: Vec::new(),
                    type_arguments: Vec::new(),
                    specialization: CheckedGenericSpecialization::default(),
                },
                debug_kind: FunctionDebugKind::Named(name),
                source_definition: None,
                params,
                capture_count: 0,
                return_type: method.return_type,
                locals,
                body: Block { statements, span },
                span,
            });
        }
        functions
    }
}

pub(super) fn contains_erased_boundary(types: &TypeInterner, ty: TypeId) -> bool {
    match types.resolve(ty) {
        Type::Interface(_) => true,
        Type::List(inner)
        | Type::Set(inner)
        | Type::Optional(inner)
        | Type::Secret(inner)
        | Type::Refinement { base: inner, .. } => contains_erased_boundary(types, *inner),
        Type::Map(a, b) | Type::Result(a, b) => {
            contains_erased_boundary(types, *a) || contains_erased_boundary(types, *b)
        }
        Type::Function { .. } => true,
        _ => false,
    }
}

pub(super) fn representation_type(types: &TypeInterner, mut ty: TypeId) -> TypeId {
    while let Type::Secret(inner) | Type::Refinement { base: inner, .. } = types.resolve(ty) {
        ty = *inner;
    }
    ty
}

fn is_secret(types: &TypeInterner, mut ty: TypeId) -> bool {
    loop {
        match types.resolve(ty) {
            Type::Secret(_) => return true,
            Type::Refinement { base, .. } => ty = *base,
            _ => return false,
        }
    }
}

/// A contextual empty/absent value can fill only bottom slots. Rebuilding the
/// container is required even without interfaces: its element ownership flags
/// may change, and a nested callable still needs its checked adapter.
fn never_slots_compatible(types: &TypeInterner, source: TypeId, target: TypeId) -> bool {
    if source == target || source == TypeInterner::NEVER {
        return true;
    }
    match (types.resolve(source), types.resolve(target)) {
        (Type::List(source), Type::List(target))
        | (Type::Optional(source), Type::Optional(target))
        | (Type::Secret(source), Type::Secret(target)) => {
            never_slots_compatible(types, *source, *target)
        }
        (Type::Map(source_key, source_value), Type::Map(target_key, target_value))
        | (Type::Result(source_key, source_value), Type::Result(target_key, target_value)) => {
            never_slots_compatible(types, *source_key, *target_key)
                && never_slots_compatible(types, *source_value, *target_value)
        }
        (_, Type::Secret(target)) => never_slots_compatible(types, source, *target),
        _ => false,
    }
}

fn coerce(value: &mut Expression, expected: TypeId, types: &TypeInterner) {
    if value.ty == expected
        || value.ty == TypeInterner::NEVER
        // Pure calls retain secret arguments so their result stays tainted.
        || (is_secret(types, value.ty) && !is_secret(types, expected)
            && !matches!(types.resolve(expected), Type::Interface(_)))
        || !(contains_erased_boundary(types, expected)
            || contains_erased_boundary(types, value.ty)
            || never_slots_compatible(types, value.ty, expected)
            || representation_type(types, expected) == representation_type(types, value.ty))
    {
        return;
    }
    let placeholder = Expression {
        kind: ExpressionKind::Nothing,
        ty: expected,
        span: value.span,
    };
    let inner = std::mem::replace(value, placeholder);
    value.kind = ExpressionKind::interface_coerce(Box::new(inner));
}

pub(super) fn coerce_program(program: &mut Program, types: &TypeInterner) {
    let mut signatures = program
        .functions
        .iter()
        .map(|function| function.params.iter().map(|param| param.ty).collect())
        .collect::<Vec<Vec<_>>>();
    let adapters = std::cell::RefCell::new(Adapters {
        next: program.functions.len() as u32,
        ids: program
            .functions
            .iter()
            .filter_map(|function| {
                if !function
                    .identity
                    .declaration
                    .name
                    .starts_with("$interface.adapter.")
                    || function.capture_count != 1
                    || function.params.is_empty()
                {
                    return None;
                }
                let signature = Type::Function {
                    params: function.params[1..].iter().map(|param| param.ty).collect(),
                    view_params: function.params[1..]
                        .iter()
                        .map(|param| param.mode == ParamMode::View)
                        .collect(),
                    return_type: function.return_type,
                };
                let target = types
                    .type_ids()
                    .find(|ty| types.resolve(*ty) == &signature)?;
                Some(((function.params[0].ty, target), function.id))
            })
            .collect(),
        pending: Vec::new(),
    });
    let mut index = 0;
    while index < program.functions.len() {
        let function = &mut program.functions[index];
        let locals = function
            .locals
            .iter()
            .map(|local| local.ty)
            .collect::<Vec<_>>();
        let pass = Coercions {
            types,
            signatures: &signatures,
            locals: &locals,
            return_type: function.return_type,
            identity: &function.identity,
            adapters: &adapters,
        };
        pass.block(&mut function.body, None);
        let pending = std::mem::take(&mut adapters.borrow_mut().pending);
        signatures.extend(pending.iter().map(|function| {
            function
                .params
                .iter()
                .map(|param| param.ty)
                .collect::<Vec<_>>()
        }));
        program.functions.extend(pending);
        index += 1;
    }
}

/// Complete checked conversions introduced by generated values, such as explicit
/// comptime results and property inputs. Existing adapters are reused, and every
/// new adapter body passes through the same conversion and validation pipeline.
pub fn complete_value_conversions(
    program: &mut Program,
    types: &TypeInterner,
) -> Result<(), Vec<LowerError>> {
    coerce_program(program, types);
    validate(program)
        .and_then(|()| validate_backend_types(program, types))
        .map_err(|errors| {
            errors
                .into_iter()
                .map(|error| LowerError {
                    span: error.span,
                    message: error.message,
                })
                .collect()
        })
}

struct Adapters {
    next: u32,
    ids: HashMap<(TypeId, TypeId), FunctionId>,
    pending: Vec<Function>,
}

impl Adapters {
    fn function(
        &mut self,
        source: TypeId,
        target: TypeId,
        types: &TypeInterner,
        identity: &FunctionIdentity,
        span: Span,
    ) -> Option<FunctionId> {
        if let Some(id) = self.ids.get(&(source, target)) {
            return Some(*id);
        }
        let Type::Function {
            return_type: source_return,
            ..
        } = types.resolve(source)
        else {
            return None;
        };
        let Type::Function {
            params: target_params,
            view_params,
            return_type,
        } = types.resolve(target)
        else {
            return None;
        };
        let id = FunctionId(self.next);
        self.next += 1;
        let mut params = vec![Param {
            local: LocalId(0),
            name: "$source".into(),
            ty: source,
            mode: ParamMode::Owned,
            mutable: false,
            span,
        }];
        params.extend(target_params.iter().zip(view_params).enumerate().map(
            |(index, (ty, view))| Param {
                local: LocalId(index as u32 + 1),
                name: format!("$argument{index}"),
                ty: *ty,
                mode: if *view {
                    ParamMode::View
                } else {
                    ParamMode::Owned
                },
                mutable: false,
                span,
            },
        ));
        let locals = params
            .iter()
            .map(|param| Local {
                id: param.local,
                name: param.name.clone(),
                ty: param.ty,
                debug_ty: param.ty,
                debug_type_name: None,
                mutable: false,
                view_source: None,
                span,
            })
            .collect();
        let args = params[1..]
            .iter()
            .map(|param| Expression {
                kind: ExpressionKind::Local(param.local),
                ty: param.ty,
                span,
            })
            .collect::<Vec<_>>();
        let value = Expression {
            kind: ExpressionKind::IndirectCall {
                callee: Box::new(Expression {
                    kind: ExpressionKind::Local(LocalId(0)),
                    ty: source,
                    span,
                }),
                evaluation_order: (0..args.len()).collect(),
                args,
            },
            ty: *source_return,
            span,
        };
        let mut identity = identity.clone();
        identity.declaration.name =
            format!("$interface.adapter.{}.{}", source.index(), target.index());
        identity.declaration.kind = DeclarationKind::Function;
        self.pending.push(Function {
            id,
            identity,
            debug_kind: FunctionDebugKind::Inline,
            source_definition: None,
            params,
            capture_count: 1,
            return_type: *return_type,
            locals,
            body: Block {
                statements: vec![Statement {
                    kind: StatementKind::Return(Some(value)),
                    span,
                }],
                span,
            },
            span,
        });
        self.ids.insert((source, target), id);
        Some(id)
    }
}

struct Coercions<'a> {
    types: &'a TypeInterner,
    signatures: &'a [Vec<TypeId>],
    locals: &'a [TypeId],
    return_type: TypeId,
    identity: &'a FunctionIdentity,
    adapters: &'a std::cell::RefCell<Adapters>,
}

impl Coercions<'_> {
    fn expected(&self, value: &mut Expression, expected: TypeId, handled: Option<TypeId>) {
        self.expression(value, handled);
        coerce(value, expected, self.types);
        self.adapt(value);
    }
    fn adapt(&self, expression: &mut Expression) {
        let ExpressionKind::InterfaceCoerce { value, .. } = &expression.kind else {
            return;
        };
        let direct = self.adapters.borrow_mut().function(
            value.ty,
            expression.ty,
            self.types,
            self.identity,
            expression.span,
        );
        if let Some(function) = direct {
            expression.kind = ExpressionKind::FunctionAdapter {
                value: value.clone(),
                function,
            };
        } else if let ExpressionKind::InterfaceCoerce { value, adapters } = &mut expression.kind {
            self.container_adapters(value.ty, expression.ty, expression.span, adapters);
        }
    }
    fn container_adapters(
        &self,
        source: TypeId,
        target: TypeId,
        span: Span,
        output: &mut Vec<InterfaceFunctionAdapter>,
    ) {
        let source = representation_type(self.types, source);
        let target = representation_type(self.types, target);
        if source == target || source == TypeInterner::NEVER {
            return;
        }
        match (self.types.resolve(source), self.types.resolve(target)) {
            (Type::Function { .. }, Type::Function { .. }) => {
                if output
                    .iter()
                    .any(|entry| entry.source == source && entry.target == target)
                {
                    return;
                }
                if let Some(function) = self.adapters.borrow_mut().function(
                    source,
                    target,
                    self.types,
                    self.identity,
                    span,
                ) {
                    output.push(InterfaceFunctionAdapter {
                        source,
                        target,
                        function,
                    });
                }
            }
            (Type::List(a), Type::List(b)) | (Type::Optional(a), Type::Optional(b)) => {
                self.container_adapters(*a, *b, span, output);
            }
            (Type::Map(_, a), Type::Map(_, b)) => {
                self.container_adapters(*a, *b, span, output);
            }
            (Type::Result(a, b), Type::Result(c, d)) => {
                self.container_adapters(*a, *c, span, output);
                self.container_adapters(*b, *d, span, output);
            }
            _ => {}
        }
    }
    fn block(&self, block: &mut Block, handled: Option<TypeId>) {
        for statement in &mut block.statements {
            match &mut statement.kind {
                StatementKind::Let { local, value } => {
                    self.expected(value, self.locals[local.index() as usize], handled)
                }
                StatementKind::Assign { target, value } => {
                    self.expression(target, handled);
                    self.expected(value, target.ty, handled);
                }
                StatementKind::Return(Some(value)) => {
                    self.expected(value, self.return_type, handled)
                }
                StatementKind::HandleDefault(value) => {
                    self.expected(value, handled.unwrap_or(value.ty), handled)
                }
                StatementKind::Expression(value) => self.expression(value, handled),
                StatementKind::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    self.expression(condition, handled);
                    self.block(then_block, handled);
                    if let Some(block) = else_block {
                        self.block(block, handled);
                    }
                }
                StatementKind::While { condition, body } => {
                    self.expression(condition, handled);
                    self.block(body, handled);
                }
                StatementKind::For { iterable, body, .. } => {
                    self.expression(iterable, handled);
                    self.block(body, handled);
                }
                StatementKind::Match { scrutinee, arms } => {
                    self.expression(scrutinee, handled);
                    for arm in arms {
                        self.block(&mut arm.body, handled);
                    }
                }
                StatementKind::Assert { condition, message } => {
                    self.expression(condition, handled);
                    if let Some(value) = message {
                        self.expression(value, handled);
                    }
                }
                StatementKind::Breakpoint {
                    condition: Some(condition),
                    ..
                } => self.expression(condition, handled),
                StatementKind::Scope(block) => self.block(block, handled),
                StatementKind::ReflectedTypeDispatch { type_info, arms } => {
                    self.expression(type_info, handled);
                    for arm in arms {
                        self.block(&mut arm.body, handled);
                    }
                }
                _ => {}
            }
        }
    }
    fn machine_payloads(
        &self,
        ty: TypeId,
        state: StateId,
        values: &mut [Expression],
        handled: Option<TypeId>,
    ) {
        let machine = match self.types.resolve(ty) {
            Type::Machine(id) | Type::MachineState { machine: id, .. } => {
                self.types.resolve_machine(*id)
            }
            _ => return,
        };
        if let Some(state) = machine.states.get(state.index() as usize) {
            for (value, (_, expected)) in values.iter_mut().zip(&state.fields) {
                self.expected(value, *expected, handled);
            }
        }
    }
    fn expression(&self, expression: &mut Expression, handled: Option<TypeId>) {
        let ty = expression.ty;
        match &mut expression.kind {
            ExpressionKind::Call { function, args, .. } => {
                for (argument, expected) in args
                    .iter_mut()
                    .zip(&self.signatures[function.index() as usize])
                {
                    self.expected(argument, *expected, handled);
                }
            }
            ExpressionKind::IndirectCall { callee, args, .. } => {
                self.expression(callee, handled);
                if let Type::Function { params, .. } = self.types.resolve(callee.ty) {
                    for (argument, expected) in args.iter_mut().zip(params) {
                        self.expected(argument, *expected, handled);
                    }
                }
            }
            ExpressionKind::StructConstruct {
                struct_type,
                fields,
                refinement_predicates,
                ..
            } => {
                if let Type::Struct(id) = self.types.resolve(*struct_type) {
                    for (index, (value, (_, expected))) in fields
                        .iter_mut()
                        .zip(&self.types.resolve_struct(*id).fields)
                        .enumerate()
                    {
                        if refinement_predicates
                            .get(index)
                            .is_some_and(|chain| !chain.is_empty())
                        {
                            // MIR validates this ancestor before giving the
                            // field its refined type. It is not an erased box
                            // from which that refinement may be extracted.
                            self.expression(value, handled);
                        } else {
                            self.expected(value, *expected, handled);
                        }
                    }
                }
            }
            ExpressionKind::EnumConstruct {
                enum_type,
                variant,
                payloads,
                ..
            } => {
                if let Type::Enum(id) = self.types.resolve(*enum_type) {
                    for (value, (_, expected)) in payloads.iter_mut().zip(
                        &self.types.resolve_enum(*id).variants[variant.index() as usize].fields,
                    ) {
                        self.expected(value, *expected, handled);
                    }
                }
            }
            ExpressionKind::ListConstruct { elements } => {
                if let Type::List(inner) = self.types.resolve(ty) {
                    for value in elements {
                        self.expected(value, *inner, handled);
                    }
                }
            }
            ExpressionKind::MapConstruct { entries } => {
                if let Type::Map(key, value) = self.types.resolve(ty) {
                    for entry in entries {
                        self.expected(&mut entry.key, *key, handled);
                        self.expected(&mut entry.value, *value, handled);
                    }
                }
            }
            ExpressionKind::ResultOk(value) => {
                if let Type::Result(inner, _) = self.types.resolve(ty) {
                    self.expected(value, *inner, handled);
                }
            }
            ExpressionKind::ResultFail(value) => {
                if let Type::Result(_, inner) = self.types.resolve(ty) {
                    self.expected(value, *inner, handled);
                }
            }
            ExpressionKind::OptionalSome(value) => {
                if let Type::Optional(inner) = self.types.resolve(ty) {
                    self.expected(value, *inner, handled);
                }
            }
            ExpressionKind::Handle {
                target,
                kind,
                failure,
                ..
            } => {
                // The checker contextualizes the success shape, while the
                // original Local/Call keeps its exact producer signature.
                // Bare bottom success has no payload and stays visible to MIR;
                // inhabited containers need an explicit checked conversion.
                let contextual = match (kind, self.types.resolve(target.ty)) {
                    (HandleKind::Optional, Type::Optional(success))
                        if *success != ty && *success != TypeInterner::NEVER =>
                    {
                        Some(Type::Optional(ty))
                    }
                    (HandleKind::Result, Type::Result(success, error))
                        if *success != ty && *success != TypeInterner::NEVER =>
                    {
                        Some(Type::Result(ty, *error))
                    }
                    _ => None,
                }
                .and_then(|contextual| {
                    self.types
                        .type_ids()
                        .find(|ty| self.types.resolve(*ty) == &contextual)
                });
                if let Some(contextual) = contextual {
                    self.expected(target, contextual, handled);
                } else {
                    self.expression(target, handled);
                }
                self.block(failure, Some(ty));
            }
            ExpressionKind::Binary { left, right, .. } => {
                self.expression(left, handled);
                self.expression(right, handled);
            }
            ExpressionKind::Unary { value, .. }
            | ExpressionKind::Field { base: value, .. }
            | ExpressionKind::StateIs { value, .. }
            | ExpressionKind::Comptime { value, .. }
            | ExpressionKind::Declassify(value)
            | ExpressionKind::Coarsen(value)
            | ExpressionKind::RefinementValidated(value)
            | ExpressionKind::DisplayResult(value)
            | ExpressionKind::EquatableResult(value)
            | ExpressionKind::RuntimeFailureMessage(value)
            | ExpressionKind::FunctionAdapter { value, .. }
            | ExpressionKind::InterfaceCoerce { value, .. }
            | ExpressionKind::InterfaceType(value)
            | ExpressionKind::Run(value)
            | ExpressionKind::Join(value)
            | ExpressionKind::Cancel(value)
            | ExpressionKind::View(value)
            | ExpressionKind::Clone(value) => self.expression(value, handled),
            ExpressionKind::StringInterpolation(parts) => {
                for part in parts {
                    if let StringSegment::Value(value) = part {
                        self.expression(value, handled);
                    }
                }
            }
            ExpressionKind::Intrinsic { args, .. }
            | ExpressionKind::BitfieldConstruct { fields: args, .. } => {
                for value in args {
                    self.expression(value, handled);
                }
            }
            ExpressionKind::MachineConstruct {
                state_type,
                state,
                payloads,
            } => {
                self.machine_payloads(*state_type, *state, payloads, handled);
            }
            ExpressionKind::MachineTransition {
                source,
                state_type,
                target,
                payloads,
            } => {
                self.expression(source, handled);
                self.machine_payloads(*state_type, *target, payloads, handled);
            }
            ExpressionKind::ActorSpawn {
                args, constructor, ..
            } => {
                if let Some(constructor) = constructor {
                    for (value, expected) in args
                        .iter_mut()
                        .zip(&self.signatures[constructor.index() as usize])
                    {
                        self.expected(value, *expected, handled);
                    }
                } else if let Type::Actor(id) = self.types.resolve(ty) {
                    let actor = self.types.resolve_actor(*id);
                    for (value, (_, expected)) in args
                        .iter_mut()
                        .zip(actor.capability_params.iter().chain(&actor.state_fields))
                    {
                        self.expected(value, *expected, handled);
                    }
                }
            }
            ExpressionKind::ActorMessage {
                actor,
                handler,
                args,
                ..
            } => {
                self.expression(actor, handled);
                let signature = &self.signatures[handler.index() as usize];
                let offset = signature.len().saturating_sub(args.len());
                for (value, expected) in args.iter_mut().zip(&signature[offset..]) {
                    self.expected(value, *expected, handled);
                }
            }
            _ => {}
        }
        self.adapt(expression);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_container_conversion_is_directional_and_preserves_known_leaves() {
        let mut types = TypeInterner::new();
        let empty = types.intern(Type::List(TypeInterner::NEVER));
        let strings = types.intern(Type::List(TypeInterner::STRING));
        let optional_empty = types.intern(Type::Optional(empty));
        let optional_strings = types.intern(Type::Optional(strings));
        assert!(never_slots_compatible(&types, empty, strings));
        assert!(never_slots_compatible(
            &types,
            optional_empty,
            optional_strings
        ));
        assert!(!never_slots_compatible(&types, strings, empty));
        assert!(!never_slots_compatible(
            &types,
            TypeInterner::INT8,
            TypeInterner::INT64
        ));
        let empty_map = types.intern(Type::Map(TypeInterner::NEVER, TypeInterner::NEVER));
        let string_map = types.intern(Type::Map(TypeInterner::STRING, TypeInterner::STRING));
        assert!(never_slots_compatible(&types, empty_map, string_map));
        assert!(!never_slots_compatible(&types, string_map, empty_map));
        let secret_strings = types.intern(Type::Secret(strings));
        assert!(never_slots_compatible(&types, empty, secret_strings));
        assert!(!never_slots_compatible(&types, secret_strings, empty));
        let set_empty = types.intern(Type::Set(TypeInterner::NEVER));
        let set_strings = types.intern(Type::Set(TypeInterner::STRING));
        assert!(!never_slots_compatible(&types, set_empty, set_strings));
    }
}
