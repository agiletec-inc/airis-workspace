use anyhow::Result;
use colored::Colorize;
use indexmap::IndexMap;
use serde::Serialize;
use std::path::Path;

use crate::manifest::{MANIFEST_FILE, Manifest};

pub enum ManifestAction {
    DevApps,
    Rule { name: String },
    Json,
}

/// Workspace truth output for LLM consumption
/// This is the single source of truth for workspace configuration
#[derive(Debug, Serialize)]
pub struct WorkspaceTruth {
    /// Format identifier for consumers
    pub format: &'static str,
    /// Schema version for backwards compatibility
    pub schema_version: u32,
    /// Absolute path to workspace root (where manifest.toml lives)
    pub workspace_root: String,
    /// List of compose files used by this workspace
    pub compose_files: Vec<String>,
    /// Full docker compose command with -f flags
    pub compose_command: String,
    /// Primary service name for exec/run
    pub service: String,
    /// Working directory inside container
    pub workdir: String,
    /// Package manager (pnpm, npm, yarn, bun)
    pub package_manager: String,
    /// Project name from manifest
    pub project_name: String,
    /// CWD policy: "repo_root_required" means commands must run from workspace root
    pub cwd_policy: &'static str,
    /// Recommended commands for common operations
    pub recommended_commands: IndexMap<String, String>,
}

impl WorkspaceTruth {
    /// Build WorkspaceTruth from manifest
    pub fn from_manifest(manifest: &Manifest) -> Result<Self> {
        let workspace_root = std::env::current_dir()?
            .to_string_lossy()
            .to_string();

        // Collect compose files
        let mut compose_files = Vec::new();

        // Add root docker-compose.yml if it exists
        if Path::new("docker-compose.yml").exists() {
            compose_files.push("docker-compose.yml".to_string());
        }

        // Add from orchestration config if present
        if let Some(dev) = &manifest.orchestration.dev {
            if let Some(workspace) = &dev.workspace
                && !compose_files.contains(workspace) {
                    compose_files.push(workspace.clone());
                }
            if let Some(supabase) = &dev.supabase {
                for f in supabase {
                    if !compose_files.contains(f) {
                        compose_files.push(f.clone());
                    }
                }
            }
            if let Some(traefik) = &dev.traefik
                && !compose_files.contains(traefik) {
                    compose_files.push(traefik.clone());
                }
        }

        // Add from dev section
        if let Some(supabase) = &manifest.dev.supabase {
            for f in supabase {
                if !compose_files.contains(f) {
                    compose_files.push(f.clone());
                }
            }
        }
        if let Some(traefik) = &manifest.dev.traefik
            && !compose_files.contains(traefik) {
                compose_files.push(traefik.clone());
            }

        // If still empty, use docker.compose from manifest
        if compose_files.is_empty() && !manifest.docker.compose.is_empty() {
            compose_files.push(manifest.docker.compose.clone());
        }

        // Build compose command
        let compose_command = if compose_files.is_empty() {
            "docker compose".to_string()
        } else {
            let file_args: Vec<String> = compose_files.iter()
                .map(|f| format!("-f {}", f))
                .collect();
            format!("docker compose {}", file_args.join(" "))
        };

        // Extract package manager base name
        let pm_full = &manifest.workspace.package_manager;
        let package_manager = if pm_full.contains('@') {
            pm_full.split('@').next().unwrap_or("pnpm").to_string()
        } else {
            pm_full.clone()
        };

        // Build recommended commands
        let mut recommended_commands = IndexMap::new();
        recommended_commands.insert("up".to_string(), "airis up".to_string());
        recommended_commands.insert("down".to_string(), "airis down".to_string());
        recommended_commands.insert("install".to_string(), "airis install".to_string());
        recommended_commands.insert("dev".to_string(), "airis dev".to_string());
        recommended_commands.insert("shell".to_string(), "airis shell".to_string());
        recommended_commands.insert("build".to_string(), "airis build".to_string());
        recommended_commands.insert("test".to_string(), "airis test".to_string());
        recommended_commands.insert("lint".to_string(), "airis lint".to_string());

        Ok(WorkspaceTruth {
            format: "airis.manifest.v1",
            schema_version: 1,
            workspace_root,
            compose_files,
            compose_command,
            service: manifest.workspace.service.clone(),
            workdir: manifest.workspace.workdir.clone(),
            package_manager,
            project_name: manifest.workspace.name.clone(),
            cwd_policy: "repo_root_required",
            recommended_commands,
        })
    }

    /// Output as JSON string with stable key ordering
    pub fn to_json(&self) -> Result<String> {
        // Use serde_json with pretty printing
        // IndexMap preserves insertion order, ensuring stable output
        Ok(serde_json::to_string_pretty(self)?)
    }
}

pub fn run(action: ManifestAction) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    if !manifest_path.exists() {
        anyhow::bail!("❌ manifest.toml not found. Run `airis init` to create it first.");
    }

    let manifest = Manifest::load(manifest_path)?;

    match action {
        ManifestAction::DevApps => {
            // Print apps_pattern (glob pattern for auto-discovery)
            println!("{}", manifest.dev.apps_pattern);
        }
        ManifestAction::Rule { name } => {
            let Some(rule) = manifest.rule.get(&name) else {
                anyhow::bail!(
                    "{} `{}` is not defined inside [rule] section of manifest.toml",
                    "❌ Rule".bright_red(),
                    name
                );
            };

            if rule.commands.is_empty() {
                println!(
                    "{} Rule `{}` has no commands configured in manifest.toml",
                    "⚠️".yellow(),
                    name
                );
                return Ok(());
            }

            for command in &rule.commands {
                println!("{command}");
            }
        }
        ManifestAction::Json => {
            let truth = WorkspaceTruth::from_manifest(&manifest)?;
            println!("{}", truth.to_json()?);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to run tests with serialization since set_current_dir is not thread-safe
    use std::sync::Mutex;
    static DIR_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_workspace_truth_serialization() {
        let _guard = DIR_LOCK.lock().unwrap();
        let manifest_content = r#"
version = 1

[workspace]
name = "test-workspace"
service = "workspace"
workdir = "/app"
package_manager = "pnpm@10.22.0"

[docker]
compose = "docker-compose.yml"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();

        // Create temp dir and change to it
        let dir = tempfile::tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        // Create docker-compose.yml
        std::fs::write("docker-compose.yml", "version: '3'").unwrap();

        let result = std::panic::catch_unwind(|| {
            let truth = WorkspaceTruth::from_manifest(&manifest).unwrap();
            assert_eq!(truth.format, "airis.manifest.v1");
            assert_eq!(truth.schema_version, 1);
            assert_eq!(truth.service, "workspace");
            assert_eq!(truth.workdir, "/app");
            assert_eq!(truth.package_manager, "pnpm");
            assert_eq!(truth.project_name, "test-workspace");
            assert_eq!(truth.cwd_policy, "repo_root_required");

            // Verify JSON serialization
            let json = truth.to_json().unwrap();
            assert!(json.contains("\"format\": \"airis.manifest.v1\""));
            assert!(json.contains("\"schema_version\": 1"));
        });

        // Always restore directory
        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_workspace_truth_recommended_commands() {
        let _guard = DIR_LOCK.lock().unwrap();
        let manifest_content = r#"
version = 1

[workspace]
name = "test"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = std::panic::catch_unwind(|| {
            let truth = WorkspaceTruth::from_manifest(&manifest).unwrap();
            assert_eq!(truth.recommended_commands.get("up"), Some(&"airis up".to_string()));
            assert_eq!(truth.recommended_commands.get("dev"), Some(&"airis dev".to_string()));
            assert_eq!(truth.recommended_commands.get("shell"), Some(&"airis shell".to_string()));
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_workspace_truth_compose_files() {
        let _guard = DIR_LOCK.lock().unwrap();
        let manifest_content = r#"
version = 1

[workspace]
name = "test"

[orchestration.dev]
workspace = "docker-compose.yml"
supabase = ["supabase/docker-compose.yml"]
traefik = "traefik/docker-compose.yml"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        // Create the compose files
        std::fs::write("docker-compose.yml", "version: '3'").unwrap();

        let result = std::panic::catch_unwind(|| {
            let truth = WorkspaceTruth::from_manifest(&manifest).unwrap();

            // Should have all compose files
            assert!(truth.compose_files.contains(&"docker-compose.yml".to_string()));
            assert!(truth.compose_files.contains(&"supabase/docker-compose.yml".to_string()));
            assert!(truth.compose_files.contains(&"traefik/docker-compose.yml".to_string()));

            // Compose command should include all -f flags
            assert!(truth.compose_command.contains("-f docker-compose.yml"));
            assert!(truth.compose_command.contains("-f supabase/docker-compose.yml"));
            assert!(truth.compose_command.contains("-f traefik/docker-compose.yml"));
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }
}
