use std::process::Command;

fn unique_temp_dir(label: &str) -> std::path::PathBuf {
    let unique = format!(
        "jett-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after the Unix epoch")
            .as_nanos()
    );
    std::env::temp_dir().join(unique)
}

#[test]
fn completion_query_agent_error_preserves_parse_diagnostics() {
    let root = unique_temp_dir("completion-query");
    std::fs::create_dir_all(&root).expect("temporary query directory should be created");
    let file = root.join("invalid.jett");
    std::fs::write(
        &file,
        "namespace app\n\nfunction broken( returns int64:\n    return 1\n",
    )
    .expect("invalid fixture should be written");

    let output = Command::new(env!("CARGO_BIN_EXE_jett"))
        .args([
            "query",
            "--agent",
            "--complete-at",
            &format!("{}:3:10", file.display()),
        ])
        .output()
        .expect("jett query should run");

    std::fs::remove_dir_all(&root).expect("temporary query directory should be removed");

    assert!(
        !output.status.success(),
        "invalid source should fail the query"
    );
    let stdout = String::from_utf8(output.stdout).expect("agent output should be UTF-8");
    assert!(stdout.starts_with("status: error\nfile: "), "{stdout}");
    assert!(
        stdout.contains("diagnostics["),
        "structured diagnostics missing from output:\n{stdout}"
    );
    assert!(
        stdout.contains("{code,severity,message,file,line,column,end_line,end_column}:"),
        "diagnostic columns missing from output:\n{stdout}"
    );
    assert!(
        !stdout.contains("error: parse errors"),
        "compiler diagnostics were flattened into prose:\n{stdout}"
    );
}
