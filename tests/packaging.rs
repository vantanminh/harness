use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
        let symlink_guard = if name == "windows.ps1" {
            "Assert-NoReparsePointPath"
        } else {
            "assert_no_symlink_components"
        };
        assert!(
            text.contains(symlink_guard),
            "{name} must reject symlinked install paths"
        );
        assert!(!text.contains(",,}"), "{name} must run on stock macOS bash");
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
    for workflow in [&ci, &rel] {
        assert!(!workflow.contains("uses: actions/checkout@v"));
        assert!(!workflow.contains("uses: actions/setup-node@v"));
        assert!(!workflow.contains("uses: actions/upload-artifact@v"));
        assert!(!workflow.contains("uses: actions/download-artifact@v"));
        assert!(!workflow.contains("dtolnay/rust-toolchain@stable"));
        assert!(!workflow.contains("npm@latest"));
        assert!(!workflow.contains("node-version: \"24.x\""));
        assert!(!workflow.contains("node-version: [\"22.x\", \"24.x\"]"));
        assert!(workflow.contains("SHA256SUMS"));
        assert!(
            workflow.contains("attest-build-provenance@c074443f1aee8d4aeeae555aebba3282517141b2")
        );
        assert!(!workflow.contains("NODE_AUTH_TOKEN"));
    }
    assert!(ci.contains("release-prep:"));
    assert!(ci.contains("needs: [build-test, rust-security, release-prep]"));
    assert!(ci.contains("needs.release-prep.outputs.ref || github.sha"));
    assert!(rel.contains("prepare:"));
    assert!(rel.contains("needs: prepare"));
    assert!(rel.contains("ref: ${{ needs.prepare.outputs.ref }}"));
    let codeql = fs::read_to_string(root().join(".github/workflows/codeql.yml")).unwrap();
    assert!(codeql.contains("security-events: write"));
    assert!(codeql.contains("github/codeql-action/init@3e6af16ff035267728e2ebc35df5d4c4cf249f81a"));
    assert!(
        codeql.contains("github/codeql-action/autobuild@3e6af16ff035267728e2ebc35df5d4c4cf249f81a")
    );
    assert!(
        codeql.contains("github/codeql-action/analyze@3e6af16ff035267728e2ebc35df5d4c4cf249f81a")
    );
    assert!(!codeql.contains("github/codeql-action/init@v"));
    assert!(!codeql.contains("github/codeql-action/autobuild@v"));
    assert!(!codeql.contains("github/codeql-action/analyze@v"));
    let dependabot = fs::read_to_string(root().join(".github/dependabot.yml")).unwrap();
    assert!(dependabot.contains("package-ecosystem: cargo"));
    assert!(fs::read_to_string(root().join("rust-toolchain.toml"))
        .unwrap()
        .contains("channel = \"1.97.1\""));
    assert!(fs::read_to_string(root().join("deny.toml"))
        .unwrap()
        .contains("unknown-registry = \"deny\""));
}

#[test]
fn security_docs_track_runtime_boundaries_and_versioned_installers() {
    let security = fs::read_to_string(root().join("docs/SECURITY.md")).unwrap();
    assert!(security.contains("Argon2id"));
    assert!(security.contains("--allow-project-command"));
    assert!(security.contains("1 MiB"));
    assert!(security.contains("src/error.rs"));
    assert!(!security.contains("src/infrastructure/"));

    let threat_model = fs::read_to_string(root().join("docs/THREAT_MODEL.md")).unwrap();
    for marker in [
        "Assets",
        "Trust boundaries",
        "Threats and mitigations",
        "Out of scope",
    ] {
        assert!(
            threat_model.contains(marker),
            "threat model missing {marker}"
        );
    }

    let readme = fs::read_to_string(root().join("README.md")).unwrap();
    assert!(!readme.contains("raw.githubusercontent.com/vantanminh/5harness/main/install"));
    assert!(!readme.contains("| iex"));
    assert!(!readme.contains("| bash"));

    let push = fs::read_to_string(root().join("scripts/git-push-release.mjs")).unwrap();
    assert!(push.find("pull",).unwrap() < push.find("Created tag").unwrap());
    assert!(push.contains("already points at"));
}

#[cfg(unix)]
#[test]
fn linux_installer_aborts_before_executing_checksum_mismatch() {
    use std::os::unix::fs::PermissionsExt;

    let root = root();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let source_dir = std::env::temp_dir().join(format!("harness-installer-integrity-{nonce}"));
    let prefix = source_dir.join("prefix");
    fs::create_dir_all(&source_dir).unwrap();
    let marker = source_dir.join("executed");
    let binary = source_dir.join("harness");
    fs::write(
        &binary,
        format!("#!/bin/sh\nprintf executed > '{}'\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();

    let output = Command::new("bash")
        .arg(root.join("install/linux.sh"))
        .env("HARNESS_INSTALL_FROM", &source_dir)
        .env("HARNESS_INSTALL_PREFIX", &prefix)
        .env("HARNESS_INSTALL_SKIP_PATH", "1")
        .env("HARNESS_INSTALL_EXPECTED_SHA256", "0".repeat(64))
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SHA-256 mismatch"), "{stderr}");
    assert!(!marker.exists(), "installer executed an unverified binary");
    assert!(!prefix.join("bin/harness").exists());
    let _ = fs::remove_dir_all(source_dir);
}

#[cfg(unix)]
#[test]
fn linux_installer_refuses_symlinked_destination() {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::{symlink, PermissionsExt};

    let root = root();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let source_dir = std::env::temp_dir().join(format!("harness-installer-dest-{nonce}"));
    let prefix = source_dir.join("prefix");
    fs::create_dir_all(prefix.join("bin")).unwrap();
    let binary = source_dir.join("harness");
    fs::write(&binary, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let digest = hex::encode(Sha256::digest(fs::read(&binary).unwrap()));
    let outside = source_dir.join("outside");
    fs::write(&outside, "untouched").unwrap();
    symlink(&outside, prefix.join("bin/harness")).unwrap();

    let output = Command::new("bash")
        .arg(root.join("install/linux.sh"))
        .env("HARNESS_INSTALL_FROM", &source_dir)
        .env("HARNESS_INSTALL_PREFIX", &prefix)
        .env("HARNESS_INSTALL_SKIP_PATH", "1")
        .env("HARNESS_INSTALL_EXPECTED_SHA256", digest)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("symlinked"), "{stderr}");
    assert_eq!(fs::read_to_string(outside).unwrap(), "untouched");
    let _ = fs::remove_dir_all(source_dir);
}

#[cfg(unix)]
#[test]
fn linux_installer_refuses_symlinked_binary_directory() {
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::{symlink, PermissionsExt};

    let root = root();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let source_dir = std::env::temp_dir().join(format!("harness-installer-bin-dir-{nonce}"));
    let prefix = source_dir.join("prefix");
    let outside = source_dir.join("outside");
    fs::create_dir_all(&prefix).unwrap();
    fs::create_dir_all(&outside).unwrap();
    symlink(&outside, prefix.join("bin")).unwrap();
    let binary = source_dir.join("harness");
    fs::write(&binary, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
    let digest = hex::encode(Sha256::digest(fs::read(&binary).unwrap()));

    let output = Command::new("bash")
        .arg(root.join("install/linux.sh"))
        .env("HARNESS_INSTALL_FROM", &source_dir)
        .env("HARNESS_INSTALL_PREFIX", &prefix)
        .env("HARNESS_INSTALL_SKIP_PATH", "1")
        .env("HARNESS_INSTALL_EXPECTED_SHA256", digest)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("symlinked"), "{stderr}");
    assert!(!outside.join("harness").exists());
    let _ = fs::remove_dir_all(source_dir);
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
