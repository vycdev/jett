use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn empty_string_repeat_with_huge_count_finishes_promptly() {
    let fixture = std::env::temp_dir().join(format!(
        "jett-empty-string-repeat-{}.jett",
        std::process::id()
    ));
    fs::write(
        &fixture,
        r#"function main() returns nothing:
    string output = string.repeat("", 9223372036854775807)
    print(output)
    return nothing
"#,
    )
    .expect("temporary Jett fixture should be writable");

    let mut child = Command::new(env!("CARGO_BIN_EXE_jett"))
        .arg("run")
        .arg(&fixture)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("jett run should start");
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .expect("jett run status should be readable")
        {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().expect("timed-out jett process should stop");
            child
                .wait()
                .expect("timed-out jett process should be reaped");
            break None;
        }
        thread::sleep(Duration::from_millis(10));
    };
    fs::remove_file(&fixture).expect("temporary Jett fixture should be removable");

    let Some(status) = status else {
        panic!("repeating an empty string consumed the huge count instead of returning");
    };
    assert!(status.success(), "jett run should accept the repeat result");
}
