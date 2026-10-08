use super::*;
use crate::resource_ownership::tests::{checked, checked_support, lowered};
use jett_common::{FileId, SourceOrigin};
use jett_comptime::checked_types::CheckedExpressionTypes;
use jett_parser::parse;
use jett_typecheck::CheckedResourceProgram;
use std::{collections::HashMap, sync::Arc};

include!(
    "../../../../jett_codegen_cranelift/src/emit/resource_execution/tests/carrier_wrapper_sources.rs"
);

const WHOLE_SOURCE: &str = include_str!("../fixtures/17_absent_aggregate_shapes.jett");
const FIXTURE_SUPPORT: &str =
    include_str!("../../../../jett_driver/tests/native_conformance/resource/resource_probe.jett");
const PURE: &str = r#"namespace app
function helper() returns int64:
    return 7
function ordinary_probe(view net: Network) returns nothing:
    int64 answer = helper()
    if answer != 7:
        return nothing
    return nothing
    int64 dead = 9
    return nothing
function peer(view net: Network) returns nothing:
    return nothing
"#;

fn named<'a>(program: &'a Program, name: &str) -> &'a Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == name
        })
        .unwrap_or_else(|| panic!("checked app.{name}"))
}

fn named_mut<'a>(program: &'a mut Program, name: &str) -> &'a mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == name
        })
        .unwrap_or_else(|| panic!("checked app.{name}"))
}

fn materialized(checked: &Arc<CheckedResourceProgram>) -> Program {
    let expression_types = Arc::new(CheckedExpressionTypes {
        resource_program: Some(checked.clone()),
        expressions: checked
            .checked()
            .type_map
            .iter()
            .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
            .collect(),
        ..Default::default()
    });
    let required = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        Arc::new(jett_types::ReflectionMetadata::new()),
        expression_types,
        Arc::new(HashMap::new()),
    );
    assert!(
        required.diagnostics.is_empty(),
        "{:?}",
        required.diagnostics
    );
    assert!(required.debug_events.is_empty());
    required.values.checked_required_values(checked).unwrap();
    assert!(!required.values.is_empty());
    let types = &checked.checked().interner;
    let mut hir = hir::lower_checked_resource_program(checked).unwrap();
    hir::materialize_checked_required_values(&mut hir, &required.values, types).unwrap();
    hir::complete_value_conversions(&mut hir, types).unwrap();
    crate::lower(&hir, types).unwrap()
}

fn prepare(program: &mut Program, types: &TypeInterner) {
    crate::prepare_native_sequences(program, types);
    crate::prepare_native_uninhabited_sums(program, types);
    crate::prepare_native_generated_functions(program, types);
}

fn assert_bridge_metadata(program: &Program, types: &TypeInterner, name: &str) {
    let selected = named(program, name);
    let ordinary = validate_resource_ownership(program, types).unwrap();
    assert!(ordinary.entry_scope().is_none());
    assert!(ordinary.function(selected.id).is_none());
    let ownership = validate_resource_ownership_for_entry(program, types, selected.id).unwrap();
    let bridge = ownership
        .entry_scope()
        .expect("selected wrapper-only Scope");
    assert_eq!(bridge.function(), selected.id);
    assert_eq!(bridge.identity(), &selected.identity);
    assert_eq!(bridge.parameters(), selected.params.as_slice());
    assert_eq!(bridge.return_type(), TypeInterner::NOTHING);
    assert_eq!(bridge.root_scope().role(), ResourceFrameRole::Scope);
    assert_eq!(bridge.root_scope().parent(), None);
    assert_eq!(bridge.root_scope().site().function(), selected.id);
    assert_eq!(bridge.root_scope().site().block(), selected.entry);
    assert_eq!(
        bridge.root_scope().site().position(),
        ResourcePosition::Terminator
    );
    assert_eq!(bridge.completion().frame(), bridge.root_scope().id());
    assert_eq!(bridge.completion().site(), bridge.root_scope().site());
    assert!(matches!(
        bridge.completion().role(),
        ResourceOperationRole::Complete { .. }
    ));
    assert!(!bridge.completion().is_expression_operation());
    assert!(bridge.completion().named_indirect_target().is_none());
    assert!(bridge.completion().indirect_hook_target().is_none());
    assert!(ownership.function(selected.id).is_none());
    assert!(
        !ownership
            .functions()
            .iter()
            .any(|function| function.function() == selected.id)
    );
    assert!(ownership.original_source(selected.id).is_some());
    assert!(!has_execution_records(selected, types));
    validate_call_ownership(program, types).unwrap();
}

fn assert_bridge(program: &Program, types: &TypeInterner, name: &str) {
    assert_bridge_metadata(program, types, name);
    let selected = named(program, name);
    let copy =
        crate::copy_values::CopyValuePlan::analyze_storage(selected, types, Some(program)).unwrap();
    let storage = crate::move_values::MoveValuePlan::analyze(program, selected, types).unwrap();
    assert_eq!(copy.owned_locals, storage.owned_locals);
    validate_call_ownership(program, types).unwrap();
}

fn reseal_general_cache(function: &mut Function) {
    let parameters = function.params.clone();
    let locals = function.locals.clone();
    let blocks = function.blocks.clone();
    let entry = function.entry;
    let witness = function.resource_lowering.as_mut().unwrap();
    witness.parameters = parameters;
    witness.locals = locals;
    witness.blocks = blocks;
    witness.entry = entry;
}

fn assert_refused_before_prune(
    mut program: Program,
    types: &TypeInterner,
    name: &str,
    locally_exact_foreign: bool,
) {
    let selected = named(&program, name).id;
    assert!(validate_resource_ownership_for_entry(&program, types, selected).is_err());
    let before = program.clone();
    crate::prepare_native_sequences(&mut program, types);
    assert!(
        program == before,
        "refused canonical pruning must preserve the graph"
    );
    assert!(validate_resource_ownership_for_entry(&program, types, selected).is_err());
    if named(&program, name).resource_lowering.is_some() {
        let function = named(&program, name);
        let witness = function.resource_lowering.as_ref().unwrap();
        assert_eq!(witness.current(function).is_ok(), locally_exact_foreign);
        if locally_exact_foreign {
            // A sibling's changed Source makes the whole archive foreign even
            // when this function's local body is still exactly authenticated.
            let original = witness.original.clone();
            let source = witness.source.clone();
            assert!(function.locals.iter().any(|local| local.name == "dead"));
            assert!(crate::sequences::prune::unreachable(named_mut(
                &mut program,
                name
            )));
            let function = named(&program, name);
            assert!(!function.locals.iter().any(|local| local.name == "dead"));
            let witness = function.resource_lowering.as_ref().unwrap();
            witness.current(function).unwrap();
            assert!(original_function_equal(&witness.original, &original));
            assert!(witness.source == source);
            let errors = match validate_resource_ownership(&program, types) {
                Err(errors) => errors,
                Ok(_) => panic!("local pruning admitted a foreign Source archive"),
            };
            assert!(
                errors.iter().any(|error| error
                    .message
                    .contains("mixes foreign original/materialized archives")),
                "{errors:?}"
            );
        } else {
            assert!(!crate::sequences::prune::unreachable(named_mut(
                &mut program,
                name
            )));
            assert!(program == before);
        }
    }
    assert!(validate_resource_ownership_for_entry(&program, types, selected).is_err());
}

#[test]
fn ordinary_entry_pure_checked_candidates_have_initial_body_seals_without_comptime_or_name_authority()
 {
    for release in [false, true] {
        let checked = checked(PURE, release);
        let types = &checked.checked().interner;
        let hir = hir::lower_checked_resource_program(&checked).unwrap();
        assert!(hir.resource_source.required_materializations().is_empty());
        let mut has_comptime = false;
        for function in hir
            .functions
            .iter()
            .filter(|function| function.identity.declaration.namespace == "app")
        {
            walk::hir_block(&function.body, &mut |value| {
                has_comptime |= matches!(value.kind, hir::ExpressionKind::Comptime { .. });
            });
        }
        assert!(!has_comptime);
        let baseline = crate::lower(&hir, types).unwrap();
        for name in ["ordinary_probe", "peer"] {
            let function = named(&baseline, name);
            let witness = function
                .resource_lowering
                .as_ref()
                .expect("initial proof-only capture");
            assert!(witness.descriptors.has_body());
            witness.descriptors.current(function).unwrap();
            assert_bridge(&baseline, types, name);
        }
        assert!(
            validate_resource_ownership(&baseline, types)
                .unwrap()
                .function(named(&baseline, "helper").id)
                .is_none()
        );
        let probe = named(&baseline, "ordinary_probe");
        assert!(probe.locals.iter().any(|local| local.name == "dead"));
        let before_locals = probe.locals.len();
        let mut compacted = baseline.clone();
        prepare(&mut compacted, types);
        assert!(named(&compacted, "ordinary_probe").locals.len() < before_locals);
        assert_bridge(&compacted, types, "ordinary_probe");
        assert_bridge(&compacted, types, "peer");
    }
}

#[test]
fn ordinary_entry_default_archive_hir_remains_ordinary_and_cannot_mint_an_entry_scope() {
    let source = "namespace app\nfunction ordinary_probe(view net: Network) returns nothing:\n    return nothing\n";
    let file = FileId::new(0);
    let parsed = parse(source, file);
    assert!(parsed.errors.is_empty());
    let resolved = jett_resolve::resolve(&parsed.module);
    let checked = jett_typecheck::check(&parsed.module, &resolved);
    assert!(
        checked
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity != jett_diagnostics::Severity::Error),
        "{:?}",
        checked.diagnostics
    );
    let origins = HashMap::from([(file, SourceOrigin::Project)]);
    let hir = hir::lower(&parsed.module, &resolved, &checked, &origins).unwrap();
    assert!(hir.resource_source.manifest().is_none());
    let program = crate::lower(&hir, &checked.interner).unwrap();
    let selected = named(&program, "ordinary_probe");
    assert!(selected.resource_lowering.is_none());
    let ordinary = validate_resource_ownership(&program, &checked.interner).unwrap();
    assert!(ordinary.function(selected.id).is_none());
    assert!(ordinary.entry_scope().is_none());
    assert!(
        validate_resource_ownership_for_entry(&program, &checked.interner, selected.id).is_err()
    );
}

#[test]
fn ordinary_entry_consuming_list_for_and_later_ordinary_caller_preserve_the_initial_proof() {
    const SOURCE: &str = r#"namespace app
function main() returns nothing:
    list[int64] values = list(1, 2)
    mutable int64 total = 0
    for number in values:
        total = total + number
    return nothing
function reenter() returns nothing:
    main()
    return nothing
"#;
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let mut program = lowered(&checked);
        let main = named(&program, "main");
        let selected = main.id;
        assert!(
            main.resource_lowering
                .as_ref()
                .unwrap()
                .descriptors
                .has_body()
        );
        assert!(
            main.blocks
                .iter()
                .any(|block| matches!(block.terminator.kind, TerminatorKind::ForEach { .. }))
        );
        let initial = validate_resource_ownership_for_entry(&program, types, selected).unwrap();
        assert!(initial.entry_scope().is_some());
        assert!(initial.function(selected).is_none());
        assert!(initial.function(named(&program, "reenter").id).is_none());
        prepare(&mut program, types);
        assert!(
            !named(&program, "main")
                .blocks
                .iter()
                .any(|block| matches!(block.terminator.kind, TerminatorKind::ForEach { .. }))
        );
        assert_bridge(&program, types, "main");
        assert_bridge(&program, types, "reenter");
        let main = named(&program, "main");
        let storage = crate::move_values::MoveValuePlan::analyze(&program, main, types).unwrap();
        let source = main
            .blocks
            .iter()
            .flat_map(|block| &block.statements)
            .find_map(|statement| {
                if let StatementKind::SequenceGet {
                    consume: true,
                    source,
                    ..
                } = &statement.kind
                {
                    Some(source.root())
                } else {
                    None
                }
            })
            .expect("actual canonical consuming-list element extraction");
        assert!(
            matches!(types.resolve(main.local(source).unwrap().ty), Type::List(inner) if *inner == TypeInterner::INT64)
        );
        assert!(storage.owned_locals.contains(&(source.index() as usize)));
    }
}

#[test]
fn ordinary_entry_source17_wrapper14_keeps_required_helper_and_main_ordinary_before_and_after_preparation()
 {
    assert_eq!(WRAPPERS.len(), 14);
    let (name, suffix) = WRAPPERS[13];
    assert_eq!(name, "14_required_primitive_controls");
    let source = format!("{WHOLE_SOURCE}{suffix}");
    for release in [false, true] {
        let checked = checked_support(&source, FIXTURE_SUPPORT, release);
        let types = &checked.checked().interner;
        let baseline = materialized(&checked);
        for prepared in [false, true] {
            let mut program = baseline.clone();
            if prepared {
                prepare(&mut program, types);
            }
            assert_bridge(&program, types, "main");
            let main = named(&program, "main");
            let helper = named(&program, "absent_required_controls");
            let ownership =
                validate_resource_ownership_for_entry(&program, types, main.id).unwrap();
            assert!(ownership.function(helper.id).is_none());
            assert!(!has_execution_records(helper, types));
            crate::copy_values::CopyValuePlan::analyze(helper, types).unwrap();
            crate::move_values::MoveValuePlan::analyze(&program, helper, types).unwrap();
            let archived = ownership.original_source(helper.id).unwrap();
            let mut required = false;
            walk::hir_block(&archived.body, &mut |value| {
                required |= matches!(value.kind, hir::ExpressionKind::Comptime { .. });
            });
            assert!(
                required,
                "the complete original required-value helper remains archived"
            );
            for constructor in [
                "empty_tokens",
                "absent_tokens",
                "empty_map",
                "absent_map",
                "empty_result_tokens",
                "failed_result_tokens",
                "absent_envelope",
                "absent_box",
                "empty_choice",
                "absent_choice",
                "absent_state",
                "absent_exact_state",
            ] {
                named(&program, constructor);
            }
            named(&program, "refined_shape_contract");
        }
        let ownership = validate_resource_ownership(&baseline, types).unwrap();
        assert!(!ownership.required_only_function_ids().is_empty());
        for &required_only in ownership.required_only_function_ids() {
            let errors =
                validate_resource_ownership_for_entry(&baseline, types, required_only).unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("required-only"))
            );
        }
    }
}

#[test]
fn ordinary_entry_missing_foreign_or_copied_witness_refuses_before_canonical_pruning() {
    for release in [false, true] {
        let checked = checked(PURE, release);
        let types = &checked.checked().interner;
        let baseline = lowered(&checked);
        let foreign = lowered(&self::checked(
            &PURE.replace("return 7", "return 8"),
            release,
        ));
        for mutation in 0..4 {
            let mut changed = baseline.clone();
            let replacement = match mutation {
                0 => None,
                1 => named(&foreign, "ordinary_probe").resource_lowering.clone(),
                2 => named(&baseline, "peer").resource_lowering.clone(),
                3 => None,
                _ => unreachable!(),
            };
            named_mut(&mut changed, "ordinary_probe").resource_lowering = replacement;
            if mutation == 3 {
                named_mut(&mut changed, "ordinary_probe").capture_count = 1;
                crate::validate(&changed).unwrap();
                let errors = match validate_resource_ownership(&changed, types) {
                    Err(errors) => errors,
                    Ok(_) => panic!("missing original entry witness was accepted"),
                };
                assert!(
                    errors
                        .iter()
                        .any(|error| error.message.contains("constructor witness")),
                    "{:?}",
                    errors
                );
            }
            assert_refused_before_prune(changed, types, "ordinary_probe", mutation == 1);
        }
    }
}

#[test]
fn ordinary_entry_changed_headers_and_dense_identity_cannot_be_resealed_into_authority() {
    for release in [false, true] {
        let checked = checked(PURE, release);
        let types = &checked.checked().interner;
        let baseline = lowered(&checked);
        let selected = named(&baseline, "ordinary_probe").id;
        for mutation in 0..7 {
            let mut changed = baseline.clone();
            let function = named_mut(&mut changed, "ordinary_probe");
            match mutation {
                0 => function.params[0].mode = ParamMode::Owned,
                1 => function.params[0].mutable = true,
                2 => function.params[0].ty = TypeInterner::INT64,
                3 => function.return_type = TypeInterner::INT64,
                4 => function.capture_count = 1,
                5 => function.identity.declaration.name = "replacement".into(),
                6 => function.id = named(&baseline, "peer").id,
                _ => unreachable!(),
            }
            reseal_general_cache(function);
            assert!(
                validate_resource_ownership_for_entry(&changed, types, selected).is_err(),
                "mutation={mutation}; release={release}"
            );
            let before = changed.clone();
            let function = changed
                .functions
                .iter_mut()
                .find(|function| {
                    function.identity.declaration.name
                        == if mutation == 5 {
                            "replacement"
                        } else {
                            "ordinary_probe"
                        }
                })
                .unwrap();
            assert!(!crate::sequences::prune::unreachable(function));
            assert!(changed == before);
        }
        assert!(
            validate_resource_ownership_for_entry(&baseline, types, FunctionId::new(u32::MAX))
                .is_err()
        );
    }
}

#[test]
fn ordinary_entry_independent_body_seal_refuses_resealed_whole_body_and_disconnected_replacement() {
    for release in [false, true] {
        let checked = checked(PURE, release);
        let types = &checked.checked().interner;
        let baseline = lowered(&checked);
        for mutation in 0..3 {
            let mut changed = baseline.clone();
            let function = named_mut(&mut changed, "ordinary_probe");
            match mutation {
                0 => {
                    let value = function
                        .blocks
                        .iter_mut()
                        .flat_map(|block| &mut block.statements)
                        .find_map(|statement| {
                            if let StatementKind::Let { value, .. } = &mut statement.kind {
                                Some(value)
                            } else {
                                None
                            }
                        })
                        .unwrap();
                    value.kind = hir::ExpressionKind::Int(99);
                }
                1 => function.blocks[usize::try_from(function.entry.index()).unwrap()]
                    .statements
                    .clear(),
                2 => {
                    let mut copied =
                        function.blocks[usize::try_from(function.entry.index()).unwrap()].clone();
                    copied.id = BlockId(u32::try_from(function.blocks.len()).unwrap());
                    copied.terminator.kind = TerminatorKind::Return(None);
                    function.blocks.push(copied);
                }
                _ => unreachable!(),
            }
            reseal_general_cache(function);
            assert_refused_before_prune(changed, types, "ordinary_probe", false);
        }
    }
}

#[test]
fn ordinary_entry_independent_graph_is_mandatory_even_when_general_cache_is_exact() {
    for release in [false, true] {
        let checked = checked(PURE, release);
        let types = &checked.checked().interner;
        let mut program = lowered(&checked);
        let function = named_mut(&mut program, "ordinary_probe");
        assert!(
            function
                .resource_lowering
                .as_ref()
                .unwrap()
                .descriptors
                .has_body()
        );
        function.resource_lowering.as_mut().unwrap().descriptors =
            returned_descriptors::DescriptorWitness::default();
        assert_refused_before_prune(program, types, "ordinary_probe", false);
    }
}

#[test]
fn ordinary_entry_execution_family_retains_its_existing_body_scope_without_an_extra_bridge() {
    const SOURCE: &str = include_str!("../fixtures/connected_entry.jett");
    for release in [false, true] {
        let checked = checked(SOURCE, release);
        let types = &checked.checked().interner;
        let program = lowered(&checked);
        let selected = named(&program, "main").id;
        let plan = validate_resource_ownership_for_entry(&program, types, selected).unwrap();
        assert!(plan.entry_scope().is_none());
        assert!(plan.function(selected).is_some());
    }
}

const RAW_REFINEMENTS: &str = r#"namespace app
type NonZeroInt = int64 where value != 0
type Positive = int64 where value > 0
function quotient(value: int64, divisor: NonZeroInt) returns int64:
    return value / divisor
function pure_entry() returns nothing:
    list[int64] values = list(1, 2)
    mutable int64 total = 0
    for number in values:
        total = total + number
    return nothing
function ordinary_probe(view net: Network) returns nothing:
    pure_entry()
    return nothing
"#;

#[test]
fn ordinary_entry_default_archive_refinement_predicates_remain_ordinary_through_preparation() {
    for release in [false, true] {
        let file = FileId::new(0);
        let parsed = parse(RAW_REFINEMENTS, file);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let resolved = jett_resolve::resolve(&parsed.module);
        let checked = jett_typecheck::check_with_options(
            &parsed.module,
            &resolved,
            jett_typecheck::CheckOptions { release },
        );
        assert!(
            checked
                .diagnostics
                .iter()
                .all(|diagnostic| { diagnostic.severity != jett_diagnostics::Severity::Error }),
            "{:?}",
            checked.diagnostics
        );
        let origins = HashMap::from([(file, SourceOrigin::Project)]);
        let hir = hir::lower(&parsed.module, &resolved, &checked, &origins).unwrap();
        assert!(hir.resource_source.manifest().is_none());
        assert_eq!(
            hir.functions
                .iter()
                .filter(|function| {
                    function.identity.declaration.kind == hir::DeclarationKind::RefinementPredicate
                })
                .count(),
            2
        );
        let types = &checked.interner;
        let baseline = crate::lower(&hir, types).unwrap();
        assert!(
            named(&baseline, "pure_entry")
                .blocks
                .iter()
                .any(|block| { matches!(block.terminator.kind, TerminatorKind::ForEach { .. }) })
        );
        for prepared in [false, true] {
            let mut program = baseline.clone();
            if prepared {
                prepare(&mut program, types);
                assert!(!named(&program, "pure_entry").blocks.iter().any(|block| {
                    matches!(block.terminator.kind, TerminatorKind::ForEach { .. })
                }));
            }
            assert!(
                program
                    .functions
                    .iter()
                    .all(|function| function.resource_lowering.is_none())
            );
            validate_call_ownership(&program, types).unwrap();
            let ordinary = validate_resource_ownership(&program, types).unwrap();
            assert!(ordinary.functions().is_empty());
            assert!(ordinary.entry_scope().is_none());
            for name in ["pure_entry", "ordinary_probe"] {
                assert!(
                    validate_resource_ownership_for_entry(
                        &program,
                        types,
                        named(&program, name).id
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn ordinary_entry_checked_refinement_archive_retains_proofs_and_refuses_missing_authority() {
    for release in [false, true] {
        let checked = checked(RAW_REFINEMENTS, release);
        let types = &checked.checked().interner;
        let hir = hir::lower_checked_resource_program(&checked).unwrap();
        assert!(hir.resource_source.manifest().is_some());
        assert!(!hir.resource_manifest.kinds().is_empty());
        let baseline = crate::lower(&hir, types).unwrap();
        let predicates = baseline
            .functions
            .iter()
            .filter(|function| {
                function.identity.declaration.kind == hir::DeclarationKind::RefinementPredicate
            })
            .collect::<Vec<_>>();
        assert_eq!(predicates.len(), 2);
        for predicate in predicates {
            let witness = predicate
                .resource_lowering
                .as_ref()
                .expect("checked refinement proof");
            assert_eq!(&witness.source, &hir.resource_source);
            witness.current(predicate).unwrap();
            assert!(!has_execution_records(predicate, types));
        }
        for prepared in [false, true] {
            let mut program = baseline.clone();
            if prepared {
                prepare(&mut program, types);
            }
            for name in ["pure_entry", "ordinary_probe"] {
                if prepared {
                    assert_bridge(&program, types, name);
                } else {
                    // The initial proof exists before executable For staging.
                    // Storage/ABI analysis follows certified preparation above.
                    assert_bridge_metadata(&program, types, name);
                }
            }
            let selected = named(&program, "pure_entry").id;
            named_mut(&mut program, "pure_entry").resource_lowering = None;
            let errors = match validate_resource_ownership(&program, types) {
                Err(errors) => errors,
                Ok(_) => panic!("lost checked entry witness was accepted"),
            };
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("constructor witness"))
            );
            assert!(validate_resource_ownership_for_entry(&program, types, selected).is_err());
        }
        // Removing the checked archive cannot turn a real Resource manifest
        // into the ordinary raw-HIR path before initial constructor capture.
        let mut no_archive = hir.clone();
        no_archive.resource_source = hir::ResourceSourceArchive::empty();
        let errors = match crate::lower(&no_archive, types) {
            Err(errors) => errors,
            Ok(_) => panic!("raw HIR with a Resource manifest was accepted"),
        };
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("no original checked HIR archive"))
        );
    }
}

#[test]
fn ordinary_entry_raw_body_only_resource_descriptor_retains_mandatory_custody_capture() {
    for release in [false, true] {
        let checked = checked(
            "namespace app\nfunction pure_entry() returns nothing:\n    return nothing\n",
            release,
        );
        let types = &checked.checked().interner;
        let hir = hir::lower_checked_resource_program(&checked).unwrap();
        let mut function = hir
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "pure_entry"
            })
            .unwrap()
            .clone();
        let descriptor = hir
            .functions
            .iter()
            .find_map(|candidate| {
                let mut found = None;
                walk::hir_block(&candidate.body, &mut |value| {
                    if matches!(value.kind, hir::ExpressionKind::ResourceHookValue { .. }) {
                        found = Some(value.clone());
                    }
                });
                found
            })
            .expect("genuine checked descriptor expression");
        assert!(!resource_type_pending(types, function.return_type));
        assert!(
            function
                .locals
                .iter()
                .all(|local| !resource_type_pending(types, local.ty))
        );
        function.body.statements.insert(
            0,
            hir::Statement {
                span: descriptor.span,
                kind: hir::StatementKind::Expression(descriptor),
            },
        );
        let no_archive = hir::ResourceSourceArchive::empty();
        let execution = ResourceExecutionClosure::default();
        assert!(runtime_custody_needed(&function, types, &execution));
        let capture = Capture::authenticated(
            &function,
            &hir.resource_manifest,
            &no_archive,
            types,
            &execution,
        );
        assert!(
            capture.witness.is_some(),
            "raw body-only custody must not be made ordinary"
        );
        let mut raw = hir;
        function.id = FunctionId::new(0);
        raw.functions = vec![function];
        raw.resource_source = no_archive;
        let errors = match crate::lower(&raw, types) {
            Err(errors) => errors,
            Ok(_) => panic!("raw body-only Resource descriptor was accepted"),
        };
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("no original checked HIR archive"))
        );
    }
}

#[test]
fn ordinary_entry_checked_scalar_refinement_lost_foreign_body_or_header_proofs_refuse() {
    for release in [false, true] {
        let checked = checked(RAW_REFINEMENTS, release);
        let types = &checked.checked().interner;
        let baseline = lowered(&checked);
        let selected = baseline
            .functions
            .iter()
            .find(|function| {
                function.identity.declaration.namespace == "app"
                    && function.identity.declaration.name == "Positive"
                    && function.identity.declaration.kind
                        == hir::DeclarationKind::RefinementPredicate
            })
            .expect("genuine generated scalar refinement predicate")
            .id;
        let foreign = lowered(&self::checked(
            &RAW_REFINEMENTS.replace("value > 0", "value > 1"),
            release,
        ));
        for mutation in 0..5 {
            let mut changed = baseline.clone();
            let function = changed
                .functions
                .iter_mut()
                .find(|function| function.id == selected)
                .unwrap();
            match mutation {
                0 => function.resource_lowering = None,
                1 => {
                    function.resource_lowering = None;
                    function.identity.declaration.kind = hir::DeclarationKind::Function;
                }
                2 => {
                    let value = function
                        .blocks
                        .iter_mut()
                        .find_map(|block| {
                            if let TerminatorKind::Return(Some(value)) = &mut block.terminator.kind
                            {
                                Some(value)
                            } else {
                                None
                            }
                        })
                        .expect("actual refinement return operand");
                    assert_eq!(value.ty, TypeInterner::BOOL);
                    value.kind = hir::ExpressionKind::Bool(false);
                }
                3 => function.return_type = TypeInterner::NOTHING,
                4 => {
                    function.resource_lowering = foreign
                        .functions
                        .iter()
                        .find(|function| function.id == selected)
                        .unwrap()
                        .resource_lowering
                        .clone();
                }
                _ => unreachable!(),
            }
            let errors = match validate_resource_ownership(&changed, types) {
                Err(errors) => errors,
                Ok(_) => panic!("refinement mutation {mutation} was accepted; release={release}"),
            };
            if mutation < 2 {
                assert!(
                    errors
                        .iter()
                        .any(|error| error.span == function_span(&baseline, selected)
                            && error.message.contains("constructor witness"))
                );
            }
            let before = changed.clone();
            crate::prepare_native_sequences(&mut changed, types);
            assert!(
                changed == before,
                "invalid refinement proof was canonically resealed"
            );
        }
    }
}

fn function_span(program: &Program, id: FunctionId) -> jett_common::Span {
    program
        .functions
        .iter()
        .find(|function| function.id == id)
        .unwrap()
        .span
}
