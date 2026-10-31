use std::collections::HashMap;

use jett_common::{FileId, STDLIB_FILE_ID_START, SourceOrigin, Span};
use jett_diagnostics::Severity;
use jett_parser::{
    ast::{Item, Module},
    parse,
};
use jett_types::ResourceKernelRecipe;

use crate::{
    DefId, DefKind, DefVisibility, ResourceKernelError, ResourceKernelSpec, resolve,
    resolve_with_resource_kernels, validate_resource_kernels,
};

fn parsed(source: &str, file: FileId) -> Module {
    let parsed = parse(source, file);
    assert!(
        !parsed
            .errors
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "parse errors: {:?}",
        parsed.errors
    );
    parsed.module
}

fn fixture() -> (
    Module,
    HashMap<FileId, SourceOrigin>,
    Vec<ResourceKernelSpec>,
) {
    let file = FileId::new(STDLIB_FILE_ID_START);
    let module = parsed(
        "namespace resource_probe\nexport resource TestHandle\n",
        file,
    );
    let Item::Resource(resource) = &module.items[1] else {
        panic!("missing resource")
    };
    let span = resource.name.span;
    let specs = vec![
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
    (module, HashMap::from([(file, SourceOrigin::Stdlib)]), specs)
}

#[test]
fn resource_kernels_are_real_private_definitions_without_source_span_replacement() {
    let (module, origins, specs) = fixture();
    let resolved = resolve_with_resource_kernels(&module, &origins, &specs).unwrap();
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != Severity::Error)
    );
    validate_resource_kernels(&module, &resolved).unwrap();
    let resource = resolved.resolutions[&specs[0].resource_declaration];
    assert_eq!(resolved.scope_table.def(resource).kind, DefKind::Resource);
    assert_eq!(resolved.resource_kernels.iter().count(), 3);
    for entry in resolved.resource_kernels.iter() {
        assert_ne!(entry.definition, resource);
        assert_eq!(entry.resource_definition, resource);
        let definition = resolved.scope_table.def(entry.definition);
        assert_eq!(definition.kind, DefKind::Function);
        assert_eq!(definition.visibility, DefVisibility::Private);
        assert_eq!(definition.namespace.as_deref(), Some("resource_probe"));
        assert_eq!(definition.span, specs[0].resource_declaration);
    }
    assert!(resolve(&module).resource_kernels.is_empty());
}

#[test]
fn resource_catalog_requires_independent_stdlib_origin_and_actual_resource() {
    let (module, origins, specs) = fixture();
    let file = specs[0].resource_declaration.file;
    for origin in [
        SourceOrigin::Project,
        SourceOrigin::Dependency("dependency".into()),
    ] {
        assert!(matches!(
            resolve_with_resource_kernels(&module, &HashMap::from([(file, origin)]), &specs),
            Err(ResourceKernelError::UntrustedOrigin(_))
        ));
    }
    assert!(matches!(
        resolve_with_resource_kernels(&module, &HashMap::new(), &specs),
        Err(ResourceKernelError::UntrustedOrigin(_))
    ));
    let project = parsed(
        "namespace resource_probe\nresource TestHandle\n",
        FileId::new(0),
    );
    let Item::Resource(resource) = &project.items[1] else {
        panic!("missing resource")
    };
    let mut project_specs = specs.clone();
    for spec in &mut project_specs {
        spec.resource_declaration = resource.name.span;
    }
    assert!(matches!(
        resolve_with_resource_kernels(
            &project,
            &HashMap::from([(FileId::new(0), SourceOrigin::Stdlib)]),
            &project_specs
        ),
        Err(ResourceKernelError::UntrustedOrigin(_))
    ));
    let mut missing = specs.clone();
    missing[0].resource_declaration = Span::new(file, 0, 1);
    assert!(matches!(
        resolve_with_resource_kernels(&module, &origins, &missing),
        Err(ResourceKernelError::MissingResource(_))
    ));
    let alias = parsed("namespace resource_probe\ntype TestHandle = int64\n", file);
    let Item::TypeAlias(alias_declaration) = &alias.items[1] else {
        panic!("missing alias")
    };
    missing[0].resource_declaration = alias_declaration.name.span;
    assert!(matches!(
        resolve_with_resource_kernels(&alias, &origins, &missing[..1]),
        Err(ResourceKernelError::MissingResource(_))
    ));
}

#[test]
fn resource_catalog_rejects_ambiguous_members_and_unnamespaced_declarations() {
    let (module, origins, specs) = fixture();
    let duplicate = vec![specs[0].clone(), specs[0].clone()];
    assert!(matches!(
        resolve_with_resource_kernels(&module, &origins, &duplicate),
        Err(ResourceKernelError::DuplicateCatalogEntry(_))
    ));
    for member in ["", "other.kernel", "1kernel", "kernel-name"] {
        let mut invalid = specs[0].clone();
        invalid.member = member.into();
        assert!(matches!(
            resolve_with_resource_kernels(&module, &origins, &[invalid]),
            Err(ResourceKernelError::InvalidMember(_))
        ));
    }
    let bare = parsed("resource TestHandle\n", specs[0].resource_declaration.file);
    let Item::Resource(resource) = &bare.items[0] else {
        panic!("missing resource")
    };
    let mut spec = specs[0].clone();
    spec.resource_declaration = resource.name.span;
    assert!(matches!(
        resolve_with_resource_kernels(&bare, &origins, &[spec]),
        Err(ResourceKernelError::MissingNamespace(_))
    ));
}

#[test]
fn resource_kernel_calls_preserve_order_private_access_and_source_duplicate_errors() {
    let file = FileId::new(STDLIB_FILE_ID_START);
    let sources = [
        "namespace resource_probe\nfunction early(view net: Network) returns nothing:\n    kernel_create(view net, 1) handle error:\n        return nothing\n    return nothing\nresource TestHandle\n",
        "namespace resource_probe\nresource TestHandle\nfunction kernel_create() returns nothing:\n    return nothing\n",
    ];
    for (source, code) in sources.into_iter().zip([205, 204]) {
        let module = parsed(source, file);
        let resource = module
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
        let spec = ResourceKernelSpec {
            resource_declaration: resource.name.span,
            member: "kernel_create".into(),
            recipe: ResourceKernelRecipe::NetworkFactory,
        };
        let resolved = resolve_with_resource_kernels(
            &module,
            &HashMap::from([(file, SourceOrigin::Stdlib)]),
            &[spec],
        )
        .unwrap();
        assert!(
            resolved
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.code() == code
                    && diagnostic.severity == Severity::Error),
            "expected E{code}, got {:?}",
            resolved.diagnostics
        );
    }
    let (mut module, origins, specs) = fixture();
    module.items.extend(parsed("namespace caller\nfunction main(view net: Network) returns nothing:\n    use resource_probe\n    resource_probe.kernel_create(view net, 1) handle error:\n        return nothing\n    return nothing\n", FileId::new(0)).items);
    let resolved = resolve_with_resource_kernels(&module, &origins, &specs).unwrap();
    assert!(
        resolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 207
                && diagnostic.severity == Severity::Error),
        "expected private access error: {:?}",
        resolved.diagnostics
    );
}

#[test]
fn unused_resource_kernel_metadata_is_preflighted_without_panics() {
    let (module, origins, specs) = fixture();
    for corruption in 0..8 {
        let mut resolved = resolve_with_resource_kernels(&module, &origins, &specs).unwrap();
        let entry = resolved.resource_kernels.iter().last().unwrap().clone();
        let definition = &mut resolved.scope_table.definitions[entry.definition.index() as usize];
        match corruption {
            0 => definition.id = DefId::new(u32::MAX),
            1 => definition.kind = DefKind::Resource,
            2 => definition.visibility = DefVisibility::Public,
            3 => definition.namespace = Some("alternate".into()),
            4 => definition.name = "resource_probe.spoof".into(),
            5 => definition.span = Span::new(FileId::new(0), 0, 1),
            6 => {
                resolved
                    .resolutions
                    .insert(specs[0].resource_declaration, entry.definition);
            }
            _ => {
                resolved.scope_table.definitions[entry.resource_definition.index() as usize].kind =
                    DefKind::Struct;
            }
        }
        assert!(
            validate_resource_kernels(&module, &resolved).is_err(),
            "corruption {corruption} admitted"
        );
    }
}

#[test]
fn source_definition_before_resource_cannot_become_a_catalog_hook() {
    let file = FileId::new(STDLIB_FILE_ID_START);
    let module = parsed(
        "namespace resource_probe\nfunction kernel_create() returns nothing:\n    return nothing\nresource TestHandle\n",
        file,
    );
    let Item::Resource(resource) = &module.items[2] else {
        panic!("missing resource")
    };
    let spec = ResourceKernelSpec {
        resource_declaration: resource.name.span,
        member: "kernel_create".into(),
        recipe: ResourceKernelRecipe::NetworkFactory,
    };
    let resolved = resolve_with_resource_kernels(
        &module,
        &HashMap::from([(file, SourceOrigin::Stdlib)]),
        &[spec],
    )
    .unwrap();
    assert!(
        resolved
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code() == 204
                && diagnostic.severity == Severity::Error)
    );
    assert!(matches!(
        validate_resource_kernels(&module, &resolved),
        Err(ResourceKernelError::UnresolvedResource(_))
    ));
}
