use super::{run_bounded, suite_options::launcher_for_options};
use std::{fs, path::Path};

fn assert_main(name: &str, stdout: &str) {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native")
        .join(name);
    fs::copy(fixture, &source).unwrap();
    let checked = jett_driver::build_file(&source);
    assert!(!checked.has_errors, "{name}: {:?}", checked.diagnostics);
    let expected = jett_driver::run_file_capture_outcome(&source)
        .unwrap_or_else(|failure| panic!("{name}: {}", failure.message));
    assert_eq!(expected.stdout, stdout, "{name}");
    assert!(expected.debug_events.is_empty(), "{name}: {expected:?}");
    assert!(
        expected.frontend_debug_observations.is_empty(),
        "{name}: {expected:?}"
    );
    let binaries = [false, true].map(|release| {
        let binary = directory.path().join(format!("actor-fields-{release}.exe"));
        let artifact = jett_driver::native::build_host_executable_with_options(
            &source,
            launcher_for_options(release),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .unwrap_or_else(|error| panic!("{name}, release={release}: {error}"));
        assert!(
            artifact.debug_observations.is_empty(),
            "{name}: {artifact:?}"
        );
        (binary, release)
    });
    fs::remove_file(&source).unwrap();
    assert!(!source.exists());
    for (binary, release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(
            actual.status.code(),
            Some(0),
            "{name}, release={release}: {actual:?}"
        );
        assert_eq!(
            actual.stdout,
            stdout.as_bytes(),
            "{name}, release={release}"
        );
        assert!(
            actual.stderr.is_empty(),
            "{name}, release={release}: {actual:?}"
        );
    }
}

#[test]
fn native_original_actor_handler_field_update_executes_without_source() {
    assert_main(
        "field_update_reference/24_actor_handler_future.jett",
        "actor:1:9\n",
    );
}

#[test]
fn native_actor_field_updates_preserve_owning_state_and_independent_instances_without_source() {
    assert_main(
        "actor_field_updates/01_nested_owners_independent_actors.jett",
        "first:4:18:left!!:2|1:9:start:9|sender:2:7:left\nafter:4:18:left!!:2|2:7:right:9\n",
    );
}

#[test]
fn native_actor_field_updates_preserve_message_and_helper_ownership_without_source() {
    for (source, stdout) in [
        (
            "actor_field_updates/10_nonshadowing_message_and_helper.jett",
            "message:110|1:initial|9:helper|210|9:helper\n",
        ),
        (
            "actor_field_updates/16_nonresponding_return_preserves_state.jett",
            "void-return:7:changed|7:changed|9:selected|1:initial\n",
        ),
    ] {
        assert_main(source, stdout);
    }
}

#[test]
fn native_actor_early_respond_abandons_field_commit_without_source() {
    assert_main(
        "actor_field_updates/03_early_respond_preserves_old_field.jett",
        "early:4:4:9:9:9\n",
    );
}

#[test]
fn native_actor_field_updates_execute_verify_and_property_suites_without_source() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/native/actor_field_updates/03_early_respond_preserves_old_field.jett");
    fs::copy(fixture, &source).unwrap();
    let reference = jett_driver::test_file(&source).expect("reference actor suites");
    assert_eq!(
        (reference.total, reference.passed, reference.failed),
        (2, 2, 0)
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let launcher = launcher_for_options(release);
        let options = jett_driver::BuildOptions { release };
        let verify = directory.path().join(format!("actor-verify-{release}.exe"));
        let property = directory
            .path()
            .join(format!("actor-property-{release}.exe"));
        jett_driver::native::build_host_verify_suite_executable_with_options(
            &source, launcher, &verify, options,
        )
        .unwrap_or_else(|error| panic!("actor verify release={release}: {error}"));
        jett_driver::native::build_host_property_suite_executable_with_options(
            &source, launcher, &property, options,
        )
        .unwrap_or_else(|error| panic!("actor property release={release}: {error}"));
        binaries.extend([verify, property]);
    }
    fs::remove_file(&source).unwrap();
    assert!(!source.exists());
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(0), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}

#[test]
fn native_actor_field_update_ownership_refusals_preserve_existing_publications() {
    use jett_driver::native::{NativeBuildError, NativeLauncherBundle};
    for name in [
        "02_shadowed_message_and_ordinary_helper.jett",
        "04_consumed_message_root_rejected.jett",
        "05_persistent_state_not_restored_rejected.jett",
        "06_view_message_root_rejected.jett",
        "07_immutable_state_root_rejected.jett",
        "08_live_whole_state_loan_rejected.jett",
        "09_live_state_field_loan_rejected.jett",
        "12_persistent_alias_owner_replacement_rejected.jett",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.jett");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/native/actor_field_updates")
                .join(name),
            &source,
        )
        .unwrap();
        let output = directory.path().join("preserved.exe");
        let sentinel = b"previous checked actor publication";
        fs::write(&output, sentinel).unwrap();
        let launcher = if cfg!(windows) {
            NativeLauncherBundle::windows_msvc_static_v1(directory.path().join("unused.lib"))
        } else {
            NativeLauncherBundle::linux_gnu_v1(directory.path().join("unused.a"))
        };
        let assert_refusal = |error: NativeBuildError| {
            let NativeBuildError::Lowering { source, .. } = error else {
                panic!("{name}: expected Source rejection before archive lookup: {error}");
            };
            let jett_driver::BackendLoweringError::Build(built) = *source else {
                panic!("{name}: expected checking rejection before HIR");
            };
            assert!(built.has_errors, "{name}");
            let errors = built
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == jett_diagnostics::Severity::Error)
                .collect::<Vec<_>>();
            assert!(!errors.is_empty(), "{name}");
            if name == "02_shadowed_message_and_ordinary_helper.jett" {
                assert!(
                    errors
                        .iter()
                        .any(|diagnostic| diagnostic.code.code() == 201),
                    "{name}: {:?}",
                    built.diagnostics
                );
            } else {
                assert!(
                    errors
                        .iter()
                        .all(|diagnostic| (400..500).contains(&diagnostic.code.code())),
                    "{name}: {:?}",
                    built.diagnostics
                );
            }
            assert_eq!(fs::read(&output).unwrap(), sentinel, "{name}");
        };
        for release in [false, true] {
            let options = jett_driver::BuildOptions { release };
            assert_refusal(
                jett_driver::native::build_host_executable_with_options(
                    &source, &launcher, &output, options,
                )
                .expect_err("invalid actor ownership prevents main publication"),
            );
            assert_refusal(
                jett_driver::native::build_host_verify_suite_executable_with_options(
                    &source, &launcher, &output, options,
                )
                .expect_err("invalid actor ownership prevents verify publication"),
            );
            assert_refusal(
                jett_driver::native::build_host_property_suite_executable_with_options(
                    &source, &launcher, &output, options,
                )
                .expect_err("invalid actor ownership prevents property publication"),
            );
        }
    }
}

#[test]
fn native_actor_persistent_views_follow_exact_owned_state_without_source() {
    assert_main(
        "actor_field_updates/11_persistent_aliases_and_independent_field.jett",
        "before:2:5:2:5:2|2:21:2:21:10\nafter:2:5:2:5:19|2:21:2:21:32\n",
    );
}

#[test]
fn native_actor_respond_is_a_terminal_handle_exit_without_source() {
    assert_main(
        "actor_field_updates/13_respond_is_terminal_handle_exit.jett",
        "early:4:4:9:9:9\n",
    );
}

#[test]
fn native_actor_projected_persistent_views_replay_the_checked_path_without_source() {
    assert_main(
        "actor_field_updates/14_projected_persistent_alias.jett",
        concat!(
            "before-first:envelope-2:102:2:inner-2:outer-2:1002|2:inner-2|2:inner-2|2\n",
            "before-second:envelope-7:107:7:inner-7:outer-7:1007|7:inner-7|7:inner-7|7\n",
            "after-first:envelope-2:102:2:inner-2:outer-2:1002|2:inner-2|2:inner-2|13\n",
            "after-second:envelope-7:107:7:inner-7:outer-7:1007|7:inner-7|7:inner-7|15\n",
        ),
    );
}

#[test]
fn native_existing_actor_receiver_handlers_keep_order_without_source_in_both_runtime_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/native/nested_actor_message_handle.jett"),
        &source,
    )
    .unwrap();
    let reference =
        jett_driver::run_file_capture_outcome(&source).expect("existing handled actor receiver");
    assert!(reference.stdout.is_empty());
    let stderr = jett_driver::render_debug_events(&reference.debug_events);
    assert_eq!(
        super::debug_print_text(&reference.debug_events),
        "text\ntext\nBefore\nBefore:Ada:Ada\nAda\ntext\ntext\nChanged\nBefore:Fallback:Fallback\nFallback\nAda\nFallback\n"
    );
    let binaries = [false, true].map(|runtime_release| {
        let binary = directory
            .path()
            .join(format!("actor-receiver-{runtime_release}.exe"));
        // Both Sources keep Debug policy because the original program uses println.
        jett_driver::native::build_host_executable(
            &source,
            launcher_for_options(runtime_release),
            &binary,
        )
        .unwrap_or_else(|error| {
            panic!("existing actor receiver runtime_release={runtime_release}: {error}")
        });
        (binary, runtime_release)
    });
    fs::remove_file(&source).unwrap();
    assert!(!source.exists());
    for (binary, runtime_release) in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(
            actual.status.code(),
            Some(0),
            "runtime_release={runtime_release}: {actual:?}"
        );
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert_eq!(
            actual.stderr,
            stderr.as_bytes(),
            "runtime_release={runtime_release}: {actual:?}"
        );
    }
}

#[test]
fn native_actor_owning_handle_reply_is_acquired_before_publication_without_source() {
    assert_main(
        "actor_field_updates/15_owning_handle_reply.jett",
        "reply:2:5|1:9|3\n",
    );
}
