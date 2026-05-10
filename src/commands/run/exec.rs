//! `airis exec` — run a command inside a workspace service container.
//!
//! Phase 1b features:
//! - `airis exec pnpm i` — service is auto-resolved from the cmd's runtime family.
//! - `airis exec --service web cargo build` — explicit service override.
//! - `airis exec workspace pnpm i` — backward-compat: a positional first
//!   argument that matches a known service name still works as a service hint.
//! - Container auto-up when the resolved service is stopped, suppressed by
//!   `AIRIS_NO_AUTO_UP=1` or a recent `airis down` marker.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};
use colored::Colorize;

use crate::manifest::Manifest;

/// Suppression window after `airis down` during which auto-up is skipped,
/// so back-to-back `airis down && airis exec ls` does not silently relaunch
/// the stack the user just intentionally tore down.
const DOWN_MARKER_TTL: Duration = Duration::from_secs(30);

/// Default service to fall back to when the manifest does not name one
/// for the resolved runtime family.
const DEFAULT_SERVICE: &str = "workspace";

/// Runtime family for a command-line invocation. Drives cmd→service routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFamily {
    Node,
    Python,
    Rust,
}

/// Map a leading argv token to its runtime family.
///
/// Returns None for commands not owned by a known runtime — caller decides
/// whether to error out or treat the token as a service name.
pub fn classify_cmd(head: &str) -> Option<RuntimeFamily> {
    // Strip an optional path prefix so `/usr/bin/python3` and `python3` both classify.
    let bare = Path::new(head)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(head);

    match bare {
        "node" | "npm" | "pnpm" | "yarn" | "bun" | "npx" | "tsx" | "tsc" | "next" | "vite"
        | "tsup" | "deno" => Some(RuntimeFamily::Node),
        "python" | "python3" | "pip" | "pip3" | "uv" | "poetry" | "ruff" | "mypy" | "pytest"
        | "ipython" => Some(RuntimeFamily::Python),
        "cargo" | "rustc" | "rustup" | "clippy-driver" | "rustfmt" => Some(RuntimeFamily::Rust),
        _ => None,
    }
}

/// Entry point for `airis exec`.
///
/// `service`: explicit `--service` value (None means auto-resolve).
/// `cmd`: the command and its args; if `service` is None and `cmd[0]` matches a
///        known service name in compose, that is interpreted as a positional
///        service hint to preserve `airis exec workspace ls` behaviour.
/// `auto_up`: run `airis up` first if the resolved service is not running.
pub fn run_exec(service: Option<&str>, cmd: &[String], auto_up: bool) -> Result<()> {
    if cmd.is_empty() {
        bail!("airis exec: missing command. Usage: airis exec [--service NAME] CMD [ARGS...]");
    }

    let manifest = Manifest::load_loose(Path::new(crate::manifest::MANIFEST_FILE)).ok();

    let (resolved_service, effective_cmd) = resolve(service, cmd, manifest.as_ref())?;

    let auto_up_disabled =
        std::env::var_os("AIRIS_NO_AUTO_UP").is_some() || down_marker_recent(manifest.as_ref());

    if auto_up && !auto_up_disabled && !is_service_running(&resolved_service) {
        eprintln!(
            "{}  Container '{}' not running, starting via 'airis up'…",
            "ℹ️ ".cyan(),
            resolved_service
        );
        super::run("up", &[])?;
    }

    docker_compose_exec(&resolved_service, &effective_cmd)
}

/// Decide which service to target and which arg slice is the actual command.
///
/// Resolution order:
/// 1. `--service` flag → explicit, pass cmd through unchanged.
/// 2. cmd[0] matches a manifest-defined service name → positional service
///    (legacy form, drop cmd[0] from the command).
/// 3. cmd[0] classifies into a runtime family → look up the service for that
///    family in the manifest; default to `workspace`.
/// 4. Otherwise → error with a hint.
fn resolve<'a>(
    service: Option<&'a str>,
    cmd: &'a [String],
    manifest: Option<&Manifest>,
) -> Result<(String, Vec<String>)> {
    if let Some(svc) = service {
        return Ok((svc.to_string(), cmd.to_vec()));
    }

    let head = &cmd[0];
    let known_services = collect_service_names(manifest);

    // Legacy positional service form: `airis exec workspace pnpm install`.
    // Only triggers when the head is *not* a runtime command — otherwise a user
    // running a tool whose name happens to match a service would lose their first arg.
    if classify_cmd(head).is_none() && known_services.iter().any(|s| s == head) && cmd.len() >= 2 {
        let rest = cmd[1..].to_vec();
        return Ok((head.clone(), rest));
    }

    if let Some(family) = classify_cmd(head) {
        let svc = service_for_family(family, manifest);
        return Ok((svc, cmd.to_vec()));
    }

    bail!(
        "airis exec: cannot resolve service for '{}'.\n\
         Pass --service NAME explicitly, or use a known runtime command \
         (node/pnpm/python/cargo/...).",
        head
    );
}

/// Service name to use for a runtime family.
///
/// Phase 1b ships a flat default: every family routes to `workspace`. Phase 1.5
/// (or a follow-up that splits the workspace image per runtime) can derive this
/// from manifest `[runtimes.<family>].service` once that knob exists.
fn service_for_family(_family: RuntimeFamily, _manifest: Option<&Manifest>) -> String {
    DEFAULT_SERVICE.to_string()
}

/// All service names declared in the manifest (services + apps), plus the
/// implicit `workspace` default which is always recognized for the legacy
/// `airis exec workspace <cmd>` form even when no manifest is present.
fn collect_service_names(manifest: Option<&Manifest>) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    if let Some(m) = manifest {
        for k in m.service.keys() {
            names.push(k.clone());
        }
        for k in m.apps.keys() {
            names.push(k.clone());
        }
    }
    if !names.iter().any(|s| s == DEFAULT_SERVICE) {
        names.push(DEFAULT_SERVICE.to_string());
    }
    names
}

/// `docker compose ps --services --filter status=running` and check membership.
fn is_service_running(service: &str) -> bool {
    let Ok(output) = Command::new("docker")
        .args(["compose", "ps", "--services", "--filter", "status=running"])
        .output()
    else {
        // No docker, no compose, no signal — assume not running and let the
        // subsequent compose exec / up surface the real error to the user.
        return false;
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|l| l.trim() == service)
}

fn docker_compose_exec(service: &str, cmd: &[String]) -> Result<()> {
    let mut args: Vec<&str> = vec!["compose", "exec", service];
    for c in cmd {
        args.push(c);
    }
    let mut child = Command::new("docker")
        .args(&args)
        .spawn()
        .with_context(|| "Failed to run docker compose exec")?;
    let status = child.wait()?;
    if !status.success() {
        bail!(
            "docker compose exec {} {} exited with {:?}",
            service,
            cmd.join(" "),
            status.code()
        );
    }
    Ok(())
}

// ============================================================================
// Down-marker — written by `airis down`, read here to suppress auto-up.
// ============================================================================

/// Path to the down-marker file for this manifest's project, if a project_id
/// can be derived. Returns None when there is no manifest or no home dir.
fn down_marker_path(manifest: Option<&Manifest>) -> Option<PathBuf> {
    let project_id = manifest
        .map(|m| m.project.id.clone())
        .filter(|id| !id.is_empty())?;
    let home = dirs::home_dir()?;
    Some(
        home.join(".airis")
            .join("state")
            .join(project_id)
            .join("down-marker"),
    )
}

fn down_marker_recent(manifest: Option<&Manifest>) -> bool {
    let Some(path) = down_marker_path(manifest) else {
        return false;
    };
    let Ok(meta) = std::fs::metadata(&path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let Ok(elapsed) = SystemTime::now().duration_since(modified) else {
        return false;
    };
    elapsed < DOWN_MARKER_TTL
}

/// Write the down-marker. Called by `airis down`. Best-effort: failures are
/// silently ignored — auto-up suppression is a convenience, not a guarantee.
pub fn write_down_marker(manifest: Option<&Manifest>) {
    let Some(path) = down_marker_path(manifest) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, b"");
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{AppConfig, Manifest, MetaSection, ServiceConfig};

    fn manifest_with(project_id: &str, services: &[&str], apps: &[&str]) -> Manifest {
        let mut m = Manifest::default_with_project(project_id);
        m.service.clear();
        for s in services {
            m.service.insert((*s).to_string(), ServiceConfig::default());
        }
        m.apps.clear();
        for a in apps {
            m.apps.insert((*a).to_string(), AppConfig::default());
        }
        m
    }

    #[test]
    fn classify_routes_node_python_rust() {
        assert_eq!(classify_cmd("pnpm"), Some(RuntimeFamily::Node));
        assert_eq!(classify_cmd("npx"), Some(RuntimeFamily::Node));
        assert_eq!(classify_cmd("python3"), Some(RuntimeFamily::Python));
        assert_eq!(classify_cmd("uv"), Some(RuntimeFamily::Python));
        assert_eq!(classify_cmd("cargo"), Some(RuntimeFamily::Rust));
        assert_eq!(classify_cmd("rustfmt"), Some(RuntimeFamily::Rust));
        assert_eq!(classify_cmd("ls"), None);
    }

    #[test]
    fn classify_strips_path_prefix() {
        assert_eq!(
            classify_cmd("/usr/bin/python3"),
            Some(RuntimeFamily::Python)
        );
        assert_eq!(
            classify_cmd("/opt/homebrew/bin/pnpm"),
            Some(RuntimeFamily::Node)
        );
    }

    #[test]
    fn resolve_explicit_service_passes_cmd_through() {
        let cmd = vec!["ls".to_string(), "-la".to_string()];
        let (svc, out) = resolve(Some("api"), &cmd, None).unwrap();
        assert_eq!(svc, "api");
        assert_eq!(out, cmd);
    }

    #[test]
    fn resolve_runtime_command_routes_to_default() {
        let cmd = vec!["pnpm".to_string(), "install".to_string()];
        let (svc, out) = resolve(None, &cmd, None).unwrap();
        assert_eq!(svc, DEFAULT_SERVICE);
        assert_eq!(out, cmd);
    }

    #[test]
    fn resolve_legacy_positional_service_strips_first_arg() {
        let m = manifest_with("test", &["api"], &[]);
        let cmd = vec!["api".to_string(), "ls".to_string()];
        let (svc, out) = resolve(None, &cmd, Some(&m)).unwrap();
        assert_eq!(svc, "api");
        assert_eq!(out, vec!["ls".to_string()]);
    }

    #[test]
    fn resolve_workspace_positional_form_still_works() {
        let cmd = vec!["workspace".to_string(), "pnpm".to_string(), "i".to_string()];
        // "workspace" is in the implicit known-services list even without a manifest.
        let (svc, out) = resolve(None, &cmd, None).unwrap();
        assert_eq!(svc, DEFAULT_SERVICE);
        assert_eq!(out, vec!["pnpm".to_string(), "i".to_string()]);
    }

    #[test]
    fn resolve_runtime_command_does_not_get_swallowed_by_service_lookup() {
        // "pnpm" must classify as Node and route, even if the user has a
        // service literally named "pnpm" in their manifest.
        let mut m = Manifest::default_with_project("test");
        m.service
            .insert("pnpm".to_string(), ServiceConfig::default());
        let cmd = vec!["pnpm".to_string(), "i".to_string()];
        let (svc, out) = resolve(None, &cmd, Some(&m)).unwrap();
        assert_eq!(svc, DEFAULT_SERVICE);
        assert_eq!(out, cmd);
    }

    #[test]
    fn resolve_unknown_command_errors_with_hint() {
        let cmd = vec!["totallymadeup".to_string()];
        let err = resolve(None, &cmd, None).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("totallymadeup"), "got: {msg}");
        assert!(msg.contains("--service"), "got: {msg}");
    }

    #[test]
    fn collect_service_names_includes_workspace_default() {
        let names = collect_service_names(None);
        assert!(names.contains(&DEFAULT_SERVICE.to_string()));
    }

    #[test]
    fn down_marker_path_requires_project_id_and_home() {
        // No manifest → no path.
        assert!(down_marker_path(None).is_none());

        // Manifest with empty project.id → no path.
        let mut m = Manifest::default_with_project("");
        m.project = MetaSection::default();
        assert!(down_marker_path(Some(&m)).is_none());

        // Manifest with project id → path under HOME/.airis/state/<id>/down-marker.
        let m = Manifest::default_with_project("demo");
        let p = down_marker_path(Some(&m)).expect("path expected when home + project id present");
        assert!(p.ends_with("down-marker"));
        assert!(p.to_string_lossy().contains("demo"));
        assert!(p.to_string_lossy().contains(".airis"));
    }

    #[test]
    fn down_marker_recent_respects_ttl() {
        let m = Manifest::default_with_project("ttltest");

        // Drop any pre-existing marker so this test is deterministic regardless
        // of whether someone ran `airis down` on this host.
        if let Some(p) = down_marker_path(Some(&m)) {
            let _ = std::fs::remove_file(&p);
        }
        assert!(!down_marker_recent(Some(&m)));

        write_down_marker(Some(&m));
        assert!(down_marker_recent(Some(&m)));

        // Cleanup so we don't pollute the user's home dir.
        if let Some(p) = down_marker_path(Some(&m)) {
            let _ = std::fs::remove_file(&p);
        }
    }
}
