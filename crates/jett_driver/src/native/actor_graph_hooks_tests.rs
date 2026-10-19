//! The exact protected Support constructor feeds the public checked Driver bake.
//! Only this child test can wrap the resulting validated object for the existing
//! public normal linker; no production object/manifest authority is exposed.
use super::*;
use jett_common::{FileId, SourceOrigin};
use jett_parser::{ast::Item, parse};
use jett_resolve::{DefKind, ResourceKernelSpec};
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use jett_types::ResourceKernelRecipe;
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
const SUPPORT: &str =
    include_str!("../../../jett_comptime/src/resource_execution/fixtures/resource_probe.jett");
const INITIALIZER: &str =
    include_str!("../../tests/actor_graph/fixtures/07_nested_hook_actor.jett");
const CAPTURE: &str =
    include_str!("../../tests/actor_graph/fixtures/08_nested_hook_capture_actor.jett");
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
struct Launcher {
    bundle: NativeLauncherBundle,
    _directory: tempfile::TempDir,
}

fn launcher() -> &'static NativeLauncherBundle {
    if let Some(manifest) = std::env::var_os("JETT_ACTOR_GRAPH_BUNDLE_MANIFEST") {
        static BUNDLE: OnceLock<NativeLauncherBundle> = OnceLock::new();
        return BUNDLE.get_or_init(|| {
            NativeLauncherBundle::from_manifest(Path::new(&manifest), !cfg!(debug_assertions))
                .expect("fresh ordinary runtime bundle matching this test profile")
        });
    }
    static LAUNCHER: OnceLock<Launcher> = OnceLock::new();
    &LAUNCHER
        .get_or_init(|| {
            let executable = std::env::current_exe().expect("test executable");
            let profile_directory = executable.parent().unwrap().parent().unwrap();
            let profile = profile_directory.file_name().unwrap().to_str().unwrap();
            let cargo_profile = if profile == "debug" { "test" } else { profile };
            let target = profile_directory
                .parent()
                .unwrap()
                .join("native-values-launcher");
            let host = host_target();
            let status = Command::new(env!("CARGO"))
                .args(["build", "-q", "-p", "jett_native_launcher"])
                .args([
                    "--target",
                    &host,
                    "--profile",
                    cargo_profile,
                    "--target-dir",
                ])
                .arg(&target)
                .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .status()
                .expect("build host- and profile-matched launcher");
            assert!(status.success(), "launcher build failed: {status}");
            let archive_name = if cfg!(windows) {
                "jett_native_launcher.lib"
            } else {
                "libjett_native_launcher.a"
            };
            let archive = target.join(&host).join(profile).join(archive_name);
            let directory = tempfile::tempdir().expect("launcher bundle directory");
            let copied = directory.path().join(archive_name);
            std::fs::copy(&archive, &copied).unwrap_or_else(|error| {
                panic!(
                    "cannot copy launcher archive {}: {error}",
                    archive.display()
                )
            });
            Launcher {
                bundle: if cfg!(windows) {
                    NativeLauncherBundle::windows_msvc_static_v1(copied)
                } else {
                    NativeLauncherBundle::linux_gnu_v1(copied)
                },
                _directory: directory,
            }
        })
        .bundle
}
fn checked(
    source: &str,
    release: bool,
) -> (
    Arc<CheckedResourceProgram>,
    jett_common::Span,
    jett_common::Span,
) {
    let project = FileId::new(0);
    let support = FileId::new(10_000);
    let mut parsed = parse(SUPPORT, support);
    let primary = parse(source, project);
    assert!(parsed.errors.is_empty());
    assert!(primary.errors.is_empty());
    let entries = primary
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function) if function.name.name == "main" => {
                Some((function.span, function.name.span))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [entry] = entries.as_slice() else {
        panic!("unique original main")
    };
    let resources = parsed
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Resource(value) => Some(value.name.span),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [resource] = resources.as_slice() else {
        panic!("unique original protected Resource")
    };
    parsed.module.items.extend(primary.module.items);
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: *resource,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    let value = CheckedResourceProgram::prepare(
        parsed,
        HashMap::from([
            (project, SourceOrigin::Project),
            (support, SourceOrigin::Stdlib),
        ]),
        &catalog,
        CheckOptions { release },
    )
    .unwrap();
    (Arc::new(value), entry.0, entry.1)
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
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(20) {
            timed_out = true;
            let _ = child.kill();
            break child.wait().unwrap();
        }
        thread::sleep(Duration::from_millis(10));
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
fn actor_graph_checked_hook_original_initializer_and_capture_native() {
    let mut rows = Vec::new();
    for (name, source) in [
        ("original_initializer", INITIALIZER),
        ("original_capture", CAPTURE),
    ] {
        for release in [false, true] {
            let (program, span, name_span) = checked(source, release);
            let required = crate::evaluate_checked_resource_required_values(&program);
            assert!(
                required.diagnostics.is_empty(),
                "{name}: {:?}",
                required.diagnostics
            );
            assert!(required.debug_events.is_empty());
            let definitions = program
                .resolved()
                .scope_table
                .definitions
                .iter()
                .filter(|definition| {
                    definition.kind == DefKind::Function && definition.span == name_span
                })
                .collect::<Vec<_>>();
            let [definition] = definitions.as_slice() else {
                panic!("exact checked entry DefId")
            };
            let mut reference = jett_comptime::Interpreter::new();
            reference
                .install_checked_resource_program(
                    program.clone(),
                    jett_comptime::ExecutionPurpose::ReferenceRuntime,
                )
                .unwrap();
            reference.register_module(program.module());
            reference.set_explicit_comptime_values(Arc::new(required.values.clone()));
            assert_eq!(
                reference
                    .call_checked_program_entry(definition.id, Vec::new())
                    .unwrap(),
                jett_comptime::Value::Nothing
            );
            let events = reference.take_debug_events();
            assert!(events.is_empty());
            let mut hir = jett_hir::lower_checked_resource_program(&program).unwrap();
            crate::bake_checked_resource_values(&mut hir, &program, &required.values).unwrap();
            jett_hir::complete_value_conversions(&mut hir, &program.checked().interner).unwrap();
            let entries = hir
                .functions
                .iter()
                .filter(|function| {
                    function.span == span
                        && function.identity.declaration.origin == SourceOrigin::Project
                        && function.identity.declaration.kind == jett_hir::DeclarationKind::Function
                })
                .map(|function| function.id)
                .collect::<Vec<_>>();
            let [entry] = entries.as_slice() else {
                panic!("exact HIR entry")
            };
            let mir = jett_mir::lower(&hir, &program.checked().interner).unwrap();
            jett_mir::validate(&mir).unwrap();
            let emitted = jett_codegen_cranelift::emit_host_program_object_with_options(
                &mir,
                &program.checked().interner,
                *entry,
                jett_codegen_cranelift::CodegenOptions { optimize: release },
            )
            .unwrap();
            let object = NativeProgramObjectArtifact {
                object: native_object(emitted.target, emitted.symbols, emitted.bytes).unwrap(),
                program_entry: *entry,
                diagnostics: Vec::new(),
                source: source.into(),
            };
            let directory = evidence()
                .join("native-hooks")
                .join(profile())
                .join(format!(
                    "{name}-{}",
                    if release {
                        "release-source"
                    } else {
                        "debug-source"
                    }
                ));
            fs::create_dir_all(&directory).unwrap();
            let source_path = directory.join("main.jett");
            let support_path = directory.join("resource_probe.jett");
            fs::write(&source_path, source).unwrap();
            fs::write(&support_path, SUPPORT).unwrap();
            fs::write(directory.join("final.obj"), object.bytes()).unwrap();
            let binary = directory.join(if cfg!(windows) {
                "graph-hook.exe"
            } else {
                "graph-hook"
            });
            link_host_object(&object, launcher(), &binary).unwrap();
            fs::remove_file(source_path).unwrap();
            fs::remove_file(support_path).unwrap();
            let executable_bytes = fs::read(&binary).unwrap();
            let executable_sha256 = digest(&executable_bytes);
            let executable_length = executable_bytes.len();
            drop(executable_bytes);
            let (native, timed_out) = run_bounded(&binary, &directory);
            fs::write(directory.join("native.stdout"), &native.stdout).unwrap();
            fs::write(directory.join("native.stderr"), &native.stderr).unwrap();
            rows.push(serde_json::json!({"name":name,"runtime_profile":profile(),"release_source":release,"required_events":0,"reference_events":format!("{events:?}"),"object_bytes":object.bytes().len(),"object_sha256":digest(object.bytes()),"executable_bytes":executable_length,"executable_sha256":executable_sha256,"source_removed":true,"support_removed":true,"native_exit":native.status.code(),"native_timed_out":timed_out,"native_terminal_reaped":true,"native_stdout":String::from_utf8_lossy(&native.stdout),"native_stderr":String::from_utf8_lossy(&native.stderr)}));
            fs::write(
                evidence().join(format!("native-hooks-{}.json", profile())),
                serde_json::to_string_pretty(&rows).unwrap(),
            )
            .unwrap();
            fs::remove_file(&binary).unwrap();
            assert!(
                !binary.exists(),
                "{name} case executable removed after reap"
            );
            assert!(!timed_out, "{name} graph hook timeout");
            assert!(native.status.success(), "{name}: {native:?}");
            assert!(native.stdout.is_empty());
            assert!(native.stderr.is_empty());
        }
    }
}
