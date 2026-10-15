//! Reconstruct facts from the immutable archive. Unknown inputs stay unknown.
use super::*;
use hir::StatementKind as H;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
struct Rows {
    values: Vec<ValueRow>,
    headers: Vec<HeaderRow>,
    formals: Vec<FormalRow>,
    returns: Vec<ValueRow>,
    returned: Option<GraphStorageProof>,
    calls: Vec<Expression>,
    edges: Vec<GraphSourceEdgeProof>,
    incoming: Vec<(Expression, FunctionId, usize, Option<GraphStorageProof>)>,
    messages: Vec<(Expression, FunctionId, Option<GraphStorageProof>)>,
    unknown_return: bool,
}
impl PartialEq for Rows {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
            && self.headers == other.headers
            && self.formals == other.formals
            && self.returns == other.returns
            && self.returned == other.returned
            && expression_rows_equal(&self.calls, &other.calls)
            && self.edges == other.edges
            && self.incoming.len() == other.incoming.len()
            && self.incoming.iter().zip(&other.incoming).all(
                |(
                    (a, target, parameter, proof),
                    (b, other_target, other_parameter, other_proof),
                )| {
                    equal(a, b)
                        && target == other_target
                        && parameter == other_parameter
                        && proof == other_proof
                },
            )
            && self.messages.len() == other.messages.len()
            && self.messages.iter().zip(&other.messages).all(
                |((a, target, proof), (b, other_target, other_proof))| {
                    equal(a, b) && target == other_target && proof == other_proof
                },
            )
            && self.unknown_return == other.unknown_return
    }
}
#[derive(Debug, Clone, PartialEq)]
struct Factory {
    plan: hir::ActorGraphPlan,
    output: Option<GraphStorageProof>,
    rows: Rows,
    fields: Vec<(usize, usize, GraphStorageProof)>,
    captures: Vec<hir::ActorGraphCaptureBinding>,
}
struct Analysis<'a> {
    archive: &'a hir::ResourceSourceArchive,
    types: &'a TypeInterner,
    factories: &'a [Factory],
    previous: &'a [Rows],
}

fn remember(rows: &mut Rows, expression: &Expression, proof: &GraphStorageProof) {
    if expression.ty != proof.ty {
        return;
    }
    if let Some(row) = rows
        .values
        .iter_mut()
        .find(|row| equal(&row.original, expression))
    {
        // Equal occurrences retain every rooted alternative, never a type-only
        // common denominator. Current selection still requires pointer membership.
        if let Some(joined) = merge([row.proof.clone(), proof.clone()]) {
            row.proof = joined;
        }
    } else {
        rows.values.push(ValueRow {
            original: expression.clone(),
            current: expression.clone(),
            proof: proof.clone(),
        });
    }
}
fn local_header(function: &hir::Function, local: LocalId, ty: TypeId) -> Option<&Local> {
    function
        .locals
        .get(local.index() as usize)
        .filter(|header| header.id == local && header.ty == ty)
}
fn written(function: &hir::Function) -> HashSet<LocalId> {
    fn walk_block(block: &hir::Block, found: &mut HashSet<LocalId>) {
        for statement in &block.statements {
            match &statement.kind {
                H::Assign { target, .. } | H::UpdateField { target, .. } => {
                    let mut base = target;
                    while let E::Field { base: inner, .. } | E::View(inner) = &base.kind {
                        base = inner;
                    }
                    if let E::Local(local) = base.kind {
                        found.insert(local);
                    }
                }
                H::Scope(body) | H::While { body, .. } | H::For { body, .. } => {
                    walk_block(body, found)
                }
                H::If {
                    then_block,
                    else_block,
                    ..
                } => {
                    walk_block(then_block, found);
                    if let Some(body) = else_block {
                        walk_block(body, found);
                    }
                }
                H::Match { arms, .. } => {
                    for arm in arms {
                        walk_block(&arm.body, found);
                    }
                }
                H::ReflectedTypeDispatch { arms, .. } => {
                    for arm in arms {
                        walk_block(&arm.body, found);
                    }
                }
                _ => {}
            }
        }
    }
    let mut found = HashSet::new();
    walk_block(&function.body, &mut found);
    walk::hir_block(&function.body, &mut |value| match &value.kind {
        E::Handle { failure, .. } => walk_block(failure, &mut found),
        E::InlineFunction { body, .. } => walk_block(body, &mut found),
        _ => {}
    });
    found
}

// This proof is deliberately smaller than purity: a pure actor helper may
// change actor state. Exact direct Source calls are followed transitively;
// unknown dispatch/effect paths withhold the frozen fact rather than becoming
// an assumed state-preservation edge.
fn unchanged_handler(
    function: &hir::Function,
    archive: &hir::ResourceSourceArchive,
    active: &mut HashSet<FunctionId>,
) -> bool {
    if !written(function).is_empty() || !active.insert(function.id) {
        return false;
    }
    let mut unchanged = true;
    walk::hir_block(&function.body, &mut |value| {
        if !unchanged {
            return;
        }
        match &value.kind {
            E::Call { .. } => {
                unchanged = source_call(value, archive)
                    .is_some_and(|(target, _, _)| unchanged_handler(target, archive, active));
            }
            E::IndirectCall { .. }
            | E::Intrinsic { .. }
            | E::ActorMessage { .. }
            | E::ActorSpawn { .. }
            | E::Run(_)
            | E::Join(_)
            | E::Cancel(_)
            | E::ResourceInvoke { .. }
            | E::ActorGraphImport(_)
            | E::ActorGraphStep { .. } => unchanged = false,
            _ => {}
        }
    });
    active.remove(&function.id);
    unchanged
}
fn plain(
    plan: &hir::ActorGraphPlan,
    value: &Expression,
    types: &TypeInterner,
) -> Option<GraphStorageProof> {
    storage_safe(types, value.ty).then(|| GraphStorageProof {
        ty: value.ty,
        shape: Shape::Plain {
            plan: plan.clone(),
            value: value.clone(),
        },
    })
}
fn fields(
    proof: &GraphStorageProof,
    field: usize,
    owner: TypeId,
    result: TypeId,
    types: &TypeInterner,
) -> Option<GraphStorageProof> {
    if proof.ty != owner {
        return None;
    }
    let Type::Struct(id) = types.resolve(owner) else {
        return None;
    };
    if types.resolve_struct(*id).fields.get(field)?.1 != result {
        return None;
    }
    let result_proof = match &proof.shape {
        Shape::Fields(values) => values.get(field)?.clone()?,
        Shape::Alternatives(alternatives) => merge(
            alternatives
                .iter()
                .map(|alternative| fields(alternative, field, owner, result, types))
                .collect::<Option<Vec<_>>>()?,
        )?,
        _ => return None,
    };
    (result_proof.ty == result).then_some(result_proof)
}
fn rooted_aggregate(
    ty: TypeId,
    values: Vec<Option<GraphStorageProof>>,
    list: bool,
    types: &TypeInterner,
) -> Option<GraphStorageProof> {
    if !storage_safe(types, ty) || !values.iter().any(Option::is_some) {
        return None;
    }
    Some(GraphStorageProof {
        ty,
        shape: if list {
            Shape::Items(values)
        } else {
            Shape::Fields(values)
        },
    })
}

fn source_call<'a>(
    value: &'a Expression,
    archive: &'a hir::ResourceSourceArchive,
) -> Option<(
    &'a hir::Function,
    &'a [Expression],
    &'a hir::SourceCallOwnership,
)> {
    let E::Call {
        function,
        args,
        evaluation_order,
        ownership: hir::CallOwnership::Source(source),
    } = &value.kind
    else {
        return None;
    };
    let target = archive
        .execution_functions()
        .get(function.index() as usize)
        .filter(|target| target.id == *function)?;
    if target.capture_count != 0
        || target.params.len() != args.len()
        || target.return_type != value.ty
        || source.bridge != hir::CallBridge::Direct
        || !source.generated_operands.is_empty()
        || source.arguments.len() != args.len()
        || !matches!(&source.target,hir::CallTarget::Function(id) if id==function)
            && !matches!(&source.target,hir::CallTarget::Declaration {identity,..} if identity==&target.identity)
        || evaluation_order.len() != args.len()
    {
        return None;
    }
    let mut order = evaluation_order.clone();
    order.sort_unstable();
    if order != (0..args.len()).collect::<Vec<_>>() {
        return None;
    }
    for (index, ((actual, parameter), argument)) in args
        .iter()
        .zip(&target.params)
        .zip(&source.arguments)
        .enumerate()
    {
        if argument.parameter_index != index
            || argument.actual_type != actual.ty
            || argument.parameter_type != parameter.ty
            || argument.staging != hir::ArgumentStaging::Original
            || argument.source_witness().occurrence_type() != actual.ty
            || argument.source_witness().context() != source.context
        {
            return None;
        }
    }
    Some((target, args, source))
}
fn required_child(ty: TypeId, proof: &Option<GraphStorageProof>, types: &TypeInterner) -> bool {
    !resource_type_pending(types, ty) || proof.as_ref().is_some_and(|proof| proof.ty == ty)
}

// Recognize the complete archived producer body, never a name or signature.
// These forms cannot create an occupied resource: the list starts empty and
// every appended item comes from the exact protected owning iterable.
struct CollectionProjection<'a> {
    output: LocalId,
    initial: &'a Expression,
    iterable: &'a Expression,
    key: LocalId,
    value: LocalId,
    assigned: &'a Expression,
    returned: &'a Expression,
    part: usize,
}
fn list_operation(function: &hir::Function, types: &TypeInterner) -> Option<hir::IntrinsicId> {
    let [statement] = function.body.statements.as_slice() else {
        return None;
    };
    let H::Return(Some(value)) = &statement.kind else {
        return None;
    };
    let E::Intrinsic {
        intrinsic,
        type_arguments,
        reflection_arguments,
        refinement_predicates,
        field_validation,
        args,
        evaluation_order,
        ..
    } = &value.kind
    else {
        return None;
    };
    let Type::List(item) = types.resolve(function.return_type) else {
        return None;
    };
    let expected = match intrinsic {
        hir::IntrinsicId::ListNew => Vec::new(),
        hir::IntrinsicId::ListAppend => vec![function.return_type, *item],
        _ => return None,
    };
    if function.capture_count != 0
        || value.ty != function.return_type
        || !storage_safe(types, value.ty)
        || type_arguments.as_slice() != [*item]
        || !reflection_arguments.is_empty()
        || !refinement_predicates.is_empty()
        || field_validation.is_some()
        || function.params.len() != expected.len()
        || args.len() != expected.len()
        || evaluation_order != &(0..args.len()).collect::<Vec<_>>()
    {
        return None;
    }
    for ((parameter, argument), expected) in function.params.iter().zip(args).zip(expected) {
        if parameter.ty != expected
            || parameter.mode != hir::ParamMode::Owned
            || parameter.mutable
            || argument.ty != expected
            || !matches!(argument.kind, E::Local(id) if id == parameter.local)
            || local_header(function, parameter.local, expected)?
                .view_source
                .is_some()
        {
            return None;
        }
    }
    Some(*intrinsic)
}
fn collection_projection<'a>(
    function: &'a hir::Function,
    archive: &hir::ResourceSourceArchive,
    types: &TypeInterner,
) -> Option<CollectionProjection<'a>> {
    let [initial, iteration, returned] = function.body.statements.as_slice() else {
        return None;
    };
    let H::Let {
        local: output,
        value: initial,
    } = &initial.kind
    else {
        return None;
    };
    let H::For {
        key,
        value: Some(value),
        iterable,
        body,
        by_view: false,
    } = &iteration.kind
    else {
        return None;
    };
    let [assignment] = body.statements.as_slice() else {
        return None;
    };
    let H::Assign {
        target,
        value: assigned,
    } = &assignment.kind
    else {
        return None;
    };
    let H::Return(Some(returned)) = &returned.kind else {
        return None;
    };
    let [parameter] = function.params.as_slice() else {
        return None;
    };
    let Type::Map(key_ty, value_ty) = types.resolve(parameter.ty) else {
        return None;
    };
    let Type::List(item_ty) = types.resolve(function.return_type) else {
        return None;
    };
    let (new, new_args, _) = source_call(initial, archive)?;
    let (append, append_args, _) = source_call(assigned, archive)?;
    let [accumulator, item] = append_args else {
        return None;
    };
    let part = match item.kind {
        E::Local(id) if id == *key && item.ty == *key_ty => 0,
        E::Local(id) if id == *value && item.ty == *value_ty => 1,
        _ => return None,
    };
    let header = local_header(function, *output, function.return_type)?;
    if function.capture_count != 0
        || parameter.mode != hir::ParamMode::Owned
        || parameter.mutable
        || !storage_safe(types, parameter.ty)
        || !header.mutable
        || header.view_source.is_some()
        || !new_args.is_empty()
        || list_operation(new, types) != Some(hir::IntrinsicId::ListNew)
        || list_operation(append, types) != Some(hir::IntrinsicId::ListAppend)
        || initial.ty != function.return_type
        || assigned.ty != function.return_type
        || target.ty != function.return_type
        || returned.ty != function.return_type
        || accumulator.ty != function.return_type
        || item.ty != *item_ty
        || iterable.ty != parameter.ty
        || !matches!(iterable.kind, E::Local(id) if id == parameter.local)
        || !matches!(target.kind, E::Local(id) if id == *output)
        || !matches!(accumulator.kind, E::Local(id) if id == *output)
        || !matches!(returned.kind, E::Local(id) if id == *output)
        || local_header(function, *key, *key_ty)?.view_source.is_some()
        || local_header(function, *value, *value_ty)?
            .view_source
            .is_some()
    {
        return None;
    }
    Some(CollectionProjection {
        output: *output,
        initial,
        iterable,
        key: *key,
        value: *value,
        assigned,
        returned,
        part,
    })
}
fn collection_formals(
    function: &hir::Function,
    archive: &hir::ResourceSourceArchive,
    types: &TypeInterner,
) -> bool {
    collection_projection(function, archive, types).is_some()
        || list_operation(function, types) == Some(hir::IntrinsicId::ListAppend)
            && archive.execution_functions().iter().any(|owner| {
                collection_projection(owner, archive, types).is_some_and(|projection| {
                    source_call(projection.assigned, archive)
                        .is_some_and(|(append, _, _)| append.id == function.id)
                })
            })
}

impl Analysis<'_> {
    fn evaluate(
        &self,
        value: &Expression,
        function: &hir::Function,
        locals: &mut HashMap<LocalId, GraphStorageProof>,
        rows: &mut Rows,
        root: Option<&hir::ActorGraphPlan>,
    ) -> Option<GraphStorageProof> {
        let proof = match &value.kind {
            E::ActorGraphStep { instruction, args } => {
                let plan = root?;
                if !plan.instructions().contains(instruction)
                    || instruction
                        .validate(function.id, &function.identity, args, value.ty, self.types)
                        .is_err()
                {
                    return None;
                }
                let hir::ActorGraphOperation::HookBox { hook, .. } = instruction.operation() else {
                    return None;
                };
                let projection = instruction.projected_hook()?;
                if projection.signature() != value.ty
                    || hook.function_type() != value.ty
                    || !projection.belongs_to(instruction.checked())
                {
                    return None;
                }
                GraphStorageProof {
                    ty: value.ty,
                    shape: Shape::Hook(instruction.clone()),
                }
            }
            E::ActorGraphImport(plan) => {
                let factory = self
                    .factories
                    .iter()
                    .find(|factory| factory.plan == *plan)?;
                for row in &factory.rows.values {
                    remember(rows, &row.original, &row.proof);
                }
                for row in &factory.rows.headers {
                    locals.insert(row.original.id, row.proof.clone());
                    if !rows.headers.iter().any(|old| old.original == row.original) {
                        rows.headers.push(row.clone());
                    }
                }
                factory.output.clone()?
            }
            E::Local(local) => locals
                .get(local)
                .filter(|proof| proof.transferable())
                .cloned()?,
            E::Clone(inner) | E::View(inner) if inner.ty == value.ty => {
                self.evaluate(inner, function, locals, rows, root)?
            }
            E::Field {
                base,
                owner_type,
                field,
            } => {
                let proof = self.evaluate(base, function, locals, rows, root)?;
                fields(
                    &proof,
                    field.index() as usize,
                    *owner_type,
                    value.ty,
                    self.types,
                )?
            }
            E::StructConstruct {
                struct_type,
                fields,
                ..
            } if *struct_type == value.ty => {
                let Type::Struct(id) = self.types.resolve(value.ty) else {
                    return None;
                };
                let shape = self.types.resolve_struct(*id);
                if shape.fields.len() != fields.len() {
                    return None;
                }
                let mut proofs = Vec::new();
                for (expression, (_, ty)) in fields.iter().zip(&shape.fields) {
                    if expression.ty != *ty {
                        return None;
                    }
                    let proof = self.evaluate(expression, function, locals, rows, root);
                    if !required_child(*ty, &proof, self.types) {
                        return None;
                    }
                    proofs.push(proof);
                }
                rooted_aggregate(value.ty, proofs, false, self.types)?
            }
            E::ListConstruct { elements } => {
                let Type::List(item) = self.types.resolve(value.ty) else {
                    return None;
                };
                let mut proofs = Vec::new();
                for expression in elements {
                    if expression.ty != *item {
                        return None;
                    }
                    let proof = self.evaluate(expression, function, locals, rows, root);
                    if !required_child(*item, &proof, self.types) {
                        return None;
                    }
                    proofs.push(proof);
                }
                if elements.is_empty() {
                    plain(root?, value, self.types)?
                } else {
                    rooted_aggregate(value.ty, proofs, true, self.types)?
                }
            }
            E::MapConstruct { entries } => {
                let Type::Map(key, item) = self.types.resolve(value.ty) else {
                    return None;
                };
                let mut proofs = Vec::new();
                for entry in entries {
                    if entry.key.ty != *key || entry.value.ty != *item {
                        return None;
                    }
                    let key_proof = self.evaluate(&entry.key, function, locals, rows, root);
                    let value_proof = self.evaluate(&entry.value, function, locals, rows, root);
                    if !required_child(*key, &key_proof, self.types)
                        || !required_child(*item, &value_proof, self.types)
                    {
                        return None;
                    }
                    proofs.push((key_proof, value_proof));
                }
                if !storage_safe(self.types, value.ty) {
                    return None;
                }
                if proofs
                    .iter()
                    .any(|(key, value)| key.is_some() || value.is_some())
                {
                    GraphStorageProof {
                        ty: value.ty,
                        shape: Shape::Entries(proofs),
                    }
                } else {
                    plain(root?, value, self.types)?
                }
            }
            E::OptionalSome(inner) => {
                let Type::Optional(item) = self.types.resolve(value.ty) else {
                    return None;
                };
                let proof = self.evaluate(inner, function, locals, rows, root)?;
                if proof.ty != *item || !storage_safe(self.types, value.ty) {
                    return None;
                }
                GraphStorageProof {
                    ty: value.ty,
                    shape: Shape::Some(Box::new(proof)),
                }
            }
            E::ResultOk(inner) | E::ResultFail(inner) => {
                let Type::Result(ok, fail) = self.types.resolve(value.ty) else {
                    return None;
                };
                let is_ok = matches!(value.kind, E::ResultOk(_));
                let proof = self.evaluate(inner, function, locals, rows, root)?;
                if proof.ty != if is_ok { *ok } else { *fail }
                    || !storage_safe(self.types, value.ty)
                {
                    return None;
                }
                GraphStorageProof {
                    ty: value.ty,
                    shape: if is_ok {
                        Shape::Ok(Box::new(proof))
                    } else {
                        Shape::Fail(Box::new(proof))
                    },
                }
            }
            E::Intrinsic {
                intrinsic, args, ..
            } if list_operation(function, self.types) == Some(*intrinsic) => {
                match intrinsic {
                    hir::IntrinsicId::ListNew => {
                        // An empty helper has no incoming operand. It is selected
                        // only after a protected projection retained this exact
                        // archived direct call; no type or name can activate it.
                        if !self.previous.iter().any(|previous| {
                            previous.calls.iter().any(|call| {
                                source_call(call, self.archive)
                                    .is_some_and(|(target, _, _)| target.id == function.id)
                            })
                        }) {
                            return None;
                        }
                        GraphStorageProof {
                            ty: value.ty,
                            shape: Shape::Items(Vec::new()),
                        }
                    }
                    hir::IntrinsicId::ListAppend => {
                        let [items, item] = args.as_slice() else {
                            return None;
                        };
                        let list = self.evaluate(items, function, locals, rows, root)?;
                        let item = self.evaluate(item, function, locals, rows, root)?;
                        let mut alternatives = vec![item];
                        if let Some(existing) = iteration_part(&list, 0) {
                            alternatives.insert(0, existing);
                        } else if !matches!(&list.shape, Shape::Items(items) if items.is_empty()) {
                            return None;
                        }
                        GraphStorageProof {
                            ty: value.ty,
                            shape: Shape::Items(vec![Some(merge(alternatives)?)]),
                        }
                    }
                    _ => return None,
                }
            }
            E::Call { .. } => {
                let (target, args, _) = source_call(value, self.archive)?;
                let mut relevant = false;
                for (index, argument) in args.iter().enumerate() {
                    let proof = self.evaluate(argument, function, locals, rows, root);
                    rows.incoming
                        .push((value.clone(), target.id, index, proof.clone()));
                    if let Some(proof) = proof.filter(|proof| proof.ty == target.params[index].ty) {
                        relevant = true;
                        rows.edges.push(GraphSourceEdgeProof {
                            original_call: value.clone(),
                            current_call: value.clone(),
                            parameter: index,
                            formal: target.params[index].clone(),
                            actual: argument.clone(),
                            proof,
                        });
                    }
                }
                let returned = self
                    .previous
                    .get(target.id.index() as usize)?
                    .returned
                    .clone();
                relevant |= returned.is_some();
                if relevant && !rows.calls.iter().any(|call| equal(call, value)) {
                    rows.calls.push(value.clone());
                }
                returned?
            }
            E::ActorMessage {
                actor,
                handler,
                args,
                ..
            } => {
                let receiver = self.evaluate(actor, function, locals, rows, root);
                rows.messages.push((value.clone(), *handler, receiver));
                for argument in args {
                    self.evaluate(argument, function, locals, rows, root);
                }
                return None;
            }
            // Every plain factory leaf is in the protected body and rooted in
            // that same required graph. Public FunctionRef/closure/type rows
            // outside this constructor never reach this branch with a root.
            E::FunctionRef(_)
            | E::ClosureRef { .. }
            | E::Int(_)
            | E::Float(_)
            | E::String(_)
            | E::Bool(_)
            | E::Nothing
            | E::OptionalNone => plain(root?, value, self.types)?,
            _ => return None,
        };
        if proof.ty != value.ty || !storage_safe(self.types, proof.ty) {
            return None;
        }
        remember(rows, value, &proof);
        Some(proof)
    }
    fn projection_block(
        &self,
        function: &hir::Function,
        locals: &mut HashMap<LocalId, GraphStorageProof>,
        rows: &mut Rows,
    ) -> Option<()> {
        let projection = collection_projection(function, self.archive, self.types)?;
        let input = self.evaluate(projection.iterable, function, locals, rows, None)?;
        let item = iteration_part(&input, projection.part)?;
        let proof = GraphStorageProof {
            ty: function.return_type,
            shape: Shape::Items(vec![Some(item)]),
        };
        // The accumulator fact describes a bounded set of possible protected
        // producers, including the empty prefix. It is not a value/length fact.
        // The exact empty initialization is a subset of that same bounded
        // producer set. Keep its occurrence and mutable header in agreement;
        // the ListNew helper's independent return remains the neutral empty set.
        remember(rows, projection.initial, &proof);
        rows.calls.push(projection.initial.clone());
        locals.insert(projection.output, proof.clone());
        let output = local_header(function, projection.output, proof.ty)?;
        rows.headers.push(HeaderRow {
            original: output.clone(),
            current: output.clone(),
            proof: proof.clone(),
        });
        for (binder, part) in [(projection.key, 0), (projection.value, 1)] {
            if let Some(fact) = iteration_part(&input, part) {
                let header = local_header(function, binder, fact.ty)?;
                locals.insert(binder, fact.clone());
                rows.headers.push(HeaderRow {
                    original: header.clone(),
                    current: header.clone(),
                    proof: fact,
                });
            }
        }
        self.evaluate(projection.assigned, function, locals, rows, None);
        remember(rows, projection.assigned, &proof);
        remember(rows, projection.returned, &proof);
        rows.returns.push(ValueRow {
            original: projection.returned.clone(),
            current: projection.returned.clone(),
            proof,
        });
        Some(())
    }
    fn block(
        &self,
        block: &hir::Block,
        function: &hir::Function,
        locals: &mut HashMap<LocalId, GraphStorageProof>,
        rows: &mut Rows,
        writes: &HashSet<LocalId>,
    ) -> bool {
        let mut live = true;
        for statement in &block.statements {
            if !live {
                break;
            }
            match &statement.kind {
                H::Let { local, value } => {
                    let proof = self.evaluate(value, function, locals, rows, None);
                    if let Some(header) = local_header(function, *local, value.ty)
                        && header.view_source.is_none()
                        && !writes.contains(local)
                        && let Some(proof) = proof
                    {
                        locals.insert(*local, proof.clone());
                        rows.headers.push(HeaderRow {
                            original: header.clone(),
                            current: header.clone(),
                            proof,
                        });
                    } else {
                        locals.remove(local);
                    }
                }
                H::Assign { target, value } | H::UpdateField { target, value, .. } => {
                    self.evaluate(value, function, locals, rows, None);
                    let mut base = target;
                    while let E::Field { base: inner, .. } | E::View(inner) = &base.kind {
                        base = inner;
                    }
                    if let E::Local(local) = base.kind {
                        locals.remove(&local);
                    }
                }
                H::Return(Some(value)) | H::Respond(value) => {
                    match self.evaluate(value, function, locals, rows, None) {
                        Some(proof) if proof.ty == function.return_type => {
                            rows.returns.push(ValueRow {
                                original: value.clone(),
                                current: value.clone(),
                                proof,
                            })
                        }
                        _ => rows.unknown_return = true,
                    }
                    live = false;
                }
                H::Return(None) => {
                    rows.unknown_return = true;
                    live = false;
                }
                H::Scope(body) => {
                    live = self.block(body, function, locals, rows, writes);
                }
                H::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    self.evaluate(condition, function, locals, rows, None);
                    let mut left = locals.clone();
                    let left_live = self.block(then_block, function, &mut left, rows, writes);
                    let mut right = locals.clone();
                    let right_live = else_block
                        .as_ref()
                        .is_none_or(|body| self.block(body, function, &mut right, rows, writes));
                    *locals = join_environments(
                        if left_live { Some(&left) } else { None },
                        if right_live { Some(&right) } else { None },
                    );
                    live = left_live || right_live;
                }
                H::While { condition, body } => {
                    self.evaluate(condition, function, locals, rows, None);
                    // A loop body has no one-pass dominance proof. Visit calls
                    // only with unknown locals; no produced value escapes it.
                    let mut unknown = HashMap::new();
                    self.block(body, function, &mut unknown, rows, writes);
                    locals.retain(|id, _| !writes.contains(id));
                }
                H::For {
                    key,
                    value,
                    iterable,
                    body,
                    by_view,
                } => {
                    let proof = self.evaluate(iterable, function, locals, rows, None);
                    let mut branch = locals.clone();
                    branch.remove(key);
                    if let Some(value) = value {
                        branch.remove(value);
                    }
                    if let Some(proof) = proof {
                        for (binder, part) in [Some((*key, 0)), value.map(|id| (id, 1))]
                            .into_iter()
                            .flatten()
                        {
                            if let Some(fact) = iteration_part(&proof, part)
                                && let Some(header) = local_header(function, binder, fact.ty)
                                && !writes.contains(&binder)
                            {
                                branch.insert(binder, fact.clone());
                                // The owning header selection is distinct from
                                // the existing by-view loop borrow certificate.
                                if !by_view && header.view_source.is_none() {
                                    rows.headers.push(HeaderRow {
                                        original: header.clone(),
                                        current: header.clone(),
                                        proof: fact,
                                    });
                                }
                            }
                        }
                    }
                    self.block(body, function, &mut branch, rows, writes);
                    locals.retain(|id, _| !writes.contains(id));
                }
                H::Match { scrutinee, arms } => {
                    self.evaluate(scrutinee, function, locals, rows, None);
                    let mut joined: Option<HashMap<LocalId, GraphStorageProof>> = None;
                    let mut any_live = false;
                    for arm in arms {
                        let mut branch = locals.clone();
                        for binder in &arm.bindings {
                            branch.remove(binder);
                        }
                        if self.block(&arm.body, function, &mut branch, rows, writes) {
                            any_live = true;
                            joined = Some(match joined {
                                Some(old) => join_environments(Some(&old), Some(&branch)),
                                None => branch,
                            });
                        }
                    }
                    *locals = joined.unwrap_or_default();
                    live = any_live;
                }
                H::ReflectedTypeDispatch { type_info, arms } => {
                    self.evaluate(type_info, function, locals, rows, None);
                    // Missing dispatch outcome remains a live unknown edge.
                    for arm in arms {
                        let mut branch = HashMap::new();
                        self.block(&arm.body, function, &mut branch, rows, writes);
                    }
                    locals.clear();
                }
                H::Expression(value) | H::HandleDefault(value) => {
                    self.evaluate(value, function, locals, rows, None);
                }
                H::Assert { condition, message } => {
                    self.evaluate(condition, function, locals, rows, None);
                    if let Some(value) = message {
                        self.evaluate(value, function, locals, rows, None);
                    }
                }
                H::Breakpoint { condition, .. } => {
                    if let Some(value) = condition {
                        self.evaluate(value, function, locals, rows, None);
                    }
                }
                H::Trace(_) => {}
                H::Break | H::Continue => {
                    locals.clear();
                    live = false;
                }
            }
        }
        live
    }
}
fn join_environments(
    left: Option<&HashMap<LocalId, GraphStorageProof>>,
    right: Option<&HashMap<LocalId, GraphStorageProof>>,
) -> HashMap<LocalId, GraphStorageProof> {
    match (left, right) {
        (Some(left), Some(right)) => left
            .iter()
            .filter_map(|(id, value)| Some((*id, merge([value.clone(), right.get(id)?.clone()])?)))
            .collect(),
        (Some(one), None) | (None, Some(one)) => one.clone(),
        (None, None) => HashMap::new(),
    }
}
fn iteration_part(proof: &GraphStorageProof, part: usize) -> Option<GraphStorageProof> {
    match &proof.shape {
        Shape::Items(items) if part == 0 => {
            merge(items.iter().cloned().collect::<Option<Vec<_>>>()?)
        }
        Shape::Entries(items) => merge(
            items
                .iter()
                .map(|(key, value)| {
                    if part == 0 {
                        key.clone()
                    } else {
                        value.clone()
                    }
                })
                .collect::<Option<Vec<_>>>()?,
        ),
        Shape::Alternatives(rows) => merge(
            rows.iter()
                .map(|row| iteration_part(row, part))
                .collect::<Option<Vec<_>>>()?,
        ),
        _ => None,
    }
}

fn factory(
    plan: &hir::ActorGraphPlan,
    owner: &hir::Function,
    archive: &hir::ResourceSourceArchive,
    types: &TypeInterner,
) -> Result<Factory, String> {
    plan.validate(owner, types)?;
    let graph = plan
        .required_proof()
        .actor_graph()
        .ok_or("graph metadata root lost its checked template")?;
    let analysis = Analysis {
        archive,
        types,
        factories: &[],
        previous: &[],
    };
    let mut rows = Rows::default();
    let mut locals: HashMap<LocalId, GraphStorageProof> = HashMap::new();
    let mut tokens: HashMap<LocalId, GraphStorageProof> = HashMap::new();
    let mut committed: Option<hir::ActorGraphInstruction> = None;
    let mut fields = Vec::new();
    let mut output = None;
    for statement in &plan.body().statements {
        let (binding, value) = match &statement.kind {
            H::Let { local, value } => (Some(*local), value),
            H::Expression(value) => (None, value),
            _ => {
                return Err("graph factory changed its protected straight-line constructor".into());
            }
        };
        let proof = if let E::ActorGraphStep { instruction, args } = &value.kind {
            if !plan.instructions().contains(instruction) {
                return Err("graph storage step belongs to another root".into());
            }
            instruction.validate(owner.id, &owner.identity, args, value.ty, types)?;
            // Exact operand occurrences belong to this protected instruction,
            // rather than inheriting storage from an equal-type local.
            for argument in args {
                if let E::Local(local) = argument.kind
                    && let Some(proof) = locals.get(&local).filter(|proof| proof.ty == argument.ty)
                {
                    remember(&mut rows, argument, proof);
                }
            }
            match instruction.operation() {
                hir::ActorGraphOperation::ReserveBase { ty, fields: count } => {
                    let node = plan
                        .instructions()
                        .iter()
                        .filter(|step| {
                            matches!(
                                step.operation(),
                                hir::ActorGraphOperation::ReserveBase { .. }
                            )
                        })
                        .position(|step| step == instruction)
                        .ok_or("graph seed has no original reserve ordinal")?;
                    let frozen = graph
                        .nodes()
                        .get(node)
                        .ok_or("graph seed ordinal is absent")?;
                    if frozen.ty() != *ty
                        || frozen.captures().len() + frozen.state().len() != *count
                        || value.ty != *ty
                    {
                        return Err("graph seed changed nominal constructor layout".into());
                    }
                    storage_safe(types, *ty).then(|| GraphStorageProof {
                        ty: *ty,
                        shape: Shape::Actor {
                            plan: plan.clone(),
                            node,
                        },
                    })
                }
                hir::ActorGraphOperation::ReservePending { ty, .. } => locals
                    .get(&local_argument(args, 1)?)
                    .cloned()
                    .filter(|proof| proof.ty == *ty),
                hir::ActorGraphOperation::HookBox { hook, .. } => {
                    let projection = instruction
                        .projected_hook()
                        .ok_or("graph hook lost its protected typed path")?;
                    if projection.signature() != value.ty
                        || hook.function_type() != value.ty
                        || !projection.belongs_to(instruction.checked())
                    {
                        return Err("graph hook changed checked required path/signature".into());
                    }
                    Some(GraphStorageProof {
                        ty: value.ty,
                        shape: Shape::Hook(instruction.clone()),
                    })
                }
                hir::ActorGraphOperation::Fill { ty, node, field } => {
                    let base = locals.get(&local_argument(args, 1)?);
                    if base.is_some()
                        && !matches!(base.map(|proof|&proof.shape),Some(Shape::Actor {plan:root,node:base_node}) if root==plan && base_node==node)
                    {
                        return Err("graph field lost its exact reserved actor node".into());
                    }
                    if let Some(proof) = locals
                        .get(&local_argument(args, 2)?)
                        .cloned()
                        .filter(|proof| proof.ty == *ty)
                    {
                        fields.push((*node, *field, proof));
                    }
                    None
                }
                hir::ActorGraphOperation::RootFill { ty } => {
                    if committed.is_some() || value.ty != TypeInterner::UINT64 {
                        return Err("graph RootFill changed its pre-Commit scalar token".into());
                    }
                    if let Some(proof) = locals
                        .get(&local_argument(args, 1)?)
                        .cloned()
                        .filter(|proof| proof.ty == *ty && proof.transferable())
                        && let Some(binding) = binding
                    {
                        let token = GraphStorageProof {
                            ty: TypeInterner::UINT64,
                            shape: Shape::HiddenRootToken {
                                fill: instruction.clone(),
                                root: Box::new(proof),
                            },
                        };
                        tokens.insert(binding, token.clone());
                        Some(token)
                    } else {
                        None
                    }
                }
                hir::ActorGraphOperation::RootPublish { ty } => {
                    let commit = committed
                        .as_ref()
                        .ok_or("graph RootPublish precedes its protected Commit")?;
                    match tokens.get(&local_argument(args, 1)?) {
                        Some(GraphStorageProof {
                            shape: Shape::HiddenRootToken { fill, root },
                            ..
                        }) if fill.same_import(instruction)
                            && commit.same_import(instruction)
                            && root.ty == *ty =>
                        {
                            Some((**root).clone())
                        }
                        _ => None,
                    }
                }
                hir::ActorGraphOperation::Commit => {
                    if committed.replace(instruction.clone()).is_some() {
                        return Err("graph factory repeated its protected Commit".into());
                    }
                    None
                }
                hir::ActorGraphOperation::Begin => None,
            }
        } else {
            analysis.evaluate(value, owner, &mut locals, &mut rows, Some(plan))
        };
        if let Some(proof) = proof {
            remember(&mut rows, value, &proof);
            if let Some(id) = binding {
                let header = local_header(owner, id, value.ty)
                    .ok_or("graph metadata factory local lost its exact original header")?;
                locals.insert(id, proof.clone());
                rows.headers.push(HeaderRow {
                    original: header.clone(),
                    current: header.clone(),
                    proof: proof.clone(),
                });
                if id == plan.output() {
                    output = Some(proof);
                }
            }
        }
    }
    Ok(Factory {
        plan: plan.clone(),
        output,
        rows,
        fields,
        captures: plan.actor_capture_bindings()?,
    })
}
fn local_argument(args: &[Expression], index: usize) -> Result<LocalId, String> {
    match args.get(index).map(|value| &value.kind) {
        Some(E::Local(local)) => Ok(*local),
        _ => Err("graph constructor operand lost exact original Local".into()),
    }
}

fn formal_inputs(index: usize, parameter: usize, previous: &[Rows]) -> Option<GraphStorageProof> {
    let mut facts = Vec::new();
    for rows in previous {
        for (_, target, formal, proof) in &rows.incoming {
            if target.index() as usize == index && *formal == parameter {
                facts.push(proof.clone()?);
            }
        }
    }
    merge(facts)
}
fn actor_nodes(proof: &GraphStorageProof) -> Option<Vec<(hir::ActorGraphPlan, usize)>> {
    match &proof.shape {
        Shape::Actor { plan, node } => Some(vec![(plan.clone(), *node)]),
        Shape::Alternatives(rows) => {
            let mut nodes = Vec::new();
            for row in rows {
                for node in actor_nodes(row)? {
                    if !nodes.contains(&node) {
                        nodes.push(node);
                    }
                }
            }
            Some(nodes)
        }
        _ => None,
    }
}
fn captured_inputs(
    function: &hir::Function,
    parameter: &Param,
    previous: &[Rows],
    factories: &[Factory],
    dynamic: &HashSet<TypeId>,
    archive: &hir::ResourceSourceArchive,
) -> Result<Option<GraphStorageProof>, String> {
    let mut candidates = Vec::new();
    for factory in factories {
        for binding in &factory.captures {
            if binding.handler() != function.id
                || binding.parameter() != parameter
                || dynamic.contains(&binding.actor_type())
            {
                continue;
            }
            if binding.mutable() {
                // The getter proves complete original handler coverage. A
                // write in any handler invalidates this frozen field fact,
                // including reads made by a different handler later.
                let stable = factory
                    .captures
                    .iter()
                    .filter(|other| {
                        other.node() == binding.node() && other.field() == binding.field()
                    })
                    .all(|other| {
                        archive
                            .execution_functions()
                            .get(other.handler().index() as usize)
                            .filter(|handler| handler.id == other.handler())
                            .is_some_and(|handler| {
                                unchanged_handler(handler, archive, &mut HashSet::new())
                            })
                    });
                if !stable {
                    continue;
                }
            }
            if let Some((_, _, proof)) = factory
                .fields
                .iter()
                .find(|(node, field, _)| *node == binding.node() && *field == binding.field())
            {
                candidates.push((factory.plan.clone(), binding.node(), proof.clone()));
            }
        }
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    // An unproved receiver of this same handler cannot inherit another actor
    // instance's frozen state. Every reconstructed message edge is retained.
    for rows in previous {
        for (_, handler, receiver) in &rows.messages {
            if *handler == function.id {
                let Some(nodes) = receiver.as_ref().and_then(actor_nodes) else {
                    return Ok(None);
                };
                if nodes.iter().any(|(plan, node)| {
                    !candidates
                        .iter()
                        .any(|(root, seed, _)| root == plan && seed == node)
                }) {
                    return Ok(None);
                }
            }
        }
    }
    Ok(merge(candidates.into_iter().map(|(_, _, proof)| proof)))
}

/// Immutable equations for one exact checked execution archive. No caller can
/// supply rows, and selection still authenticates the complete owner and types.
#[derive(Debug)]
pub(in crate::resource_ownership) struct OriginalGraphAnalysis {
    source: hir::ResourceSourceArchive,
    rows: Vec<Witness>,
    // Derived only after the entire original analysis has completed. False
    // permits negative metadata answers; it never supplies a storage proof.
    has_rows: bool,
}
impl OriginalGraphAnalysis {
    pub(in crate::resource_ownership) fn same_source(&self, other: &Self) -> bool {
        self.source == other.source
    }
    pub(in crate::resource_ownership) fn has_rows(&self) -> bool {
        self.has_rows
    }
    pub(in crate::resource_ownership) fn has_no_rows_for(
        &self,
        archive: &hir::ResourceSourceArchive,
    ) -> bool {
        &self.source == archive && !self.has_rows
    }
    pub(in crate::resource_ownership) fn prepare(
        archive: &hir::ResourceSourceArchive,
        types: &TypeInterner,
    ) -> Result<Self, String> {
        archive.validate_types(types)?;
        let functions = archive.execution_functions();
        if functions
            .iter()
            .enumerate()
            .any(|(index, function)| function.id.index() as usize != index)
        {
            return Err("graph storage archive lost its dense original function identity".into());
        }
        let rows = reconstruct(archive, types)?;
        let has_rows = rows.iter().any(Witness::needed);
        Ok(Self {
            source: archive.clone(),
            rows,
            has_rows,
        })
    }
    pub(in crate::resource_ownership) fn validate_archive(
        &self,
        archive: &hir::ResourceSourceArchive,
        types: &TypeInterner,
    ) -> Result<(), String> {
        if &self.source != archive {
            return Err(
                "graph storage analysis belongs to another checked execution archive".into(),
            );
        }
        archive.validate_types(types)
    }
    pub(in crate::resource_ownership) fn capture(
        &self,
        original: &hir::Function,
        archive: &hir::ResourceSourceArchive,
        types: &TypeInterner,
    ) -> Result<&Witness, String> {
        self.validate_archive(archive, types)?;
        let index = original.id.index() as usize;
        if !archive
            .execution_functions()
            .get(index)
            .is_some_and(|function| original_function_equal(function, original))
        {
            return Err("graph storage owner differs from whole original checked archive".into());
        }
        self.rows
            .get(index)
            .ok_or_else(|| "graph owner ID is outside checked dense archive".into())
    }
}

fn reconstruct(
    archive: &hir::ResourceSourceArchive,
    types: &TypeInterner,
) -> Result<Vec<Witness>, String> {
    let functions = archive.execution_functions();
    let mut factories = Vec::new();
    let mut dynamic = HashSet::new();
    let mut indirect_targets = HashSet::new();
    for function in functions
        .iter()
        .filter(|function| !archive.required_only_function_ids().contains(&function.id))
    {
        let mut plans = Vec::new();
        walk::hir_block(&function.body, &mut |value| {
            if let E::ActorGraphImport(plan) = &value.kind {
                if !plans.contains(plan) {
                    plans.push(plan.clone());
                }
            }
            if let E::ActorSpawn {
                constructor: Some(target),
                ..
            } = &value.kind
                && let Some(target) = functions.get(target.index() as usize)
            {
                dynamic.insert(target.return_type);
            }
            // This packet reconstructs direct Source incoming edges only. A
            // first-class target cannot inherit their facts while its indirect
            // callers/capture contexts remain unproved.
            if let E::FunctionRef(target)
            | E::ClosureRef {
                function: target, ..
            } = &value.kind
            {
                indirect_targets.insert(*target);
            }
        });
        for plan in plans {
            factories.push(factory(&plan, function, archive, types)?);
        }
    }
    if factories.is_empty() {
        return Ok(vec![Witness::default(); functions.len()]);
    }
    let mut previous = vec![Rows::default(); functions.len()];
    // Positive equations have a finite rooted producer set. No fact is inferred
    // from an unknown edge. Recursive shape growth is left pending, not assumed.
    let bound = functions
        .len()
        .saturating_add(factories.len())
        .saturating_mul(4)
        .max(8);
    for _ in 0..bound {
        let analysis = Analysis {
            archive,
            types,
            factories: &factories,
            previous: &previous,
        };
        let mut next = Vec::new();
        for (index, function) in functions.iter().enumerate() {
            if archive.required_only_function_ids().contains(&function.id) {
                next.push(Rows::default());
                continue;
            }
            let mut rows = Rows::default();
            let mut locals = HashMap::new();
            let writes = written(function);
            for (position, parameter) in function.params.iter().enumerate() {
                if writes.contains(&parameter.local) || !storage_safe(types, parameter.ty) {
                    continue;
                }
                let proof = if position < function.capture_count {
                    captured_inputs(
                        function, parameter, &previous, &factories, &dynamic, archive,
                    )?
                } else if (!archive.exported_function_ids().contains(&function.id)
                    || collection_formals(function, archive, types))
                    && !indirect_targets.contains(&function.id)
                {
                    formal_inputs(index, position, &previous)
                } else {
                    None
                };
                if let Some(proof) = proof.filter(|proof| proof.ty == parameter.ty)
                    && let Some(header) = local_header(function, parameter.local, parameter.ty)
                {
                    locals.insert(parameter.local, proof.clone());
                    rows.formals.push(FormalRow {
                        original: parameter.clone(),
                        current: parameter.clone(),
                        proof: proof.clone(),
                    });
                    rows.headers.push(HeaderRow {
                        original: header.clone(),
                        current: header.clone(),
                        proof,
                    });
                }
            }
            let live = if analysis
                .projection_block(function, &mut locals, &mut rows)
                .is_some()
            {
                false
            } else {
                analysis.block(&function.body, function, &mut locals, &mut rows, &writes)
            };
            // Count every original call/message site, including calls nested in
            // expressions whose storage form is not reconstructed by this packet.
            // Missing sites are unknown incoming edges, never silently omitted.
            walk::hir_block(&function.body, &mut |value| {
                if let E::Call {
                    function: target,
                    args,
                    ..
                } = &value.kind
                {
                    for parameter in 0..args.len() {
                        if !rows.incoming.iter().any(|(call, id, index, _)| {
                            *id == *target && *index == parameter && equal(call, value)
                        }) {
                            rows.incoming
                                .push((value.clone(), *target, parameter, None));
                        }
                    }
                }
                if let E::ActorMessage { handler, .. } = &value.kind
                    && !rows
                        .messages
                        .iter()
                        .any(|(call, id, _)| *id == *handler && equal(call, value))
                {
                    rows.messages.push((value.clone(), *handler, None));
                }
            });
            if !live && !rows.unknown_return {
                rows.returned = merge(rows.returns.iter().map(|row| row.proof.clone()));
            }
            next.push(rows);
        }
        if next == previous {
            return Ok(functions
                .iter()
                .zip(next)
                .map(|(original, rows)| {
                    if archive.required_only_function_ids().contains(&original.id) {
                        return Witness::default();
                    }
                    Witness {
                        owner: Some(original.clone()),
                        local_map: original
                            .locals
                            .iter()
                            .map(|header| (header.clone(), header.clone()))
                            .collect(),
                        values: rows.values,
                        headers: rows.headers,
                        formals: rows.formals,
                        returns: rows.returns,
                        returned: rows.returned,
                        calls: rows.calls,
                        edges: rows.edges,
                        graph: None,
                        constructor_outputs: Vec::new(),
                        sequence_outputs: Vec::new(),
                    }
                })
                .collect());
        }
        previous = next;
    }
    // No incomplete iteration becomes admission evidence.
    Ok(vec![Witness::default(); functions.len()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_projection_requires_the_complete_archived_producer_body() {
        use jett_common::{FileId, SourceOrigin};
        let source =
            include_str!("../../../../jett_driver/tests/actor_graph/fixtures/10_map_shared.jett");
        let mut parsed = jett_parser::parse(source, FileId::new(0));
        let mut origins = HashMap::from([(FileId::new(0), SourceOrigin::Project)]);
        for (file, support) in [
            (
                FileId::new(10_000),
                include_str!("../../../../../stdlib/map.jett"),
            ),
            (
                FileId::new(10_001),
                include_str!("../../../../../stdlib/list.jett"),
            ),
        ] {
            let support = jett_parser::parse(support, file);
            assert!(support.errors.is_empty());
            let mut items = support.module.items;
            items.append(&mut parsed.module.items);
            parsed.module.items = items;
            origins.insert(file, SourceOrigin::Stdlib);
        }
        let checked = std::sync::Arc::new(
            jett_typecheck::CheckedResourceProgram::prepare(
                parsed,
                origins,
                &[],
                jett_typecheck::CheckOptions { release: false },
            )
            .unwrap(),
        );
        let program = hir::lower_checked_ordinary_program(&checked).unwrap();
        let archive = &program.resource_source;
        let types = &checked.checked().interner;
        let function = archive
            .execution_functions()
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "map"
                    && function.identity.declaration.name == "values"
            })
            .unwrap();
        let projection = collection_projection(function, archive, types).unwrap();
        let (append, _, _) = source_call(projection.assigned, archive).unwrap();
        assert_eq!(
            list_operation(append, types),
            Some(hir::IntrinsicId::ListAppend)
        );

        let mut extra_statement = function.clone();
        extra_statement
            .body
            .statements
            .push(function.body.statements[2].clone());
        assert!(collection_projection(&extra_statement, archive, types).is_none());

        let mut viewed_loop = function.clone();
        let H::For { by_view, .. } = &mut viewed_loop.body.statements[1].kind else {
            unreachable!();
        };
        *by_view = true;
        assert!(collection_projection(&viewed_loop, archive, types).is_none());

        let mut different_item = function.clone();
        let H::For { body, .. } = &mut different_item.body.statements[1].kind else {
            unreachable!();
        };
        let H::Assign { value, .. } = &mut body.statements[0].kind else {
            unreachable!();
        };
        let E::Call { args, .. } = &mut value.kind else {
            unreachable!();
        };
        // Preserve the Actor TypeId, but remove its exact loop occurrence.
        args[1].kind = E::Local(LocalId::new(999));
        assert!(collection_projection(&different_item, archive, types).is_none());

        let mut different_intrinsic = append.clone();
        let H::Return(Some(value)) = &mut different_intrinsic.body.statements[0].kind else {
            unreachable!();
        };
        let E::Intrinsic { intrinsic, .. } = &mut value.kind else {
            unreachable!();
        };
        *intrinsic = hir::IntrinsicId::ListLength;
        assert!(list_operation(&different_intrinsic, types).is_none());

        let mut mutable_formal = function.clone();
        mutable_formal.params[0].mutable = true;
        assert!(collection_projection(&mutable_formal, archive, types).is_none());
    }

    #[test]
    fn fixed_point_call_and_message_rows_compare_retained_float_bits() {
        let expression = Expression {
            kind: E::Float(f64::from_bits(0x7ff8_0000_0000_0011)),
            ty: TypeInterner::FLOAT64,
            span: jett_common::Span::new(jett_common::FileId::new(0), 0, 1),
        };
        let mut rows = Rows::default();
        rows.calls.push(expression.clone());
        rows.incoming
            .push((expression.clone(), FunctionId::new(0), 0, None));
        rows.messages
            .push((expression.clone(), FunctionId::new(1), None));
        assert_eq!(rows, rows.clone());
        let mut changed = rows.clone();
        changed.messages[0].0.kind = E::Float(f64::from_bits(0x7ff8_0000_0000_0012));
        assert_ne!(rows, changed);
    }
}
