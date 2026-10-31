//! Optional small evidence rows; this never copies linked programs or objects.
//! A final observation.json appears only after the existing oracles and disposal.
use super::{Archive, cases, digest, report};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Output,
};

pub(super) const ENV: &str = "JETT_RESOURCE_SUM_NATIVE_OBSERVATIONS_DIR_V1";

fn selected_root() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os(ENV)?);
    assert!(
        root.is_absolute(),
        "lightweight evidence root must be absolute"
    );
    let metadata = fs::symlink_metadata(&root).unwrap();
    assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
    Some(root)
}

pub(super) fn assert_fresh_root() {
    if let Some(root) = selected_root() {
        assert!(
            std::env::var_os("JETT_RESOURCE_NATIVE_EVIDENCE_DIR_V1").is_none(),
            "lightweight observations exclude copy-all evidence"
        );
        assert!(
            fs::read_dir(root).unwrap().next().is_none(),
            "use an empty fresh evidence root"
        );
    }
}

fn absent(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => panic!("observe {}: {error}", path.display()),
    }
}

fn file_identity(path: &Path) -> serde_json::Value {
    let metadata = fs::metadata(path).unwrap();
    assert!(metadata.is_file() && metadata.len() <= 256 * 1024 * 1024);
    let bytes = fs::read(path).unwrap();
    serde_json::json!({"path": path, "bytes": bytes.len(), "sha256": digest(&bytes)})
}

fn write_new(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
}

pub(super) struct Capture {
    destination: PathBuf,
    execution_root: PathBuf,
    compilation_root: PathBuf,
    metadata: serde_json::Value,
    payloads_saved: bool,
}

impl Capture {
    pub(super) fn prepare(
        mode: &str,
        case: &cases::Case,
        ready: Option<&cases::Case>,
        support: &str,
        archive: &Archive,
        object: &Path,
        binary: &Path,
        execution_root: &Path,
        compilation_root: &Path,
        build_ordinal: usize,
        freshly_built: bool,
    ) -> Option<Self> {
        let root = selected_root()?;
        assert!(matches!(mode, "single" | "reentry"));
        assert!(
            case.name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        );
        assert!(absent(&compilation_root.join("project")));
        assert!(absent(&compilation_root.join("synthetic-stdlib")));
        assert!(absent(&compilation_root.join("project/main.jett")));
        assert!(absent(
            &compilation_root.join("synthetic-stdlib/resource_probe.jett")
        ));
        assert!(execution_root.is_dir());
        let destination = root.join(format!("{mode}-{}-{}", archive.profile, case.name));
        fs::create_dir(&destination).unwrap();
        let metadata = serde_json::json!({
            "schema": "resource-sum-lightweight-native-observation-v1",
            "mode": mode,
            "case": case.name,
            "ready_case": ready.map(|case| case.name),
            "source_sha256": digest(case.source.as_bytes()),
            "support_sha256": digest(support.as_bytes()),
            "ready_source_sha256": ready.map(|case| digest(case.source.as_bytes())),
            "profile": archive.profile,
            "checked_release": archive.profile == "release",
            "target": archive.target,
            "linked_program_build_ordinal": build_ordinal,
            "build_ordinal_scope": if mode == "single" { "profile" } else { "child" },
            "freshly_built_for_case": freshly_built,
            "runtime_abi": 1,
            "layout_wire": 3,
            "validated_archive_receipt_sha256": super::RECEIPT_SHA256,
            "archive": {"path": archive.path, "sha256": archive.archive_sha256},
            "object_before_launch": file_identity(object),
            "executable_before_launch": file_identity(binary),
            "execution_cwd": execution_root,
            "compilation_directory": compilation_root,
            "source_and_support_absent_before_launch": true,
            "copies_objects_or_executables": false,
            "physical_storage_measured": false,
            "checked_program_weak_release_measured": false,
            "full_acceptance": false,
        });
        Some(Self {
            destination,
            execution_root: execution_root.to_owned(),
            compilation_root: compilation_root.to_owned(),
            metadata,
            payloads_saved: false,
        })
    }

    // Retain raw process bytes before any report decode or case assertion can fail.
    // Without the final metadata such a row remains incomplete, never accepted.
    pub(super) fn save_output(&mut self, bytes: &[u8], actual: &Output) {
        assert!(!self.payloads_saved);
        for (name, payload) in [
            ("report.bin", bytes),
            ("stdout.bin", actual.stdout.as_slice()),
            ("stderr.bin", actual.stderr.as_slice()),
        ] {
            write_new(&self.destination.join(name), payload);
            self.metadata[name] = serde_json::json!({
                "bytes": payload.len(), "sha256": digest(payload),
            });
        }
        self.payloads_saved = true;
    }

    pub(super) fn finish(mut self, observed: &report::Report, actual: &Output) {
        assert!(self.payloads_saved);
        assert!(
            absent(&self.execution_root),
            "execution TempDir.close must succeed first"
        );
        self.metadata["process"] = serde_json::json!({
            "bounded_runner_returned": true,
            "exit_code": actual.status.code(),
            "success": actual.status.success(),
            "status_debug": format!("{:?}", actual.status),
        });
        self.metadata["execution_tempdir_closed_and_absent"] = serde_json::json!(true);
        self.metadata["compilation_directory_absent_after_run"] =
            serde_json::json!(absent(&self.compilation_root));
        self.metadata["attempt_count"] = serde_json::json!(observed.attempts.len());
        self.metadata["attempts"] = serde_json::json!(
            observed
                .attempts
                .iter()
                .map(|attempt| {
                    serde_json::json!({
                        "id": attempt.completion.attempt.to_string(),
                        "body_status": attempt.completion.body_status,
                        "cleanup_status": attempt.completion.cleanup_status,
                        "channel": format!("{:?}", attempt.completion.selected_channel),
                        "event_sequence_end": attempt.event_sequence_end.to_string(),
                        "resources_retired": attempt.counts.resources_retired(),
                        "ordinary_empty": attempt.counts.ordinary_empty,
                    })
                })
                .collect::<Vec<_>>()
        );
        self.metadata["observations"] = serde_json::json!({
            "flags": observed.observations.flags,
            "counters": observed.observations.counters.values.map(|value| value.to_string()),
            "attempts": observed.observations.attempts.iter().map(|attempt| {
                serde_json::json!({
                    "id": attempt.identity.id.to_string(),
                    "ordinal": attempt.identity.ordinal,
                    "begin_counters": attempt.begin.counters.values.map(|value| value.to_string()),
                    "end_counters": attempt.end.map(|end| end.counters.values.map(|value| value.to_string())),
                })
            }).collect::<Vec<_>>(),
        });
        self.metadata["existing_oracles_and_observer_returned"] = serde_json::json!(true);
        let bytes = serde_json::to_vec_pretty(&self.metadata).unwrap();
        write_new(&self.destination.join("observation.json"), &bytes);
    }
}
