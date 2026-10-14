//! Linked graph controls use ordinary runtime bundles and remove Source before execution.
#![cfg(all(
    target_arch = "x86_64",
    any(
        all(target_os = "windows", target_env = "msvc"),
        all(target_os = "linux", target_env = "gnu")
    )
))]
use jett_driver::native::NativeLauncherBundle;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    time::{Duration, Instant},
};

fn evidence() -> PathBuf {
    std::env::var_os("JETT_ACTOR_GRAPH_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/actor-graph-tests")
        })
}
fn profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}
fn launcher() -> &'static NativeLauncherBundle {
    static BUNDLE: OnceLock<NativeLauncherBundle> = OnceLock::new();
    BUNDLE.get_or_init(|| {
        let manifest = std::env::var_os("JETT_ACTOR_GRAPH_BUNDLE_MANIFEST")
            .expect("fresh ordinary 56-export runtime bundle manifest must be supplied");
        NativeLauncherBundle::from_manifest(Path::new(&manifest), !cfg!(debug_assertions))
            .expect("fresh ordinary runtime bundle matching this test profile")
    })
}
fn digest(bytes: &[u8]) -> String {
    jett_runtime::crypto::sha256_digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn run_bounded(binary: &Path, cwd: &Path) -> (std::process::Output, bool) {
    let mut out = tempfile::NamedTempFile::new().unwrap();
    let mut err = tempfile::NamedTempFile::new().unwrap();
    let mut child = Command::new(binary)
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(out.reopen().unwrap())
        .stderr(err.reopen().unwrap())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > Duration::from_secs(20) {
            timed_out = true;
            let _ = child.kill();
            break child.wait().unwrap();
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = Vec::new();
    out.read_to_end(&mut stdout).unwrap();
    let mut stderr = Vec::new();
    err.read_to_end(&mut stderr).unwrap();
    (
        std::process::Output {
            status,
            stdout,
            stderr,
        },
        timed_out,
    )
}
#[test]
fn actor_graph_public_driver_reference_and_native_without_source() {
    let mut rows = Vec::new();
    let selected = std::env::var("JETT_ACTOR_GRAPH_CASE").ok();
    for (name, text) in [
        ("01_original_escape", "1\n"),
        ("02_shared_fresh", "true false\n8 5\n5\n5\n"),
        (
            "03_nested_order",
            "list(actor#1, actor#1, actor#3)\nlist(actor#5, actor#5, actor#7)\n",
        ),
        ("04_primitive_callback", "baked\n"),
        (
            "05_pending_graph",
            "actor#0 pending(actor#0) pending(pending(actor#0))\nactor#0 actor#0\n",
        ),
        ("06_callback_list", "left right\n"),
        ("09_enum_carrier", "7\n"),
        ("10_map_shared", "3\n3\n"),
        ("11_optional_result", "true 4\n"),
        ("12_adapted_callback", "5\n5\n"),
        ("13_adapted_callback_production", "5\n5\n"),
    ] {
        if selected.as_deref().is_some_and(|selected| selected != name) {
            continue;
        }
        let production_output = name == "13_adapted_callback_production";
        let source_policies: &[bool] = if production_output {
            &[false, true]
        } else {
            &[false]
        };
        for &release in source_policies {
            let mut directory = evidence().join("native").join(profile()).join(name);
            if production_output {
                directory = directory.join(if release {
                    "release-source"
                } else {
                    "debug-source"
                });
            }
            fs::create_dir_all(&directory).unwrap();
            let source = directory.join("main.jett");
            fs::copy(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/actor_graph/fixtures")
                    .join(format!("{name}.jett")),
                &source,
            )
            .unwrap();
            let reference = jett_driver::run_file_capture_output(&source)
                .unwrap_or_else(|error| panic!("{name} reference: {error:?}"));
            let printed: String = reference
                .debug_events
                .iter()
                .filter(|e| {
                    matches!(
                        e.kind,
                        jett_driver::DebugEventKind::Print | jett_driver::DebugEventKind::Println
                    )
                })
                .map(|e| e.text.as_str())
                .collect();
            if production_output {
                assert_eq!(reference.stdout, text, "{name} selected semantics");
                assert!(reference.debug_events.is_empty());
            } else {
                assert_eq!(printed, text, "{name} selected semantics");
                assert!(reference.stdout.is_empty());
            }
            let object = jett_driver::native::emit_host_program_object_for_file_with_options(
                &source,
                jett_driver::BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("{name} object: {error}"));
            fs::write(directory.join("final.obj"), object.bytes()).unwrap();
            let binary = directory.join(if cfg!(windows) { "graph.exe" } else { "graph" });
            jett_driver::native::link_host_object(&object, launcher(), &binary)
                .unwrap_or_else(|error| panic!("{name} link: {error}"));
            fs::remove_file(&source).unwrap();
            let executable_bytes = fs::read(&binary).unwrap();
            let executable_sha256 = digest(&executable_bytes);
            let executable_length = executable_bytes.len();
            drop(executable_bytes);
            let (native, timed_out) = run_bounded(&binary, &directory);
            fs::write(directory.join("native.stdout"), &native.stdout).unwrap();
            fs::write(directory.join("native.stderr"), &native.stderr).unwrap();
            let row = serde_json::json!({"name":name,"runtime_profile":profile(),"source_release_policy":release,"source_removed":!source.exists(),"reference_stdout":reference.stdout,"reference_prints":printed,"reference_events":format!("{:?}",reference.debug_events),"object_bytes":object.bytes().len(),"object_sha256":digest(object.bytes()),"executable_bytes":executable_length,"executable_sha256":executable_sha256,"native_exit":native.status.code(),"native_timed_out":timed_out,"native_terminal_reaped":true,"native_stdout":String::from_utf8_lossy(&native.stdout),"native_stderr":String::from_utf8_lossy(&native.stderr)});
            rows.push(row);
            fs::write(
                evidence().join(format!("native-{}.json", profile())),
                serde_json::to_string_pretty(&rows).unwrap(),
            )
            .unwrap();
            fs::remove_file(&binary).unwrap();
            assert!(
                !binary.exists(),
                "{name} case executable removed after reap"
            );
            assert!(!timed_out, "{name} graph native timeout");
            assert!(native.status.success(), "{name} native: {native:?}");
            assert_eq!(native.stdout, reference.stdout.as_bytes(), "{name}");
            assert_eq!(
                native.stderr,
                jett_driver::render_debug_events(&reference.debug_events).as_bytes(),
                "{name}"
            );
        }
    }
    assert!(
        !rows.is_empty(),
        "selected graph control must exist in the unchanged source table"
    );
}

#[test]
fn actor_graph_map_lowering_observation() {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/actor_graph/fixtures/10_map_shared.jett");
    let lowered = jett_driver::lower_file_for_backend(&source)
        .expect("actual checked map control reaches backend lowering");
    let selected = |identity: &jett_hir::FunctionIdentity| {
        identity.declaration.namespace == "app"
            || (identity.declaration.namespace == "map" && identity.declaration.name == "values")
    };
    let rows = lowered.hir.functions.iter().filter(|function| selected(&function.identity)).map(|function| {
        let current = lowered.mir.functions.iter().find(|current| current.id == function.id).unwrap();
        let loops=current.blocks.iter().filter_map(|block|match &block.terminator.kind {
            jett_mir::TerminatorKind::ForEach {key,value,by_view,iterable,body,exit}=>Some(serde_json::json!({"header":block.id.index(),"key":key.index(),"value":value.map(|value|value.index()),"by_view":by_view,"iterable":expression_fact(iterable),"body":body.index(),"exit":exit.index()})),
            _=>None,
        }).collect::<Vec<_>>();
        serde_json::json!({"id":function.id.index(),"identity":format!("{:?}",function.identity),"hir_params":format!("{:#?}",function.params),"hir_locals":format!("{:#?}",function.locals),"hir_body":block_fact(&function.body),"mir_params":format!("{:#?}",current.params),"mir_locals":format!("{:#?}",current.locals),"mir_unprepared_loops":loops})
    }).collect::<Vec<_>>();
    fs::create_dir_all(evidence()).unwrap();
    fs::write(
        evidence().join("map-lowering-observation.json"),
        serde_json::to_string_pretty(&rows).unwrap(),
    )
    .unwrap();
    // This collector asserts lowering only; its retained ForEach is not an object/native acceptance.
}

fn expression_fact(value: &jett_hir::Expression) -> serde_json::Value {
    use jett_hir::ExpressionKind as E;
    let detail = match &value.kind {
        E::Local(local) => serde_json::json!({"local":local.index()}),
        E::Call {
            function,
            args,
            evaluation_order,
            ownership,
        } => {
            let source = if let jett_hir::CallOwnership::Source(source) = ownership {
                Some(
                    serde_json::json!({"bridge":format!("{:?}",source.bridge),"target":format!("{:?}",source.target),"arguments":source.arguments.iter().map(|argument|serde_json::json!({"parameter":argument.parameter_index,"actual_type":argument.actual_type.index(),"parameter_type":argument.parameter_type.index(),"staging":format!("{:?}",argument.staging)})).collect::<Vec<_>>() }),
                )
            } else {
                None
            };
            serde_json::json!({"function":function.index(),"args":args.iter().map(expression_fact).collect::<Vec<_>>(),"evaluation_order":evaluation_order,"source":source})
        }
        E::Intrinsic {
            intrinsic,
            args,
            type_arguments,
            evaluation_order,
            ..
        } => {
            serde_json::json!({"intrinsic":format!("{intrinsic:?}"),"args":args.iter().map(expression_fact).collect::<Vec<_>>(),"type_arguments":type_arguments.iter().map(|ty|ty.index()).collect::<Vec<_>>(),"evaluation_order":evaluation_order})
        }
        E::View(inner) | E::Clone(inner) => serde_json::json!({"inner":expression_fact(inner)}),
        // Never recursively format the private graph origin or Source witnesses.
        // The collector records bounded operand/occurrence facts, not new proof.
        _ => serde_json::Value::Null,
    };
    serde_json::json!({"kind":format!("{:?}",std::mem::discriminant(&value.kind)),"type":value.ty.index(),"span":format!("{:?}",value.span),"detail":detail})
}
fn block_fact(block: &jett_hir::Block) -> serde_json::Value {
    use jett_hir::StatementKind as H;
    serde_json::json!(block.statements.iter().map(|statement|match &statement.kind {
        H::Let {local,value}=>serde_json::json!({"kind":"Let","local":local.index(),"value":expression_fact(value)}),
        H::Assign {target,value}=>serde_json::json!({"kind":"Assign","target":expression_fact(target),"value":expression_fact(value)}),
        H::Return(value)=>serde_json::json!({"kind":"Return","value":value.as_ref().map(expression_fact)}),
        H::For {key,value,by_view,iterable,body}=>serde_json::json!({"kind":"For","key":key.index(),"value":value.map(|value|value.index()),"by_view":by_view,"iterable":expression_fact(iterable),"body":block_fact(body)}),
        H::Scope(body)=>serde_json::json!({"kind":"Scope","body":block_fact(body)}),
        _=>serde_json::json!({"kind":format!("{:?}",std::mem::discriminant(&statement.kind))}),
    }).collect::<Vec<_>>())
}
