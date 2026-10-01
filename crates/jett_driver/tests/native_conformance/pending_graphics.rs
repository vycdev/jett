use super::*;

const CONFIG_PROBE: &str = r#"namespace app
function update(state: int64, key: graphics.Key) returns int64:
    return state
function render(view state: int64) returns graphics.Scene:
    graphics.Color background = graphics.Color(red: 0, green: 0, blue: 0)
    return graphics.Scene(background: background, rectangles: list.new[graphics.Rect](), texts: list.new[graphics.Text]())
function main(display: Graphics, stdout: Stdout) returns nothing:
    graphics.Config config = graphics.Config(title: "pending", width: run 0, height: 1)
    Stdout.write(view stdout, "before\n")
    graphics.run[int64](view display, config, 0, update, render) handle error:
        Stdout.write(view stdout, "handled:{error}\n")
        return nothing
    Stdout.write(view stdout, "after\n")
"#;

#[test]
fn native_pending_graphics_config_matches_terminal_reference_failure() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("main.jett");
    fs::write(&source, CONFIG_PROBE).unwrap();
    let expected = jett_driver::run_file_capture_outcome(&source)
        .expect_err("pending width must fail decoding before opening a window");
    assert_eq!(expected.output.stdout, "before\n");
    assert_eq!(
        expected.message,
        "runtime error: graphics: graphics.Config.width must be int64"
    );
    let mut binaries = Vec::new();
    for release in [false, true] {
        let binary = directory
            .path()
            .join(format!("pending_config_{release}.exe"));
        jett_driver::native::build_host_executable_with_options(
            &source,
            launcher(),
            &binary,
            jett_driver::BuildOptions { release },
        )
        .expect("native pending Graphics Config probe");
        binaries.push(binary);
    }
    fs::remove_file(&source).unwrap();
    for binary in binaries {
        let actual = run_bounded(&binary, directory.path());
        assert_eq!(actual.status.code(), Some(71), "{actual:?}");
        assert_eq!(actual.stdout, expected.output.stdout.as_bytes());
        assert_eq!(actual.stderr, format!("{}\n", expected.message).as_bytes());
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DecoderKind {
    Config,
    Scene,
}

#[derive(Clone, Copy)]
enum DecoderExpectation {
    Terminal(&'static str),
    Handled(&'static str),
}

struct DecoderCase {
    name: String,
    kind: DecoderKind,
    body: String,
    expectation: DecoderExpectation,
}

const CONFIG_BODY: &str = r#"    return graphics.Config(title: "pending", width: 16, height: 16)
"#;

// An invalid ready background keeps old implementations headless when they
// accidentally ignore a pending value. Shape decoding must precede this domain
// failure, even when the pending value appears in a later rectangle or text.
const SCENE_BODY: &str = r#"    graphics.Color background = graphics.Color(red: 999, green: 0, blue: 0)
    graphics.Color ink = graphics.Color(red: 1, green: 2, blue: 3)
    graphics.Rect rectangle = graphics.Rect(x: 0, y: 0, width: 1, height: 1, color: ink)
    graphics.Color letter_ink = graphics.Color(red: 4, green: 5, blue: 6)
    graphics.Text label = graphics.Text(x: 0, y: 0, text: "label", scale: 1, color: letter_ink)
    list[graphics.Rect] rectangles = list(rectangle)
    list[graphics.Text] texts = list(label)
    return graphics.Scene(background: background, rectangles: rectangles, texts: texts)
"#;

const DIMENSIONS_ERROR: &str = "graphics.run: window dimensions must be in 1..2048";
const COLOR_ERROR: &str = "graphics.run: color channels must be in 0..255";

fn replace_fixture(source: &str, changes: &[(&str, &str)]) -> String {
    changes
        .iter()
        .fold(source.to_owned(), |source, (old, new)| {
            assert!(source.contains(old), "fixture replacement missing {old:?}");
            source.replacen(old, new, 1)
        })
}

fn decoder_case(
    name: &str,
    kind: DecoderKind,
    changes: &[(&str, &str)],
    expectation: DecoderExpectation,
) -> DecoderCase {
    DecoderCase {
        name: name.to_owned(),
        kind,
        body: replace_fixture(
            match kind {
                DecoderKind::Config => CONFIG_BODY,
                DecoderKind::Scene => SCENE_BODY,
            },
            changes,
        ),
        expectation,
    }
}

fn config_cases() -> Vec<DecoderCase> {
    use DecoderExpectation::{Handled, Terminal};
    let mut cases = vec![
        decoder_case(
            "config_root",
            DecoderKind::Config,
            &[("return graphics.Config", "return run graphics.Config")],
            Terminal("graphics: expected graphics.Config"),
        ),
        decoder_case(
            "config_root_twice",
            DecoderKind::Config,
            &[
                ("return graphics.Config", "return run (run graphics.Config"),
                ("height: 16)", "height: 16))"),
            ],
            Terminal("graphics: expected graphics.Config"),
        ),
        decoder_case(
            "config_title",
            DecoderKind::Config,
            &[("title: \"pending\"", "title: run \"pending\"")],
            Terminal("graphics: graphics.Config.title must be string"),
        ),
        decoder_case(
            "config_title_twice",
            DecoderKind::Config,
            &[("title: \"pending\"", "title: run (run \"pending\")")],
            Terminal("graphics: graphics.Config.title must be string"),
        ),
        decoder_case(
            "config_width",
            DecoderKind::Config,
            &[("width: 16", "width: run 0")],
            Terminal("graphics: graphics.Config.width must be int64"),
        ),
        decoder_case(
            "config_height",
            DecoderKind::Config,
            &[("height: 16", "height: run 0")],
            Terminal("graphics: graphics.Config.height must be int64"),
        ),
        decoder_case(
            "config_title_precedes_width",
            DecoderKind::Config,
            &[
                ("title: \"pending\"", "title: run \"pending\""),
                ("width: 16", "width: run 0"),
            ],
            Terminal("graphics: graphics.Config.title must be string"),
        ),
        decoder_case(
            "config_width_precedes_height",
            DecoderKind::Config,
            &[
                ("width: 16", "width: run 0"),
                ("height: 16", "height: run 0"),
            ],
            Terminal("graphics: graphics.Config.width must be int64"),
        ),
        decoder_case(
            "config_decoding_precedes_dimensions",
            DecoderKind::Config,
            &[("width: 16", "width: 0"), ("height: 16", "height: run 0")],
            Terminal("graphics: graphics.Config.height must be int64"),
        ),
        decoder_case(
            "config_comptime_width",
            DecoderKind::Config,
            &[("width: 16", "width: comptime run 0")],
            Terminal("graphics: graphics.Config.width must be int64"),
        ),
        decoder_case(
            "config_ready_dimensions",
            DecoderKind::Config,
            &[("width: 16", "width: 0")],
            Handled(DIMENSIONS_ERROR),
        ),
    ];
    cases.push(DecoderCase {
        name: "config_partial_join".into(),
        kind: DecoderKind::Config,
        body: r#"    graphics.Config pending = run (run graphics.Config(title: "pending", width: 0, height: 1))
    graphics.Config first = join pending handle error:
        default graphics.Config(title: "fallback", width: 0, height: 1)
    return first
"#.into(),
        expectation: Terminal("graphics: expected graphics.Config"),
    });
    cases.push(DecoderCase {
        name: "config_partial_string_join".into(),
        kind: DecoderKind::Config,
        body: r#"    string pending = run (run "pending")
    string first = join pending handle error:
        default "fallback"
    return graphics.Config(title: first, width: 0, height: 1)
"#
        .into(),
        expectation: Terminal("graphics: graphics.Config.title must be string"),
    });
    cases.push(DecoderCase {
        name: "config_partial_integer_join".into(),
        kind: DecoderKind::Config,
        body: r#"    int64 pending = run (run 0)
    int64 first = join pending handle error:
        default 0
    return graphics.Config(title: "pending", width: first, height: 1)
"#
        .into(),
        expectation: Terminal("graphics: graphics.Config.width must be int64"),
    });
    cases.push(DecoderCase {
        name: "config_joined_root_and_fields".into(),
        kind: DecoderKind::Config,
        body: r#"    string title_task = run (run "joined")
    string first_title = join title_task handle error:
        default "fallback"
    string title = join first_title handle error:
        default "fallback"
    int64 width_task = run (run 0)
    int64 first_width = join width_task handle error:
        default 0
    int64 width = join first_width handle error:
        default 0
    graphics.Config task = run (run graphics.Config(title: title, width: width, height: 1))
    graphics.Config first = join task handle error:
        default graphics.Config(title: "fallback", width: 0, height: 1)
    return join first handle error:
        default graphics.Config(title: "fallback", width: 0, height: 1)
"#
        .into(),
        expectation: Handled(DIMENSIONS_ERROR),
    });
    cases
}

fn scene_cases() -> Vec<DecoderCase> {
    use DecoderExpectation::{Handled, Terminal};
    let mut cases = vec![
        decoder_case(
            "scene_root",
            DecoderKind::Scene,
            &[("return graphics.Scene", "return run graphics.Scene")],
            Terminal("graphics: expected graphics.Scene"),
        ),
        decoder_case(
            "scene_root_twice",
            DecoderKind::Scene,
            &[
                ("return graphics.Scene", "return run (run graphics.Scene"),
                ("texts: texts)", "texts: texts))"),
            ],
            Terminal("graphics: expected graphics.Scene"),
        ),
        decoder_case(
            "scene_background",
            DecoderKind::Scene,
            &[(
                "background = graphics.Color",
                "background = run graphics.Color",
            )],
            Terminal("graphics: expected graphics.Color"),
        ),
        decoder_case(
            "scene_background_red",
            DecoderKind::Scene,
            &[("red: 999", "red: run 999")],
            Terminal("graphics: graphics.Color.red must be int64"),
        ),
        decoder_case(
            "scene_background_green",
            DecoderKind::Scene,
            &[("green: 0", "green: run 0")],
            Terminal("graphics: graphics.Color.green must be int64"),
        ),
        decoder_case(
            "scene_background_blue",
            DecoderKind::Scene,
            &[("blue: 0", "blue: run 0")],
            Terminal("graphics: graphics.Color.blue must be int64"),
        ),
        decoder_case(
            "scene_rectangles",
            DecoderKind::Scene,
            &[(
                "rectangles = list(rectangle)",
                "rectangles = run list(rectangle)",
            )],
            Terminal("graphics: graphics.Scene.rectangles must be a list"),
        ),
        decoder_case(
            "scene_rectangles_twice",
            DecoderKind::Scene,
            &[(
                "rectangles = list(rectangle)",
                "rectangles = run (run list(rectangle))",
            )],
            Terminal("graphics: graphics.Scene.rectangles must be a list"),
        ),
        decoder_case(
            "scene_rectangle",
            DecoderKind::Scene,
            &[("rectangle = graphics.Rect", "rectangle = run graphics.Rect")],
            Terminal("graphics: expected graphics.Rect"),
        ),
        decoder_case(
            "scene_rectangle_twice",
            DecoderKind::Scene,
            &[(
                "rectangle = graphics.Rect(x: 0, y: 0, width: 1, height: 1, color: ink)",
                "rectangle = run (run graphics.Rect(x: 0, y: 0, width: 1, height: 1, color: ink))",
            )],
            Terminal("graphics: expected graphics.Rect"),
        ),
        decoder_case(
            "scene_texts",
            DecoderKind::Scene,
            &[("texts = list(label)", "texts = run list(label)")],
            Terminal("graphics: graphics.Scene.texts must be a list"),
        ),
        decoder_case(
            "scene_texts_twice",
            DecoderKind::Scene,
            &[("texts = list(label)", "texts = run (run list(label))")],
            Terminal("graphics: graphics.Scene.texts must be a list"),
        ),
        decoder_case(
            "scene_text",
            DecoderKind::Scene,
            &[("label = graphics.Text", "label = run graphics.Text")],
            Terminal("graphics: expected graphics.Text"),
        ),
        decoder_case(
            "scene_text_twice",
            DecoderKind::Scene,
            &[(
                "label = graphics.Text(x: 0, y: 0, text: \"label\", scale: 1, color: letter_ink)",
                "label = run (run graphics.Text(x: 0, y: 0, text: \"label\", scale: 1, color: letter_ink))",
            )],
            Terminal("graphics: expected graphics.Text"),
        ),
        decoder_case(
            "scene_rectangle_color",
            DecoderKind::Scene,
            &[("ink = graphics.Color", "ink = run graphics.Color")],
            Terminal("graphics: expected graphics.Color"),
        ),
        decoder_case(
            "scene_text_color",
            DecoderKind::Scene,
            &[(
                "letter_ink = graphics.Color",
                "letter_ink = run graphics.Color",
            )],
            Terminal("graphics: expected graphics.Color"),
        ),
        decoder_case(
            "scene_background_precedes_rectangles",
            DecoderKind::Scene,
            &[
                ("green: 0", "green: run 0"),
                (
                    "rectangles = list(rectangle)",
                    "rectangles = run list(rectangle)",
                ),
            ],
            Terminal("graphics: graphics.Color.green must be int64"),
        ),
        decoder_case(
            "scene_rectangles_precede_texts",
            DecoderKind::Scene,
            &[
                (
                    "rectangles = list(rectangle)",
                    "rectangles = run list(rectangle)",
                ),
                ("texts = list(label)", "texts = run list(label)"),
            ],
            Terminal("graphics: graphics.Scene.rectangles must be a list"),
        ),
        decoder_case(
            "scene_rectangle_precedes_text",
            DecoderKind::Scene,
            &[
                (
                    "rectangle = graphics.Rect(x: 0",
                    "rectangle = graphics.Rect(x: run 0",
                ),
                (
                    "label = graphics.Text(x: 0",
                    "label = graphics.Text(x: run 0",
                ),
            ],
            Terminal("graphics: graphics.Rect.x must be int64"),
        ),
        decoder_case(
            "scene_ready_color",
            DecoderKind::Scene,
            &[],
            Handled(COLOR_ERROR),
        ),
    ];
    for (field, value, error) in [
        ("x", "0", "graphics: graphics.Rect.x must be int64"),
        ("y", "0", "graphics: graphics.Rect.y must be int64"),
        ("width", "1", "graphics: graphics.Rect.width must be int64"),
        (
            "height",
            "1",
            "graphics: graphics.Rect.height must be int64",
        ),
    ] {
        let original = "rectangle = graphics.Rect(x: 0, y: 0, width: 1, height: 1, color: ink)";
        let changed = original.replacen(
            &format!("{field}: {value}"),
            &format!("{field}: run {value}"),
            1,
        );
        cases.push(decoder_case(
            &format!("scene_rectangle_{field}"),
            DecoderKind::Scene,
            &[(original, &changed)],
            Terminal(error),
        ));
    }
    for (field, value, error) in [
        ("x", "0", "graphics: graphics.Text.x must be int64"),
        ("y", "0", "graphics: graphics.Text.y must be int64"),
        (
            "text",
            "\"label\"",
            "graphics: graphics.Text.text must be string",
        ),
        ("scale", "1", "graphics: graphics.Text.scale must be int64"),
    ] {
        let original =
            "label = graphics.Text(x: 0, y: 0, text: \"label\", scale: 1, color: letter_ink)";
        let changed = original.replacen(
            &format!("{field}: {value}"),
            &format!("{field}: run {value}"),
            1,
        );
        cases.push(decoder_case(
            &format!("scene_text_{field}"),
            DecoderKind::Scene,
            &[(original, &changed)],
            Terminal(error),
        ));
    }
    for (owner, values) in [("ink", ["1", "2", "3"]), ("letter_ink", ["4", "5", "6"])] {
        for ((field, value), error) in ["red", "green", "blue"].into_iter().zip(values).zip([
            "graphics: graphics.Color.red must be int64",
            "graphics: graphics.Color.green must be int64",
            "graphics: graphics.Color.blue must be int64",
        ]) {
            let original = format!(
                "{owner} = graphics.Color(red: {}, green: {}, blue: {})",
                values[0], values[1], values[2]
            );
            let changed = original.replacen(
                &format!("{field}: {value}"),
                &format!("{field}: run {value}"),
                1,
            );
            cases.push(decoder_case(
                &format!("scene_{owner}_{field}"),
                DecoderKind::Scene,
                &[(&original, &changed)],
                Terminal(error),
            ));
        }
    }
    for (name, kind, binding, fallback, error) in [
        (
            "scene_partial_root_join",
            "graphics.Scene",
            "graphics.Scene(background: background, rectangles: rectangles, texts: texts)",
            "fallback_scene()",
            "graphics: expected graphics.Scene",
        ),
        (
            "scene_partial_rectangles_join",
            "list[graphics.Rect]",
            "list(rectangle)",
            "list.new[graphics.Rect]()",
            "graphics: graphics.Scene.rectangles must be a list",
        ),
        (
            "scene_partial_texts_join",
            "list[graphics.Text]",
            "list(label)",
            "list.new[graphics.Text]()",
            "graphics: graphics.Scene.texts must be a list",
        ),
    ] {
        let original = match name {
            "scene_partial_root_join" => format!("    return {binding}\n"),
            "scene_partial_rectangles_join" => format!("    {kind} rectangles = {binding}\n"),
            _ => format!("    {kind} texts = {binding}\n"),
        };
        let target = match name {
            "scene_partial_root_join" => "    return first\n".to_owned(),
            "scene_partial_rectangles_join" => format!("    {kind} rectangles = first\n"),
            _ => format!("    {kind} texts = first\n"),
        };
        let changed = format!(
            "    {kind} task = run (run {binding})\n    {kind} first = join task handle error:\n        default {fallback}\n{target}"
        );
        cases.push(decoder_case(
            name,
            DecoderKind::Scene,
            &[(&original, &changed)],
            Terminal(error),
        ));
    }
    cases.push(decoder_case(
        "scene_comptime_text",
        DecoderKind::Scene,
        &[("text: \"label\"", "text: comptime run (run \"label\")")],
        Terminal("graphics: graphics.Text.text must be string"),
    ));
    cases
}

fn append_dispatch(source: &mut String, stem: &str, cases: &[&DecoderCase], result: &str) {
    for (group, chunk) in cases.chunks(8).enumerate() {
        source.push_str(&format!(
            "function {stem}_group_{group}(name: string) returns {result}:\n"
        ));
        for case in chunk {
            source.push_str(&format!(
                "    if name == \"{}\":\n        return probe_{}()\n",
                case.name, case.name
            ));
        }
        source.push_str(&format!("    return fallback_{stem}()\n"));
    }
    source.push_str(&format!(
        "function choose_{stem}(bucket: string, name: string) returns {result}:\n"
    ));
    for group in 0..cases.len().div_ceil(8) {
        source.push_str(&format!(
            "    if bucket == \"{group}\":\n        return {stem}_group_{group}(name)\n"
        ));
    }
    source.push_str(&format!("    return fallback_{stem}()\n"));
}

fn matrix_source(cases: &[DecoderCase]) -> String {
    let mut source = r#"namespace app
struct Request:
    bucket: string
    name: string
function fallback_config() returns graphics.Config:
    return graphics.Config(title: "ready", width: 16, height: 16)
function fallback_scene() returns graphics.Scene:
    graphics.Color background = graphics.Color(red: 999, green: 0, blue: 0)
    return graphics.Scene(background: background, rectangles: list.new[graphics.Rect](), texts: list.new[graphics.Text]())
"#.to_owned();
    for case in cases {
        let result = match case.kind {
            DecoderKind::Config => "graphics.Config",
            DecoderKind::Scene => "graphics.Scene",
        };
        source.push_str(&format!(
            "function probe_{}() returns {result}:\n{}",
            case.name, case.body
        ));
    }
    for (kind, stem, result) in [
        (DecoderKind::Config, "config", "graphics.Config"),
        (DecoderKind::Scene, "scene", "graphics.Scene"),
    ] {
        let selected = cases
            .iter()
            .filter(|case| case.kind == kind)
            .collect::<Vec<_>>();
        append_dispatch(&mut source, stem, &selected, result);
    }
    source.push_str(
        r#"function update(state: Request, key: graphics.Key) returns Request:
    return state
function render(view state: Request) returns graphics.Scene:
    return choose_scene(state.bucket, state.name)
function request_text(view env: Environment, key: string) returns string:
    optional[string] value = Environment.get(view env, key) handle error:
        return error
    return value handle:
        default "missing"
function main(display: Graphics, stdout: Stdout, env: Environment) returns nothing:
    string name = request_text(view env, "case")
    string bucket = request_text(view env, "bucket")
    graphics.Config config = choose_config(bucket, name)
    Request request = Request(bucket: bucket, name: name)
    Stdout.write(view stdout, "before\n")
    graphics.run[Request](view display, config, request, update, render) handle error:
        Stdout.write(view stdout, "handled:{error}\n")
        return nothing
    Stdout.write(view stdout, "after\n")
"#,
    );
    source
}

fn case_snapshot(
    cases: &[DecoderCase],
    case: &DecoderCase,
) -> environment::EnvironmentTestSnapshot {
    let position = cases
        .iter()
        .filter(|candidate| candidate.kind == case.kind)
        .position(|candidate| candidate.name == case.name)
        .unwrap();
    environment::EnvironmentTestSnapshot {
        arguments: Vec::new(),
        entries: [
            ("case", case.name.clone()),
            ("bucket", (position / 8).to_string()),
        ]
        .into_iter()
        .map(|(name, value)| environment::EnvironmentTestEntry {
            name: environment::EnvironmentTestText::Unicode(name.into()),
            value: environment::EnvironmentTestText::Unicode(value),
        })
        .collect(),
    }
}

fn build_profiles(source: &Path, directory: &Path, name: &str) -> Vec<(std::path::PathBuf, bool)> {
    [false, true]
        .into_iter()
        .map(|release| {
            let binary = directory.join(format!("{name}_{release}.exe"));
            jett_driver::native::build_host_executable_with_options(
                source,
                launcher(),
                &binary,
                jett_driver::BuildOptions { release },
            )
            .expect("native pending Graphics decoder matrix");
            (binary, release)
        })
        .collect()
}

#[test]
fn native_pending_graphics_decoders_preserve_reference_errors_and_order_in_both_profiles() {
    let mut cases = config_cases();
    cases.extend(scene_cases());
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("matrix.jett");
    fs::write(&source, matrix_source(&cases)).unwrap();
    // Pin the reference independently before emitting any native binaries.
    for case in &cases {
        let outcome = jett_driver::run_file_capture_outcome_with_environment_test_snapshot(
            &source,
            case_snapshot(&cases, case),
        );
        match case.expectation {
            DecoderExpectation::Terminal(message) => {
                let expected = outcome.expect_err(&format!("{} must fail decoding", case.name));
                assert_eq!(
                    expected.message,
                    format!("runtime error: {message}"),
                    "{}",
                    case.name
                );
                assert_eq!(expected.output.stdout, "before\n", "{}", case.name);
                assert!(expected.output.debug_output.is_empty(), "{}", case.name);
            }
            DecoderExpectation::Handled(message) => {
                let expected = outcome.unwrap_or_else(|error| panic!("{}: {error:?}", case.name));
                assert_eq!(
                    expected.stdout,
                    format!("before\nhandled:{message}\n"),
                    "{}",
                    case.name
                );
                assert!(expected.debug_output.is_empty(), "{}", case.name);
            }
        }
    }
    let binaries = build_profiles(&source, directory.path(), "decoder_matrix");
    fs::remove_file(&source).unwrap();
    for (binary, _) in binaries {
        for case in &cases {
            let script = environment::encode_test_snapshot(&case_snapshot(&cases, case));
            let actual = run_bounded_with_env(
                &binary,
                directory.path(),
                Some((environment::TEST_SNAPSHOT_ENV, &script)),
            );
            match case.expectation {
                DecoderExpectation::Terminal(message) => {
                    assert_eq!(actual.status.code(), Some(71), "{}: {actual:?}", case.name);
                    assert_eq!(actual.stdout, b"before\n", "{}", case.name);
                    assert_eq!(
                        actual.stderr,
                        format!("runtime error: {message}\n").as_bytes(),
                        "{}",
                        case.name
                    );
                }
                DecoderExpectation::Handled(message) => {
                    assert!(actual.status.success(), "{}: {actual:?}", case.name);
                    assert_eq!(
                        actual.stdout,
                        format!("before\nhandled:{message}\n").as_bytes(),
                        "{}",
                        case.name
                    );
                    assert!(actual.stderr.is_empty(), "{}: {actual:?}", case.name);
                }
            }
        }
    }
}

const SCRIPTED_FAILURE: &str = r#"namespace app
function update(state: int64, key: graphics.Key) returns int64:
    return 1
function render(view state: int64) returns graphics.Scene:
    if state == 1:
        graphics.Color bad_background = graphics.Color(red: 999, green: 0, blue: 0)
        graphics.Color ink = graphics.Color(red: 0, green: 0, blue: 0)
        graphics.Text label = graphics.Text(x: run 0, y: 0, text: "pending", scale: 1, color: ink)
        return graphics.Scene(background: bad_background, rectangles: list.new[graphics.Rect](), texts: list(label))
    graphics.Color background = graphics.Color(red: 0, green: 0, blue: 0)
    return graphics.Scene(background: background, rectangles: list.new[graphics.Rect](), texts: list.new[graphics.Text]())
function main(display: Graphics, stdout: Stdout) returns nothing:
    graphics.Config config = graphics.Config(title: "scripted", width: 16, height: 16)
    Stdout.write(view stdout, "before\n")
    graphics.run[int64](view display, config, INITIAL_STATE, update, render) handle error:
        Stdout.write(view stdout, "handled:{error}\n")
        return nothing
    Stdout.write(view stdout, "after\n")
"#;

#[test]
fn native_pending_graphics_render_failures_preserve_terminal_error_and_cleanup() {
    for (name, initial, events) in [
        (
            "initial",
            "1",
            vec![graphics::TestEvent::HostError("must remain unused".into())],
        ),
        (
            "later",
            "0",
            vec![
                graphics::TestEvent::Key(graphics::Key::Right),
                graphics::TestEvent::Close,
                graphics::TestEvent::Key(graphics::Key::Left),
            ],
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("{name}.jett"));
        fs::write(&source, SCRIPTED_FAILURE.replace("INITIAL_STATE", initial)).unwrap();
        let expected = jett_driver::run_file_capture_outcome_with_graphics_test_events(
            &source,
            events.clone(),
        )
        .expect_err("scene decoding must fail before presentation or further provider input");
        assert_eq!(expected.output.stdout, "before\n", "{name}");
        assert!(expected.output.debug_output.is_empty(), "{name}");
        assert_eq!(
            expected.message, "runtime error: graphics: graphics.Text.x must be int64",
            "{name}"
        );
        let binaries = build_profiles(&source, directory.path(), name);
        fs::remove_file(&source).unwrap();
        let script = graphics::encode_test_script(&events);
        for (binary, _) in binaries {
            let actual = run_bounded_with_env(
                &binary,
                directory.path(),
                Some((graphics::TEST_SCRIPT_ENV, &script)),
            );
            assert_eq!(actual.status.code(), Some(71), "{name}: {actual:?}");
            assert_eq!(actual.stdout, b"before\n", "{name}");
            assert_eq!(
                actual.stderr, b"runtime error: graphics: graphics.Text.x must be int64\n",
                "{name}"
            );
        }
    }
}

const JOINED_CONTROL: &str = r#"namespace app
function joined[T](value: T, fallback: T) returns T:
    T task = run (run value)
    T first = join task handle error:
        default clone fallback
    return join first handle error:
        default fallback
function zero_color() returns graphics.Color:
    return graphics.Color(red: 0, green: 0, blue: 0)
function plain_rectangle() returns graphics.Rect:
    return graphics.Rect(x: 0, y: 0, width: 1, height: 1, color: zero_color())
function plain_text() returns graphics.Text:
    return graphics.Text(x: 0, y: 0, text: "fallback", scale: 1, color: zero_color())
function plain_scene() returns graphics.Scene:
    return graphics.Scene(background: zero_color(), rectangles: list.new[graphics.Rect](), texts: list.new[graphics.Text]())
function ready_rectangle() returns graphics.Rect:
    graphics.Color ink = joined[graphics.Color](zero_color(), zero_color())
    graphics.Rect rectangle = graphics.Rect(x: joined[int64](0, 0), y: 0, width: 1, height: 1, color: ink)
    return joined[graphics.Rect](rectangle, plain_rectangle())
function ready_text() returns graphics.Text:
    graphics.Color ink = joined[graphics.Color](zero_color(), zero_color())
    string text = joined[string]("joined", "fallback")
    graphics.Text label = graphics.Text(x: 0, y: 0, text: text, scale: joined[int64](1, 1), color: ink)
    return joined[graphics.Text](label, plain_text())
function ready_scene() returns graphics.Scene:
    graphics.Color background = joined[graphics.Color](zero_color(), zero_color())
    list[graphics.Rect] rectangles = joined[list[graphics.Rect]](list(ready_rectangle()), list.new[graphics.Rect]())
    list[graphics.Text] texts = joined[list[graphics.Text]](list(ready_text()), list.new[graphics.Text]())
    graphics.Scene scene = graphics.Scene(background: background, rectangles: rectangles, texts: texts)
    return joined[graphics.Scene](scene, plain_scene())
function ready_config() returns graphics.Config:
    string title = joined[string]("joined", "fallback")
    graphics.Config config = graphics.Config(title: title, width: joined[int64](16, 1), height: joined[int64](16, 1))
    return joined[graphics.Config](config, graphics.Config(title: "fallback", width: 1, height: 1))
function update(state: int64, key: graphics.Key) returns int64:
    trace state
    return run state
function render(view state: int64) returns graphics.Scene:
    trace state
    return ready_scene()
function main(display: Graphics, stdout: Stdout) returns nothing:
    graphics.Config config = comptime ready_config()
    int64 initial = run 5
    Stdout.write(view stdout, "before\n")
    graphics.run[int64](view display, config, initial, update, render) handle error:
        Stdout.write(view stdout, "handled:{error}\n")
        return nothing
    Stdout.write(view stdout, "closed\n")
verify fully_joined_graphics_data:
    graphics.Config config = ready_config()
    assert config.title == "joined"
    assert config.width == 16
    assert config.height == 16
    graphics.Scene scene = ready_scene()
    assert scene.background.red == 0
    assert list.length(view scene.rectangles) == 1
    assert list.length(view scene.texts) == 1
    graphics.Rect rectangle = list.get[graphics.Rect](view scene.rectangles, 0) handle:
        default plain_rectangle()
    assert rectangle.width == 1
    graphics.Text label = list.get[graphics.Text](view scene.texts, 0) handle:
        default plain_text()
    assert label.text == "joined"
property joined_graphics_data_trials:
    given seed: int64
    assert joined[int64](seed, 0) == seed
    graphics.Scene scene = ready_scene()
    assert scene.background.red == 0
    assert list.length(view scene.rectangles) == 1
    assert list.length(view scene.texts) == 1
"#;

#[test]
fn native_joined_graphics_data_and_pending_state_match_reference_in_both_profiles() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("joined.jett");
    fs::write(&source, JOINED_CONTROL).unwrap();
    let events = vec![
        graphics::TestEvent::Key(graphics::Key::Right),
        graphics::TestEvent::Close,
    ];
    let expected =
        jett_driver::run_file_capture_outcome_with_graphics_test_events(&source, events.clone())
            .expect("fully joined graphics data must accept a still-pending callback State");
    assert_eq!(expected.stdout, "before\nclosed\n");
    assert_eq!(
        expected.debug_output,
        [
            "trace state: int64 = pending(5)",
            "trace state: int64 = pending(5)",
            "trace state: int64 = pending(pending(5))",
        ]
    );
    let binaries = build_profiles(&source, directory.path(), "joined");
    let verify_binary = directory.path().join("joined_verify.exe");
    build_host_verify_suite_executable(&source, launcher(), &verify_binary)
        .expect("compiled joined graphics data verify suite");
    let property_binary = directory.path().join("joined_property.exe");
    build_host_property_suite_executable(&source, launcher(), &property_binary)
        .expect("compiled joined graphics data property suite");
    fs::remove_file(&source).unwrap();
    let script = graphics::encode_test_script(&events);
    for (binary, release) in binaries {
        let actual = run_bounded_with_env(
            &binary,
            directory.path(),
            Some((graphics::TEST_SCRIPT_ENV, &script)),
        );
        assert!(actual.status.success(), "{actual:?}");
        assert_eq!(actual.stdout, b"before\nclosed\n");
        let debug = if release {
            String::new()
        } else {
            format!("{}\n", expected.debug_output.join("\n"))
        };
        assert_eq!(actual.stderr, debug.as_bytes(), "{actual:?}");
    }
    for binary in [verify_binary, property_binary] {
        let actual = run_bounded(&binary, directory.path());
        assert!(actual.status.success(), "{actual:?}");
        assert!(actual.stdout.is_empty(), "{actual:?}");
        assert!(actual.stderr.is_empty(), "{actual:?}");
    }
}
