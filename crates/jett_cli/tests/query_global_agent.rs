use std::process::Command;

fn run_invalid_project_query(flag: &str, value: Option<&str>) -> String {
    let unique = format!(
        "jett-global-query-{flag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should follow the Unix epoch")
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&root).expect("temporary query directory should be created");
    std::fs::write(root.join("jett.proj"), "name: query_fixture\n")
        .expect("project marker should be written");
    std::fs::write(
        root.join("invalid.jett"),
        "function broken( returns nothing:\n    return nothing\n",
    )
    .expect("invalid query source should be written");

    let mut command = Command::new(env!("CARGO_BIN_EXE_jett"));
    command
        .current_dir(&root)
        .arg("query")
        .arg("--agent")
        .arg(flag);
    if let Some(value) = value {
        command.arg(value);
    }
    let output = command.output().expect("global query command should run");
    std::fs::remove_dir_all(&root).expect("temporary query directory should be removed");

    assert!(
        !output.status.success(),
        "invalid project should fail the query"
    );
    String::from_utf8(output.stdout).expect("agent output should be UTF-8")
}

fn assert_structured_parse_diagnostics(stdout: &str) {
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
        !stdout.contains("error: query support parse errors"),
        "compiler diagnostics were flattened into prose:\n{stdout}"
    );
}

#[test]
fn namespace_query_agent_error_preserves_project_parse_diagnostics() {
    let stdout = run_invalid_project_query("--namespaces", None);
    assert_structured_parse_diagnostics(&stdout);
}

#[test]
fn signature_query_agent_error_preserves_project_parse_diagnostics() {
    let stdout = run_invalid_project_query("--signature", Some("app.main"));
    assert_structured_parse_diagnostics(&stdout);
}
