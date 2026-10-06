use super::*;
use jett_common::{FileId, SourceOrigin};
use jett_parser::{ast::Item, parse};
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use jett_types::ResourceKernelRecipe;
use std::{collections::HashMap, sync::Arc};
mod named_indirect;
const SUPPORT: &str = r#"namespace resource_probe
export resource TestHandle
export function create(view net: Network, label: int64) returns result[TestHandle, string]:
    return kernel_create(label: label, net: view net)
export function borrow(view net: Network, view token: TestHandle) returns result[int64, string]:
    return kernel_borrow(view net, view token)
export function close(token: TestHandle) returns nothing:
    kernel_close(token)
    return nothing
export function empty() returns optional[TestHandle]:
    return none
export function descriptor() returns function(TestHandle) returns nothing:
    return kernel_close
export function terminal_label() returns int64:
    list[int64] numbers = list(1)
    list[int64] removed = list.__remove_at[int64](numbers, -1)
    return 1
"#;
fn checked(source: &str, release: bool) -> Arc<CheckedResourceProgram> {
    let stdlib = FileId::new(10_000);
    let project = FileId::new(0);
    let mut parsed = parse(SUPPORT, stdlib);
    let primary = parse(source, project);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(primary.errors.is_empty(), "{:?}", primary.errors);
    let declaration = parsed
        .module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Resource(resource) => Some(resource.name.span),
            _ => None,
        })
        .unwrap();
    parsed.module.items.extend(primary.module.items);
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: declaration,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    Arc::new(
        CheckedResourceProgram::prepare(
            parsed,
            HashMap::from([
                (stdlib, SourceOrigin::Stdlib),
                (project, SourceOrigin::Project),
            ]),
            &catalog,
            CheckOptions { release },
        )
        .unwrap_or_else(|error| panic!("{error:?}; {:?}", error.diagnostics())),
    )
}

const SOURCE: &str = include_str!("../resource_layout/connected_entry.jett");
fn lower_source(checked: &Arc<CheckedResourceProgram>) -> Program {
    let hir = jett_hir::lower_checked_resource_program(checked).unwrap();
    jett_mir::lower(&hir, &checked.checked().interner).unwrap()
}
fn entry(program: &Program) -> FunctionId {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "main"
        })
        .unwrap()
        .id
}

fn emitted(source: &str, release: bool) -> ObjectArtifact {
    let checked = checked(source, release);
    let program = lower_source(&checked);
    super::super::emit_for_triple(
        &program,
        &checked.checked().interner,
        HOST,
        Some(entry(&program)),
        CodegenOptions { optimize: release },
    )
    .unwrap()
}
fn symbols(artifact: &ObjectArtifact) -> (Vec<String>, Vec<String>) {
    use object::{Object, ObjectSymbol};
    let file = object::File::parse(artifact.bytes.as_slice()).unwrap();
    let mut definitions = Vec::new();
    let mut imports = Vec::new();
    for symbol in file.symbols() {
        if let Ok(name) = symbol.name() {
            if symbol.is_definition() {
                definitions.push(name.into());
            } else if symbol.is_undefined() {
                imports.push(name.into());
            }
        }
    }
    (definitions, imports)
}
#[test]
fn resource_native_body_emits_original_connected_source_lifecycle_and_selected_leaves() {
    for release in [false, true] {
        let artifact = emitted(SOURCE, release);
        let (definitions, imports) = symbols(&artifact);
        assert!(
            definitions
                .iter()
                .any(|name| name == JETT_AOT_ENTRY_SYMBOL_V1)
        );
        assert!(
            definitions
                .iter()
                .any(|name| name == "jett_aot_resource_v1_manifest")
        );
        for required in [
            "entry_scope",
            "entry_network",
            "entry_outcome",
            "scope_validate",
            "source_enter",
            "source_parameter",
            "source_status",
            "factory_commit",
            "borrow_commit",
            "close",
            "sum_take",
            "failure_companion_take",
            "scope_complete",
            "return_publish",
        ] {
            assert!(
                imports
                    .iter()
                    .any(|name| name == &format!("jett_rt_v1_resource_{required}")),
                "{required}: {imports:?}"
            );
        }
    }
}
#[test]
fn resource_native_body_preserves_domain_abi_plus_exact_hidden_scope_without_public_scalar_admission()
 {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let program = lower_source(&checked);
        let types = &checked.checked().interner;
        let layout = EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
        let helper = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "pass_owner"
            })
            .unwrap();
        let isa = isa::lookup(HOST)
            .unwrap()
            .finish(settings::Flags::new(settings::builder()))
            .unwrap();
        let module = ObjectModule::new(
            ObjectBuilder::new(isa, "resource_signature", default_libcall_names()).unwrap(),
        );
        let selected = signature(&module, helper, types, &layout).unwrap();
        assert_eq!(selected.params.len(), 4);
        assert!(
            selected
                .params
                .iter()
                .all(|parameter| parameter.value_type == ir::types::I64)
        );
        assert_eq!(selected.returns.len(), 1);
        assert_eq!(selected.returns[0].value_type, ir::types::I64);
        assert!(scalar_kind(types, helper.return_type, "public Resource scalar gate").is_err());
        assert!(MoveValuePlan::analyze(&program, helper, types).is_err());
    }
}
#[test]
fn resource_native_body_stages_owned_actual_before_a_later_fallible_source_actual() {
    let source = r#"namespace app
function consume(token: resource_probe.TestHandle, label: int64) returns nothing:
    use resource_probe
    resource_probe.close(token)
    return nothing
function main(net: Network) returns nothing:
    use resource_probe
    resource_probe.TestHandle token = resource_probe.create(view net, 1) handle error:
        return nothing
    consume(token, resource_probe.terminal_label())
    return nothing
"#;
    for release in [false, true] {
        let artifact = emitted(source, release);
        let (_, imports) = symbols(&artifact);
        for required in [
            "transfer",
            "operation_begin",
            "operation_complete",
            "scope_complete",
            "source_actual",
            "source_parameter",
        ] {
            assert!(
                imports
                    .iter()
                    .any(|name| name == &format!("jett_rt_v1_resource_{required}"))
            );
        }
    }
}
#[test]
fn resource_native_body_authenticates_original_source_before_emission_and_keeps_absent_failure_shells_typed()
 {
    let source = r#"namespace app
function main(net: Network) returns nothing:
    use resource_probe
    optional[resource_probe.TestHandle] absent = resource_probe.empty()
    result[resource_probe.TestHandle, string] rejected = fail("label")
    return nothing
"#;
    for release in [false, true] {
        let artifact = emitted(source, release);
        let (_, imports) = symbols(&artifact);
        for required in [
            "absent_sum",
            "failure_sum",
            "scope_complete",
            "return_publish",
        ] {
            assert!(
                imports
                    .iter()
                    .any(|name| name == &format!("jett_rt_v1_resource_{required}"))
            );
        }
        let checked = checked(SOURCE, release);
        let original = lower_source(&checked);
        let types = &checked.checked().interner;
        let selected = entry(&original);
        for mutation in 0..3 {
            let mut changed = original.clone();
            match mutation {
                0 => {
                    changed.functions[selected.index() as usize].params[0].mode =
                        jett_mir::ParamMode::View
                }
                1 => changed.functions[selected.index() as usize].blocks[0]
                    .statements
                    .clear(),
                2 => changed.resource_manifest = jett_hir::ResourceManifest::empty(),
                _ => unreachable!(),
            }
            assert!(
                super::super::emit_for_triple(
                    &changed,
                    types,
                    HOST,
                    Some(selected),
                    CodegenOptions::default()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn resource_native_body_refuses_ordinary_descriptor_abi_for_an_exact_hidden_scope_family_target() {
    let source = r#"namespace app
function lifecycle(view net: Network) returns nothing:
    use resource_probe
    resource_probe.TestHandle token = resource_probe.create(view net, 1) handle error:
        return nothing
    resource_probe.close(token)
    return nothing
function main(net: Network) returns nothing:
    lifecycle(view net)
    function(view Network) returns nothing callback = lifecycle
    callback(view net)
    return nothing
"#;
    for release in [false, true] {
        let checked = checked(source, release);
        let program = lower_source(&checked);
        let error = super::super::emit_for_triple(
            &program,
            &checked.checked().interner,
            HOST,
            Some(entry(&program)),
            CodegenOptions { optimize: release },
        )
        .unwrap_err();
        assert!(
            matches!(error, CodegenError::UnsupportedMir { construct, .. } if construct == "pending Resource family descriptor hidden Scope ABI")
        );
    }
}

#[test]
fn resource_native_replacement_original_source_emits_replace_reseat_and_vacant_routes() {
    const SOURCE: &str = include_str!("replacement/mutable_assignment.jett");
    for release in [false, true] {
        for (selected, replaces) in [
            ("replace_live", true),
            ("rebind_self", false),
            ("rebind_after_close", false),
            ("failed_rhs_keeps_owner", true),
        ] {
            // Native verification deliberately emits every fresh plan function.
            // Isolate this exact actual Source function so an unused live-replace
            // helper cannot supply a misleading Replace import for a reseat.
            let mut declarations = SOURCE.split("export function ");
            let prefix = declarations.next().unwrap();
            let body = declarations
                .find(|body| body.starts_with(&format!("{selected}(")))
                .unwrap();
            let source = format!(
                "{prefix}export function {body}export function main(net: Network) returns nothing:
    {selected}(view net)
    return nothing
"
            );
            let artifact = emitted(&source, release);
            let (_, imports) = symbols(&artifact);
            assert_eq!(
                imports
                    .iter()
                    .any(|name| name == "jett_rt_v1_resource_replace"),
                replaces,
                "{selected}: {imports:?}"
            );
            for leaf in ["source_enter", "scope_complete", "transfer", "close"] {
                assert!(
                    imports
                        .iter()
                        .any(|name| name == &format!("jett_rt_v1_resource_{leaf}")),
                    "{selected}/{leaf}: {imports:?}"
                );
            }
        }
    }
}

#[test]
fn resource_native_replacement_ordinary_rhs_failure_keeps_original_owner_until_cleanup() {
    const SOURCE: &str = r#"namespace app
function replacement_label() returns int64:
    use resource_probe
    return resource_probe.terminal_label()
export function main(net: Network) returns nothing:
    use resource_probe
    mutable resource_probe.TestHandle token = resource_probe.create(view net, 2441) handle error:
        return nothing
    token = resource_probe.create(view net, replacement_label()) handle error:
        return nothing
    resource_probe.close(token)
    return nothing
"#;
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let program = lower_source(&checked);
        let layout = EmittedResourceLayout::from_program(
            &program,
            &checked.checked().interner,
            entry(&program),
        )
        .unwrap();
        let main = layout.plan().function(entry(&program)).unwrap();
        assert!(
            main.operations()
                .iter()
                .any(|operation| matches!(operation.role(), Role::Replace { .. }))
        );
        let artifact = emitted(SOURCE, release);
        let (_, imports) = symbols(&artifact);
        for leaf in ["replace", "operation_complete", "scope_complete"] {
            assert!(
                imports
                    .iter()
                    .any(|name| name == &format!("jett_rt_v1_resource_{leaf}"))
            );
        }
        assert!(imports.iter().any(|name| name.contains("failure")));
    }
}

#[test]
fn resource_staged_real_source_emits_cross_block_prepare_stage_invoke_and_retirement_leaves() {
    for release in [false, true] {
        for source in [
            include_str!("staged_call/09_later_handle_minimal.jett"),
            include_str!("staged_call/10_later_handle_original.jett"),
            include_str!("staged_call/11_later_handle_success.jett"),
            include_str!("staged_call/12_later_handle_written_view.jett"),
            include_str!("staged_call/13_first_handle_failure.jett"),
            include_str!("staged_call/14_first_handle_success.jett"),
        ] {
            let checked = checked(source, release);
            let original = lower_source(&checked);
            let selected = entry(&original);
            assert!(
                original
                    .functions
                    .iter()
                    .any(|function| function.blocks.iter().any(|block| block
                        .statements
                        .iter()
                        .any(|statement| matches!(
                            statement.kind,
                            StatementKind::ResourceCall(jett_mir::ResourceCallNode::End {
                                outcome: jett_mir::ResourceCompletion::Abort,
                                ..
                            })
                        ))))
            );
            for function in &original.functions {
                for block in &function.blocks {
                    for statement in &block.statements {
                        if let StatementKind::ResourceCall(jett_mir::ResourceCallNode::Stage {
                            value,
                            ..
                        }) = &statement.kind
                        {
                            assert!(!matches!(value.kind, ExpressionKind::RuntimeFailure(_)));
                        }
                    }
                }
            }
            let artifact = super::super::emit_for_triple(
                &original,
                &checked.checked().interner,
                HOST,
                Some(selected),
                CodegenOptions { optimize: release },
            )
            .unwrap();
            let (_, imports) = symbols(&artifact);
            for leaf in [
                "source_prepare",
                "source_actual",
                "source_enter",
                "source_parameter",
                "source_status",
                "borrow_begin",
                "borrow_end",
                "operation_begin",
                "operation_complete",
                "scope_complete",
                "entry_outcome",
            ] {
                assert!(
                    imports
                        .iter()
                        .any(|name| name == &format!("jett_rt_v1_resource_{leaf}")),
                    "{leaf}: {imports:?}"
                );
            }
            // This inspects an emitted object only. Provider events and all-before-destroy
            // outcomes belong to Root's linked Source-deleted conformance gate.
        }
    }
}

#[test]
fn resource_staged_object_refuses_altered_ordinary_endpoint_and_forged_current_node() {
    let source = include_str!("staged_call/09_later_handle_minimal.jett");
    for release in [false, true] {
        let checked = checked(source, release);
        let original = lower_source(&checked);
        let selected = entry(&original);
        let (function, block, index, local) = original
            .functions
            .iter()
            .find_map(|function| {
                function.blocks.iter().find_map(|block| {
                    block
                        .statements
                        .iter()
                        .enumerate()
                        .find_map(|(index, statement)| match &statement.kind {
                            StatementKind::ResourceCall(jett_mir::ResourceCallNode::Stage {
                                ordinary: Some(local),
                                ..
                            }) => Some((function.id, block.id, index, *local)),
                            _ => None,
                        })
                })
            })
            .unwrap();
        for mutation in 0..3 {
            let mut changed = original.clone();
            let current = &mut changed.functions[function.index() as usize];
            match mutation {
                0 => {
                    assert!(current.locals[local.index() as usize].mutable);
                    current.locals[local.index() as usize].mutable = false;
                }
                1 => {
                    let StatementKind::ResourceCall(jett_mir::ResourceCallNode::Stage {
                        ordinary,
                        ..
                    }) = &mut current.blocks[block.index() as usize].statements[index].kind
                    else {
                        unreachable!()
                    };
                    *ordinary = Some(current.params[0].local);
                }
                2 => {
                    let copied = current.blocks[block.index() as usize].statements[index].clone();
                    current.blocks[block.index() as usize]
                        .statements
                        .push(copied);
                }
                _ => unreachable!(),
            }
            assert!(
                super::super::emit_for_triple(
                    &changed,
                    &checked.checked().interner,
                    HOST,
                    Some(selected),
                    CodegenOptions { optimize: release }
                )
                .is_err(),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn resource_staged_scope_sum_keeps_active_operation_rows_and_forgery_refusal() {
    for release in [false, true] {
        for source in [
            include_str!("staged_call/13_first_handle_failure.jett"),
            include_str!("staged_call/14_first_handle_success.jett"),
        ] {
            let checked = checked(source, release);
            let original = lower_source(&checked);
            let selected = entry(&original);
            let layout = EmittedResourceLayout::from_program(
                &original,
                &checked.checked().interner,
                selected,
            )
            .unwrap();
            let flow = layout.plan().function(selected).unwrap();
            let active = flow
                .operations()
                .iter()
                .find(|operation| matches!(operation.role(), Role::BeginSourceFunction { .. }))
                .unwrap()
                .frame();
            let mut extractions = 0;
            for operation in flow.operations() {
                if matches!(
                    operation.role(),
                    Role::SumTake { .. } | Role::TakeFailureCompanion { .. }
                ) {
                    extractions += 1;
                    assert_eq!(operation.frame(), active);
                    assert!(layout.operation(selected, operation.id()).is_some());
                }
            }
            assert_eq!(extractions, 2);
            let main = &original.functions[selected.index() as usize];
            let (block, index) = main
                .blocks
                .iter()
                .find_map(|block| {
                    block
                        .statements
                        .iter()
                        .enumerate()
                        .find_map(|(index, statement)| {
                            matches!(statement.kind, StatementKind::SumTag { .. })
                                .then_some((block.id, index))
                        })
                })
                .unwrap();
            assert_eq!(
                flow.execution_frame(block, jett_mir::ResourcePosition::Statement(index)),
                Some(active)
            );
            super::super::emit_for_triple(
                &original,
                &checked.checked().interner,
                HOST,
                Some(selected),
                CodegenOptions { optimize: release },
            )
            .unwrap();
            for copied in [false, true] {
                let mut changed = original.clone();
                let current = &mut changed.functions[selected.index() as usize];
                if copied {
                    let statement =
                        current.blocks[block.index() as usize].statements[index].clone();
                    current.blocks[block.index() as usize]
                        .statements
                        .insert(index, statement);
                } else {
                    let StatementKind::SumTag { source, .. } =
                        &mut current.blocks[block.index() as usize].statements[index].kind
                    else {
                        unreachable!()
                    };
                    *source = current.params[0].local;
                }
                assert!(matches!(
                    super::super::emit_for_triple(
                        &changed,
                        &checked.checked().interner,
                        HOST,
                        Some(selected),
                        CodegenOptions { optimize: release }
                    ),
                    Err(CodegenError::InvalidMir(_))
                ));
            }
        }
    }
}
