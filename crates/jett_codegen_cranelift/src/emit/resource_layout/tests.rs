use super::*;
use jett_common::{FileId, SourceOrigin};
use jett_parser::{ast::Item, parse};
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use jett_types::ResourceKernelRecipe;
use std::{collections::HashMap, sync::Arc};
#[path = "carrier_tests.rs"]
mod carrier_tests;
#[path = "returned_hooks_tests.rs"]
mod returned_hooks;
#[path = "sum_views_tests.rs"]
mod sum_views;
const SUPPORT: &str =
    include_str!("../../../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
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

const SOURCE: &str = include_str!("connected_entry.jett");
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
#[test]
fn resource_emitted_layout_joins_actual_driver_identity_and_unused_hooks_deterministically() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let program = lower_source(&checked);
        let types = &checked.checked().interner;
        let selected = entry(&program);
        let first = EmittedResourceLayout::from_program(&program, types, selected).unwrap();
        let second = EmittedResourceLayout::from_program(&program, types, selected).unwrap();
        assert_eq!(first.entry(), second.entry());
        assert_eq!(first.bytes(), second.bytes());
        assert_eq!(&first.bytes()[..8], b"JTRSC001");
        assert_eq!(
            u32::from_le_bytes(first.bytes()[8..12].try_into().unwrap()),
            2
        );
        assert_eq!(
            u64::from_le_bytes(first.bytes()[16..24].try_into().unwrap()),
            first.bytes().len() as u64
        );
        assert_eq!(first.entry().function, selected.index());
        let mut rows = Rows::new(first.plan()).unwrap();
        rows.populate().unwrap();
        assert_eq!(rows.hooks.len(), 3);
        assert_eq!(
            rows.hook_ids.len(),
            program.resource_manifest.hooks().count()
        );
        for function in first.plan().functions() {
            let signature = first.function_signature(function.function()).unwrap();
            for frame in function.frames() {
                assert_eq!(
                    rows.frames[first.frame(function.function(), frame.id()).unwrap() as usize]
                        .signature,
                    signature
                );
            }
        }
        let wrong = program
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "pass_owner"
            })
            .unwrap();
        let error = EmittedResourceLayout::from_program(&program, types, wrong.id).unwrap_err();
        match error {
            CodegenError::InvalidMir(errors) => assert!(errors.iter().any(|error| {
                error.span == wrong.span
                    && error.message
                        == "native Resource entry changed its closed Network/Nothing Source header"
            })),
            other => panic!("wrong selected entry used an unexpected refusal boundary: {other}"),
        }
    }
}
#[test]
fn resource_emitted_layout_keeps_call_site_result_destinations_separate_from_callee_return() {
    let source = SOURCE.replace("result[int64, string] observed = resource_probe.borrow(view net, view moved)", "resource_probe.TestHandle final = pass_owner(moved)\n    result[int64, string] observed = resource_probe.borrow(view net, view final)").replace("resource_probe.close(moved)", "resource_probe.close(final)");
    for release in [false, true] {
        let checked = checked(&source, release);
        let program = lower_source(&checked);
        let layout = EmittedResourceLayout::from_program(
            &program,
            &checked.checked().interner,
            entry(&program),
        )
        .unwrap();
        let helper = layout
            .plan()
            .functions()
            .iter()
            .find(|function| {
                function.identity().declaration.namespace == "app"
                    && function.identity().declaration.name == "pass_owner"
            })
            .unwrap();
        let mut calls = Vec::new();
        for function in layout.plan().functions() {
            for operation in function.operations() {
                if let Role::InvokeSourceFunction {
                    function: target,
                    result: CallResult::Owned { slot },
                    ..
                } = operation.role()
                    && *target == helper.function()
                {
                    calls.push((
                        layout
                            .operation(function.function(), operation.id())
                            .unwrap(),
                        layout.slot(function.function(), *slot).unwrap(),
                        helper.provisional_return().unwrap().id(),
                    ));
                    assert_ne!(
                        function.owner_slots()[slot.index()].frame(),
                        operation.frame()
                    );
                }
            }
        }
        assert_eq!(calls.len(), 2);
        assert_ne!(calls[0].0, calls[1].0);
        assert_ne!(calls[0].1, calls[1].1);
        assert_eq!(calls[0].2, calls[1].2);
        let mut rows = Rows::new(layout.plan()).unwrap();
        rows.populate().unwrap();
        let scope = layout
            .frame(helper.function(), helper.root_scope().id())
            .unwrap();
        for operation in helper.operations() {
            if let Role::Transfer {
                source,
                destination,
            } = operation.role()
                && matches!(
                    helper.owner_slots()[destination.index()].storage(),
                    custody::ResourceSlotStorage::Return { .. }
                )
            {
                let id = layout.operation(helper.function(), operation.id()).unwrap();
                assert_eq!(
                    rows.operations[id as usize].1,
                    [
                        2,
                        scope,
                        layout.slot(helper.function(), *source).unwrap(),
                        layout.slot(helper.function(), *destination).unwrap()
                    ]
                );
                assert_eq!(operation.frame(), helper.provisional_return().unwrap().id());
                assert_eq!(
                    helper.owner_slots()[destination.index()].frame(),
                    operation.frame()
                );
            }
            if let Role::CompleteReturnAfterCleanup { source } = operation.role() {
                let id = layout.operation(helper.function(), operation.id()).unwrap();
                assert_eq!(
                    rows.operations[id as usize].1,
                    [18, scope, layout.slot(helper.function(), *source).unwrap()]
                );
                assert_ne!(
                    layout.frame(helper.function(), operation.frame()).unwrap(),
                    scope
                );
                assert_eq!(
                    helper.owner_slots()[source.index()].frame(),
                    helper.provisional_return().unwrap().id()
                );
            }
        }
    }
}
#[test]
fn resource_emitted_layout_refuses_changed_current_source_graph_before_projecting_rows() {
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let program = lower_source(&checked);
        let selected = entry(&program);
        for mutation in 0..3 {
            let mut changed = program.clone();
            let function = &mut changed.functions[selected.index() as usize];
            match mutation {
                0 => function.params[0].mode = jett_mir::ParamMode::View,
                1 => function.identity.declaration.name.push_str("Foreign"),
                2 => function.blocks[function.entry.index() as usize]
                    .statements
                    .clear(),
                _ => unreachable!(),
            }
            let error = EmittedResourceLayout::from_program(&changed, types, selected).unwrap_err();
            assert!(matches!(error, CodegenError::InvalidMir(_)), "{error:?}");
        }
    }
}

#[test]
fn resource_manifest_accessor_emits_only_the_immutable_compiler_owned_projection() {
    use object::{Object, ObjectSymbol};
    let checked = checked(SOURCE, false);
    let program = lower_source(&checked);
    let layout =
        EmittedResourceLayout::from_program(&program, &checked.checked().interner, entry(&program))
            .unwrap();
    let target = isa::lookup(HOST)
        .unwrap()
        .finish(settings::Flags::new(settings::builder()))
        .unwrap();
    let object = ObjectBuilder::new(
        target,
        "resource_manifest_accessor",
        default_libcall_names(),
    )
    .unwrap();
    let mut module = ObjectModule::new(object);
    layout.define_accessor(&mut module).unwrap();
    assert!(matches!(
        layout.define_accessor(&mut module),
        Err(CodegenError::DuplicateSymbol(_))
    ));
    let bytes = module.finish().emit().unwrap();
    let object = object::File::parse(bytes.as_slice()).unwrap();
    assert!(object.symbols().any(|symbol| symbol.is_definition()
        && symbol.name().ok() == Some("jett_aot_resource_v1_manifest")));
    assert!(!object.symbols().any(|symbol| symbol.is_definition()
        && symbol.name().ok() == Some(super::super::JETT_AOT_ENTRY_SYMBOL_V1)));
    // This control emits metadata/accessor only: no body, leaf or provider execution.
}

#[test]
fn resource_layout_replacement_uses_exact_rhs_tag12_and_reseat_has_no_row() {
    const SOURCE: &str = include_str!("../resource_execution/replacement/mutable_assignment.jett");
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let program = lower_source(&checked);
        let layout = EmittedResourceLayout::from_program(
            &program,
            &checked.checked().interner,
            entry(&program),
        )
        .unwrap();
        let mut rows = Rows::new(layout.plan()).unwrap();
        rows.populate().unwrap();
        let mut replacements = 0;
        let mut reseats = 0;
        for function in layout.plan().functions() {
            for operation in function.operations() {
                match operation.role() {
                    Role::Replace {
                        destination,
                        replacement,
                        old,
                    } => {
                        replacements += 1;
                        assert_eq!(*old, custody::ResourceOccupancy::Occupied);
                        assert_ne!(destination, replacement);
                        let ordinal = layout
                            .operation(function.function(), operation.id())
                            .unwrap();
                        assert_eq!(
                            rows.operations[ordinal as usize].1,
                            [
                                12,
                                layout
                                    .frame(function.function(), operation.frame())
                                    .unwrap(),
                                layout.slot(function.function(), *destination).unwrap(),
                                layout.slot(function.function(), *replacement).unwrap(),
                            ]
                        );
                    }
                    Role::SelfRebind { .. } => {
                        reseats += 1;
                        assert!(
                            layout
                                .operation(function.function(), operation.id())
                                .is_none()
                        );
                    }
                    _ => {}
                }
            }
        }
        assert!(replacements >= 2);
        assert_eq!(reseats, 1);
    }
}

#[test]
fn resource_layout_sum_replacement_stays_a_separate_transport_refusal() {
    const SOURCE: &str = r#"namespace app
export function main(net: Network) returns nothing:
    use resource_probe
    mutable optional[resource_probe.TestHandle] token = none
    token = resource_probe.empty()
    return nothing
"#;
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let program = lower_source(&checked);
        let error = EmittedResourceLayout::from_program(
            &program,
            &checked.checked().interner,
            entry(&program),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains(
                "mutable replacement requires distinct occupied plain owners in the exact Scope"
            ),
            "{error:?}"
        );
    }
}

const STAGED_SOURCES: &[&str] = &[
    include_str!("../resource_execution/staged_call/09_later_handle_minimal.jett"),
    include_str!("../resource_execution/staged_call/10_later_handle_original.jett"),
    include_str!("../resource_execution/staged_call/11_later_handle_success.jett"),
    include_str!("../resource_execution/staged_call/12_later_handle_written_view.jett"),
    include_str!("../resource_execution/staged_call/13_first_handle_failure.jett"),
    include_str!("../resource_execution/staged_call/14_first_handle_success.jett"),
];

#[test]
fn resource_staged_layout_keeps_complete_source_tuple_and_exact_activation_aliases() {
    for release in [false, true] {
        for &source in STAGED_SOURCES {
            let checked = checked(source, release);
            let program = lower_source(&checked);
            let types = &checked.checked().interner;
            let selected = entry(&program);
            let layout = EmittedResourceLayout::from_program(&program, types, selected).unwrap();
            let repeated = EmittedResourceLayout::from_program(&program, types, selected).unwrap();
            assert_eq!(layout.bytes(), repeated.bytes());
            assert_eq!(
                u32::from_le_bytes(layout.bytes()[8..12].try_into().unwrap()),
                2
            );
            let mut rows = Rows::new(layout.plan()).unwrap();
            rows.populate().unwrap();
            let mut regions = 0;
            let mut aliases = 0;
            let mut abandoned = 0;
            for function in layout.plan().functions() {
                for operation in function.operations() {
                    if let Role::BeginSourceFunction {
                        function: target,
                        formals,
                        evaluation_order,
                        operands,
                        ..
                    } = operation.role()
                    {
                        regions += 1;
                        assert_eq!(evaluation_order, &[1, 0, 2]);
                        assert_eq!(formals.len(), 3);
                        assert_eq!(operands.len(), 3);
                        let id = layout
                            .operation(function.function(), operation.id())
                            .unwrap();
                        let row = &rows.operations[id as usize].1;
                        assert_eq!(row[0], 16);
                        assert_eq!(row[2], target.index());
                        assert_eq!(row[3], layout.function_signature(*target).unwrap());
                        assert_eq!(
                            rows.frames[row[1] as usize].signature,
                            layout.function_signature(function.function()).unwrap()
                        );
                        // Full formal metadata exists on Begin before any reached Stage/Invoke.
                        for operand in operands {
                            if let Operand::Borrowed { parameter, loan } = operand {
                                let record = &function.loans()[loan.index()];
                                if let custody::ResourceLoanSource::Owner(_) = record.source() {
                                    assert_eq!(record.parameter(), Some(*parameter));
                                    let mut prepared = function.operations().iter().filter(|candidate|
                                        matches!(candidate.role(), Role::PrepareSourceBorrow { loan: expected, parameter: p } if expected == loan && p == parameter));
                                    let prepared = prepared.next().unwrap();
                                    let borrow_id = layout
                                        .operation(function.function(), prepared.id())
                                        .unwrap();
                                    assert_eq!(rows.operations[borrow_id as usize].1[0], 3);
                                    assert_eq!(prepared.site(), operation.site());
                                }
                            }
                        }
                    }
                    if let Some(prepared) = staged_alias(function, operation).unwrap() {
                        aliases += 1;
                        assert_eq!(
                            layout.operation(function.function(), operation.id()),
                            layout.operation(function.function(), prepared.id())
                        );
                    }
                    if matches!(operation.role(), Role::StageSourceActual { .. }) {
                        assert!(
                            layout
                                .operation(function.function(), operation.id())
                                .is_none()
                        );
                    }
                    if matches!(
                        operation.role(),
                        Role::Complete {
                            outcome: custody::ResourceCompletion::Abort
                        }
                    ) && function.operations().iter().any(|begin| {
                        begin.frame() == operation.frame()
                            && matches!(begin.role(), Role::BeginSourceFunction { .. })
                    }) {
                        abandoned += 1;
                        let id = layout
                            .operation(function.function(), operation.id())
                            .unwrap();
                        assert_eq!(
                            rows.operations[id as usize].1,
                            [
                                13,
                                layout
                                    .frame(function.function(), operation.frame())
                                    .unwrap()
                            ]
                        );
                    }
                }
            }
            assert_eq!(regions, 1);
            assert!(aliases >= 2);
            assert!(abandoned >= 1);
        }
    }
}

#[test]
fn resource_staged_layout_refuses_edited_and_copied_public_nodes_before_rows() {
    for release in [false, true] {
        let checked = checked(STAGED_SOURCES[0], release);
        let original = lower_source(&checked);
        let selected = entry(&original);
        let stages = original
            .functions
            .iter()
            .flat_map(|function| {
                function.blocks.iter().flat_map(move |block| {
                    block
                        .statements
                        .iter()
                        .enumerate()
                        .filter_map(move |(index, statement)| {
                            matches!(
                                statement.kind,
                                StatementKind::ResourceCall(
                                    custody::ResourceCallNode::Stage { .. }
                                )
                            )
                            .then_some((function.id, block.id, index))
                        })
                })
            })
            .collect::<Vec<_>>();
        assert!(stages.len() >= 3);
        for mutation in 0..5 {
            let mut changed = original.clone();
            let (function, block, index) = stages[0];
            let current = &mut changed.functions[function.index() as usize];
            match mutation {
                0 => {
                    let StatementKind::ResourceCall(custody::ResourceCallNode::Stage {
                        source_index,
                        ..
                    }) = &mut current.blocks[block.index() as usize].statements[index].kind
                    else {
                        unreachable!()
                    };
                    *source_index += 1;
                }
                1 => {
                    let StatementKind::ResourceCall(custody::ResourceCallNode::Stage {
                        parameter,
                        ..
                    }) = &mut current.blocks[block.index() as usize].statements[index].kind
                    else {
                        unreachable!()
                    };
                    *parameter = 0;
                }
                2 => {
                    let StatementKind::ResourceCall(custody::ResourceCallNode::Stage {
                        value, ..
                    }) = &mut current.blocks[block.index() as usize].statements[index].kind
                    else {
                        unreachable!()
                    };
                    value.ty = TypeInterner::INT64;
                }
                3 => {
                    let copied = current.blocks[block.index() as usize].statements[index].clone();
                    current.blocks[block.index() as usize]
                        .statements
                        .insert(index, copied);
                }
                4 => {
                    let copied = current.blocks[block.index() as usize].statements[index].clone();
                    current.blocks[current.entry.index() as usize]
                        .statements
                        .push(copied);
                }
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    EmittedResourceLayout::from_program(
                        &changed,
                        &checked.checked().interner,
                        selected
                    ),
                    Err(CodegenError::InvalidMir(_))
                ),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn resource_staged_sum_rows_preserve_scope_storage_and_active_execution() {
    for release in [false, true] {
        for &source in &STAGED_SOURCES[4..] {
            let checked = checked(source, release);
            let program = lower_source(&checked);
            let selected = entry(&program);
            let layout = EmittedResourceLayout::from_program(
                &program,
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
            let scope = flow.root_scope().id();
            let mut rows = Rows::new(layout.plan()).unwrap();
            rows.populate().unwrap();
            let mut extractions = 0;
            for operation in flow.operations() {
                let source = match operation.role() {
                    Role::SumTake {
                        source,
                        destination,
                        ..
                    } => {
                        assert_eq!(flow.owner_slots()[destination.index()].frame(), scope);
                        *source
                    }
                    Role::TakeFailureCompanion { source, .. } => *source,
                    _ => continue,
                };
                extractions += 1;
                assert_eq!(flow.owner_slots()[source.index()].frame(), scope);
                assert_eq!(operation.frame(), active);
                let ordinal = layout.operation(selected, operation.id()).unwrap();
                assert_eq!(
                    rows.operations[ordinal as usize].1[1],
                    layout.frame(selected, active).unwrap()
                );
            }
            assert_eq!(extractions, 2);
        }
    }
}
