use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn package_json_keeps_5harness_bins() {
    let pkg: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root().join("package.json")).unwrap()).unwrap();
    assert_eq!(pkg["name"], "5harness");
    assert_eq!(pkg["license"], "MIT");
    assert_eq!(pkg["bin"]["harness"], "dist/cli.js");
    assert_eq!(pkg["bin"]["5harness"], "dist/cli.js");
    assert_eq!(pkg["bin"]["5hn"], "dist/cli.js");
    assert!(pkg["files"]
        .as_array()
        .unwrap()
        .iter()
        .all(|entry| entry != "npm" && entry != "npm/"));
    for hook in ["preinstall", "install", "postinstall"] {
        assert!(
            pkg["scripts"].get(hook).is_none(),
            "unexpected install hook: {hook}"
        );
    }
}

#[test]
fn cargo_version_matches_package_json() {
    let pkg: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root().join("package.json")).unwrap()).unwrap();
    assert_eq!(pkg["version"].as_str().unwrap(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn all_native_install_scripts_exist_and_support_local_artifacts() {
    for name in ["linux.sh", "macos.sh", "windows.ps1"] {
        let path = root().join("install").join(name);
        assert!(path.is_file(), "{}", path.display());
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("HARNESS_INSTALL_FROM"),
            "{name} lacks offline install support"
        );
        assert!(
            text.contains("HARNESS_INSTALL_PREFIX"),
            "{name} lacks configurable prefix"
        );
        assert!(
            text.contains("--version"),
            "{name} lacks post-install smoke check"
        );
        assert!(
            text.contains("SHA256SUMS") && text.contains("HARNESS_INSTALL_EXPECTED_SHA256"),
            "{name} must verify a release checksum before execution"
        );
    }
    let linux = fs::read_to_string(root().join("install/linux.sh")).unwrap();
    assert!(linux.contains("x86_64-unknown-linux-gnu"));
    assert!(linux.contains("aarch64-unknown-linux-gnu"));
    let win = fs::read_to_string(root().join("install/windows.ps1")).unwrap();
    assert!(win.contains("Expand-Archive"));
    assert!(win.contains("harness-$Target.exe"));
    let stage = fs::read_to_string(root().join("scripts/stage-native.mjs")).unwrap();
    let checksums = fs::read_to_string(root().join("scripts/checksums.mjs")).unwrap();
    assert!(checksums.contains("sha256"));
    assert!(checksums.contains("SHA256SUMS"));
    for target in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc",
    ] {
        assert!(
            stage.contains(target),
            "native staging matrix lacks {target}"
        );
    }
}

#[test]
fn ci_still_publishes_to_npmjs_with_provenance() {
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    let rel = fs::read_to_string(root().join(".github/workflows/release.yml")).unwrap();
    assert!(ci.contains("npm publish --access public --provenance"));
    assert!(rel.contains("npm publish --access public --provenance"));
    assert!(ci.contains("id-token: write"));
    assert!(ci.contains("install/linux.sh"));
    assert!(ci.contains("install/macos.sh"));
    assert!(ci.contains("install/windows.ps1"));
    assert!(ci.contains("npm run install:smoke"));
    assert!(rel.contains("stage-native.mjs"));
    for target in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc",
    ] {
        assert!(ci.contains(target), "ci matrix lacks {target}");
        assert!(rel.contains(target), "release matrix lacks {target}");
    }
    for asset in [
        "bin/harness-x86_64-unknown-linux-gnu",
        "bin/harness-aarch64-unknown-linux-gnu",
        "bin/harness-x86_64-apple-darwin",
        "bin/harness-aarch64-apple-darwin",
        "bin/harness-x86_64-pc-windows-msvc.exe",
        "bin/harness-aarch64-pc-windows-msvc.exe",
    ] {
        assert!(ci.contains(asset), "ci release assets lack {asset}");
        assert!(rel.contains(asset), "release assets lack {asset}");
    }
}

#[test]
fn native_shim_does_not_load_typescript_cli() {
    let shim = fs::read_to_string(root().join("npm").join("shim.mjs")).unwrap();
    assert!(!shim.contains("src/cli.ts"));
    assert!(shim.contains("spawnSync"));
    assert!(shim.contains("shell: false"));
    assert!(!shim.contains("node:fs"));
    assert!(!shim.contains("process.env"));
    assert!(!shim.contains("HARNESS_NATIVE_BIN"));
}
