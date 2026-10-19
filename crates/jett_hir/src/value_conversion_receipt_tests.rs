use super::*;
use crate::{
    ExpressionKind, Function, ReflectedCallablePlan, ResourceSourceArchive, StatementKind,
};
use jett_common::SourceOrigin;
use jett_typecheck::CheckedResourceProgram;
use std::collections::HashMap;
use std::sync::Arc;

// Both required occurrences come from the real closed reflected read. The
// ordinary temporary owner shares their immutable checked archive.
const SOURCE: &str = r#"namespace app
type Positive = int64 where value > 0
struct Holder:
    callback: function() returns int64
function five() returns int64:
    return 5
function positive() returns Positive:
    return 5
function stored() returns function() returns Positive:
    Holder holder = Holder(callback: five)
    mutable function() returns Positive selected = positive
    for field in type.fields[Holder]():
        selected = type.field_value[Holder, function() returns Positive](view holder, view field)
    return selected
struct Packet:
    data: list[int64]
    sibling: list[int64]
function make_packet() returns Packet:
    return Packet(data: list(1, 2), sibling: list(3, 4))
function inspect(view data: list[int64]) returns nothing:
    return nothing
function main() returns nothing:
    function() returns Positive first = comptime stored()
    function() returns Positive second = comptime stored()
    int64 first_value = coarsen first()
    int64 second_value = coarsen second()
    list[int64] borrowed = view make_packet().data
    inspect(view borrowed)
    list[int64] independent = clone borrowed
"#;

fn checked(source: &str, release: bool) -> Arc<CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions { release },
        )
        .unwrap(),
    )
}

fn staged_source(source: &str, release: bool) -> (Program, Arc<CheckedResourceProgram>) {
    let checked = checked(source, release);
    let evaluation = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        checked.checked().reflection_metadata.clone(),
        Arc::new(
            jett_comptime::checked_types::CheckedExpressionTypes::from_checked_program(&checked),
        ),
        Arc::new(checked.checked().breakpoint_exclusions.clone()),
    );
    assert!(
        evaluation.diagnostics.is_empty(),
        "{:?}",
        evaluation.diagnostics
    );
    let mut program = crate::lower_checked_ordinary_program(&checked).unwrap();
    crate::bake_required_values(
        &mut program,
        &evaluation.values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    (program, checked)
}

fn staged(release: bool) -> (Program, Arc<CheckedResourceProgram>) {
    let (program, checked) = staged_source(SOURCE, release);
    assert_eq!(staged_plans(&program).len(), 2);
    (program, checked)
}

fn main(program: &Program) -> &Function {
    program
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap()
}

fn main_mut(program: &mut Program) -> &mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap()
}

fn staged_plans(program: &Program) -> Vec<ReflectedCallablePlan> {
    let mut plans = Vec::new();
    for function in &program.functions {
        let mut body = function.body.clone();
        crate::resource_materialization::walk_block(&mut body, &mut |value| {
            if let ExpressionKind::InterfaceCoerce { adapters, .. } = &value.kind {
                plans.extend(adapters.iter().filter_map(|entry| {
                    entry
                        .reflected
                        .as_ref()
                        .filter(|plan| plan.is_materialized() && !plan.is_sealed())
                        .cloned()
                }));
            }
            Ok(())
        })
        .unwrap();
    }
    plans
}

fn materialized_plans(program: &Program) -> Vec<ReflectedCallablePlan> {
    program
        .functions
        .iter()
        .flat_map(crate::reflected_callable_plans)
        .filter(ReflectedCallablePlan::is_materialized)
        .collect()
}

fn required_statements(program: &Program) -> Vec<usize> {
    main(program)
        .body
        .statements
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            let StatementKind::Let { value, .. } = &statement.kind else {
                return None;
            };
            let ExpressionKind::InterfaceCoerce { adapters, .. } = &value.kind else {
                return None;
            };
            adapters
                .iter()
                .any(|entry| {
                    entry
                        .reflected
                        .as_ref()
                        .is_some_and(|plan| plan.is_materialized())
                })
                .then_some(index)
        })
        .collect()
}

fn replace_first_staged_plan(program: &mut Program, plan: ReflectedCallablePlan) {
    let mut replacement = Some(plan);
    for function in &mut program.functions {
        crate::resource_materialization::walk_block(&mut function.body, &mut |value| {
            if replacement.is_some() {
                if let ExpressionKind::InterfaceCoerce { adapters, .. } = &mut value.kind {
                    if let Some(entry) = adapters.iter_mut().find(|entry| {
                        entry
                            .reflected
                            .as_ref()
                            .is_some_and(|plan| plan.is_materialized())
                    }) {
                        entry.reflected = replacement.take();
                    }
                }
            }
            Ok(())
        })
        .unwrap();
    }
    assert!(replacement.is_none());
}

fn same_program(left: &Program, right: &Program) -> bool {
    metadata_equal(left, right)
        && crate::resource_materialization::functions_equal(&left.functions, &right.functions)
}

#[test]
fn canonical_conversion_composes_fresh_required_callables_with_a_genuine_temporary() {
    for release in [false, true] {
        let (before, checked) = staged(release);
        let original_source = before.resource_source.functions().to_vec();
        let types = &checked.checked().interner;
        let mut program = before.clone();
        crate::complete_value_conversions(&mut program, types).unwrap();
        program
            .resource_source
            .validate_current(&program, types)
            .unwrap();
        let plans = materialized_plans(&program);
        assert_eq!(plans.len(), 2);
        assert!(
            plans
                .iter()
                .all(|plan| plan.is_sealed() && plan.validate_original())
        );
        assert_ne!(
            plans[0], plans[1],
            "equal sampled values keep separate occurrences"
        );
        assert_eq!(
            crate::temporary_local_backings(&program, main(&program), types)
                .unwrap()
                .len(),
            1
        );
        assert!(crate::resource_materialization::functions_equal(
            program.resource_source.functions(),
            &original_source,
        ));
        let converted_functions = program.functions.clone();
        crate::complete_value_conversions(&mut program, types).unwrap();
        program
            .resource_source
            .validate_current(&program, types)
            .unwrap();
        assert!(crate::resource_materialization::functions_equal(
            &converted_functions,
            &program.functions,
        ));
        assert_eq!(plans, materialized_plans(&program));
    }
}

#[test]
fn canonical_conversion_receipt_replays_every_actual_bound_occurrence_without_reminting() {
    let (before, checked) = staged(false);
    let types = &checked.checked().interner;
    let mut actual = before.clone();
    let receipt = coerce(&mut actual, &before, types).unwrap();
    assert_eq!(actual.resource_source, before.resource_source);
    let actual_plans = materialized_plans(&actual);
    assert_eq!(actual_plans.len(), 2);
    assert_ne!(actual_plans[0], actual_plans[1]);
    assert!(actual_plans.iter().all(|plan| !plan.is_sealed()));
    assert!(actual.functions.len() > before.functions.len());
    for _ in 0..3 {
        let replay = receipt.replay(&before, types).unwrap();
        assert!(same_program(&actual, &replay));
        assert_eq!(actual_plans, materialized_plans(&replay));
    }
    crate::temporary_local_views::conversion(&actual, &before, types, &receipt)
        .unwrap()
        .unwrap();
}

#[test]
fn canonical_conversion_receipt_refuses_another_execution_of_the_same_checked_input() {
    let (before, checked) = staged(false);
    let types = &checked.checked().interner;
    let mut actual = before.clone();
    let receipt = coerce(&mut actual, &before, types).unwrap();
    let mut independent = before.clone();
    let independent_receipt = coerce(&mut independent, &before, types).unwrap();
    assert!(!crate::resource_materialization::functions_equal(
        &actual.functions,
        &independent.functions,
    ));
    assert!(
        crate::temporary_local_views::conversion(&independent, &before, types, &receipt).is_err()
    );
    crate::temporary_local_views::conversion(&independent, &before, types, &independent_receipt)
        .unwrap()
        .unwrap();
}

#[derive(Clone, Copy, Debug)]
enum InputEdit {
    ForeignPlan,
    ForeignArchive,
    BodySpan,
    LocalType,
    FunctionHeader,
    EqualityMethods,
    DroppedOccurrence,
    DuplicatedOccurrence,
    ReorderedOccurrences,
    DroppedFunction,
    DuplicatedFunction,
    ReorderedFunctions,
}

fn edit_input(program: &mut Program, foreign: &Program, edit: InputEdit) {
    match edit {
        InputEdit::ForeignPlan => {
            replace_first_staged_plan(program, staged_plans(foreign)[0].clone())
        }
        InputEdit::ForeignArchive => program.resource_source = foreign.resource_source.clone(),
        InputEdit::BodySpan => main_mut(program).body.span.end += 1,
        InputEdit::LocalType => {
            main_mut(program)
                .locals
                .iter_mut()
                .find(|local| local.name == "borrowed")
                .unwrap()
                .ty = TypeInterner::BOOL;
        }
        InputEdit::FunctionHeader => main_mut(program).capture_count += 1,
        InputEdit::EqualityMethods => {
            let owner = main(program).id;
            program.equality_methods.insert(TypeInterner::BOOL, owner);
        }
        InputEdit::DroppedOccurrence => {
            let indices = required_statements(program);
            main_mut(program).body.statements.remove(indices[0]);
        }
        InputEdit::DuplicatedOccurrence => {
            let indices = required_statements(program);
            let statement = main(program).body.statements[indices[0]].clone();
            main_mut(program)
                .body
                .statements
                .insert(indices[0], statement);
        }
        InputEdit::ReorderedOccurrences => {
            let indices = required_statements(program);
            main_mut(program)
                .body
                .statements
                .swap(indices[0], indices[1]);
        }
        InputEdit::DroppedFunction => {
            program.functions.pop();
        }
        InputEdit::DuplicatedFunction => {
            let function = program.functions.last().unwrap().clone();
            program.functions.push(function);
        }
        InputEdit::ReorderedFunctions => program.functions.swap(0, 1),
    }
}

#[test]
fn canonical_conversion_receipt_refuses_foreign_or_edited_complete_preimages() {
    let (before, checked) = staged(false);
    let (foreign, foreign_checked) = staged(false);
    let types = &checked.checked().interner;
    let mut actual = before.clone();
    let receipt = coerce(&mut actual, &before, types).unwrap();
    assert!(
        receipt
            .replay(&before, &foreign_checked.checked().interner)
            .is_err()
    );
    for edit in [
        InputEdit::ForeignPlan,
        InputEdit::ForeignArchive,
        InputEdit::BodySpan,
        InputEdit::LocalType,
        InputEdit::FunctionHeader,
        InputEdit::EqualityMethods,
        InputEdit::DroppedOccurrence,
        InputEdit::DuplicatedOccurrence,
        InputEdit::ReorderedOccurrences,
        InputEdit::DroppedFunction,
        InputEdit::DuplicatedFunction,
        InputEdit::ReorderedFunctions,
    ] {
        let mut altered = before.clone();
        edit_input(&mut altered, &foreign, edit);
        assert!(receipt.replay(&altered, types).is_err(), "{edit:?}");
        let altered_before = altered.clone();
        assert!(coerce(&mut altered, &before, types).is_err(), "{edit:?}");
        assert!(same_program(&altered, &altered_before), "{edit:?}");
    }
}

#[derive(Clone, Copy, Debug)]
enum OutputEdit {
    AdapterBody,
    PredicateBody,
    DroppedOccurrence,
    DuplicatedOccurrence,
    ReorderedOccurrences,
    DroppedAdapter,
    DuplicatedAdapter,
    ReorderedAdapters,
}

fn converted_required_statements(program: &Program, original_count: usize) -> Vec<usize> {
    main(program)
        .body
        .statements
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            let StatementKind::Let { value, .. } = &statement.kind else {
                return None;
            };
            let ExpressionKind::FunctionAdapter { function, .. } = &value.kind else {
                return None;
            };
            (function.index() as usize >= original_count).then_some(index)
        })
        .collect()
}

fn edit_output(program: &mut Program, original_count: usize, edit: OutputEdit) {
    match edit {
        OutputEdit::AdapterBody => {
            let mut edited = false;
            crate::resource_materialization::walk_block(
                &mut program.functions[original_count].body,
                &mut |expression| {
                    if let ExpressionKind::IndirectCall { callee, .. } = &mut expression.kind {
                        callee.kind = ExpressionKind::Nothing;
                        edited = true;
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert!(edited);
        }
        OutputEdit::PredicateBody => {
            let plans = materialized_plans(program);
            let predicate = plans[0].return_plan().predicates()[0].function;
            let mut edited = false;
            crate::resource_materialization::walk_block(
                &mut program.functions[predicate.index() as usize].body,
                &mut |expression| {
                    if let ExpressionKind::Int(value) = &mut expression.kind {
                        *value += 1;
                        edited = true;
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert!(edited);
        }
        OutputEdit::DroppedOccurrence => {
            let indices = converted_required_statements(program, original_count);
            main_mut(program).body.statements.remove(indices[0]);
        }
        OutputEdit::DuplicatedOccurrence => {
            let indices = converted_required_statements(program, original_count);
            let statement = main(program).body.statements[indices[0]].clone();
            main_mut(program)
                .body
                .statements
                .insert(indices[0], statement);
        }
        OutputEdit::ReorderedOccurrences => {
            let indices = converted_required_statements(program, original_count);
            main_mut(program)
                .body
                .statements
                .swap(indices[0], indices[1]);
        }
        OutputEdit::DroppedAdapter => {
            program.functions.pop();
        }
        OutputEdit::DuplicatedAdapter => {
            let function = program.functions.last().unwrap().clone();
            program.functions.push(function);
        }
        OutputEdit::ReorderedAdapters => program.functions.swap(original_count, original_count + 1),
    }
}

#[test]
fn canonical_conversion_receipt_does_not_authorize_edited_or_reordered_output_occurrences() {
    let (before, checked) = staged(false);
    let types = &checked.checked().interner;
    let mut actual = before.clone();
    let receipt = coerce(&mut actual, &before, types).unwrap();
    assert_eq!(actual.functions.len(), before.functions.len() + 2);
    assert_eq!(
        converted_required_statements(&actual, before.functions.len()).len(),
        2
    );
    for edit in [
        OutputEdit::AdapterBody,
        OutputEdit::PredicateBody,
        OutputEdit::DroppedOccurrence,
        OutputEdit::DuplicatedOccurrence,
        OutputEdit::ReorderedOccurrences,
        OutputEdit::DroppedAdapter,
        OutputEdit::DuplicatedAdapter,
        OutputEdit::ReorderedAdapters,
    ] {
        let mut altered = actual.clone();
        edit_output(&mut altered, before.functions.len(), edit);
        assert!(
            crate::temporary_local_views::conversion(&altered, &before, types, &receipt).is_err(),
            "{edit:?}"
        );
    }
}

#[test]
fn canonical_conversion_failure_keeps_the_complete_input_unpublished() {
    let (mut invalid, checked) = staged(false);
    let types = &checked.checked().interner;
    // A private negative control removes Source authority while retaining its
    // real staged occurrences. The early baked/Graph gates are empty; canonical
    // coercion changes the candidate before the temporary gate rejects it.
    invalid.resource_source = ResourceSourceArchive::empty();
    crate::reflected_callable::baked::validate_conversion_input(&invalid).unwrap();
    crate::actor_graph::validate_conversion_input(&invalid).unwrap();
    let before = invalid.clone();
    let mut preview = before.clone();
    let receipt = coerce(&mut preview, &before, types).unwrap();
    assert!(!same_program(&preview, &before));
    drop(receipt);
    assert!(crate::complete_value_conversions(&mut invalid, types).is_err());
    assert!(same_program(&invalid, &before));
    assert_eq!(staged_plans(&invalid).len(), 2);
}

#[test]
fn canonical_conversion_receipt_checks_hidden_arc_identities_even_without_original_data() {
    let (genuine, checked) = staged(false);
    let types = &checked.checked().interner;
    let mut before = genuine.clone();
    before.resource_source = ResourceSourceArchive::empty();
    let mut actual = before.clone();
    let receipt = coerce(&mut actual, &before, types).unwrap();
    let mut reflected = ResourceSourceArchive::empty();
    reflected.reflected_values = genuine.resource_source.reflected_values.clone();
    assert!(reflected.reflected_values.is_some());
    let mut execution = ResourceSourceArchive::empty();
    execution.install_materialized(crate::resource_source::Materialized {
        functions: genuine.functions.clone(),
        values: genuine.resource_source.required_materializations().to_vec(),
        required_only: genuine
            .resource_source
            .required_only_function_ids()
            .to_vec(),
    });
    let graph_source = "namespace app\nactor Counter:\n    mutable int64 count = 1\n    receive read responds int64:\n        respond count\nfunction make_counter() returns Counter:\n    return comptime spawn Counter()\nfunction main() returns nothing:\n    return nothing\n";
    let (graph_program, _) = staged_source(graph_source, false);
    let mut graph = ResourceSourceArchive::empty();
    graph.graph_values = graph_program.resource_source.graph_values.clone();
    assert!(graph.graph_values.is_some());
    assert_eq!(before.resource_source, before.resource_source.clone());
    for hidden in [reflected, execution, graph] {
        let mut altered = before.clone();
        altered.resource_source = hidden;
        assert_ne!(before.resource_source, altered.resource_source);
        assert!(receipt.replay(&altered, types).is_err());
    }
}

fn set_float(program: &mut Program, value: f64) {
    let mut count = 0;
    for function in &mut program.functions {
        crate::resource_materialization::walk_block(&mut function.body, &mut |expression| {
            if let ExpressionKind::Float(literal) = &mut expression.kind {
                *literal = value;
                count += 1;
            }
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(count, 1);
}

#[test]
fn canonical_conversion_receipt_preserves_nan_payloads_and_rejects_float_bit_edits() {
    let source = "namespace app\nfunction floating() returns float64:\n    return 0.0\nfunction main() returns nothing:\n    return nothing\n";
    let checked = checked(source, false);
    let types = &checked.checked().interner;
    let original = crate::lower_checked_ordinary_program(&checked).unwrap();
    // NaNs are synthetic private equality controls, not Source admission claims.
    for (accepted, changed) in [
        (0.0, -0.0),
        (
            f64::from_bits(0x7ff8_0000_0000_0001),
            f64::from_bits(0x7ff8_0000_0000_0002),
        ),
    ] {
        let mut before = original.clone();
        set_float(&mut before, accepted);
        let mut actual = before.clone();
        let receipt = coerce(&mut actual, &before, types).unwrap();
        let replay = receipt.replay(&before, types).unwrap();
        assert!(same_program(&actual, &replay));
        let mut altered = before.clone();
        set_float(&mut altered, changed);
        assert!(receipt.replay(&altered, types).is_err());
    }
}

#[test]
fn canonical_conversion_receipt_has_no_retained_arc_cycle() {
    let (before, checked) = staged(false);
    let weak = Arc::downgrade(&checked);
    let mut actual = before.clone();
    let receipt = coerce(&mut actual, &before, &checked.checked().interner).unwrap();
    let replay = receipt
        .replay(&before, &checked.checked().interner)
        .unwrap();
    drop(replay);
    drop(receipt);
    drop(actual);
    drop(before);
    drop(checked);
    assert!(weak.upgrade().is_none());
}
