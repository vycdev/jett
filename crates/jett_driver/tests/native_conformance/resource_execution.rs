//! Explicit matched single-runtime tests. Production launcher metadata is never used.
use super::{resource_cases as cases, resource_report as report};
use jett_common::{FileId, SourceOrigin};
use jett_parser::{ast::Item, parse};
use jett_resolve::ResourceKernelSpec;
use jett_typecheck::{CheckOptions, CheckedResourceProgram};
use jett_types::ResourceKernelRecipe;
use std::{
    collections::HashMap,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};

const RECEIPT_ENV: &str = "JETT_RESOURCE_NATIVE_TEST_ARCHIVE_RECEIPT_V1";
const RECEIPT_SHA256: &str = "54a38248f8975fad43e964100abdd14f2b2445c39ee37e8c3eb07f19357b541a";
struct Archive {
    path: PathBuf,
    target: String,
    profile: String,
    linker: PathBuf,
    libraries: Vec<OsString>,
    sdk_lib: Option<OsString>,
}
fn digest(bytes: &[u8]) -> String {
    jett_runtime::crypto::sha256_digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn field<'a>(value: &'a serde_json::Value, name: &str) -> &'a str {
    value
        .get(name)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("archive receipt lacks string {name}"))
}
fn measured_file(value: &serde_json::Value, path: &str, hash: &str) -> PathBuf {
    let path = PathBuf::from(field(value, path));
    assert!(path.is_absolute(), "receipt paths must be absolute");
    let metadata = fs::metadata(&path).unwrap();
    assert!(
        metadata.is_file() && metadata.len() <= 256 * 1024 * 1024,
        "{}",
        path.display()
    );
    let bytes = fs::read(&path).unwrap();
    assert_eq!(
        digest(&bytes),
        field(value, hash).to_ascii_lowercase(),
        "{}",
        path.display()
    );
    path
}
fn archive(release: bool) -> Archive {
    let path = PathBuf::from(
        std::env::var_os(RECEIPT_ENV)
            .expect("explicit Root-measured matched archive receipt is required"),
    );
    let bytes = fs::read(path).unwrap();
    assert!(bytes.len() <= 1024 * 1024);
    assert_eq!(
        digest(&bytes),
        RECEIPT_SHA256,
        "exact Root-measured archive receipt"
    );
    let receipt: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        receipt.get("version").and_then(serde_json::Value::as_u64),
        Some(1)
    );
    let profile = if release { "release" } else { "debug" };
    let rows = receipt
        .get("archives")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    let matching = rows
        .iter()
        .filter(|row| field(row, "profile") == profile)
        .collect::<Vec<_>>();
    let [row] = matching.as_slice() else {
        panic!("exact {profile} archive receipt must be unique");
    };
    let target = field(row, "target");
    assert_eq!(target, jett_driver::native::host_target());
    assert_eq!(
        row.get("runtime_abi").and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        row.get("layout_wire").and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        row.get("private_cfgs"),
        Some(&serde_json::json!([
            "test",
            "jett_resource_native_test_archive"
        ]))
    );
    assert!(matches!(field(row, "crt_mode"), "static" | "dynamic"));
    let path = measured_file(row, "archive_path", "archive_sha256");
    let linker = PathBuf::from(field(row, "linker_path"));
    assert!(linker.is_absolute() && linker.is_file());
    for name in ["build_receipt", "native_static_libs_receipt", "crt_receipt"] {
        let provenance = row.get(name).unwrap();
        let _ = measured_file(provenance, "path", "sha256");
    }
    let libraries = row
        .get("native_library_args")
        .and_then(serde_json::Value::as_array)
        .unwrap()
        .iter()
        .map(|item| {
            let value = item.as_str().expect("exact native library argument");
            let lower = value.to_ascii_lowercase();
            assert!(
                !lower.contains("jett_runtime") && !lower.contains("jett_native_launcher"),
                "a second Rust runtime archive is forbidden"
            );
            OsString::from(value)
        })
        .collect::<Vec<_>>();
    assert!(
        !libraries.is_empty(),
        "actual native-static-libs vector must be recorded"
    );
    let library_receipt = measured_file(
        row.get("native_static_libs_receipt").unwrap(),
        "path",
        "sha256",
    );
    let observed: serde_json::Value =
        serde_json::from_slice(&fs::read(library_receipt).unwrap()).unwrap();
    assert_eq!(
        row.get("native_library_args"),
        observed.get("arguments"),
        "exact measured library order and duplicates"
    );
    let crt_receipt = measured_file(row.get("crt_receipt").unwrap(), "path", "sha256");
    let crt: serde_json::Value = serde_json::from_slice(&fs::read(crt_receipt).unwrap()).unwrap();
    assert_eq!(field(row, "crt_mode"), field(&crt, "mode"));
    assert_eq!(field(row, "linker_path"), field(&crt, "linker_path"));
    let sdk_lib = crt
        .get("sdk_LIB")
        .and_then(serde_json::Value::as_str)
        .map(OsString::from);
    Archive {
        path,
        target: target.into(),
        profile: profile.into(),
        linker,
        libraries,
        sdk_lib,
    }
}
fn checked(
    project: &Path,
    stdlib: &Path,
    release: bool,
) -> (Arc<CheckedResourceProgram>, jett_common::Span) {
    let project_text = fs::read_to_string(project).unwrap();
    let support_text = fs::read_to_string(stdlib).unwrap();
    let primary = FileId::new(0);
    let support = FileId::new(10_000);
    let mut parsed = parse(&support_text, support);
    let project = parse(&project_text, primary);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    assert!(project.errors.is_empty(), "{:?}", project.errors);
    let entries = project
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function)
                if function.name.name == "main" && function.name.span.file == primary =>
            {
                Some(function.span)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [entry] = entries.as_slice() else {
        panic!("actual primary main declaration must be unique");
    };
    let resources = parsed
        .module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Resource(resource) => Some(resource.name.span),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [resource] = resources.as_slice() else {
        panic!("actual synthetic Stdlib Resource must be unique");
    };
    let catalog = [
        ("kernel_create", ResourceKernelRecipe::NetworkFactory),
        ("kernel_borrow", ResourceKernelRecipe::NetworkBorrow),
        ("kernel_close", ResourceKernelRecipe::Finalize),
    ]
    .into_iter()
    .map(|(member, recipe)| ResourceKernelSpec {
        resource_declaration: *resource,
        member: member.into(),
        recipe,
    })
    .collect::<Vec<_>>();
    parsed.module.items.extend(project.module.items);
    let checked = CheckedResourceProgram::prepare(
        parsed,
        HashMap::from([
            (primary, SourceOrigin::Project),
            (support, SourceOrigin::Stdlib),
        ]),
        &catalog,
        CheckOptions { release },
    )
    .unwrap_or_else(|error| {
        panic!(
            "original checked Source: {error:?}; {:?}",
            error.diagnostics()
        )
    });
    (Arc::new(checked), *entry)
}
fn object(project: &Path, stdlib: &Path, release: bool) -> jett_codegen_cranelift::ObjectArtifact {
    let (checked, span) = checked(project, stdlib, release);
    let hir = jett_hir::lower_checked_resource_program(&checked).unwrap();
    // Same original declaration span -> FunctionId join as the actual driver.
    let entries = hir
        .functions
        .iter()
        .filter(|function| {
            function.span == span
                && function.identity.declaration.origin == SourceOrigin::Project
                && function.identity.declaration.kind == jett_hir::DeclarationKind::Function
        })
        .map(|function| function.id)
        .collect::<Vec<_>>();
    let [entry] = entries.as_slice() else {
        panic!("original driver entry has no unique HIR identity");
    };
    let mir = jett_mir::lower(&hir, &checked.checked().interner).unwrap();
    jett_codegen_cranelift::emit_host_program_object_with_options(
        &mir,
        &checked.checked().interner,
        *entry,
        jett_codegen_cranelift::CodegenOptions { optimize: release },
    )
    .unwrap()
}
fn prefix(prefix: &str, path: &Path) -> OsString {
    let mut argument = OsString::from(prefix);
    argument.push(path.as_os_str());
    argument
}
fn command(archive: &Archive, object: &Path, binary: &Path) -> Command {
    #[cfg(windows)]
    let mut command = {
        let tool = find_msvc_tools::find_tool(&archive.target, "link.exe")
            .expect("same MSVC linker discovery used by driver");
        assert_eq!(
            fs::canonicalize(tool.path()).unwrap(),
            fs::canonicalize(&archive.linker).unwrap(),
            "measured linker identity"
        );
        let mut command = tool.to_command();
        let actual_lib = command
            .get_envs()
            .find_map(|(name, value)| {
                name.to_str()
                    .filter(|name| name.eq_ignore_ascii_case("LIB"))
                    .and(value)
            })
            .map(|value| value.to_os_string())
            .or_else(|| std::env::var_os("LIB"));
        assert_eq!(
            actual_lib, archive.sdk_lib,
            "preserved measured MSVC SDK LIB environment"
        );
        command.args([
            "/NOLOGO",
            "/MACHINE:X64",
            "/SUBSYSTEM:CONSOLE",
            "/INCREMENTAL:NO",
            "/MANIFEST:EMBED",
        ]);
        command
            .arg(prefix("/OUT:", binary))
            .arg(prefix("/PDB:", &binary.with_extension("pdb")));
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new(&archive.linker);
        command.arg("-o").arg(binary);
        command
    };
    command
        .arg(object)
        .arg(&archive.path)
        .args(&archive.libraries);
    command
}
fn link(archive: &Archive, object: &Path, binary: &Path, directory: &Path) {
    // Match the driver native link timeout, file-backed capture and kill+reap contract.
    let mut stdout = tempfile::NamedTempFile::new().unwrap();
    let mut stderr = tempfile::NamedTempFile::new().unwrap();
    let mut command = command(archive, object, binary);
    command
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(stdout.reopen().unwrap())
        .stderr(stderr.reopen().unwrap());
    let mut child =
        super::ExecutionChild(command.spawn().expect("link sole matched private runtime"));
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            start.elapsed() < jett_driver::native::DEFAULT_NATIVE_LINK_TIMEOUT,
            "Resource native linker exceeded 60 seconds"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let read = |file: &mut fs::File| {
        let mut bytes = Vec::new();
        file.take(file.metadata().unwrap().len())
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    };
    let out = read(stdout.as_file_mut());
    let err = read(stderr.as_file_mut());
    assert!(
        status.success(),
        "{status}; stdout={} stderr={}",
        String::from_utf8_lossy(&out),
        String::from_utf8_lossy(&err)
    );
    assert!(binary.is_file());
}
fn script(case: &cases::Case) -> String {
    let mut output = b"JTRST001".to_vec();
    output.extend(1u32.to_le_bytes());
    output.extend(0u32.to_le_bytes());
    output.extend(0u64.to_le_bytes());
    output.extend(1u32.to_le_bytes());
    output.extend((case.script.len() as u32).to_le_bytes());
    for row in case.script {
        let (tag, label) = match *row {
            cases::Script::Construct(label) => (1u32, label),
            cases::Script::ConstructFail(label, _) => (2, label),
            cases::Script::FinalizerPanic(label) => (3, label),
            cases::Script::Borrow(label, _) => (4, label),
        };
        output.extend(tag.to_le_bytes());
        output.extend(label.to_le_bytes());
        match *row {
            cases::Script::ConstructFail(_, error) => {
                output.extend((error.len() as u32).to_le_bytes());
                output.extend(error.as_bytes());
            }
            cases::Script::Borrow(_, value) => output.extend(value.to_le_bytes()),
            _ => {}
        }
    }
    let length = output.len() as u64;
    output[16..24].copy_from_slice(&length.to_le_bytes());
    output.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn event(expected: cases::Event) -> (i64, report::EventKind) {
    match expected {
        cases::Event::Constructed(label) => (label, report::EventKind::Constructed),
        cases::Event::ConstructionFailed(label) => (label, report::EventKind::ConstructionFailed),
        cases::Event::Borrowed(label) => (label, report::EventKind::Borrowed),
        cases::Event::Finalized(label) => (label, report::EventKind::Finalized),
    }
}
fn assert_report(case: &cases::Case, report: &report::Report) {
    assert_eq!(report.attempts.len(), 1, "{}", case.name);
    let attempt = &report.attempts[0];
    assert_eq!(
        attempt.completion.body_status, case.body,
        "{} original body",
        case.name
    );
    assert_eq!(
        attempt.completion.cleanup_status, case.cleanup,
        "{} cleanup",
        case.name
    );
    let channel = match case.channel {
        0 => report::FailureChannel::Clean,
        1 => report::FailureChannel::Body,
        2 => report::FailureChannel::HostPanic,
        3 => report::FailureChannel::Cleanup,
        _ => panic!("invalid oracle"),
    };
    assert_eq!(
        attempt.completion.selected_channel, channel,
        "{} channel",
        case.name
    );
    assert_eq!(
        attempt.ordinary_message, case.ordinary_message,
        "{} ordinary message",
        case.name
    );
    assert_eq!(
        attempt.resource_message, case.resource_message,
        "{} resource message",
        case.name
    );
    assert!(
        report.observations_ready_for_acceptance(),
        "{} zero BEFORE-teardown obligations: {report:?}",
        case.name
    );
    assert_eq!(report.events.len(), case.events.len());
    for (index, (actual, expected)) in report.events.iter().zip(case.events).enumerate() {
        let (label, kind) = event(*expected);
        assert_eq!(
            (actual.sequence, actual.label, actual.kind),
            ((index + 1) as u64, label, kind),
            "{} event {index}",
            case.name
        );
    }
    assert_eq!(attempt.event_sequence_end, case.events.len() as u64);
    assert_eq!(report.script_remaining, 0);
    assert_eq!(
        (
            report.created,
            report.destroy_attempts,
            report.destroy_status,
            report.write_status
        ),
        (true, 1, 0, 0)
    );
    assert_eq!(
        report.all_attempts_succeeded(),
        case.body == 0 && case.cleanup == 0 && case.channel == 0
    );
}
#[test]
#[ignore = "requires exact Root-measured double-cfg Debug/Release single-runtime archive receipt"]
fn native_resource_original_source_lifecycle_matches_reference_and_retires_before_teardown() {
    for release in [false, true] {
        let archive = archive(release);
        for case in cases::CASES {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path();
            let project = root.join("project");
            let stdlib = root.join("synthetic-stdlib");
            fs::create_dir(&project).unwrap();
            fs::create_dir(&stdlib).unwrap();
            let entry = project.join("main.jett");
            let support = stdlib.join("resource_probe.jett");
            fs::write(&entry, case.source).unwrap();
            fs::write(&support, cases::SUPPORT).unwrap();
            let artifact = object(&entry, &support, release);
            assert_eq!(artifact.target, archive.target);
            assert!(
                artifact
                    .symbols
                    .iter()
                    .any(|symbol| symbol == jett_codegen_cranelift::JETT_AOT_ENTRY_SYMBOL_V1)
            );
            assert!(
                artifact
                    .symbols
                    .iter()
                    .any(|symbol| symbol == "jett_aot_resource_v1_manifest")
            );
            let object_path = root.join("resource.obj");
            let binary = root.join(if cfg!(windows) {
                "resource.exe"
            } else {
                "resource"
            });
            fs::write(&object_path, artifact.bytes).unwrap();
            link(&archive, &object_path, &binary, root);
            // Native execution has neither primary Source nor the synthetic Stdlib files.
            fs::remove_dir_all(&project).unwrap();
            fs::remove_dir_all(&stdlib).unwrap();
            assert!(!entry.exists() && !support.exists() && !project.exists() && !stdlib.exists());
            let encoded = script(case);
            let actual = super::run_bounded_with_env(
                &binary,
                root,
                Some(("JETT_RESOURCE_NATIVE_TEST_SCRIPT_V1", &encoded)),
            );
            let path = root.join("resource-native-report-v1.bin");
            assert!(path.metadata().unwrap().len() <= 16 * 1024 * 1024);
            let bytes = fs::read(path).unwrap();
            let report = report::decode(&bytes).unwrap();
            assert_eq!(
                actual.status.code(),
                Some(case.exit),
                "{} {}: {actual:?}; {report:?}",
                case.name,
                archive.profile
            );
            assert!(actual.stdout.is_empty(), "{actual:?}");
            assert!(actual.stderr.is_empty(), "{actual:?}");
            assert_report(case, &report);
        }
    }
}
