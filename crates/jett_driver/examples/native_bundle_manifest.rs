//! Packaging helper: serialize the driver's canonical linker contract.
use jett_driver::native::{self, NativeLauncherBundle};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let archive = PathBuf::from(args.next().ok_or("expected archive path")?);
    let profile = args.next().ok_or("expected debug or release profile")?;
    let profile = profile.to_str().ok_or("profile must be UTF-8")?;
    if !matches!(profile, "debug" | "release") || args.next().is_some() {
        return Err("usage: native_bundle_manifest ARCHIVE debug|release".into());
    }
    let bundle = match native::host_target().as_str() {
        native::LINUX_GNU_NATIVE_TARGET => NativeLauncherBundle::linux_gnu_v1(&archive),
        native::WINDOWS_MSVC_NATIVE_TARGET => {
            NativeLauncherBundle::windows_msvc_static_v1(&archive)
        }
        _ => return Err("unsupported packaging host".into()),
    };
    let libraries = bundle
        .native_library_args
        .iter()
        .map(|arg| arg.to_str().ok_or("library argument must be UTF-8"))
        .collect::<Result<Vec<_>, _>>()?;
    let value = serde_json::json!({
        "manifest_version": 1,
        "compiler_version": env!("CARGO_PKG_VERSION"),
        "target": bundle.target,
        "runtime_abi_version": bundle.runtime_abi_version,
        "crt_mode": bundle.crt_mode.to_string(),
        "profile": profile,
        "archive": archive.file_name().and_then(|name| name.to_str()).ok_or("archive name must be UTF-8")?,
        "native_library_args": libraries,
    });
    let manifest = archive.with_file_name("launcher.json");
    std::fs::write(
        &manifest,
        format!("{}\n", serde_json::to_string_pretty(&value)?),
    )?;
    NativeLauncherBundle::from_manifest(&manifest, profile == "release")?;
    Ok(())
}
