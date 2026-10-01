use super::*;

const PENDING_AUTHORITY_PROBE: &str = r#"namespace app
function update(state: int64, key: graphics.Key) returns int64:
    return state
function render(view state: int64) returns graphics.Scene:
    graphics.Color background = graphics.Color(red: 0, green: 0, blue: 0)
    return graphics.Scene(background: background, rectangles: list.new[graphics.Rect](), texts: list.new[graphics.Text]())
function attempt(view display: Graphics, view stdout: Stdout) returns nothing:
    graphics.Config config = graphics.Config(title: "authority", width: 0, height: 1)
    Stdout.write(view stdout, "before\n")
    graphics.run[int64](view display, config, 0, update, render) handle error:
        Stdout.write(view stdout, "handled:{error}\n")
        return nothing
    Stdout.write(view stdout, "must not run\n")
function main(display: Graphics, stdout: Stdout) returns nothing:
    attempt(view run display, view stdout)
    Stdout.write(view stdout, "after\n")
"#;

#[test]
fn native_pending_graphics_authority_fails_before_config_validation_in_both_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("authority.jett");
    fs::write(&source, PENDING_AUTHORITY_PROBE).unwrap();
    let expected = jett_driver::run_file_capture_outcome(&source)
        .expect_err("pending Graphics authority must fail before the width-zero domain error");
    assert_eq!(expected.output.stdout, "before\n");
    assert!(expected.output.debug_output.is_empty());
    assert_eq!(
        expected.message,
        "runtime error: graphics.__run expects a Graphics capability"
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory.path().join(format!("authority_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native forwarded pending Graphics authority probe");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(71), "{actual:?}");
        assert_eq!(actual.stdout, b"before\n");
        assert_eq!(
            actual.stderr,
            b"runtime error: graphics.__run expects a Graphics capability\n"
        );
    }
}

const CONFIG_BINDING: &str =
    "    graphics.Config config = graphics.Config(title: \"authority\", width: 0, height: 1)\n";
const MAIN_CALL: &str = "    attempt(view run display, view stdout)\n";
const READY_CALL: &str = "    attempt(view display, view stdout)\n";
const TWICE_PENDING_CALL: &str = "    attempt(view run (run display), view stdout)\n";
const PARTIALLY_JOINED_CALL: &str = r#"    Graphics nested = run (run display)
    Graphics first = join nested handle error:
        return nothing
    attempt(view first, view stdout)
"#;
const FULLY_JOINED_CALL: &str = r#"    Graphics nested = run (run display)
    Graphics first = join nested handle error:
        return nothing
    Graphics ready = join first handle error:
        return nothing
    attempt(view ready, view stdout)
"#;
const FULLY_JOINED_REUSED_CALL: &str = r#"    Graphics nested = run (run display)
    Graphics first = join nested handle error:
        return nothing
    Graphics ready = join first handle error:
        return nothing
    attempt(view ready, view stdout)
    attempt(view ready, view stdout)
"#;

#[derive(Clone, Copy)]
enum AuthorityExpectation {
    Terminal(&'static str),
    Handled { attempts: usize },
}

fn authority_source(call: &str, config: &str) -> String {
    assert!(PENDING_AUTHORITY_PROBE.contains(MAIN_CALL));
    assert!(PENDING_AUTHORITY_PROBE.contains(CONFIG_BINDING));
    PENDING_AUTHORITY_PROBE
        .replacen(MAIN_CALL, call, 1)
        .replacen(CONFIG_BINDING, config, 1)
}

fn handled_output(attempts: usize) -> String {
    let mut stdout = String::new();
    for _ in 0..attempts {
        stdout.push_str("before\nhandled:graphics.run: window dimensions must be in 1..2048\n");
    }
    stdout.push_str("after\n");
    stdout
}

#[test]
fn native_graphics_authority_depth_and_ready_controls_match_reference_in_both_profiles() {
    const AUTHORITY_ERROR: &str = "graphics.__run expects a Graphics capability";
    const PENDING_CONFIG: &str = "    graphics.Config config = run graphics.Config(title: \"authority\", width: 0, height: 1)\n";
    const PENDING_TITLE: &str = "    graphics.Config config = graphics.Config(title: run \"authority\", width: 0, height: 1)\n";
    const PENDING_WIDTH: &str = "    graphics.Config config = graphics.Config(title: \"authority\", width: run 0, height: 1)\n";

    let cases = [
        (
            "depth_two",
            TWICE_PENDING_CALL,
            CONFIG_BINDING,
            AuthorityExpectation::Terminal(AUTHORITY_ERROR),
        ),
        (
            "depth_two_precedes_pending_config",
            TWICE_PENDING_CALL,
            PENDING_CONFIG,
            AuthorityExpectation::Terminal(AUTHORITY_ERROR),
        ),
        (
            "partial_join",
            PARTIALLY_JOINED_CALL,
            CONFIG_BINDING,
            AuthorityExpectation::Terminal(AUTHORITY_ERROR),
        ),
        (
            "partial_join_precedes_pending_title",
            PARTIALLY_JOINED_CALL,
            PENDING_TITLE,
            AuthorityExpectation::Terminal(AUTHORITY_ERROR),
        ),
        (
            "partial_join_precedes_pending_width",
            PARTIALLY_JOINED_CALL,
            PENDING_WIDTH,
            AuthorityExpectation::Terminal(AUTHORITY_ERROR),
        ),
        (
            "ready",
            READY_CALL,
            CONFIG_BINDING,
            AuthorityExpectation::Handled { attempts: 1 },
        ),
        (
            "joined",
            FULLY_JOINED_CALL,
            CONFIG_BINDING,
            AuthorityExpectation::Handled { attempts: 1 },
        ),
        (
            "joined_reused",
            FULLY_JOINED_REUSED_CALL,
            CONFIG_BINDING,
            AuthorityExpectation::Handled { attempts: 2 },
        ),
        (
            "ready_then_pending_title",
            READY_CALL,
            PENDING_TITLE,
            AuthorityExpectation::Terminal("graphics: graphics.Config.title must be string"),
        ),
        (
            "joined_then_pending_config",
            FULLY_JOINED_CALL,
            PENDING_CONFIG,
            AuthorityExpectation::Terminal("graphics: expected graphics.Config"),
        ),
        (
            "joined_then_pending_width",
            FULLY_JOINED_CALL,
            PENDING_WIDTH,
            AuthorityExpectation::Terminal("graphics: graphics.Config.width must be int64"),
        ),
    ];
    // All cases fail or return handled domain data before opening a window.
    // Source-level callbacks and Graphics capability policy stay unchanged.
    for (name, call, config, expectation) in cases {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{name}.jett"));
        fs::write(&source, authority_source(call, config)).unwrap();
        let outcome = jett_driver::run_file_capture_outcome(&source);
        match expectation {
            AuthorityExpectation::Terminal(message) => {
                let expected =
                    outcome.expect_err(&format!("{name} must fail before graphics effects"));
                assert_eq!(expected.output.stdout, "before\n", "{name}");
                assert!(expected.output.debug_output.is_empty(), "{name}");
                assert_eq!(
                    expected.message,
                    format!("runtime error: {message}"),
                    "{name}"
                );
            }
            AuthorityExpectation::Handled { attempts } => {
                let expected = outcome.unwrap_or_else(|error| panic!("{name}: {error:?}"));
                assert_eq!(expected.stdout, handled_output(attempts), "{name}");
                assert!(expected.debug_output.is_empty(), "{name}");
            }
        }
        let mut binaries = Vec::new();
        for release in [false, true] {
            let binary = directory.path().join(format!("{name}_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                &source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .unwrap_or_else(|error| panic!("native {name}: {error:?}"));
            binaries.push(binary);
        }
        fs::remove_file(&source).unwrap();
        for binary in binaries {
            let actual = run_bounded(&binary, directory.path());
            match expectation {
                AuthorityExpectation::Terminal(message) => {
                    assert_eq!(actual.status.code(), Some(71), "{name}: {actual:?}");
                    assert_eq!(actual.stdout, b"before\n", "{name}");
                    assert_eq!(
                        actual.stderr,
                        format!("runtime error: {message}\n").as_bytes(),
                        "{name}"
                    );
                }
                AuthorityExpectation::Handled { attempts } => {
                    assert!(actual.status.success(), "{name}: {actual:?}");
                    assert_eq!(actual.stdout, handled_output(attempts).as_bytes(), "{name}");
                    assert!(actual.stderr.is_empty(), "{name}: {actual:?}");
                }
            }
        }
    }
}

#[test]
fn native_fully_joined_graphics_authority_reopens_scripted_sessions_in_both_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("joined_sessions.jett");
    let program = authority_source(
        FULLY_JOINED_REUSED_CALL,
        "    graphics.Config config = graphics.Config(title: \"authority\", width: 16, height: 16)\n",
    )
    .replacen(
        "    Stdout.write(view stdout, \"must not run\\n\")\n",
        "    Stdout.write(view stdout, \"closed\\n\")\n",
        1,
    );
    fs::write(&source, program).unwrap();
    let events = vec![graphics::TestEvent::Close, graphics::TestEvent::Close];
    let expected =
        jett_driver::run_file_capture_outcome_with_graphics_test_events(&source, events.clone())
            .expect("fully joined Graphics authority must open two successive scripted sessions");
    assert_eq!(expected.stdout, "before\nclosed\nbefore\nclosed\nafter\n");
    assert!(expected.debug_output.is_empty());
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("joined_sessions_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native reusable joined Graphics authority");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    let script = graphics::encode_test_script(&events);
    for binary in binaries {
        let actual = run_bounded_with_env(
            &binary,
            directory.path(),
            Some((graphics::TEST_SCRIPT_ENV, &script)),
        );
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, b"before\nclosed\nbefore\nclosed\nafter\n");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}
