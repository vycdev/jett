use super::super::tests::{checked_support, lowered};
use super::*;

const SUPPORT: &str =
    include_str!("../../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
const SOURCE: &str = include_str!("reflected_field_source.jett");

fn main(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "main"
        })
        .unwrap()
}
fn main_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "main"
        })
        .unwrap()
}

fn reseal_current_graph(function: &mut Function) {
    let copy = function.clone();
    let witness = function.resource_lowering.as_mut().unwrap();
    witness.blocks = copy.blocks.clone();
    witness.descriptors.seal_body(&copy);
    seal(witness);
}

#[test]
fn reflected_field_original_equal_types_keep_distinct_checked_ordinals_and_scopes() {
    let indexed = SOURCE.replace("            total = total + observed", "            if field.index == 0:\n                total = total + observed\n            else:\n                total = total + observed + 1").replace("total != 34", "total != 35");
    let named = SOURCE.replace("            total = total + observed", "            if field.name == \"first\":\n                total = total + observed\n            else:\n                total = total + observed + 1").replace("total != 34", "total != 35");
    for release in [false, true] {
        for source in [SOURCE, indexed.as_str(), named.as_str()] {
            let checked = checked_support(source, SUPPORT, release);
            let types = &checked.checked().interner;
            let mut program = lowered(&checked);
            let function = main(&program);
            let witness = function.resource_lowering.as_ref().unwrap();
            let [row] = witness.reflected_fields.as_slice() else {
                panic!("exact original field binding");
            };
            assert_eq!(row.original.iteration_index(0), Some(0));
            assert_eq!(row.original.iteration_index(1), Some(1));
            assert_eq!(
                row.original.arms()[0].bound_type,
                row.original.arms()[1].bound_type
            );
            assert_eq!(
                row.original.arms()[0].reflection_identity,
                row.original.arms()[1].reflection_identity
            );
            assert_ne!(row.paths[0], row.paths[1]);
            assert_ne!(row.targets[0], row.targets[1]);
            assert_eq!(witness.borrowed_sums.len(), 2);
            assert_ne!(
                witness.borrowed_sums[0].alias(),
                witness.borrowed_sums[1].alias()
            );
            assert_eq!(
                witness.borrowed_sums[0].backing(),
                witness.borrowed_sums[1].backing()
            );
            for dispatch in &row.dispatches {
                let TerminatorKind::ReflectedTypeDispatch { arms, .. } =
                    &function.blocks[dispatch.index() as usize].terminator.kind
                else {
                    panic!("selected one-arm dispatch");
                };
                assert_eq!(arms.len(), 1);
            }
            validate_resource_ownership(&program, types).unwrap();
            crate::prepare_native_sequences(&mut program, types);
            validate_resource_ownership(&program, types).unwrap();
            let once = program.clone();
            crate::prepare_native_sequences(&mut program, types);
            assert_eq!(program, once);
        }
    }
}

#[test]
fn reflected_field_reordered_equal_type_guards_and_redirected_targets_cannot_reseal() {
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let types = &checked.checked().interner;
        let original = lowered(&checked);
        for mutation in 0..3 {
            let mut edited = original.clone();
            let function = main_mut(&mut edited);
            let row = function
                .resource_lowering
                .as_ref()
                .unwrap()
                .reflected_fields[0]
                .clone();
            assert_eq!(
                row.original.arms()[0].bound_type,
                row.original.arms()[1].bound_type
            );
            match mutation {
                0 => {
                    for (index, guard) in row.guards.iter().enumerate() {
                        let TerminatorKind::Branch { condition, .. } =
                            &mut function.blocks[guard.index() as usize].terminator.kind
                        else {
                            unreachable!();
                        };
                        *condition =
                            ordinal_guard(&row.original, &row.type_info, 1 - index).unwrap();
                    }
                }
                1 => {
                    let TerminatorKind::Branch { then_block, .. } = &mut function.blocks
                        [row.guards[0].index() as usize]
                        .terminator
                        .kind
                    else {
                        unreachable!();
                    };
                    *then_block = row.dispatches[1];
                }
                2 => {
                    let TerminatorKind::ReflectedTypeDispatch { arms, .. } = &mut function.blocks
                        [row.dispatches[0].index() as usize]
                        .terminator
                        .kind
                    else {
                        unreachable!();
                    };
                    arms[0].target = row.targets[1];
                }
                _ => unreachable!(),
            }
            reseal_current_graph(function);
            let expected = if mutation == 2 {
                "exact one-arm body/reflection join"
            } else {
                "exact ordinal selecting guard"
            };
            let reflected_message =
                super::current(function.resource_lowering.as_ref().unwrap(), function).unwrap_err();
            assert!(reflected_message.contains(expected), "{reflected_message}");
            let message = validate_resource_ownership(&edited, types)
                .unwrap_err()
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                message.contains(expected)
                    || (mutation != 0
                        && message.contains(
                            "borrowed Resource Handle is disconnected from its exact selecting edge",
                        )),
                "{message}"
            );
        }
    }
}

#[test]
fn reflected_field_stale_scope_path_cannot_be_replaced_by_equal_type_body() {
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let types = &checked.checked().interner;
        let mut edited = lowered(&checked);
        let function = main_mut(&mut edited);
        let witness = function.resource_lowering.as_mut().unwrap();
        witness.reflected_fields[0].paths[1] = witness.reflected_fields[0].paths[0].clone();
        seal(witness);
        let message = validate_resource_ownership(&edited, types)
            .unwrap_err()
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            message.contains("reordered ordinals or changed original Scope paths"),
            "{message}"
        );
    }
}

#[test]
fn reflected_field_public_hir_reordering_has_no_original_source_authority() {
    for release in [false, true] {
        let checked = checked_support(SOURCE, SUPPORT, release);
        let types = &checked.checked().interner;
        let original = hir::lower_checked_resource_program(&checked).unwrap();
        for mutation in 0..3 {
            let mut edited = original.clone();
            let function = edited
                .functions
                .iter_mut()
                .find(|function| function.identity.declaration.name == "main")
                .unwrap();
            let statement = function
                .body
                .statements
                .iter_mut()
                .find(|statement| matches!(statement.kind, hir::StatementKind::For { .. }))
                .unwrap();
            let hir::StatementKind::For { iterable, body, .. } = &mut statement.kind else {
                unreachable!();
            };
            if mutation == 0 {
                let hir::StatementKind::ReflectedTypeDispatch { arms, .. } =
                    &mut body.statements[0].kind
                else {
                    unreachable!();
                };
                assert_eq!(arms[0].bound_type, arms[1].bound_type);
                arms.swap(0, 1);
            } else {
                let hir::ExpressionKind::ListConstruct { elements } = &mut iterable.kind else {
                    unreachable!();
                };
                if mutation == 1 {
                    elements.swap(0, 1);
                } else {
                    let hir::ExpressionKind::StructConstruct { fields, .. } = &mut elements[1].kind
                    else {
                        unreachable!();
                    };
                    let hir::ExpressionKind::StructConstruct { fields, .. } = &mut fields[9].kind
                    else {
                        unreachable!();
                    };
                    fields[0].kind = hir::ExpressionKind::String("foreign-int64".into());
                }
            }
            let message = crate::lower(&edited, types)
                .unwrap_err()
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                message.contains("original checked HIR archive"),
                "{message}"
            );
        }
    }
}

#[test]
fn reflected_field_plain_helper_gets_only_exact_archived_proof_without_runtime_scope() {
    let helper = "function field_ordinal_total() returns int64:\n    mutable int64 total = 0\n    for field in type.fields[IterationShape]():\n        comptime type Element = field.type_info:\n            Element observed = 17\n            if field.index == 0:\n                total = total + observed\n            else:\n                total = total + observed + 1\n    return total\n";
    let source = SOURCE.replace(
        "export function main",
        &format!("{helper}export function main"),
    );
    for release in [false, true] {
        let checked = checked_support(&source, SUPPORT, release);
        let types = &checked.checked().interner;
        let mut program = lowered(&checked);
        let function = program
            .functions
            .iter()
            .find(|function| function.identity.declaration.name == "field_ordinal_total")
            .unwrap();
        let witness = function.resource_lowering.as_ref().unwrap();
        assert_eq!(witness.reflected_fields.len(), 1);
        assert_eq!(witness.reflected_fields[0].original.arms().len(), 2);
        assert_eq!(
            witness.reflected_fields[0].original.arms()[0].bound_type,
            witness.reflected_fields[0].original.arms()[1].bound_type
        );
        assert!(!super::super::has_execution_records(function, types));
        let id = function.id;
        let plan = validate_resource_ownership(&program, types).unwrap();
        assert!(plan.function(id).is_none());
        crate::prepare_native_sequences(&mut program, types);
        validate_resource_ownership(&program, types).unwrap();

        let mut edited = program.clone();
        let function = edited
            .functions
            .iter_mut()
            .find(|function| function.id == id)
            .unwrap();
        let row = function
            .resource_lowering
            .as_ref()
            .unwrap()
            .reflected_fields[0]
            .clone();
        let TerminatorKind::Branch { condition, .. } = &mut function.blocks
            [row.guards[0].index() as usize]
            .terminator
            .kind
        else {
            unreachable!();
        };
        *condition = ordinal_guard(&row.original, &row.type_info, 1).unwrap();
        reseal_current_graph(function);
        let message = validate_resource_ownership(&edited, types)
            .unwrap_err()
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            message.contains("exact ordinal selecting guard"),
            "{message}"
        );
    }
}

#[test]
fn reflected_field_plain_helper_preserves_mixed_ordinary_sequence_preparation() {
    let head = "function field_ordinal_total() returns int64:\n    mutable int64 total = 0\n    for field in type.fields[IterationShape]():\n        comptime type Element = field.type_info:\n            Element observed = 17\n            total = total + observed\n";
    let ordinary = [
        "    for letter in \"ab\":\n        total = total + 1\n",
        "    for key, value in map(1: 2):\n        total = total + key + value\n",
        "    list[int64] values = list(1, 2)\n    for observed in view values:\n        total = total + observed\n",
        "    for impossible in list():\n        total = total + 1\n",
    ];
    for release in [false, true] {
        for body in ordinary {
            let helper = format!("{head}{body}    return total\n");
            let source = SOURCE.replace(
                "export function main",
                &format!("{helper}export function main"),
            );
            let checked = checked_support(&source, SUPPORT, release);
            let types = &checked.checked().interner;
            let mut program = lowered(&checked);
            let function = program
                .functions
                .iter()
                .find(|function| function.identity.declaration.name == "field_ordinal_total")
                .unwrap();
            assert_eq!(
                function
                    .resource_lowering
                    .as_ref()
                    .unwrap()
                    .reflected_fields
                    .len(),
                1
            );
            assert!(!super::super::has_execution_records(function, types));
            let id = function.id;
            crate::prepare_native_sequences(&mut program, types);
            validate_resource_ownership(&program, types).unwrap();
            let function = program
                .functions
                .iter()
                .find(|function| function.id == id)
                .unwrap();
            assert!(
                !function
                    .blocks
                    .iter()
                    .any(|block| matches!(block.terminator.kind, TerminatorKind::ForEach { .. })),
                "ordinary sequence preparation rolled back: {body}"
            );
            assert!(
                validate_resource_ownership(&program, types)
                    .unwrap()
                    .function(id)
                    .is_none()
            );
            let once = program.clone();
            crate::prepare_native_sequences(&mut program, types);
            assert_eq!(program, once);
        }
    }
}
