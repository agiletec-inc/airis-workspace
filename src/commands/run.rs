use anyhow::{bail, Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use std::path::Path;
use std::process::Command;

use crate::manifest::Manifest;

/// Build docker compose command with orchestration files
fn build_compose_command(manifest: &Manifest, base_cmd: &str) -> String {
    // Check if orchestration.dev is configured
    if let Some(dev) = &manifest.orchestration.dev {
        let mut compose_files = Vec::new();

        // Add workspace compose file
        if let Some(workspace) = &dev.workspace {
            compose_files.push(format!("-f {}", workspace));
        }

        // Add supabase compose files
        if let Some(supabase) = &dev.supabase {
            for file in supabase {
                compose_files.push(format!("-f {}", file));
            }
        }

        // Add traefik compose file
        if let Some(traefik) = &dev.traefik {
            compose_files.push(format!("-f {}", traefik));
        }

        if !compose_files.is_empty() {
            return format!("docker compose {} {}", compose_files.join(" "), base_cmd);
        }
    }

    // Fall back to default (workspace/docker-compose.yml if exists)
    let workspace_compose = Path::new("workspace/docker-compose.yml");
    if workspace_compose.exists() {
        format!("docker compose -f workspace/docker-compose.yml {}", base_cmd)
    } else {
        format!("docker compose {}", base_cmd)
    }
}

/// Build clean command from manifest.toml [workspace.clean] section
fn build_clean_command(manifest: &Manifest) -> String {
    let clean = &manifest.workspace.clean;
    let mut parts = Vec::new();

    // Recursive patterns (e.g., node_modules)
    for pattern in &clean.recursive {
        parts.push(format!(
            "find . -name '{}' -type d -prune -exec rm -rf {{}} + 2>/dev/null",
            pattern
        ));
    }

    // Root directories
    if !clean.dirs.is_empty() {
        let dirs = clean.dirs.iter()
            .map(|d| format!("./{}", d))
            .collect::<Vec<_>>()
            .join(" ");
        parts.push(format!("rm -rf {}", dirs));
    }

    // Always clean .DS_Store
    parts.push("find . -name '.DS_Store' -delete 2>/dev/null || true".to_string());

    // Success message
    parts.push("echo '✅ Cleaned all build artifacts'".to_string());

    parts.join("; ")
}

/// Default commands when manifest.toml [commands] is empty
fn default_commands(manifest: &Manifest) -> IndexMap<String, String> {
    let mut cmds = IndexMap::new();
    cmds.insert("up".to_string(), build_compose_command(manifest, "up -d"));
    cmds.insert("down".to_string(), build_compose_command(manifest, "down --remove-orphans"));
    cmds.insert("shell".to_string(), build_compose_command(manifest, "exec -it workspace sh"));
    cmds.insert("install".to_string(), build_compose_command(manifest, "exec workspace pnpm install"));
    cmds.insert("dev".to_string(), build_compose_command(manifest, "exec workspace pnpm dev"));
    cmds.insert("build".to_string(), build_compose_command(manifest, "exec workspace pnpm build"));
    cmds.insert("test".to_string(), build_compose_command(manifest, "exec workspace pnpm test"));
    cmds.insert("lint".to_string(), build_compose_command(manifest, "exec workspace pnpm lint"));
    cmds.insert("clean".to_string(), build_clean_command(manifest));
    cmds.insert("logs".to_string(), build_compose_command(manifest, "logs -f"));
    cmds.insert("ps".to_string(), build_compose_command(manifest, "ps"));
    cmds
}

/// Execute a command defined in manifest.toml [commands] section
pub fn run(task: &str) -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "❌ manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

    // Use manifest commands or fall back to defaults
    let commands = if manifest.commands.is_empty() {
        default_commands(&manifest)
    } else {
        manifest.commands.clone()
    };

    // Check if command exists
    let cmd = commands
        .get(task)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "❌ Command '{}' not found in manifest.toml [commands] section.\n\n\
                 Available commands:\n{}",
                task.bold(),
                commands
                    .keys()
                    .map(|k| format!("  - {}", k))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })?;

    println!("🚀 Running: {}", cmd.cyan());

    // Execute command
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", cmd])
            .status()
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .status()
    }
    .with_context(|| format!("Failed to execute: {}", cmd))?;

    if !status.success() {
        bail!("Command failed with exit code: {:?}", status.code());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_run_missing_manifest() {
        let dir = tempdir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = run("test");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("manifest.toml not found"));
    }

    #[test]
    fn test_run_missing_command() {
        let dir = tempdir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        // Create minimal manifest
        let manifest_content = r#"
version = 1

[workspace]
name = "test"

[commands]
test = "echo 'test'"
"#;
        fs::write(dir.path().join("manifest.toml"), manifest_content).unwrap();

        let result = run("nonexistent");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("nonexistent") && err_msg.contains("not found"),
            "Expected error about 'nonexistent' not found, got: {}",
            err_msg
        );
    }
}
