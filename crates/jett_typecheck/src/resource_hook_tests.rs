use std::collections::HashMap;

use jett_common::{FileId, STDLIB_FILE_ID_START, SourceOrigin};
use jett_diagnostics::Severity;
use jett_parser::{
    ast::{Item, Module},
    parse,
};
use jett_resolve::{
    DefId, ResolveResult, ResourceKernelSpec, resolve, resolve_with_resource_kernels,
};
use jett_types::{ResourceHookKind, ResourceKernelRecipe, Type, TypeInterner};

use crate::{
    CheckOptions, CheckResult, ResourceHookError, check, check_with_resource_kernels,
    validate_resource_hooks,
};

const SOURCE: &str = "namespace resource_probe\nresource TestHandle\ntype HandleAlias = TestHandle\nfunction checked_calls(view net: Network) returns nothing:\n    TestHandle first = kernel_create(view net, 1) handle error:\n        return nothing\n    int64 observed = kernel_borrow(view net, view first) handle error:\n        return nothing\n    kernel_close(first)\n    return nothing\nfunction alias_calls(view net: Network) returns nothing:\n    HandleAlias first = kernel_create(view net, 2) handle error:\n        return nothing\n    int64 observed = kernel_borrow(view net, view first) handle error:\n        return nothing\n    kernel_close(first)\n    return nothing\n";

fn fixture(source: &str) -> (Module, ResolveResult) {
    let file = FileId::new(STDLIB_FILE_ID_START);
    let parsed = parse(source, file);
    assert!(
        !parsed
            .errors
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "parse errors: {:?}",
        parsed.errors
    );
    let resource = parsed
        .module
        .items
        .iter()
        .find_map(|item| {
            if let Item::Resource(resource) = item {
                Some(resource)
            } else {
                None
            }
        })
        .unwrap();
    let span = resource.name.span;
    let specs = [
        ResourceKernelSpec {
            resource_declaration: span,
            member: "kernel_create".into(),
            recipe: ResourceKernelRecipe::NetworkFactory,
        },
        ResourceKernelSpec {
            resource_declaration: span,
            member: "kernel_borrow".into(),
            recipe: ResourceKernelRecipe::NetworkBorrow,
        },
        ResourceKernelSpec {
            resource_declaration: span,
            member: "kernel_close".into(),
            recipe: ResourceKernelRecipe::Finalize,
        },
    ];
    let resolved = resolve_with_resource_kernels(
        &parsed.module,
        &HashMap::from([(file, SourceOrigin::Stdlib)]),
        &specs,
    )
    .unwrap();
    assert!(
        !resolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "resolution errors: {:?}",
        resolved.diagnostics
    );
    (parsed.module, resolved)
}

fn accepted() -> (Module, ResolveResult, CheckResult) {
    let (module, resolved) = fixture(SOURCE);
    let checked = check_with_resource_kernels(&module, &resolved, CheckOptions::default()).unwrap();
    assert!(
        !checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "checking errors: {:?}",
        checked.diagnostics
    );
    (module, resolved, checked)
}

#[test]
fn checked_resource_kernel_calls_use_exact_declarations_modes_and_alias_identity() {
    let (module, resolved, checked) = accepted();
    validate_resource_hooks(&module, &resolved, &checked).unwrap();
    assert_eq!(checked.resource_hooks.len(), 3);
    let resource = checked
        .resource_hooks
        .values()
        .next()
        .unwrap()
        .resource_type;
    assert!(
        matches!(checked.interner.resolve(resource), Type::Resource(name) if name == "resource_probe.TestHandle")
    );
    let Item::Resource(declaration) = &module.items[1] else {
        panic!("missing resource")
    };
    let definition = resolved.resolutions[&declaration.name.span];
    assert_eq!(
        checked.definition_types[&definition], resource,
        "synthetic source attribution must not overwrite the resource span join"
    );
    assert_eq!(
        checked
            .reflection_metadata
            .get_type_info_for_id(resource)
            .unwrap()
            .kind,
        "resource"
    );
    assert!(
        checked
            .reflection_metadata
            .get_type_fields_for_id(resource)
            .is_none()
    );
    for entry in resolved.resource_kernels.iter() {
        let hook = &checked.resource_hooks[&entry.definition];
        assert_eq!(hook.definition, entry.definition);
        assert_eq!(hook.resource_definition, definition);
        assert_eq!(hook.resource_type, resource);
        assert_eq!(hook.kind, entry.recipe.kind());
        let Type::Function {
            params,
            view_params,
            return_type,
        } = checked.interner.resolve(hook.function_type)
        else {
            panic!("not a function")
        };
        match entry.recipe {
            ResourceKernelRecipe::NetworkFactory => {
                assert_eq!(params, &[TypeInterner::NETWORK, TypeInterner::INT64]);
                assert_eq!(view_params, &[true, false]);
                assert!(
                    matches!(checked.interner.resolve(*return_type), Type::Result(value, error)
                    if *value == resource && *error == TypeInterner::STRING)
                );
            }
            ResourceKernelRecipe::NetworkBorrow => {
                assert_eq!(params, &[TypeInterner::NETWORK, resource]);
                assert_eq!(view_params, &[true, true]);
                assert!(
                    matches!(checked.interner.resolve(*return_type), Type::Result(value, error)
                    if *value == TypeInterner::INT64 && *error == TypeInterner::STRING)
                );
            }
            ResourceKernelRecipe::Finalize => {
                assert_eq!(params, &[resource]);
                assert_eq!(view_params, &[false]);
                assert_eq!(*return_type, TypeInterner::NOTHING);
            }
        }
    }
    // These call type facts are produced by normal source checking, not a
    // private helper bypass. The alias function uses the same Resource ID.
    assert!(
        checked
            .type_map
            .values()
            .filter(|ty| **ty == resource)
            .count()
            >= 4
    );
    assert!(checked.intrinsic_ids.is_empty());
}

#[test]
fn resource_catalog_requires_explicit_checker_entry_and_empty_catalog_has_no_authority() {
    let (module, resolved) = fixture(SOURCE);
    let legacy = check(&module, &resolved);
    assert!(legacy.diagnostics.iter().any(|diagnostic| diagnostic.code.code() == 0 && diagnostic.severity == Severity::Error));
    assert!(legacy.resource_hooks.is_empty());
    let ordinary = resolve(&module);
    assert!(
        ordinary
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 200
                && diagnostic.severity == Severity::Error)
    );
    let checked = check(&module, &ordinary);
    assert!(checked.resource_hooks.is_empty());
}

#[test]
fn checked_resource_calls_preserve_ordinary_argument_type_and_owned_view_rejection() {
    for (source, expected) in [
        (
            SOURCE.replace(
                "kernel_create(view net, 1)",
                "kernel_create(view net, true)",
            ),
            304,
        ),
        (
            SOURCE.replace("kernel_close(first)", "kernel_close(view first)"),
            375,
        ),
    ] {
        let (module, resolved) = fixture(&source);
        let checked =
            check_with_resource_kernels(&module, &resolved, CheckOptions::default()).unwrap();
        assert!(
            checked
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.code() == expected
                    && diagnostic.severity == Severity::Error),
            "expected E{expected}, got {:?}",
            checked.diagnostics
        );
    }
}

#[test]
fn checked_resource_hook_metadata_rejects_unused_identity_corruption() {
    for corruption in 0..5 {
        let (module, resolved, mut checked) = accepted();
        let definition = resolved.resource_kernels.iter().last().unwrap().definition;
        let hook = checked.resource_hooks.get_mut(&definition).unwrap();
        match corruption {
            0 => hook.definition = DefId::new(u32::MAX),
            1 => hook.resource_definition = definition,
            2 => hook.resource_type = TypeInterner::INT64,
            3 => hook.kind = ResourceHookKind::Construct,
            _ => hook.function_type = TypeInterner::NOTHING,
        }
        assert!(validate_resource_hooks(&module, &resolved, &checked).is_err());
    }
    let (module, resolved, mut checked) = accepted();
    checked.resource_hooks.clear();
    assert_eq!(
        validate_resource_hooks(&module, &resolved, &checked),
        Err(ResourceHookError::UnexpectedCheckedHooks)
    );
}

#[test]
fn checked_resource_hook_function_metadata_requires_complete_exact_shape() {
    for corruption in 0..7 {
        let (module, resolved, mut checked) = accepted();
        let entry = resolved
            .resource_kernels
            .iter()
            .find(|entry| entry.recipe == ResourceKernelRecipe::NetworkBorrow)
            .unwrap();
        let original = checked.definition_types[&entry.definition];
        let Type::Function {
            mut params,
            mut view_params,
            mut return_type,
        } = checked.interner.resolve(original).clone()
        else {
            panic!("not a function")
        };
        match corruption {
            0 => params[1] = TypeInterner::INT64,
            1 => {
                params.pop();
            }
            2 => view_params[1] = false,
            3 => {
                view_params.pop();
            }
            4 => return_type = TypeInterner::INT64,
            5 => {
                return_type = checked
                    .interner
                    .intern(Type::Result(TypeInterner::INT64, TypeInterner::BOOL))
            }
            _ => params[0] = TypeInterner::FILESYSTEM,
        }
        let malformed = checked.interner.intern(Type::Function {
            params,
            view_params,
            return_type,
        });
        checked.definition_types.insert(entry.definition, malformed);
        checked
            .resource_hooks
            .get_mut(&entry.definition)
            .unwrap()
            .function_type = malformed;
        assert_eq!(
            validate_resource_hooks(&module, &resolved, &checked),
            Err(ResourceHookError::InvalidFunctionShape(entry.definition))
        );
    }
    let (module, resolved, mut checked) = accepted();
    let definition = resolved.resource_kernels.iter().next().unwrap().definition;
    checked.definition_types.remove(&definition);
    assert_eq!(
        validate_resource_hooks(&module, &resolved, &checked),
        Err(ResourceHookError::MissingFunctionType(definition))
    );
    let (module, resolved, mut checked) = accepted();
    let resource = resolved
        .resource_kernels
        .iter()
        .next()
        .unwrap()
        .resource_definition;
    checked
        .definition_types
        .insert(resource, TypeInterner::INT64);
    assert_eq!(
        validate_resource_hooks(&module, &resolved, &checked),
        Err(ResourceHookError::InvalidResourceType(resource))
    );

    let (module, resolved, mut checked) = accepted();
    let entry = resolved
        .resource_kernels
        .iter()
        .find(|entry| entry.recipe == ResourceKernelRecipe::NetworkBorrow)
        .unwrap();
    let mut foreign = TypeInterner::new();
    let mut foreign_id = TypeInterner::INT64;
    for _ in 0..checked.interner.len() + 1 {
        foreign_id = foreign.intern(Type::List(foreign_id));
    }
    assert!(foreign_id.index() as usize >= checked.interner.len());
    let malformed = checked.interner.intern(Type::Function {
        params: vec![
            TypeInterner::NETWORK,
            checked.resource_hooks[&entry.definition].resource_type,
        ],
        view_params: vec![true, true],
        return_type: foreign_id,
    });
    checked.definition_types.insert(entry.definition, malformed);
    checked
        .resource_hooks
        .get_mut(&entry.definition)
        .unwrap()
        .function_type = malformed;
    assert_eq!(
        validate_resource_hooks(&module, &resolved, &checked),
        Err(ResourceHookError::InvalidFunctionShape(entry.definition))
    );
}

#[test]
fn privileged_resource_entry_rejects_unused_resolver_corruption_and_checks_source_bodies() {
    let (module, mut resolved) = fixture("namespace resource_probe\nresource TestHandle\n");
    let definition = resolved.resource_kernels.iter().last().unwrap().definition;
    resolved.scope_table.definitions[definition.index() as usize].id = DefId::new(u32::MAX);
    assert!(matches!(
        check_with_resource_kernels(&module, &resolved, CheckOptions::default()),
        Err(ResourceHookError::Resolution(_))
    ));

    let (module, resolved) = fixture(&SOURCE.replace(
        "    kernel_close(first)",
        "    int64 broken = true\n    kernel_close(first)",
    ));
    let checked = check_with_resource_kernels(&module, &resolved, CheckOptions::default()).unwrap();
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 311
                && diagnostic.severity == Severity::Error),
        "source bodies must remain normally checked: {:?}",
        checked.diagnostics
    );
    assert_eq!(
        validate_resource_hooks(&module, &resolved, &checked),
        Err(ResourceHookError::SourceCheckingFailed),
        "diagnostic-bearing checker results must not pass the public phase gate"
    );
}

#[test]
fn unrelated_same_named_source_function_does_not_acquire_hook_identity() {
    let file = FileId::new(0);
    let parsed = parse(
        "function kernel_close(value: int64) returns nothing:\n    return nothing\nfunction main() returns nothing:\n    kernel_close(1)\n    return nothing\n",
        file,
    );
    assert!(parsed.errors.is_empty());
    let resolved = resolve(&parsed.module);
    let checked = check(&parsed.module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    assert!(checked.resource_hooks.is_empty());
}
