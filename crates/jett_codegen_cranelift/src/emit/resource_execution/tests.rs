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
