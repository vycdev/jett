use std::collections::HashMap;
use std::sync::Arc;

use jett_common::FileId;
use jett_parser::ast::Item;
use jett_resolve::{DefId, DefKind};
use jett_types::{Type, TypeId};

use super::*;
use crate::resource_execution::tests::program;
use crate::{Interpreter, Value};

const SOURCE: &str = include_str!("../fixtures/17_absent_aggregate_shapes.jett");

fn source_entry(program: &CheckedResourceProgram, name: &str) -> DefId {
    let functions = program
        .module()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function)
                if function.name.span.file == FileId::new(0) && function.name.name == name =>
            {
                Some(function)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [function] = functions.as_slice() else {
        panic!("one primary ordinary source entry {name}");
    };
    let definitions = program
        .resolved()
        .scope_table
        .definitions
        .iter()
        .filter(|definition| {
            definition.kind == DefKind::Function
                && definition.span == function.name.span
                && definition.namespace.as_deref() == Some("app")
        })
        .map(|definition| definition.id)
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        panic!("one exact retained ordinary declaration {name}");
    };
    *definition
}

fn returned_type(program: &CheckedResourceProgram, name: &str) -> TypeId {
    let definition = source_entry(program, name);
    let signature = program.checked().definition_types[&definition];
    let Type::Function { return_type, .. } = program.checked().interner.resolve(signature) else {
        panic!("exact checked signature");
    };
    *return_type
}

fn absent_values() -> Vec<(&'static str, Value)> {
    vec![
        ("empty_tokens", Value::List(Vec::new())),
        (
            "absent_tokens",
            Value::List(vec![Value::OptionalNone, Value::OptionalNone]),
        ),
        ("empty_map", Value::Map(Vec::new())),
        (
            "absent_map",
            Value::Map(vec![(Value::String("empty".into()), Value::OptionalNone)]),
        ),
        (
            "empty_result_tokens",
            Value::ResultOk(Box::new(Value::List(Vec::new()))),
        ),
        (
            "failed_result_tokens",
            Value::ResultFail(Box::new(Value::String("empty".into()))),
        ),
        (
            "absent_envelope",
            Value::Struct {
                type_name: "app.TokenEnvelope".into(),
                concrete_type: None,
                fields: vec![
                    ("tokens".into(), Value::List(Vec::new())),
                    ("fallback".into(), Value::OptionalNone),
                    ("marker".into(), Value::Int64(17)),
                ],
            },
        ),
        (
            "absent_box",
            Value::Struct {
                type_name: "app.TokenBox".into(),
                concrete_type: Some("app.TokenBox[resource_probe.TestHandle]".into()),
                fields: vec![("items".into(), Value::List(Vec::new()))],
            },
        ),
        (
            "empty_choice",
            Value::Enum {
                type_name: "app.TokenChoice".into(),
                variant: "empty".into(),
                fields: Vec::new(),
            },
        ),
        (
            "absent_choice",
            Value::Enum {
                type_name: "app.TokenChoice".into(),
                variant: "vacant".into(),
                fields: vec![Value::Int64(17), Value::List(Vec::new())],
            },
        ),
        (
            "absent_state",
            Value::Machine {
                type_name: "app.TokenState".into(),
                state: "vacant".into(),
                fields: vec![Value::OptionalNone, Value::String("empty".into())],
            },
        ),
        (
            "absent_exact_state",
            Value::Machine {
                type_name: "app.TokenState".into(),
                state: "vacant".into(),
                fields: vec![Value::OptionalNone, Value::String("empty".into())],
            },
        ),
    ]
}

#[test]
fn resource_absent_aggregates_execute_exact_checked_source_without_provider_events() {
    for release in [false, true] {
        let checked = program(SOURCE, release);
        for (name, expected) in absent_values() {
            let target = source_entry(&checked, name);
            let mut interpreter = Interpreter::from_checked_resource_program(
                checked.clone(),
                ExecutionPurpose::ReferenceRuntime,
            )
            .unwrap();
            interpreter
                .install_resource_test_script(Vec::new())
                .unwrap();
            assert_eq!(
                interpreter
                    .call_checked_program_entry(target, Vec::new())
                    .unwrap(),
                expected,
                "{name}, release={release}"
            );
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (Vec::new(), 0, 0)
            );
            assert!(interpreter.take_debug_events().is_empty());
        }
    }
}

#[test]
fn resource_absent_aggregates_remain_pure_in_all_required_purposes_and_workers() {
    for release in [false, true] {
        let checked = program(SOURCE, release);
        for purpose in [
            ExecutionPurpose::NamespaceConstant,
            ExecutionPurpose::ExplicitComptime,
            ExecutionPurpose::Verify,
            ExecutionPurpose::Property,
        ] {
            for (name, expected) in absent_values() {
                let target = source_entry(&checked, name);
                let mut interpreter =
                    Interpreter::from_checked_resource_program(checked.clone(), purpose).unwrap();
                assert!(
                    interpreter
                        .install_resource_test_script(Vec::new())
                        .is_err()
                );
                assert_eq!(
                    interpreter
                        .call_checked_program_entry(target, Vec::new())
                        .unwrap(),
                    expected
                );
                assert!(interpreter.take_debug_events().is_empty());
            }
        }
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
        let explicit = crate::evaluate_explicit_comptime_expressions_capture(
            checked.module(),
            Arc::new(jett_types::ReflectionMetadata::new()),
            types.clone(),
            Arc::new(HashMap::new()),
        );
        assert!(
            explicit.diagnostics.is_empty(),
            "{:?}",
            explicit.diagnostics
        );
        assert!(explicit.debug_events.is_empty());
        let declaration = checked
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::VarDecl(declaration) if declaration.name.name == "absent_namespace_value" => {
                    Some(declaration.name.span)
                }
                _ => None,
            })
            .expect("one actual source namespace constant");
        assert_eq!(
            explicit.values.constant(declaration),
            Some(&Value::Int64(17))
        );
        let blocks = crate::run_verify_blocks_detailed_with_metadata_and_expression_types(
            checked.module(),
            None,
            Some(types),
            None,
        );
        assert_eq!(blocks.len(), 2);
        assert!(
            blocks
                .iter()
                .all(|block| block.passed && block.debug_events.is_empty()),
            "{blocks:?}"
        );
        assert_eq!(blocks.iter().filter(|block| block.is_property).count(), 1);
        assert_eq!(
            blocks
                .iter()
                .find(|block| block.is_property)
                .unwrap()
                .iterations,
            Some(100)
        );
    }
}

#[test]
fn resource_absent_aggregate_shape_checks_reject_wrong_elements_keys_and_sum_payloads() {
    for release in [false, true] {
        let checked = program(SOURCE, release);
        let runtime =
            ResourceTransport::checked_only(checked.clone(), ExecutionPurpose::ReferenceRuntime, 1)
                .unwrap();
        let bad = vec![
            ("empty_tokens", Value::String("not a list".into())),
            ("empty_tokens", Value::List(vec![Value::Nothing])),
            ("absent_tokens", Value::List(vec![Value::Int64(1)])),
            (
                "absent_tokens",
                Value::List(vec![Value::OptionalSome(Box::new(Value::Nothing))]),
            ),
            ("empty_map", Value::List(Vec::new())),
            (
                "absent_map",
                Value::Map(vec![(Value::Int64(1), Value::OptionalNone)]),
            ),
            (
                "absent_map",
                Value::Map(vec![(Value::String("empty".into()), Value::Nothing)]),
            ),
            (
                "empty_result_tokens",
                Value::ResultOk(Box::new(Value::String("not a list".into()))),
            ),
            (
                "failed_result_tokens",
                Value::ResultFail(Box::new(Value::Int64(1))),
            ),
            (
                "absent_tokens",
                Value::Typed {
                    type_name: "list[optional[string]]".into(),
                    value: Box::new(Value::List(Vec::new())),
                },
            ),
        ];
        for (name, value) in bad {
            assert_eq!(
                runtime.validate_value_type(
                    &EvaluatedValue::ordinary(value),
                    returned_type(&checked, name)
                ),
                Err(ResourceExecutionError::InvalidPayloadPath),
                "{name}"
            );
        }
        let signature =
            checked.checked().definition_types[&source_entry(&checked, "refined_shape_contract")];
        let Type::Function { params, .. } = checked.checked().interner.resolve(signature) else {
            panic!("exact refined shape contract");
        };
        let refined = params[0];
        for marker in [0, 17] {
            let unproved = Value::Struct {
                type_name: "app.RefinedEnvelope".into(),
                concrete_type: None,
                fields: vec![
                    (
                        "marker".into(),
                        Value::Typed {
                            type_name: "app.PositiveMarker".into(),
                            value: Box::new(Value::Int64(marker)),
                        },
                    ),
                    ("tokens".into(), Value::List(Vec::new())),
                ],
            };
            assert_eq!(
                runtime.validate_value_type(&EvaluatedValue::ordinary(unproved), refined),
                Err(ResourceExecutionError::InvalidPayloadPath)
            );
        }
        assert_eq!(runtime.live_owners(), 0);
        assert_eq!(runtime.registry_live_count(), 0);
    }
}

#[test]
fn resource_absent_nominal_shape_checks_reject_foreign_names_fields_variants_states_and_generics() {
    for release in [false, true] {
        let checked = program(SOURCE, release);
        let runtime =
            ResourceTransport::checked_only(checked.clone(), ExecutionPurpose::ReferenceRuntime, 1)
                .unwrap();
        for (name, value) in absent_values()
            .into_iter()
            .filter(|(name, _)| name.starts_with("absent_") || *name == "empty_choice")
        {
            let ty = returned_type(&checked, name);
            assert_eq!(
                runtime.validate_value_type(&EvaluatedValue::ordinary(value.clone()), ty),
                Ok(())
            );
            let mut corruptions = Vec::new();
            match &value {
                Value::Struct { .. } => {
                    let mut wrong = value.clone();
                    if let Value::Struct { type_name, .. } = &mut wrong {
                        *type_name = "foreign.SameShape".into();
                    }
                    corruptions.push(wrong);
                    let mut wrong = value.clone();
                    if let Value::Struct { fields, .. } = &mut wrong {
                        fields[0].0 = "foreign".into();
                    }
                    corruptions.push(wrong);
                    let mut wrong = value.clone();
                    if let Value::Struct { fields, .. } = &mut wrong {
                        fields.pop();
                    }
                    corruptions.push(wrong);
                    let mut wrong = value.clone();
                    if let Value::Struct { fields, .. } = &mut wrong {
                        fields.push(fields[0].clone());
                    }
                    corruptions.push(wrong);
                    let mut wrong = value.clone();
                    if let Value::Struct { concrete_type, .. } = &mut wrong {
                        *concrete_type = Some("app.TokenBox[string]".into());
                    }
                    corruptions.push(wrong);
                    if name == "absent_envelope" {
                        let mut wrong = value.clone();
                        if let Value::Struct { fields, .. } = &mut wrong {
                            fields.swap(0, 1);
                        }
                        corruptions.push(wrong);
                        let mut wrong = value.clone();
                        if let Value::Struct { fields, .. } = &mut wrong {
                            fields[2].1 = Value::String("17".into());
                        }
                        corruptions.push(wrong);
                    }
                }
                Value::Enum { .. } => {
                    let mut wrong = value.clone();
                    if let Value::Enum { type_name, .. } = &mut wrong {
                        *type_name = "foreign.TokenChoice".into();
                    }
                    corruptions.push(wrong);
                    let mut wrong = value.clone();
                    if let Value::Enum { variant, .. } = &mut wrong {
                        *variant = "missing".into();
                    }
                    corruptions.push(wrong);
                    let mut wrong = value.clone();
                    if let Value::Enum { fields, .. } = &mut wrong {
                        fields.push(Value::Nothing);
                    }
                    corruptions.push(wrong);
                    let mut wrong = value.clone();
                    if let Value::Enum {
                        variant, fields, ..
                    } = &mut wrong
                    {
                        *variant = "occupied".into();
                        *fields = vec![Value::Nothing];
                    }
                    corruptions.push(wrong);
                }
                Value::Machine { .. } => {
                    let mut wrong = value.clone();
                    if let Value::Machine { type_name, .. } = &mut wrong {
                        *type_name = "foreign.TokenState".into();
                    }
                    corruptions.push(wrong);
                    let mut wrong = value.clone();
                    if let Value::Machine { state, .. } = &mut wrong {
                        *state = "missing".into();
                    }
                    corruptions.push(wrong);
                    let mut wrong = value.clone();
                    if let Value::Machine { fields, .. } = &mut wrong {
                        fields.pop();
                    }
                    corruptions.push(wrong);
                    let mut wrong = value.clone();
                    if let Value::Machine { state, fields, .. } = &mut wrong {
                        *state = "occupied".into();
                        *fields = vec![Value::Nothing];
                    }
                    corruptions.push(wrong);
                    if name == "absent_exact_state" {
                        let mut wrong = value.clone();
                        if let Value::Machine { state, .. } = &mut wrong {
                            *state = "waiting".into();
                        }
                        corruptions.push(wrong);
                    }
                }
                _ => continue,
            }
            for wrong in corruptions {
                assert_eq!(
                    runtime.validate_value_type(&EvaluatedValue::ordinary(wrong), ty),
                    Err(ResourceExecutionError::InvalidPayloadPath),
                    "{name}"
                );
            }
        }
        assert_eq!(runtime.live_owners(), 0);
        assert_eq!(runtime.registry_live_count(), 0);
    }
}

#[test]
fn resource_absent_shape_validation_keeps_occupied_aggregate_custody_refused() {
    use crate::resource_execution::{ProviderEvent, ScriptOperation};
    use jett_types::ResourceHookKind;

    for release in [false, true] {
        let checked = program(SOURCE, release);
        let mut runtime =
            ResourceTransport::checked_only(checked.clone(), ExecutionPurpose::ReferenceRuntime, 1)
                .unwrap();
        runtime.checked_source_active = true;
        let grant = runtime
            .install_script(vec![ScriptOperation::Construct {
                label: 1701,
                outcome: Ok(()),
            }])
            .unwrap();
        let definition = checked
            .checked()
            .resource_hooks
            .values()
            .find(|hook| hook.kind == ResourceHookKind::Construct)
            .unwrap()
            .definition;
        let span = *checked
            .checked()
            .call_ownership
            .iter()
            .find(|(_, packet)| packet.target == CheckedInvocationTarget::Resolved(definition))
            .unwrap()
            .0;
        let invocation = runtime.checked.invocation(span).unwrap();
        let descriptor = runtime.checked.descriptor(definition).unwrap();
        let produced = runtime
            .invoke_hook(
                &invocation,
                &descriptor,
                vec![
                    EvaluatedValue::ordinary(Value::GrantedNetwork(grant.clone())),
                    EvaluatedValue::ordinary(Value::Int64(1701)),
                ],
            )
            .unwrap();
        let Value::ResultOk(token) = &produced.value else {
            panic!("occupied factory result");
        };
        // Physical test copies have no custody; no aggregate owner path is
        // authorized by the new absent-layout proof.
        let malformed = vec![
            ("empty_tokens", Value::List(vec![token.as_ref().clone()])),
            (
                "absent_map",
                Value::Map(vec![(
                    Value::String("occupied".into()),
                    Value::OptionalSome(token.clone()),
                )]),
            ),
            (
                "absent_envelope",
                Value::Struct {
                    type_name: "app.TokenEnvelope".into(),
                    concrete_type: None,
                    fields: vec![
                        ("tokens".into(), Value::List(vec![token.as_ref().clone()])),
                        ("fallback".into(), Value::OptionalNone),
                        ("marker".into(), Value::Int64(17)),
                    ],
                },
            ),
            (
                "empty_choice",
                Value::Enum {
                    type_name: "app.TokenChoice".into(),
                    variant: "occupied".into(),
                    fields: vec![token.as_ref().clone()],
                },
            ),
            (
                "absent_state",
                Value::Machine {
                    type_name: "app.TokenState".into(),
                    state: "occupied".into(),
                    fields: vec![token.as_ref().clone()],
                },
            ),
            (
                "empty_tokens",
                Value::List(vec![Value::GrantedNetwork(grant)]),
            ),
        ];
        for (name, value) in malformed {
            assert_eq!(
                runtime.validate_value_type(
                    &EvaluatedValue::ordinary(value),
                    returned_type(&checked, name)
                ),
                Err(ResourceExecutionError::InvalidPayloadPath),
                "{name}"
            );
        }
        assert_eq!(runtime.live_owners(), 1);
        runtime.pop_scope();
        runtime.check_cleanup().unwrap();
        assert_eq!(
            runtime.provider_events().unwrap(),
            vec![
                ProviderEvent::Constructed(1701),
                ProviderEvent::Finalized(1701)
            ]
        );
        assert_eq!(runtime.live_owners(), 0);
        assert_eq!(runtime.registry_live_count(), 0);
    }
}

#[path = "absence_tests/augmented_baseline.rs"]
mod augmented_baseline;
