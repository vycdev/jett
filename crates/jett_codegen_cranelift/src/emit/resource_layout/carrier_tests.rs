use super::*;
use jett_comptime::checked_types::CheckedExpressionTypes;
use jett_mir::{ResourceCarrierNode, ResourceCarrierOperationRole, ResourceCarrierProjectionPath};
use object::{Object, ObjectSymbol};

include!("../resource_execution/tests/carrier_wrapper_sources.rs");

const WHOLE_SOURCE: &str = include_str!(
    "../../../../jett_mir/src/resource_ownership/fixtures/17_absent_aggregate_shapes.jett"
);

fn checked_wrapper(suffix: &str, release: bool) -> Arc<CheckedResourceProgram> {
    checked_wrapper_source(WHOLE_SOURCE, suffix, release)
}

fn checked_wrapper_source(
    prefix: &str,
    suffix: &str,
    release: bool,
) -> Arc<CheckedResourceProgram> {
    let mut parsed = parse(SUPPORT, FileId::new(10_000));
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let declaration = parsed
        .module
        .items
        .iter()
        .find_map(|item| match item {
            Item::Resource(resource) => Some(resource.name.span),
            _ => None,
        })
        .unwrap();
    let source = format!("{prefix}{suffix}");
    assert!(source.starts_with(prefix));
    let mut origins = HashMap::from([(FileId::new(10_000), SourceOrigin::Stdlib)]);
    for (text, file, origin) in [
        (LIST_LENGTH, FileId::new(10_001), SourceOrigin::Stdlib),
        (MAP_LENGTH, FileId::new(10_002), SourceOrigin::Stdlib),
        (source.as_str(), FileId::new(0), SourceOrigin::Project),
    ] {
        let next = parse(text, file);
        assert!(next.errors.is_empty(), "{:?}", next.errors);
        parsed.module.items.extend(next.module.items);
        origins.insert(file, origin);
    }
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
        CheckedResourceProgram::prepare(parsed, origins, &catalog, CheckOptions { release })
            .unwrap_or_else(|error| panic!("{error:?}; {:?}", error.diagnostics())),
    )
}

fn lowered_wrapper(checked: &Arc<CheckedResourceProgram>) -> Program {
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
    assert!(
        required
            .values
            .values()
            .all(|value| matches!(value, jett_comptime::Value::Int64(17)))
    );
    let types = &checked.checked().interner;
    let mut hir = jett_hir::lower_checked_resource_program(checked).unwrap();
    jett_hir::materialize_checked_required_values(&mut hir, &required.values, types).unwrap();
    jett_hir::complete_value_conversions(&mut hir, types).unwrap();
    jett_mir::lower(&hir, types).unwrap()
}

fn named<'a>(program: &'a Program, name: &str) -> &'a Function {
    program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == name
        })
        .unwrap()
}

fn named_mut<'a>(program: &'a mut Program, name: &str) -> &'a mut Function {
    program
        .functions
        .iter_mut()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == name
        })
        .unwrap()
}

#[test]
fn resource_carrier_whole_source17_all_fourteen_wrappers_emit_real_objects_in_both_profiles() {
    for release in [false, true] {
        for (name, suffix) in WRAPPERS {
            let checked = checked_wrapper(suffix, release);
            let types = &checked.checked().interner;
            let program = lowered_wrapper(&checked);
            // The unused refined formal and latent occupied constructors remain in
            // the original source and in its authenticated closure for every row.
            let refined = named(&program, "refined_shape_contract");
            assert_eq!(refined.params.len(), 1);
            assert_eq!(refined.return_type, TypeInterner::NOTHING);
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
            let artifact = crate::emit::emit_for_triple(
                &program,
                types,
                HOST,
                Some(entry(&program)),
                CodegenOptions { optimize: release },
            )
            .unwrap_or_else(|error| panic!("{name}, release={release}: {error:?}"));
            let object = object::File::parse(artifact.bytes.as_slice()).unwrap();
            let symbols = object
                .symbols()
                .filter_map(|symbol| {
                    symbol
                        .name()
                        .ok()
                        .map(|name| (name.to_owned(), symbol.is_definition()))
                })
                .collect::<Vec<_>>();
            assert!(
                symbols
                    .iter()
                    .any(|(name, defined)| name == "jett_aot_resource_v1_manifest" && *defined)
            );
            for leaf in [
                "carrier_construct_begin",
                "carrier_child",
                "carrier_commit",
                "carrier_publish",
                "source_enter",
                "source_status",
                "scope_complete",
            ] {
                assert!(
                    symbols.iter().any(|(name, defined)| name
                        == &format!("jett_rt_v1_resource_{leaf}")
                        && !*defined),
                    "{name}, release={release}, missing {leaf}: {symbols:?}"
                );
            }
        }
    }
}

#[test]
fn resource_carrier_v3_layout_retains_latent_shapes_refinement_and_exact_source_rows() {
    for release in [false, true] {
        let checked = checked_wrapper(WRAPPERS[6].1, release);
        let program = lowered_wrapper(&checked);
        let types = &checked.checked().interner;
        let first = EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
        let second = EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
        assert_eq!(first.bytes(), second.bytes());
        assert_eq!(
            u32::from_le_bytes(first.bytes()[8..12].try_into().unwrap()),
            3
        );
        assert_eq!(
            u64::from_le_bytes(first.bytes()[16..24].try_into().unwrap()),
            u64::try_from(first.bytes().len()).unwrap()
        );
        assert!(first.bytes().windows(8).any(|bytes| bytes == b"JTCAR003"));
        let mut rows = Rows::new(first.plan()).unwrap();
        rows.populate().unwrap();
        assert!(!rows.carrier_slots.is_empty());
        assert!(!rows.carrier_operations.is_empty());
        assert!(rows.shapes.iter().any(|row| row.first() == Some(&11)));
        for tag in [2, 7, 8, 9, 10, 11] {
            assert!(
                rows.carrier_graph
                    .rows()
                    .iter()
                    .any(|row| u32::from_le_bytes(row[..4].try_into().unwrap()) == tag),
                "retained node {tag}"
            );
        }
        let refined = first
            .plan()
            .function(named(&program, "refined_shape_contract").id)
            .unwrap();
        let graph = refined.carriers().unwrap();
        assert!(graph.shapes().iter().any(|shape| matches!(
            shape.node(),
            ResourceCarrierNode::Refinement { .. }
        ) && shape.refinement_predicate().is_some()));
        // A carrier is never installed into the legacy leaf-owner table merely
        // because its static graph contains a Resource in an inactive arm.
        assert!(
            rows.operations
                .iter()
                .any(|(_, row)| row.first() == Some(&26))
        );
    }
}

#[test]
fn resource_carrier_prepared_map_keeps_key_before_value_reset_and_legacy_optional_adapter() {
    for release in [false, true] {
        let checked = checked_wrapper(WRAPPERS[3].1, release);
        let types = &checked.checked().interner;
        let mut program = lowered_wrapper(&checked);
        jett_mir::prepare_native_sequences(&mut program, types);
        let layout = EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
        let function = layout.plan().function(entry(&program)).unwrap();
        let graph = function.carriers().unwrap();
        let [iteration] = graph.iterations() else {
            panic!("one original map loop")
        };
        assert!(iteration.cursor.is_some());
        let current = named(&program, "main");
        let gets = current.blocks[usize::try_from(iteration.body.index()).unwrap()]
            .statements
            .iter()
            .filter_map(|statement| match &statement.kind {
                StatementKind::SequenceGet { target, part, .. } => Some((*target, *part)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            gets,
            vec![
                (iteration.binders[0], jett_mir::SequencePart::Key),
                (iteration.binders[1], jett_mir::SequencePart::Value)
            ]
        );
        assert!(graph.operations().iter().any(|operation|
            matches!(operation.role(), ResourceCarrierOperationRole::RetireIteration { binders, .. }
                if binders == &iteration.binders)));
        assert!(graph.operations().iter().any(|operation|
            matches!(operation.role(), ResourceCarrierOperationRole::AdaptSum { path, loan: None, .. }
                if matches!(path.as_slice(), [ResourceCarrierProjectionPath::MapValue { .. }]))));
        let mut rows = Rows::new(layout.plan()).unwrap();
        rows.populate().unwrap();
        assert!(
            rows.carrier_operations
                .iter()
                .any(|row| u32::from_le_bytes(row[16..20].try_into().unwrap()) == 10)
        );
        assert!(
            rows.operations
                .iter()
                .any(|(_, row)| row.first() == Some(&16)),
            "the ordinary Source body keeps its exact v2 formal tuple inside wire v3"
        );
    }
}

#[test]
fn resource_carrier_object_gate_rejects_public_return_header_and_unused_refined_formal_substitution()
 {
    let checked = checked_wrapper(WRAPPERS[0].1, false);
    let types = &checked.checked().interner;
    let program = lowered_wrapper(&checked);
    EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
    // Clone a publicly available Function from a separately authenticated Source.
    // Its private witness remains opaque; the receiving program keeps its own
    // original archive and manifest. Both complete originals are valid alone.
    let foreign_source = WHOLE_SOURCE.replacen("marker: 17)", "marker: 19)", 1);
    assert_ne!(foreign_source, WHOLE_SOURCE);
    let foreign_checked = checked_wrapper_source(&foreign_source, WRAPPERS[0].1, false);
    let foreign_program = lowered_wrapper(&foreign_checked);
    EmittedResourceLayout::from_program(
        &foreign_program,
        &foreign_checked.checked().interner,
        entry(&foreign_program),
    )
    .unwrap();
    let foreign_function = named(&foreign_program, "absent_envelope");
    let original_function = named(&program, "absent_envelope");
    assert_eq!(foreign_function.id, original_function.id);
    assert_eq!(foreign_function.identity, original_function.identity);
    assert_eq!(foreign_function.return_type, original_function.return_type);
    assert_ne!(foreign_function.blocks, original_function.blocks);
    for mutation in 0..3 {
        let mut changed = program.clone();
        let map = named(&changed, "empty_map").return_type;
        let envelope = named(&changed, "absent_envelope").return_type;
        match mutation {
            0 => named_mut(&mut changed, "empty_tokens").return_type = map,
            1 => {
                let function = named_mut(&mut changed, "refined_shape_contract");
                let local = function.params[0].local;
                function.locals[usize::try_from(local.index()).unwrap()].ty = envelope;
            }
            2 => {
                *named_mut(&mut changed, "absent_envelope") = foreign_function.clone();
                let errors = jett_mir::validate_resource_ownership(&changed, types).unwrap_err();
                assert!(
                    errors.iter().any(|error| error.message
                        == "Resource ownership mixes foreign original/materialized archives"),
                    "a public Function clone cannot transplant its opaque archive: {errors:?}"
                );
            }
            _ => unreachable!(),
        }
        assert!(
            EmittedResourceLayout::from_program(&changed, types, entry(&changed)).is_err(),
            "fresh archive/type/CFG authentication, mutation {mutation}"
        );
        assert!(
            crate::emit::emit_for_triple(
                &changed,
                types,
                HOST,
                Some(entry(&changed)),
                CodegenOptions { optimize: false }
            )
            .is_err(),
            "no object, mutation {mutation}"
        );
    }
}

#[test]
fn resource_carrier_source13_and14_keep_original_required_evaluation_and_bounds_control() {
    assert!(WRAPPERS[12].1.contains("view values) != 1"));
    assert!(WRAPPERS[12].1.contains("resource_probe.terminal_label()"));
    assert!(WRAPPERS[13].1.contains("answer != 34"));
    assert!(WRAPPERS[13].1.contains("absent_namespace_value != 17"));
    for release in [false, true] {
        let checked = checked_wrapper(WRAPPERS[13].1, release);
        let program = lowered_wrapper(&checked);
        assert_eq!(
            named(&program, "absent_required_controls").return_type,
            TypeInterner::INT64
        );
        crate::emit::emit_for_triple(
            &program,
            &checked.checked().interner,
            HOST,
            Some(entry(&program)),
            CodegenOptions { optimize: release },
        )
        .unwrap();
    }
    // Runtime status/message goldens for these two exact Sources remain in the
    // full driver gate: wrong empty length reaches list.__remove_at bounds error;
    // required controls return 34. Object tests do not claim native execution.
}

#[test]
fn resource_carrier_observation_rows_distinguish_scalars_from_owned_strings_and_exact_adapter_imports()
 {
    for release in [false, true] {
        let mut observed_scalar = false;
        let mut observed_string = false;
        let mut imported_adapt = false;
        let mut imported_end = false;
        for suffix in [WRAPPERS[6].1, WRAPPERS[10].1] {
            let checked = checked_wrapper(suffix, release);
            let program = lowered_wrapper(&checked);
            let types = &checked.checked().interner;
            let layout =
                EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
            let mut rows = Rows::new(layout.plan()).unwrap();
            rows.populate().unwrap();
            for function in &program.functions {
                let Some(carriers) = layout
                    .plan()
                    .function(function.id)
                    .and_then(|plan| plan.carriers())
                else {
                    continue;
                };
                for operation in carriers.operations() {
                    let ResourceCarrierOperationRole::Observe {
                        observation: jett_mir::ResourceCarrierObservation::Ordinary { ty, .. },
                        ..
                    } = operation.role()
                    else {
                        continue;
                    };
                    let record =
                        rows.carrier_record_ids[&(function.id.index(), operation.id().index())];
                    let row = &rows.carrier_operations[usize::try_from(record).unwrap()];
                    let tail = &row[row.len() - 12..];
                    assert_eq!(u32::from_le_bytes(tail[..4].try_into().unwrap()), 4);
                    let shape = usize::try_from(u32::from_le_bytes(tail[4..8].try_into().unwrap()))
                        .unwrap();
                    let clone = u32::from_le_bytes(tail[8..12].try_into().unwrap());
                    match types.resolve(*ty) {
                        Type::Int64 => {
                            assert_eq!(rows.shapes[shape][0], 1);
                            assert_eq!(clone, 0, "a scalar is never a NativeValues handle");
                            observed_scalar = true;
                        }
                        Type::String => {
                            assert_eq!(rows.shapes[shape][0], 4);
                            assert_eq!(
                                clone, 1,
                                "a String observation needs independent ownership"
                            );
                            observed_string = true;
                        }
                        _ => {}
                    }
                }
            }
            let artifact = crate::emit::emit_for_triple(
                &program,
                types,
                HOST,
                Some(entry(&program)),
                CodegenOptions { optimize: release },
            )
            .unwrap();
            let object = object::File::parse(artifact.bytes.as_slice()).unwrap();
            for symbol in object.symbols().filter(|symbol| !symbol.is_definition()) {
                match symbol.name().unwrap() {
                    "jett_rt_v1_resource_carrier_adapt" => imported_adapt = true,
                    "jett_rt_v1_resource_carrier_end" => imported_end = true,
                    "jett_rt_v1_resource_carrier_adapt_sum"
                    | "jett_rt_v1_resource_carrier_end_borrow" => {
                        panic!("native import must name the exact existing exported leaf")
                    }
                    _ => {}
                }
            }
        }
        assert!(observed_scalar && observed_string);
        assert!(imported_adapt && imported_end);
    }
}

#[test]
fn resource_carrier_lexical_constructor_order_and_local_machine_qualification_emit_from_original_source()
 {
    let original = "TokenEnvelope(tokens: list(), fallback: none, marker: 17)";
    assert_eq!(WHOLE_SOURCE.matches(original).count(), 1);
    let reordered = WHOLE_SOURCE.replacen(
        original,
        "TokenEnvelope(marker: 17, tokens: list(), fallback: none)",
        1,
    );
    for release in [false, true] {
        let checked = checked_wrapper_source(&reordered, WRAPPERS[6].1, release);
        let program = lowered_wrapper(&checked);
        let types = &checked.checked().interner;
        let layout = EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
        let plan = layout
            .plan()
            .function(named(&program, "absent_envelope").id)
            .unwrap()
            .carriers()
            .unwrap();
        let storage_indices = plan
            .operations()
            .iter()
            .filter_map(|operation| match operation.role() {
                ResourceCarrierOperationRole::ConstructorChild { child, .. } => Some(child.index),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            storage_indices,
            vec![2, 0, 1],
            "declared storage order differs from lexical arrival 0,1,2"
        );
        crate::emit::emit_for_triple(
            &program,
            types,
            HOST,
            Some(entry(&program)),
            CodegenOptions { optimize: release },
        )
        .unwrap();

        let suffix = format!(
            "\nexport function source17_qualified_local() returns TokenState:\n    TokenState value = TokenState(vacant, none, \"empty\")\n    return value\n{}",
            WRAPPERS[10].1
        );
        let checked = checked_wrapper(&suffix, release);
        let program = lowered_wrapper(&checked);
        let types = &checked.checked().interner;
        let layout = EmittedResourceLayout::from_program(&program, types, entry(&program)).unwrap();
        let plan = layout
            .plan()
            .function(named(&program, "source17_qualified_local").id)
            .unwrap()
            .carriers()
            .unwrap();
        assert!(plan.operations().iter().any(|operation| matches!(
            operation.role(),
            ResourceCarrierOperationRole::QualifyMachine { .. }
        )));
        crate::emit::emit_for_triple(
            &program,
            types,
            HOST,
            Some(entry(&program)),
            CodegenOptions { optimize: release },
        )
        .unwrap();
    }
}
