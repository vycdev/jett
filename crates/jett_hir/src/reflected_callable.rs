//! Constructor-only reflected callable evidence. Labels are diagnostic data;
//! custody comes from the immutable checked session and original typed tuple.

use super::*;

#[path = "reflected_callable_baked.rs"]
pub(crate) mod baked;
#[path = "reflected_callable_materialized.rs"]
mod materialized_source;
pub use baked::{has_reflected_callable_plans, materialize_checked_reflected_values};

#[derive(Debug, Clone)]
pub struct ReflectedCallablePlan {
    data: Arc<Plan>,
}

#[derive(Debug, Clone)]
struct Plan {
    checked: Arc<jett_typecheck::CheckedResourceProgram>,
    admission: jett_typecheck::CheckedReflectedCallablePlan,
    returned: ReflectedFieldPlan,
    span: Span,
    function: Option<FunctionId>,
    context: Option<FunctionIdentity>,
    seal: Option<Arc<Seal>>,
    materialized: bool,
    ordinal: Option<usize>,
    reference: Option<Arc<jett_comptime::value::ReflectedCallableValue>>,
}

#[derive(Debug)]
struct Seal {
    owner: FunctionId,
    getter: Expression,
    adapter: Function,
    predicates: Vec<Function>,
    sources: Arc<Vec<Function>>,
}

impl PartialEq for ReflectedCallablePlan {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }
}
impl Eq for ReflectedCallablePlan {}

impl ReflectedCallablePlan {
    pub(crate) fn new(
        checked: Arc<jett_typecheck::CheckedResourceProgram>,
        admission: jett_typecheck::CheckedReflectedCallablePlan,
        returned: ReflectedFieldPlan,
        span: Span,
        ordinal: Option<usize>,
    ) -> Self {
        Self {
            data: Arc::new(Plan {
                checked,
                admission,
                returned,
                span,
                function: None,
                context: None,
                seal: None,
                materialized: false,
                ordinal,
                reference: None,
            }),
        }
    }

    pub fn source_type(&self) -> TypeId {
        self.data.admission.source_type()
    }
    pub fn requested_type(&self) -> TypeId {
        self.data.admission.requested_type()
    }
    pub fn checked_plan(&self) -> &jett_typecheck::CheckedReflectedCallablePlan {
        &self.data.admission
    }
    pub fn return_plan(&self) -> &ReflectedFieldPlan {
        &self.data.returned
    }
    pub fn function(&self) -> FunctionId {
        self.data.function.unwrap_or(FunctionId(u32::MAX))
    }
    pub fn canonical_adapter(&self) -> Option<&Function> {
        self.data.seal.as_ref().map(|seal| &seal.adapter)
    }
    pub fn original_owner(&self) -> Option<FunctionId> {
        self.data.seal.as_ref().map(|seal| seal.owner)
    }
    pub fn original_getter(&self) -> Option<&Expression> {
        self.data.seal.as_ref().map(|seal| &seal.getter)
    }
    pub fn original_predicates(&self) -> &[Function] {
        self.data
            .seal
            .as_ref()
            .map_or(&[], |seal| seal.predicates.as_slice())
    }
    pub fn is_materialized(&self) -> bool {
        self.data.materialized
    }
    pub fn is_sealed(&self) -> bool {
        self.data.seal.is_some()
    }

    pub(crate) fn resolve_return(&mut self, functions: &[Function]) -> Result<(), String> {
        if self.data.materialized && self.data.function.is_none() {
            let returned = returned_plan(
                &self.data.checked,
                functions,
                ReflectedFieldRequirementPlan {
                    source_type: self.data.admission.source_return(),
                    requested_type: self.data.admission.requested_return(),
                    requirement: self.data.admission.return_requirement().clone(),
                },
            )?;
            Arc::make_mut(&mut self.data).returned = returned;
        }
        Ok(())
    }

    pub(crate) fn bind(&mut self, function: FunctionId, context: &FunctionIdentity) {
        let data = Arc::make_mut(&mut self.data);
        data.function = Some(function);
        data.context = Some(context.clone());
    }

    pub fn validate_types(&self, types: &TypeInterner) -> Result<(), &'static str> {
        let original = self.data.checked.checked();
        if types.len() < original.interner.len() {
            return Err("reflected callable lost its original checked type table");
        }
        for ty in original.interner.type_ids() {
            let expected = original.interner.resolve(ty);
            if types.resolve(ty) != expected
                || types.nominal_type_arguments(ty) != original.interner.nominal_type_arguments(ty)
                || !types.contains_nominal_definition(expected)
            {
                return Err("reflected callable original type meaning changed");
            }
            let same = match expected {
                Type::Struct(id) => {
                    types.resolve_struct(*id) == original.interner.resolve_struct(*id)
                }
                Type::Enum(id) => types.resolve_enum(*id) == original.interner.resolve_enum(*id),
                Type::Bitfield(id) => {
                    types.resolve_bitfield(*id) == original.interner.resolve_bitfield(*id)
                }
                Type::Interface(id) => {
                    types.resolve_interface(*id) == original.interner.resolve_interface(*id)
                }
                Type::Actor(id) => types.resolve_actor(*id) == original.interner.resolve_actor(*id),
                Type::Machine(id) | Type::MachineState { machine: id, .. } => {
                    types.resolve_machine(*id) == original.interner.resolve_machine(*id)
                }
                _ => true,
            };
            if !same {
                return Err("reflected callable original nominal definition changed");
            }
        }
        if !self.data.admission.validate(original)
            || self.source_type().index() as usize >= types.len()
            || self.requested_type().index() as usize >= types.len()
            || types.resolve(self.source_type()) != original.interner.resolve(self.source_type())
            || types.resolve(self.requested_type())
                != original.interner.resolve(self.requested_type())
            || self.data.returned.source_type != self.data.admission.source_return()
            || self.data.returned.requested_type != self.data.admission.requested_return()
        {
            return Err("reflected callable admission differs from its checked session");
        }
        let expected = ReflectedFieldRequirementPlan {
            source_type: self.data.returned.source_type,
            requested_type: self.data.returned.requested_type,
            requirement: self.data.admission.return_requirement().clone(),
        };
        super::reflected_fields::validate_requirement_node(types, &expected, &self.data.returned)
    }

    pub fn validate_original(&self) -> bool {
        let Some(seal) = &self.data.seal else {
            return false;
        };
        self.validate_types(&self.data.checked.checked().interner)
            .is_ok()
            && self.data.function == Some(seal.adapter.id)
            && self.data.context.as_ref().is_some_and(|context| {
                seal.adapter.identity.scoped_type_bindings == context.scoped_type_bindings
                    && seal.adapter.identity.type_arguments == context.type_arguments
                    && seal.adapter.identity.specialization == context.specialization
                    && seal.adapter.identity.declaration.origin == context.declaration.origin
                    && seal.adapter.identity.declaration.namespace == context.declaration.namespace
            })
    }

    pub(crate) fn validate_current(
        &self,
        program: &Program,
        owner: &Function,
        getter: &Expression,
        ordinal: Option<usize>,
    ) -> Result<(), &'static str> {
        let seal = self
            .data
            .seal
            .as_ref()
            .ok_or("reflected callable has no original constructor seal")?;
        if !self.validate_original()
            || seal.owner != owner.id
            || self.data.ordinal != ordinal
            || self.data.context.as_ref() != Some(&owner.identity)
            || getter.span != self.data.span
            || !super::resource_materialization::expression_equal(
                &erase_callable(getter),
                &seal.getter,
            )
            || !program
                .functions
                .get(self.function().index() as usize)
                .is_some_and(|function| {
                    super::resource_materialization::functions_equal(
                        std::slice::from_ref(&erase_adapter(function)),
                        std::slice::from_ref(&seal.adapter),
                    )
                })
            || seal.predicates.iter().any(|expected| {
                !program
                    .functions
                    .get(expected.id.index() as usize)
                    .is_some_and(|current| {
                        let Ok(current) = original_function(
                            current,
                            &program.resource_source,
                            &self.data.checked,
                        ) else {
                            return false;
                        };
                        let Ok(expected) = original_function(
                            expected,
                            &program.resource_source,
                            &self.data.checked,
                        ) else {
                            return false;
                        };
                        super::resource_materialization::functions_equal(
                            std::slice::from_ref(&current),
                            std::slice::from_ref(&expected),
                        )
                    })
            })
            || !sources_match(&seal.sources, program, &self.data.checked)
        {
            return Err("reflected callable getter, adapter, predicate or context changed");
        }
        if program.resource_source.checked_program().is_some()
            && !program.resource_source.belongs_to(&self.data.checked)
        {
            return Err("reflected callable belongs to another original checked session");
        }
        let context = self
            .data
            .context
            .as_ref()
            .ok_or("reflected callable context is missing")?;
        let expected = adapter(
            self,
            self.function(),
            context,
            &self.data.checked.checked().interner,
        )
        .ok_or("reflected callable adapter cannot be reconstructed")?;
        let mut reconstruction = program.clone();
        reconstruction.functions[self.function().index() as usize] = expected;
        super::interface_values::coerce_program(
            &mut reconstruction,
            &self.data.checked.checked().interner,
        );
        if !super::resource_materialization::functions_equal(
            std::slice::from_ref(&erase_adapter(
                &reconstruction.functions[self.function().index() as usize],
            )),
            std::slice::from_ref(&seal.adapter),
        ) {
            return Err("reflected callable canonical body, tuple or return conversion changed");
        }
        Ok(())
    }
}

fn erase_callable(expression: &Expression) -> Expression {
    let mut erased = expression.clone();
    if let ExpressionKind::Intrinsic {
        field_validation: Some(ReflectedFieldValidation::Validate(plans)),
        ..
    } = &mut erased.kind
    {
        for plan in plans {
            if matches!(plan.action, ReflectedFieldAction::CallableReturn(_)) {
                plan.action = ReflectedFieldAction::Exact;
            }
        }
    }
    erased
}

pub(crate) fn adapter(
    plan: &ReflectedCallablePlan,
    id: FunctionId,
    context: &FunctionIdentity,
    types: &TypeInterner,
) -> Option<Function> {
    plan.validate_types(types).ok()?;
    let Type::Function {
        params: source_params,
        view_params: source_views,
        return_type: source_return,
    } = types.resolve(plan.source_type())
    else {
        return None;
    };
    let Type::Function {
        params: target_params,
        view_params: target_views,
        return_type: target_return,
    } = types.resolve(plan.requested_type())
    else {
        return None;
    };
    let span = plan.data.span;
    let mut params = vec![Param {
        local: LocalId(0),
        name: "$source".into(),
        ty: plan.source_type(),
        mode: ParamMode::Owned,
        mutable: false,
        span,
    }];
    params.extend(
        target_params
            .iter()
            .zip(target_views)
            .enumerate()
            .map(|(index, (ty, view))| Param {
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
            }),
    );
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
    let mut args = params[1..]
        .iter()
        .map(|param| Expression {
            kind: ExpressionKind::Local(param.local),
            ty: param.ty,
            span,
        })
        .collect::<Vec<_>>();
    for (argument, expected) in args.iter_mut().zip(source_params) {
        super::interface_values::coerce(argument, *expected, types);
        if argument.ty != *expected {
            return None;
        }
    }
    let access = source_views
        .iter()
        .map(|view| {
            if *view {
                CheckedCalleeAccess::View
            } else {
                CheckedCalleeAccess::Owned
            }
        })
        .collect::<Vec<_>>();
    let order = (0..args.len()).collect::<Vec<_>>();
    let ownership = CallOwnership::generated(
        GeneratedOperation::ReflectedCallableAdapter {
            callee: LocalId(0),
            plan: plan.checked_plan().clone(),
        },
        &args,
        source_params,
        &access,
        *source_return,
        &order,
        types,
    )
    .ok()?;
    let invoked = Expression {
        kind: ExpressionKind::IndirectCall {
            ownership,
            callee: Box::new(Expression {
                kind: ExpressionKind::Local(LocalId(0)),
                ty: plan.source_type(),
                span,
            }),
            args,
            evaluation_order: order,
        },
        ty: *source_return,
        span,
    };
    let returned = Expression {
        kind: ExpressionKind::Handle {
            target: Box::new(invoked),
            kind: HandleKind::ReflectedReturn {
                plan: Box::new(plan.return_plan().clone()),
                callable: None,
            },
            error_local: None,
            failure: Block {
                statements: Vec::new(),
                span,
            },
        },
        ty: *target_return,
        span,
    };
    let mut identity = context.clone();
    identity.declaration.name = format!("$reflected.callable.{}", id.index());
    identity.declaration.kind = DeclarationKind::Function;
    Some(Function {
        id,
        identity,
        debug_kind: FunctionDebugKind::Inline,
        source_definition: None,
        params,
        capture_count: 1,
        return_type: *target_return,
        locals,
        body: Block {
            statements: vec![Statement {
                kind: StatementKind::Return(Some(returned)),
                span,
            }],
            span,
        },
        span,
    })
}

pub(crate) fn seal_program(program: &mut Program) -> Result<(), Vec<LowerError>> {
    let originals = program.functions.clone();
    let sources = Arc::new(originals.iter().map(erase_function).collect::<Vec<_>>());
    let mut attached = Vec::new();
    for owner in &mut program.functions {
        let identity = owner.identity.clone();
        let owner_id = owner.id;
        super::resource_materialization::walk_block(&mut owner.body, &mut |expression| {
            let getter = erase_callable(expression);
            if let ExpressionKind::Intrinsic {
                field_validation: Some(ReflectedFieldValidation::Validate(plans)),
                ..
            } = &mut expression.kind
            {
                for (ordinal, plan) in plans.iter_mut().enumerate() {
                    if let ReflectedFieldAction::CallableReturn(callable) = &mut plan.action {
                        if callable.is_sealed() {
                            continue;
                        }
                        if callable.data.ordinal != Some(ordinal) {
                            return Err("reflected callable selected field ordinal changed".into());
                        }
                        let adapter = originals
                            .get(callable.function().index() as usize)
                            .ok_or("reflected callable adapter is missing")?
                            .clone();
                        let predicates = callable
                            .return_plan()
                            .predicates()
                            .into_iter()
                            .map(|predicate| {
                                originals
                                    .get(predicate.function.index() as usize)
                                    .cloned()
                                    .ok_or("reflected callable predicate is missing")
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let data = Arc::make_mut(&mut callable.data);
                        if data.context.as_ref() != Some(&identity) {
                            return Err("reflected callable constructor context changed".into());
                        }
                        data.seal = Some(Arc::new(Seal {
                            owner: owner_id,
                            getter: getter.clone(),
                            adapter,
                            predicates,
                            sources: sources.clone(),
                        }));
                        attached.push((callable.function(), callable.clone()));
                    }
                }
            }
            Ok(())
        })
        .map_err(|message| {
            vec![LowerError {
                span: owner.span,
                message,
            }]
        })?;
    }
    for function in &originals {
        let mut body = function.body.clone();
        super::resource_materialization::walk_block(&mut body, &mut |expression| {
            if let ExpressionKind::Handle { kind: HandleKind::ReflectedReturn { callable: Some(callable), .. }, .. } = &expression.kind {
                if !callable.is_materialized() || callable.is_sealed() { return Ok(()); }
                let mut callable = callable.clone();
                let context = callable.data.context.clone().ok_or("materialized reflected callable lacks context")?;
                let mut sites = Vec::new();
                for owner in &originals {
                    let mut owner_body = owner.body.clone();
                    super::resource_materialization::walk_block(&mut owner_body, &mut |value| {
                        if matches!(value.kind, ExpressionKind::FunctionAdapter { function: id, .. } if id == callable.function()) {
                            sites.push((owner.id, owner.identity.clone(), value.clone()));
                        }
                        Ok(())
                    })?;
                }
                if sites.len() != 1 || sites[0].1 != context { return Err("materialized reflected callable has no exact construction site".into()); }
                let predicates = callable.return_plan().predicates().into_iter().map(|predicate| originals.get(predicate.function.index() as usize).cloned().ok_or("materialized reflected predicate is missing")).collect::<Result<Vec<_>, _>>()?;
                let reference = callable.data.reference.as_ref().ok_or("materialized reflected callable lost read provenance")?;
                if !program.resource_source.belongs_to(reference.checked_program()) { return Err("materialized reflected callable lacks its original checked HIR archive".into()); }
                if !baked::owns_materialized_owner(&program.resource_source, &originals[sites[0].0.index() as usize]) {
                    return Err("materialized reflected callable has no protected checked required-value construction site".into());
                }
                for expected in &predicates {
                    let original = program.resource_source.functions().get(expected.id.index() as usize).ok_or("materialized predicate is absent from original archive")?;
                    let expected = original_function(expected,&program.resource_source,reference.checked_program())?;
                    if !super::resource_materialization::functions_equal(std::slice::from_ref(original), std::slice::from_ref(&expected)) {
                        return Err("materialized reflected predicate body differs from original checked Source".into());
                    }
                }
                materialized_source::validate(reference, &sites[0].2, &originals[sites[0].0.index() as usize], &originals, &program.resource_source)?;
                Arc::make_mut(&mut callable.data).seal = Some(Arc::new(Seal { owner: sites[0].0, getter: sites[0].2.clone(), adapter: erase_adapter(function), predicates, sources: sources.clone() }));
                attached.push((function.id, callable));
            }
            Ok(())
        }).map_err(|message| vec![LowerError { span: function.span, message }])?;
    }
    for (id, callable) in attached {
        let function = &mut program.functions[id.index() as usize];
        if let StatementKind::Return(Some(Expression {
            kind:
                ExpressionKind::Handle {
                    kind:
                        HandleKind::ReflectedReturn {
                            callable: stored, ..
                        },
                    ..
                },
            ..
        })) = &mut function.body.statements[0].kind
        {
            *stored = Some(callable);
        }
    }
    Ok(())
}

fn erase_adapter(function: &Function) -> Function {
    let mut function = function.clone();
    let _ = super::resource_materialization::walk_block(&mut function.body, &mut |expression| {
        if let ExpressionKind::Handle {
            kind: HandleKind::ReflectedReturn { callable, .. },
            ..
        } = &mut expression.kind
        {
            *callable = None;
        }
        Ok(())
    });
    function
}

fn erase_function(function: &Function) -> Function {
    let mut function = erase_adapter(function);
    let _ = super::resource_materialization::walk_block(&mut function.body, &mut |expression| {
        *expression = erase_callable(expression);
        Ok(())
    });
    function
}

pub(crate) fn validate_program(program: &Program) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    let mut authorized = HashMap::new();
    for owner in &program.functions {
        let mut body = owner.body.clone();
        let _ = super::resource_materialization::walk_block(&mut body, &mut |getter| {
            if let ExpressionKind::Intrinsic {
                field_validation: Some(ReflectedFieldValidation::Validate(plans)),
                ..
            } = &getter.kind
            {
                for (ordinal, plan) in plans.iter().enumerate() {
                    if let ReflectedFieldAction::CallableReturn(callable) = &plan.action {
                        if let Err(message) =
                            callable.validate_current(program, owner, getter, Some(ordinal))
                        {
                            errors.push(ValidationError {
                                span: getter.span,
                                message: message.into(),
                            });
                        } else {
                            authorized.insert(callable.function(), callable.clone());
                        }
                    }
                }
            }
            if let ExpressionKind::FunctionAdapter { function, .. } = &getter.kind {
                if let Some(adapter) = program.functions.get(function.index() as usize) {
                    if let Some(Statement {
                        kind:
                            StatementKind::Return(Some(Expression {
                                kind:
                                    ExpressionKind::Handle {
                                        kind:
                                            HandleKind::ReflectedReturn {
                                                callable: Some(callable),
                                                ..
                                            },
                                        ..
                                    },
                                ..
                            })),
                        ..
                    }) = adapter.body.statements.first()
                    {
                        if callable.is_materialized() {
                            if let Err(message) =
                                callable.validate_current(program, owner, getter, None)
                            {
                                errors.push(ValidationError {
                                    span: getter.span,
                                    message: message.into(),
                                });
                            } else {
                                authorized.insert(*function, callable.clone());
                            }
                        }
                    }
                }
            }
            Ok(())
        });
    }
    for function in &program.functions {
        let mut body = function.body.clone();
        let _ = super::resource_materialization::walk_block(&mut body, &mut |expression| {
            if let ExpressionKind::Handle {
                kind: HandleKind::ReflectedReturn { plan, callable },
                ..
            } = &expression.kind
            {
                if !callable.as_ref().is_some_and(|callable| {
                    authorized.get(&function.id) == Some(callable)
                        && callable.function() == function.id
                        && callable.return_plan() == plan.as_ref()
                }) {
                    errors.push(ValidationError {
                        span: expression.span,
                        message: "reflected callable return has no authenticated original getter"
                            .into(),
                    });
                }
            }
            if let ExpressionKind::IndirectCall {
                ownership: CallOwnership::Generated(generated),
                ..
            } = &expression.kind
            {
                if let GeneratedOperation::ReflectedCallableAdapter { plan, .. } =
                    &generated.operation
                {
                    if !authorized
                        .get(&function.id)
                        .is_some_and(|callable| callable.checked_plan() == plan)
                    {
                        errors.push(ValidationError {
                            span: expression.span,
                            message:
                                "reflected callable source invocation has no authenticated adapter"
                                    .into(),
                        });
                    }
                }
            }
            Ok(())
        });
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Stage an already admitted, owned reflected callable for ordinary conversion
/// completion. This constructs no callback result and runs no predicate.
pub fn lower_reflected_callable_value(
    value: Expression,
    admitted: &jett_comptime::value::ReflectedCallableValue,
    span: Span,
) -> Result<Expression, String> {
    let checked = admitted.checked_program().clone();
    let admission = admitted.checked_plan().clone();
    if value.ty != admission.source_type() || !admission.validate(checked.checked()) {
        return Err("materialized reflected callable has no exact checked admission".into());
    }
    let source = admission.source_type();
    let target = admission.requested_type();
    let placeholder = ReflectedFieldPlan {
        source_type: admission.source_return(),
        requested_type: admission.requested_return(),
        action: ReflectedFieldAction::Exact,
    };
    let mut callable = ReflectedCallablePlan::new(checked, admission, placeholder, span, None);
    Arc::make_mut(&mut callable.data).materialized = true;
    Arc::make_mut(&mut callable.data).reference = Some(Arc::new(admitted.clone()));
    Ok(Expression {
        kind: ExpressionKind::InterfaceCoerce {
            value: Box::new(value),
            adapters: vec![InterfaceFunctionAdapter {
                source,
                target,
                function: FunctionId(u32::MAX),
                reflected: Some(callable),
            }],
        },
        ty: target,
        span,
    })
}

fn sources_match(
    expected: &[Function],
    program: &Program,
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
) -> bool {
    expected.iter().all(|expected| {
        program
            .functions
            .get(expected.id.index() as usize)
            .is_some_and(|current| {
                let Ok(current) = original_function(current, &program.resource_source, checked)
                else {
                    return false;
                };
                let Ok(expected) = original_function(expected, &program.resource_source, checked)
                else {
                    return false;
                };
                super::resource_materialization::functions_equal(
                    std::slice::from_ref(&erase_function(&current)),
                    std::slice::from_ref(&erase_function(&expected)),
                )
            })
    })
}

/// Undo only protected checked replacements. Public edited HIR cannot create a
/// row or renew the seal, and each row must still occupy its exact owner/site.
fn original_function(
    function: &Function,
    archive: &ResourceSourceArchive,
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
) -> Result<Function, String> {
    if let Some(original) = baked::original_function(function, archive, checked)? {
        return Ok(original);
    }
    let mut original = function.clone();
    for row in archive
        .required_materializations()
        .iter()
        .filter(|row| row.function() == function.id)
    {
        if !archive.belongs_to(checked)
            || !row.proof.belongs_to(checked)
            || row.identity() != &function.identity
        {
            return Err(
                "reflected callable required replacement has no original checked owner".into(),
            );
        }
        let mut matches = 0usize;
        super::resource_materialization::walk_block(&mut original.body, &mut |expression| {
            if super::resource_materialization::expression_equal(expression, row.current()) {
                matches += 1;
                *expression = row.original().clone();
            } else if super::resource_materialization::expression_equal(expression, row.original())
            {
                matches += 1;
            }
            Ok(())
        })?;
        if matches != 1 {
            return Err("reflected callable required replacement moved or was duplicated".into());
        }
    }
    Ok(original)
}

fn returned_plan(
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
    functions: &[Function],
    node: ReflectedFieldRequirementPlan,
) -> Result<ReflectedFieldPlan, String> {
    use ReflectedFieldAction as A;
    use ReflectedFieldRequirement as R;
    let predicates = |ids: Vec<TypeId>| -> Result<Vec<RefinementPredicate>, String> {
        ids.into_iter()
            .map(|refined_type| {
                let types = &checked.checked().interner;
                let Type::Refinement { name, .. } = types.resolve(refined_type) else {
                    return Err("materialized return predicate is not a refinement".into());
                };
                let (base_type, _) = super::reflected_fields::refinement_chain(types, refined_type)
                    .ok_or("invalid materialized return chain")?;
                let input_type = match types.resolve(base_type) {
                    Type::Secret(inner) => *inner,
                    _ => base_type,
                };
                let mut matches = functions.iter().filter(|function| {
                    function.source_definition.is_some_and(|definition| {
                        checked.checked().definition_types.get(&definition) == Some(&refined_type)
                    }) && refinement_predicate_declaration_matches(
                        &function.identity.declaration,
                        name,
                    ) && function.params.len() == 1
                        && function.params[0].ty == input_type
                        && function.params[0].mode == ParamMode::Owned
                        && function.capture_count == 0
                        && function.return_type == TypeInterner::BOOL
                });
                let function = matches
                    .next()
                    .ok_or("materialized return has no checked predicate declaration")?;
                if matches.next().is_some() {
                    return Err("materialized return predicate context is ambiguous".into());
                }
                Ok(RefinementPredicate {
                    refined_type,
                    type_name: name.clone(),
                    function: function.id,
                    base_type,
                    input_type,
                })
            })
            .collect()
    };
    let action = match node.requirement {
        R::Exact => A::Exact,
        R::Predicates(ids) => A::Predicates(predicates(ids)?),
        R::Unsupported(reason) => A::Unsupported(reason),
        R::Refine {
            base,
            predicates: ids,
        } => A::Refine {
            base: Box::new(returned_plan(checked, functions, *base)?),
            predicates: predicates(ids)?,
        },
        R::List(child) => A::List(Box::new(returned_plan(checked, functions, *child)?)),
        R::Set(child) => A::Set(Box::new(returned_plan(checked, functions, *child)?)),
        R::Optional(child) => A::Optional(Box::new(returned_plan(checked, functions, *child)?)),
        R::Map { key, value } => A::Map {
            key: Box::new(returned_plan(checked, functions, *key)?),
            value: Box::new(returned_plan(checked, functions, *value)?),
        },
        R::Result { ok, error } => A::Result {
            ok: Box::new(returned_plan(checked, functions, *ok)?),
            error: Box::new(returned_plan(checked, functions, *error)?),
        },
    };
    Ok(ReflectedFieldPlan {
        source_type: node.source_type,
        requested_type: node.requested_type,
        action,
    })
}

pub fn reflected_callable_plans(function: &Function) -> Vec<ReflectedCallablePlan> {
    let mut plans = Vec::new();
    let mut body = function.body.clone();
    let _ = super::resource_materialization::walk_block(&mut body, &mut |expression| {
        if let ExpressionKind::Intrinsic {
            field_validation: Some(ReflectedFieldValidation::Validate(fields)),
            ..
        } = &expression.kind
        {
            for field in fields {
                if let ReflectedFieldAction::CallableReturn(plan) = &field.action {
                    plans.push(plan.clone());
                }
            }
        }
        if let ExpressionKind::Handle {
            kind:
                HandleKind::ReflectedReturn {
                    callable: Some(plan),
                    ..
                },
            ..
        } = &expression.kind
        {
            plans.push(plan.clone());
        }
        Ok(())
    });
    plans
}

#[cfg(test)]
#[path = "reflected_callable_tests.rs"]
mod tests;
