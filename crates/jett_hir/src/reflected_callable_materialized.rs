//! Match materialized descriptors and capture values without invoking Source.
use super::*;
use jett_comptime::Value;
use std::cell::RefCell;

pub(super) fn validate(
    reference: &jett_comptime::value::ReflectedCallableValue,
    site: &Expression,
    owner: &Function,
    functions: &[Function],
    archive: &ResourceSourceArchive,
) -> Result<(), String> {
    let ExpressionKind::FunctionAdapter { value, .. } = &site.kind else {
        return Err("materialized reflected callable has no source descriptor".into());
    };
    if !archive.belongs_to(reference.checked_program()) || value.ty != reference.source_type() {
        return Err(
            "materialized reflected callable source belongs to another checked context".into(),
        );
    }
    let context = Context {
        owner,
        functions,
        archive,
        checked: reference.checked_program(),
        used: RefCell::new(HashSet::new()),
    };
    if !context.matches(
        reference.source(),
        value,
        context.checked.checked().interner.len(),
    ) {
        return Err("materialized source callback, captures or pending layers differ from the admitted read".into());
    }
    let mut descriptor = value.as_ref();
    while let ExpressionKind::Run(inner) = &descriptor.kind {
        descriptor = inner;
    }
    let id = match &descriptor.kind {
        ExpressionKind::FunctionRef(id) | ExpressionKind::ClosureRef { function: id, .. } => {
            Some(*id)
        }
        ExpressionKind::FunctionAdapter { .. } => None,
        _ => return Err("materialized callback lost its original descriptor".into()),
    };
    if let Some(id) = id {
        let original = archive
            .functions()
            .get(id.index() as usize)
            .ok_or("materialized callback target is absent")?;
        if (reference.source_definition().is_some()
            && original.source_definition != reference.source_definition())
            || reference
                .source_body()
                .is_some_and(|body| original.body.span != body.span)
        {
            return Err("materialized callback selects another original source declaration".into());
        }
    }
    Ok(())
}

struct Context<'a> {
    owner: &'a Function,
    functions: &'a [Function],
    archive: &'a ResourceSourceArchive,
    checked: &'a Arc<jett_typecheck::CheckedResourceProgram>,
    used: RefCell<HashSet<LocalId>>,
}

pub(super) fn validate_required_values(
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
    archive: &ResourceSourceArchive,
    owner: &Function,
    functions: &[Function],
    replacements: &[(jett_comptime::CheckedRequiredValue, Expression)],
) -> Result<HashSet<LocalId>, String> {
    let context = Context {
        owner,
        functions,
        archive,
        checked,
        used: RefCell::new(HashSet::new()),
    };
    for (proof, current) in replacements {
        if !proof.belongs_to(checked)
            || !context.matches(proof.value(), current, checked.checked().interner.len())
        {
            return Err(
                "ordinary required replacement differs from its opaque checked value".into(),
            );
        }
    }
    Ok(context.used.into_inner())
}

impl Context<'_> {
    fn target(&self, id: FunctionId, ty: TypeId) -> Option<&Function> {
        let current = self.functions.get(id.index() as usize)?;
        let original = self.archive.functions().get(id.index() as usize)?;
        let current = original_function(current, self.archive, self.checked).ok()?;
        if !super::resource_materialization::functions_equal(
            std::slice::from_ref(&current),
            std::slice::from_ref(original),
        ) {
            return None;
        }
        let signature = Type::Function {
            params: original.params[original.capture_count..]
                .iter()
                .map(|param| param.ty)
                .collect(),
            view_params: original.params[original.capture_count..]
                .iter()
                .map(|param| param.mode == ParamMode::View)
                .collect(),
            return_type: original.return_type,
        };
        (self.checked.checked().interner.resolve(ty) == &signature).then_some(original)
    }

    fn binding(&self, local: LocalId) -> Option<Expression> {
        let metadata = self.owner.locals.get(local.index() as usize)?;
        if metadata.id != local || metadata.mutable || metadata.view_source.is_some() {
            return None;
        }
        let mut values = Vec::new();
        bindings(&self.owner.body, local, &mut values);
        let [value] = values.as_slice() else {
            return None;
        };
        self.used.borrow_mut().insert(local);
        (value.ty == metadata.ty).then(|| value.clone())
    }

    fn matches(&self, value: &Value, expression: &Expression, budget: usize) -> bool {
        if budget == 0 || expression.ty.index() as usize >= self.checked.checked().interner.len() {
            return false;
        }
        let child =
            |value: &Value, expression: &Expression| self.matches(value, expression, budget - 1);
        let types = &self.checked.checked().interner;
        use ExpressionKind as E;
        if let E::Clone(inner) = &expression.kind {
            return child(value, inner);
        }
        if let E::Local(local) = &expression.kind {
            return self
                .binding(*local)
                .is_some_and(|binding| child(value, &binding));
        }
        if let Value::Typed { type_name, value } = value {
            if let E::RefinementValidated(inner) = &expression.kind {
                return types.type_name(expression.ty) == *type_name && child(value, inner);
            }
            return child(value, expression);
        }
        if let Value::Pending(value) = value {
            return matches!(&expression.kind, E::Run(inner) if inner.ty == expression.ty && child(value, inner));
        }
        if let E::Run(_) = &expression.kind {
            return false;
        }
        if let E::RefinementValidated(inner) = &expression.kind {
            return matches!(types.resolve(expression.ty), Type::Refinement { base, .. } if *base == inner.ty)
                && child(value, inner);
        }
        if let E::InterfaceCoerce {
            value: inner,
            adapters,
        } = &expression.kind
        {
            if let Value::ReflectedCallable(admitted) = value {
                return adapters.len() == 1
                    && adapters[0].reflected.as_ref().is_some_and(|plan| {
                        plan.is_materialized()
                            && plan.checked_plan() == admitted.checked_plan()
                            && Arc::ptr_eq(&plan.data.checked, admitted.checked_program())
                            && plan.data.reference.is_some()
                            && expression.ty == admitted.requested_type()
                            && inner.ty == admitted.source_type()
                            && child(admitted.source(), inner)
                    });
            }
            return adapters.is_empty()
                && child(value, inner)
                && self
                    .checked
                    .checked()
                    .ordinary_types_compatible(expression.ty, inner.ty);
        }
        match (value, &expression.kind) {
            (Value::Int64(a), E::Int(b)) => i128::from(*a) == *b,
            (Value::Uint64(a), E::Int(b)) => i128::from(*a) == *b,
            (Value::Float64(a), E::Float(b)) => a.to_bits() == b.to_bits(),
            (Value::String(a), E::String(b)) => a == b,
            (Value::Bool(a), E::Bool(b)) => a == b,
            (Value::Nothing, E::Nothing) | (Value::OptionalNone, E::OptionalNone) => true,
            (Value::OptionalSome(a), E::OptionalSome(b))
            | (Value::ResultOk(a), E::ResultOk(b))
            | (Value::ResultFail(a), E::ResultFail(b)) => child(a, b),
            (Value::List(a), E::ListConstruct { elements: b }) => {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| child(a, b))
            }
            (Value::Map(a), E::MapConstruct { entries: b }) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|((ak, av), b)| child(ak, &b.key) && child(av, &b.value))
            }
            (
                Value::Struct {
                    type_name,
                    concrete_type,
                    fields,
                },
                E::StructConstruct {
                    struct_type,
                    fields: current,
                    evaluation_order,
                    validates_refinements,
                    refinement_predicates,
                },
            ) => {
                let Type::Struct(id) = types.resolve(*struct_type) else {
                    return false;
                };
                let definition = types.resolve_struct(*id);
                *struct_type == expression.ty
                    && !validates_refinements
                    && refinement_predicates.is_empty()
                    && concrete_type.as_ref().unwrap_or(type_name) == &types.type_name(*struct_type)
                    && *evaluation_order == (0..current.len()).collect::<Vec<_>>()
                    && fields.len() == current.len()
                    && definition.fields.len() == current.len()
                    && definition
                        .fields
                        .iter()
                        .zip(current)
                        .all(|((name, ty), current)| {
                            current.ty == *ty
                                && fields
                                    .iter()
                                    .find(|(field, _)| field == name)
                                    .is_some_and(|(_, value)| child(value, current))
                        })
            }
            (
                Value::Enum {
                    type_name,
                    variant,
                    fields,
                },
                E::EnumConstruct {
                    enum_type,
                    variant: current,
                    payloads,
                    evaluation_order,
                },
            ) => {
                let Type::Enum(id) = types.resolve(*enum_type) else {
                    return false;
                };
                let Some(definition) = types
                    .resolve_enum(*id)
                    .variants
                    .get(current.index() as usize)
                else {
                    return false;
                };
                *enum_type == expression.ty
                    && *type_name == types.type_name(*enum_type)
                    && definition.name == *variant
                    && fields.len() == payloads.len()
                    && definition.fields.len() == payloads.len()
                    && *evaluation_order == (0..payloads.len()).collect::<Vec<_>>()
                    && fields
                        .iter()
                        .zip(payloads)
                        .zip(&definition.fields)
                        .all(|((a, b), (_, ty))| b.ty == *ty && child(a, b))
            }
            (Value::NamedFunction(name), E::FunctionRef(id)) => {
                let Some(original) = self.target(*id, expression.ty) else {
                    return false;
                };
                let Some(definition) = original.source_definition else {
                    return false;
                };
                let info = self.checked.resolved().scope_table.def(definition);
                original.capture_count == 0
                    && info.kind == jett_resolve::DefKind::Function
                    && info.name == *name
            }
            (Value::Function { body, captures, .. }, E::FunctionRef(id))
            | (Value::Function { body, captures, .. }, E::ClosureRef { function: id, .. }) => {
                let Some(original) = self.target(*id, expression.ty) else {
                    return false;
                };
                let locals: &[LocalId] = match &expression.kind {
                    E::ClosureRef { captures, .. } => captures,
                    _ => &[],
                };
                original.source_definition.is_none()
                    && original.body.span == body.span
                    && original.capture_count == locals.len()
                    && original.params[..original.capture_count]
                        .iter()
                        .zip(locals)
                        .all(|(param, local)| {
                            self.binding(*local).is_some_and(|current| {
                                current.ty == param.ty
                                    && captures
                                        .get(&param.name)
                                        .is_some_and(|value| child(value, &current))
                            })
                        })
            }
            (Value::ReflectedCallable(admitted), E::FunctionAdapter { function, .. }) => {
                let Some(adapter) = self.functions.get(function.index() as usize) else {
                    return false;
                };
                let Some(Statement {
                    kind:
                        StatementKind::Return(Some(Expression {
                            kind:
                                E::Handle {
                                    kind:
                                        HandleKind::ReflectedReturn {
                                            callable: Some(plan),
                                            ..
                                        },
                                    ..
                                },
                            ..
                        })),
                    ..
                }) = adapter.body.statements.first()
                else {
                    return false;
                };
                plan.is_materialized()
                    && plan.checked_plan() == admitted.checked_plan()
                    && Arc::ptr_eq(&plan.data.checked, admitted.checked_program())
                    && expression.ty == admitted.requested_type()
                    && validate(
                        admitted,
                        expression,
                        self.owner,
                        self.functions,
                        self.archive,
                    )
                    .is_ok()
            }
            _ => false,
        }
    }
}

fn bindings(block: &Block, local: LocalId, values: &mut Vec<Expression>) {
    for statement in &block.statements {
        match &statement.kind {
            StatementKind::Let {
                local: current,
                value,
            } if *current == local => values.push(value.clone()),
            StatementKind::Assign {
                target:
                    Expression {
                        kind: ExpressionKind::Local(current),
                        ..
                    },
                value,
            } if *current == local => values.push(value.clone()),
            StatementKind::If {
                then_block,
                else_block,
                ..
            } => {
                bindings(then_block, local, values);
                if let Some(block) = else_block {
                    bindings(block, local, values);
                }
            }
            StatementKind::For { body, .. }
            | StatementKind::While { body, .. }
            | StatementKind::Scope(body) => bindings(body, local, values),
            StatementKind::Match { arms, .. } => {
                for arm in arms {
                    bindings(&arm.body, local, values);
                }
            }
            StatementKind::ReflectedTypeDispatch { arms, .. } => {
                for arm in arms {
                    bindings(&arm.body, local, values);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialized_values_preserve_nominal_identity_and_every_pending_layer() {
        let file = FileId::new(0);
        let parsed = jett_parser::parse(
            "namespace app\nstruct First:\n    raw: int64\nstruct Second:\n    raw: int64\nfunction context() returns nothing:\n    return nothing\n",
            file,
        );
        assert!(parsed.errors.is_empty());
        let checked = Arc::new(
            jett_typecheck::CheckedResourceProgram::prepare(
                parsed,
                HashMap::from([(file, SourceOrigin::Project)]),
                &[],
                jett_typecheck::CheckOptions { release: false },
            )
            .unwrap(),
        );
        let program = lower_checked_resource_program(&checked).unwrap();
        let owner = &program.functions[0];
        let context = Context {
            owner,
            functions: &program.functions,
            archive: &program.resource_source,
            checked: &checked,
            used: RefCell::new(HashSet::new()),
        };
        let types = &checked.checked().interner;
        let first = types
            .type_ids()
            .find(|ty| types.type_name(*ty) == "app.First")
            .unwrap();
        let second = types
            .type_ids()
            .find(|ty| types.type_name(*ty) == "app.Second")
            .unwrap();
        let span = owner.span;
        let primitive = Expression {
            kind: ExpressionKind::Int(5),
            ty: TypeInterner::INT64,
            span,
        };
        let construct = |ty| Expression {
            kind: ExpressionKind::StructConstruct {
                struct_type: ty,
                fields: vec![primitive.clone()],
                evaluation_order: vec![0],
                validates_refinements: false,
                refinement_predicates: Vec::new(),
            },
            ty,
            span,
        };
        let retained = Value::Struct {
            type_name: "app.First".into(),
            concrete_type: None,
            fields: vec![("raw".into(), Value::Int64(5))],
        };
        assert!(context.matches(&retained, &construct(first), types.len()));
        assert!(
            !context.matches(&retained, &construct(second), types.len()),
            "same-layout nominal substitution"
        );
        let run = |value| Expression {
            kind: ExpressionKind::Run(Box::new(value)),
            ty: TypeInterner::INT64,
            span,
        };
        let pending = Value::Pending(Box::new(Value::Pending(Box::new(Value::Int64(5)))));
        assert!(context.matches(&pending, &run(run(primitive.clone())), types.len()));
        assert!(
            !context.matches(&pending, &run(primitive.clone()), types.len()),
            "pending layer removed"
        );
        assert!(
            !context.matches(&pending, &run(run(run(primitive))), types.len()),
            "pending layer added"
        );
    }
}
