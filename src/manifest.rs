use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::config::{Mode, WorkspaceApp, WorkspaceConfig, Workspaces};

pub const MANIFEST_FILE: &str = "manifest.toml";

fn default_version() -> u32 {
    1
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Manifest {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub workspace: WorkspaceSection,
    #[serde(default)]
    pub catalog: IndexMap<String, String>,
    #[serde(default)]
    pub workspaces: WorkspacesSection,
    #[serde(default)]
    pub dev: DevSection,
    #[serde(default)]
    pub apps: IndexMap<String, AppConfig>,
    #[serde(default)]
    pub libs: IndexMap<String, LibConfig>,
    #[serde(default)]
    pub docker: DockerSection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub just: Option<JustSection>,
    #[serde(default)]
    pub service: IndexMap<String, ServiceConfig>,
    #[serde(default)]
    pub rule: IndexMap<String, RuleConfig>,
    #[serde(default)]
    pub packages: PackagesSection,
    #[serde(default)]
    pub guards: GuardsSection,
    #[serde(default)]
    pub project: Vec<ProjectDefinition>,
    #[serde(default)]
    pub orchestration: OrchestrationSection,
    /// User-defined commands (airis run <task>)
    #[serde(default)]
    pub commands: IndexMap<String, String>,
    /// LLM command remapping (e.g., "npm install" → "airis install")
    #[serde(default)]
    pub remap: IndexMap<String, String>,
    /// Version management configuration
    #[serde(default)]
    pub versioning: VersioningSection,
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
            catalog: IndexMap::new(),
            root: PackageDefinition {
                dependencies: IndexMap::new(),
                dev_dependencies: {
                    let mut dev = IndexMap::new();
                    dev.insert("typescript".to_string(), "5.6.2".to_string());
                    dev.insert("eslint".to_string(), "9.3.0".to_string());
                    dev
                },
                optional_dependencies: IndexMap::new(),
                scripts: IndexMap::new(),
                engines: IndexMap::new(),
                pnpm: PnpmConfig::default(),
            },
            app: vec![AppPackageDefinition {
                pattern: "apps/*".to_string(),
                dependencies: IndexMap::new(),
                dev_dependencies: IndexMap::new(),
                scripts: IndexMap::new(),
            }],
        };

        Manifest {
            version: 1,
            mode: Mode::DockerFirst,
            workspace: WorkspaceSection {
                name: name.to_string(),
                package_manager: "pnpm@10.22.0".to_string(),
                service: "workspace".to_string(),
                image: "node:22-alpine".to_string(),
                workdir: "/app".to_string(),
                volumes: vec!["workspace-node-modules:/app/node_modules".to_string()],
            },
            catalog: IndexMap::new(),
            workspaces: WorkspacesSection::default(),
            dev: DevSection::default(),
            apps: IndexMap::new(),
            libs: IndexMap::new(),
            docker: DockerSection::default(),
            just: None,
            service: IndexMap::new(),
            rule,
            packages,
            guards: GuardsSection::default(),
            project: vec![],
            orchestration: OrchestrationSection::default(),
            commands: IndexMap::new(),
            remap: IndexMap::new(),
            versioning: VersioningSection::default(),
        }
    }

    pub fn to_workspace_config(&self) -> WorkspaceConfig {
        let mut config = WorkspaceConfig::default();
        config.name = self.workspace.name.clone();
        config.mode = self.mode.clone();

        let apps = self
            .dev
            .autostart
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
    pub autostart: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supabase: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traefik: Option<String>,
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LibConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
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
    pub catalog: IndexMap<String, CatalogEntry>,
    #[serde(default)]
    pub root: PackageDefinition,
    #[serde(rename = "app", default)]
    pub app: Vec<AppPackageDefinition>,
}

/// Catalog entry can be:
/// - "latest" → resolve to latest npm version
/// - "lts" → resolve to LTS version
/// - "^5.0.0" → specific semver (used as-is)
/// - { follow = "react" } → follow another package's version
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CatalogEntry {
    Follow(FollowConfig),
    Policy(VersionPolicy),
    Version(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowConfig {
    pub follow: String,
}

impl Default for CatalogEntry {
    fn default() -> Self {
        CatalogEntry::Version("*".to_string())
    }
}

impl CatalogEntry {
    /// Returns the resolved version string (for already-resolved entries)
    /// or the policy/follow target (for unresolved entries)
    pub fn as_str(&self) -> &str {
        match self {
            CatalogEntry::Follow(f) => &f.follow,
            CatalogEntry::Policy(p) => p.as_str(),
            CatalogEntry::Version(v) => v.as_str(),
        }
    }

    /// Check if this entry needs resolution (is a policy or follow)
    pub fn needs_resolution(&self) -> bool {
        matches!(self, CatalogEntry::Policy(_) | CatalogEntry::Follow(_))
    }

    /// Check if this entry follows another package
    pub fn is_follow(&self) -> bool {
        matches!(self, CatalogEntry::Follow(_))
    }

    /// Get the follow target if this is a Follow entry
    pub fn follow_target(&self) -> Option<&str> {
        match self {
            CatalogEntry::Follow(f) => Some(&f.follow),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionPolicy {
    Latest,
    Lts,
}

impl VersionPolicy {
    pub fn as_str(&self) -> &str {
        match self {
            VersionPolicy::Latest => "latest",
            VersionPolicy::Lts => "lts",
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PackageDefinition {
    #[serde(default)]
    pub dependencies: IndexMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    pub dev_dependencies: IndexMap<String, String>,
    #[serde(rename = "optionalDependencies", default)]
    pub optional_dependencies: IndexMap<String, String>,
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
    #[serde(default)]
    pub engines: IndexMap<String, String>,
    #[serde(default)]
    pub pnpm: PnpmConfig,
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
pub struct PnpmConfig {
    #[serde(default)]
    pub overrides: IndexMap<String, String>,
    #[serde(rename = "peerDependencyRules", default)]
    pub peer_dependency_rules: PeerDependencyRules,
    #[serde(rename = "onlyBuiltDependencies", default)]
    pub only_built_dependencies: Vec<String>,
    #[serde(rename = "allowedScripts", default)]
    pub allowed_scripts: IndexMap<String, bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PeerDependencyRules {
    #[serde(rename = "ignoreMissing", default)]
    pub ignore_missing: Vec<String>,
    #[serde(rename = "allowedVersions", default)]
    pub allowed_versions: IndexMap<String, String>,
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

    /// LLM-specific: completely forbid these commands
    #[serde(default)]
    pub forbid: Vec<String>,

    /// LLM-specific: dangerous commands (warn humans, block LLMs)
    #[serde(default)]
    pub danger: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct WorkspacesSection {
    #[serde(default)]
    pub apps: Vec<WorkspaceAppMeta>,
    #[serde(default)]
    pub libs: Vec<WorkspaceLibMeta>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkspaceAppMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub app_type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkspaceLibMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub lib_type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct DockerSection {
    #[serde(rename = "baseImage", default)]
    pub base_image: String,
    #[serde(default)]
    pub workdir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<DockerWorkspaceSection>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DockerWorkspaceSection {
    pub service: String,
    #[serde(default)]
    pub volumes: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JustSection {
    pub output: String,
    #[serde(default)]
    pub features: Vec<String>,
}

/// Project definition for full package.json generation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProjectDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,  // "app" | "lib" | "service"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,  // "react-vite" | "nextjs" | "node" | "rust"
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
    #[serde(default)]
    pub deps: IndexMap<String, String>,
    #[serde(default)]
    pub dev_deps: IndexMap<String, String>,
}

/// Orchestration configuration for multi-compose setup
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct OrchestrationSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev: Option<OrchestrationDev>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OrchestrationDev {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supabase: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traefik: Option<String>,
}

/// Version management configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersioningSection {
    /// Version bump strategy
    #[serde(default = "default_versioning_strategy")]
    pub strategy: VersioningStrategy,
    /// Source of truth version (manually maintained or auto-updated)
    #[serde(default = "default_version_source")]
    pub source: String,
}

impl Default for VersioningSection {
    fn default() -> Self {
        VersioningSection {
            strategy: default_versioning_strategy(),
            source: default_version_source(),
        }
    }
}

fn default_versioning_strategy() -> VersioningStrategy {
    VersioningStrategy::Manual
}

fn default_version_source() -> String {
    "0.1.0".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum VersioningStrategy {
    /// Manual version bumps only
    Manual,
    /// Auto-increment minor version on every commit
    Auto,
    /// Use Conventional Commits to determine bump type
    ConventionalCommits,
}

impl VersioningSection {
    /// Parse semver version string into (major, minor, patch)
    pub fn parse_version(version: &str) -> Result<(u32, u32, u32)> {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            anyhow::bail!("Invalid version format: {}", version);
        }

        let major = parts[0].parse::<u32>()?;
        let minor = parts[1].parse::<u32>()?;
        let patch = parts[2].parse::<u32>()?;

        Ok((major, minor, patch))
    }

    /// Bump major version (x.0.0)
    pub fn bump_major(&mut self) -> Result<String> {
        let (major, _, _) = Self::parse_version(&self.source)?;
        self.source = format!("{}.0.0", major + 1);
        Ok(self.source.clone())
    }

    /// Bump minor version (x.y.0)
    pub fn bump_minor(&mut self) -> Result<String> {
        let (major, minor, _) = Self::parse_version(&self.source)?;
        self.source = format!("{}.{}.0", major, minor + 1);
        Ok(self.source.clone())
    }

    /// Bump patch version (x.y.z)
    pub fn bump_patch(&mut self) -> Result<String> {
        let (major, minor, patch) = Self::parse_version(&self.source)?;
        self.source = format!("{}.{}.{}", major, minor, patch + 1);
        Ok(self.source.clone())
    }
}
