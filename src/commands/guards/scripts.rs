use std::fs;
use std::io::BufRead;
use std::path::Path;

use anyhow::{Result, bail};

use super::GLOBAL_GUARD_MARKER;
use crate::manifest::GuardLevel;

/// Validate that a command name is safe for use in guard scripts.
fn validate_guard_cmd(cmd: &str) -> Result<()> {
    if cmd.is_empty() {
        bail!("Guard command name cannot be empty");
    }
    if !cmd
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
    {
        bail!(
            "Guard command name contains invalid characters: \"{}\". Only [a-zA-Z0-9._+-] allowed.",
            cmd
        );
    }
    Ok(())
}

/// Ensure a guard script path is safe to write to.
pub(super) fn ensure_safe_guard_path(path: &Path) -> Result<()> {
    if path.is_symlink() {
        fs::remove_file(path)?;
    } else if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_file() {
            bail!("Guard path {} is not a regular file.", path.display());
        }
    }
    Ok(())
}

/// Install a global guard script with specific level
pub(super) fn install_global_guard(bin_dir: &Path, cmd: &str, level: GuardLevel) -> Result<()> {
    validate_guard_cmd(cmd)?;
    let script_path = bin_dir.join(cmd);
    ensure_safe_guard_path(&script_path)?;

    let level_str = match level {
        GuardLevel::Off => "off",
        GuardLevel::Warn => "warn",
        GuardLevel::Enforce => "enforce",
    };

    let content = format!(
        r#"#!/usr/bin/env bash
# {GLOBAL_GUARD_MARKER}
# DO NOT EDIT - managed by airis global guards

LEVEL="{level}"

# Helper: find the real command on the host (excluding airis bin)
find_real_cmd() {{
    PATH=$(echo "$PATH" | tr ':' '\n' | grep -v '\.airis/bin' | tr '\n' ':' | sed 's/:$//') which "$1" 2>/dev/null
}}

# Helper: find airis context (has manifest.toml or .airis directory)
find_airis_context() {{
    local dir="$PWD"
    while [[ "$dir" != "/" ]]; do
        if [[ -f "$dir/manifest.toml" ]] || [[ -d "$dir/.airis" ]]; then
            echo "$dir"
            return 0
        fi
        dir="$(dirname "$dir")"
    done
    return 1
}}

# 1. Bypass check: Skip guard if AIRIS_SKIP_GUARD, AIRIS_HOST, or AIRIS_BYPASS is set
# Also skip if the first argument is 'bypass' or 'host'
if [[ "${{AIRIS_SKIP_GUARD:-}}" == "1" ]] || [[ "${{AIRIS_HOST:-}}" == "1" ]] || [[ "${{AIRIS_BYPASS:-}}" == "1" ]] || [[ "${{AIRIS_BYPASS:-}}" == "true" ]]; then
    REAL_CMD=$(find_real_cmd "{cmd}")
    if [[ -n "$REAL_CMD" ]]; then exec "$REAL_CMD" "$@"; else exit 127; fi
fi

if [[ "$1" == "bypass" ]] || [[ "$1" == "host" ]]; then
    shift
    REAL_CMD=$(find_real_cmd "{cmd}")
    if [[ -n "$REAL_CMD" ]]; then exec "$REAL_CMD" "$@"; else exit 127; fi
fi

REAL_CMD=$(find_real_cmd "{cmd}")

# 2. Inside Docker/CI: always allow host command
if [[ -f /.dockerenv ]] || grep -qsE 'docker|containerd' /proc/1/cgroup 2>/dev/null || [[ "${{DOCKER_CONTAINER:-}}" == "true" ]] || [[ "${{CI:-}}" == "true" ]]; then
    if [[ -n "$REAL_CMD" ]]; then exec "$REAL_CMD" "$@"; else exit 127; fi
fi

# 3. Airis Context detection & Smart Proxy
if find_airis_context >/dev/null; then
    # We are in an airis project. `airis exec <cmd>` auto-routes by command
    # name: pnpm/npm/node→workspace, python/uv→workspace, cargo→workspace.
    if command -v airis &>/dev/null; then
        exec airis exec {cmd} "$@"
    else
        echo "⚠️  Airis context detected but 'airis' command not found." >&2
    fi
fi

# 4. Not in airis workspace: Gentle allow
# Outside an airis project, we stay out of the way to ensure host/docker coexistence.
if [[ -n "$REAL_CMD" ]]; then exec "$REAL_CMD" "$@"; else exit 127; fi
"#,
        GLOBAL_GUARD_MARKER = GLOBAL_GUARD_MARKER,
        level = level_str,
        cmd = cmd
    );

    fs::write(&script_path, content)?;
    make_executable(&script_path)?;
    Ok(())
}

pub(super) fn make_executable(_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(_path)?.permissions();
        perms.set_mode(0o755); // rwxr-xr-x
        fs::set_permissions(_path, perms)?;
    }
    Ok(())
}

pub(super) fn is_global_guard(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let file = fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    for line in reader.lines().take(5).flatten() {
        if line.contains(GLOBAL_GUARD_MARKER) {
            return Ok(true);
        }
    }
    Ok(false)
}
