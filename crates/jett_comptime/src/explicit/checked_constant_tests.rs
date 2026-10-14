//! Ordinary checked programs must share one original namespace initializer attempt.

use super::*;
use crate::ExecutionPurpose;
use jett_common::SourceOrigin;
use jett_typecheck::{CheckOptions, CheckedResourceProgram};

fn checked_program(source: &str, release: bool) -> Arc<CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            CheckOptions { release },
        )
        .expect("ordinary checked constant source"),
    )
}

fn capture(program: &Arc<CheckedResourceProgram>) -> ExplicitComptimeEvaluation {
    evaluate_explicit_comptime_expressions_capture(
        program.module(),
        program.checked().reflection_metadata.clone(),
        Arc::new(CheckedExpressionTypes {
            resource_program: Some(program.clone()),
            expressions: program
                .checked()
                .type_map
                .iter()
                .map(|(span, ty)| (*span, program.checked().interner.type_name(*ty)))
                .collect(),
            ..Default::default()
        }),
        Arc::new(HashMap::new()),
    )
}

fn declaration(program: &CheckedResourceProgram) -> &jett_parser::ast::VarDecl {
    program
        .module()
        .items
        .iter()
        .find_map(|item| match item {
            Item::VarDecl(declaration) => Some(declaration),
            _ => None,
        })
        .expect("namespace constant")
}

#[test]
fn ordinary_checked_constant_and_local_comptime_observations_run_once() {
    let program = checked_program(
        r#"namespace app
function observe(label: string) returns int64:
    print(view label)
    return 7
int64 cached = comptime observe("constant:")
function main() returns int64:
    int64 baked = comptime observe("expression:")
    return cached + baked
"#,
        false,
    );
    let captured = capture(&program);
    assert!(
        captured.diagnostics.is_empty(),
        "{:?}",
        captured.diagnostics
    );
    assert_eq!(
        captured.debug_events,
        vec![
            DebugEvent {
                kind: crate::DebugEventKind::Print,
                text: "constant:".into()
            },
            DebugEvent {
                kind: crate::DebugEventKind::Print,
                text: "expression:".into()
            },
        ]
    );
    assert_eq!(captured.values.len(), 3);
    assert_eq!(
        captured
            .values
            .checked_required_values(&program)
            .unwrap()
            .len(),
        2
    );
    assert!(captured.values.checked_values_are_mirrored());
    assert_eq!(
        captured
            .values
            .constant(declaration(&program).name.span)
            .unwrap()
            .payload(),
        &Value::Int64(7)
    );

    let mut interpreter = Interpreter::from_checked_resource_program(
        program.clone(),
        ExecutionPurpose::ReferenceRuntime,
    )
    .unwrap();
    interpreter.set_explicit_comptime_values(Arc::new(captured.values));
    interpreter.register_module(program.module());
    assert_eq!(
        interpreter.resource_test_custody_counts().unwrap_err(),
        "missing checked Resource transport"
    );
    assert_eq!(
        interpreter
            .call_function("app.main", vec![])
            .unwrap()
            .payload(),
        &Value::Int64(14)
    );
    assert!(interpreter.take_debug_events().is_empty());
}

#[test]
fn ordinary_failed_constant_preserves_one_attempt_before_later_comptime() {
    let program = checked_program(
        r#"namespace app
function reject() returns int64:
    println("constant failure")
    list[int64] rejected = range(0, 9223372036854775807)
    return 0
function accept() returns int64:
    print("later:")
    return 1
int64 failed = comptime reject()
function main() returns int64:
    return comptime accept()
"#,
        false,
    );
    let captured = capture(&program);
    assert_eq!(captured.diagnostics.len(), 1);
    assert!(
        captured.diagnostics[0]
            .message
            .contains("range: requested output is too large")
    );
    assert_eq!(
        captured.diagnostics[0].span,
        declaration(&program).value.span()
    );
    assert_eq!(
        captured.debug_events,
        vec![
            DebugEvent {
                kind: crate::DebugEventKind::Println,
                text: "constant failure\n".into()
            },
            DebugEvent {
                kind: crate::DebugEventKind::Print,
                text: "later:".into()
            },
        ]
    );
    assert!(
        captured
            .values
            .constant(declaration(&program).name.span)
            .is_none()
    );
    assert_eq!(captured.values.len(), 1);
    assert_eq!(
        captured
            .values
            .checked_required_values(&program)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn ordinary_constant_preparation_requires_original_program_node_and_purpose() {
    for release in [false, true] {
        let program = checked_program("namespace app\nint64 cached = comptime (3 + 4)\n", release);
        let original = declaration(&program);
        let mut interpreter = Interpreter::from_checked_resource_program(
            program.clone(),
            ExecutionPurpose::NamespaceConstant,
        )
        .unwrap();
        assert_eq!(
            interpreter.resource_test_custody_counts().unwrap_err(),
            "missing checked Resource transport"
        );
        let prepared = interpreter
            .checked_namespace_initializer(original)
            .unwrap()
            .unwrap();
        assert_eq!(prepared.purpose(), ExecutionPurpose::NamespaceConstant);
        let Expr::Comptime(inner, span) = &original.value else {
            panic!("explicit initializer")
        };
        interpreter.checked_execution_purpose(ExecutionPurpose::ExplicitComptime);
        assert!(interpreter.checked_namespace_initializer(original).is_err());
        let contexts = interpreter
            .checked_explicit_contexts(inner, *span, None, &[])
            .unwrap()
            .unwrap();
        assert_eq!(contexts.len(), 1);
        assert_eq!(prepared.key(), contexts[0].key());
        interpreter.checked_execution_purpose(ExecutionPurpose::NamespaceConstant);
        assert!(
            interpreter
                .checked_namespace_initializer(&original.clone())
                .is_err()
        );
        let foreign = checked_program("namespace app\nint64 cached = comptime (3 + 4)\n", release);
        assert!(
            interpreter
                .checked_namespace_initializer(declaration(&foreign))
                .is_err()
        );
        assert_eq!(
            interpreter.resource_test_custody_counts().unwrap_err(),
            "missing checked Resource transport"
        );
        assert!(interpreter.take_debug_events().is_empty());
    }
}
