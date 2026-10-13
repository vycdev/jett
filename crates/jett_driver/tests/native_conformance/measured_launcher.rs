//! Reuse explicitly supplied normal bundles in native integration tests.
use jett_driver::native::NativeLauncherBundle;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::OnceLock;

const DEBUG_MANIFEST: &str = "JETT_NATIVE_TEST_DEBUG_BUNDLE_MANIFEST";
const RELEASE_MANIFEST: &str = "JETT_NATIVE_TEST_RELEASE_BUNDLE_MANIFEST";

fn selected_manifest(
    debug: Option<OsString>,
    release: Option<OsString>,
    use_release: bool,
) -> Result<Option<PathBuf>, String> {
    match (debug, release) {
        (None, None) => Ok(None),
        (Some(debug), Some(release)) => {
            if debug.is_empty() || release.is_empty() {
                return Err("supplied native test bundle manifests must be nonempty".into());
            }
            let debug = PathBuf::from(debug);
            let release = PathBuf::from(release);
            if !debug.is_absolute() || !release.is_absolute() {
                return Err("supplied native test bundle manifests must be absolute paths".into());
            }
            Ok(Some(if use_release { release } else { debug }))
        }
        _ => Err(format!(
            "{DEBUG_MANIFEST} and {RELEASE_MANIFEST} must be supplied together"
        )),
    }
}

pub(super) fn supplied(use_release: bool) -> Option<&'static NativeLauncherBundle> {
    static DEBUG: OnceLock<(PathBuf, NativeLauncherBundle)> = OnceLock::new();
    static RELEASE: OnceLock<(PathBuf, NativeLauncherBundle)> = OnceLock::new();
    let manifest = selected_manifest(
        std::env::var_os(DEBUG_MANIFEST),
        std::env::var_os(RELEASE_MANIFEST),
        use_release,
    )
    .expect("explicit native test bundle selection")?;
    let slot = if use_release { &RELEASE } else { &DEBUG };
    let selected = slot.get_or_init(|| {
        let bundle = NativeLauncherBundle::from_manifest(&manifest, use_release)
            .expect("explicit native test bundle must pass normal manifest validation");
        (manifest.clone(), bundle)
    });
    assert_eq!(
        selected.0, manifest,
        "explicit native test bundle changed after selection"
    );
    Some(&selected.1)
}

#[test]
fn explicit_manifest_selection_requires_a_complete_nonempty_pair() {
    assert_eq!(selected_manifest(None, None, false).unwrap(), None);
    assert!(selected_manifest(Some("debug.json".into()), None, false).is_err());
    assert!(selected_manifest(None, Some("release.json".into()), true).is_err());
    assert!(selected_manifest(Some(OsString::new()), Some("release.json".into()), false).is_err());
    assert!(selected_manifest(Some("debug.json".into()), Some(OsString::new()), true).is_err());
    assert!(
        selected_manifest(
            Some("debug.json".into()),
            Some("release.json".into()),
            false
        )
        .is_err()
    );
}

#[test]
fn explicit_manifest_selection_uses_the_requested_runtime_profile() {
    let directory = tempfile::tempdir().unwrap();
    let debug = directory.path().join("debug.json");
    let release = directory.path().join("release.json");
    for use_release in [false, true] {
        assert_eq!(
            selected_manifest(
                Some(debug.as_os_str().to_owned()),
                Some(release.as_os_str().to_owned()),
                use_release,
            )
            .unwrap(),
            Some(if use_release {
                release.clone()
            } else {
                debug.clone()
            }),
        );
    }
}

#[test]
fn explicit_manifest_validation_rejects_missing_invalid_and_wrong_profile_bundles() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = directory.path().join("runtime.json");
    assert!(NativeLauncherBundle::from_manifest(&manifest, false).is_err());
    std::fs::write(&manifest, b"invalid JSON").unwrap();
    assert!(NativeLauncherBundle::from_manifest(&manifest, false).is_err());
    std::fs::write(
        &manifest,
        serde_json::to_vec(&serde_json::json!({
            "manifest_version": 1,
            "compiler_version": env!("CARGO_PKG_VERSION"),
            "profile": "debug",
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(
        NativeLauncherBundle::from_manifest(&manifest, true)
            .unwrap_err()
            .contains("profile mismatch")
    );
}
