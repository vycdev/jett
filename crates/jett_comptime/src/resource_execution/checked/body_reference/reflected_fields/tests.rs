//! Exact original-loop and ordinal-specific checked-body proofs.
//! Deliberate proof corruption remains confined to this private test module.

use super::*;
use crate::{Interpreter, Value};
use jett_common::FileId;
use jett_parser::ast::{Block, ComptimeTypeBindStmt, ForStmt, FunctionDef, Item, Stmt};
use jett_types::{ReflectionMetadata, TypeInterner};
use std::sync::Arc;

// The first two fields intentionally share both TypeId and reflection. The
// third has the same canonical TypeId but retains an explicit source alias.
// Both loops in `two_loops` use the same owner and variable spelling, so only
// original Source occurrence identity can separate their prepared proofs.
const SOURCE: &str = r#"namespace app
type Count = int64
struct IterationShape:
    first: int64
    second: int64
    named: Count
enum NoFields:
    missing
export function reflected() returns nothing:
    for field in type.fields[IterationShape]():
        comptime type Element = field.type_info:
            string label = type.name[Element]()
    return nothing
export function other() returns nothing:
    for field in type.fields[IterationShape]():
        comptime type Element = field.type_info:
            string label = type.name[Element]()
    return nothing
export function two_loops() returns nothing:
    for field in type.fields[IterationShape]():
        comptime type Element = field.type_info:
            string label = type.name[Element]()
    for field in type.fields[IterationShape]():
        comptime type Element = field.type_info:
            string label = type.name[Element]()
    return nothing
export function ordinary_loop() returns nothing:
    for value in list(1):
        int64 marker = value
    return nothing
export function nested_scope() returns nothing:
    for field in type.fields[IterationShape]():
        comptime type Outer = type.info[int64]():
            comptime type Element = field.type_info:
                string label = type.name[Element]()
    return nothing
export function reflected_labels() returns string:
    mutable string labels = ""
    for field in type.fields[IterationShape]():
        comptime type Element = field.type_info:
            string label = type.name[Element]()
            labels = "{labels}{label}|"
    return labels
export function two_loop_labels() returns string:
    mutable string labels = ""
    for field in type.fields[IterationShape]():
        comptime type Element = field.type_info:
            string label = type.name[Element]()
            labels = "{labels}{label}|"
    for field in type.fields[IterationShape]():
        comptime type Element = field.type_info:
            string label = type.name[Element]()
            labels = "{labels}{label}|"
    return labels
function helper_labels() returns string:
    mutable string labels = ""
    for field in type.fields[IterationShape]():
        comptime type Element = field.type_info:
            string label = type.name[Element]()
            labels = "{labels}{label}|"
    return labels
export function helper_inside_loop_labels() returns string:
    mutable string labels = ""
    for field in type.fields[IterationShape]():
        comptime type Element = field.type_info:
            string before = type.name[Element]()
            string inside = helper_labels()
            string after = type.name[Element]()
            labels = "{labels}{before}>{inside}>{after}|"
    return labels
export function empty_fields() returns int64:
    mutable int64 total = 0
    for field in type.fields[int64]():
        total = total + 1
    for field in type.fields[NoFields]():
        total = total + 1
    for field in type.fields[resource_probe.TestHandle]():
        total = total + 1
    return total
"#;

fn function<'a>(program: &'a CheckedResourceProgram, name: &str) -> &'a FunctionDef {
    program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function)
                if function.name.name == name && function.name.span.file == FileId::new(0) =>
            {
                Some(function)
            }
            _ => None,
        })
        .expect("one original project function")
}

fn loop_at(function: &FunctionDef, index: usize) -> &ForStmt {
    function
        .body
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            Stmt::For(statement) => Some(statement),
            _ => None,
        })
        .nth(index)
        .expect("one original top-level loop at the requested position")
}

fn field_scope(block: &Block) -> &ComptimeTypeBindStmt {
    block
        .stmts
        .iter()
        .find_map(|statement| match statement {
            Stmt::ComptimeTypeBind(binding) => Some(binding),
            _ => None,
        })
        .expect("original field.type_info binding")
}

fn install_entry(
    checked: &mut CheckedExecution,
    program: &CheckedResourceProgram,
    name: &str,
) -> CheckedBodyReference {
    let source = function(program, name);
    let definition = checked.declaration_definition(source.name.span).unwrap();
    let reference = checked
        .prepare_function_body(&checked.entry(definition).unwrap())
        .unwrap();
    checked.install_function_body(&reference).unwrap();
    reference
}

fn execution(
    release: bool,
    name: &str,
) -> (
    Arc<CheckedResourceProgram>,
    CheckedExecution,
    CheckedBodyReference,
) {
    let program = crate::resource_execution::tests::program(SOURCE, release);
    let mut checked =
        CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
    let reference = install_entry(&mut checked, &program, name);
    (program, checked, reference)
}

fn current_key(checked: &CheckedExecution) -> CheckedAttemptKey {
    checked.attempt_key(&checked.cursor()).unwrap()
}

// Deliberate proof forgery is confined to the child test module. No public
// constructor or production Clone requirement is introduced by this helper.
fn copy_loop(source: &PreparedReflectedFieldLoop) -> PreparedReflectedFieldLoop {
    PreparedReflectedFieldLoop {
        parent: source.parent.clone(),
        key: source.key.clone(),
        loop_span: source.loop_span,
        variable: source.variable,
        owner: source.owner,
        fields: source.fields.clone(),
    }
}

fn assert_scope_refused_without_mutation(
    checked: &CheckedExecution,
    binding: &ComptimeTypeBindStmt,
    iteration: &PreparedReflectedFieldIteration,
    reason: &str,
) {
    let before = current_key(checked);
    assert!(
        checked
            .prepare_reflected_field_scope(binding, iteration)
            .is_err(),
        "{reason}"
    );
    assert_eq!(current_key(checked), before, "{reason}: cursor changed");
}

#[test]
fn reflected_field_scope_keeps_equal_types_and_source_aliases_in_distinct_iterations() {
    for release in [false, true] {
        let (program, mut checked, parent) = execution(release, "reflected");
        let original_loop = loop_at(function(&program, "reflected"), 0);
        let binding = field_scope(&original_loop.body);
        let before = current_key(&checked);
        let prepared = checked
            .prepare_reflected_field_loop(original_loop)
            .unwrap()
            .expect("trusted direct type.fields loop");
        assert_eq!(current_key(&checked), before);
        assert_eq!(prepared.fields.len(), 3);
        assert_eq!(prepared.fields[0].0, TypeInterner::INT64);
        assert_eq!(prepared.fields[0].0, prepared.fields[1].0);
        assert_eq!(prepared.fields[0].0, prepared.fields[2].0);
        assert_eq!(
            prepared.fields[0].1.type_info,
            prepared.fields[1].1.type_info
        );
        assert_eq!(prepared.fields[2].1.type_info.kind, "alias");
        assert_eq!(prepared.fields[2].1.type_info.type_name, "app.Count");
        assert_ne!(
            prepared.fields[0].1.type_info,
            prepared.fields[2].1.type_info
        );

        for index in 0..prepared.fields.len() {
            let iteration = prepared.iteration(index).unwrap();
            let scope = checked
                .prepare_reflected_field_scope(binding, &iteration)
                .unwrap();
            assert_eq!(current_key(&checked), before);
            assert!(std::ptr::eq(scope.body().unwrap(), &binding.body));
            assert_eq!(scope.bound_name(), "int64");
            assert_eq!(scope.reflection(), &prepared.fields[index].1.type_info);
            let selected = scope.reference.cursor.scopes.last().unwrap();
            assert_eq!(
                selected.selection,
                CheckedComptimeTypeSelection::ReflectedIteration(index)
            );
            assert_eq!(selected.bound_type, prepared.fields[index].0);

            checked.install_direct_scope(&scope).unwrap();
            assert_ne!(current_key(&checked), before);
            assert_scope_refused_without_mutation(
                &checked,
                binding,
                &iteration,
                "parent-loop proof cannot enter from its own selected child body",
            );
            checked.install_function_body(&parent).unwrap();
            assert_eq!(current_key(&checked), before);
        }
        assert!(prepared.iteration(prepared.fields.len()).is_err());
        assert_eq!(current_key(&checked), before);
    }
}

#[test]
fn reflected_field_loop_and_scope_require_original_nodes_within_the_current_body() {
    for release in [false, true] {
        let (program, checked, _) = execution(release, "reflected");
        let original_loop = loop_at(function(&program, "reflected"), 0);
        let binding = field_scope(&original_loop.body);
        let before = current_key(&checked);
        let cloned_loop = original_loop.clone();
        assert!(checked.prepare_reflected_field_loop(&cloned_loop).is_err());
        assert_eq!(current_key(&checked), before);

        let prepared = checked
            .prepare_reflected_field_loop(original_loop)
            .unwrap()
            .unwrap();
        let iteration = prepared.iteration(0).unwrap();
        let cloned_binding = binding.clone();
        assert_scope_refused_without_mutation(
            &checked,
            &cloned_binding,
            &iteration,
            "a clone preserves spans but has no original binding identity",
        );

        let other_loop = loop_at(function(&program, "other"), 0);
        assert!(checked.prepare_reflected_field_loop(other_loop).is_err());
        assert_scope_refused_without_mutation(
            &checked,
            field_scope(&other_loop.body),
            &iteration,
            "a different function's original binding is not the selected loop body",
        );
        assert_eq!(current_key(&checked), before);
    }
}

#[test]
fn reflected_field_iteration_cannot_cross_programs_or_changed_parent_frames() {
    for release in [false, true] {
        let (program, mut checked, _) = execution(release, "reflected");
        let original_loop = loop_at(function(&program, "reflected"), 0);
        let binding = field_scope(&original_loop.body);
        let prepared = checked
            .prepare_reflected_field_loop(original_loop)
            .unwrap()
            .unwrap();
        let iteration = prepared.iteration(0).unwrap();

        let (foreign_program, foreign, _) = execution(release, "reflected");
        let foreign_loop = loop_at(function(&foreign_program, "reflected"), 0);
        assert_scope_refused_without_mutation(
            &foreign,
            field_scope(&foreign_loop.body),
            &iteration,
            "identical Source in another compiler session has no shared authority",
        );

        install_entry(&mut checked, &program, "other");
        assert_scope_refused_without_mutation(
            &checked,
            binding,
            &iteration,
            "a valid proof cannot be replayed after the active function changes",
        );
    }
}

#[test]
fn reflected_field_iteration_cannot_cross_equal_looking_original_loops() {
    for release in [false, true] {
        let (program, checked, _) = execution(release, "two_loops");
        let function = function(&program, "two_loops");
        let first = loop_at(function, 0);
        let second = loop_at(function, 1);
        let first_proof = checked
            .prepare_reflected_field_loop(first)
            .unwrap()
            .unwrap();
        let second_proof = checked
            .prepare_reflected_field_loop(second)
            .unwrap()
            .unwrap();
        assert_eq!(first_proof.owner, second_proof.owner);
        assert_eq!(first_proof.fields, second_proof.fields);
        assert_ne!(first_proof.loop_span, second_proof.loop_span);
        assert_ne!(first_proof.variable, second_proof.variable);
        assert_scope_refused_without_mutation(
            &checked,
            field_scope(&second.body),
            &first_proof.iteration(0).unwrap(),
            "same owner, variable spelling, index and type do not identify a Source loop",
        );
        assert_scope_refused_without_mutation(
            &checked,
            field_scope(&first.body),
            &second_proof.iteration(0).unwrap(),
            "the converse loop replay must also fail",
        );
    }
}

#[test]
fn reflected_field_scope_revalidates_private_iteration_and_loop_proof_contents() {
    for release in [false, true] {
        let (program, checked, _) = execution(release, "reflected");
        let original_loop = loop_at(function(&program, "reflected"), 0);
        let binding = field_scope(&original_loop.body);
        let prepared = checked
            .prepare_reflected_field_loop(original_loop)
            .unwrap()
            .unwrap();
        let (_, foreign, foreign_parent) = execution(release, "reflected");
        let bad_index = PreparedReflectedFieldIteration {
            owner: prepared.clone(),
            index: prepared.fields.len(),
        };
        assert_scope_refused_without_mutation(
            &checked,
            binding,
            &bad_index,
            "out-of-range index must fail even if iteration() was bypassed",
        );

        for corruption in 0..8 {
            let mut forged = copy_loop(&prepared);
            match corruption {
                0 => forged.variable = DefId::new(u32::MAX),
                1 => forged.owner = TypeInterner::STRING,
                2 => forged.fields[0].0 = TypeInterner::STRING,
                3 => forged.fields[0].1.type_info.kind = "struct".to_string(),
                4 => forged.fields[0].1.name = "different_field".to_string(),
                5 => forged.loop_span = loop_at(function(&program, "other"), 0).span,
                6 => forged.key = current_key(&foreign),
                7 => forged.parent = foreign_parent.clone(),
                _ => unreachable!(),
            }
            let iteration = PreparedReflectedFieldIteration {
                owner: Arc::new(forged),
                index: 0,
            };
            assert_scope_refused_without_mutation(
                &checked,
                binding,
                &iteration,
                &format!("private proof corruption {corruption}"),
            );
        }
    }
}

#[test]
fn ordinary_source_loop_returns_none_without_minting_a_reflected_proof() {
    for release in [false, true] {
        let (program, checked, _) = execution(release, "ordinary_loop");
        let original_loop = loop_at(function(&program, "ordinary_loop"), 0);
        let before = current_key(&checked);
        assert!(
            checked
                .prepare_reflected_field_loop(original_loop)
                .unwrap()
                .is_none()
        );
        assert_eq!(current_key(&checked), before);
    }
}

#[test]
fn reflected_field_scope_accepts_a_direct_lexical_descendant_of_its_original_loop() {
    for release in [false, true] {
        let (program, mut checked, parent) = execution(release, "nested_scope");
        let original_loop = loop_at(function(&program, "nested_scope"), 0);
        let outer_binding = field_scope(&original_loop.body);
        let element_binding = field_scope(&outer_binding.body);
        let prepared_loop = checked
            .prepare_reflected_field_loop(original_loop)
            .unwrap()
            .unwrap();
        let parent_key = current_key(&checked);
        for index in 0..prepared_loop.fields.len() {
            let iteration = prepared_loop.iteration(index).unwrap();
            let outer = checked.prepare_direct_scope(outer_binding).unwrap();
            checked.install_direct_scope(&outer).unwrap();
            let outer_key = current_key(&checked);
            assert_ne!(outer_key, parent_key);
            let element = checked
                .prepare_reflected_field_scope(element_binding, &iteration)
                .unwrap();
            assert_eq!(current_key(&checked), outer_key);
            assert!(std::ptr::eq(element.body().unwrap(), &element_binding.body));
            assert_eq!(
                element.reflection(),
                &prepared_loop.fields[index].1.type_info
            );
            assert_eq!(element.reference.cursor.scopes.len(), 2);
            assert_eq!(
                element.reference.cursor.scopes[1].selection,
                CheckedComptimeTypeSelection::ReflectedIteration(index)
            );
            assert_eq!(
                element.reference.cursor.scopes[1].bound_type,
                prepared_loop.fields[index].0
            );
            checked.install_direct_scope(&element).unwrap();
            assert_ne!(current_key(&checked), outer_key);
            checked.install_function_body(&parent).unwrap();
            assert_eq!(current_key(&checked), parent_key);
        }
    }
}

fn assert_runtime_has_no_resource_effects(interpreter: &mut Interpreter) {
    // No script is installed by these pure Source tests. Type-level metadata
    // does not create a Network grant or permit a runtime Resource operation.
    assert_eq!(
        interpreter.resource_test_observations(),
        Err(ResourceExecutionError::ProviderDisabled.to_string())
    );
    assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
    assert!(interpreter.take_debug_events().is_empty());
}

#[test]
fn reflected_field_runtime_uses_retained_metadata_and_retires_each_ordinal_entry_scope() {
    for release in [false, true] {
        let program = crate::resource_execution::tests::program(SOURCE, release);
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        // An ambient table cannot supply the retained owner's field metadata.
        interpreter.set_reflection_metadata(Arc::new(ReflectionMetadata::new()));
        let baseline = interpreter.resource_test_entry_context().unwrap();
        assert_runtime_has_no_resource_effects(&mut interpreter);
        for (name, expected) in [
            ("reflected", Value::Nothing),
            ("two_loops", Value::Nothing),
            (
                "reflected_labels",
                Value::String("int64|int64|app.Count|".to_string()),
            ),
            (
                "two_loop_labels",
                Value::String("int64|int64|app.Count|int64|int64|app.Count|".to_string()),
            ),
            (
                "reflected_labels",
                Value::String("int64|int64|app.Count|".to_string()),
            ),
        ] {
            let definition = checked
                .declaration_definition(function(&program, name).name.span)
                .unwrap();
            assert_eq!(
                interpreter
                    .call_checked_program_entry(definition, Vec::new())
                    .unwrap(),
                expected,
                "{name} release={release}"
            );
            // The entry observation includes checked reflected selector depth.
            // Equality to the fresh-entry baseline proves it returns to zero.
            assert_eq!(interpreter.resource_test_entry_context().unwrap(), baseline);
            assert_runtime_has_no_resource_effects(&mut interpreter);
        }
    }
}

#[test]
fn reflected_field_callable_frames_select_their_own_loops_and_restore_caller_aliases() {
    const EXPECTED: &str = concat!(
        "int64>int64|int64|app.Count|>int64|",
        "int64>int64|int64|app.Count|>int64|",
        "app.Count>int64|int64|app.Count|>app.Count|",
    );
    for release in [false, true] {
        let program = crate::resource_execution::tests::program(SOURCE, release);
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            ExecutionPurpose::ReferenceRuntime,
        )
        .unwrap();
        interpreter.set_reflection_metadata(Arc::new(ReflectionMetadata::new()));
        let baseline = interpreter.resource_test_entry_context().unwrap();
        for (name, expected) in [
            ("helper_inside_loop_labels", EXPECTED),
            ("helper_inside_loop_labels", EXPECTED),
            ("reflected_labels", "int64|int64|app.Count|"),
        ] {
            let definition = checked
                .declaration_definition(function(&program, name).name.span)
                .unwrap();
            assert_eq!(
                interpreter
                    .call_checked_program_entry(definition, Vec::new())
                    .unwrap(),
                Value::String(expected.to_string()),
                "{name} release={release}"
            );
            // The helper uses the same field/Element spellings as its caller.
            // Its three labels prove its own ordinals were selected; the outer
            // labels before and after the call prove caller alias restoration.
            assert_eq!(interpreter.resource_test_entry_context().unwrap(), baseline);
            assert_runtime_has_no_resource_effects(&mut interpreter);
        }
    }
}

#[test]
fn primitive_enum_and_opaque_resource_field_metadata_are_empty_without_a_provider() {
    for release in [false, true] {
        let program = crate::resource_execution::tests::program(SOURCE, release);
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let definition = checked
            .declaration_definition(function(&program, "empty_fields").name.span)
            .unwrap();
        let mut interpreter =
            Interpreter::from_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        interpreter.set_reflection_metadata(Arc::new(ReflectionMetadata::new()));
        let baseline = interpreter.resource_test_entry_context().unwrap();
        for _ in 0..2 {
            assert_eq!(
                interpreter
                    .call_checked_program_entry(definition, Vec::new())
                    .unwrap(),
                Value::Int64(0),
                "release={release}"
            );
            assert_eq!(interpreter.resource_test_entry_context().unwrap(), baseline);
            assert_runtime_has_no_resource_effects(&mut interpreter);
        }
    }
}

#[test]
fn closed_pure_reflected_labels_are_captured_by_actual_required_evaluation() {
    // Preserve every byte of SOURCE and append one required occurrence after
    // its pure callee. This is the real required worker, not a runtime entry.
    let source = format!(
        "{SOURCE}{}",
        r#"
export function required_labels() returns string:
    return comptime reflected_labels()
"#
    );
    for release in [false, true] {
        let program = crate::resource_execution::tests::program(&source, release);
        let types = Arc::new(crate::checked_types::CheckedExpressionTypes {
            resource_program: Some(program.clone()),
            expressions: program
                .checked()
                .type_map
                .iter()
                .map(|(span, ty)| (*span, program.checked().interner.type_name(*ty)))
                .collect(),
            ..Default::default()
        });
        let required = crate::evaluate_explicit_comptime_expressions_capture(
            program.module(),
            Arc::new(ReflectionMetadata::new()),
            types,
            Arc::new(std::collections::HashMap::new()),
        );
        assert!(
            required.diagnostics.is_empty(),
            "release={release}: {:?}",
            required.diagnostics
        );
        assert!(required.debug_events.is_empty());
        let expected = Value::String("int64|int64|app.Count|".to_string());
        assert_eq!(required.values.len(), 1);
        assert_eq!(
            required.values.values().cloned().collect::<Vec<_>>(),
            vec![expected.clone()]
        );
        assert!(required.values.checked_values_are_mirrored());

        // The worker receives no runtime provider, capability argument or
        // live Resource value. Its capture API exposes values and debug events,
        // not provider observations; do not invent an internal state assertion.
        assert!(
            required
                .values
                .checked_resource_hook_values(&program)
                .unwrap()
                .is_empty()
        );
        let proofs = required.values.checked_required_values(&program).unwrap();
        let [proof] = proofs.as_slice() else {
            panic!("exactly one authenticated required occurrence");
        };
        let original = function(&program, "required_labels");
        let [Stmt::Return(statement)] = original.body.stmts.as_slice() else {
            panic!("one original return containing the explicit comptime call");
        };
        let expression = statement.value.as_ref().unwrap();
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ExplicitComptime).unwrap();
        assert_eq!(proof.value(), &expected);
        assert!(proof.belongs_to(&program));
        assert!(proof.is_explicit_comptime());
        assert!(proof.matches_original_comptime(expression));
        assert_eq!(
            proof.owner(),
            crate::CheckedRequiredOwner::Function {
                definition: checked.declaration_definition(original.name.span).unwrap(),
                declaration: original.name.span,
            }
        );
        assert!(proof.checked_hook(&program).unwrap().is_none());
    }
}

// Original checked Sources exercise signals from selected field bodies.

mod selected_field_control_flow {
    use super::*;

    const CASES: &[(&str, &str, i64)] = &[
        (
            "return_from_second_selected_field",
            include_str!("control_flow/01_reflected_return.jett"),
            12,
        ),
        (
            "break_from_second_selected_field",
            include_str!("control_flow/02_reflected_break.jett"),
            18_927,
        ),
        (
            "continue_from_second_selected_field",
            include_str!("control_flow/03_reflected_continue.jett"),
            18_923_897,
        ),
        (
            "default_then_resume_selected_alias_body",
            include_str!("control_flow/04_reflected_default.jett"),
            1_193,
        ),
    ];

    fn run_original_source(name: &str, source: &str, expected: i64, release: bool) {
        // This helper retains actual checked SourceOrigin and Resource kernel
        // declarations through the existing synthetic Support/program path.
        let program = crate::resource_execution::tests::program(source, release);
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ReferenceRuntime).unwrap();
        let definition = checked
            .declaration_definition(function(&program, "scenario").name.span)
            .unwrap();
        let mut interpreter =
            Interpreter::from_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        // The checked intrinsic must use retained metadata, not an ambient map.
        interpreter.set_reflection_metadata(Arc::new(ReflectionMetadata::new()));
        let baseline = interpreter.resource_test_entry_context().unwrap();
        for attempt in 0..2 {
            assert_eq!(
                interpreter
                    .call_checked_program_entry(definition, Vec::new())
                    .unwrap(),
                Value::Int64(expected),
                "{name} release={release} attempt={attempt}"
            );
            // This observation includes private reflected selector depth.
            assert_eq!(
                interpreter.resource_test_entry_context().unwrap(),
                baseline,
                "{name} release={release} attempt={attempt}: entry state changed"
            );
            // No provider script or grant is installed for these pure controls.
            assert_eq!(
                interpreter.resource_test_observations(),
                Err(ResourceExecutionError::ProviderDisabled.to_string()),
                "{name} release={release} attempt={attempt}: provider became enabled"
            );
            assert_eq!(
                interpreter.resource_test_custody_counts().unwrap(),
                (0, 0),
                "{name} release={release} attempt={attempt}: custody remained"
            );
            assert!(
                interpreter.take_debug_events().is_empty(),
                "{name} release={release} attempt={attempt}: unexpected debug events"
            );
        }
    }

    #[test]
    fn reflected_field_return_break_continue_and_default_execute_original_sources() {
        for release in [false, true] {
            for &(name, source, expected) in CASES {
                run_original_source(name, source, expected, release);
            }
        }
    }
}
