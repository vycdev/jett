use std::path::PathBuf;
use std::process::Command;

#[test]
fn build_rejects_cross_target_before_reading_source() {
    for target in ["wasm32-unknown-unknown", "not-a-target"] {
        let output = Command::new(env!("CARGO_BIN_EXE_jett"))
            .args(["build", "absent-source.jett", "--target", target])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("unsupported native target"), "{stderr}");
        assert!(stderr.contains(target), "{stderr}");
        assert!(
            stderr.contains(&jett_driver::native::host_target()),
            "{stderr}"
        );
        assert!(!stderr.contains("No such file"), "{stderr}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn build_accepts_explicit_supported_host() {
    let host = jett_driver::native::host_target();
    if !matches!(
        host.as_str(),
        jett_driver::native::LINUX_GNU_NATIVE_TARGET
            | jett_driver::native::WINDOWS_MSVC_NATIVE_TARGET
    ) {
        return;
    }
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/run_pass/native_scalar_entry.jett");
    let output = Command::new(env!("CARGO_BIN_EXE_jett"))
        .arg("build")
        .arg(source)
        .args(["--target", &host])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}
