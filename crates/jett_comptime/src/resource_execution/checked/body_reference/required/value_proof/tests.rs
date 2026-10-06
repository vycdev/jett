use super::*;
use crate::checked_types::CheckedExpressionTypes;
use crate::explicit::ComptimeContext;
use crate::resource_execution::tests::program as checked_program;
use jett_common::{FileId, SourceOrigin};
use jett_diagnostics::Severity;
use jett_parser::parse;
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::CheckOptions;
use jett_types::{ResourceHookKind, ResourceKernelRecipe};

const SOURCE: &str = include_str!("../../../../fixtures/26_original_required_regions.jett");
const GENERIC_SCOPED: &str =
    include_str!("../../../../fixtures/27_original_generic_scoped_required.jett");
const SHARED_SUPPORT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jett_driver/tests/native_conformance/resource/resource_probe.jett"
));

fn required(program: &Arc<CheckedResourceProgram>) -> crate::ExplicitComptimeEvaluation {
    crate::evaluate_explicit_comptime_expressions_capture(
        program.module(),
        Arc::new(jett_types::ReflectionMetadata::new()),
        Arc::new(CheckedExpressionTypes {
            resource_program: Some(program.clone()),
            expressions: program
                .checked()
                .type_map
                .iter()
                .map(|(span, ty)| (*span, program.checked().interner.type_name(*ty)))
                .collect(),
            ..Default::default()
        }),
        Arc::new(HashMap::new()),
    )
}

fn successful(program: &Arc<CheckedResourceProgram>) -> ExplicitComptimeValues {
    let evaluated = required(program);
    assert!(
        evaluated.diagnostics.is_empty(),
        "{:?}",
        evaluated.diagnostics
    );
    assert!(evaluated.debug_events.is_empty());
    evaluated.values
}

fn shared_program(source: &str, release: bool) -> Arc<CheckedResourceProgram> {
    let stdlib = FileId::new(10_000);
    let project = FileId::new(0);
    let mut parsed = parse(SHARED_SUPPORT, stdlib);
    let application = parse(source, project);
    assert!(
        !parsed
            .errors
            .iter()
            .chain(&application.errors)
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    );
    let resource = parsed
        .module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Resource(value) => Some(value.name.span),
            _ => None,
        })
        .unwrap();
    parsed.module.items.extend(application.module.items);
    parsed.errors.extend(application.errors);
    let kernels = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: resource,
        member: member.to_string(),
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
            &kernels,
            CheckOptions { release },
        )
        .unwrap_or_else(|error| panic!("{error:?}; {:?}", error.diagnostics())),
    )
}

#[test]
fn resource_required_value_proof_keeps_outer_node_owner_primitive_absence_and_hook() {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let cache = successful(&program);
        let values = cache.checked_required_values(&program).unwrap();
        assert_eq!(values.len(), 5);
        for value in &values {
            assert!(value.belongs_to(&program));
            assert!(value.is_explicit_comptime());
            let original = value.original_comptime().unwrap();
            assert!(matches!(original, Expr::Comptime(_, _)));
            assert_eq!(value.source_span(), original.span());
            assert!(value.matches_original_comptime(original));
            assert!(!value.matches_original_comptime(&original.clone()));
            assert_eq!(value.generic_index(), None);
            assert!(value.type_arguments().is_empty());
            assert_eq!(
                value.specialization(),
                &CheckedGenericSpecialization::default()
            );
            assert!(value.scoped_bindings().is_empty());
        }
        assert_eq!(
            values
                .iter()
                .filter(|value| value.value() == &Value::Int64(7))
                .count(),
            3
        );
        assert_eq!(
            values
                .iter()
                .filter(|value| value.value() == &Value::OptionalNone)
                .count(),
            1
        );
        assert_eq!(
            values
                .iter()
                .filter(|value| matches!(
                    value.owner(),
                    CheckedRequiredOwner::NamespaceConstant { .. }
                ))
                .count(),
            1
        );
        assert_eq!(
            values
                .iter()
                .filter(|value| matches!(value.owner(), CheckedRequiredOwner::Verify { .. }))
                .count(),
            1
        );
        assert_eq!(
            values
                .iter()
                .filter(|value| matches!(value.owner(), CheckedRequiredOwner::Property { .. }))
                .count(),
            1
        );
        let hooks = cache.checked_resource_hook_values(&program).unwrap();
        assert_eq!(hooks.len(), 1);
        let hook = &hooks[0];
        assert_eq!(
            hook.checked_hook(&program).unwrap().kind,
            ResourceHookKind::Close
        );
        assert!(matches!(hook.value().payload(), Value::ResourceHook(_)));
        let function = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(value) if value.name.name == "scenario" => Some(value),
                _ => None,
            })
            .unwrap();
        let checked =
            CheckedExecution::new(program.clone(), ExecutionPurpose::ExplicitComptime).unwrap();
        assert_eq!(
            hook.owner(),
            CheckedRequiredOwner::Function {
                definition: checked.declaration_definition(function.name.span).unwrap(),
                declaration: function.name.span,
            }
        );
        assert!(hook.matches_original_comptime(hook.original_comptime().unwrap()));
        assert!(!hook.matches_original_comptime(&hook.original_comptime().unwrap().clone()));
    }
}

#[test]
fn resource_required_value_proof_distinguishes_raw_namespace_initializer() {
    for release in [false, true] {
        let source = SOURCE.replace("int64 baked = comptime seven()", "int64 baked = 7");
        let program = checked_program(&source, release);
        let cache = successful(&program);
        let values = cache.checked_required_values(&program).unwrap();
        let constant = values
            .iter()
            .find(|value| {
                matches!(
                    value.owner(),
                    CheckedRequiredOwner::NamespaceConstant { .. }
                )
            })
            .unwrap();
        let declaration = program
            .module()
            .items
            .iter()
            .find_map(|item| match item {
                Item::VarDecl(value) => Some(value),
                _ => None,
            })
            .unwrap();
        assert_eq!(constant.value(), &Value::Int64(7));
        assert_eq!(
            constant.owner(),
            CheckedRequiredOwner::NamespaceConstant {
                declaration: declaration.name.span
            }
        );
        assert_eq!(constant.source_span(), declaration.value.span());
        assert!(!constant.is_explicit_comptime());
        assert!(constant.original_comptime().is_err());
        assert!(!constant.matches_original_comptime(&declaration.value));
        assert!(constant.checked_hook(&program).unwrap().is_none());
        assert!(
            values
                .iter()
                .filter(|value| value.is_explicit_comptime())
                .all(|value| value.original_comptime().is_ok())
        );
    }
}

#[test]
fn resource_required_value_proof_refuses_foreign_program_and_public_mirror_replacement() {
    for release in [false, true] {
        let program = checked_program(SOURCE, release);
        let foreign = checked_program(SOURCE, release);
        let mut cache = successful(&program);
        let hooks = cache.checked_resource_hook_values(&program).unwrap();
        let hook = &hooks[0];
        let checked_hook = hook.checked_hook(&program).unwrap();
        // Same source produces the same numeric definition and signature, but
        // those observations cannot authenticate an independent program Arc.
        assert_eq!(
            foreign
                .checked()
                .resource_hooks
                .get(&checked_hook.definition),
            Some(checked_hook)
        );
        assert!(!hook.belongs_to(&foreign));
        assert!(hook.checked_hook(&foreign).is_err());
        assert!(cache.checked_required_values(&foreign).is_err());
        assert!(cache.checked_resource_hook_values(&foreign).is_err());
        let foreign_cache = successful(&foreign);
        let foreign_hook = foreign_cache
            .checked_resource_hook_values(&foreign)
            .unwrap()
            .remove(0);
        assert_eq!(foreign_hook.source_span(), hook.source_span());
        assert!(!hook.matches_original_comptime(foreign_hook.original_comptime().unwrap()));
        assert!(foreign_hook.checked_hook(&program).is_err());
        cache.insert(
            hook.source_span(),
            ComptimeContext::default(),
            Value::Int64(99),
        );
        assert_eq!(
            cache.get(hook.source_span(), &ComptimeContext::default()),
            Some(&Value::Int64(99))
        );
        let retained = cache
            .checked_resource_hook_values(&program)
            .unwrap()
            .remove(0);
        assert_eq!(retained.checked_hook(&program).unwrap(), checked_hook);
        assert!(matches!(retained.value().payload(), Value::ResourceHook(_)));
        let mut mirror = ExplicitComptimeValues::default();
        mirror.insert(
            hook.source_span(),
            ComptimeContext::default(),
            hook.value().clone(),
        );
        assert_eq!(mirror.len(), 1);
        assert!(mirror.checked_required_values(&program).unwrap().is_empty());
        assert!(
            mirror
                .checked_resource_hook_values(&program)
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn resource_required_value_proof_preserves_complete_generic_and_nested_scoped_context() {
    for release in [false, true] {
        let program = checked_program(GENERIC_SCOPED, release);
        let cache = successful(&program);
        let values = cache.checked_required_values(&program).unwrap();
        assert_eq!(values.len(), 2);
        let generic = values
            .iter()
            .find(|value| value.generic_index().is_some())
            .unwrap();
        let instance =
            &program.checked().generic_function_instantiations[generic.generic_index().unwrap()];
        assert_eq!(generic.type_arguments(), instance.concrete_args);
        assert_eq!(generic.specialization(), &instance.specialization);
        assert!(
            matches!(generic.owner(), CheckedRequiredOwner::Function { definition, .. } if definition == instance.definition)
        );
        assert_eq!(generic.value(), &Value::String("int64".to_string()));
        let mut changed_generic = generic.key.clone();
        changed_generic.node.generic = None;
        assert!(CheckedRequiredValue::from_cache(&cache, &changed_generic, &program).is_err());
        assert!(changed_generic.value_site().is_err());
        let scoped = values
            .iter()
            .find(|value| value.scoped_bindings().len() == 2)
            .unwrap();
        assert_eq!(
            scoped
                .scoped_bindings()
                .iter()
                .map(CheckedRequiredScope::name)
                .collect::<Vec<_>>(),
            ["Outer", "Element"]
        );
        let mut bindings = &program.checked().comptime_type_bindings;
        for scope in scoped.scoped_bindings() {
            let selected = &bindings[&scope.owner()][scope.index()];
            assert_eq!(scope.bound_type(), selected.bound_type);
            assert_eq!(scope.selection(), selected.selection);
            assert_eq!(scope.reflection(), &selected.reflection);
            bindings = &selected.body.comptime_type_bindings;
        }
        assert_eq!(scoped.value(), &Value::String("int64".to_string()));
        let mut changed_scope = scoped.key.clone();
        changed_scope.node.scopes[1].3 = Some(0);
        assert!(CheckedRequiredValue::from_cache(&cache, &changed_scope, &program).is_err());
        assert!(changed_scope.value_site().is_err());
        let mut changed_owner = scoped.key.clone();
        changed_owner.node.origin = generic.key.node.origin;
        assert!(CheckedRequiredValue::from_cache(&cache, &changed_owner, &program).is_err());
        assert!(changed_owner.value_site().is_err());
        let mut changed_occurrence = scoped.key.clone();
        let Some(RegionStep::Expression(index)) = changed_occurrence.node.path.last_mut() else {
            panic!("required expression path");
        };
        *index += 1;
        assert!(CheckedRequiredValue::from_cache(&cache, &changed_occurrence, &program).is_err());
    }
}

#[test]
fn resource_required_value_proof_covers_actual_all_recipe_alias_relay_unused_and_pure_sources() {
    let sources = [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../jett_driver/tests/native_conformance/resource/33_comptime_hook_close.jett"
            )),
            Some(ResourceHookKind::Close),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../jett_driver/tests/native_conformance/resource/34_comptime_hook_factory.jett"
            )),
            Some(ResourceHookKind::Construct),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../jett_driver/tests/native_conformance/resource/35_comptime_hook_borrow.jett"
            )),
            Some(ResourceHookKind::BorrowOperation),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../jett_driver/tests/native_conformance/resource/36_comptime_hook_alias.jett"
            )),
            Some(ResourceHookKind::Close),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../jett_driver/tests/native_conformance/resource/37_comptime_hook_relay.jett"
            )),
            Some(ResourceHookKind::Close),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../jett_driver/tests/native_conformance/resource/38_comptime_hook_unused.jett"
            )),
            Some(ResourceHookKind::Close),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../jett_driver/tests/native_conformance/resource/39_immediate_comptime_hook_close.jett"
            )),
            Some(ResourceHookKind::Close),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../jett_driver/tests/native_conformance/resource/40_comptime_hook_absence.jett"
            )),
            None,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../jett_driver/tests/native_conformance/resource/41_comptime_hook_pure_choice.jett"
            )),
            Some(ResourceHookKind::Close),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../jett_driver/tests/native_conformance/resource/42_comptime_hook_generic.jett"
            )),
            Some(ResourceHookKind::Close),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../jett_driver/tests/native_conformance/resource/43_comptime_hook_scoped.jett"
            )),
            Some(ResourceHookKind::Close),
        ),
    ];
    for release in [false, true] {
        for (source, expected) in sources {
            let program = shared_program(source, release);
            let cache = successful(&program);
            let values = cache.checked_required_values(&program).unwrap();
            assert_eq!(values.len(), 1);
            assert!(values[0].matches_original_comptime(values[0].original_comptime().unwrap()));
            let hooks = cache.checked_resource_hook_values(&program).unwrap();
            match expected {
                Some(kind) => {
                    assert_eq!(hooks.len(), 1);
                    assert_eq!(hooks[0].checked_hook(&program).unwrap().kind, kind);
                    assert_eq!(
                        values[0].checked_hook(&program).unwrap(),
                        Some(hooks[0].checked_hook(&program).unwrap())
                    );
                }
                None => {
                    assert!(hooks.is_empty());
                    assert_eq!(values[0].value(), &Value::OptionalNone);
                    assert!(values[0].checked_hook(&program).unwrap().is_none());
                }
            }
        }
    }
}

#[test]
fn resource_required_value_proof_cannot_mint_without_successful_checked_evaluation() {
    const FAILED: &str = "namespace app\nexport function scenario() returns nothing:\n    use resource_probe\n    nothing failed = comptime resource_probe.terminal_failure()\n    return nothing\n";
    for release in [false, true] {
        let program = checked_program(FAILED, release);
        let evaluated = required(&program);
        assert!(!evaluated.diagnostics.is_empty());
        assert!(
            evaluated
                .values
                .checked_required_values(&program)
                .unwrap()
                .is_empty()
        );
        assert!(
            evaluated
                .values
                .checked_resource_hook_values(&program)
                .unwrap()
                .is_empty()
        );
    }
}
