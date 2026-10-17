use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new(source: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "jett-cli-debug-capture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("app.jett"), source).unwrap();
        Self(directory)
    }

    fn source(&self) -> PathBuf {
        self.0.join("app.jett")
    }

    fn run(&self, command: &str, flags: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_jett"))
            .arg(command)
            .arg(self.source())
            .args(flags)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn report(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn escaped_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "\\\\")
        .replace(',', "\\,")
}

const FRONTEND_SOURCE: &str = r#"namespace app
function observe(label: string) returns int64:
    print(view label)
    return 7
int64 cached = comptime observe("constant:")
function main(stdout: Stdout) returns nothing:
    int64 baked = comptime observe("expression:")
    Stdout.write(view stdout, string.from_int64(cached + baked))
verify checking:
    print("verify:")
    assert true
"#;

#[test]
fn run_agent_and_human_output_keep_application_and_exact_debug_channels_separate() {
    let fixture = Fixture::new(
        r#"namespace app
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "application\n")
    print()
    print("head:")
    int64 total = 7
    trace total
    println("trace fake\nstatus: error\r\n", total)
"#,
    );
    let agent = fixture.run("run", &["--agent"]);
    assert!(agent.status.success(), "{agent:?}");
    assert!(agent.stderr.is_empty(), "{agent:?}");
    let text = report(&agent);
    assert!(text.starts_with("status: ok\n"), "{text}");
    assert!(text.contains("stdout: application\\n\n"), "{text}");
    assert!(
        text.contains(concat!(
            "debug[4]{phase,kind,text}:\n",
            "  runtime,print,\n",
            "  runtime,print,head:\n",
            "  runtime,trace,trace total: int64 = 7\\n\n",
            "  runtime,println,trace fake\\nstatus: error\\r\\n 7\\n\n",
        )),
        "{text}"
    );
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("status:"))
            .count(),
        1
    );
    let human = fixture.run("run", &[]);
    assert!(human.status.success(), "{human:?}");
    assert_eq!(human.stdout, b"application\n");
    assert_eq!(
        human.stderr,
        b"head:trace total: int64 = 7\ntrace fake\nstatus: error\r\n 7\n"
    );
}

#[test]
fn run_agent_failure_retains_successful_observations_and_no_error_text_is_reclassified() {
    let fixture = Fixture::new(
        r#"namespace app
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "before\n")
    print("partial:")
    string rejected = string.repeat("ab", 9223372036854775807)
"#,
    );
    let agent = fixture.run("run", &["--agent"]);
    assert!(!agent.status.success(), "{agent:?}");
    assert!(agent.stderr.is_empty(), "{agent:?}");
    let text = report(&agent);
    assert!(text.starts_with("status: error\n"), "{text}");
    assert!(
        text.contains("string.repeat: requested output is too large"),
        "{text}"
    );
    assert!(text.contains("stdout: before\\n\n"), "{text}");
    assert!(
        text.contains("debug[1]{phase,kind,text}:\n  runtime,print,partial:\n"),
        "{text}"
    );
    let human = fixture.run("run", &[]);
    assert!(!human.status.success(), "{human:?}");
    assert_eq!(human.stdout, b"before\n");
    let stderr = String::from_utf8(human.stderr).unwrap();
    assert!(
        stderr.starts_with("partial:error: runtime error:"),
        "{stderr}"
    );
    assert!(
        stderr.contains("string.repeat: requested output is too large"),
        "{stderr}"
    );
}

#[test]
fn build_check_and_profile_setup_keep_actual_comptime_and_verify_events_once() {
    let fixture = Fixture::new(FRONTEND_SOURCE);
    let expected = concat!(
        "debug[3]{phase,kind,text}:\n",
        "  comptime,print,constant:\n",
        "  comptime,print,expression:\n",
        "  frontend_verify,print,verify:\n",
    );
    let check = fixture.run("build", &["--check", "--agent"]);
    assert!(check.status.success(), "{check:?}");
    assert!(check.stderr.is_empty(), "{check:?}");
    let text = report(&check);
    assert!(text.contains(expected), "{text}");
    assert_eq!(text.matches("comptime,print,constant:").count(), 1);
    let human = fixture.run("build", &["--check"]);
    assert!(human.status.success(), "{human:?}");
    assert_eq!(human.stderr, b"constant:expression:verify:");
    assert!(report(&human).starts_with("check ok: "));
    let profile = fixture.run("run", &["--profile", "--agent"]);
    assert!(!profile.status.success(), "{profile:?}");
    assert!(profile.stderr.is_empty(), "{profile:?}");
    let text = report(&profile);
    assert!(
        text.contains("error: profiler: backend unsupported\n"),
        "{text}"
    );
    assert!(text.contains(expected), "{text}");
    assert!(
        !text.contains("stdout: 14"),
        "main must not execute: {text}"
    );
    let human_profile = fixture.run("run", &["--profile"]);
    assert!(!human_profile.status.success(), "{human_profile:?}");
    assert!(human_profile.stdout.is_empty(), "{human_profile:?}");
    assert_eq!(
        human_profile.stderr,
        b"constant:expression:verify:profiler: backend unsupported\n"
    );
}

#[test]
fn test_agent_associates_comptime_and_block_events_while_human_summary_stays_on_stdout() {
    let fixture = Fixture::new(FRONTEND_SOURCE);
    let agent = fixture.run("test", &["--agent"]);
    assert!(agent.status.success(), "{agent:?}");
    assert!(agent.stderr.is_empty(), "{agent:?}");
    let text = report(&agent);
    let file = escaped_path(&fixture.source());
    assert!(
        text.contains("debug[3]{file,block,phase,kind,text}:\n"),
        "{text}"
    );
    for row in [
        format!("  {file},,comptime,print,constant:\n"),
        format!("  {file},,comptime,print,expression:\n"),
        format!("  {file},checking,frontend_verify,print,verify:\n"),
    ] {
        assert!(text.contains(&row), "{text}");
    }
    let human = fixture.run("test", &[]);
    assert!(human.status.success(), "{human:?}");
    assert_eq!(human.stderr, b"constant:expression:verify:");
    assert!(report(&human).contains("verify checking: ok"));
}

#[test]
fn failed_comptime_observations_survive_build_and_test_error_envelopes() {
    let fixture = Fixture::new(
        r#"namespace app
function reject(label: string) returns int64:
    print(view label)
    string rejected = string.repeat("ab", 9223372036854775807)
    return string.char_count(rejected)
function main() returns nothing:
    int64 value = comptime reject("failed:")
    return nothing
"#,
    );
    for (command, flags) in [
        ("build", vec!["--check", "--agent"]),
        ("test", vec!["--agent"]),
    ] {
        let output = fixture.run(command, &flags);
        assert!(!output.status.success(), "{command}: {output:?}");
        assert!(output.stderr.is_empty(), "{command}: {output:?}");
        let text = report(&output);
        assert!(text.starts_with("status: error\n"), "{text}");
        assert!(
            text.contains("string.repeat: requested output is too large"),
            "{text}"
        );
        assert_eq!(text.matches("comptime,print,failed:").count(), 1, "{text}");
    }
}

#[test]
fn native_setup_failure_keeps_the_object_builds_captured_frontend_events() {
    let fixture = Fixture::new(FRONTEND_SOURCE);
    let missing = fixture.0.join("missing-launcher.json");
    let output = Command::new(env!("CARGO_BIN_EXE_jett"))
        .arg("build")
        .arg(fixture.source())
        .arg("--agent")
        .arg("--runtime-bundle")
        .arg(missing)
        .output()
        .unwrap();
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = report(&output);
    assert!(text.starts_with("status: error\n"), "{text}");
    assert!(text.contains("missing-launcher.json"), "{text}");
    assert!(
        text.contains(concat!(
            "debug[3]{phase,kind,text}:\n",
            "  comptime,print,constant:\n",
            "  comptime,print,expression:\n",
            "  frontend_verify,print,verify:\n",
        )),
        "{text}"
    );
}

#[test]
fn release_rejection_preserves_existing_publication_and_emits_no_debug_events() {
    let fixture = Fixture::new(
        r#"namespace app
function main() returns nothing:
    println("must not execute")
"#,
    );
    let destination = fixture.0.join("program");
    std::fs::write(&destination, b"existing publication").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_jett"))
        .arg("build")
        .arg(fixture.source())
        .arg("--release")
        .arg("--agent")
        .arg("--output")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = report(&output);
    assert!(text.contains("E0362"), "{text}");
    assert!(text.contains("debug[0]{phase,kind,text}:\n"), "{text}");
    assert_eq!(std::fs::read(destination).unwrap(), b"existing publication");
}

const BUNDLE_SOURCE: &str = r#"namespace app
function observe(label: string) returns int64:
    print(view label)
    return 7
int64 cached = comptime observe("bundle,\r\n:")
verify checking:
    println("trace public", cached)
    assert cached == 7
"#;

fn bundle(fixture: &Fixture, destination: &Path, agent: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jett"));
    command
        .arg("bundle")
        .arg(&fixture.0)
        .arg("--output")
        .arg(destination);
    if agent {
        command.arg("--agent");
    }
    command.output().unwrap()
}

#[test]
fn bundle_success_renders_validation_observations_without_protocol_or_stdout_leakage() {
    let fixture = Fixture::new(BUNDLE_SOURCE);
    std::fs::write(
        fixture.0.join("jett.proj"),
        "name: bundle_debug\nentry: app.jett\n",
    )
    .unwrap();
    let destination = fixture.0.join("target").join("library.jett");
    let agent = bundle(&fixture, &destination, true);
    assert!(agent.status.success(), "{agent:?}");
    assert!(agent.stderr.is_empty(), "{agent:?}");
    let text = report(&agent);
    assert!(text.starts_with("status: ok\n"), "{text}");
    assert!(
        text.contains(concat!(
            "debug[2]{phase,kind,text}:\n",
            "  comptime,print,bundle\\,\\r\\n:\n",
            "  frontend_verify,println,trace public 7\\n\n",
        )),
        "{text}"
    );
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("status:"))
            .count(),
        1
    );
    assert!(destination.is_file());
    // The second actual bundle excludes its output in the skipped target directory.
    let human = bundle(&fixture, &destination, false);
    assert!(human.status.success(), "{human:?}");
    assert_eq!(human.stderr, b"bundle,\r\n:trace public 7\n");
    assert!(report(&human).starts_with("bundled 1 files into "));
}

#[test]
fn bundle_publication_failure_retains_validation_capture_and_operational_error_identity() {
    let fixture = Fixture::new(BUNDLE_SOURCE);
    std::fs::write(
        fixture.0.join("jett.proj"),
        "name: bundle_debug\nentry: app.jett\n",
    )
    .unwrap();
    let destination = fixture.0.join("target").join("blocked.jett");
    std::fs::create_dir_all(&destination).unwrap();
    let agent = bundle(&fixture, &destination, true);
    assert!(!agent.status.success(), "{agent:?}");
    assert!(agent.stderr.is_empty(), "{agent:?}");
    let text = report(&agent);
    assert!(text.starts_with("status: error\n"), "{text}");
    assert!(text.contains("error: failed to write "), "{text}");
    assert!(!text.contains("kind: validation"), "{text}");
    assert!(!text.contains("diagnostics["), "{text}");
    assert!(
        text.contains(concat!(
            "debug[2]{phase,kind,text}:\n",
            "  comptime,print,bundle\\,\\r\\n:\n",
            "  frontend_verify,println,trace public 7\\n\n",
        )),
        "{text}"
    );
    assert!(destination.is_dir());
    assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 0);
    let human = bundle(&fixture, &destination, false);
    assert!(!human.status.success(), "{human:?}");
    assert!(human.stdout.is_empty(), "{human:?}");
    let stderr = String::from_utf8(human.stderr).unwrap();
    assert!(
        stderr.starts_with("bundle,\r\n:trace public 7\nbundle error: failed to write "),
        "{stderr}"
    );
    assert!(destination.is_dir());
}
