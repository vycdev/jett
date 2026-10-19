use super::*;
use crate::resource_execution::{ProviderEvent, ScriptOperation};
use jett_common::FileId;

const SINGLE: &str = include_str!("../fixtures/33_owned_struct_scope_drop_entry.jett");
const REVERSE: &str = include_str!("../fixtures/34_owned_struct_reverse_entry.jett");
const PREFIX: &str = include_str!("../fixtures/35_owned_struct_abandoned_prefix_entry.jett");
const PRIMITIVES: &str = include_str!("../fixtures/36_owned_struct_primitive_companions.jett");
const ALIASES: &str = include_str!("../fixtures/37_owned_struct_alias_companions.jett");
const PENDING: &str = include_str!("../fixtures/38_owned_struct_pending_int64_transport.jett");
const PENDING_INSPECTION: &str =
    include_str!("../fixtures/39_owned_struct_pending_int64_inspection.jett");
const PENDING_PRIMITIVES: &str =
    include_str!("../fixtures/40_owned_struct_pending_companions.jett");
const PENDING_PREFIX: &str = include_str!("../fixtures/41_owned_struct_pending_prior_failure.jett");
const PENDING_MATRIX: &str =
    include_str!("../fixtures/42_owned_struct_pending_primitive_matrix.jett");
const PENDING_UNIT_INSPECTION: &str =
    include_str!("../fixtures/43_owned_struct_pending_nothing_inspection.jett");
const PENDING_TEXT_INSPECTION: &str =
    include_str!("../fixtures/44_owned_struct_pending_string_inspection.jett");

fn entry(program: &jett_typecheck::CheckedResourceProgram) -> DefId {
    program
        .resolved()
        .scope_table
        .definitions
        .iter()
        .find(|info| {
            info.kind == jett_resolve::DefKind::Function
                && info.namespace.as_deref() == Some("app")
                && info.name == "app.main"
                && info.span.file == FileId::new(0)
        })
        .unwrap()
        .id
}

fn run(source: &str, script: Vec<ScriptOperation>, events: Vec<ProviderEvent>) {
    run_program(
        source,
        script,
        events,
        crate::resource_execution::tests::program,
    );
}

fn run_program(
    source: &str,
    script: Vec<ScriptOperation>,
    events: Vec<ProviderEvent>,
    prepare: fn(&str, bool) -> Arc<jett_typecheck::CheckedResourceProgram>,
) {
    for release in [false, true] {
        let program = prepare(source, release);
        let target = entry(&program);
        let mut interpreter =
            Interpreter::from_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let grant = interpreter
            .install_resource_test_script(script.clone())
            .unwrap();
        for invocation in 0..2 {
            assert_eq!(
                interpreter
                    .call_checked_program_entry(target, vec![grant.clone()])
                    .unwrap(),
                Value::Nothing,
                "release={release}, invocation={invocation}"
            );
            let expected = events
                .iter()
                .cloned()
                .cycle()
                .take(events.len() * (invocation + 1))
                .collect::<Vec<_>>();
            // The tuple's third field is registry_live_count, never remaining
            // provider script. Both custody counts are zero at this entry boundary.
            assert_eq!(
                interpreter.resource_test_observations().unwrap(),
                (expected, 0, 0)
            );
        }
    }
}

#[test]
fn occupied_struct_owned_formal_moves_into_checked_root_and_reenters_without_teardown() {
    run(
        SINGLE,
        vec![
            ScriptOperation::Construct {
                label: 1001,
                outcome: Ok(())
            };
            2
        ],
        vec![
            ProviderEvent::Constructed(1001),
            ProviderEvent::Finalized(1001),
        ],
    );
    run(
        SINGLE,
        vec![
            ScriptOperation::Construct {
                label: 1001,
                outcome: Err("denied".into())
            };
            2
        ],
        vec![ProviderEvent::ConstructionFailed(1001)],
    );
}

#[test]
fn occupied_struct_primitive_companions_keep_checked_widths_aliases_and_pending_layers() {
    for (source, label) in [
        (PRIMITIVES, 1101),
        (ALIASES, 1102),
        (PENDING, 1103),
        (PENDING_PRIMITIVES, 1104),
        (PENDING_MATRIX, 1106),
    ] {
        run_program(
            source,
            vec![
                ScriptOperation::Construct {
                    label,
                    outcome: Ok(())
                };
                2
            ],
            vec![
                ProviderEvent::Constructed(label),
                ProviderEvent::Finalized(label),
            ],
            crate::resource_execution::tests::companion_program,
        );
    }
}

#[test]
fn occupied_struct_pending_inspection_and_prior_failure_retire_before_reentry() {
    for release in [false, true] {
        for (source, label, expected) in [
            (
                PENDING_INSPECTION,
                1103,
                "unsupported binary operation: pending(17) Eq 17",
            ),
            (
                PENDING_PREFIX,
                1105,
                "list.__remove_at: index -1 out of bounds",
            ),
            (
                PENDING_UNIT_INSPECTION,
                1107,
                "unsupported binary operation: pending(nothing) Eq nothing",
            ),
            (
                PENDING_TEXT_INSPECTION,
                1108,
                "unsupported binary operation: pending(retained) Eq retained",
            ),
        ] {
            let program = crate::resource_execution::tests::companion_program(source, release);
            let target = entry(&program);
            let mut interpreter = Interpreter::from_checked_resource_program(
                program,
                ExecutionPurpose::ReferenceRuntime,
            )
            .unwrap();
            let grant = interpreter
                .install_resource_test_script(vec![
                    ScriptOperation::Construct {
                        label,
                        outcome: Ok(())
                    };
                    2
                ])
                .unwrap();
            for invocation in 0..2 {
                let error = interpreter
                    .call_checked_program_entry(target, vec![grant.clone()])
                    .unwrap_err();
                assert_eq!(
                    error, expected,
                    "release={release}, invocation={invocation}"
                );
                let expected_events = [
                    ProviderEvent::Constructed(label),
                    ProviderEvent::Finalized(label),
                ]
                .into_iter()
                .cycle()
                .take((invocation + 1) * 2)
                .collect::<Vec<_>>();
                assert_eq!(
                    interpreter.resource_test_observations().unwrap(),
                    (expected_events, 0, 0)
                );
            }
        }
    }
}

#[test]
fn occupied_struct_reverse_cleanup_follows_lexical_actuals_not_field_positions() {
    run(
        REVERSE,
        vec![
            ScriptOperation::Construct {
                label: 1002,
                outcome: Ok(()),
            },
            ScriptOperation::Construct {
                label: 1003,
                outcome: Ok(()),
            },
            ScriptOperation::Construct {
                label: 1002,
                outcome: Ok(()),
            },
            ScriptOperation::Construct {
                label: 1003,
                outcome: Ok(()),
            },
        ],
        vec![
            ProviderEvent::Constructed(1002),
            ProviderEvent::Constructed(1003),
            ProviderEvent::Finalized(1002),
            ProviderEvent::Finalized(1003),
        ],
    );
}

#[test]
fn occupied_struct_later_handler_return_retires_acquired_prefix_and_reenters() {
    run(
        PREFIX,
        vec![
            ScriptOperation::Construct {
                label: 1004,
                outcome: Ok(())
            };
            2
        ],
        vec![
            ProviderEvent::Constructed(1004),
            ProviderEvent::Finalized(1004),
        ],
    );
}

#[test]
fn occupied_struct_finalizer_panic_retires_owner_before_error_and_keeps_ready_reentry() {
    for release in [false, true] {
        let program = crate::resource_execution::tests::program(SINGLE, release);
        let target = entry(&program);
        let mut interpreter =
            Interpreter::from_checked_resource_program(program, ExecutionPurpose::ReferenceRuntime)
                .unwrap();
        let grant = interpreter
            .install_resource_test_script(vec![
                ScriptOperation::ConstructFinalizerPanic { label: 1001 },
                ScriptOperation::Construct {
                    label: 1001,
                    outcome: Ok(()),
                },
            ])
            .unwrap();
        let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            interpreter.call_checked_program_entry(target, vec![grant.clone()])
        }));
        assert!(first.is_err());
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (
                vec![
                    ProviderEvent::Constructed(1001),
                    ProviderEvent::Finalized(1001)
                ],
                0,
                0
            )
        );
        assert_eq!(
            interpreter
                .call_checked_program_entry(target, vec![grant])
                .unwrap(),
            Value::Nothing
        );
        assert_eq!(
            interpreter.resource_test_observations().unwrap(),
            (
                vec![
                    ProviderEvent::Constructed(1001),
                    ProviderEvent::Finalized(1001),
                    ProviderEvent::Constructed(1001),
                    ProviderEvent::Finalized(1001)
                ],
                0,
                0
            )
        );
    }
}

/// Historical drafts retain their actual E0503 boundary. Owning field custody
/// must never be inferred by weakening the checked external entry policy.
#[test]
fn occupied_struct_original_view_main_is_still_rejected_before_entry() {
    use jett_common::SourceOrigin;
    use jett_parser::{ast::Item, parse};
    use jett_resolve::ResourceKernelSpec;
    use jett_typecheck::{CheckOptions, CheckedResourceProgram};
    use jett_types::ResourceKernelRecipe;
    use std::collections::HashMap;
    const SUPPORT: &str = include_str!("../fixtures/resource_probe.jett");
    const INVALID: [&str; 3] = [
        include_str!("../fixtures/30_owned_struct_scope_drop.jett"),
        include_str!("../fixtures/31_owned_struct_reverse.jett"),
        include_str!("../fixtures/32_owned_struct_abandoned_prefix.jett"),
    ];
    for release in [false, true] {
        for source in INVALID {
            let stdlib = FileId::new(10_000);
            let project = FileId::new(0);
            let mut parsed = parse(SUPPORT, stdlib);
            let original = parse(source, project);
            assert!(parsed.errors.is_empty() && original.errors.is_empty());
            let resource = parsed
                .module
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Resource(resource) => Some(resource.name.span),
                    _ => None,
                })
                .unwrap();
            parsed.module.items.extend(original.module.items);
            let catalog = [
                ("kernel_create", ResourceKernelRecipe::NetworkFactory),
                ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
                ("kernel_close", ResourceKernelRecipe::Finalize),
            ]
            .into_iter()
            .map(|(member, recipe)| ResourceKernelSpec {
                resource_declaration: resource,
                member: member.into(),
                recipe,
            })
            .collect::<Vec<_>>();
            let error = CheckedResourceProgram::prepare(
                parsed,
                HashMap::from([
                    (stdlib, SourceOrigin::Stdlib),
                    (project, SourceOrigin::Project),
                ]),
                &catalog,
                CheckOptions { release },
            )
            .err()
            .expect("original viewed main remains rejected");
            let diagnostics = error
                .diagnostics()
                .expect("original E0503 checker diagnostics");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code.code() == 503
                        && diagnostic.span.file == project
                        && diagnostic.message
                            == "`main` must own runtime capability parameter `net`; remove `view`"),
                "release={release}: {diagnostics:?}"
            );
        }
    }
}
