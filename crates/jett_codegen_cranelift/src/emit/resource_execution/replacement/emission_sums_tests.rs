//! New whole Sources, actual private endpoints, and canonical pure suites.
use super::*;

const ORIGINAL: &str = include_str!("original_optional_assignment.jett");
const OPTIONAL: &str = include_str!("optional_transitions.jett");
const RESULT: &str = include_str!("result_transitions.jett");
const PRIMITIVE: &str = include_str!("primitive_failure.jett");
const PURE: &str = include_str!("pure_replacement_suites.jett");
const CONDITIONAL_OPTIONAL: &str = include_str!("conditional_optional.jett");
const CONDITIONAL_RESULT: &str = include_str!("conditional_result.jett");

#[test]
fn resource_native_direct_sum_replacements_emit_the_existing_compound_leaf() {
    for release in [false, true] {
        for source in [
            ORIGINAL,
            OPTIONAL,
            RESULT,
            PRIMITIVE,
            CONDITIONAL_OPTIONAL,
            CONDITIONAL_RESULT,
        ] {
            let artifact = emitted(source, release);
            let (_, imports) = symbols(&artifact);
            assert!(
                imports
                    .iter()
                    .any(|name| name == "jett_rt_v1_resource_replace")
            );
            assert!(
                imports
                    .iter()
                    .any(|name| name == "jett_rt_v1_resource_scope_complete")
            );
            assert!(!artifact.bytes.is_empty());
        }
    }
}

#[test]
fn resource_native_replacement_endpoint_projection_refuses_foreign_dropped_duplicated_reordered_rows()
 {
    for release in [false, true] {
        let checked = checked(ORIGINAL, release);
        let program = lower_source(&checked);
        let layout = EmittedResourceLayout::from_program(
            &program,
            &checked.checked().interner,
            entry(&program),
        )
        .unwrap();
        let function = layout.plan().function(entry(&program)).unwrap();
        let replace = function
            .operations()
            .iter()
            .find(|operation| matches!(operation.role(), Role::Replace { .. }))
            .unwrap();
        let Role::Replace {
            destination,
            replacement,
            ..
        } = replace.role()
        else {
            unreachable!();
        };
        let rows = function
            .operations()
            .iter()
            .filter(|operation| {
                operation.site() == replace.site() && !operation.is_expression_operation()
            })
            .collect::<Vec<_>>();
        let transfer = rows
            .iter()
            .copied()
            .find(|operation| {
                matches!(operation.role(), Role::Transfer { source, destination: selected }
                if source == replacement && selected == destination)
            })
            .unwrap();
        let drop_index = rows.iter().position(|operation|
            matches!(operation.role(), Role::Drop { source, .. } if source == destination)
        ).unwrap();
        let transfer_index = rows
            .iter()
            .position(|operation| std::ptr::eq(*operation, transfer))
            .unwrap();
        let validate = |current: &[&jett_mir::ResourceOperation]| {
            super::super::replacement::validate_rows(function, current, replace, transfer)
        };
        validate(&rows).unwrap();
        let mut changed = rows.clone();
        changed.remove(drop_index);
        assert!(validate(&changed).is_err());
        let mut changed = rows.clone();
        changed.push(rows[drop_index]);
        assert!(validate(&changed).is_err());
        let mut changed = rows.clone();
        changed.swap(drop_index, transfer_index);
        assert!(validate(&changed).is_err());

        let independent_checked = super::checked(ORIGINAL, release);
        let independent_program = lower_source(&independent_checked);
        let independent_layout = EmittedResourceLayout::from_program(
            &independent_program,
            &independent_checked.checked().interner,
            entry(&independent_program),
        )
        .unwrap();
        let independent = independent_layout
            .plan()
            .function(entry(&independent_program))
            .unwrap();
        let foreign_rows = independent
            .operations()
            .iter()
            .filter(|operation| {
                operation.site() == replace.site() && !operation.is_expression_operation()
            })
            .collect::<Vec<_>>();
        assert!(!foreign_rows.is_empty());
        assert!(validate(&foreign_rows).is_err());
    }
}

#[test]
fn resource_native_direct_sum_replacement_refuses_changed_mutable_header_and_rhs() {
    for release in [false, true] {
        let checked = checked(ORIGINAL, release);
        let program = lower_source(&checked);
        let selected = entry(&program);
        let mut changed = program.clone();
        let local = changed.functions[selected.index() as usize]
            .locals
            .iter_mut()
            .find(|local| local.mutable)
            .unwrap();
        local.mutable = false;
        assert!(
            super::super::super::emit_for_triple(
                &changed,
                &checked.checked().interner,
                HOST,
                Some(selected),
                CodegenOptions { optimize: release },
            )
            .is_err()
        );
        let mut changed = program.clone();
        let statement = changed.functions[selected.index() as usize]
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.statements)
            .find(|statement| matches!(statement.kind, StatementKind::Assign { .. }))
            .unwrap();
        let StatementKind::Assign { target, value } = &mut statement.kind else {
            unreachable!();
        };
        *value = target.clone();
        assert!(
            super::super::super::emit_for_triple(
                &changed,
                &checked.checked().interner,
                HOST,
                Some(selected),
                CodegenOptions { optimize: release },
            )
            .is_err()
        );
    }
}

#[test]
fn resource_native_pure_sum_replacements_use_real_required_verify_and_property_entries() {
    for release in [false, true] {
        let checked = checked(PURE, release);
        let types = &checked.checked().interner;
        let evaluation = jett_comptime::evaluate_explicit_comptime_expressions_capture(
            checked.module(),
            checked.checked().reflection_metadata.clone(),
            Arc::new(
                jett_comptime::checked_types::CheckedExpressionTypes::from_checked_program(
                    &checked,
                ),
            ),
            Arc::new(checked.checked().breakpoint_exclusions.clone()),
        );
        assert!(
            evaluation.diagnostics.is_empty(),
            "{:?}",
            evaluation.diagnostics
        );
        assert!(evaluation.debug_events.is_empty());
        assert_eq!(evaluation.values.len(), 6);
        let mut constants = 0;
        for item in &checked.module().items {
            let Item::VarDecl(declaration) = item else {
                continue;
            };
            if declaration.name.span.file != FileId::new(0) {
                continue;
            }
            let expected = match declaration.name.name.as_str() {
                "baked_optional" => jett_comptime::Value::Int64(7),
                "baked_string" => jett_comptime::Value::String("after".into()),
                "baked_integer" => jett_comptime::Value::Int64(23),
                name => panic!("unexpected Source constant: {name}"),
            };
            let jett_parser::ast::Expr::Comptime(_, span) = &declaration.value else {
                panic!("original required initializer");
            };
            assert_eq!(
                evaluation
                    .values
                    .get(*span, &jett_comptime::ComptimeContext::default()),
                Some(&expected),
            );
            constants += 1;
        }
        assert_eq!(constants, 3);
        evaluation.values.checked_required_values(&checked).unwrap();
        let values = Arc::new(evaluation.values);
        let mut hir = jett_hir::lower_checked_resource_program_with_test_bodies(&checked).unwrap();
        jett_hir::bake_required_values(
            &mut hir,
            &values,
            types,
            &checked.checked().reflection_metadata,
            &checked.checked().method_value_definitions,
        )
        .unwrap();
        let verify =
            jett_hir::append_checked_native_verify_suite(&mut hir, &checked, FileId::new(0))
                .unwrap()
                .unwrap();
        let cases = jett_comptime::verify::collect_checked_property_cases_with_checked_values(
            checked.clone(),
            values.clone(),
        )
        .unwrap();
        assert!(values.same_checked_cache(cases.required_values()));
        let property = jett_hir::append_checked_native_property_suite(
            &mut hir,
            &checked,
            FileId::new(0),
            &cases,
        )
        .unwrap()
        .unwrap();
        assert_eq!(property.trials().len(), 100);
        jett_hir::complete_value_conversions(&mut hir, types).unwrap();
        jett_hir::validate(&hir).unwrap();
        let program = jett_mir::lower(&hir, types).unwrap();
        // The exact MIR companion proof distinguishes primitive bits from an
        // ordinary owning handle. Emission must honor that bound, rather than
        // allocate a fake owner for the integer failure payload.
        let ownership = jett_mir::validate_resource_ownership(&program, types).unwrap();
        for (name, expected_type, owns_payload) in [
            ("integer_marker", TypeInterner::INT64, false),
            ("string_marker", TypeInterner::STRING, true),
        ] {
            let function = program
                .functions
                .iter()
                .find(|function| {
                    function.identity.declaration.namespace == "app"
                        && function.identity.declaration.name == name
                })
                .unwrap();
            let companion =
                jett_mir::ResourceCompanionPlan::analyze(&ownership, function.id).unwrap();
            let targets = function
                .blocks
                .iter()
                .flat_map(|block| &block.statements)
                .filter_map(|statement| match statement.kind {
                    StatementKind::SumTake {
                        target,
                        success: false,
                        ..
                    } => Some(target),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(targets.len(), 1, "one genuine {name} failure extraction");
            let target = targets[0];
            assert_eq!(function.local(target).unwrap().ty, expected_type);
            assert_eq!(
                companion
                    .storage()
                    .owned_locals
                    .contains(&(target.index() as usize)),
                owns_payload,
                "{name} payload storage must follow its type"
            );
            if owns_payload {
                assert!(companion.storage().temporary_slots > 0);
            } else {
                assert_eq!(companion.storage().temporary_slots, 0);
            }
        }
        for selected in [verify, property.entry()] {
            let artifact = super::super::super::emit_for_triple(
                &program,
                types,
                HOST,
                Some(selected),
                CodegenOptions { optimize: release },
            )
            .unwrap();
            let (_, imports) = symbols(&artifact);
            assert!(
                imports
                    .iter()
                    .any(|name| name == "jett_rt_v1_resource_replace")
            );
        }
    }
}
