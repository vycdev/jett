//! Opaque deterministic cases from the original successful checked worker.
use std::sync::Arc;

use jett_diagnostics::Diagnostic;
use jett_parser::ast::Item;
use jett_typecheck::CheckedResourceProgram;

use super::{
    PROPERTY_DEFAULT_ITERATIONS, PropertyCase, run_verification, verify_results_to_diagnostics,
};
use crate::ExplicitComptimeValues;
use crate::checked_types::CheckedExpressionTypes;

#[derive(Debug, Clone)]
pub struct CheckedPropertyCases {
    data: Arc<Cases>,
}

#[derive(Debug)]
struct Cases {
    program: Arc<CheckedResourceProgram>,
    values: Arc<ExplicitComptimeValues>,
    rows: Vec<PropertyCase>,
}

impl CheckedPropertyCases {
    pub fn cases(&self) -> &[PropertyCase] {
        &self.data.rows
    }

    pub fn belongs_to(&self, program: &Arc<CheckedResourceProgram>) -> bool {
        Arc::ptr_eq(&self.data.program, program)
    }

    pub fn required_values(&self) -> &ExplicitComptimeValues {
        &self.data.values
    }
}

/// No source tree, metadata projection, raw case values, or trial order is supplied.
/// The existing worker and deterministic sampler retain their execution behavior.
pub fn collect_checked_property_cases_with_checked_values(
    program: Arc<CheckedResourceProgram>,
    values: Arc<ExplicitComptimeValues>,
) -> Result<CheckedPropertyCases, Vec<Diagnostic>> {
    let values = Arc::new(
        values
            .successful_checked_view(&program)
            .map_err(|message| vec![Diagnostic::error(9000, message, program.module().span)])?,
    );
    let run = run_verification(
        program.module(),
        Some(program.checked().reflection_metadata.clone()),
        Some(Arc::new(CheckedExpressionTypes::from_original_program(
            &program,
        ))),
        Some(Arc::new(program.checked().breakpoint_exclusions.clone())),
        Some(values.clone()),
        true,
    );
    let diagnostics = verify_results_to_diagnostics(&run.results);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let mut rows = run.property_cases.iter();
    for item in &program.module().items {
        let Item::Property(property) = item else {
            continue;
        };
        for iteration in 0..PROPERTY_DEFAULT_ITERATIONS {
            if !rows.next().is_some_and(|row| {
                row.property_span == property.span
                    && row.iteration == iteration
                    && row.arguments.len() == property.givens.len()
            }) {
                return Err(vec![Diagnostic::error(
                    9000,
                    "checked property sampler lost its original property/iteration/parameter order",
                    property.span,
                )]);
            }
        }
    }
    if rows.next().is_some() {
        return Err(vec![Diagnostic::error(
            9000,
            "checked property sampler retained an extra trial",
            program.module().span,
        )]);
    }
    Ok(CheckedPropertyCases {
        data: Arc::new(Cases {
            program,
            values,
            rows: run.property_cases,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ComptimeContext, Value};
    use jett_common::{FileId, SourceOrigin, Span};
    use std::collections::HashMap;

    fn program(source: &str) -> Arc<CheckedResourceProgram> {
        let file = FileId::new(0);
        Arc::new(
            CheckedResourceProgram::prepare(
                jett_parser::parse(source, file),
                HashMap::from([(file, SourceOrigin::Project)]),
                &[],
                jett_typecheck::CheckOptions::default(),
            )
            .unwrap(),
        )
    }

    fn capture(program: &Arc<CheckedResourceProgram>) -> crate::ExplicitComptimeEvaluation {
        crate::evaluate_explicit_comptime_expressions_capture(
            program.module(),
            program.checked().reflection_metadata.clone(),
            Arc::new(CheckedExpressionTypes::from_checked_program(program)),
            Arc::new(program.checked().breakpoint_exclusions.clone()),
        )
    }

    const SOURCE: &str =
        "namespace app\nproperty scalar:\n    given value: int64\n    assert value == value\n";

    #[test]
    fn empty_success_retains_exact_program_cache_and_one_hundred_ordered_trials() {
        let checked = program(SOURCE);
        let evaluation = capture(&checked);
        assert!(evaluation.diagnostics.is_empty());
        assert!(evaluation.values.is_empty());
        let values = Arc::new(evaluation.values);
        let cases =
            collect_checked_property_cases_with_checked_values(checked.clone(), values.clone())
                .unwrap();
        assert!(cases.belongs_to(&checked));
        assert!(cases.required_values().same_checked_cache(&values));
        assert!(!capture(&checked).values.same_checked_cache(&values));
        assert_eq!(cases.cases().len(), PROPERTY_DEFAULT_ITERATIONS);
        for (iteration, case) in cases.cases().iter().enumerate() {
            assert_eq!(case.iteration, iteration);
            assert_eq!(case.arguments.len(), 1);
        }
        let foreign = program(SOURCE);
        assert!(!cases.belongs_to(&foreign));
        assert!(collect_checked_property_cases_with_checked_values(foreign, values).is_err());
    }

    #[test]
    fn public_mirror_inserts_do_not_change_the_private_successful_worker_view() {
        let checked = program(SOURCE);
        let original = capture(&checked).values;
        let mut mirrors = original.clone();
        let span = Span::new(FileId::new(0), u32::MAX - 1, u32::MAX);
        mirrors.insert(span, ComptimeContext::default(), Value::Int64(42));
        assert!(mirrors.get(span, &ComptimeContext::default()).is_some());
        assert!(mirrors.same_checked_cache(&original));
        let cases =
            collect_checked_property_cases_with_checked_values(checked, Arc::new(mirrors)).unwrap();
        assert!(cases.required_values().same_checked_cache(&original));
        assert!(
            cases
                .required_values()
                .get(span, &ComptimeContext::default())
                .is_none()
        );
    }

    #[test]
    fn unstamped_or_forged_projections_cannot_mint_cases_but_old_reads_still_work() {
        let checked = program(SOURCE);
        let mut public = ExplicitComptimeValues::default();
        public.insert(
            checked.module().span,
            ComptimeContext::default(),
            Value::Int64(9),
        );
        assert!(
            public
                .get(checked.module().span, &ComptimeContext::default())
                .is_some()
        );
        assert!(public.checked_required_values(&checked).unwrap().is_empty());
        assert!(
            collect_checked_property_cases_with_checked_values(checked.clone(), Arc::new(public))
                .is_err()
        );
        for projection in 0..5 {
            let mut types = CheckedExpressionTypes::from_original_program(&checked);
            let mut metadata = checked.checked().reflection_metadata.clone();
            let mut exclusions = checked.checked().breakpoint_exclusions.clone();
            match projection {
                0 => {
                    types
                        .expressions
                        .insert(checked.module().span, "bool".into());
                }
                1 => {
                    types.functions.insert(
                        checked.module().span,
                        vec![Arc::new(
                            crate::checked_types::CheckedFunctionTypes::default(),
                        )],
                    );
                }
                2 => {
                    metadata = Arc::new(jett_types::ReflectionMetadata::new());
                }
                3 => {
                    exclusions.insert(checked.module().span, std::collections::HashSet::new());
                }
                4 => {
                    types.bindings.insert(
                        checked.module().span,
                        vec![Arc::new(crate::checked_types::CheckedScopedTypes::default())],
                    );
                }
                _ => unreachable!(),
            }
            let evaluation = crate::evaluate_explicit_comptime_expressions_capture(
                checked.module(),
                metadata,
                Arc::new(types),
                Arc::new(exclusions),
            );
            // Empty old evaluation still succeeds; it simply grants no new proof.
            assert!(evaluation.diagnostics.is_empty());
            assert!(
                collect_checked_property_cases_with_checked_values(
                    checked.clone(),
                    Arc::new(evaluation.values)
                )
                .is_err()
            );
        }
    }

    #[test]
    fn failed_required_evaluation_and_failed_property_do_not_mint_opaque_cases() {
        let failed = program(
            "namespace app\nfunction fail_required() returns int64:\n    list[int64] rejected = range(0, 9223372036854775807)\n    return 0\nint64 cached = comptime fail_required()\nproperty scalar:\n    given value: int64\n    assert value == value\n",
        );
        let evaluation = capture(&failed);
        assert!(
            evaluation.diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("range: requested output is too large")),
            "{:?}",
            evaluation.diagnostics
        );
        assert!(
            collect_checked_property_cases_with_checked_values(failed, Arc::new(evaluation.values))
                .is_err()
        );
        let failed_property =
            program("namespace app\nproperty failing:\n    given value: int64\n    assert false\n");
        let values = capture(&failed_property).values;
        assert!(
            collect_checked_property_cases_with_checked_values(failed_property, Arc::new(values))
                .is_err()
        );
    }

    #[test]
    fn snapshot_observer_preserves_float_bits_typed_tags_and_pending_depth() {
        let first = Value::Float64(f64::from_bits(0x7ff8_0000_0000_0123));
        assert!(first.same_checked_snapshot(&first.clone()));
        assert!(
            !first.same_checked_snapshot(&Value::Float64(f64::from_bits(0x7ff8_0000_0000_0124)))
        );
        let tagged = Value::Pending(Box::new(Value::Typed {
            type_name: "app.Positive".into(),
            value: Box::new(Value::Int64(5)),
        }));
        assert!(tagged.same_checked_snapshot(&tagged.clone()));
        assert!(
            !tagged.same_checked_snapshot(&Value::Pending(Box::new(Value::Pending(Box::new(
                Value::Typed {
                    type_name: "app.Positive".into(),
                    value: Box::new(Value::Int64(5)),
                }
            )))))
        );
        assert!(
            !tagged.same_checked_snapshot(&Value::Pending(Box::new(Value::Typed {
                type_name: "app.Foreign".into(),
                value: Box::new(Value::Int64(5)),
            })))
        );
    }

    #[test]
    fn callable_observer_authenticates_only_the_original_checked_signature() {
        let checked = program(
            "namespace app\nfunction five() returns int64:\n    return 5\nproperty scalar:\n    given value: int64\n    assert value == value\n",
        );
        let definition = checked
            .resolved()
            .scope_table
            .definitions
            .iter()
            .find(|definition| definition.name == "app.five")
            .unwrap();
        let actual = checked.checked().definition_types[&definition.id];
        assert!(
            Value::NamedFunction("app.five".into())
                .validate_checked_callable_source(&checked, actual)
                .is_ok()
        );
        assert!(
            Value::NamedFunction("app.absent".into())
                .validate_checked_callable_source(&checked, actual)
                .is_err()
        );
        assert!(
            Value::Int64(5)
                .validate_checked_callable_source(&checked, actual)
                .is_err()
        );
    }
}
