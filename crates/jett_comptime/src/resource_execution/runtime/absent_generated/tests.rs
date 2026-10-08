//! Source17 executions remain distinct from the private malformed-value controls.
use super::*;
use crate::resource_execution::{ExecutionPurpose, ProviderEvent, ScriptOperation};
use crate::{Interpreter, Value};
use jett_common::FileId;
use jett_parser::ast::{ForStmt, FunctionDef, Item, MatchStmt, Pattern, Stmt};
use jett_types::ResourceHookKind;

#[path = "../../../../../jett_driver/tests/native_conformance/resource_absent_aggregate_baseline_inputs.rs"]
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

fn install_main(runtime: &mut ResourceTransport, program: &CheckedResourceProgram) {
    let definition = runtime
        .checked
        .declaration_definition(function(program, "main").name.span)
        .unwrap();
    let body = runtime
        .checked
        .prepare_function_body(&runtime.checked.entry(definition).unwrap())
        .unwrap();
    runtime.checked.install_function_body(&body).unwrap();
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

fn parent(index: usize) -> Value {
    match index {
        1 => Value::List(vec![Value::OptionalNone, Value::OptionalNone]),
        3 => Value::Map(vec![(Value::String("empty".into()), Value::OptionalNone)]),
        9 => Value::Enum {
            type_name: "app.TokenChoice".into(),
            variant: "vacant".into(),
            fields: vec![Value::Int64(17), Value::List(Vec::new())],
        },
        _ => unreachable!(),
    }
}

fn absent_runtime(index: usize, release: bool) -> (Arc<CheckedResourceProgram>, ResourceTransport) {
    let program = checked_program(index, release);
    let mut runtime =
        ResourceTransport::checked_only(program.clone(), ExecutionPurpose::ReferenceRuntime, 1)
            .unwrap();
    runtime.checked_source_active = true;
    install_main(&mut runtime, &program);
    (program, runtime)
}

fn clean(runtime: &ResourceTransport) {
    assert_eq!(runtime.live_owners(), 0);
    assert_eq!(runtime.registry_live_count(), 0);
    assert_eq!(
        runtime.provider_events(),
        Err(ResourceExecutionError::ProviderDisabled)
    );
}

fn execute_twice(input: &inputs::Input, release: bool) -> usize {
    let (checked, _) = inputs::prepare(input, release, &mut Vec::new()).unwrap();
    let types = Arc::new(crate::checked_types::CheckedExpressionTypes {
        resource_program: Some(checked.clone()),
        expressions: checked
            .checked()
            .type_map
            .iter()
            .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
            .collect(),
        ..Default::default()
    });
    let metadata = Arc::new(jett_types::ReflectionMetadata::new());
    let exclusions = Arc::new(HashMap::new());
    let required = crate::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        metadata.clone(),
        types.clone(),
        exclusions.clone(),
    );
    assert!(
        required.diagnostics.is_empty(),
        "{} release={release}: {:?}",
        input.name,
        required.diagnostics
    );
    assert!(required.debug_events.is_empty());
    required.values.checked_required_values(&checked).unwrap();
    assert!(required.values.checked_values_are_mirrored());
    assert!(
        required
            .values
            .values()
            .all(|value| !value.contains_live_resource_or_grant())
    );
    let mut interpreter = Interpreter::new();
    interpreter.set_reflection_metadata(metadata);
    interpreter.set_checked_expression_types(types);
    interpreter.set_breakpoint_exclusions(exclusions);
    interpreter.set_explicit_comptime_values(Arc::new(required.values));
    interpreter.register_module(checked.module());
    interpreter
        .install_checked_resource_program(checked.clone(), ExecutionPurpose::ReferenceRuntime)
        .unwrap();
    let main_span = function(&checked, "main").name.span;
    let definitions = checked
        .resolved()
        .scope_table
        .definitions
        .iter()
        .filter(|definition| {
            definition.kind == jett_resolve::DefKind::Function
                && definition.span == main_span
                && definition.namespace.as_deref() == Some("app")
        })
        .map(|definition| definition.id)
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        panic!("unique original main identity")
    };
    let grant = interpreter
        .install_resource_test_script(Vec::new())
        .unwrap();
    let baseline = interpreter.resource_test_entry_context().unwrap();
    for repeat in 0..2 {
        let actual = interpreter.call_checked_program_entry(*definition, vec![grant.clone()]);
        match (input.reference, actual) {
            (inputs::ReferenceExpectation::Nothing, Ok(Value::Nothing)) => {}
            (inputs::ReferenceExpectation::Error(expected), Err(actual)) if actual == expected => {}
            (_, actual) => panic!(
                "{} release={release} repeat={repeat}: {actual:?}",
                input.name
            ),
        }
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (Vec::new(), 0, 0)
        );
        assert_eq!(interpreter.resource_test_custody_counts().unwrap(), (0, 0));
        assert_eq!(interpreter.resource_test_entry_context().unwrap(), baseline);
        assert!(interpreter.take_debug_events().is_empty());
    }
    2
}

#[test]
fn absent_generated_unchanged_source17_wrappers_repeat_exact_outcomes_and_restore_entry() {
    assert_eq!(inputs::INPUTS.len(), 14);
    let mut entries = 0;
    for release in [false, true] {
        for input in inputs::INPUTS {
            entries += execute_twice(input, release);
        }
    }
    assert_eq!(entries, 56);
}

#[test]
fn absent_generated_view_continue_break_and_post_loop_reuse_restore_every_entry() {
    let input = inputs::Input {
        name: "source17_view_continue_break_reuse",
        source: include_str!("view_control.jett"),
        reference: inputs::ReferenceExpectation::Nothing,
    };
    let mut entries = 0;
    for release in [false, true] {
        entries += execute_twice(&input, release);
    }
    assert_eq!(entries, 4);
}

#[test]
fn absent_generated_parent_child_shapes_and_duplicate_publication_remain_checked() {
    for release in [false, true] {
        for index in [1, 3, 9] {
            let (program, mut runtime) = absent_runtime(index, release);
            let original = parent(index);
            if index == 1 {
                let types = &program.checked().interner;
                let refined = types
                    .type_ids()
                    .find(|ty| types.type_name(*ty) == "app.RefinedEnvelope")
                    .expect("original retained RefinedEnvelope");
                let jett_types::Type::Struct(owner) = types.resolve(refined) else {
                    panic!("original RefinedEnvelope struct");
                };
                let marker = types
                    .resolve_struct(*owner)
                    .fields
                    .iter()
                    .find(|(name, _)| name == "marker")
                    .unwrap()
                    .1;
                assert_eq!(types.type_name(marker), "app.PositiveMarker");
                let claimed = Value::Typed {
                    type_name: "app.PositiveMarker".into(),
                    value: Box::new(Value::Int64(17)),
                };
                for value in [Value::Int64(17), claimed.clone()] {
                    assert_eq!(
                        absent_shape(&value, marker, &runtime.checked),
                        Err(ResourceExecutionError::InvalidPayloadPath)
                    );
                }
                let unproved = Value::Struct {
                    type_name: "app.RefinedEnvelope".into(),
                    concrete_type: None,
                    fields: vec![
                        ("marker".into(), claimed),
                        ("tokens".into(), Value::List(Vec::new())),
                    ],
                };
                assert_eq!(
                    absent_shape(&unproved, refined, &runtime.checked),
                    Err(ResourceExecutionError::InvalidPayloadPath)
                );
                clean(&runtime);
            }
            let bad_parents = match index {
                1 => vec![
                    Value::List(vec![Value::Nothing]),
                    Value::List(vec![Value::Int64(1)]),
                    Value::List(vec![Value::OptionalSome(Box::new(Value::Nothing))]),
                    Value::Typed {
                        type_name: "list[optional[string]]".into(),
                        value: Box::new(original.clone()),
                    },
                ],
                3 => vec![
                    Value::Map(vec![(Value::Int64(1), Value::OptionalNone)]),
                    Value::Map(vec![(Value::String("empty".into()), Value::Nothing)]),
                    Value::List(Vec::new()),
                ],
                9 => vec![
                    Value::Enum {
                        type_name: "app.TokenChoice".into(),
                        variant: "empty".into(),
                        fields: Vec::new(),
                    },
                    Value::Enum {
                        type_name: "foreign.TokenChoice".into(),
                        variant: "vacant".into(),
                        fields: vec![Value::Int64(17), Value::List(Vec::new())],
                    },
                    Value::Enum {
                        type_name: "app.TokenChoice".into(),
                        variant: "missing".into(),
                        fields: Vec::new(),
                    },
                    Value::Enum {
                        type_name: "app.TokenChoice".into(),
                        variant: "vacant".into(),
                        fields: vec![Value::List(Vec::new()), Value::Int64(17)],
                    },
                    Value::Enum {
                        type_name: "app.TokenChoice".into(),
                        variant: "vacant".into(),
                        fields: vec![Value::Int64(17)],
                    },
                ],
                _ => unreachable!(),
            };
            for wrong in bad_parents {
                let result = if index == 9 {
                    runtime.prepare_absent_match(match_statement(&program), 0, &wrong)
                } else {
                    runtime.prepare_absent_for(for_statement(&program), &wrong)
                };
                assert!(result.is_err(), "index={index}, malformed parent={wrong:?}");
                clean(&runtime);
            }
            if index == 9 {
                assert!(
                    runtime
                        .prepare_absent_match(match_statement(&program), 1, &original)
                        .is_err(),
                    "vacant belongs to the earlier original variant arm, not Other"
                );
                clean(&runtime);
            }
            let proof = if index == 9 {
                runtime
                    .prepare_absent_match(match_statement(&program), 0, &original)
                    .unwrap()
                    .unwrap()
            } else {
                runtime
                    .prepare_absent_for(for_statement(&program), &original)
                    .unwrap()
                    .unwrap()
            };
            let (binder, ordinal, valid) = match index {
                1 => (&for_statement(&program).variable, 0, Value::OptionalNone),
                3 => (
                    for_statement(&program).value_variable.as_ref().unwrap(),
                    1,
                    Value::OptionalNone,
                ),
                9 => {
                    let Pattern::Variant(_, names) = &match_statement(&program).arms[0].pattern
                    else {
                        unreachable!()
                    };
                    (&names[1], 1, Value::List(Vec::new()))
                }
                _ => unreachable!(),
            };
            let fact = proof.validate_binding(binder, ordinal).unwrap();
            runtime.push_scope();
            if index != 1 {
                let (scalar_binder, scalar, wrong_scalar) = if index == 3 {
                    (
                        &for_statement(&program).variable,
                        Value::String("empty".into()),
                        Value::Int64(17),
                    )
                } else {
                    let Pattern::Variant(_, names) = &match_statement(&program).arms[0].pattern
                    else {
                        unreachable!()
                    };
                    (&names[0], Value::Int64(17), Value::String("17".into()))
                };
                let scalar_fact = proof.validate_binding(scalar_binder, 0).unwrap();
                assert!(
                    runtime
                        .validate_absent_binding(&proof, scalar_binder, 0, &wrong_scalar)
                        .is_err()
                );
                assert!(
                    runtime
                        .publish_absent_binding(&proof, scalar_binder, 0, wrong_scalar)
                        .is_err()
                );
                runtime
                    .publish_absent_binding(&proof, scalar_binder, 0, scalar)
                    .unwrap();
                assert!(
                    !runtime.has_binding(scalar_fact.definition),
                    "ordinary scalar child must not acquire a Resource slot"
                );
            }
            assert!(
                runtime
                    .publish_absent_binding(&proof, &binder.clone(), ordinal, valid.clone())
                    .is_err()
            );
            assert!(
                runtime
                    .publish_absent_binding(&proof, binder, usize::MAX, valid.clone())
                    .is_err()
            );
            assert!(
                runtime
                    .publish_absent_binding(&proof, binder, ordinal, Value::Nothing)
                    .is_err()
            );
            assert!(!runtime.has_binding(fact.definition));
            runtime
                .publish_absent_binding(&proof, binder, ordinal, valid.clone())
                .unwrap();
            assert!(runtime.has_binding(fact.definition));
            assert!(
                runtime
                    .publish_absent_binding(&proof, binder, ordinal, valid.clone())
                    .is_err()
            );
            let borrowed = runtime.binding(fact.definition, true).unwrap();
            assert_eq!(borrowed.value, valid);
            assert!(borrowed.custody.is_empty());
            assert_eq!(
                runtime.binding(fact.definition, false).unwrap().value,
                valid
            );
            assert!(runtime.binding(fact.definition, false).is_err());
            runtime.pop_scope();
            runtime.check_cleanup().unwrap();
            assert!(!runtime.has_binding(fact.definition));
            runtime.checked_source_active = false;
            let inactive = if index == 9 {
                runtime.prepare_absent_match(match_statement(&program), 0, &original)
            } else {
                runtime.prepare_absent_for(for_statement(&program), &original)
            };
            assert!(matches!(inactive, Ok(None)));
            assert!(
                runtime
                    .publish_absent_binding(&proof, binder, ordinal, valid)
                    .is_err()
            );
            clean(&runtime);
        }
    }
}

#[test]
fn absent_generated_binding_does_not_require_unrelated_live_owners_to_be_zero() {
    for release in [false, true] {
        let program = checked_program(1, release);
        let mut runtime =
            ResourceTransport::checked_only(program.clone(), ExecutionPurpose::ReferenceRuntime, 1)
                .unwrap();
        runtime.checked_source_active = true;
        let grant = runtime
            .install_script(vec![ScriptOperation::Construct {
                label: 1717,
                outcome: Ok(()),
            }])
            .unwrap();
        let definition = program
            .checked()
            .resource_hooks
            .values()
            .find(|hook| hook.kind == ResourceHookKind::Construct)
            .unwrap()
            .definition;
        let span = *program
            .checked()
            .call_ownership
            .iter()
            .find(|(_, packet)| packet.target == CheckedInvocationTarget::Resolved(definition))
            .unwrap()
            .0;
        let invocation = runtime.checked.invocation(span).unwrap();
        let descriptor = runtime.checked.descriptor(definition).unwrap();
        let owner = runtime
            .invoke_hook(
                &invocation,
                &descriptor,
                vec![
                    EvaluatedValue::ordinary(Value::GrantedNetwork(grant.clone())),
                    EvaluatedValue::ordinary(Value::Int64(1717)),
                ],
            )
            .unwrap();
        let Value::ResultOk(token) = &owner.value else {
            panic!("original factory success")
        };
        assert!(!owner.custody.is_empty());
        assert_eq!(
            (runtime.live_owners(), runtime.registry_live_count()),
            (1, 1)
        );
        install_main(&mut runtime, &program);
        let statement = for_statement(&program);
        let original = parent(1);
        let proof = runtime
            .prepare_absent_for(statement, &original)
            .unwrap()
            .unwrap();
        let fact = proof.validate_binding(&statement.variable, 0).unwrap();
        runtime.push_scope();
        for live in [
            Value::OptionalSome(token.clone()),
            Value::GrantedNetwork(grant),
        ] {
            assert!(
                runtime
                    .publish_absent_binding(&proof, &statement.variable, 0, live.clone())
                    .is_err()
            );
            assert!(
                runtime
                    .prepare_absent_for(statement, &Value::List(vec![live]))
                    .is_err()
            );
            assert!(!runtime.has_binding(fact.definition));
            assert_eq!(
                (runtime.live_owners(), runtime.registry_live_count()),
                (1, 1)
            );
        }
        runtime
            .publish_absent_binding(&proof, &statement.variable, 0, Value::OptionalNone)
            .unwrap();
        assert_eq!(
            runtime.binding(fact.definition, true).unwrap().value,
            Value::OptionalNone
        );
        assert_eq!(
            (runtime.live_owners(), runtime.registry_live_count()),
            (1, 1)
        );
        runtime.pop_scope();
        runtime.check_cleanup().unwrap();
        assert_eq!(
            (runtime.live_owners(), runtime.registry_live_count()),
            (1, 1)
        );
        assert_eq!(
            runtime.provider_events().unwrap(),
            vec![ProviderEvent::Constructed(1717)]
        );
        runtime.pop_scope();
        runtime.check_cleanup().unwrap();
        assert_eq!(
            (runtime.live_owners(), runtime.registry_live_count()),
            (0, 0)
        );
        assert_eq!(
            runtime.provider_events().unwrap(),
            vec![
                ProviderEvent::Constructed(1717),
                ProviderEvent::Finalized(1717)
            ]
        );
    }
}
