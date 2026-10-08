//! Required values become execution data only through the original checked cache.
use crate::resource_source::{Materialized, RequiredMaterialization};
use crate::{
    Block, DeclarationKind, Expression, ExpressionKind as E, Function, FunctionId, LowerError,
    Program, ResourceHookRef, StatementKind as S, StringSegment,
};
use jett_common::{FileId, Span};
use jett_comptime::{CheckedRequiredOwner, CheckedRequiredValue, ExplicitComptimeValues, Value};
use jett_parser::ast::{Expr, Item};
use jett_typecheck::CheckedResourceProgram;
use jett_types::{Type, TypeId, TypeInterner};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// Derive required replacements from successful private checked entries. This
/// does not evaluate Source, install a provider, or accept a caller-baked view.
pub fn materialize_checked_required_values(
    program: &mut Program,
    values: &ExplicitComptimeValues,
    types: &TypeInterner,
) -> Result<(), Vec<LowerError>> {
    let archive = program.resource_source.clone();
    let span = program
        .functions
        .first()
        .map_or(Span::new(FileId::new(0), 0, 0), |f| f.span);
    let result = (|| -> Result<Materialized, String> {
        archive.validate_types(types)?;
        if archive.manifest() != Some(&program.resource_manifest)
            || archive.equality_methods() != Some(&program.equality_methods)
            || !functions_equal(archive.execution_functions(), &program.functions)
        {
            return Err(
                "required materialization changed original checked HIR before authentication"
                    .into(),
            );
        }
        let checked = archive
            .checked_program()
            .ok_or("required materialization has no original checked program")?;
        let proofs = values.checked_required_values(checked)?;
        let mut functions = program.functions.clone();
        let mut rows = archive.required_materializations().to_vec();
        let mut candidates = HashSet::new();
        for function in &mut functions {
            let owner = function.clone();
            walk_block(&mut function.body, &mut |expression| {
                if !matches!(expression.kind, E::Comptime { .. } | E::Constant { .. }) {
                    return Ok(());
                }
                let original = expression.clone();
                let selected = proofs
                    .iter()
                    .filter(|proof| proof_matches(proof, &owner, expression, checked))
                    .collect::<Vec<_>>();
                let [proof] = selected.as_slice() else {
                    return Err("required expression has no unique original checked value and complete context".into());
                };
                if let E::Comptime { value, .. } = &original.kind {
                    references(value, &mut candidates);
                }
                let (kind, hook) = derive_value(
                    proof,
                    expression.ty,
                    &program.resource_manifest,
                    checked,
                    types,
                )?;
                expression.kind = kind;
                rows.push(RequiredMaterialization {
                    function: owner.id,
                    identity: owner.identity.clone(),
                    original,
                    current: expression.clone(),
                    hook,
                    proof: (*proof).clone(),
                });
                Ok(())
            })?;
        }
        // Include transitive helpers of the removed required operands. They are
        // excluded only if no preserved runtime root can still reach them.
        for row in &rows {
            if let E::Comptime { value, .. } = &row.original.kind {
                references(value, &mut candidates);
            }
            if !row.proof.belongs_to(checked) {
                return Err("required execution view has foreign proof".into());
            }
        }
        let original_edges = call_graph(archive.functions());
        closure(&mut candidates, &original_edges);
        let exports = archive
            .exported_function_ids()
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mut live = functions
            .iter()
            .filter(|f| {
                !candidates.contains(&f.id)
                    || exports.contains(&f.id)
                    || f.identity.declaration.name == "main"
                    || f.identity.declaration.kind != DeclarationKind::Function
            })
            .map(|f| f.id)
            .collect::<HashSet<_>>();
        closure(&mut live, &call_graph(&functions));
        let mut required_only = candidates.difference(&live).copied().collect::<Vec<_>>();
        required_only.sort_by_key(|id| id.index());
        Ok(Materialized {
            functions,
            values: rows,
            required_only,
        })
    })();
    match result {
        Ok(view) => {
            program.functions = view.functions.clone();
            program.resource_source.install_materialized(view);
            Ok(())
        }
        Err(message) => Err(vec![LowerError { span, message }]),
    }
}

fn proof_matches(
    proof: &CheckedRequiredValue,
    function: &Function,
    expression: &Expression,
    checked: &Arc<CheckedResourceProgram>,
) -> bool {
    if !proof.belongs_to(checked) {
        return false;
    }
    if let E::Constant { declaration } = expression.kind {
        return proof.owner() == CheckedRequiredOwner::NamespaceConstant { declaration }
            && proof.generic_index().is_none()
            && proof.scoped_bindings().is_empty();
    }
    let E::Comptime {
        source_span,
        bindings,
        scopes,
        ..
    } = &expression.kind
    else {
        return false;
    };
    if proof.source_span() != *source_span || !proof.is_explicit_comptime() {
        return false;
    }
    let Ok(original) = proof.original_comptime() else {
        return false;
    };
    if !matches!(original, Expr::Comptime(_, span) if span == source_span)
        || !proof.matches_original_comptime(original)
    {
        return false;
    }
    let owner_matches = match proof.owner() {
        CheckedRequiredOwner::Function { definition, declaration } => {
            function.source_definition == Some(definition)
                && checked.module().items.iter().any(|item| {
                    matches!(item, Item::Function(value)
                        if value.name.span == declaration && value.span == function.span
                            && (checked.resolved().scope_table.def(definition).span == value.name.span
                                || checked.resolved().resolutions.get(&value.name.span) == Some(&definition)))
                })
                && function.identity.declaration.kind == DeclarationKind::Function
        }
        CheckedRequiredOwner::Verify { declaration } => function.identity.declaration.kind == DeclarationKind::Verify
            && checked.module().items.iter().any(|item| matches!(item, Item::Verify(v) if v.name.span == declaration && v.span == function.span)),
        CheckedRequiredOwner::Property { declaration } => function.identity.declaration.kind == DeclarationKind::Property
            && checked.module().items.iter().any(|item| matches!(item, Item::Property(v) if v.name.span == declaration && v.span == function.span)),
        CheckedRequiredOwner::NamespaceConstant { .. } => false,
    };
    if !owner_matches
        || proof.type_arguments() != function.identity.type_arguments
        || proof.specialization() != &function.identity.specialization
    {
        return false;
    }
    let generic = if function.identity.type_arguments.is_empty() {
        None
    } else {
        let matches = checked
            .checked()
            .generic_function_instantiations
            .iter()
            .enumerate()
            .filter(|(_, instance)| {
                Some(instance.definition) == function.source_definition
                    && instance.concrete_args == function.identity.type_arguments
                    && instance.specialization == function.identity.specialization
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [index] = matches.as_slice() else {
            return false;
        };
        Some(*index)
    };
    proof.generic_index() == generic
        && proof.scoped_bindings().len() == bindings.len()
        && scopes.len() == bindings.len()
        && proof
            .scoped_bindings()
            .iter()
            .zip(bindings)
            .zip(scopes)
            .all(|((proof, binding), scope)| {
                proof.name() == binding.name
                    && proof.bound_type() == binding.ty
                    && proof.reflection() == &binding.reflection
                    && proof.owner() == scope.owner
                    && proof.index() == scope.index
                    && proof.selection() == scope.selection
            })
}

fn derive_value(
    proof: &CheckedRequiredValue,
    ty: TypeId,
    manifest: &crate::ResourceManifest,
    checked: &Arc<CheckedResourceProgram>,
    types: &TypeInterner,
) -> Result<(E, Option<ResourceHookRef>), String> {
    if let Some(original) = proof.checked_hook(checked)? {
        let hook = manifest
            .hook_for_definition(original.definition)
            .ok_or("required hook is absent from the original Resource manifest")?;
        if hook.function_type() != ty
            || hook.function_type() != original.function_type
            || hook.resource_type() != original.resource_type
            || hook.resource_kind().definition() != original.resource_definition
        {
            return Err(
                "required hook changed its exact checked signature or nominal Resource identity"
                    .into(),
            );
        }
        hook.validate(types)?;
        return Ok((E::ResourceHookValue { hook: hook.clone() }, Some(hook)));
    }
    if matches!(types.resolve(ty), Type::Secret(_) | Type::Refinement { .. }) {
        return Err("checked required value is evaluated, but qualified native materialized value transport is pending".into());
    }
    let value = proof.value().payload();
    let kind = match (types.resolve(ty), value) {
        (Type::Int8 | Type::Int16 | Type::Int32 | Type::Int64
            | Type::Uint8 | Type::Uint16 | Type::Uint32 | Type::Uint64, Value::Int64(value)) =>
            E::Int(*value as i128),
        (Type::Int8 | Type::Int16 | Type::Int32 | Type::Int64
            | Type::Uint8 | Type::Uint16 | Type::Uint32 | Type::Uint64, Value::Uint64(value)) =>
            E::Int(*value as i128),
        (Type::Float32 | Type::Float64, Value::Float64(value)) => E::Float(*value),
        (Type::String, Value::String(value)) => E::String(value.clone()),
        (Type::Bool, Value::Bool(value)) => E::Bool(*value),
        (Type::Nothing, Value::Nothing) => E::Nothing,
        (Type::Optional(_), Value::OptionalNone) => E::OptionalNone,
        _ => return Err("checked required value is evaluated, but its native materialized value transport is pending".into()),
    };
    Ok((kind, None))
}

pub(crate) fn exported_functions(
    program: &Program,
    checked: &CheckedResourceProgram,
) -> Vec<FunctionId> {
    let mut spans = HashSet::new();
    for item in &checked.module().items {
        match item {
            Item::Function(value) if value.exported => {
                spans.insert(value.name.span);
            }
            Item::Mutual(value) => {
                for function in &value.declarations {
                    if function.exported {
                        spans.insert(function.name.span);
                    }
                }
            }
            _ => {}
        }
    }
    program
        .functions
        .iter()
        .filter(|f| {
            f.source_definition.is_some_and(|definition| {
                spans.contains(&checked.resolved().scope_table.def(definition).span)
            })
        })
        .map(|f| f.id)
        .collect()
}
fn references(expression: &Expression, output: &mut HashSet<FunctionId>) {
    let mut copy = expression.clone();
    let _ = walk_expression(&mut copy, &mut |expression| {
        match &expression.kind {
            E::Call { function, .. }
            | E::FunctionRef(function)
            | E::ClosureRef { function, .. }
            | E::FunctionAdapter { function, .. } => {
                output.insert(*function);
            }
            E::ActorSpawn {
                constructor: Some(function),
                ..
            } => {
                output.insert(*function);
            }
            E::ActorMessage { handler, .. } => {
                output.insert(*handler);
            }
            E::InterfaceCoerce { adapters, .. } => {
                output.extend(adapters.iter().map(|a| a.function))
            }
            _ => {}
        }
        Ok(())
    });
}
fn call_graph(functions: &[Function]) -> HashMap<FunctionId, HashSet<FunctionId>> {
    functions
        .iter()
        .map(|function| {
            let mut body = function.body.clone();
            let mut targets = HashSet::new();
            let _ = walk_block(&mut body, &mut |expression| {
                // This visitor already recurses; collecting one node avoids repeated traversal.
                match &expression.kind {
                    E::Call { function, .. }
                    | E::FunctionRef(function)
                    | E::ClosureRef { function, .. }
                    | E::FunctionAdapter { function, .. } => {
                        targets.insert(*function);
                    }
                    E::ActorSpawn {
                        constructor: Some(function),
                        ..
                    } => {
                        targets.insert(*function);
                    }
                    E::ActorMessage { handler, .. } => {
                        targets.insert(*handler);
                    }
                    E::InterfaceCoerce { adapters, .. } => {
                        targets.extend(adapters.iter().map(|a| a.function))
                    }
                    _ => {}
                }
                Ok(())
            });
            (function.id, targets)
        })
        .collect()
}
fn closure(ids: &mut HashSet<FunctionId>, graph: &HashMap<FunctionId, HashSet<FunctionId>>) {
    let mut pending = ids.iter().copied().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if let Some(edges) = graph.get(&id) {
            for edge in edges {
                if ids.insert(*edge) {
                    pending.push(*edge);
                }
            }
        }
    }
}

// Float equality authenticates exact bits, including NaN payloads and signed zero.
pub(crate) fn functions_equal(left: &[Function], right: &[Function]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter().zip(right).all(|(left, right)| {
        let mut left = left.clone();
        let mut right = right.clone();
        let a = clear_floats(&mut left.body);
        let b = clear_floats(&mut right.body);
        a == b && left == right
    })
}
pub(crate) fn expression_equal(left: &Expression, right: &Expression) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    let mut a = Vec::new();
    let mut b = Vec::new();
    let _ = walk_expression(&mut left, &mut |e| {
        clear_float(e, &mut a);
        Ok(())
    });
    let _ = walk_expression(&mut right, &mut |e| {
        clear_float(e, &mut b);
        Ok(())
    });
    a == b && left == right
}
fn clear_float(expression: &mut Expression, bits: &mut Vec<u64>) {
    if let E::Float(value) = &mut expression.kind {
        bits.push(value.to_bits());
        *value = 0.0;
    }
}
fn clear_floats(block: &mut Block) -> Vec<u64> {
    let mut bits = Vec::new();
    let _ = walk_block(block, &mut |expression| {
        clear_float(expression, &mut bits);
        Ok(())
    });
    bits
}

pub(crate) fn walk_block(
    block: &mut Block,
    visit: &mut impl FnMut(&mut Expression) -> Result<(), String>,
) -> Result<(), String> {
    for statement in &mut block.statements {
        match &mut statement.kind {
            S::Let { value, .. }
            | S::Expression(value)
            | S::HandleDefault(value)
            | S::Respond(value) => walk_expression(value, visit)?,
            S::Assign { target, value } => {
                walk_expression(target, visit)?;
                walk_expression(value, visit)?;
            }
            S::Return(value) => {
                if let Some(value) = value {
                    walk_expression(value, visit)?;
                }
            }
            S::If {
                condition,
                then_block,
                else_block,
            } => {
                walk_expression(condition, visit)?;
                walk_block(then_block, visit)?;
                if let Some(block) = else_block {
                    walk_block(block, visit)?;
                }
            }
            S::While { condition, body } => {
                walk_expression(condition, visit)?;
                walk_block(body, visit)?;
            }
            S::For { iterable, body, .. } => {
                walk_expression(iterable, visit)?;
                walk_block(body, visit)?;
            }
            S::Match { scrutinee, arms } => {
                walk_expression(scrutinee, visit)?;
                for arm in arms {
                    walk_block(&mut arm.body, visit)?;
                }
            }
            S::Assert { condition, message } => {
                walk_expression(condition, visit)?;
                if let Some(value) = message {
                    walk_expression(value, visit)?;
                }
            }
            S::Breakpoint { condition, .. } => {
                if let Some(value) = condition {
                    walk_expression(value, visit)?;
                }
            }
            S::Scope(block) => walk_block(block, visit)?,
            S::ReflectedTypeDispatch { type_info, arms } => {
                walk_expression(type_info, visit)?;
                for arm in arms {
                    walk_block(&mut arm.body, visit)?;
                }
            }
            S::Break | S::Continue | S::Trace(_) => {}
        }
    }
    Ok(())
}
fn walk_expression(
    expression: &mut Expression,
    visit: &mut impl FnMut(&mut Expression) -> Result<(), String>,
) -> Result<(), String> {
    visit(expression)?;
    match &mut expression.kind {
        E::Binary { left, right, .. } => {
            walk_expression(left, visit)?;
            walk_expression(right, visit)?;
        }
        E::Unary { value, .. }
        | E::ResultOk(value)
        | E::ResultFail(value)
        | E::OptionalSome(value)
        | E::DisplayResult(value)
        | E::EquatableResult(value)
        | E::Comptime { value, .. }
        | E::Declassify(value)
        | E::Coarsen(value)
        | E::RefinementValidated(value)
        | E::InterfaceCoerce { value, .. }
        | E::FunctionAdapter { value, .. }
        | E::InterfaceType(value)
        | E::RuntimeFailureMessage(value)
        | E::StateIs { value, .. }
        | E::Run(value)
        | E::Join(value)
        | E::Cancel(value)
        | E::Field { base: value, .. }
        | E::View(value)
        | E::Clone(value) => walk_expression(value, visit)?,
        E::Call { args, .. }
        | E::Intrinsic { args, .. }
        | E::ResourceInvoke { args, .. }
        | E::ActorSpawn { args, .. } => {
            for value in args {
                walk_expression(value, visit)?;
            }
        }
        E::IndirectCall { callee, args, .. } => {
            for value in args {
                walk_expression(value, visit)?;
            }
            walk_expression(callee, visit)?;
        }
        E::ActorMessage { actor, args, .. } => {
            walk_expression(actor, visit)?;
            for value in args {
                walk_expression(value, visit)?;
            }
        }
        E::StructConstruct { fields, .. } | E::BitfieldConstruct { fields, .. } => {
            for value in fields {
                walk_expression(value, visit)?;
            }
        }
        E::MachineConstruct { payloads, .. } | E::EnumConstruct { payloads, .. } => {
            for value in payloads {
                walk_expression(value, visit)?;
            }
        }
        E::MachineTransition {
            source, payloads, ..
        } => {
            walk_expression(source, visit)?;
            for value in payloads {
                walk_expression(value, visit)?;
            }
        }
        E::ListConstruct { elements } => {
            for value in elements {
                walk_expression(value, visit)?;
            }
        }
        E::MapConstruct { entries } => {
            for entry in entries {
                walk_expression(&mut entry.key, visit)?;
                walk_expression(&mut entry.value, visit)?;
            }
        }
        E::Handle {
            target, failure, ..
        } => {
            walk_expression(target, visit)?;
            walk_block(failure, visit)?;
        }
        E::StringInterpolation(parts) => {
            for part in parts {
                if let StringSegment::Value(value) = part {
                    walk_expression(value, visit)?;
                }
            }
        }
        E::InlineFunction { body, .. } => walk_block(body, visit)?,
        E::Int(_)
        | E::Float(_)
        | E::String(_)
        | E::Bool(_)
        | E::Nothing
        | E::Local(_)
        | E::Constant { .. }
        | E::FunctionRef(_)
        | E::ResourceHookValue { .. }
        | E::ClosureRef { .. }
        | E::OptionalNone
        | E::RuntimeFailure(_)
        | E::PropertyCaseContext(_) => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "resource_materialization_tests.rs"]
mod tests;
