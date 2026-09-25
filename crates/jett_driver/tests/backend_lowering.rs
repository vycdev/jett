use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use jett_common::SourceOrigin;
use jett_driver::{lower_file_for_backend, lower_file_for_native_verify_suite};
use jett_hir::{DeclarationKind, ExpressionKind, StatementKind};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/run_pass")
}

fn run_pass_fixtures() -> Vec<PathBuf> {
    let mut fixtures = fs::read_dir(fixture_dir())
        .expect("run-pass fixture directory should be readable")
        .map(|entry| entry.expect("fixture entry should be readable").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jett")
        })
        .collect::<Vec<_>>();
    fixtures.sort();
    fixtures
}

fn fixture_name(path: &Path) -> String {
    path.file_name()
        .expect("fixture should have a file name")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn backend_lowering_retains_explicit_project_and_stdlib_origins() {
    let fixture = fixture_dir().join("simple.jett");
    let lowered = lower_file_for_backend(&fixture).expect("simple fixture should lower");

    assert_eq!(
        lowered.source_origins.get(&jett_common::FileId::new(0)),
        Some(&SourceOrigin::Project)
    );
    assert!(
        lowered
            .source_origins
            .values()
            .any(|origin| *origin == SourceOrigin::Stdlib),
        "lowering should retain an explicit stdlib source origin"
    );
    assert!(!lowered.hir.functions.is_empty());
    assert_eq!(lowered.hir.functions.len(), lowered.mir.functions.len());
    assert_eq!(
        lowered.program_entry, None,
        "a verification-only source must not invent a native program entry"
    );
}

#[test]
fn backend_lowering_publishes_the_exact_primary_program_entry() {
    let fixture = fixture_dir().join("breakpoint_basic.jett");
    let lowered = lower_file_for_backend(&fixture).expect("main fixture should lower");
    let entry = lowered
        .program_entry
        .expect("the primary source main must have a checked function identity");

    let hir_entry = lowered
        .hir
        .functions
        .iter()
        .find(|function| function.id == entry)
        .expect("program entry must name one HIR function");
    let mir_entry = lowered
        .mir
        .functions
        .iter()
        .find(|function| function.id == entry)
        .expect("program entry must name the corresponding MIR function");

    assert_eq!(hir_entry.identity.declaration.name, "main");
    assert_eq!(hir_entry.identity.declaration.origin, SourceOrigin::Project);
    assert_eq!(hir_entry.identity, mir_entry.identity);
    assert_eq!(hir_entry.span, mir_entry.span);
}

#[test]
fn native_verify_suite_calls_primary_file_bodies_in_declaration_order() {
    let fixture = fixture_dir().join("verify_test.jett");
    let lowered =
        lower_file_for_native_verify_suite(&fixture).expect("verify fixture should lower");
    assert_eq!(lowered.program_entry, None);
    let entry = lowered
        .native_verify_entry
        .expect("verify fixture must have a suite entry");
    let suite = &lowered.hir.functions[entry.index() as usize];
    assert_eq!(suite.identity.declaration.namespace, "app");
    assert!(
        suite
            .identity
            .declaration
            .name
            .starts_with("__native_verify_suite:")
    );
    let called = suite
        .body
        .statements
        .iter()
        .map(|statement| match &statement.kind {
            StatementKind::Expression(expression) => match &expression.kind {
                ExpressionKind::Call { function, args, .. } if args.is_empty() => *function,
                other => panic!("suite statement does not call a verify body: {other:?}"),
            },
            other => panic!("suite has an unexpected statement: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(called.len(), 2);
    let bodies = called
        .iter()
        .map(|id| &lowered.hir.functions[id.index() as usize])
        .collect::<Vec<_>>();
    assert!(bodies.iter().all(|body| {
        body.identity.declaration.kind == DeclarationKind::Verify
            && body.span.file == suite.span.file
    }));
    assert!(bodies[0].identity.declaration.name.starts_with("add:"));
    assert!(bodies[1].identity.declaration.name.starts_with("multiply:"));
}

#[test]
fn backend_lowering_handles_function_values_and_qualified_type_operations() {
    let fixtures = [
        "generic_function_value_wrappers.jett",
        "graphics_callback_runtime_error.jett",
        "graphics_pipeline_scripted.jett",
        "graphics_scene.jett",
        "graphics_scripted.jett",
        "json_namespace_duplicate_machine_envelope.jett",
        "list_higher_order.jett",
        "list_operations.jett",
        "list_shape_helpers.jett",
        "list_source_surface.jett",
        "namespace_duplicate_leaf_types.jett",
        "namespace_duplicate_leaf_interfaces.jett",
        "namespace_exports_syntax.jett",
        "namespace_machine_branch_narrowing.jett",
        "namespace_qualified_interface_implement.jett",
        "namespace_qualified_types.jett",
        "namespace_use_alias.jett",
        "reflection_type_id_duplicate_named_owners.jett",
    ];

    let failures = fixtures
        .into_iter()
        .filter_map(|name| {
            lower_file_for_backend(&fixture_dir().join(name))
                .err()
                .map(|error| (name, error.to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    assert!(
        failures.is_empty(),
        "checked function/type operations failed backend lowering:\n{failures:#?}"
    );
}

#[test]
fn backend_lowering_handles_reflection_driven_generic_bodies() {
    let fixtures = [
        "captured_local_function_name.jett",
        "comptime_type_bind.jett",
        "json_reflection_flat_decoder.jett",
        "json_reflection_flat_serializer.jett",
        "type_construction_builder.jett",
        "type_construction_enum.jett",
        "type_construction_machine.jett",
    ];

    let failures = fixtures
        .into_iter()
        .filter_map(|name| {
            lower_file_for_backend(&fixture_dir().join(name))
                .err()
                .map(|error| (name, error.to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    assert!(
        failures.is_empty(),
        "reflection generic bodies failed backend lowering:\n{failures:#?}"
    );
}

#[test]
fn run_pass_backend_lowering_gaps_are_explicit_and_monotonic() {
    let fixtures = run_pass_fixtures();
    assert_eq!(fixtures.len(), 182, "update the native parity denominator");

    let worker_count = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .min(8)
        .min(fixtures.len());
    let chunk_size = fixtures.len().div_ceil(worker_count);
    let failures = std::thread::scope(|scope| {
        let workers = fixtures.chunks(chunk_size).map(|chunk| {
            scope.spawn(|| {
                chunk
                    .iter()
                    .filter_map(|fixture| {
                        lower_file_for_backend(fixture)
                            .err()
                            .map(|error| (fixture_name(fixture), error.to_string()))
                    })
                    .collect::<Vec<_>>()
            })
        });
        workers
            .flat_map(|worker| worker.join().expect("lowering worker should not panic"))
            .collect::<BTreeMap<_, _>>()
    });

    assert!(
        failures.is_empty(),
        "run-pass fixtures still outside backend lowering:\n{failures:#?}"
    );
}

#[test]
fn backend_lowering_bakes_explicit_comptime_but_preserves_runtime_calls() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("constants.jett");
    fs::write(&source, "function double(value: int64) returns int64:\n    return value * 2\nfunction main() returns nothing:\n    int64 baked = comptime double(21)\n    int64 dynamic = double(20)\n    return nothing\n").unwrap();
    let lowered = lower_file_for_backend(&source).expect("checked constants lower");
    let entry = lowered.program_entry.unwrap();
    let main = &lowered.mir.functions[entry.index() as usize];
    let jett_mir::StatementKind::Let { value: baked, .. } = &main.blocks[0].statements[0].kind
    else {
        panic!("baked let")
    };
    assert!(
        matches!(baked.kind, jett_hir::ExpressionKind::Int(42)),
        "explicit comptime must become typed constant, got {:?}",
        baked.kind
    );
    let jett_mir::StatementKind::Let { value: dynamic, .. } = &main.blocks[0].statements[1].kind
    else {
        panic!("dynamic let")
    };
    assert!(
        matches!(dynamic.kind, jett_hir::ExpressionKind::Call { .. }),
        "ordinary pure call must remain runtime MIR"
    );
}
