use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::config::{Mode, WorkspaceApp, WorkspaceConfig, Workspaces};

pub const MANIFEST_FILE: &str = "manifest.toml";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Manifest {
    #[serde(default)]
    pub workspace: WorkspaceSection,
    #[serde(default)]
    pub dev: DevSection,
    #[serde(default)]
    pub apps: IndexMap<String, AppConfig>,
    #[serde(default)]
    pub service: IndexMap<String, ServiceConfig>,
    #[serde(default)]
    pub rule: IndexMap<String, RuleConfig>,
    #[serde(default)]
    pub packages: PackagesSection,
    #[serde(default)]
    pub guards: GuardsSection,
}

impl Manifest {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {:?}", path.as_ref()))?;

        let manifest: Manifest =
            toml::from_str(&content).with_context(|| "Failed to parse manifest.toml")?;

        Ok(manifest)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize manifest.toml contents")?;

        fs::write(path.as_ref(), content)
            .with_context(|| format!("Failed to write {:?}", path.as_ref()))?;

        Ok(())
    }

    pub fn default_with_project(name: &str) -> Self {
        let mut rule = IndexMap::new();
        rule.insert(
            "verify".to_string(),
            RuleConfig {
                commands: vec!["just lint".to_string(), "just test-all".to_string()],
            },
        );
        rule.insert(
            "ci".to_string(),
            RuleConfig {
                commands: vec![
                    "just lint".to_string(),
                    "just test-all".to_string(),
                    "just typecheck".to_string(),
                ],
            },
        );

        let packages = PackagesSection {
            workspaces: vec!["apps/*".to_string(), "packages/*".to_string()],
            root: PackageDefinition {
                dependencies: IndexMap::new(),
                dev_dependencies: {
                    let mut dev = IndexMap::new();
                    dev.insert("typescript".to_string(), "5.6.2".to_string());
                    dev.insert("eslint".to_string(), "9.3.0".to_string());
                    dev
                },
                scripts: IndexMap::new(),
            },
            app: vec![AppPackageDefinition {
                pattern: "apps/*".to_string(),
                dependencies: IndexMap::new(),
                dev_dependencies: IndexMap::new(),
                scripts: IndexMap::new(),
            }],
        };

        Manifest {
            workspace: WorkspaceSection {
                name: name.to_string(),
                package_manager: "pnpm@10.22.0".to_string(),
                service: "workspace".to_string(),
                image: "node:22-alpine".to_string(),
                workdir: "/app".to_string(),
                volumes: vec!["workspace-node-modules:/app/node_modules".to_string()],
            },
            dev: DevSection::default(),
            apps: IndexMap::new(),
            service: IndexMap::new(),
            rule,
            packages,
            guards: GuardsSection::default(),
        }
    }

    pub fn to_workspace_config(&self) -> WorkspaceConfig {
        let mut config = WorkspaceConfig::default();
        config.name = self.workspace.name.clone();
        config.mode = Mode::DockerFirst;

        let apps = self
            .dev
            .apps
            .iter()
            .map(|name| WorkspaceApp::Simple(name.clone()))
            .collect();

        config.workspaces = Workspaces { apps, libs: vec![] };
        config
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkspaceSection {
    #[serde(default = "default_workspace_name")]
    pub name: String,
    #[serde(default = "default_package_manager")]
    pub package_manager: String,
    #[serde(default = "default_workspace_service")]
    pub service: String,
    #[serde(default = "default_workspace_image")]
    pub image: String,
    #[serde(default = "default_workspace_workdir")]
    pub workdir: String,
    #[serde(default)]
    pub volumes: Vec<String>,
}

impl Default for WorkspaceSection {
    fn default() -> Self {
        WorkspaceSection {
            name: default_workspace_name(),
            package_manager: default_package_manager(),
            service: default_workspace_service(),
            image: default_workspace_image(),
            workdir: default_workspace_workdir(),
            volumes: vec!["workspace-node-modules:/app/node_modules".to_string()],
        }
    }
}

fn default_workspace_name() -> String {
    "airis-workspace".to_string()
}

fn default_package_manager() -> String {
    "pnpm@10.22.0".to_string()
}

fn default_workspace_service() -> String {
    "workspace".to_string()
}

fn default_workspace_image() -> String {
    "node:22-alpine".to_string()
}

fn default_workspace_workdir() -> String {
    "/app".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct DevSection {
    #[serde(default)]
    pub apps: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub app_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ServiceConfig {
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default)]
    pub env: IndexMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RuleConfig {
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PackagesSection {
    #[serde(default)]
    pub workspaces: Vec<String>,
    #[serde(default)]
    pub root: PackageDefinition,
    #[serde(rename = "app", default)]
    pub app: Vec<AppPackageDefinition>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PackageDefinition {
    #[serde(default)]
    pub dependencies: IndexMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    pub dev_dependencies: IndexMap<String, String>,
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AppPackageDefinition {
    pub pattern: String,
    #[serde(default)]
    pub dependencies: IndexMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    pub dev_dependencies: IndexMap<String, String>,
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct GuardsSection {
    /// Commands to completely deny (e.g., ["npm", "yarn"])
    #[serde(default)]
    pub deny: Vec<String>,

    /// Commands to wrap with Docker execution
    /// e.g., {"pnpm": "docker compose exec workspace pnpm"}
    #[serde(default)]
    pub wrap: IndexMap<String, String>,

    /// Commands to deny with custom messages
    #[serde(default)]
    pub deny_with_message: IndexMap<String, String>,
}
