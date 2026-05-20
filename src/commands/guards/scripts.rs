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

# Helper: find docker-first context by walking up the directory tree.
# Matches on .airis/ directory (airis-specific) or any compose file (Docker-based project).
find_airis_context() {{
    local dir="$PWD"
    while [[ "$dir" != "/" ]]; do
        if [[ -d "$dir/.airis" ]] \
            || [[ -f "$dir/compose.yaml" ]] \
            || [[ -f "$dir/compose.yml" ]] \
            || [[ -f "$dir/docker-compose.yaml" ]] \
            || [[ -f "$dir/docker-compose.yml" ]]; then
            echo "$dir"
            return 0
        fi
        dir="$(dirname "$dir")"
    done
    return 1
}}

REAL_CMD=$(find_real_cmd '{cmd}')

# 0. Explicit bypass: AIRIS_BYPASS=1 or `airis host <cmd>`
if [[ "${{AIRIS_BYPASS:-}}" == "1" ]]; then
    if [[ -n "$REAL_CMD" ]]; then exec "$REAL_CMD" "$@"; else exit 127; fi
fi

# 1. Inside Docker/CI: always allow host command
if [[ -f /.dockerenv ]] || grep -qsE 'docker|containerd' /proc/1/cgroup 2>/dev/null || [[ "${{DOCKER_CONTAINER:-}}" == "true" ]] || [[ "${{CI:-}}" == "true" ]]; then
    if [[ -n "$REAL_CMD" ]]; then exec "$REAL_CMD" "$@"; else exit 127; fi
fi

# 2. Airis Context detection & Smart Proxy
if find_airis_context >/dev/null; then
    # We are in an airis docker-first project. Route to Docker via airis exec.
    if command -v airis &>/dev/null; then
        # Non-interactive (no TTY on stdout): disable auto-up so that background
        # scripts such as statusline commands and hooks don't trigger a full
        # Docker stack launch when the container happens to be stopped.
        if [[ ! -t 1 ]]; then
            export AIRIS_NO_AUTO_UP=1
        fi
        exec airis exec '{cmd}' "$@"
    else
        echo "❌ AIRIS: Context detected but 'airis' command not found." >&2
        echo "   Cannot route to Docker. Host execution is blocked for hygiene." >&2
        exit 124
    fi
fi

# 3. Not in any airis workspace — pass through unconditionally.
#    Guards only enforce Docker-first hygiene *inside* a workspace.
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
