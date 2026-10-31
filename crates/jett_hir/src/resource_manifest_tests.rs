use super::*;
use crate::{
    CallBridge, CallOwnership, CallTarget, Expression, ExpressionKind as E, StatementKind as S,
};
use jett_common::{FileId, SourceOrigin};
use jett_parser::{ast::Item, parse};
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::{CheckOptions, CheckedCallerEffect, CheckedCallerSyntax};
use std::collections::HashMap;

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
function descriptor() returns function(resource_probe.TestHandle) returns nothing:
    use resource_probe
    return resource_probe.descriptor()
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
        .flat_map(|function| &function.body.statements)
        .find_map(|statement| match &statement.kind {
            S::Return(Some(value)) | S::Expression(value)
                if matches!(value.kind, E::ResourceInvoke { .. }) =>
            {
                Some(value)
            }
            _ => None,
        })
        .expect("exact Source hook invocation")
}
fn invoke_mut(program: &mut crate::Program) -> &mut Expression {
    program
        .functions
        .iter_mut()
        .flat_map(|function| &mut function.body.statements)
        .find_map(|statement| match &mut statement.kind {
            S::Return(Some(value)) | S::Expression(value)
                if matches!(value.kind, E::ResourceInvoke { .. }) =>
            {
                Some(value)
            }
            _ => None,
        })
        .unwrap()
}

#[test]
fn resource_manifest_keeps_source_hooks_descriptors_and_lexical_argument_order() {
    for release in [false, true] {
        let original = checked(release);
        let program = crate::lower_checked_resource_program(&original).unwrap();
        let types = &original.checked().interner;
        crate::validate_backend_types(&program, types).unwrap();
        assert_eq!(program.resource_manifest.kinds().len(), 1);
        assert_eq!(program.resource_manifest.hooks().count(), 3); // Borrow remains unused in Source.
        let E::ResourceInvoke {
            hook,
            args,
            evaluation_order,
            ownership: CallOwnership::Source(source),
        } = &invoke(&program).kind
        else {
            panic!("Source hook node");
        };
        assert_eq!(hook.recipe(), ResourceKernelRecipe::NetworkFactory);
        let kind = hook.resource_kind();
        kind.validate(types).unwrap();
        assert!(program.resource_manifest.contains_kind(&kind));
        assert_eq!(kind.ty(), hook.resource_type());
        assert_eq!(
            program.resource_manifest.kind_for_type(kind.ty()),
            Some(kind)
        );
        assert_eq!(evaluation_order, &[1, 0]);
        assert_eq!(args.len(), 2);
        assert_eq!(source.target, CallTarget::ResourceHook(hook.clone()));
        assert_eq!(
            source.bridge,
            CallBridge::ResourceHook { hook: hook.clone() }
        );
        assert_eq!(source.arguments[0].source_index, 1);
        assert_eq!(source.arguments[0].syntax, CheckedCallerSyntax::WrittenView);
        assert_eq!(
            source.arguments[0].effect,
            CheckedCallerEffect::RetainBorrow
        );
        assert_eq!(source.arguments[1].effect, CheckedCallerEffect::Copy);
        assert!(program.functions.iter().flat_map(|function| &function.body.statements).any(|statement| matches!(&statement.kind, S::Return(Some(Expression { kind: E::ResourceHookValue { hook }, ty, .. })) if hook.recipe() == ResourceKernelRecipe::Finalize && *ty == hook.function_type())));
        assert!(
            crate::lower(
                original.module(),
                original.resolved(),
                original.checked(),
                original.source_origins()
            )
            .is_err()
        );
        crate::lower_checked_resource_program_with_test_bodies(&original).unwrap();
    }
}

#[test]
fn resource_manifest_validates_unused_hooks_and_nominal_kind_records() {
    for release in [false, true] {
        let original = checked(release);
        let program = crate::lower_checked_resource_program(&original).unwrap();
        let data = program.resource_manifest.data.as_ref().unwrap();
        for mutation in 0..4 {
            let mut kinds = data.kinds.clone();
            let mut hooks = data.hooks.clone();
            match mutation {
                0 => {
                    hooks.retain(|hook| hook.recipe != ResourceKernelRecipe::NetworkBorrow);
                }
                1 => {
                    hooks
                        .iter_mut()
                        .find(|hook| hook.recipe == ResourceKernelRecipe::NetworkBorrow)
                        .unwrap()
                        .recipe = ResourceKernelRecipe::Finalize;
                }
                2 => {
                    kinds[0].ty = TypeInterner::STRING;
                }
                _ => {
                    kinds[0].declaration.end += 1;
                }
            }
            let mut changed = program.clone();
            changed.resource_manifest = ResourceManifest {
                data: Some(Arc::new(ManifestData {
                    original: original.clone(),
                    kinds,
                    hooks,
                })),
            };
            let errors =
                crate::validate_backend_types(&changed, &original.checked().interner).unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("Resource manifest differs")),
                "{errors:?}"
            );
        }
    }
}

#[test]
fn resource_manifest_refuses_foreign_refs_empty_manifest_and_source_packet_changes() {
    for release in [false, true] {
        let original = checked(release);
        let program = crate::lower_checked_resource_program(&original).unwrap();
        let foreign = crate::lower_checked_resource_program(&checked(release)).unwrap();
        for mutation in 0..6 {
            let mut changed = program.clone();
            if mutation == 0 {
                changed.resource_manifest = ResourceManifest::empty();
            } else {
                let call = invoke_mut(&mut changed);
                let E::ResourceInvoke {
                    hook,
                    evaluation_order,
                    args,
                    ownership: CallOwnership::Source(source),
                } = &mut call.kind
                else {
                    unreachable!()
                };
                match mutation {
                    1 => {
                        *hook = foreign
                            .resource_manifest
                            .hooks()
                            .find(|hook| hook.recipe() == ResourceKernelRecipe::NetworkFactory)
                            .unwrap();
                    }
                    2 => {
                        *evaluation_order = vec![0, 1];
                    }
                    3 => {
                        source.bridge = CallBridge::Direct;
                    }
                    4 => {
                        args[1].ty = TypeInterner::STRING;
                    }
                    _ => {
                        call.ty = TypeInterner::STRING;
                    }
                }
            }
            assert!(
                crate::validate_backend_types(&changed, &original.checked().interner).is_err(),
                "mutation {mutation}"
            );
        }
    }
}
