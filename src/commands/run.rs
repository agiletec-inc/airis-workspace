use anyhow::{bail, Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use std::path::Path;
use std::process::Command;

use crate::manifest::Manifest;

/// Default commands when manifest.toml [commands] is empty
fn default_commands() -> IndexMap<String, String> {
    let mut cmds = IndexMap::new();
    cmds.insert("up".to_string(), "docker compose up -d".to_string());
    cmds.insert("down".to_string(), "docker compose down --remove-orphans".to_string());
    cmds.insert("shell".to_string(), "docker compose exec -it workspace sh".to_string());
    cmds.insert("install".to_string(), "docker compose exec workspace pnpm install".to_string());
    cmds.insert("dev".to_string(), "docker compose exec workspace pnpm dev".to_string());
    cmds.insert("build".to_string(), "docker compose exec workspace pnpm build".to_string());
    cmds.insert("test".to_string(), "docker compose exec workspace pnpm test".to_string());
    cmds.insert("lint".to_string(), "docker compose exec workspace pnpm lint".to_string());
    cmds.insert("clean".to_string(), "rm -rf ./node_modules ./dist ./.next ./build ./target".to_string());
    cmds.insert("logs".to_string(), "docker compose logs -f".to_string());
    cmds.insert("ps".to_string(), "docker compose ps".to_string());
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
        default_commands()
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
