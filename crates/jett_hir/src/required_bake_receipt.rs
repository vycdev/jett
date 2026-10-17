//! Ephemeral receipt of the actual checked ordinary required-value bake.
use super::*;

struct Bake<'types> {
    types: &'types TypeInterner,
    before: Program,
    after: Program,
    checked: Arc<jett_typecheck::CheckedResourceProgram>,
    original: Vec<Function>,
    cache: ExplicitComptimeValues,
}

/// Minted only after the private producer authenticates every replacement and
/// generated binding. No archive stores this temporary receipt.
pub(crate) struct CheckedRequiredBake<'types>(Box<Bake<'types>>);

fn metadata_equal(left: &Program, right: &Program) -> bool {
    let Program {
        resource_manifest,
        resource_source,
        functions: _,
        equality_methods,
    } = left;
    let Program {
        resource_manifest: other_manifest,
        resource_source: other_source,
        functions: _,
        equality_methods: other_equality,
    } = right;
    resource_manifest == other_manifest
        && resource_source == other_source
        && equality_methods == other_equality
}

impl CheckedRequiredBake<'_> {
    pub(super) fn candidate(&self) -> Program {
        self.0.after.clone()
    }
    pub(super) fn original_functions(&self) -> &[Function] {
        &self.0.original
    }
    pub(super) fn checked(&self) -> &Arc<jett_typecheck::CheckedResourceProgram> {
        &self.0.checked
    }

    pub(crate) fn authenticate_output(
        &self,
        before: &Program,
        after: &Program,
        types: &TypeInterner,
    ) -> Result<(), String> {
        if !std::ptr::eq(types, self.0.types)
            || !metadata_equal(before, &self.0.before)
            || !crate::resource_materialization::functions_equal(
                &before.functions,
                &self.0.before.functions,
            )
            || !metadata_equal(after, &self.0.after)
            || !crate::resource_materialization::functions_equal(
                &after.functions,
                &self.0.after.functions,
            )
        {
            return Err(
                "required bake receipt has another complete input, output or type arena".into(),
            );
        }
        Ok(())
    }

    pub(crate) fn replay(
        &self,
        before: &Program,
        values: &ExplicitComptimeValues,
        types: &TypeInterner,
    ) -> Result<Program, String> {
        self.authenticate_output(before, &self.0.after, types)?;
        if !self.0.cache.same_checked_cache(values)
            || !self.0.before.resource_source.belongs_to(&self.0.checked)
        {
            return Err(
                "required bake receipt has another successful cache or checked program".into(),
            );
        }
        Ok(self.0.after.clone())
    }
}

/// This is the sole production constructor. A caller supplies a typed value
/// constructor, never a positive transition proof or an output function table.
pub(super) fn produce<'types>(
    program: &Program,
    values: &ExplicitComptimeValues,
    types: &'types TypeInterner,
    mut materialize: impl FnMut(
        &Value,
        TypeId,
        Span,
        &mut Vec<Local>,
        &mut Vec<Statement>,
    ) -> Result<Expression, String>,
) -> Result<CheckedRequiredBake<'types>, String> {
    if program.resource_source.checked_program().is_none()
        || !program.resource_manifest.kinds().is_empty()
    {
        return Err("ordinary required transition has no retained checked ordinary archive".into());
    }
    crate::actor_graph::validate_conversion_input(program)
        .map_err(|errors| format!("invalid protected Graph input: {errors:?}"))?;
    crate::reflected_callable::validate_program(program)
        .map_err(|errors| format!("invalid original reflected owner: {errors:?}"))?;
    program.resource_source.validate_current(program, types)?;
    crate::actor_handlers::validate_program(program)
        .map_err(|errors| format!("invalid original actor snapshot: {errors:?}"))?;
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
        .map(|function| {
            crate::reflected_callable::original_function(
                function,
                &program.resource_source,
                &checked,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !super::equal(&original, program.resource_source.functions()) {
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
        crate::resource_materialization::walk_block(&mut function.body, &mut |expression| {
            if !matches!(
                expression.kind,
                ExpressionKind::Constant { .. } | ExpressionKind::Comptime { .. }
            ) {
                return Ok(());
            }
            let selected = proofs
                .iter()
                .filter(|proof| {
                    crate::resource_materialization::proof_matches(
                        proof,
                        &owner,
                        expression,
                        &checked,
                        &program.resource_source,
                    )
                })
                .collect::<Vec<_>>();
            let [proof] = selected.as_slice() else {
                return Err("required value lacks a unique original checked owner/context".into());
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
        })?;
        if function.locals.get(..original_local_count) != Some(owner.locals.as_slice()) {
            return Err("required materializer changed an original local".into());
        }
        function.body.statements.splice(0..0, bindings.clone());
        let used = crate::reflected_callable::materialized_source::validate_required_values(
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

    Ok(CheckedRequiredBake(Box::new(Bake {
        types,
        before: program.clone(),
        after: candidate,
        checked,
        original,
        cache: values.clone(),
    })))
}
