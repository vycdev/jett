use super::*;
use jett_common::{FileId, SourceOrigin};
use jett_parser::{ast::Item, parse};
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use jett_types::ResourceKernelRecipe;
use std::{collections::HashMap, sync::Arc};
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
            .unwrap()
            .id;
        assert!(
            EmittedResourceLayout::from_program(&program, types, wrong)
                .unwrap_err()
                .to_string()
                .contains("original closed Network/Nothing header")
        );
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
