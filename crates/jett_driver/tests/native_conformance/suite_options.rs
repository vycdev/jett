use super::run_bounded;
use jett_driver::native::*;
use jett_driver::{BackendLoweringError, BuildOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

const SUITE: &str = r#"namespace app
function checked(value: bool) returns bool:
    string marker = "suite"
    trace marker
    breakpoint false
    return value == value
function main() returns nothing:
    return nothing
verify checked_values:
    assert checked(true)
property checked_trials:
    given chosen: bool
    assert checked(chosen)
"#;

const POLICY: &str = r#"namespace app
function debug_helper() returns nothing:
    println("debug")
function main() returns nothing:
    return nothing
verify debug_source:
    assert true
property debug_trials:
    given chosen: bool
    assert chosen == chosen
"#;

const OPTIMIZATION: &str = r#"namespace app
function identity(value: int64) returns int64:
    mutable int64 total = value
    for offset in list(0, 1, 2, 3, 4, 5, 6, 7):
        total = total + offset
    return total - 28
function main() returns nothing:
    return nothing
verify arithmetic:
    assert identity(12) == 12
property arithmetic_trials:
    given input: int64
    assert identity(input) == input
"#;

struct ProfileLauncher {
    bundle: NativeLauncherBundle,
    _directory: tempfile::TempDir,
}

/// Suite options select Jett source/optimization policy. The caller separately
/// supplies an archive compiled for that runtime profile, as the public API requires.
pub(super) fn launcher_for_options(release: bool) -> &'static NativeLauncherBundle {
    static DEBUG: OnceLock<ProfileLauncher> = OnceLock::new();
    static RELEASE: OnceLock<ProfileLauncher> = OnceLock::new();
    let slot = if release { &RELEASE } else { &DEBUG };
    &slot
        .get_or_init(|| {
            let executable = std::env::current_exe().expect("test executable");
            let target = executable
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("native-values-launcher");
            let host = host_target();
            let profile = if release { "release" } else { "debug" };
            let cargo_profile = if release { "release" } else { "test" };
            let status = Command::new(env!("CARGO"))
                .args([
                    "build",
                    "-q",
                    "-p",
                    "jett_native_launcher",
                    "--target",
                    &host,
                    "--profile",
                    cargo_profile,
                    "--target-dir",
                ])
                .arg(&target)
                .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .status()
                .expect("build explicit suite runtime profile");
            assert!(status.success(), "suite runtime build failed: {status}");
            let archive_name = if cfg!(windows) {
                "jett_native_launcher.lib"
            } else {
                "libjett_native_launcher.a"
            };
            let archive = target.join(&host).join(profile).join(archive_name);
            let directory = tempfile::tempdir().unwrap();
            fs::copy(&archive, directory.path().join(archive_name)).unwrap();
            let template = if cfg!(windows) {
                NativeLauncherBundle::windows_msvc_static_v1(directory.path().join(archive_name))
            } else {
                NativeLauncherBundle::linux_gnu_v1(directory.path().join(archive_name))
            };
            let manifest = directory.path().join("runtime.json");
            fs::write(
                &manifest,
                serde_json::to_vec(&serde_json::json!({
                    "manifest_version": 1,
                    "compiler_version": env!("CARGO_PKG_VERSION"),
                    "profile": profile,
                    "archive": archive_name,
                    "runtime_abi_version": template.runtime_abi_version,
                    "crt_mode": template.crt_mode.to_string(),
                    "target": template.target,
                    "native_library_args": template.native_library_args.iter()
                        .map(|arg| arg.to_str().unwrap()).collect::<Vec<_>>(),
                }))
                .unwrap(),
            )
            .unwrap();
            let bundle = NativeLauncherBundle::from_manifest(&manifest, release).unwrap();
            assert!(
                NativeLauncherBundle::from_manifest(&manifest, !release)
                    .unwrap_err()
                    .contains("profile mismatch")
            );
            ProfileLauncher {
                bundle,
                _directory: directory,
            }
        })
        .bundle
}

#[test]
fn native_suite_options_preserve_defaults_and_select_source_and_runtime_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, SUITE).unwrap();
    let mut binaries: Vec<(PathBuf, bool, usize)> = Vec::new();
    for property in [false, true] {
        let default = if property {
            emit_host_property_suite_object_for_file(&source).unwrap()
        } else {
            emit_host_verify_suite_object_for_file(&source).unwrap()
        };
        let explicit = if property {
            emit_host_property_suite_object_for_file_with_options(&source, BuildOptions::default())
                .unwrap()
        } else {
            emit_host_verify_suite_object_for_file_with_options(&source, BuildOptions::default())
                .unwrap()
        };
        assert_eq!(
            default.bytes(),
            explicit.bytes(),
            "default suite object changed"
        );
        let default_binary = directory.path().join(format!("default_{property}.exe"));
        if property {
            build_host_property_suite_executable(
                &source,
                launcher_for_options(false),
                &default_binary,
            )
            .unwrap();
        } else {
            build_host_verify_suite_executable(
                &source,
                launcher_for_options(false),
                &default_binary,
            )
            .unwrap();
        }
        binaries.push((default_binary, false, if property { 100 } else { 1 }));
        for release in [false, true] {
            let binary = directory
                .path()
                .join(format!("suite_{property}_{release}.exe"));
            if property {
                build_host_property_suite_executable_with_options(
                    &source,
                    launcher_for_options(release),
                    &binary,
                    BuildOptions { release },
                )
                .unwrap();
            } else {
                build_host_verify_suite_executable_with_options(
                    &source,
                    launcher_for_options(release),
                    &binary,
                    BuildOptions { release },
                )
                .unwrap();
            }
            binaries.push((binary, release, if property { 100 } else { 1 }));
        }
    }
    fs::remove_file(&source).unwrap();
    for (binary, release, trials) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        let expected = if release {
            String::new()
        } else {
            "trace marker: string = suite\n".repeat(trials)
        };
        assert_eq!(actual.stderr, expected.as_bytes(), "{actual:?}");
    }
}

#[test]
fn native_suite_release_policy_rejects_debug_print_before_publication() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, POLICY).unwrap();
    for property in [false, true] {
        let default = if property {
            jett_driver::lower_file_for_native_property_suite(&source)
        } else {
            jett_driver::lower_file_for_native_verify_suite(&source)
        };
        default.expect("default debug suite still admits debug printing");
        let rejected = if property {
            jett_driver::lower_file_for_native_property_suite_with_options(
                &source,
                BuildOptions { release: true },
            )
        } else {
            jett_driver::lower_file_for_native_verify_suite_with_options(
                &source,
                BuildOptions { release: true },
            )
        };
        let Err(BackendLoweringError::Build(result)) = rejected else {
            panic!("release policy did not reject debug print")
        };
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| u32::from(d.code.code()) == 362)
        );
        let output = directory.path().join(format!("preserved_{property}.exe"));
        fs::write(&output, b"existing output").unwrap();
        let error = if property {
            build_host_property_suite_executable_with_options(
                &source,
                launcher_for_options(true),
                &output,
                BuildOptions { release: true },
            )
        } else {
            build_host_verify_suite_executable_with_options(
                &source,
                launcher_for_options(true),
                &output,
                BuildOptions { release: true },
            )
        }
        .unwrap_err();
        assert!(matches!(error, NativeBuildError::Lowering { .. }));
        assert_eq!(fs::read(&output).unwrap(), b"existing output");
    }
}

#[test]
fn native_suite_release_objects_use_optimized_codegen() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, OPTIMIZATION).unwrap();
    for property in [false, true] {
        let lowered = if property {
            jett_driver::lower_file_for_native_property_suite_with_options(
                &source,
                BuildOptions { release: true },
            )
            .unwrap()
        } else {
            jett_driver::lower_file_for_native_verify_suite_with_options(
                &source,
                BuildOptions { release: true },
            )
            .unwrap()
        };
        let entry = if property {
            lowered.native_property_entry
        } else {
            lowered.native_verify_entry
        }
        .unwrap();
        let emit = |optimize| {
            jett_codegen_cranelift::emit_host_program_object_with_options(
                &lowered.mir,
                &lowered.interner,
                entry,
                jett_codegen_cranelift::CodegenOptions { optimize },
            )
            .unwrap()
            .bytes
        };
        let optimized = emit(true);
        assert_ne!(
            optimized,
            emit(false),
            "fixture must distinguish optimization"
        );
        let actual = if property {
            emit_host_property_suite_object_for_file_with_options(
                &source,
                BuildOptions { release: true },
            )
            .unwrap()
        } else {
            emit_host_verify_suite_object_for_file_with_options(
                &source,
                BuildOptions { release: true },
            )
            .unwrap()
        };
        assert_eq!(actual.bytes(), optimized);
    }
}
