use std::fs;
use std::process::Command;

use crate::manifest::{GlobalConfig, GuardLevel};

use super::GLOBAL_GUARD_MARKER;
use super::scripts::install_global_guard;

#[test]
fn test_global_config_default() {
    let config = GlobalConfig::default();
    assert_eq!(config.version, 1);

    // Test through get_level instead of direct field access
    assert_eq!(config.guards.get_level("npm"), GuardLevel::Enforce);
    assert_eq!(config.guards.get_level("yarn"), GuardLevel::Enforce);
    assert_eq!(config.guards.get_level("pnpm"), GuardLevel::Enforce);
    assert_eq!(config.guards.get_level("bun"), GuardLevel::Enforce);
    assert_eq!(config.guards.get_level("npx"), GuardLevel::Enforce);
}

#[test]
fn test_global_config_paths() {
    let config_path = GlobalConfig::config_path();
    assert!(config_path.is_ok());

    let bin_dir = GlobalConfig::bin_dir();
    assert!(bin_dir.is_ok());

    let config_path = config_path.unwrap();
    assert!(config_path.to_string_lossy().contains(".airis"));
    assert!(config_path.to_string_lossy().contains("global-config.toml"));

    let bin_dir = bin_dir.unwrap();
    assert!(bin_dir.to_string_lossy().contains(".airis"));
    assert!(bin_dir.to_string_lossy().ends_with("bin"));
}

#[test]
fn test_global_config_serialization() {
    let config = GlobalConfig::default();
    let toml_str = toml::to_string_pretty(&config).unwrap();

    let parsed: GlobalConfig = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.version, config.version);
}

#[test]
fn installed_shim_invokes_airis_exec_workspace() {
    let dir = tempfile::tempdir().unwrap();
    install_global_guard(dir.path(), "pnpm", GuardLevel::Enforce).unwrap();

    let content = fs::read_to_string(dir.path().join("pnpm")).unwrap();
    assert!(
        content.contains("exec airis exec workspace pnpm \"$@\""),
        "shim must route through workspace service (Phase 0 contract). Got:\n{content}"
    );
}

#[test]
fn marker_present_in_generated_script() {
    let dir = tempfile::tempdir().unwrap();
    install_global_guard(dir.path(), "pnpm", GuardLevel::Warn).unwrap();

    let content = fs::read_to_string(dir.path().join("pnpm")).unwrap();
    assert!(
        content.contains(GLOBAL_GUARD_MARKER),
        "marker must appear so 'airis guards uninstall' can recognize the file"
    );
}

#[cfg(unix)]
#[test]
fn enforce_fails_outside_airis_context() {
    // Create a guard shim in a tempdir, then run it from another tempdir that
    // contains no manifest.toml / compose.yaml. The enforce branch must hit
    // and exit 126.
    let bin_dir = tempfile::tempdir().unwrap();
    install_global_guard(bin_dir.path(), "fakecmd", GuardLevel::Enforce).unwrap();

    // tmp_home: HOME for the shim. PWD lives below this so find_airis_context
    // walks up without finding a manifest.
    let tmp_home = tempfile::tempdir().unwrap();
    let work_dir = tmp_home.path().join("work");
    fs::create_dir_all(&work_dir).unwrap();

    let script = bin_dir.path().join("fakecmd");
    let output = Command::new("bash")
        .arg(&script)
        .current_dir(&work_dir)
        .env_clear()
        .env("HOME", tmp_home.path())
        .env("PATH", "/usr/bin:/bin")
        .output()
        .expect("bash must be available");

    assert_eq!(
        output.status.code(),
        Some(126),
        "enforce outside airis context must exit 126.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("is enforced"),
        "expected enforcement message in stderr, got: {stderr}"
    );
}
