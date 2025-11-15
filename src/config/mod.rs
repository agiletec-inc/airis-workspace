use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkspaceConfig {
    pub version: u8,
    pub name: String,
    pub mode: Mode,
    #[serde(default)]
    pub catalog: IndexMap<String, String>,
    #[serde(default = "default_package_manager")]
    pub package_manager: String,
    #[serde(default)]
    pub workspaces: Workspaces,
    #[serde(default)]
    pub apps: IndexMap<String, AppConfig>,
    #[serde(default)]
    pub docker: DockerConfig,
    #[serde(default)]
    pub rules: Rules,
    #[serde(default)]
    pub just: JustConfig,
    #[serde(default)]
    pub types: IndexMap<String, TypeConfig>,
}

fn default_package_manager() -> String {
    "pnpm@10.22.0".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    DockerFirst,
    Hybrid,
    Strict,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::DockerFirst
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Workspaces {
    #[serde(default)]
    pub apps: Vec<WorkspaceApp>,
    #[serde(default)]
    pub libs: Vec<WorkspaceLib>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum WorkspaceApp {
    Simple(String),
    Detailed {
        name: String,
        #[serde(rename = "type")]
        app_type: String,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum WorkspaceLib {
    Simple(String),
    Detailed {
        name: String,
        #[serde(rename = "type")]
        lib_type: String,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    #[serde(rename = "type")]
    pub app_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<Runtime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    Docker,
    Local,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DockerConfig {
    #[serde(rename = "baseImage", skip_serializing_if = "Option::is_none")]
    pub base_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    #[serde(default)]
    pub workspace: WorkspaceService,
}

impl Default for DockerConfig {
    fn default() -> Self {
        DockerConfig {
            base_image: Some("node:22-alpine".to_string()),
            workdir: Some("/app".to_string()),
            workspace: WorkspaceService::default(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkspaceService {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
}

impl Default for WorkspaceService {
    fn default() -> Self {
        WorkspaceService {
            service: Some("workspace".to_string()),
            volumes: vec!["node_modules".to_string()],
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Rules {
    #[serde(rename = "no-host-pnpm", skip_serializing_if = "Option::is_none")]
    pub no_host_pnpm: Option<String>,
    #[serde(rename = "catalog-only", skip_serializing_if = "Option::is_none")]
    pub catalog_only: Option<String>,
    #[serde(rename = "no-env-local", skip_serializing_if = "Option::is_none")]
    pub no_env_local: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JustConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default)]
    pub features: Vec<String>,
}

impl Default for JustConfig {
    fn default() -> Self {
        JustConfig {
            output: Some("justfile".to_string()),
            features: vec![
                "docker-first-guard".to_string(),
                "type-specific-commands".to_string(),
            ],
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TypeConfig {
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
    #[serde(rename = "buildMode", skip_serializing_if = "Option::is_none")]
    pub build_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        WorkspaceConfig {
            version: 1,
            name: "my-monorepo".to_string(),
            mode: Mode::DockerFirst,
            catalog: IndexMap::new(),
            package_manager: default_package_manager(),
            workspaces: Workspaces::default(),
            apps: IndexMap::new(),
            docker: DockerConfig::default(),
            rules: Rules::default(),
            just: JustConfig::default(),
            types: IndexMap::new(),
        }
    }
}

impl WorkspaceConfig {
    #[allow(dead_code)]
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read {:?}", path.as_ref()))?;

        let config: WorkspaceConfig =
            serde_yaml::from_str(&content).with_context(|| "Failed to parse workspace.yaml")?;

        Ok(config)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let yaml =
            serde_yaml::to_string(self).with_context(|| "Failed to serialize workspace config")?;

        fs::write(path.as_ref(), yaml)
            .with_context(|| format!("Failed to write {:?}", path.as_ref()))?;

        Ok(())
    }

    /// Get workspace service name
    #[allow(dead_code)]
    pub fn workspace_service(&self) -> String {
        self.docker
            .workspace
            .service
            .clone()
            .unwrap_or_else(|| "workspace".to_string())
    }

    /// Get app name from WorkspaceApp
    #[allow(dead_code)]
    pub fn get_app_name(app: &WorkspaceApp) -> String {
        match app {
            WorkspaceApp::Simple(name) => name.clone(),
            WorkspaceApp::Detailed { name, .. } => name.clone(),
        }
    }

    /// Get app type from WorkspaceApp
    #[allow(dead_code)]
    pub fn get_app_type(&self, app: &WorkspaceApp) -> Option<String> {
        match app {
            WorkspaceApp::Simple(name) => self.apps.get(name).map(|c| c.app_type.clone()),
            WorkspaceApp::Detailed { app_type, .. } => Some(app_type.clone()),
        }
    }

    /// Get lib name from WorkspaceLib
    #[allow(dead_code)]
    pub fn get_lib_name(lib: &WorkspaceLib) -> String {
        match lib {
            WorkspaceLib::Simple(name) => name.clone(),
            WorkspaceLib::Detailed { name, .. } => name.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_config() {
        let yaml = r#"
version: 1
name: test-monorepo
mode: docker-first

catalog:
  react: 19.0.0
  next: 15.4.0

workspaces:
  apps:
    - dashboard
    - api
  libs:
    - core
"#;

        let config: WorkspaceConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.version, 1);
        assert_eq!(config.name, "test-monorepo");
        assert_eq!(config.catalog.get("react"), Some(&"19.0.0".to_string()));
    }
}
