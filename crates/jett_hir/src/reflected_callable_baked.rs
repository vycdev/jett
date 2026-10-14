//! Private checked transition for ordinary required values in an adapter owner.
use super::*;
use jett_comptime::{ExplicitComptimeValues, Value};

#[derive(Debug)]
pub(crate) struct Transition {
    checked: Arc<jett_typecheck::CheckedResourceProgram>,
    original: Vec<Function>,
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
        if program.resource_source.checked_program().is_none()
            || !program.resource_manifest.kinds().is_empty()
        {
            return Err(
                "ordinary required transition has no retained checked ordinary archive".into(),
            );
        }
        super::super::actor_graph::validate_conversion_input(program)
            .map_err(|errors| format!("invalid protected Graph input: {errors:?}"))?;
        super::validate_program(program)
            .map_err(|errors| format!("invalid original reflected owner: {errors:?}"))?;
        program.resource_source.validate_types(types)?;
        let checked = program
            .resource_source
            .checked_program()
            .ok_or("reflected required transition lacks original checked Source")?
            .clone();
        // The private Graph witness authenticates the entire derived table,
        // including appended adapters. Original Source still compares exactly.
        let source_count = if program.resource_source.graph_values.is_some() {
            program.resource_source.functions().len()
        } else {
            program.functions.len()
        };
        let original = program
            .functions
            .iter()
            .take(source_count)
            .map(|function| super::original_function(function, &program.resource_source, &checked))
            .collect::<Result<Vec<_>, _>>()?;
        if !equal(&original, program.resource_source.functions()) {
            return Err(
                "reflected required transition changed original Source before construction".into(),
            );
        }
        let proofs = values.checked_required_values(&checked)?;
        let mut candidate = program.clone();
        let functions = program.functions.clone();
        for function in &mut candidate.functions {
            let owner = function.clone();
            let mut bindings = Vec::new();
            let original_local_count = function.locals.len();
            let mut replacements = Vec::new();
            super::super::resource_materialization::walk_block(
                &mut function.body,
                &mut |expression| {
                    if !matches!(
                        expression.kind,
                        ExpressionKind::Constant { .. } | ExpressionKind::Comptime { .. }
                    ) {
                        return Ok(());
                    }
                    let selected = proofs
                        .iter()
                        .filter(|proof| {
                            super::super::resource_materialization::proof_matches(
                                proof, &owner, expression, &checked,
                            )
                        })
                        .collect::<Vec<_>>();
                    let [proof] = selected.as_slice() else {
                        return Err(
                            "required value lacks a unique original checked owner/context".into(),
                        );
                    };
                    let replacement = materialize(
                        proof.value(),
                        expression.ty,
                        expression.span,
                        &mut function.locals,
                        &mut bindings,
                    )?;
                    if replacement.ty != expression.ty || replacement.span != expression.span {
                        return Err("required replacement changed its original typed site".into());
                    }
                    replacements.push(((*proof).clone(), replacement.clone()));
                    *expression = replacement;
                    Ok(())
                },
            )?;
            if function.locals.get(..original_local_count) != Some(owner.locals.as_slice()) {
                return Err("required materializer changed an original local".into());
            }
            function.body.statements.splice(0..0, bindings.clone());
            let used = super::materialized_source::validate_required_values(
                &checked,
                &candidate.resource_source,
                function,
                &functions,
                &replacements,
            )?;
            let added = &function.locals[original_local_count..];
            if added.len() != bindings.len() || used.len() != added.len()
                || added.iter().enumerate().any(|(offset,local)| {
                    local.id.index() as usize != original_local_count + offset
                        || local.mutable || local.view_source.is_some() || local.debug_ty != local.ty
                        || local.debug_type_name.is_some() || !used.contains(&local.id)
                })
                || bindings.iter().any(|statement| !matches!(statement.kind, StatementKind::Let { local, .. } if used.contains(&local)))
            { return Err("required materializer added an unauthenticated local or statement".into()); }
        }
        // Every ordinary replacement and added local has passed the existing
        // private value matcher above. Renew Graph custody at this fixed boundary
        // before installing the corresponding reflected transition.
        super::super::actor_graph::complete_conversion(&mut candidate, program)
            .map_err(|errors| format!("invalid combined Graph transition: {errors:?}"))?;
        candidate.resource_source.reflected_values = Some(Arc::new(Transition {
            checked,
            original,
            current: candidate.functions.clone(),
        }));
        if program
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
        current: program.functions.clone(),
    }));
    Ok(())
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
