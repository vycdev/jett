use super::*;
use jett_common::{FileId, SourceOrigin};
use jett_comptime::checked_types::CheckedExpressionTypes;
use std::{collections::HashMap, sync::Arc};
fn prepared(source: &str) -> Arc<jett_typecheck::CheckedResourceProgram> {
    let file = FileId::new(0);
    Arc::new(
        jett_typecheck::CheckedResourceProgram::prepare(
            jett_parser::parse(source, file),
            HashMap::from([(file, SourceOrigin::Project)]),
            &[],
            jett_typecheck::CheckOptions { release: false },
        )
        .unwrap(),
    )
}
fn baked(checked: &Arc<jett_typecheck::CheckedResourceProgram>) -> jett_hir::Program {
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
    let mut hir = jett_hir::lower_checked_ordinary_program(checked).unwrap();
    jett_hir::bake_required_values(
        &mut hir,
        &evaluation.values,
        &checked.checked().interner,
        &checked.checked().reflection_metadata,
        &checked.checked().method_value_definitions,
    )
    .unwrap();
    jett_hir::complete_value_conversions(&mut hir, &checked.checked().interner).unwrap();
    hir
}
#[test]
fn original_direct_actor_import_emits_an_object_from_checked_graph_steps() {
    let checked = prepared(
        r#"namespace app
actor Counter:
    mutable int64 count = 1
    receive read responds int64:
        respond count
function main() returns nothing:
    Counter worker = comptime spawn Counter()
    int64 observed = ask worker.read
    println(observed)
    return nothing
"#,
    );
    let hir = baked(&checked);
    let mir = jett_mir::lower(&hir, &checked.checked().interner).unwrap();
    let bytes = crate::emit_host_object(&mir, &checked.checked().interner).unwrap();
    assert!(!bytes.bytes.is_empty());
}
#[test]
fn nested_state_graph_emits_typed_nonowning_actor_references() {
    let checked = prepared(
        "namespace app\nactor Counter:\n    mutable int64 count = 1\nactor Holder:\n    app.Counter child = spawn app.Counter()\nfunction main() returns nothing:\n    Holder worker = comptime spawn Holder()\n    return nothing\n",
    );
    let hir = baked(&checked);
    let mir = jett_mir::lower(&hir, &checked.checked().interner).unwrap();
    let bytes = crate::emit_host_object(&mir, &checked.checked().interner).unwrap();
    assert!(!bytes.bytes.is_empty());
}
#[test]
fn emitted_field_schema_uses_exact_ja1_and_jd1_headers() {
    let checked = prepared(
        "namespace app\nactor Counter:\n    mutable int64 count = 1\nfunction main() returns nothing:\n    Counter worker = comptime spawn Counter()\n    return nothing\n",
    );
    let hir = baked(&checked);
    let mut plans = Vec::new();
    for function in &hir.functions {
        let mut body = function.body.clone();
        jett_hir::visit_actor_graph_plans(&mut body, |plan| plans.push(plan.clone())).unwrap();
    }
    let fill = plans[0]
        .instructions()
        .iter()
        .find(|step| matches!(step.operation(), ActorGraphOperation::Fill { .. }))
        .unwrap();
    let (bytes, functions) = import_layout(
        fill,
        &checked.checked().interner,
        TypeInterner::INT64,
        HashMap::new(),
    )
    .unwrap();
    assert_eq!(&bytes[..3], b"JA\x01");
    assert_eq!(&bytes[7..10], b"JD\x01");
    assert!(functions.is_empty());
}

#[test]
fn actor_callback_field_keeps_exact_admitted_primitive_captures_and_compiled_code() {
    let checked = prepared(
        "namespace app\nfunction create_callback() returns function() returns string:\n    string text = \"baked\"\n    return function() returns string: return text\nactor Keeper:\n    mutable function() returns string callback = create_callback()\n    receive read responds string:\n        respond callback()\nfunction main() returns nothing:\n    Keeper worker = comptime spawn Keeper()\n    string text = ask worker.read\n    println(text)\n    return nothing\n",
    );
    let hir = baked(&checked);
    let mir = jett_mir::lower(&hir, &checked.checked().interner).unwrap();
    let artifact = crate::emit_host_object(&mir, &checked.checked().interner).unwrap();
    assert!(!artifact.bytes.is_empty());
}

#[test]
fn ordinary_shared_graph_factory_emits_without_resource_scope_or_layout() {
    let checked = prepared(include_str!(
        "../../../../jett_driver/tests/actor_graph/fixtures/02_shared_fresh.jett"
    ));
    let hir = baked(&checked);
    let types = &checked.checked().interner;
    let mir = jett_mir::lower(&hir, types).unwrap();
    assert!(mir.resource_manifest.kinds().is_empty());
    let ownership = jett_mir::validate_resource_ownership(&mir, types).unwrap();
    assert!(ownership.entry_scope().is_none());
    assert!(ownership.functions().is_empty());
    let entry = mir
        .functions
        .iter()
        .find(|function| function.identity.declaration.name == "main")
        .unwrap()
        .id;
    for optimize in [false, true] {
        let artifact = crate::emit_host_program_object_with_options(
            &mir,
            types,
            entry,
            crate::CodegenOptions { optimize },
        )
        .unwrap();
        assert!(!artifact.bytes.is_empty());
        assert!(
            artifact
                .symbols
                .iter()
                .all(|symbol| symbol != "jett_aot_resource_v1_manifest")
        );
    }
    let mut forged = mir;
    let function = forged
        .functions
        .iter_mut()
        .find(|function| function.identity.declaration.name == "baked")
        .unwrap();
    function.locals[0].span.end += 1;
    assert!(crate::emit_host_program_object(&forged, types, entry).is_err());
}

#[test]
fn map_shared_graph_iteration_emits_both_ordinary_object_options() {
    let source =
        include_str!("../../../../jett_driver/tests/actor_graph/fixtures/10_map_shared.jett");
    let mut parsed = jett_parser::parse(source, FileId::new(0));
    let mut origins = HashMap::from([(FileId::new(0), SourceOrigin::Project)]);
    for (index, support) in [
        include_str!("../../../../../stdlib/map.jett"),
        include_str!("../../../../../stdlib/list.jett"),
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
    let hir = baked(&checked);
    let types = &checked.checked().interner;
    let mir = jett_mir::lower(&hir, types).unwrap();
    assert!(mir.resource_manifest.kinds().is_empty());
    let ownership = jett_mir::validate_resource_ownership(&mir, types).unwrap();
    assert!(ownership.entry_scope().is_none());
    assert!(ownership.functions().is_empty());
    let entry = mir
        .functions
        .iter()
        .find(|function| {
            function.identity.declaration.namespace == "app"
                && function.identity.declaration.name == "main"
        })
        .unwrap()
        .id;
    for optimize in [false, true] {
        let artifact = crate::emit_host_program_object_with_options(
            &mir,
            types,
            entry,
            crate::CodegenOptions { optimize },
        )
        .unwrap();
        assert!(!artifact.bytes.is_empty());
        assert!(
            artifact
                .symbols
                .iter()
                .all(|symbol| symbol != "jett_aot_resource_v1_manifest")
        );
    }
}
