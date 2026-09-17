use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use jett_driver::{run_file_capture_outcome, run_file_capture_output};

static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TemporarySource {
    root: PathBuf,
    path: PathBuf,
}

impl TemporarySource {
    fn new(source: &str) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should follow the Unix epoch")
            .as_nanos();
        let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "jett_captured_run_failure_{}_{}_{}",
            std::process::id(),
            timestamp,
            sequence
        ));
        fs::create_dir(&root).expect("temporary source directory should be created");
        let path = root.join("main.jett");
        fs::write(&path, source).expect("temporary Jett source should be written");
        Self { root, path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporarySource {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn captured_run_failure_retains_stdout_and_debug_output() {
    let fixture = TemporarySource::new(
        r#"namespace app
function main(stdout: Stdout) returns nothing:
    Stdout.write(view stdout, "before failure")
    int64 marker = 42
    trace marker
    string impossible = string.repeat("ab", 9223372036854775807)
    Stdout.write(view stdout, impossible)
"#,
    );

    let failure = run_file_capture_outcome(fixture.path())
        .expect_err("the oversized repeat should fail after producing output");
    let expected_message = "runtime error: string.repeat: requested output is too large";

    assert_eq!(failure.message, expected_message);
    assert_eq!(failure.to_string(), expected_message);
    assert_eq!(failure.output.stdout, "before failure");
    assert_eq!(failure.output.debug_output, ["trace marker: int64 = 42"]);

    assert_eq!(
        run_file_capture_output(fixture.path())
            .expect_err("the legacy capture API should preserve its String failure"),
        expected_message
    );
}
