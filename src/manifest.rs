use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::config::Mode;

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
    /// Project metadata (SoT for Cargo.toml, Homebrew, etc.)
    #[serde(default)]
    pub project: MetaSection,
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
    /// App definitions (for package.json generation)
    #[serde(default)]
    pub app: Vec<ProjectDefinition>,
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
    /// Documentation management (CLAUDE.md, .cursorrules, etc.)
    #[serde(default)]
    pub docs: DocsSection,
    /// CI/CD configuration
    #[serde(default)]
    pub ci: CiSection,
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
        // Rule definitions
        let mut rule = IndexMap::new();
        rule.insert(
            "verify".to_string(),
            RuleConfig {
                commands: vec!["airis lint".to_string(), "airis test".to_string()],
            },
        );
        rule.insert(
            "ci".to_string(),
            RuleConfig {
                commands: vec![
                    "airis lint".to_string(),
                    "airis test".to_string(),
                    "airis build".to_string(),
                ],
            },
        );

        // Catalog with common TypeScript/React dependencies
        let mut catalog = IndexMap::new();
        catalog.insert("react".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("react-dom".to_string(), CatalogEntry::Follow(FollowConfig { follow: "react".to_string() }));
        catalog.insert("next".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("typescript".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("tailwindcss".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("zod".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("vitest".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("eslint".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("prettier".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));

        // Packages section
        let packages = PackagesSection {
            workspaces: vec!["apps/*".to_string(), "libs/*".to_string(), "packages/*".to_string()],
            catalog,
            root: PackageDefinition {
                dependencies: IndexMap::new(),
                dev_dependencies: IndexMap::new(),
                optional_dependencies: IndexMap::new(),
                scripts: {
                    let mut scripts = IndexMap::new();
                    scripts.insert("dev".to_string(), "echo 'Run: airis dev'".to_string());
                    scripts.insert("build".to_string(), "echo 'Run: airis build'".to_string());
                    scripts.insert("lint".to_string(), "echo 'Run: airis lint'".to_string());
                    scripts.insert("test".to_string(), "echo 'Run: airis test'".to_string());
                    scripts
                },
                engines: IndexMap::new(),
                pnpm: PnpmConfig::default(),
            },
            app: vec![],
        };

        // Guards for Docker-first enforcement
        let guards = GuardsSection {
            deny: vec![
                "npm".to_string(),
                "yarn".to_string(),
                "pnpm".to_string(),
                "bun".to_string(),
            ],
            wrap: IndexMap::new(),
            deny_with_message: IndexMap::new(),
            forbid: vec![
                "npm".to_string(),
                "yarn".to_string(),
                "pnpm".to_string(),
                "docker".to_string(),
                "docker-compose".to_string(),
            ],
            danger: vec![
                "rm -rf /".to_string(),
                "chmod -R 777".to_string(),
            ],
        };

        // Remap common commands to airis
        let mut remap = IndexMap::new();
        remap.insert("npm install".to_string(), "airis install".to_string());
        remap.insert("pnpm install".to_string(), "airis install".to_string());
        remap.insert("yarn install".to_string(), "airis install".to_string());
        remap.insert("npm run dev".to_string(), "airis dev".to_string());
        remap.insert("pnpm dev".to_string(), "airis dev".to_string());
        remap.insert("docker compose up".to_string(), "airis up".to_string());
        remap.insert("docker compose down".to_string(), "airis down".to_string());

        Manifest {
            version: 1,
            mode: Mode::DockerFirst,
            project: MetaSection {
                id: name.to_string(),
                binary_name: String::new(),
                version: "1.0.0".to_string(),
                description: format!("{} - Docker-first monorepo", name),
                authors: vec![],
                license: "MIT".to_string(),
                homepage: String::new(),
                repository: String::new(),
                keywords: vec!["monorepo".to_string(), "docker".to_string(), "typescript".to_string()],
                categories: vec!["development-tools".to_string()],
                rust_edition: String::new(),
            },
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
            docker: DockerSection {
                base_image: "node:22-alpine".to_string(),
                workdir: "/app".to_string(),
                workspace: Some(DockerWorkspaceSection {
                    service: "workspace".to_string(),
                    volumes: vec!["node_modules".to_string()],
                }),
            },
            just: None,
            service: IndexMap::new(),
            rule,
            packages,
            guards,
            app: vec![],
            orchestration: OrchestrationSection::default(),
            commands: {
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
            },
            remap,
            versioning: VersioningSection {
                strategy: VersioningStrategy::Manual,
                source: "1.0.0".to_string(),
            },
            docs: DocsSection::default(),
            ci: CiSection::default(),
        }
    }
}

/// Project metadata - Source of Truth for Cargo.toml, Homebrew formula, etc.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct MetaSection {
    /// Project ID (e.g., "airis-workspace")
    #[serde(default)]
    pub id: String,
    /// CLI binary name (e.g., "airis")
    #[serde(default)]
    pub binary_name: String,
    /// Semantic version (e.g., "1.4.0")
    #[serde(default)]
    pub version: String,
    /// Short description
    #[serde(default)]
    pub description: String,
    /// Authors list
    #[serde(default)]
    pub authors: Vec<String>,
    /// License (e.g., "MIT")
    #[serde(default)]
    pub license: String,
    /// Project homepage URL
    #[serde(default)]
    pub homepage: String,
    /// Repository URL
    #[serde(default)]
    pub repository: String,
    /// Keywords for discovery
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Categories for classification
    #[serde(default)]
    pub categories: Vec<String>,
    /// Rust edition (e.g., "2024")
    #[serde(default)]
    pub rust_edition: String,
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

/// Documentation management configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DocsSection {
    /// List of documentation files to manage (e.g., ["CLAUDE.md", ".cursorrules"])
    #[serde(default)]
    pub targets: Vec<String>,
    /// Overwrite mode: "warn" (default) or "backup"
    #[serde(default = "default_docs_mode")]
    pub mode: DocsMode,
}

impl Default for DocsSection {
    fn default() -> Self {
        DocsSection {
            targets: vec![],
            mode: default_docs_mode(),
        }
    }
}

fn default_docs_mode() -> DocsMode {
    DocsMode::Warn
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DocsMode {
    /// Warn and refuse to overwrite existing files
    Warn,
    /// Create .bak backup before overwriting
    Backup,
}

/// CI/CD configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CiSection {
    /// Enable CI workflow generation
    #[serde(default = "default_ci_enabled")]
    pub enabled: bool,
    /// Auto-merge from development branch to main
    #[serde(default)]
    pub auto_merge: AutoMergeConfig,
    /// Auto-versioning using Conventional Commits
    #[serde(default = "default_true")]
    pub auto_version: bool,
    /// GitHub repository owner/name (e.g., "agiletec-inc/my-project")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Homebrew tap repository (e.g., "agiletec-inc/homebrew-tap")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homebrew_tap: Option<String>,
}

impl Default for CiSection {
    fn default() -> Self {
        CiSection {
            enabled: default_ci_enabled(),
            auto_merge: AutoMergeConfig::default(),
            auto_version: true,
            repository: None,
            homebrew_tap: None,
        }
    }
}

fn default_ci_enabled() -> bool {
    true
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AutoMergeConfig {
    /// Enable auto-merge
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Source branch (default: "next")
    #[serde(default = "default_source_branch")]
    pub from: String,
    /// Target branch (default: "main")
    #[serde(default = "default_target_branch")]
    pub to: String,
}

impl Default for AutoMergeConfig {
    fn default() -> Self {
        AutoMergeConfig {
            enabled: true,
            from: default_source_branch(),
            to: default_target_branch(),
        }
    }
}

fn default_source_branch() -> String {
    "next".to_string()
}

fn default_target_branch() -> String {
    "main".to_string()
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
