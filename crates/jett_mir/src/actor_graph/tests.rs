use super::*;
use jett_common::{FileId, SourceOrigin};
use jett_comptime::checked_types::CheckedExpressionTypes;
use std::{collections::HashMap, sync::Arc};
const SOURCE: &str = r#"namespace app
actor Counter:
    mutable int64 count = 1
    receive read responds int64:
        respond count
function make() returns Counter:
    return comptime spawn Counter()
function main() returns nothing:
    Counter worker = make()
    int64 observed = ask worker.read
    println(observed)
    return nothing
"#;
fn prepared_source(source: &str) -> Arc<jett_typecheck::CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions { release: false },
        )
        .expect("checked graph Source"),
    )
}
fn prepared() -> Arc<jett_typecheck::CheckedResourceProgram> {
    prepared_source(SOURCE)
}
fn values(
    checked: &Arc<jett_typecheck::CheckedResourceProgram>,
) -> jett_comptime::ExplicitComptimeValues {
    let evaluation = jett_comptime::evaluate_explicit_comptime_expressions_capture(
        checked.module(),
        checked.checked().reflection_metadata.clone(),
        Arc::new(CheckedExpressionTypes {
            resource_program: Some(checked.clone()),
            expressions: checked
                .checked()
                .type_map
                .iter()
                .map(|(span, ty)| (*span, checked.checked().interner.type_name(*ty)))
                .collect(),
            ..Default::default()
        }),
        Arc::new(HashMap::new()),
    );
    assert!(
        evaluation.diagnostics.is_empty(),
        "{:?}",
        evaluation.diagnostics
    );
    assert!(evaluation.debug_events.is_empty());
    evaluation.values
}
fn graph() -> (Program, Arc<jett_typecheck::CheckedResourceProgram>) {
    let checked = prepared();
    let values = values(&checked);
    let mut hir = hir::lower_checked_ordinary_program(&checked).unwrap();
    hir::bake_required_values(
        &mut hir,
        &values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    hir::complete_value_conversions(&mut hir, &checked.checked().interner).unwrap();
    (
        crate::lower(&hir, &checked.checked().interner).unwrap(),
        checked,
    )
}
#[test]
fn lowering_installs_independent_original_and_current_graph_seals() {
    let (program, _) = graph();
    validate_native_actor_graphs(&program).unwrap();
    assert_eq!(
        program
            .functions
            .iter()
            .filter(|function| has_records(function))
            .count(),
        1
    );
    crate::validate(&program).unwrap();
}
#[test]
fn raw_cfg_header_and_operand_edits_refuse() {
    let (program, _) = graph();
    let index = program.functions.iter().position(has_records).unwrap();
    let mut changed = program.clone();
    changed.functions[index].capture_count += 1;
    assert!(validate_native_actor_graphs(&changed).is_err());
    let mut changed = program.clone();
    changed.functions[index].blocks[0].statements.clear();
    assert!(validate_native_actor_graphs(&changed).is_err());
    let mut changed = program;
    changed.functions[index].actor_graph = None;
    assert!(validate_native_actor_graphs(&changed).is_err());
}
#[test]
fn canonical_native_preparation_keeps_the_import_activation_seal() {
    let (mut program, checked) = graph();
    let types = &checked.checked().interner;
    crate::prepare_native_generated_functions(&mut program, types);
    crate::prepare_native_sequences(&mut program, types);
    crate::prepare_native_uninhabited_sums(&mut program, types);
    validate_native_actor_graphs(&program).unwrap();
    crate::validate(&program).unwrap();
}

#[test]
fn ordinary_shared_graph_factory_calls_keep_ordinary_storage_authority() {
    let checked = prepared_source(include_str!(
        "../../../jett_driver/tests/actor_graph/fixtures/02_shared_fresh.jett"
    ));
    let values = values(&checked);
    let types = &checked.checked().interner;
    let mut hir = hir::lower_checked_ordinary_program(&checked).unwrap();
    hir::bake_required_values(
        &mut hir,
        &values,
        types,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    hir::complete_value_conversions(&mut hir, types).unwrap();
    let mut program = crate::lower(&hir, types).unwrap();
    crate::prepare_native_sequences(&mut program, types);
    crate::prepare_native_uninhabited_sums(&mut program, types);
    crate::prepare_native_generated_functions(&mut program, types);
    let ownership = crate::validate_resource_ownership(&program, types).unwrap();
    for function in &program.functions {
        if ownership
            .required_only_function_ids()
            .contains(&function.id)
        {
            continue;
        }
        let storage = if ownership.function(function.id).is_some()
            || crate::native_graph_storage_has_records(function, types)
        {
            crate::ResourceCompanionPlan::analyze(&ownership, function.id)
                .map(|plan| plan.storage().owned_locals.clone())
        } else {
            crate::move_values::MoveValuePlan::analyze(&program, function, types)
                .map(|plan| plan.owned_locals)
        };
        assert!(
            storage.is_ok(),
            "{}: {:?}; graph rows: {}",
            function.identity.declaration.name,
            storage,
            crate::native_graph_storage_has_records(function, types)
        );
    }
}

#[test]
fn map_shared_graph_iteration_prepares_ordinary_owned_element_storage() {
    let source = include_str!("../../../jett_driver/tests/actor_graph/fixtures/10_map_shared.jett");
    let mut parsed = jett_parser::parse(source, FileId::new(0));
    let mut origins = HashMap::from([(FileId::new(0), SourceOrigin::Project)]);
    for (index, support) in [
        include_str!("../../../../stdlib/map.jett"),
        include_str!("../../../../stdlib/list.jett"),
    ]
    .into_iter()
    .enumerate()
    {
        let file = FileId::new(10_000 + index as u32);
        let support = jett_parser::parse(support, file);
        assert!(support.errors.is_empty());
        let mut items = support.module.items;
        items.append(&mut parsed.module.items);
        parsed.module.items = items;
        origins.insert(file, SourceOrigin::Stdlib);
    }
    let checked = Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            parsed,
            origins,
            &[],
            jett_typecheck::CheckOptions { release: false },
        )
        .unwrap(),
    );
    let values = values(&checked);
    let types = &checked.checked().interner;
    let mut hir = hir::lower_checked_ordinary_program(&checked).unwrap();
    hir::bake_required_values(
        &mut hir,
        &values,
        types,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    hir::complete_value_conversions(&mut hir, types).unwrap();
    let mut program = crate::lower(&hir, types).unwrap();
    crate::prepare_native_sequences(&mut program, types);
    let main = program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "main"
        })
        .unwrap();
    assert!(
        main.blocks
            .iter()
            .all(|block| !matches!(block.terminator.kind, TerminatorKind::ForEach { .. }))
    );
    validate_native_actor_graphs(&program).unwrap();
    let ownership = crate::validate_resource_ownership(&program, types).unwrap();
    let storage = crate::ResourceCompanionPlan::analyze(&ownership, main.id).unwrap();
    assert!(storage.storage().owned_locals.iter().any(|local| {
        main.locals
            .get(*local)
            .is_some_and(|header| header.name.starts_with("$iterator"))
    }));
}

#[test]
fn protected_graph_list_iteration_transports_only_its_exact_generated_owner() {
    let checked = prepared_source(
        "namespace app\nactor Counter:\n    mutable int64 count = 3\n    receive read responds int64:\n        respond count\nfunction main() returns nothing:\n    list[Counter] workers = comptime list(spawn Counter())\n    for worker in workers:\n        println(ask worker.read)\n    return nothing\n",
    );
    let evaluated = values(&checked);
    let types = &checked.checked().interner;
    let mut hir = hir::lower_checked_ordinary_program(&checked).unwrap();
    hir::bake_required_values(
        &mut hir,
        &evaluated,
        types,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    hir::complete_value_conversions(&mut hir, types).unwrap();
    let mut program = crate::lower(&hir, types).unwrap();
    crate::prepare_native_sequences(&mut program, types);
    let main = program
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "main"
        })
        .unwrap();
    assert!(
        main.blocks
            .iter()
            .all(|block| !matches!(block.terminator.kind, TerminatorKind::ForEach { .. }))
    );
    let ownership = crate::validate_resource_ownership(&program, types).unwrap();
    crate::ResourceCompanionPlan::analyze(&ownership, main.id).unwrap();
    let main_id = main.id;
    let source = main
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .find_map(|statement| match &statement.kind {
            StatementKind::SequenceLength {
                source: SequenceSource::Local(local),
                ..
            } => Some(*local),
            _ => None,
        })
        .unwrap();
    for forgery in 0..5 {
        let mut forged = program.clone();
        let function = &mut forged.functions[main_id.index() as usize];
        match forgery {
            0 => function.locals[source.index() as usize].span.end += 1,
            1 => {
                let statement = function.blocks.iter_mut().flat_map(|block| &mut block.statements)
                    .find(|statement| matches!(&statement.kind, StatementKind::Let { local, .. } if *local == source)).unwrap();
                let StatementKind::Let { value, .. } = &mut statement.kind else {
                    unreachable!()
                };
                value.span.end += 1;
            }
            2 => {
                let statement = function
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.statements)
                    .find(|statement| matches!(statement.kind, StatementKind::SequenceGet { .. }))
                    .unwrap();
                let StatementKind::SequenceGet { consume, .. } = &mut statement.kind else {
                    unreachable!()
                };
                *consume = false;
            }
            3 => {
                let statement = function
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.statements)
                    .find(|statement| matches!(statement.kind, StatementKind::SequenceGet { .. }))
                    .unwrap();
                let StatementKind::SequenceGet { part, .. } = &mut statement.kind else {
                    unreachable!()
                };
                *part = SequencePart::Value;
            }
            4 => {
                let block = function
                    .blocks
                    .iter_mut()
                    .find(|block| {
                        block.statements.iter().any(|statement| {
                            matches!(statement.kind, StatementKind::SequenceGet { .. })
                        })
                    })
                    .unwrap();
                block.statements.swap(0, 1);
            }
            _ => unreachable!(),
        }
        assert!(
            crate::validate_resource_ownership(&forged, types).is_err(),
            "accepted forgery {forgery}"
        );
    }
}
