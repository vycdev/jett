//! Join interpreter projections to complete immutable checker facts.
use super::OriginalClosureOwner;
use crate::checked_types::{
    CheckedExpressionTypes, CheckedFunctionTypes, CheckedScopedBindings, CheckedScopedTypes,
};
use crate::value::ClosureTypeContext;
use jett_common::Span;
use jett_typecheck::{CheckedComptimeTypeBinding, CheckedGenericFunctionInstantiation};
use jett_types::{TypeId, TypeInterner};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

fn remove_scoped(
    names: &mut HashMap<Span, String>,
    bindings: &HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
) {
    for binding in bindings.values().flatten() {
        for span in binding.body.type_map.keys() {
            names.remove(span);
        }
        remove_scoped(names, &binding.body.comptime_type_bindings);
    }
}
fn projection(
    types: &HashMap<Span, TypeId>,
    bindings: &HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
    interner: &TypeInterner,
) -> (HashMap<Span, String>, CheckedScopedBindings) {
    let mut names = types
        .iter()
        .map(|(span, ty)| (*span, interner.type_name(*ty)))
        .collect();
    remove_scoped(&mut names, bindings);
    let nested: HashSet<_> = bindings
        .values()
        .flatten()
        .flat_map(|binding| binding.body.comptime_type_bindings.keys().copied())
        .collect();
    let scopes = bindings
        .iter()
        .filter(|(span, _)| !nested.contains(span))
        .map(|(span, candidates)| {
            (
                *span,
                candidates
                    .iter()
                    .map(|binding| {
                        let (expressions, bindings) = projection(
                            &binding.body.type_map,
                            &binding.body.comptime_type_bindings,
                            interner,
                        );
                        Arc::new(CheckedScopedTypes {
                            bound_type: interner.type_name(binding.bound_type),
                            reflection: Some(binding.reflection.clone()),
                            expressions,
                            bindings,
                        })
                    })
                    .collect(),
            )
        })
        .collect();
    (names, scopes)
}
pub(super) fn root_matches(
    checked: &jett_typecheck::CheckedResourceProgram,
    installed: &CheckedExpressionTypes,
) -> bool {
    let checked = checked.checked();
    let (mut expressions, bindings) = projection(
        &checked.type_map,
        &checked.comptime_type_bindings,
        &checked.interner,
    );
    for instance in &checked.generic_function_instantiations {
        for span in instance.type_map.keys() {
            expressions.remove(span);
        }
        remove_scoped(&mut expressions, &instance.comptime_type_bindings);
    }
    installed.expressions == expressions && installed.bindings == bindings
}
fn function_matches(
    current: &CheckedFunctionTypes,
    context: &ClosureTypeContext,
    original: &CheckedGenericFunctionInstantiation,
    interner: &TypeInterner,
) -> bool {
    let (names, bindings) = projection(
        &original.type_map,
        &original.comptime_type_bindings,
        interner,
    );
    let args: Vec<_> = original
        .concrete_args
        .iter()
        .map(|ty| interner.type_name(*ty))
        .collect();
    let facts = &original.specialization;
    current.expressions.as_ref() == &names
        && current.bindings == bindings
        && current.type_arguments == args
        && current.type_argument_reflections == facts.type_argument_reflections
        && current.type_info_kinds == facts.type_info_kinds
        && current.type_info_primitives == facts.type_info_primitives
        && current.type_kind_values == facts.type_kind_values
        && current.type_primitive_values == facts.type_primitive_values
        && context.arguments.len() == args.len()
        && context
            .arguments
            .iter()
            .zip(&args)
            .all(|(actual, expected)| &actual.canonical_name == expected)
        && (facts.type_argument_reflections.is_empty()
            || context
                .arguments
                .iter()
                .zip(&facts.type_argument_reflections)
                .all(|(actual, expected)| actual.reflection.as_ref() == Some(expected)))
}
fn scoped_types(
    current: &CheckedScopedTypes,
    span: Span,
    bindings: &HashMap<Span, Vec<CheckedComptimeTypeBinding>>,
    interner: &TypeInterner,
    found: &mut Vec<TypeId>,
) {
    for original in bindings.values().flatten() {
        let (expressions, bindings) = projection(
            &original.body.type_map,
            &original.body.comptime_type_bindings,
            interner,
        );
        let expected = CheckedScopedTypes {
            bound_type: interner.type_name(original.bound_type),
            reflection: Some(original.reflection.clone()),
            expressions,
            bindings,
        };
        if current == &expected {
            if let Some(ty) = original.body.type_map.get(&span) {
                found.push(*ty);
            }
        }
        scoped_types(
            current,
            span,
            &original.body.comptime_type_bindings,
            interner,
            found,
        );
    }
}
pub(super) fn callable_type(
    origin: &OriginalClosureOwner,
    context: &ClosureTypeContext,
    span: Span,
) -> Option<TypeId> {
    let checked = origin.checked.checked();
    let mut found = Vec::new();
    if let Some(current) = &context.checked_function {
        for original in &checked.generic_function_instantiations {
            if original.definition != origin.definition
                || !function_matches(current, context, original, &checked.interner)
            {
                continue;
            }
            if let Some(scope) = &context.checked_scope {
                scoped_types(
                    scope,
                    span,
                    &original.comptime_type_bindings,
                    &checked.interner,
                    &mut found,
                );
            } else if let Some(ty) = original.type_map.get(&span) {
                found.push(*ty);
            }
        }
    } else if context.arguments.is_empty() {
        if let Some(scope) = &context.checked_scope {
            scoped_types(
                scope,
                span,
                &checked.comptime_type_bindings,
                &checked.interner,
                &mut found,
            );
        } else if let Some(ty) = checked.type_map.get(&span) {
            found.push(*ty);
        }
    }
    let selected = *found.first()?;
    found.iter().all(|ty| *ty == selected).then_some(selected)
}
