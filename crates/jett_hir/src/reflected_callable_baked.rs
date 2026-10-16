//! Private checked transition for ordinary required values in an adapter owner.
use super::*;
use jett_comptime::{ExplicitComptimeValues, Value};

#[path = "required_bake_receipt.rs"]
mod required_bake_receipt;
pub(crate) use required_bake_receipt::CheckedRequiredBake;

#[derive(Debug)]
pub(crate) struct Transition {
    checked: Arc<jett_typecheck::CheckedResourceProgram>,
    original: Vec<Function>,
    cache: ExplicitComptimeValues,
    property: Option<crate::CheckedNativePropertyPlan>,
    current: Vec<Function>,
}

impl Transition {
    pub(crate) fn current_functions(&self) -> &[Function] {
        &self.current
    }
}

fn equal(left: &[Function], right: &[Function]) -> bool {
    let left = left.iter().map(erase_function).collect::<Vec<_>>();
    let right = right.iter().map(erase_function).collect::<Vec<_>>();
    super::super::resource_materialization::functions_equal(&left, &right)
}

pub fn has_reflected_callable_plans(program: &Program) -> bool {
    program
        .functions
        .iter()
        .any(|function| !reflected_callable_plans(function).is_empty())
}

/// The callback constructs typed HIR only. Every result, added local and binding
/// is checked against the private required cache before becoming a transition.
pub fn materialize_checked_reflected_values(
    program: &mut Program,
    values: &ExplicitComptimeValues,
    types: &TypeInterner,
    mut materialize: impl FnMut(
        &Value,
        TypeId,
        Span,
        &mut Vec<Local>,
        &mut Vec<Statement>,
    ) -> Result<Expression, String>,
) -> Result<(), Vec<LowerError>> {
    let span = program
        .functions
        .first()
        .map_or(Span::new(FileId::new(0), 0, 0), |function| function.span);
    let result = (|| -> Result<Program, String> {
        let bake = required_bake_receipt::produce(program, values, types, &mut materialize)?;
        let mut candidate = bake.candidate();
        crate::actor_handlers::required_bake(&mut candidate, program, values, types, &bake)?;
        let checked = bake.checked().clone();
        let original = bake.original_functions().to_vec();
        // Every ordinary replacement and added local has passed the existing
        // private value matcher above. Renew Graph custody at this fixed boundary
        // before installing the corresponding reflected transition.
        super::super::actor_graph::complete_conversion(&mut candidate, program)
            .map_err(|errors| format!("invalid combined Graph transition: {errors:?}"))?;
        candidate.resource_source.reflected_values = Some(Arc::new(Transition {
            checked,
            original,
            cache: values.clone(),
            property: None,
            current: candidate.functions.clone(),
        }));
        if crate::temporary_local_views::has_markers(program)
            || program
                .resource_source
                .required_materializations()
                .iter()
                .any(|row| row.proof.actor_graph().is_some())
        {
            candidate
                .resource_source
                .install_materialized(crate::resource_source::Materialized {
                    functions: candidate.functions.clone(),
                    values: program.resource_source.required_materializations().to_vec(),
                    required_only: program
                        .resource_source
                        .required_only_function_ids()
                        .to_vec(),
                });
        }
        Ok(candidate)
    })();
    match result {
        Ok(candidate) => {
            *program = candidate;
            Ok(())
        }
        Err(message) => Err(vec![LowerError { span, message }]),
    }
}

pub(super) fn original_function(
    function: &Function,
    archive: &ResourceSourceArchive,
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
) -> Result<Option<Function>, String> {
    let Some(transition) = &archive.reflected_values else {
        return Ok(None);
    };
    if !archive.belongs_to(checked) || !Arc::ptr_eq(&transition.checked, checked) {
        return Err("ordinary reflected materialization belongs to another checked Source".into());
    }
    let Some(current) = transition.current.get(function.id.index() as usize) else {
        return Ok(None);
    };
    if !equal(
        std::slice::from_ref(function),
        std::slice::from_ref(current),
    ) {
        return Ok(None);
    }
    Ok(transition
        .original
        .get(function.id.index() as usize)
        .cloned())
}

pub(crate) fn validate_conversion_input(program: &Program) -> Result<(), Vec<LowerError>> {
    let Some(transition) = &program.resource_source.reflected_values else {
        return Ok(());
    };
    if !program.resource_source.belongs_to(&transition.checked)
        || !equal(&program.functions, &transition.current)
    {
        return Err(vec![LowerError {
            span: program
                .functions
                .first()
                .map_or(Span::new(FileId::new(0), 0, 0), |function| function.span),
            message: "reflected conversion changed its protected required-value input".into(),
        }]);
    }
    Ok(())
}

/// Called only around the ordinary canonical conversion pass after its gate.
pub(crate) fn complete_conversion(
    program: &mut Program,
    before: &Program,
) -> Result<(), Vec<LowerError>> {
    validate_conversion_input(before)?;
    let Some(transition) = &before.resource_source.reflected_values else {
        return Ok(());
    };
    if program.resource_source != before.resource_source {
        return Err(vec![LowerError {
            span: before.functions[0].span,
            message: "reflected conversion replaced original archive".into(),
        }]);
    }
    program.resource_source.reflected_values = Some(Arc::new(Transition {
        checked: transition.checked.clone(),
        original: transition.original.clone(),
        cache: transition.cache.clone(),
        property: transition.property.clone(),
        current: program.functions.clone(),
    }));
    Ok(())
}

/// Renew this exact checked cache view from the retained actual sealer output.
/// Full function equality includes actor metadata; no erased view can grant it.
pub(super) fn callable_sealing(
    program: &Program,
    before: &Program,
    types: &TypeInterner,
    receipt: &super::CheckedCallableSealing<'_>,
) -> Result<Option<Arc<Transition>>, Vec<LowerError>> {
    let result = (|| -> Result<Option<Arc<Transition>>, String> {
        let Some(transition) = &before.resource_source.reflected_values else {
            return Ok(None);
        };
        let mut replay = receipt.replay(before, types)?;
        if program.resource_source != before.resource_source
            || program.resource_manifest != before.resource_manifest
            || program.equality_methods != before.equality_methods
            || !before.resource_source.belongs_to(&transition.checked)
            || !receipt
                .checked()
                .is_some_and(|checked| Arc::ptr_eq(checked, &transition.checked))
            || !crate::resource_materialization::functions_equal(
                &before.functions,
                &transition.current,
            )
        {
            return Err(
                "baked callable sealing changed its exact checked cache or full input table".into(),
            );
        }
        // This peer's real renewal changes private snapshots. Replay that same
        // producer on retained actual sealer output, preserving every actor field.
        crate::mutable_sum_backings::callable_sealing(&mut replay, before, &receipt.token_pairs())
            .map_err(|errors| format!("baked callable sealing sum replay: {errors:?}"))?;
        for checked_program in [before, program] {
            crate::actor_handlers::validate_program(checked_program)
                .map_err(|errors| format!("baked callable sealing actor snapshot: {errors:?}"))?;
        }
        if !crate::resource_materialization::functions_equal(&replay.functions, &program.functions)
        {
            return Err("baked callable sealing differs from its retained complete output".into());
        }
        Ok(Some(Arc::new(Transition {
            checked: transition.checked.clone(),
            original: transition.original.clone(),
            cache: transition.cache.clone(),
            property: transition.property.clone(),
            current: replay.functions,
        })))
    })();
    result.map_err(|message| {
        vec![LowerError {
            span: before
                .functions
                .first()
                .map_or(Span::new(FileId::new(0), 0, 0), |function| function.span),
            message,
        }]
    })
}

pub(super) fn owns_materialized_owner(archive: &ResourceSourceArchive, owner: &Function) -> bool {
    archive.reflected_values.as_ref().is_some_and(|transition| {
        archive.belongs_to(&transition.checked)
            && transition
                .current
                .get(owner.id.index() as usize)
                .is_some_and(|current| {
                    equal(std::slice::from_ref(current), std::slice::from_ref(owner))
                })
    })
}

/// Readonly association with the private successful required-cache snapshot.
pub(crate) fn same_required_cache(program: &Program, values: &ExplicitComptimeValues) -> bool {
    program
        .resource_source
        .reflected_values
        .as_ref()
        .is_some_and(|transition| {
            program.resource_source.belongs_to(&transition.checked)
                && transition.cache.same_checked_cache(values)
        })
}
pub(crate) fn has_property_suite(program: &Program) -> bool {
    program
        .resource_source
        .reflected_values
        .as_ref()
        .is_some_and(|transition| transition.property.is_some())
}
pub(crate) fn owns_property_suite(
    program: &Program,
    plan: &crate::CheckedNativePropertyPlan,
) -> bool {
    program
        .resource_source
        .reflected_values
        .as_ref()
        .and_then(|transition| transition.property.as_ref())
        .is_some_and(|current| current.same_plan(plan))
}
/// Only the private canonical Property constructor calls this after reconstructing
/// its complete wrapper. Public mutable HIR has no append/reseal entry point.
pub(crate) fn install_property_suite(
    program: &mut Program,
    plan: crate::CheckedNativePropertyPlan,
) -> Result<(), Vec<LowerError>> {
    validate_conversion_input(program)?;
    let transition = program
        .resource_source
        .reflected_values
        .as_ref()
        .ok_or_else(|| {
            vec![LowerError {
                span: program.functions[0].span,
                message: "native Property transition lost its protected required-value graph"
                    .into(),
            }]
        })?;
    if transition.property.is_some() {
        return Err(vec![LowerError {
            span: program.functions[0].span,
            message: "native Property transition already owns a generated suite".into(),
        }]);
    }
    program.resource_source.reflected_values = Some(Arc::new(Transition {
        checked: transition.checked.clone(),
        original: transition.original.clone(),
        cache: transition.cache.clone(),
        property: Some(plan),
        current: transition.current.clone(),
    }));
    Ok(())
}

#[cfg(test)]
#[path = "actor_required_bake_tests.rs"]
mod actor_required_bake_tests;
