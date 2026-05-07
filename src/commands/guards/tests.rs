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
fn installed_shim_invokes_airis_exec_with_cmd_only() {
    // Phase 1b contract: shim hands off to `airis exec <cmd>` and lets the
    // CLI auto-route to the right service based on the command's runtime
    // family. The earlier Phase 0 form `airis exec workspace <cmd>` worked
    // around the broken CLI signature; now that's no longer needed.
    let dir = tempfile::tempdir().unwrap();
    install_global_guard(dir.path(), "pnpm", GuardLevel::Enforce).unwrap();

    let content = fs::read_to_string(dir.path().join("pnpm")).unwrap();
    assert!(
        content.contains("exec airis exec pnpm \"$@\""),
        "shim must hand off to `airis exec <cmd>` for auto-routing. Got:\n{content}"
    );
    assert!(
        !content.contains("exec airis exec workspace"),
        "Phase 0 workaround must be gone now that auto-routing exists. Got:\n{content}"
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
fn enforce_passes_through_outside_airis_context() {
    // Guards only enforce Docker-first hygiene *inside* a workspace.
    // Outside any airis context, the shim must pass through regardless of level.
    // The real command doesn't exist here, so we expect 127 (not found) — not
    // a guard-imposed error code like 126.
    let bin_dir = tempfile::tempdir().unwrap();
    install_global_guard(bin_dir.path(), "fakecmd", GuardLevel::Enforce).unwrap();

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
        Some(127),
        "outside airis context, shim must pass through (exit 127 = real cmd not found).\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[cfg(unix)]
#[test]
fn airis_bypass_skips_guard() {
    // AIRIS_BYPASS=1 must short-circuit all guard logic and exec the real command.
    // We use `true` (always exits 0) as the guarded command.
    let bin_dir = tempfile::tempdir().unwrap();
    install_global_guard(bin_dir.path(), "true", GuardLevel::Enforce).unwrap();

    let tmp_home = tempfile::tempdir().unwrap();
    let work_dir = tmp_home.path().join("work");
    // Plant a manifest.toml so the guard would normally route to Docker.
    fs::create_dir_all(&work_dir).unwrap();
    fs::write(work_dir.join("manifest.toml"), "[workspace]\n").unwrap();

    let script = bin_dir.path().join("true");
    let output = Command::new("bash")
        .arg(&script)
        .current_dir(&work_dir)
        .env_clear()
        .env("HOME", tmp_home.path())
        .env("PATH", "/usr/bin:/bin")
        .env("AIRIS_BYPASS", "1")
        .output()
        .expect("bash must be available");

    assert_eq!(
        output.status.code(),
        Some(0),
        "AIRIS_BYPASS=1 must exec the real command and exit 0.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
