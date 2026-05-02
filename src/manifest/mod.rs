mod schema;
// pub(crate) so tests submodule can reach validation::levenshtein_distance
mod global_config;
pub(crate) mod validation;

#[cfg(test)]
mod tests;

pub use global_config::*;
pub use schema::*;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use colored::Colorize;
use indexmap::IndexMap;

pub const MANIFEST_FILE: &str = "manifest.toml";

impl Manifest {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {:?}", path.as_ref()))?;

        Self::parse(&content)
    }

    /// Load and parse manifest WITHOUT strict validation (loose mode).
    pub fn load_loose<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {:?}", path.as_ref()))?;

        Self::parse_loose(&content)
    }

    /// Parse manifest from TOML string and perform post-processing WITHOUT strict validation.
    pub fn parse_loose(content: &str) -> Result<Self> {
        let mut manifest: Manifest =
            toml::from_str(content).with_context(|| "Failed to parse manifest.toml")?;

        manifest.migrate_testing_to_policy();
        manifest.warn_runtime_image_overlap();

        if let Err(e) = manifest.validate() {
            eprintln!(
                "\n{} {}",
                "⚠️  Manifest Validation Warning:".yellow().bold(),
                e
            );
            eprintln!("   Attempting to continue despite validation errors...\n");
        }

        manifest.resolve_conventions();
        Ok(manifest)
    }

    /// Parse manifest from TOML string and perform post-processing (migration, validation, resolution)
    pub fn parse(content: &str) -> Result<Self> {
        let mut manifest: Manifest =
            toml::from_str(content).with_context(|| "Failed to parse manifest.toml")?;

        // [testing] → [policy.testing] migration fallback
        manifest.migrate_testing_to_policy();
        manifest.warn_runtime_image_overlap();

        manifest.validate()?;
        manifest.resolve_conventions();

        // Post-resolve validation
        for (i, app) in manifest.app.iter().enumerate() {
            if app.name.is_empty() {
                bail!(
                    "[[app]] entry #{} has empty name and no path to derive from",
                    i + 1
                );
            }
        }

        Ok(manifest)
    }

    /// Warn when [workspace].image and [runtimes].node both define the Node version.
    ///
    /// Phase 1a only emits an advisory; the workspace Dockerfile generator that
    /// honours [runtimes] lands in Phase 1c. Until then [workspace].image keeps
    /// driving Node selection so existing manifests stay byte-identical.
    fn warn_runtime_image_overlap(&self) {
        let default_image = crate::channel::defaults::NODE_LTS_IMAGE;
        let workspace_image_overridden = self.workspace.image != default_image;

        if let Some(node) = &self.runtimes.node
            && workspace_image_overridden
        {
            eprintln!(
                "⚠️  Both [workspace] image (\"{}\") and [runtimes.node] (\"{}\") are set. \
                 Phase 1c will pick [runtimes.node]; [workspace] image is deprecated.",
                self.workspace.image,
                node.version()
            );
        }
    }

    /// Migrate top-level [testing] to [policy.testing] with deprecation warning.
    fn migrate_testing_to_policy(&mut self) {
        let has_top_level_testing = self.testing != TestingSection::default();
        let has_policy_testing = self.policy.testing != TestingSection::default();

        if has_top_level_testing && !has_policy_testing {
            self.policy.testing = self.testing.clone();
            eprintln!("⚠️  [testing] is deprecated. Move to [policy.testing] in manifest.toml");
        } else if has_top_level_testing && has_policy_testing {
            eprintln!(
                "⚠️  Both [testing] and [policy.testing] found. Using [policy.testing]. Remove [testing]."
            );
        }
    }

    /// Apply convention-based defaults and discover projects from disk.
    ///
    /// Implements 'Thin Manifest + Strong Convention' philosophy:
    /// 1. Discover projects matching convention patterns (apps/*, libs/*, products/*).
    /// 2. Merge explicit overrides from [apps.xxx] and [libs.xxx] table formats.
    /// 3. Merge explicit entries from [[app]] list.
    fn resolve_conventions(&mut self) {
        let workspace = self.workspace.clone();
        let mut normalized = IndexMap::new();

        // Step 1: Discover from disk (Repo Convention)
        let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let patterns = if !self.packages.workspaces.is_empty() {
            self.packages.workspaces.clone()
        } else {
            vec![
                "apps/*".to_string(),
                "libs/*".to_string(),
                "products/*".to_string(),
            ]
        };

        if let Ok(discovered) =
            crate::commands::discover::discover_from_workspaces(&patterns, &root)
        {
            for disc in discovered {
                let kind = if disc.path.starts_with("libs/") {
                    "lib"
                } else {
                    "app"
                };
                let mut project = ProjectDefinition {
                    name: disc.name.clone(),
                    path: Some(disc.path.clone()),
                    framework: Some(disc.framework.to_string()),
                    kind: Some(kind.to_string()),
                    ..Default::default()
                };
                project.resolve(&workspace);
                normalized.insert(project.name.clone(), project);
            }
        }

        // Step 2: Merge Map-based overrides ([apps.xxx], [libs.xxx])
        for (name, config) in &self.apps {
            let entry = normalized.entry(name.clone()).or_insert_with(|| {
                let mut p = ProjectDefinition {
                    name: name.clone(),
                    kind: Some("app".to_string()),
                    ..Default::default()
                };
                p.resolve(&workspace);
                p
            });

            if let Some(ref path) = config.path {
                entry.path = Some(path.clone());
            }
            if let Some(ref fw) = config.framework.clone().or_else(|| config.app_type.clone()) {
                entry.framework = Some(fw.clone());
            }
            if !config.deps.is_empty() {
                entry.deps.extend(config.deps.clone());
            }
            if !config.dev_deps.is_empty() {
                entry.dev_deps.extend(config.dev_deps.clone());
            }
            if !config.scripts.is_empty() {
                entry.scripts.extend(config.scripts.clone());
            }
            entry.resolve(&workspace);
        }

        for (name, config) in &self.libs {
            let entry = normalized.entry(name.clone()).or_insert_with(|| {
                let mut p = ProjectDefinition {
                    name: name.clone(),
                    kind: Some("lib".to_string()),
                    ..Default::default()
                };
                p.resolve(&workspace);
                p
            });

            if let Some(ref path) = config.path {
                entry.path = Some(path.clone());
            }
            if let Some(ref fw) = config.framework {
                entry.framework = Some(fw.clone());
            }
            if !config.deps.is_empty() {
                entry.deps.extend(config.deps.clone());
            }
            if !config.scripts.is_empty() {
                entry.scripts.extend(config.scripts.clone());
            }
            entry.resolve(&workspace);
        }

        // Step 3: Merge Vector-based entries ([[app]])
        for explicit in &self.app {
            let entry = normalized.entry(explicit.name.clone()).or_insert_with(|| {
                let mut p = explicit.clone();
                p.resolve(&workspace);
                p
            });

            // Merge fields from explicit into entry
            if explicit.path.is_some() {
                entry.path = explicit.path.clone();
            }
            if explicit.framework.is_some() {
                entry.framework = explicit.framework.clone();
            }
            if !explicit.deps.is_empty() {
                entry.deps.extend(explicit.deps.clone());
            }
            if !explicit.dev_deps.is_empty() {
                entry.dev_deps.extend(explicit.dev_deps.clone());
            }
            if !explicit.scripts.is_empty() {
                entry.scripts.extend(explicit.scripts.clone());
            }
            entry.resolve(&workspace);
        }

        // Final result: the normalized vector used by the rest of the tool
        self.app = normalized.into_values().collect();
    }

    /// Returns true if this manifest defines a Node.js workspace.
    /// Determined by whether package_manager is explicitly set in [workspace].
    pub fn has_workspace(&self) -> bool {
        !self.workspace.package_manager.is_empty()
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize manifest.toml contents")?;

        fs::write(path.as_ref(), content)
            .with_context(|| format!("Failed to write {:?}", path.as_ref()))?;

        Ok(())
    }

    /// Get the effective Node.js version.
    /// Priority: [workspace].node > [ci].node_version > extracted from [workspace].image > "22"
    #[allow(dead_code)]
    pub fn node_version(&self) -> String {
        // v2: explicit node field
        if let Some(ref v) = self.workspace.node {
            return v.clone();
        }
        // ci.node_version override
        if let Some(ref v) = self.ci.node_version {
            return v.clone();
        }
        // Extract from image string like "node:24-bookworm"
        let image = &self.workspace.image;
        if image.starts_with("node:")
            && let Some(version_part) = image.strip_prefix("node:")
        {
            let version = version_part.split('-').next().unwrap_or("22");
            return version.to_string();
        }
        "22".to_string()
    }

    /// Get deploy profiles from [profile] section.
    /// Returns profiles that have a branch (i.e., are deploy targets).
    #[allow(dead_code)]
    pub fn deploy_profiles(&self) -> Vec<(&str, &ProfileSection)> {
        self.profile
            .iter()
            .filter(|(_, p)| p.branch.is_some())
            .map(|(name, p)| (name.as_str(), p))
            .collect()
    }

    /// Collect all workspace directory paths from apps, libs, and packages.workspaces globs.
    ///
    /// Returns paths like "apps/corporate", "libs/ui", "products/my-app" etc.
    /// Uses the `path` field if set, otherwise defaults to "apps/{key}" or "libs/{key}".
    /// Also expands glob patterns from `packages.workspaces` (e.g. "products/*")
    /// relative to the given root directory.
    #[allow(dead_code)]
    pub fn all_workspace_paths(&self) -> Vec<String> {
        self.all_workspace_paths_in(".")
    }

    /// Like `all_workspace_paths` but resolves glob patterns relative to a specific root.
    pub fn all_workspace_paths_in(&self, root: &str) -> Vec<String> {
        let mut paths = Vec::new();

        for (key, app) in &self.apps {
            let path = app.path.clone().unwrap_or_else(|| format!("apps/{}", key));
            paths.push(path);
        }

        for (key, lib) in &self.libs {
            let path = lib.path.clone().unwrap_or_else(|| format!("libs/{}", key));
            paths.push(path);
        }

        // Expand glob patterns from packages.workspaces (e.g. "products/*", "packages/*")
        let root_path = Path::new(root);
        for pattern in &self.packages.workspaces {
            if pattern.starts_with('!') {
                continue; // skip exclude patterns
            }
            let full_pattern = root_path.join(pattern).to_string_lossy().to_string();
            if let Ok(entries) = glob::glob(&full_pattern) {
                for entry in entries.flatten() {
                    // Skip paths inside node_modules or build artifact directories
                    // (transitive deps and generated files are not workspaces)
                    let skip_dirs = [
                        "node_modules",
                        ".next",
                        "dist",
                        ".turbo",
                        ".swc",
                        "build",
                        "out",
                    ];
                    if entry
                        .components()
                        .any(|c| skip_dirs.iter().any(|d| c.as_os_str() == *d))
                    {
                        continue;
                    }
                    if entry.is_dir() && entry.join("package.json").exists() {
                        // Strip root prefix to get relative path
                        let p = entry
                            .strip_prefix(root_path)
                            .unwrap_or(&entry)
                            .to_string_lossy()
                            .to_string();
                        if !paths.contains(&p) {
                            paths.push(p);
                        }
                    }
                }
            }
        }

        paths
    }

    /// Create a default manifest with project name
    /// NOTE: This is kept as reference for MCP agent's manifest generation
    #[allow(dead_code)]
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
        catalog.insert(
            "react".to_string(),
            CatalogEntry::Policy(VersionPolicy::Latest),
        );
        catalog.insert(
            "react-dom".to_string(),
            CatalogEntry::Follow(FollowConfig {
                follow: "react".to_string(),
            }),
        );
        catalog.insert(
            "next".to_string(),
            CatalogEntry::Policy(VersionPolicy::Latest),
        );
        catalog.insert(
            "typescript".to_string(),
            CatalogEntry::Policy(VersionPolicy::Latest),
        );
        catalog.insert(
            "tailwindcss".to_string(),
            CatalogEntry::Policy(VersionPolicy::Latest),
        );
        catalog.insert(
            "zod".to_string(),
            CatalogEntry::Policy(VersionPolicy::Latest),
        );
        catalog.insert(
            "vitest".to_string(),
            CatalogEntry::Policy(VersionPolicy::Latest),
        );
        catalog.insert(
            "eslint".to_string(),
            CatalogEntry::Policy(VersionPolicy::Latest),
        );
        catalog.insert(
            "prettier".to_string(),
            CatalogEntry::Policy(VersionPolicy::Latest),
        );

        // Packages section
        let packages = PackagesSection {
            workspaces: vec![
                "apps/*".to_string(),
                "libs/*".to_string(),
                "packages/*".to_string(),
            ],
            default_policy: None,
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
                "npx".to_string(),
                "pip".to_string(),
                "pip3".to_string(),
                "uv".to_string(),
                "poetry".to_string(),
                "cargo".to_string(),
            ],
            allow: vec![],
            wrap: {
                let mut wrap = IndexMap::new();
                wrap.insert(
                    "pnpm".to_string(),
                    "docker compose exec workspace pnpm".to_string(),
                );
                wrap.insert(
                    "npm".to_string(),
                    "docker compose exec workspace npm".to_string(),
                );
                wrap
            },
            deny_with_message: IndexMap::new(),
            forbid: vec![
                "npm".to_string(),
                "yarn".to_string(),
                "pnpm".to_string(),
                "bun".to_string(),
                "docker".to_string(),
                "docker-compose".to_string(),
            ],
            danger: vec!["rm -rf /".to_string(), "chmod -R 777".to_string()],
        };

        // Remap common commands to airis
        let mut remap = IndexMap::new();
        remap.insert("npm install".to_string(), "airis up".to_string());
        remap.insert("pnpm install".to_string(), "airis up".to_string());
        remap.insert("yarn install".to_string(), "airis up".to_string());
        remap.insert("npm run dev".to_string(), "airis up".to_string());
        remap.insert("pnpm dev".to_string(), "airis up".to_string());
        remap.insert("docker compose up".to_string(), "airis up".to_string());
        remap.insert("docker compose down".to_string(), "airis down".to_string());

        Manifest {
            version: 1,
            project: MetaSection {
                id: name.to_string(),
                binary_name: String::new(),
                version: "1.0.0".to_string(),
                description: format!("{} - Docker-first monorepo", name),
                authors: vec![],
                license: "MIT".to_string(),
                homepage: String::new(),
                repository: String::new(),
                keywords: vec![
                    "monorepo".to_string(),
                    "docker".to_string(),
                    "typescript".to_string(),
                ],
                categories: vec!["development-tools".to_string()],
                rust_edition: String::new(),
            },
            workspace: WorkspaceSection {
                name: format!("airis-{}", name), // Prefix to avoid Docker name collisions
                scope: None,
                package_manager: "pnpm@10.33.0".to_string(),
                node: None,
                service: String::new(),
                image: crate::channel::defaults::NODE_LTS_IMAGE.to_string(),
                workdir: "/app".to_string(),
                workspaces: vec![],
                volumes: vec![format!("{}-node-modules:/app/node_modules", name)],
                clean: CleanSection::default(),
            },
            catalog: IndexMap::new(),
            workspaces: WorkspacesSection::default(),
            dev: HooksSection::default(),
            apps: IndexMap::new(),
            libs: IndexMap::new(),
            docker: DockerSection {
                base_image: crate::channel::defaults::NODE_LTS_IMAGE.to_string(),
                workdir: "/app".to_string(),
                workspace: Some(DockerWorkspaceSection {
                    service: "workspace".to_string(),
                    volumes: vec!["node_modules".to_string()],
                }),
                compose: default_compose_file(),
                service: String::new(),
                routes: vec![],
                shim_commands: default_shim_commands(),
            },
            just: None,
            service: IndexMap::new(),
            rule,
            packages,
            guards,
            stack: IndexMap::new(),
            app: vec![],
            orchestration: OrchestrationSection::default(),
            commands: {
                let mut cmds = IndexMap::new();
                cmds.insert(
                    "up".to_string(),
                    "docker compose up -d --build --remove-orphans".to_string(),
                );
                cmds.insert(
                    "down".to_string(),
                    "docker compose down --remove-orphans".to_string(),
                );
                cmds.insert("ps".to_string(), "docker compose ps".to_string());
                cmds
            },
            hooks: PreCommandHooks::default(),
            remap,
            versioning: VersioningSection {
                strategy: VersioningStrategy::Manual,
                source: "1.0.0".to_string(),
            },
            docs: DocsSection::default(),
            ci: CiSection::default(),
            templates: TemplatesSection::default(),
            runtimes: RuntimesSection::default(),
            env: EnvSection::default(),
            secrets: None,
            typescript: TypescriptSection::default(),
            profile: IndexMap::new(),
            dep_group: IndexMap::new(),
            env_group: IndexMap::new(),
            preset: IndexMap::new(),
            external: IndexMap::new(),
            root: None,
            overrides: IndexMap::new(),
            mcp: McpSection::default(),
            testing: TestingSection::default(),
            policy: PolicySection::default(),
        }
    }
}
