//! Required evaluation and native materialization share one checked Resource session.
use std::sync::Arc;

use jett_comptime::{ExplicitComptimeEvaluation, ExplicitComptimeValues};
use jett_hir::{LowerError, Program};
use jett_typecheck::CheckedResourceProgram;

/// Evaluate required values with the exact checked module and no runtime provider.
/// Observations belong to this phase; reading the cache never replays them.
pub fn evaluate_checked_resource_required_values(
    checked: &Arc<CheckedResourceProgram>,
) -> ExplicitComptimeEvaluation {
    let mut types = crate::expression_type_names(checked.checked(), checked.resolved());
    types.resource_program = Some(checked.clone());
    jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        checked.checked().reflection_metadata.clone(),
        Arc::new(types),
        Arc::new(checked.checked().breakpoint_exclusions.clone()),
    )
}

/// Bake required values without replacing the retained original Source archive.
/// Resource authority comes from the private checked cache, never its public mirror.
pub fn bake_checked_resource_values(
    program: &mut Program,
    checked: &Arc<CheckedResourceProgram>,
    values: &ExplicitComptimeValues,
) -> Result<(), Vec<LowerError>> {
    if !program.resource_source.belongs_to(checked) {
        return Err(vec![LowerError {
            span: checked.module().span,
            message: "native required-value materialization received a foreign checked program"
                .into(),
        }]);
    }
    crate::native_constants::bake_values(
        program,
        values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use jett_common::{FileId, SourceOrigin};
    use jett_parser::{ast::Item, parse};
    use jett_resolve::ResourceKernelSpec;
    use jett_typecheck::CheckOptions;
    use jett_types::ResourceKernelRecipe;
    use std::collections::HashMap;

    fn checked(release: bool) -> Arc<CheckedResourceProgram> {
        let support = FileId::new(10_000);
        let primary = FileId::new(0);
        let mut parsed = parse(
            include_str!("../tests/native_conformance/resource/resource_probe.jett"),
            support,
        );
        let project = parse(
            include_str!("../tests/native_conformance/resource/38_comptime_hook_unused.jett"),
            primary,
        );
        assert!(parsed.errors.is_empty());
        assert!(project.errors.is_empty());
        let declarations = parsed
            .module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Resource(resource) => Some(resource.name.span),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [declaration] = declarations.as_slice() else {
            panic!("one original synthetic Resource declaration");
        };
        let catalog = [
            ("kernel_create", ResourceKernelRecipe::NetworkFactory),
            ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
            ("kernel_close", ResourceKernelRecipe::Finalize),
        ]
        .into_iter()
        .map(|(member, recipe)| ResourceKernelSpec {
            resource_declaration: *declaration,
            member: member.into(),
            recipe,
        })
        .collect::<Vec<_>>();
        parsed.module.items.extend(project.module.items);
        Arc::new(
            CheckedResourceProgram::prepare(
                parsed,
                HashMap::from([
                    (support, SourceOrigin::Stdlib),
                    (primary, SourceOrigin::Project),
                ]),
                &catalog,
                CheckOptions { release },
            )
            .unwrap(),
        )
    }

    #[test]
    fn native_required_materialization_rejects_foreign_checked_context() {
        for release in [false, true] {
            let original = checked(release);
            let same_source_foreign = checked(release);
            let required = evaluate_checked_resource_required_values(&original);
            assert!(required.diagnostics.is_empty());
            let mut program = jett_hir::lower_checked_resource_program(&original).unwrap();
            let before = program.clone();
            let errors =
                bake_checked_resource_values(&mut program, &same_source_foreign, &required.values)
                    .unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("foreign checked program"))
            );
            assert_eq!(program, before);
            bake_checked_resource_values(&mut program, &original, &required.values).unwrap();
        }
    }
}
