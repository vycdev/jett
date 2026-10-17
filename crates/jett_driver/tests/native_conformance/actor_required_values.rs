//! The unchanged actor required callback through ordinary measured launchers.
//! Output observations do not establish physical lifetime or reentry acceptance.
use super::*;

const SOURCE: &[u8] = include_bytes!(
    "../../../jett_comptime/src/resource_execution/fixtures/actor_handler_required_callable.jett"
);
const SOURCE_SHA256: &str = "ce01b25e20a4955067e87fbc7389f1957b21fd10de6e76575e337f2f98111665";
const EVIDENCE_ENV: &str = "JETT_ACTOR_REQUIRED_NATIVE_EVIDENCE_V1";

fn digest(bytes: &[u8]) -> String {
    jett_runtime::crypto::sha256_digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn artifact(path: &Path) -> serde_json::Value {
    let bytes = fs::read(path).unwrap();
    serde_json::json!({ "path": path, "bytes": bytes.len(), "sha256": digest(&bytes) })
}

#[test]
#[ignore = "requires explicitly supplied measured Debug/Release normal runtime bundles"]
fn actor_handler_required_callable_executes_unchanged_without_source_in_both_modes() {
    assert_eq!(digest(SOURCE), SOURCE_SHA256);
    assert!(
        std::env::var_os("JETT_RESOURCE_NATIVE_EVIDENCE_DIR_V1").is_none(),
        "global executable/object retention must stay disabled"
    );
    let evidence = std::path::PathBuf::from(
        std::env::var_os(EVIDENCE_ENV).expect("fresh bounded actor required evidence file"),
    );
    assert!(evidence.is_absolute() && evidence.parent().unwrap().is_dir());
    assert!(!evidence.exists(), "never reuse a prior observation file");
    let mut rows = Vec::new();
    for release in [false, true] {
        let profile = if release { "release" } else { "debug" };
        // A complete pair is mandatory. There is no Cargo or unmeasured archive
        // fallback for this opt-in observation.
        let launcher = measured_launcher::supplied(release)
            .expect("explicitly supplied normal Debug/Release runtime bundle pair");
        let manifest_name = if release {
            "JETT_NATIVE_TEST_RELEASE_BUNDLE_MANIFEST"
        } else {
            "JETT_NATIVE_TEST_DEBUG_BUNDLE_MANIFEST"
        };
        let manifest = std::path::PathBuf::from(std::env::var_os(manifest_name).unwrap());
        let manifest_before = artifact(&manifest);
        let archive_before = artifact(&launcher.archive_path);
        let directory = tempfile::tempdir().unwrap();
        let directory_path = directory.path().to_path_buf();
        let source = directory.path().join("main.jett");
        fs::write(&source, SOURCE).unwrap();
        assert_eq!(fs::read(&source).unwrap(), SOURCE);
        let reference = jett_driver::run_file_capture_outcome(&source)
            .unwrap_or_else(|failure| panic!("{profile} reference: {failure:?}"));
        assert_eq!(reference.stdout, "observed:5\n");
        assert!(reference.debug_events.is_empty());
        assert!(reference.frontend_debug_observations.is_empty());
        let object = jett_driver::native::emit_host_program_object_for_file_with_options(
            &source,
            jett_driver::BuildOptions { release },
        )
        .unwrap_or_else(|error| panic!("{profile} original object emission: {error}"));
        assert_eq!(object.source().as_bytes(), SOURCE);
        assert!(object.debug_observations().is_empty());
        let object_fact = serde_json::json!({
            "bytes": object.bytes().len(), "sha256": digest(object.bytes()),
            "target": object.target(), "entry": object.program_entry().index(),
            "symbols": object.symbols(),
        });
        let binary = directory
            .path()
            .join(if cfg!(windows) { "actor.exe" } else { "actor" });
        let linked = jett_driver::native::link_host_object(&object, launcher, &binary)
            .unwrap_or_else(|error| panic!("{profile} ordinary measured launcher link: {error}"));
        assert!(linked.debug_observations.is_empty());
        assert_eq!(linked.target, launcher.target);
        let executable_fact = artifact(&binary);
        // The public linker owns and removes its temporary object/PDB directory.
        // Keep only the linked executable in this isolated launch directory.
        drop(object);
        drop(linked);
        drop(reference);
        fs::remove_file(&source).unwrap();
        assert!(!source.exists());
        let remaining = fs::read_dir(directory.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(remaining, vec![binary.clone()]);
        // The shared helper clears the environment, spools output to files,
        // enforces its deadline and kills/waits in Drop even on panic.
        let actual = run_bounded(&binary, directory.path());
        fs::remove_file(&binary).unwrap();
        let binary_removed = !binary.exists();
        directory.close().unwrap();
        let directory_removed = !directory_path.exists();
        assert_eq!(artifact(&manifest), manifest_before);
        assert_eq!(artifact(&launcher.archive_path), archive_before);
        rows.push(serde_json::json!({
            "runtime_profile": profile, "source_release_policy": release,
            "source_sha256": SOURCE_SHA256,
            "runtime_manifest": manifest_before, "runtime_archive": archive_before,
            "runtime_abi_version": launcher.runtime_abi_version,
            "runtime_target": launcher.target, "runtime_crt_mode": launcher.crt_mode.to_string(),
            "object": object_fact, "executable": executable_fact,
            "source_deleted_before_launch": true,
            "isolated_directory_contained_only_executable_before_launch": true,
            "object_host_value_dropped_before_launch": true,
            "native_exit": actual.status.code(), "native_terminal_reaped": true,
            "native_stdout_bytes": actual.stdout, "native_stderr_bytes": actual.stderr,
            "executable_removed_after_reap": binary_removed,
            "temporary_directory_removed": directory_removed,
            "reference_policy": "debug/default interpreter; exact literal output also asserted",
            "physical_lifetime_measured": false, "reentry_measured": false,
            "full_native_acceptance": false,
        }));
        fs::write(
            &evidence,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "actor-required-native-execution-v1", "rows": rows,
                "full_native_acceptance": false,
                "scope": "unchanged whole Source paired ordinary native output observations only",
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(binary_removed && directory_removed);
        assert_eq!(actual.status.code(), Some(0), "{profile}: {actual:?}");
        assert_eq!(actual.stdout, b"observed:5\n", "{profile}: {actual:?}");
        assert!(actual.stderr.is_empty(), "{profile}: {actual:?}");
        eprintln!(
            "Actor required native observation: profile={profile}; source_deleted=true; exit=0; stdout=observed:5; stderr_bytes=0; terminal_reaped=true; executable_removed=true"
        );
    }
    assert_eq!(rows.len(), 2);
    eprintln!(
        "Actor required native observations: 1 unchanged Source; 2 objects; 2 Source-deleted executions; profiles=debug,release; physical_lifetime_measured=false; reentry_measured=false; full_native_acceptance=false"
    );
}
