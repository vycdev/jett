//! Original callback Sources through reference and all three native purposes.
use super::*;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
enum Purpose {
    Program,
    Verify,
    Property,
}

impl Purpose {
    fn name(self) -> &'static str {
        match self {
            Self::Program => "program",
            Self::Verify => "verify",
            Self::Property => "property",
        }
    }

    fn emit(
        self,
        source: &Path,
    ) -> Result<
        jett_driver::native::NativeProgramObjectArtifact,
        jett_driver::native::NativeBuildError,
    > {
        let options = jett_driver::BuildOptions { release: false };
        match self {
            Self::Program => {
                jett_driver::native::emit_host_program_object_for_file_with_options(source, options)
            }
            Self::Verify => {
                jett_driver::native::emit_host_verify_suite_object_for_file_with_options(
                    source, options,
                )
            }
            Self::Property => {
                jett_driver::native::emit_host_property_suite_object_for_file_with_options(
                    source, options,
                )
            }
        }
    }
}

struct Original {
    name: &'static str,
    source: &'static str,
    verify: &'static str,
    property: &'static str,
}

const SOURCES: &[Original] = &[
    Original {
        name: "named_required_suites",
        source: include_str!("reflected_callable_sources/19_generated_named_suites.jett"),
        verify: "lazy_callback_result",
        property: "lazy_callback_trials",
    },
    Original {
        name: "captured_suites",
        source: include_str!("reflected_callable_sources/20_generated_captured_suites.jett"),
        verify: "captured_callback_result",
        property: "captured_callback_trials",
    },
];

fn digest(bytes: &[u8]) -> String {
    jett_runtime::crypto::sha256_digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn original_sources_keep_both_reference_blocks_and_one_hundred_property_trials() {
    for original in SOURCES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(&source, original.source).unwrap();
        let frontend = jett_driver::build_file_with_options(
            &source,
            jett_driver::BuildOptions { release: false },
        );
        assert!(
            !frontend.has_errors,
            "{}: {:?}",
            original.name, frontend.diagnostics
        );
        let reference = jett_driver::test_file_capture_outcome(&source)
            .unwrap_or_else(|error| panic!("{}: {error:?}", original.name));
        assert_eq!(
            (reference.total, reference.passed, reference.failed),
            (2, 2, 0)
        );
        assert_eq!(reference.blocks.len(), 2);
        assert_eq!(reference.blocks[0].name, original.verify);
        assert!(!reference.blocks[0].is_property);
        assert!(reference.blocks[0].passed);
        assert_eq!(reference.blocks[1].name, original.property);
        assert!(reference.blocks[1].is_property && reference.blocks[1].passed);
        assert_eq!(reference.blocks[1].iterations, Some(100));
        assert!(
            reference
                .blocks
                .iter()
                .all(|block| block.debug_events.is_empty())
        );
        let main = jett_driver::run_file_capture_output(&source)
            .unwrap_or_else(|error| panic!("{} reference main: {error}", original.name));
        assert!(main.stdout.is_empty());
        assert_eq!(jett_driver::render_debug_events(&main.debug_events), "5\n");
    }
}

#[test]
fn original_sources_emit_all_six_program_verify_property_objects() {
    let mut objects = 0;
    for original in SOURCES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(&source, original.source).unwrap();
        for purpose in [Purpose::Program, Purpose::Verify, Purpose::Property] {
            let object = purpose
                .emit(&source)
                .unwrap_or_else(|error| panic!("{} {}: {error}", original.name, purpose.name()));
            assert!(!object.bytes().is_empty());
            objects += 1;
        }
    }
    assert_eq!(objects, 6);
}

#[test]
fn original_sources_delete_source_before_twelve_normal_profile_executions() {
    let evidence = std::env::var_os("JETT_REFLECTED_CALLABLE_SUITE_EVIDENCE").map(PathBuf::from);
    if let Some(root) = &evidence {
        assert!(root.is_absolute());
        fs::create_dir(root).expect("fresh callback suite evidence directory");
    }
    let mut object_count = 0;
    let mut execution_count = 0;
    for original in SOURCES {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::write(&source, original.source).unwrap();
        let mut objects = Vec::new();
        for purpose in [Purpose::Program, Purpose::Verify, Purpose::Property] {
            let object = purpose
                .emit(&source)
                .unwrap_or_else(|error| panic!("{} {}: {error}", original.name, purpose.name()));
            let case = evidence
                .as_ref()
                .map(|root| root.join(format!("{}-{}", original.name, purpose.name())));
            if let Some(case) = &case {
                fs::create_dir(case).unwrap();
                fs::write(case.join("object.obj"), object.bytes()).unwrap();
            }
            object_count += 1;
            objects.push((purpose, object, case));
        }
        fs::remove_file(&source).unwrap();
        assert!(!source.exists());
        for (purpose, object, case) in objects {
            for release in [false, true] {
                let profile = if release { "release" } else { "debug" };
                let executable = directory
                    .path()
                    .join(format!("{}-{profile}.exe", purpose.name()));
                jett_driver::native::link_host_object(
                    &object,
                    suite_options::launcher_for_options(release),
                    &executable,
                )
                .unwrap();
                let executable_sha256 = digest(&fs::read(&executable).unwrap());
                assert!(!source.exists());
                let output = run_bounded(&executable, directory.path());
                // The existing bounded runner reaps before returning. Retire
                // only this one temporary executable, even on oracle failure.
                fs::remove_file(&executable).unwrap();
                assert!(!executable.exists());
                if let Some(case) = &case {
                    fs::write(case.join(format!("{profile}-stdout.bin")), &output.stdout).unwrap();
                    fs::write(case.join(format!("{profile}-stderr.bin")), &output.stderr).unwrap();
                    let observation = serde_json::json!({
                        "case": original.name, "purpose": purpose.name(),
                        "runtime_profile": profile, "source_build_release": false,
                        "source_sha256": digest(original.source.as_bytes()),
                        "source_deleted_before_launch": !source.exists(),
                        "object_sha256": digest(object.bytes()),
                        "object_entry": object.program_entry().index(),
                        "object_target": object.target(),
                        "executable_sha256": executable_sha256,
                        "executable_reaped_and_removed": true,
                        "status": output.status.code(), "stdout": output.stdout,
                        "stderr": output.stderr,
                        "public_ordinary_context_retirement_observation": null,
                        "retirement_observation_available": false
                    });
                    fs::write(
                        case.join(format!("{profile}-observation.json")),
                        serde_json::to_vec_pretty(&observation).unwrap(),
                    )
                    .unwrap();
                }
                assert_eq!(
                    output.status.code(),
                    Some(0),
                    "{} {} {profile}: {output:?}",
                    original.name,
                    purpose.name()
                );
                assert!(
                    output.stdout.is_empty(),
                    "{} {} {profile}: {output:?}",
                    original.name,
                    purpose.name()
                );
                assert_eq!(
                    output.stderr,
                    if matches!(purpose, Purpose::Program) {
                        b"5\n".as_slice()
                    } else {
                        b"".as_slice()
                    },
                    "{} {} {profile}",
                    original.name,
                    purpose.name()
                );
                execution_count += 1;
            }
        }
    }
    assert_eq!((object_count, execution_count), (6, 12));
    println!(
        "callback suites: 2 unchanged Sources, 6 objects, 12 Source-deleted normal profile executions"
    );
}
