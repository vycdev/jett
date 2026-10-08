use crate::{StatementKind as S, TerminatorKind as T};
use jett_common::{FileId, SourceOrigin};
use jett_hir::{CallOwnership, Expression, ExpressionKind as E};
use jett_parser::{ast::Item, parse};
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use jett_types::{ResourceKernelRecipe, TypeInterner};
use std::collections::HashMap;
use std::sync::Arc;
const SUPPORT: &str = r#"namespace resource_probe
export resource TestHandle
export function create(view net: Network, label: int64) returns result[TestHandle, string]:
    return kernel_create(label: label, net: view net)
export function close(token: TestHandle) returns nothing:
    kernel_close(token)
    return nothing
export function descriptor() returns function(TestHandle) returns nothing:
    return kernel_close
"#;
const SOURCE: &str = r#"namespace app
function factory(view net: Network, label: int64) returns result[resource_probe.TestHandle, string]:
    use resource_probe
    return resource_probe.create(view net, label)
"#;
fn checked(release: bool) -> Arc<CheckedResourceProgram> {
    let stdlib = FileId::new(10_000);
    let project = FileId::new(0);
    let mut parsed = parse(SUPPORT, stdlib);
    let source = parse(SOURCE, project);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(source.errors.is_empty(), "{:?}", source.errors);
    let declaration = parsed
        .module
        .items
        .iter()
        .find_map(|item| {
            if let Item::Resource(resource) = item {
                Some(resource.name.span)
            } else {
                None
            }
        })
        .unwrap();
    parsed.module.items.extend(source.module.items);
    parsed.errors.extend(source.errors);
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
fn invoke(program: &crate::Program) -> &Expression {
    program
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .find_map(|block| {
            if let T::Return(Some(value)) = &block.terminator.kind
                && matches!(value.kind, E::ResourceInvoke { .. })
            {
                return Some(value);
            }
            block
                .statements
                .iter()
                .find_map(|statement| match &statement.kind {
                    S::Let { value, .. } | S::Evaluate(value)
                        if matches!(value.kind, E::ResourceInvoke { .. }) =>
                    {
                        Some(value)
                    }
                    _ => None,
                })
        })
        .expect("Source ResourceInvoke survives typed lowering")
}
fn invoke_mut(program: &mut crate::Program) -> &mut Expression {
    program
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.blocks)
        .find_map(|block| {
            if let T::Return(Some(value)) = &mut block.terminator.kind
                && matches!(value.kind, E::ResourceInvoke { .. })
            {
                return Some(value);
            }
            block
                .statements
                .iter_mut()
                .find_map(|statement| match &mut statement.kind {
                    S::Let { value, .. } | S::Evaluate(value)
                        if matches!(value.kind, E::ResourceInvoke { .. }) =>
                    {
                        Some(value)
                    }
                    _ => None,
                })
        })
        .unwrap()
}
#[test]
fn resource_manifest_survives_mir_without_ordinary_owner_authority() {
    for release in [false, true] {
        let original = checked(release);
        let types = &original.checked().interner;
        let hir = jett_hir::lower_checked_resource_program(&original).unwrap();
        let mut mir = crate::lower(&hir, types).unwrap();
        assert_eq!(mir.resource_manifest, hir.resource_manifest);
        let E::ResourceInvoke {
            hook,
            evaluation_order,
            ownership: CallOwnership::Source(source),
            ..
        } = &invoke(&mir).kind
        else {
            panic!("original Source hook");
        };
        assert_eq!(hook.recipe(), ResourceKernelRecipe::NetworkFactory);
        assert_eq!(evaluation_order, &[1, 0]);
        assert!(mir.resource_manifest.contains_hook(hook));
        assert!(
            source
                .arguments
                .iter()
                .all(|argument| matches!(argument.staging, jett_hir::ArgumentStaging::Original))
        );
        crate::validate_call_ownership(&mir, types).unwrap();
        let operations = mir.functions.iter().filter(|function| function.blocks.iter().any(|block| {
            matches!(&block.terminator.kind, T::Return(Some(Expression { kind: E::ResourceInvoke { hook, .. }, .. })) if hook.recipe() == ResourceKernelRecipe::NetworkFactory)
        })).collect::<Vec<_>>();
        let [function] = operations.as_slice() else {
            panic!("one exact original factory body");
        };
        assert!(
            crate::validate_caller_acquisitions(&mir, function, types)
                .unwrap_err()
                .contains("pending ResourceOwnershipPlan")
        );
        assert!(
            crate::move_values::MoveValuePlan::analyze(&mir, function, types)
                .unwrap_err()
                .contains("pending ResourceOwnershipPlan")
        );
        assert!(
            crate::copy_values::CopyValuePlan::analyze(function, types)
                .unwrap_err()
                .contains("pending ResourceOwnershipPlan")
        );
        crate::prepare_native_sequences(&mut mir, types);
        crate::validate_call_ownership(&mir, types).unwrap();
        assert_eq!(mir.resource_manifest, hir.resource_manifest);
    }
}
#[test]
fn resource_manifest_mir_refuses_foreign_hook_and_original_packet_corruption() {
    for release in [false, true] {
        let original = checked(release);
        let types = &original.checked().interner;
        let hir = jett_hir::lower_checked_resource_program(&original).unwrap();
        let mir = crate::lower(&hir, types).unwrap();
        let foreign = jett_hir::lower_checked_resource_program(&checked(release)).unwrap();
        for mutation in 0..6 {
            let mut changed = mir.clone();
            if mutation == 0 {
                changed.resource_manifest = jett_hir::ResourceManifest::empty();
            } else if mutation == 5 {
                let descriptor = changed
                    .functions
                    .iter_mut()
                    .flat_map(|function| &mut function.blocks)
                    .find_map(|block| {
                        if let T::Return(Some(value)) = &mut block.terminator.kind
                            && matches!(value.kind, E::ResourceHookValue { .. })
                        {
                            Some(value)
                        } else {
                            None
                        }
                    })
                    .expect("exact original descriptor result");
                descriptor.ty = TypeInterner::STRING;
            } else {
                let expression = invoke_mut(&mut changed);
                let E::ResourceInvoke {
                    hook,
                    args,
                    evaluation_order,
                    ownership: CallOwnership::Source(source),
                } = &mut expression.kind
                else {
                    unreachable!()
                };
                match mutation {
                    1 => {
                        *hook = foreign
                            .resource_manifest
                            .hooks()
                            .find(|hook| hook.recipe() == ResourceKernelRecipe::NetworkFactory)
                            .unwrap()
                    }
                    2 => *evaluation_order = vec![0, 1],
                    3 => {
                        source.arguments[0].staging = jett_hir::ArgumentStaging::Copied {
                            value: jett_hir::LocalId::new(0),
                        }
                    }
                    _ => args[1].ty = TypeInterner::STRING,
                }
            }
            let errors = crate::validate_call_ownership(&changed, types).unwrap_err();
            if mutation == 5 {
                assert!(
                    errors
                        .iter()
                        .any(|error| error.message.contains("Resource descriptor differs")),
                    "{errors:?}"
                );
            }
        }
    }
}
