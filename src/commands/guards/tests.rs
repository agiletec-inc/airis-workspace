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
        content.contains("exec airis exec 'pnpm' \"$@\""),
        "shim must hand off to `airis exec <cmd>` for auto-routing. Got:\n{content}"
    );
    assert!(
        !content.contains("exec airis exec workspace"),
        "Phase 0 workaround must be gone now that auto-routing exists. Got:\n{content}"
    );
}

#[test]
fn generated_script_single_quotes_command_name() {
    // Defense-in-depth (issue #247): the command name must be single-quoted at
    // every interpolation point so a future unvalidated command name cannot
    // break out of its shell token. `validate_guard_cmd` already restricts the
    // charset, but the template must not rely on that alone.
    let dir = tempfile::tempdir().unwrap();
    install_global_guard(dir.path(), "pnpm", GuardLevel::Enforce).unwrap();

    let content = fs::read_to_string(dir.path().join("pnpm")).unwrap();
    assert!(
        content.contains("REAL_CMD=$(find_real_cmd 'pnpm')"),
        "command name passed to find_real_cmd must be single-quoted. Got:\n{content}"
    );
    assert!(
        content.contains("exec airis exec 'pnpm' \"$@\""),
        "command name passed to `airis exec` must be single-quoted. Got:\n{content}"
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
fn shim_routes_to_airis_exec_inside_workspace() {
    // When run inside an airis workspace the shim must exec `airis exec <cmd>`.
    // We verify this by placing a mock `airis` script on PATH that records its
    // arguments to a file, then asserting the recorded args match expectations.
    let bin_dir = tempfile::tempdir().unwrap();
    install_global_guard(bin_dir.path(), "pnpm", GuardLevel::Enforce).unwrap();

    let tmp_home = tempfile::tempdir().unwrap();
    let work_dir = tmp_home.path().join("work");
    fs::create_dir_all(&work_dir).unwrap();
    // Workspace marker — .airis/ directory is the airis-specific context indicator.
    fs::create_dir_all(work_dir.join(".airis")).unwrap();

    // Mock `airis` binary: writes its arguments to a file, then exits 0.
    let recorded = tmp_home.path().join("airis-args.txt");
    let mock_airis = tmp_home.path().join("airis");
    fs::write(
        &mock_airis,
        format!(
            "#!/usr/bin/env bash\necho \"$@\" > \"{}\"\nexit 0\n",
            recorded.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&mock_airis, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let shim = bin_dir.path().join("pnpm");
    let path = format!("{}:/usr/bin:/bin", tmp_home.path().display());
    let output = Command::new("bash")
        .arg(&shim)
        .args(["install", "--frozen-lockfile"])
        .current_dir(&work_dir)
        .env_clear()
        .env("HOME", tmp_home.path())
        .env("PATH", &path)
        .output()
        .expect("bash must be available");

    assert!(
        output.status.success(),
        "shim must exit 0 when mock airis succeeds.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let args = fs::read_to_string(&recorded).expect("mock airis must have written args file");
    assert_eq!(
        args.trim(),
        "exec pnpm install --frozen-lockfile",
        "shim must call `airis exec pnpm <args>`, got: {args}"
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

#[test]
fn generated_script_branches_on_guard_level() {
    // Regression: the `LEVEL` variable used to be dead code — every level
    // behaved like Enforce. The generated script must actually branch on it.
    let dir = tempfile::tempdir().unwrap();
    install_global_guard(dir.path(), "pnpm", GuardLevel::Warn).unwrap();

    let content = fs::read_to_string(dir.path().join("pnpm")).unwrap();
    assert!(
        content.contains("LEVEL=\"warn\""),
        "guard level must be embedded in the script. Got:\n{content}"
    );
    assert!(
        content.contains("if [[ \"$LEVEL\" != \"enforce\" ]]"),
        "script must branch on $LEVEL so warn/off do not route to Docker. Got:\n{content}"
    );
}

#[cfg(unix)]
#[test]
fn warn_level_passes_through_inside_workspace() {
    // Regression: a `warn`-level guard must notify but proceed with the host
    // command — it must NOT route to `airis exec`. We prove this with a mock
    // `airis` that always fails: if the guard wrongly routed to it the shim
    // would exit non-zero. With warn it execs the real `true` and exits 0.
    let bin_dir = tempfile::tempdir().unwrap();
    install_global_guard(bin_dir.path(), "true", GuardLevel::Warn).unwrap();

    let tmp_home = tempfile::tempdir().unwrap();
    let work_dir = tmp_home.path().join("work");
    fs::create_dir_all(&work_dir).unwrap();
    // Plant .airis/ so the guard would route to Docker under Enforce.
    fs::create_dir_all(work_dir.join(".airis")).unwrap();

    // Mock `airis` that always fails — must not be invoked under warn.
    let mock_airis = tmp_home.path().join("airis");
    fs::write(&mock_airis, "#!/usr/bin/env bash\nexit 1\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&mock_airis, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let script = bin_dir.path().join("true");
    let path = format!("{}:/usr/bin:/bin", tmp_home.path().display());
    let output = Command::new("bash")
        .arg(&script)
        .current_dir(&work_dir)
        .env_clear()
        .env("HOME", tmp_home.path())
        .env("PATH", &path)
        .output()
        .expect("bash must be available");

    assert_eq!(
        output.status.code(),
        Some(0),
        "warn level must exec the real command (true), not route to airis.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("guard level: warn"),
        "warn level must notify the user on stderr. Got stderr:\n{stderr}"
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
    // Plant .airis/ so the guard would normally route to Docker.
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".airis")).unwrap();

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
