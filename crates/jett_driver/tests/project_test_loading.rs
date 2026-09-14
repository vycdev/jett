use jett_driver::{build_file, test_file, test_project};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static FIXTURE_NUMBER: AtomicU64 = AtomicU64::new(0);

struct ProjectFixture {
    root: PathBuf,
}

impl ProjectFixture {
    fn new() -> Self {
        let number = FIXTURE_NUMBER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "jett-driver-project-test-{}-{number}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create unique project fixture");
        fs::create_dir(root.join("src")).expect("create source directory");
        fs::write(
            root.join("jett.proj"),
            "name: project_test_loading\nversion: 0.1.0\nentry: src/main.jett\n",
        )
        .expect("write project manifest");
        Self { root }
    }

    fn write(&self, path: &str, source: &str) {
        fs::write(self.root.join(path), source).expect("write project source");
    }

    fn with_dependencies() -> Self {
        let fixture = Self::new();
        fixture.write(
            "src/00_core.jett",
            "namespace core\nexport function answer() returns int64:\n    return 7\nverify core_answer:\n    assert answer() == 7\n",
        );
        fixture.write(
            "src/10_report.jett",
            "namespace report\nexport function doubled() returns int64:\n    use core\n    return core.answer() * 2\nverify doubled_answer:\n    assert doubled() == 14\n",
        );
        fixture.write(
            "src/main.jett",
            "namespace app\nfunction main() returns int64:\n    use report\n    return report.doubled()\nverify final_answer:\n    assert main() == 14\n",
        );
        fixture
    }
}

impl Drop for ProjectFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn project_tests_keep_dependencies_before_each_selected_file() {
    let fixture = ProjectFixture::with_dependencies();
    let outcome = test_project(&fixture.root).expect("test dependent project files");
    assert_eq!(outcome.total_files, 3);
    assert_eq!(outcome.total_blocks, 3);
    assert_eq!(outcome.total_passed, 3);
    assert_eq!(outcome.total_failed, 0);
    for file in outcome.file_results {
        assert_eq!(file.total, 1, "each file contributes its own check once");
        assert_eq!(file.passed, 1);
    }
}

#[test]
fn selected_file_tests_run_only_the_selected_checks() {
    let fixture = ProjectFixture::with_dependencies();
    for file in ["00_core.jett", "10_report.jett", "main.jett"] {
        let outcome = test_file(&fixture.root.join("src").join(file))
            .expect("test a file that also has project dependents");
        assert_eq!(outcome.total, 1, "selected {file}");
        assert_eq!(outcome.passed, 1, "selected {file}");
        assert_eq!(outcome.failed, 0, "selected {file}");
    }
}

#[test]
fn project_tests_keep_the_manifest_entry_last() {
    let fixture = ProjectFixture::new();
    fixture.write(
        "jett.proj",
        "name: early_entry\nversion: 0.1.0\nentry: src/00_checks.jett\n",
    );
    fixture.write(
        "src/00_checks.jett",
        "namespace app\nverify imported_answer:\n    use model\n    assert model.answer() == 7\n",
    );
    fixture.write(
        "src/model.jett",
        "namespace model\nexport function answer() returns int64:\n    return 7\nverify model_answer:\n    assert answer() == 7\n",
    );
    let outcome = test_project(&fixture.root).expect("lexically early manifest entry");
    assert_eq!(outcome.total_blocks, 2);
    assert_eq!(outcome.total_passed, 2);
    let selected = test_file(&fixture.root.join("src/model.jett")).expect("selected sibling");
    assert_eq!(selected.total, 1);
    assert_eq!(selected.passed, 1);
}

#[test]
fn relative_project_sources_work_from_project_root() {
    const CHILD_MARKER: &str = "JETT_DRIVER_RELATIVE_PROJECT_CHILD";
    if std::env::var_os(CHILD_MARKER).is_some() {
        let built = build_file(Path::new("src/main.jett"));
        assert!(!built.has_errors, "relative build: {:?}", built.diagnostics);
        let selected = test_file(Path::new("src/00_core.jett")).expect("relative file test");
        assert_eq!(selected.passed, 1);
        let project = test_project(Path::new(".")).expect("relative project test");
        assert_eq!(project.total_passed, 3);
        return;
    }

    let fixture = ProjectFixture::with_dependencies();
    let output = Command::new(std::env::current_exe().expect("integration test executable"))
        .args([
            "--exact",
            "relative_project_sources_work_from_project_root",
            "--nocapture",
        ])
        .current_dir(&fixture.root)
        .env(CHILD_MARKER, "1")
        .output()
        .expect("run isolated relative-path test");
    assert!(
        output.status.success(),
        "relative-path child failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn project_tests_preserve_declaration_order_inside_each_file() {
    let fixture = ProjectFixture::new();
    fixture.write(
        "src/main.jett",
        "namespace app\nfunction main() returns int64:\n    return later()\nfunction later() returns int64:\n    return 1\nverify answer:\n    assert main() == 1\n",
    );
    let error = test_project(&fixture.root)
        .err()
        .expect("forward reference fails");
    assert!(error.contains("E0205"), "{error}");
}

#[test]
fn project_tests_preserve_lexical_order_between_siblings() {
    let fixture = ProjectFixture::new();
    fixture.write(
        "src/00_caller.jett",
        "namespace caller\nexport function answer() returns int64:\n    use later\n    return later.value()\n",
    );
    fixture.write(
        "src/later.jett",
        "namespace later\nexport function value() returns int64:\n    return 1\nverify value_check:\n    assert value() == 1\n",
    );
    fixture.write(
        "src/main.jett",
        "namespace app\nfunction main() returns int64:\n    use caller\n    return caller.answer()\n",
    );
    let error = test_file(&fixture.root.join("src/later.jett"))
        .err()
        .expect("later sibling cannot satisfy an earlier forward reference");
    assert!(error.contains("E0205"), "{error}");
}

#[cfg(any(unix, windows))]
#[test]
fn project_tests_preserve_logical_source_symlink_order() {
    let fixture = ProjectFixture::with_dependencies();
    fs::create_dir(fixture.root.join("target")).expect("create excluded target directory");
    let linked_source = fixture.root.join("src/00_core.jett");
    fs::rename(&linked_source, fixture.root.join("target/z_core.jett"))
        .expect("move core source to an in-root symlink target");
    #[cfg(unix)]
    std::os::unix::fs::symlink("../target/z_core.jett", &linked_source)
        .expect("create in-root source symlink");
    #[cfg(windows)]
    if let Err(error) = std::os::windows::fs::symlink_file("../target/z_core.jett", &linked_source)
    {
        if error.kind() == std::io::ErrorKind::PermissionDenied
            || error.raw_os_error() == Some(1314)
        {
            eprintln!(
                "skipping source-symlink integration: Windows symlink permission unavailable"
            );
            return;
        }
        panic!("create in-root source symlink: {error}");
    }

    let built = build_file(&fixture.root.join("src/main.jett"));
    assert!(!built.has_errors, "symlink build: {:?}", built.diagnostics);
    for file in ["00_core.jett", "10_report.jett", "main.jett"] {
        let outcome = test_file(&fixture.root.join("src").join(file))
            .expect("selected-file tests preserve logical source order");
        assert_eq!(outcome.total, 1, "selected {file}");
        assert_eq!(outcome.passed, 1, "selected {file}");
    }
    let outcome =
        test_project(&fixture.root).expect("test a project with an in-root source symlink");
    assert_eq!(outcome.total_files, 3);
    assert_eq!(outcome.total_blocks, 3);
    assert_eq!(outcome.total_passed, 3);
    assert_eq!(outcome.total_failed, 0);
}
