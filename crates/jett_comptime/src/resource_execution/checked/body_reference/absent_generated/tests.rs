//! Private rejection controls for exact generated binding proofs.
use super::*;
use jett_common::FileId;
use jett_parser::ast::{FunctionDef, Item};
use jett_types::TypeInterner;

#[path = "../../../../../../jett_driver/tests/native_conformance/resource_absent_aggregate_baseline_inputs.rs"]
mod inputs;

fn checked_program(index: usize, release: bool) -> Arc<CheckedResourceProgram> {
    inputs::prepare(&inputs::INPUTS[index], release, &mut Vec::new())
        .unwrap()
        .0
}

fn function<'a>(program: &'a CheckedResourceProgram, name: &str) -> &'a FunctionDef {
    program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function)
                if function.name.span.file == FileId::new(0) && function.name.name == name =>
            {
                Some(function)
            }
            _ => None,
        })
        .unwrap()
}

fn install(checked: &mut CheckedExecution, program: &CheckedResourceProgram, name: &str) {
    let function = function(program, name);
    let definition = checked.declaration_definition(function.name.span).unwrap();
    let body = checked
        .prepare_function_body(&checked.entry(definition).unwrap())
        .unwrap();
    checked.install_function_body(&body).unwrap();
}

fn for_statement(program: &CheckedResourceProgram) -> &ForStmt {
    function(program, "main")
        .body
        .stmts
        .iter()
        .find_map(|statement| match statement {
            Stmt::For(statement) => Some(statement),
            _ => None,
        })
        .unwrap()
}

fn match_statement(program: &CheckedResourceProgram) -> &MatchStmt {
    function(program, "main")
        .body
        .stmts
        .iter()
        .find_map(|statement| match statement {
            Stmt::Match(statement) => Some(statement),
            _ => None,
        })
        .unwrap()
}

fn current_main(checked: &CheckedExecution, program: &CheckedResourceProgram) {
    let current = CheckedBodyReference {
        cursor: checked.cursor(),
    };
    assert!(std::ptr::eq(
        current.function().unwrap(),
        function(program, "main")
    ));
    assert!(checked.body.scopes.is_empty());
}

#[test]
fn absent_generated_proofs_select_original_list_map_and_variant_binders() {
    for release in [false, true] {
        for (index, count, parent_name, names, types) in [
            (
                1,
                1,
                "list[optional[resource_probe.TestHandle]]",
                vec!["candidate"],
                vec!["optional[resource_probe.TestHandle]"],
            ),
            (
                3,
                2,
                "map[string, optional[resource_probe.TestHandle]]",
                vec!["key", "candidate"],
                vec!["string", "optional[resource_probe.TestHandle]"],
            ),
            (
                9,
                2,
                "app.TokenChoice",
                vec!["marker", "tokens"],
                vec!["int64", "list[resource_probe.TestHandle]"],
            ),
        ] {
            let program = checked_program(index, release);
            let mut checked =
                CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
            install(&mut checked, &program, "main");
            let proof = if index == 9 {
                checked
                    .prepare_absent_match(match_statement(&program), 0)
                    .unwrap()
                    .unwrap()
            } else {
                checked
                    .prepare_absent_for(for_statement(&program))
                    .unwrap()
                    .unwrap()
            };
            assert_eq!(proof.binding_count(), count);
            assert_eq!(
                program.checked().interner.type_name(proof.parent_type()),
                parent_name
            );
            for (ordinal, (binder, fact)) in proof.bindings.iter().enumerate() {
                assert_eq!(binder.name, names[ordinal]);
                assert_eq!(
                    program.checked().interner.type_name(fact.ty),
                    types[ordinal]
                );
                assert_eq!(fact.mode, CheckedBindingMode::Owned);
                assert!(!fact.mutable);
                assert_eq!(fact.declaration_span, binder.span);
                assert_eq!(proof.validate_binding(binder, ordinal).unwrap(), *fact);
            }
            checked.revalidate_absent_bindings(&proof).unwrap();
            current_main(&checked, &program);
        }
    }
}

#[test]
fn absent_generated_proofs_reject_copied_controls_binders_and_wrong_ordinals() {
    for release in [false, true] {
        for index in [1, 3] {
            let program = checked_program(index, release);
            let mut checked =
                CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
            install(&mut checked, &program, "main");
            let statement = for_statement(&program);
            let copied = statement.clone();
            assert!(checked.prepare_absent_for(&copied).is_err());
            let proof = checked.prepare_absent_for(statement).unwrap().unwrap();
            let binder = statement.variable.clone();
            assert!(proof.validate_binding(&binder, 0).is_err());
            assert!(
                proof
                    .validate_binding(&statement.variable, proof.binding_count())
                    .is_err()
            );
            if let Some(value) = &statement.value_variable {
                assert!(proof.validate_binding(value, 0).is_err());
                assert!(proof.validate_binding(&statement.variable, 1).is_err());
            }
            current_main(&checked, &program);
        }
        let program = checked_program(9, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        install(&mut checked, &program, "main");
        let statement = match_statement(&program);
        let copied = statement.clone();
        assert!(checked.prepare_absent_match(&copied, 0).is_err());
        assert!(checked.prepare_absent_match(statement, usize::MAX).is_err());
        let proof = checked.prepare_absent_match(statement, 0).unwrap().unwrap();
        let Pattern::Variant(_, binders) = &statement.arms[0].pattern else {
            panic!("original vacant pattern")
        };
        assert!(proof.validate_binding(&binders[0].clone(), 0).is_err());
        assert!(proof.validate_binding(&binders[1], 0).is_err());
        assert!(proof.validate_binding(&binders[0], 1).is_err());
        current_main(&checked, &program);
    }
}

#[test]
fn absent_generated_proofs_reject_foreign_program_changed_body_and_corrupt_facts() {
    for release in [false, true] {
        let program = checked_program(1, release);
        let foreign_program = checked_program(1, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let mut foreign =
            CheckedExecution::new(foreign_program.clone(), ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        install(&mut checked, &program, "main");
        install(&mut foreign, &foreign_program, "main");
        let statement = for_statement(&program);
        let proof = checked.prepare_absent_for(statement).unwrap().unwrap();
        assert!(foreign.revalidate_absent_bindings(&proof).is_err());
        assert!(foreign.prepare_absent_for(statement).is_err());
        install(&mut checked, &program, "absent_tokens");
        assert!(checked.revalidate_absent_bindings(&proof).is_err());
        install(&mut checked, &program, "main");
        let copied = statement.clone();
        for corruption in 0..8 {
            let mut bad = checked.prepare_absent_for(statement).unwrap().unwrap();
            match corruption {
                0 => bad.parent = TypeInterner::STRING,
                1 => bad.bindings[0].1.ty = TypeInterner::STRING,
                2 => bad.bindings[0].1.definition = DefId::new(u32::MAX),
                3 => bad.bindings[0].1.declaration_span = Span::new(FileId::new(0), 0, 0),
                4 => {
                    bad.bindings[0].1.mode = CheckedBindingMode::View {
                        source: CheckedViewSource::Other,
                    }
                }
                5 => bad.bindings[0].1.mutable = true,
                6 => bad.control = Control::For(&copied),
                7 => bad.key = foreign.attempt_key(&foreign.body).unwrap(),
                _ => unreachable!(),
            }
            assert!(
                checked.revalidate_absent_bindings(&bad).is_err(),
                "corruption {corruption}"
            );
            current_main(&checked, &program);
        }
        checked.revalidate_absent_bindings(&proof).unwrap();
    }
}

#[test]
fn absent_generated_match_proof_rejects_corrupt_arm_name_and_binding_order() {
    for release in [false, true] {
        let program = checked_program(9, release);
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        install(&mut checked, &program, "main");
        let statement = match_statement(&program);
        for corruption in 0..4 {
            let mut bad = checked.prepare_absent_match(statement, 0).unwrap().unwrap();
            match corruption {
                0 => bad.control = Control::Match(statement, 0, "occupied".into()),
                1 => bad.control = Control::Match(statement, 1, "vacant".into()),
                2 => bad.bindings.swap(0, 1),
                3 => bad.payload_count = Some(usize::MAX),
                _ => unreachable!(),
            }
            assert!(
                checked.revalidate_absent_bindings(&bad).is_err(),
                "corruption {corruption}"
            );
            current_main(&checked, &program);
        }
    }
}

#[test]
fn absent_generated_view_loops_keep_copyable_keys_owned_and_exact_occurrences_distinct() {
    let input = inputs::Input {
        name: "source17_view_continue_break_reuse",
        source: include_str!("../../../runtime/absent_generated/view_control.jett"),
        reference: inputs::ReferenceExpectation::Nothing,
    };
    for release in [false, true] {
        let program = inputs::prepare(&input, release, &mut Vec::new()).unwrap().0;
        let mut checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        for name in ["source17_absent_view_list", "source17_absent_view_map"] {
            install(&mut checked, &program, name);
            let loops = function(&program, name)
                .body
                .stmts
                .iter()
                .filter_map(|statement| match statement {
                    Stmt::For(statement) => Some(statement),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(loops.len(), 2);
            assert!(loops.iter().all(|statement| statement.view));
            let proof = checked.prepare_absent_for(loops[0]).unwrap().unwrap();
            let second = checked.prepare_absent_for(loops[1]).unwrap().unwrap();
            assert!(proof.validate_binding(&loops[1].variable, 0).is_err());
            let first_mode = proof.validate_binding(&loops[0].variable, 0).unwrap().mode;
            if name == "source17_absent_view_map" {
                assert_eq!(first_mode, CheckedBindingMode::Owned);
                let value = loops[0].value_variable.as_ref().unwrap();
                assert_eq!(
                    proof.validate_binding(value, 1).unwrap().mode,
                    CheckedBindingMode::View {
                        source: CheckedViewSource::Other
                    }
                );
                assert!(
                    proof
                        .validate_binding(loops[1].value_variable.as_ref().unwrap(), 1)
                        .is_err()
                );
            } else {
                assert_eq!(
                    first_mode,
                    CheckedBindingMode::View {
                        source: CheckedViewSource::Other
                    }
                );
            }
            assert_eq!(proof.parent_type(), second.parent_type());
            let mut replaced = checked.prepare_absent_for(loops[0]).unwrap().unwrap();
            replaced.control = Control::For(loops[1]);
            assert!(checked.revalidate_absent_bindings(&replaced).is_err());
            checked.revalidate_absent_bindings(&proof).unwrap();
            checked.revalidate_absent_bindings(&second).unwrap();
            assert!(checked.body.scopes.is_empty());
        }
    }
}
