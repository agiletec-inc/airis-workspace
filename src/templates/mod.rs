use anyhow::{Context, Result};
use handlebars::Handlebars;
use indexmap::IndexMap;
use serde_json::json;
use std::process::Command;

use crate::version_resolver::resolve_version;
use crate::manifest::{MANIFEST_FILE, Manifest};


/// Resolve dependency versions by expanding catalog references and version policies
///
/// Supports:
/// - "catalog:" → look up package name in resolved_catalog
/// - "catalog:key" → look up "key" in resolved_catalog
/// - "latest" / "lts" → resolve from npm registry
/// - Specific version (e.g. "^1.0.0") → use as-is
fn resolve_dependencies(
    deps: &IndexMap<String, String>,
    resolved_catalog: &IndexMap<String, String>,
) -> Result<IndexMap<String, String>> {
    let mut resolved = IndexMap::new();

    for (package, version_spec) in deps {
        let resolved_version = if version_spec == "catalog:" {
            // "catalog:" → use package name as key
            resolved_catalog
                .get(package)
                .cloned()
                .unwrap_or_else(|| {
                    eprintln!(
                        "⚠️  Warning: {} not found in catalog, using original spec: {}",
                        package, version_spec
                    );
                    version_spec.clone()
                })
        } else if let Some(catalog_key) = version_spec.strip_prefix("catalog:") {
            // "catalog:key" → look up specific key
            resolved_catalog
                .get(catalog_key)
                .cloned()
                .unwrap_or_else(|| {
                    eprintln!(
                        "⚠️  Warning: catalog key '{}' not found for {}, using original spec: {}",
                        catalog_key, package, version_spec
                    );
                    version_spec.clone()
                })
        } else if version_spec == "latest" || version_spec == "lts" {
            // Resolve from npm registry
            resolve_version(package, version_spec)
                .unwrap_or_else(|e| {
                    eprintln!(
                        "⚠️  Warning: Failed to resolve {} for {}: {}. Using original spec.",
                        version_spec, package, e
                    );
                    version_spec.clone()
                })
        } else {
            // Use as-is (specific version)
            version_spec.clone()
        };

        resolved.insert(package.clone(), resolved_version);
    }

    Ok(resolved)
}

/// Parse GitHub repository info from git remote URL
/// Returns (owner, repo) tuple
fn detect_github_repo() -> Option<(String, String)> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8(output.stdout).ok()?.trim().to_string();

    // Parse various GitHub URL formats:
    // https://github.com/owner/repo.git
    // git@github.com:owner/repo.git
    // https://github.com/owner/repo
    let repo_path = if url.contains("github.com") {
        if url.starts_with("git@") {
            // git@github.com:owner/repo.git
            url.split(':').nth(1)?
        } else {
            // https://github.com/owner/repo.git
            url.split("github.com/").nth(1)?
        }
    } else {
        return None;
    };

    // Remove .git suffix if present
    let repo_path = repo_path.trim_end_matches(".git");

    // Split into owner/repo
    let parts: Vec<&str> = repo_path.split('/').collect();
    if parts.len() >= 2 {
        Some((parts[0].to_string(), parts[1].to_string()))
    } else {
        None
    }
}

pub struct TemplateEngine {
    hbs: Handlebars<'static>,
}

impl TemplateEngine {
    pub fn new() -> Result<Self> {
        let mut hbs = Handlebars::new();

        // Disable HTML escaping for JSON/YAML output
        hbs.register_escape_fn(handlebars::no_escape);

        hbs.register_template_string("package_json", PACKAGE_JSON_TEMPLATE)?;
        hbs.register_template_string("pnpm_workspace", PNPM_WORKSPACE_TEMPLATE)?;
        hbs.register_template_string("docker_compose", DOCKER_COMPOSE_TEMPLATE)?;
        hbs.register_template_string("dockerfile", DOCKERFILE_TEMPLATE)?;
        hbs.register_template_string("ci_yml", CI_YML_TEMPLATE)?;
        hbs.register_template_string("release_yml", RELEASE_YML_TEMPLATE)?;
        // Note: Cargo.toml template removed - Cargo.toml is source of truth for Rust projects

        Ok(TemplateEngine { hbs })
    }

    pub fn render_ci_yml(&self, manifest: &Manifest) -> Result<String> {
        let data = self.prepare_ci_data(manifest)?;
        self.hbs
            .render("ci_yml", &data)
            .context("Failed to render ci.yml")
    }

    pub fn render_release_yml(&self, manifest: &Manifest) -> Result<String> {
        let data = self.prepare_ci_data(manifest)?;
        self.hbs
            .render("release_yml", &data)
            .context("Failed to render release.yml")
    }

    fn prepare_ci_data(&self, manifest: &Manifest) -> Result<serde_json::Value> {
        // Detect Rust project by checking rust_edition or binary_name
        let is_rust_project = !manifest.project.rust_edition.is_empty()
            || !manifest.project.binary_name.is_empty();

        let binary_name = if manifest.project.binary_name.is_empty() {
            manifest.project.id.clone()
        } else {
            manifest.project.binary_name.clone()
        };

        // Convert project_id to PascalCase for Ruby class name (Formula name = project_id)
        let formula_class = manifest.project.id
            .split(['-', '_'])
            .map(|s| {
                let mut chars = s.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                }
            })
            .collect::<String>();

        // Auto-detect repository info from git remote if not specified
        let (detected_owner, detected_repo) = detect_github_repo().unwrap_or_default();

        // Use manifest values if set, otherwise use auto-detected values
        let repository = manifest
            .ci
            .repository
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{}/{}", detected_owner, detected_repo));

        let homebrew_tap = manifest
            .ci
            .homebrew_tap
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{}/homebrew-tap", detected_owner));

        let has_homebrew = !homebrew_tap.is_empty() && !detected_owner.is_empty();

        Ok(json!({
            "project": manifest.workspace.name,
            "auto_merge_enabled": manifest.ci.auto_merge.enabled,
            "source_branch": manifest.ci.auto_merge.from,
            "target_branch": manifest.ci.auto_merge.to,
            "auto_version": manifest.ci.auto_version,
            "homebrew_tap": homebrew_tap,
            "has_homebrew": has_homebrew,
            "is_rust_project": is_rust_project,
            "binary_name": binary_name,
            "formula_class": formula_class,
            "project_id": manifest.project.id,
            "description": manifest.project.description,
            "repository": repository,
            "runner": manifest.ci.runner.as_deref().unwrap_or("ubuntu-latest"),
            "node_version": manifest.ci.node_version.as_deref().unwrap_or("22"),
            "affected": manifest.ci.affected,
            "concurrency_cancel": manifest.ci.concurrency_cancel,
            "cache": manifest.ci.cache,
            "pnpm_store_path": manifest.ci.pnpm_store_path.as_deref().unwrap_or(""),
            "has_pnpm_store_path": manifest.ci.pnpm_store_path.is_some(),
        }))
    }

    pub fn render_package_json(
        &self,
        manifest: &Manifest,
        resolved_catalog: &IndexMap<String, String>,
    ) -> Result<String> {
        let root = &manifest.packages.root;

        // Build package.json directly with serde_json to avoid Handlebars escaping issues
        let mut package_json = serde_json::json!({
            "name": manifest.workspace.name,
            "version": "0.0.0",
            "private": true,
            "type": "module",
        });

        let obj = package_json.as_object_mut().unwrap();

        // Add engines if present
        if !root.engines.is_empty() {
            obj.insert("engines".to_string(), serde_json::to_value(&root.engines)?);
        }

        // Add packageManager
        obj.insert("packageManager".to_string(), serde_json::json!(manifest.workspace.package_manager));

        // Add workspaces if this is a monorepo with packages
        // This replaces pnpm-workspace.yaml and works with pnpm/npm/yarn/bun
        if !manifest.packages.workspaces.is_empty() {
            obj.insert("workspaces".to_string(), serde_json::to_value(&manifest.packages.workspaces)?);
        }

        // Resolve and add dependencies
        let dependencies = resolve_dependencies(&root.dependencies, resolved_catalog)?;
        obj.insert("dependencies".to_string(), serde_json::to_value(&dependencies)?);

        // Resolve and add devDependencies
        let dev_dependencies = resolve_dependencies(&root.dev_dependencies, resolved_catalog)?;
        obj.insert("devDependencies".to_string(), serde_json::to_value(&dev_dependencies)?);

        // Resolve and add optionalDependencies if present
        if !root.optional_dependencies.is_empty() {
            let optional_dependencies = resolve_dependencies(&root.optional_dependencies, resolved_catalog)?;
            obj.insert("optionalDependencies".to_string(), serde_json::to_value(&optional_dependencies)?);
        }

        // Add pnpm config if present
        if !root.pnpm.overrides.is_empty()
            || !root.pnpm.peer_dependency_rules.ignore_missing.is_empty()
            || !root.pnpm.only_built_dependencies.is_empty()
            || !root.pnpm.allowed_scripts.is_empty()
        {
            obj.insert("pnpm".to_string(), serde_json::to_value(&root.pnpm)?);
        }

        // Add scripts
        obj.insert("scripts".to_string(), serde_json::to_value(&root.scripts)?);

        // Add generation metadata
        obj.insert("_generated".to_string(), serde_json::json!({
            "by": "airis init",
            "from": "manifest.toml",
            "warning": "⚠️  DO NOT EDIT - Update manifest.toml then rerun `airis init`"
        }));

        // Serialize to pretty JSON
        serde_json::to_string_pretty(&package_json)
            .context("Failed to serialize package.json")
    }

    pub fn render_pnpm_workspace(
        &self,
        manifest: &Manifest,
    ) -> Result<String> {
        let data = self.prepare_pnpm_workspace_data(manifest)?;
        self.hbs
            .render("pnpm_workspace", &data)
            .context("Failed to render pnpm-workspace.yaml")
    }

    pub fn render_docker_compose(&self, manifest: &Manifest) -> Result<String> {
        let data = self.prepare_docker_compose_data(manifest)?;
        self.hbs
            .render("docker_compose", &data)
            .context("Failed to render docker-compose.yml")
    }

    pub fn render_dockerfile(&self, manifest: &Manifest) -> Result<String> {
        let data = self.prepare_dockerfile_data(manifest)?;
        self.hbs
            .render("dockerfile", &data)
            .context("Failed to render Dockerfile")
    }

    /// Generate .env.example from manifest.toml [env] section
    pub fn render_env_example(&self, manifest: &Manifest) -> Result<String> {
        let mut lines = vec![
            "# Auto-generated by airis init".to_string(),
            "# DO NOT commit .env file - this is just an example".to_string(),
            "# Copy to .env and fill in actual values".to_string(),
            "".to_string(),
        ];

        // Required variables
        if !manifest.env.required.is_empty() {
            lines.push("# Required environment variables".to_string());
            for var in &manifest.env.required {
                let validation = manifest.env.validation.get(var);
                if let Some(v) = validation
                    && let Some(desc) = &v.description {
                        lines.push(format!("# {}", desc));
                    }
                let example_value = validation
                    .and_then(|v| v.example.as_ref())
                    .map(|e| e.as_str())
                    .unwrap_or("your_value_here");
                lines.push(format!("{}={}", var, example_value));
            }
            lines.push("".to_string());
        }

        // Optional variables
        if !manifest.env.optional.is_empty() {
            lines.push("# Optional environment variables".to_string());
            for var in &manifest.env.optional {
                let validation = manifest.env.validation.get(var);
                if let Some(v) = validation
                    && let Some(desc) = &v.description {
                        lines.push(format!("# {}", desc));
                    }
                let example_value = validation
                    .and_then(|v| v.example.as_ref())
                    .map(|e| e.as_str())
                    .unwrap_or("");
                lines.push(format!("# {}={}", var, example_value));
            }
        }

        Ok(lines.join("\n"))
    }


    /// Generate .envrc for direnv
    /// Adds .airis/bin to PATH and sets COMPOSE_PROJECT_NAME
    pub fn render_envrc(&self, manifest: &Manifest) -> Result<String> {
        let lines = vec![
            "# Auto-generated by airis init".to_string(),
            "# Enable with: direnv allow".to_string(),
            "".to_string(),
            "# Add guards to PATH".to_string(),
            "export PATH=\"$PWD/.airis/bin:$PATH\"".to_string(),
            "".to_string(),
            "# Docker Compose".to_string(),
            "export COMPOSE_PROFILES=\"${COMPOSE_PROFILES:-shell,web}\"".to_string(),
            format!(
                "export COMPOSE_PROJECT_NAME=\"{}\"",
                manifest.workspace.name
            ),
        ];

        Ok(lines.join("\n"))
    }


    /// Generate .npmrc for pnpm store isolation
    pub fn render_npmrc(&self) -> Result<String> {
        Ok(NPMRC_TEMPLATE.to_string())
    }

    fn prepare_pnpm_workspace_data(
        &self,
        manifest: &Manifest,
    ) -> Result<serde_json::Value> {
        Ok(json!({
            "packages": manifest.packages.workspaces,
            "manifest": MANIFEST_FILE,
        }))
    }

    fn prepare_dockerfile_data(&self, manifest: &Manifest) -> Result<serde_json::Value> {
        let pm_bin = manifest.workspace.package_manager.split('@').next().unwrap_or("pnpm");
        Ok(json!({
            "workspace_image": manifest.workspace.image,
            "workdir": manifest.workspace.workdir,
            "pm_bin": pm_bin,
        }))
    }

    fn prepare_docker_compose_data(&self, manifest: &Manifest) -> Result<serde_json::Value> {
        // External services (databases, etc.) - optional, usually empty
        // Most services are defined in their own docker-compose.yml (apps/*, supabase/, traefik/)
        let services: Vec<serde_json::Value> = manifest
            .service
            .iter()
            .map(|(name, svc)| {
                json!({
                    "name": name,
                    "image": svc.image,
                    "port": svc.port,
                    "ports": svc.ports,
                    "command": svc.command,
                    "volumes": svc.volumes,
                    "env": svc.env,
                    "profiles": svc.profiles,
                    "depends_on": svc.depends_on,
                    "restart": svc.restart,
                    "shm_size": svc.shm_size,
                    "container_name": svc.container_name,
                    "working_dir": svc.working_dir,
                    "extra_hosts": svc.extra_hosts,
                    "deploy": svc.deploy,
                    "watch": svc.watch,
                    "extends": svc.extends,
                })
            })
            .collect();

        // Get proxy network from orchestration.networks config (None if not set)
        let proxy_network = manifest
            .orchestration
            .networks
            .as_ref()
            .and_then(|n| n.proxy.clone());

        let default_external = manifest
            .orchestration
            .networks
            .as_ref()
            .map(|n| n.default_external)
            .unwrap_or(false);

        // Workspace volumes from manifest (format: "volume-name:/container/path")
        // Use manifest volumes if defined, otherwise use sensible defaults
        let workdir = &manifest.workspace.workdir;
        let workspace_volumes: Vec<String> = if manifest.workspace.volumes.is_empty() {
            // Default volumes for Node.js workspace isolation
            vec![
                format!("node_modules:{}/node_modules", workdir),
                format!("pnpm_virtual:{}/.pnpm", workdir),
                format!("pnpm_store:/pnpm/store", ),
                format!("next_build:{}/.next", workdir),
                format!("dist_build:{}/dist", workdir),
                format!("build_output:{}/build", workdir),
                format!("out_export:{}/out", workdir),
                format!("turbo_cache:{}/.turbo", workdir),
                format!("swc_cache:{}/.swc", workdir),
                format!("cache_dir:{}/.cache", workdir),
            ]
        } else {
            manifest.workspace.volumes.clone()
        };

        // Auto-generate node_modules volumes for each workspace (apps/libs)
        // This prevents container-installed node_modules from leaking to the host via bind mount
        let mut workspace_volumes = workspace_volumes;
        for ws_path in manifest.all_workspace_paths() {
            let vol_name = format!("ws_nm_{}", ws_path.replace('/', "_"));
            let mount = format!("{}:{}/{}/node_modules", vol_name, workdir, ws_path);
            if !workspace_volumes.iter().any(|v| v.contains(&format!("{}/node_modules", ws_path))) {
                workspace_volumes.push(mount);
            }
        }

        // Extract volume names for the volumes declaration section
        // Format: "volume-name:/path" -> "volume-name"
        let mut volume_names: Vec<String> = workspace_volumes
            .iter()
            .filter_map(|v| v.split(':').next())
            .map(String::from)
            .collect();

        // Also extract named volumes from service definitions
        for svc in manifest.service.values() {
            for vol in &svc.volumes {
                // Named volumes have format "name:/path" (no ./ or / prefix)
                if let Some(name) = vol.split(':').next() {
                    if !name.starts_with('.') && !name.starts_with('/') && !volume_names.contains(&name.to_string()) {
                        volume_names.push(name.to_string());
                    }
                }
            }
        }

        Ok(json!({
            "project": manifest.workspace.name,
            "workspace_image": manifest.workspace.image,
            "workdir": manifest.workspace.workdir,
            "services": services,
            "proxy_network": proxy_network,
            "default_external": default_external,
            "workspace_volumes": workspace_volumes,
            "volume_names": volume_names,
        }))
    }

    // Note: prepare_cargo_toml_data removed - Cargo.toml is source of truth for Rust projects
}

const NPMRC_TEMPLATE: &str = "\
# Auto-generated by airis init
# DO NOT EDIT — regenerate with: airis generate files
# Ensures pnpm store stays inside the container volume
store-dir=/pnpm/store
virtual-store-dir=.pnpm
";

const PACKAGE_JSON_TEMPLATE: &str = r#"{
  "name": "{{name}}",
  "version": "0.0.0",
  "private": true,
  "type": "module",
{{#if has_engines}}
  "engines": {
{{#each engines}}
    "{{@key}}": "{{{this}}}"{{#unless @last}},{{/unless}}
{{/each}}
  },
{{/if}}
  "packageManager": "{{package_manager}}",
  "dependencies": {
{{#each dependencies}}
    "{{@key}}": "{{this}}"{{#unless @last}},{{/unless}}
{{/each}}
  },
  "devDependencies": {
{{#each dev_dependencies}}
    "{{@key}}": "{{this}}"{{#unless @last}},{{/unless}}
{{/each}}
  },
{{#if has_optional_deps}}
  "optionalDependencies": {
{{#each optional_dependencies}}
    "{{@key}}": "{{this}}"{{#unless @last}},{{/unless}}
{{/each}}
  },
{{/if}}
{{#if has_pnpm_config}}
  "pnpm": {
{{#if pnpm.overrides}}
    "overrides": {
{{#each pnpm.overrides}}
      "{{@key}}": "{{this}}"{{#unless @last}},{{/unless}}
{{/each}}
    }{{#if pnpm.peerDependencyRules.ignoreMissing}},{{else}}{{#if pnpm.onlyBuiltDependencies}},{{else}}{{#if pnpm.allowedScripts}},{{/if}}{{/if}}{{/if}}
{{/if}}
{{#if pnpm.peerDependencyRules.ignoreMissing}}
    "peerDependencyRules": {
      "ignoreMissing": [
{{#each pnpm.peerDependencyRules.ignoreMissing}}
        "{{this}}"{{#unless @last}},{{/unless}}
{{/each}}
      ]{{#if pnpm.peerDependencyRules.allowedVersions}},{{/if}}
{{#if pnpm.peerDependencyRules.allowedVersions}}
      "allowedVersions": {
{{#each pnpm.peerDependencyRules.allowedVersions}}
        "{{@key}}": "{{this}}"{{#unless @last}},{{/unless}}
{{/each}}
      }
{{/if}}
    }{{#if pnpm.onlyBuiltDependencies}},{{else}}{{#if pnpm.allowedScripts}},{{/if}}{{/if}}
{{/if}}
{{#if pnpm.onlyBuiltDependencies}}
    "onlyBuiltDependencies": [
{{#each pnpm.onlyBuiltDependencies}}
      "{{this}}"{{#unless @last}},{{/unless}}
{{/each}}
    ]{{#if pnpm.allowedScripts}},{{/if}}
{{/if}}
{{#if pnpm.allowedScripts}}
    "allowedScripts": {
{{#each pnpm.allowedScripts}}
      "{{@key}}": {{this}}{{#unless @last}},{{/unless}}
{{/each}}
    }
{{/if}}
  },
{{/if}}
  "scripts": {
{{#each scripts}}
    "{{@key}}": "{{this}}"{{#unless @last}},{{/unless}}
{{/each}}
  },
  "_generated": {
    "by": "airis init",
    "from": "manifest.toml",
    "warning": "⚠️  DO NOT EDIT - Update manifest.toml then rerun `airis init`"
  }
}
"#;

const PNPM_WORKSPACE_TEMPLATE: &str = r#"# Auto-generated by airis init
# DO NOT EDIT - change manifest.toml instead.
#
# NOTE: No catalog section needed!
# airis resolves versions from manifest.toml [packages.catalog] and writes
# them directly to package.json. This is a superior approach because:
# - Works with any package manager (pnpm/npm/yarn/bun)
# - Supports "latest", "lts", "follow" policies via airis
# - No dependency on pnpm's catalog feature
#
# Use manifest.toml [packages.catalog] for version management:
#   [packages.catalog]
#   next = "latest"      # airis resolves to ^16.0.3
#   react = "lts"        # airis resolves to ^18.3.1
#
# Then reference in dependencies:
#   [packages.root.devDependencies]
#   next = "catalog:"    # → ^16.0.3 in package.json

packages:
{{#each packages}}
  - "{{this}}"
{{/each}}
"#;

const DOCKERFILE_TEMPLATE: &str = r#"FROM {{workspace_image}}

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      build-essential ca-certificates git curl openssh-client \
      python3 pkg-config tini \
      libnspr4 libnss3 libdbus-1-3 libatk1.0-0 libatk-bridge2.0-0 \
      libcups2 libxkbcommon0 libatspi2.0-0 libxcomposite1 libxdamage1 \
      libxfixes3 libxrandr2 libgbm1 libasound2 \
      libdrm2 libxshmfence1 libxcb1 libpango-1.0-0 libcairo2 \
      libglib2.0-0 && \
    rm -rf /var/lib/apt/lists/* && \
    corepack enable

RUN set -eux; \
    if ! id -u app >/dev/null 2>&1; then \
      useradd -m -s /bin/bash app; \
    fi; \
    chown -R app:app /home/app

RUN mkdir -p \
      {{workdir}}/node_modules \
      {{workdir}}/.pnpm \
      {{workdir}}/.next \
      {{workdir}}/dist \
      {{workdir}}/build \
      {{workdir}}/out \
      {{workdir}}/.swc \
      {{workdir}}/.cache \
      {{workdir}}/.turbo \
      /pnpm/store && \
    chown -R app:app {{workdir}} /pnpm

ENV PNPM_HOME=/pnpm
ENV PNPM_STORE_DIR=/pnpm/store

WORKDIR {{workdir}}

COPY --chown=app:app . .

USER app
RUN {{pm_bin}} install --frozen-lockfile

ENTRYPOINT ["tini","--"]
"#;

const DOCKER_COMPOSE_TEMPLATE: &str = r#"# ============================================================
# {{project}} - Local Development
# ============================================================
# Generated by `airis generate files` - DO NOT EDIT MANUALLY
# Source of truth: manifest.toml
#
# airis up = local development only (always hot-reload).
# Production deploys via GitOps - this file is never used there.
# ============================================================

x-app-base: &app-base
  image: {{project}}-base
  working_dir: {{workdir}}
  deploy:
    replicas: 1
  volumes:
    - ./:{{workdir}}:delegated
{{#each workspace_volumes}}
    - {{this}}
{{/each}}
  extra_hosts:
    - "host.docker.internal:host-gateway"
  environment:
    DOCKER_ENV: "true"
    NODE_ENV: development
    PNPM_HOME: /pnpm
    PNPM_STORE_DIR: /pnpm/store
    CHOKIDAR_USEPOLLING: "true"
    WATCHPACK_POLLING: "true"

services:
{{#each services}}
  {{name}}:
{{#if extends}}
    <<: *{{extends}}
{{/if}}
{{#if container_name}}
    container_name: {{container_name}}
{{/if}}
{{#unless extends}}
    image: {{image}}
{{/unless}}
{{#if working_dir}}
{{#unless extends}}
    working_dir: {{working_dir}}
{{/unless}}
{{/if}}
{{#if deploy}}
{{#unless extends}}
    deploy:
      replicas: {{deploy.replicas}}
{{/unless}}
{{/if}}
{{#if command}}
    command: {{{command}}}
{{/if}}
{{#if profiles}}
    profiles:
{{#each profiles}}
      - "{{this}}"
{{/each}}
{{/if}}
{{#if depends_on}}
    depends_on:
{{#each depends_on}}
      - {{this}}
{{/each}}
{{/if}}
{{#if ports}}
    ports:
{{#each ports}}
      - "{{this}}"
{{/each}}
{{else if port}}
    ports:
      - "{{port}}:{{port}}"
{{/if}}
{{#if extra_hosts}}
{{#unless extends}}
    extra_hosts:
{{#each extra_hosts}}
      - "{{this}}"
{{/each}}
{{/unless}}
{{/if}}
{{#if env}}
    environment:
{{#each env}}
      {{@key}}: "{{this}}"
{{/each}}
{{/if}}
{{#if volumes}}
    volumes:
{{#each volumes}}
      - {{this}}
{{/each}}
{{/if}}
{{#if shm_size}}
    shm_size: "{{shm_size}}"
{{/if}}
{{#if restart}}
    restart: {{restart}}
{{/if}}
{{#if watch}}
    develop:
      watch:
{{#each watch}}
        - path: {{path}}
          action: {{action}}
          target: {{target}}
{{#if initial_sync}}
          initial_sync: true
{{/if}}
{{#if ignore}}
          ignore:
{{#each ignore}}
            - {{this}}
{{/each}}
{{/if}}
{{/each}}
{{/if}}

{{/each}}

networks:
  default:
    name: {{project}}_default
    external: {{default_external}}
  traefik:
    name: traefik_default
    external: true
{{#if proxy_network}}
  {{proxy_network}}:
    external: true
{{/if}}

volumes:
{{#each volume_names}}
  {{this}}:
{{/each}}
"#;

const CI_YML_TEMPLATE: &str = r#"# Auto-generated by airis init
# DO NOT EDIT - change manifest.toml instead.

name: CI

on:
  push:
    branches:
      - {{source_branch}}
  pull_request:
    branches:
      - {{target_branch}}

concurrency:
  group: $\{{github.workflow}}-$\{{github.ref}}
  cancel-in-progress: {{concurrency_cancel}}

jobs:
  test:
{{#if is_rust_project}}
    runs-on: macos-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1

      - name: Run tests
        run: cargo test

      - name: Build release
        run: cargo build --release
{{else}}
    runs-on: {{runner}}
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup pnpm
        uses: pnpm/action-setup@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '{{node_version}}'
{{#if cache}}
          cache: 'pnpm'
{{/if}}

{{#if has_pnpm_store_path}}
      - name: Configure pnpm store
        run: pnpm config set store-dir {{pnpm_store_path}}

{{/if}}
      - name: Install dependencies
        run: pnpm install --frozen-lockfile

{{#if affected}}
      - name: Lint (affected)
        run: pnpm turbo run lint --affected

      - name: Typecheck (affected)
        run: pnpm turbo run typecheck --affected

      - name: Build (affected)
        run: pnpm turbo run build --affected
{{else}}
      - name: Lint
        run: pnpm lint:check

      - name: Typecheck
        run: pnpm typecheck

      - name: Build
        run: pnpm build
{{/if}}
{{/if}}

{{#if auto_merge_enabled}}
  merge-to-{{target_branch}}:
    needs: test
    if: github.ref == 'refs/heads/{{source_branch}}' && github.event_name == 'push'
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 0
          token: $\{{secrets.GITHUB_TOKEN}}

      - name: Configure git
        run: |
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"

      - name: Merge {{source_branch}} to {{target_branch}}
        run: |
          git fetch origin {{target_branch}}
          git checkout {{target_branch}}
          git merge origin/{{source_branch}} --no-edit
          git push origin {{target_branch}}

          echo "✅ Merged {{source_branch}} → {{target_branch}}"
{{/if}}
"#;

const RELEASE_YML_TEMPLATE: &str = r##"# Auto-generated by airis init
# DO NOT EDIT - change manifest.toml instead.

name: Release to Homebrew

on:
  push:
    branches:
      - {{target_branch}}
  workflow_dispatch:

jobs:
  release:
{{#if is_rust_project}}
    runs-on: macos-latest
{{else}}
    runs-on: ubuntu-latest
{{/if}}
    permissions:
      contents: write
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

{{#if is_rust_project}}
      - name: Setup Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1
{{else}}
      - name: Setup pnpm
        uses: pnpm/action-setup@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: 'pnpm'
{{/if}}

{{#if is_rust_project}}
      - name: Read version from Cargo.toml
        id: version
        run: |
          # Read version from Cargo.toml (source of truth)
          VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/version = "\(.*\)"/\1/')
          echo "📦 Version from Cargo.toml: $VERSION"
          echo "version=$VERSION" >> $GITHUB_OUTPUT
{{else}}
      - name: Read version from package.json
        id: version
        run: |
          # Read version from package.json (source of truth)
          VERSION=$(node -p "require('./package.json').version")
          echo "📦 Version from package.json: $VERSION"
          echo "version=$VERSION" >> $GITHUB_OUTPUT
{{/if}}

      - name: Check if already released
        id: check_tag
        run: |
          if git rev-parse "v$\{{steps.version.outputs.version}}" >/dev/null 2>&1; then
            echo "exists=true" >> $GITHUB_OUTPUT
            echo "⚠️  Tag v$\{{steps.version.outputs.version}} already exists, skipping"
          else
            echo "exists=false" >> $GITHUB_OUTPUT
            echo "✅ Will create release v$\{{steps.version.outputs.version}}"
          fi

{{#if is_rust_project}}
      - name: Detect architecture
        if: steps.check_tag.outputs.exists == 'false'
        id: arch
        run: |
          ARCH=$(uname -m)
          if [ "$ARCH" = "arm64" ]; then
            echo "arch=aarch64-apple-darwin" >> $GITHUB_OUTPUT
          else
            echo "arch=x86_64-apple-darwin" >> $GITHUB_OUTPUT
          fi
          echo "📦 Architecture: $ARCH"

      - name: Create version tag for release build
        if: steps.check_tag.outputs.exists == 'false'
        run: |
          VERSION=$\{{steps.version.outputs.version}}
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git tag "v${VERSION}"
          git push origin "v${VERSION}"
          echo "✅ Created and pushed tag v${VERSION}"

      - name: Build release binary
        if: steps.check_tag.outputs.exists == 'false'
        run: |
          cargo build --release
          strip target/release/{{binary_name}}
          tar -czf {{binary_name}}-$\{{steps.version.outputs.version}}-$\{{steps.arch.outputs.arch}}.tar.gz -C target/release {{binary_name}}

      - name: Calculate SHA256
        if: steps.check_tag.outputs.exists == 'false'
        id: sha256
        run: |
          SHA256=$(shasum -a 256 {{binary_name}}-$\{{steps.version.outputs.version}}-$\{{steps.arch.outputs.arch}}.tar.gz | awk '{print $1}')
          echo "sha256=$SHA256" >> $GITHUB_OUTPUT
          echo "🔐 SHA256: $SHA256"

      - name: Create GitHub Release
        if: steps.check_tag.outputs.exists == 'false'
        env:
          GITHUB_TOKEN: $\{{secrets.GITHUB_TOKEN}}
        run: |
          VERSION=$\{{steps.version.outputs.version}}
          ARCH=$\{{steps.arch.outputs.arch}}

          echo "🚀 Creating GitHub Release v${VERSION}..."

          gh release create "v${VERSION}" \
            --title "Release v${VERSION}" \
            --generate-notes \
            "{{binary_name}}-${VERSION}-${ARCH}.tar.gz"

          echo "✅ Release v${VERSION} created successfully!"

{{#if has_homebrew}}
      - name: Update Homebrew formula
        if: steps.check_tag.outputs.exists == 'false'
        env:
          HOMEBREW_TAP_TOKEN: $\{{secrets.HOMEBREW_TAP_TOKEN}}
        run: |
          set -e

          VERSION=$\{{steps.version.outputs.version}}
          SHA256=$\{{steps.sha256.outputs.sha256}}
          ARCH=$\{{steps.arch.outputs.arch}}

          echo "📦 Updating Homebrew formula..."
          echo "   Version: $VERSION"
          echo "   SHA256: $SHA256"
          echo "   Arch: $ARCH"

          # Clone homebrew-tap repository
          git clone https://$HOMEBREW_TAP_TOKEN@github.com/{{homebrew_tap}}.git
          cd $(basename {{homebrew_tap}})

          # Ensure we're on main branch
          git checkout main || git checkout -b main

          # Create Formula directory if it doesn't exist
          mkdir -p Formula

          # Update formula - build with echo to avoid YAML parsing issues
          {
            echo 'class {{formula_class}} < Formula'
            echo '  desc "{{description}}"'
            echo '  homepage "https://github.com/{{repository}}"'
            echo '  license "MIT"'
            echo "  url \"https://github.com/{{repository}}/releases/download/v${VERSION}/{{binary_name}}-${VERSION}-${ARCH}.tar.gz\""
            echo "  sha256 \"${SHA256}\""
            echo "  version \"${VERSION}\""
            echo ''
            echo '  # Docker backend is required - this is a Docker-first tool'
            echo '  on_arm do'
            echo '    depends_on cask: "orbstack"'
            echo '  end'
            echo ''
            echo '  on_intel do'
            echo '    depends_on cask: "docker"'
            echo '  end'
            echo ''
            echo '  def install'
            echo '    bin.install "{{binary_name}}"'
            echo '  end'
            echo ''
            echo '  def caveats'
            echo '    <<~EOS'
            echo '      Make sure your Docker backend is running before using {{binary_name}}:'
            echo '        - Apple Silicon: OrbStack (installed as dependency)'
            echo '        - Intel Mac: Docker Desktop (installed as dependency)'
            echo '    EOS'
            echo '  end'
            echo ''
            echo '  test do'
            echo '    system "#{bin}/{{binary_name}}", "--version"'
            echo '  end'
            echo 'end'
          } > Formula/{{project_id}}.rb

          # Commit and push
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git add Formula/{{project_id}}.rb
          git commit -m "Update {{project_id}} to v${VERSION}" || echo "No changes to commit"
          git push origin main || echo "Push failed, check if token has permissions"

          echo "✅ Homebrew formula updated to v${VERSION}"
{{/if}}
{{else}}
      - name: Install dependencies
        if: steps.check_tag.outputs.exists == 'false'
        run: pnpm install

      - name: Build
        if: steps.check_tag.outputs.exists == 'false'
        run: pnpm build

      - name: Create GitHub Release
        if: steps.check_tag.outputs.exists == 'false'
        env:
          GITHUB_TOKEN: $\{{secrets.GITHUB_TOKEN}}
        run: |
          VERSION=$\{{steps.version.outputs.version}}

          echo "🚀 Creating GitHub Release v${VERSION}..."

          gh release create "v${VERSION}" \
            --title "Release v${VERSION}" \
            --generate-notes

          echo "✅ Release v${VERSION} created successfully!"
{{/if}}
"##;

// Note: CARGO_TOML_TEMPLATE removed - Cargo.toml is source of truth for Rust projects
// Use `airis bump-version` to sync versions between manifest.toml and Cargo.toml

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Manifest;

    fn minimal_manifest() -> Manifest {
        let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"
volumes = []

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
        toml::from_str(toml_str).unwrap()
    }

    #[test]
    fn test_compose_context_default_volumes() {
        let manifest = minimal_manifest();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest).unwrap();

        let workspace_volumes = context["workspace_volumes"].as_array().unwrap();
        let volume_names = context["volume_names"].as_array().unwrap();

        // Should have 10 default volumes (8 original + pnpm_virtual + pnpm_store)
        assert_eq!(workspace_volumes.len(), 10);
        assert_eq!(volume_names.len(), 10);

        // Check default volume format
        assert_eq!(workspace_volumes[0], "node_modules:/app/node_modules");
        assert_eq!(workspace_volumes[1], "pnpm_virtual:/app/.pnpm");
        assert_eq!(workspace_volumes[2], "pnpm_store:/pnpm/store");
        assert_eq!(workspace_volumes[3], "next_build:/app/.next");

        // Check volume names extraction
        assert_eq!(volume_names[0], "node_modules");
        assert_eq!(volume_names[1], "pnpm_virtual");
    }

    #[test]
    fn test_compose_context_no_workspace_service() {
        let manifest = minimal_manifest();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest).unwrap();

        // workspace_service and workspace_env should not exist
        assert!(context.get("workspace_service").is_none());
        assert!(context.get("workspace_env").is_none());
    }

    #[test]
    fn test_dockerfile_includes_install() {
        let manifest = minimal_manifest();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_dockerfile(&manifest).unwrap();

        // Dockerfile should contain install step
        assert!(result.contains("RUN pnpm install --frozen-lockfile"));
        // Should NOT contain sleep infinity
        assert!(!result.contains("sleep infinity"));
        // Should contain COPY
        assert!(result.contains("COPY --chown=app:app . ."));
    }

    #[test]
    fn test_dockerfile_uses_correct_pm_bin() {
        let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"
package_manager = "bun@1.2.0"
volumes = []

[commands]
dev = "bun dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_dockerfile(&manifest).unwrap();

        assert!(result.contains("RUN bun install --frozen-lockfile"));
    }

    #[test]
    fn test_compose_no_workspace_service_block() {
        let manifest = minimal_manifest();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_docker_compose(&manifest).unwrap();

        // Should NOT contain workspace service definition
        assert!(!result.contains("command: sleep infinity"));
        assert!(!result.contains("healthcheck:"));
        // Should still contain x-app-base anchor
        assert!(result.contains("x-app-base: &app-base"));
    }

    #[test]
    fn test_render_npmrc() {
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_npmrc().unwrap();
        assert!(result.contains("store-dir=/pnpm/store"));
        assert!(result.contains("virtual-store-dir=.pnpm"));
        assert!(result.contains("DO NOT EDIT"));
    }

    #[test]
    fn test_compose_context_custom_volumes() {
        let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["custom_vol:/app/custom", "data_vol:/app/data"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest).unwrap();

        let workspace_volumes = context["workspace_volumes"].as_array().unwrap();
        let volume_names = context["volume_names"].as_array().unwrap();

        // Should use custom volumes, not defaults
        assert_eq!(workspace_volumes.len(), 2);
        assert_eq!(volume_names.len(), 2);

        assert_eq!(workspace_volumes[0], "custom_vol:/app/custom");
        assert_eq!(workspace_volumes[1], "data_vol:/app/data");

        assert_eq!(volume_names[0], "custom_vol");
        assert_eq!(volume_names[1], "data_vol");
    }

    #[test]
    fn test_compose_template_renders_volumes() {
        let manifest = minimal_manifest();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_docker_compose(&manifest).unwrap();

        // Should contain volume mounts in services section
        assert!(result.contains("- node_modules:/app/node_modules"));
        assert!(result.contains("- pnpm_virtual:/app/.pnpm"));
        assert!(result.contains("- pnpm_store:/pnpm/store"));
        assert!(result.contains("- next_build:/app/.next"));

        // Should contain volume declarations
        assert!(result.contains("volumes:"));
        assert!(result.contains("  node_modules:"));
        assert!(result.contains("  pnpm_virtual:"));
        assert!(result.contains("  pnpm_store:"));
        assert!(result.contains("  next_build:"));
    }

    #[test]
    fn test_compose_template_renders_custom_volumes() {
        let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["my_cache:/app/.cache"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_docker_compose(&manifest).unwrap();

        // Should contain custom volume mount
        assert!(result.contains("- my_cache:/app/.cache"));

        // Should NOT contain default volumes
        assert!(!result.contains("- node_modules:/app/node_modules"));

        // Should declare custom volume
        assert!(result.contains("  my_cache:"));
    }

    #[test]
    fn test_compose_context_different_workdir() {
        let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/workspace/app"
volumes = []

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest).unwrap();

        let workspace_volumes = context["workspace_volumes"].as_array().unwrap();

        // Should use the custom workdir in paths
        assert_eq!(workspace_volumes[0], "node_modules:/workspace/app/node_modules");
        assert_eq!(workspace_volumes[1], "pnpm_virtual:/workspace/app/.pnpm");
        assert_eq!(workspace_volumes[2], "pnpm_store:/pnpm/store");
        assert_eq!(workspace_volumes[3], "next_build:/workspace/app/.next");
    }

    #[test]
    fn test_compose_context_volume_with_mode() {
        // Volumes can have :ro or :rw suffix (e.g., "vol:/path:ro")
        let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["config_vol:/app/config:ro", "data_vol:/app/data:rw"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest).unwrap();

        let volume_names = context["volume_names"].as_array().unwrap();

        // Should extract only the volume name (before first colon)
        assert_eq!(volume_names.len(), 2);
        assert_eq!(volume_names[0], "config_vol");
        assert_eq!(volume_names[1], "data_vol");
    }

    #[test]
    fn test_compose_context_malformed_volume_no_colon() {
        // Edge case: volume without colon should still work
        let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["just_a_name"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest).unwrap();

        let workspace_volumes = context["workspace_volumes"].as_array().unwrap();
        let volume_names = context["volume_names"].as_array().unwrap();

        // Should handle gracefully - volume is passed through
        assert_eq!(workspace_volumes.len(), 1);
        assert_eq!(workspace_volumes[0], "just_a_name");

        // Volume name extraction should still work (takes everything before colon, or whole string)
        assert_eq!(volume_names.len(), 1);
        assert_eq!(volume_names[0], "just_a_name");
    }

    #[test]
    fn test_compose_context_empty_string_volume() {
        // Edge case: empty string in volumes array
        let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["", "valid_vol:/app/valid"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest).unwrap();

        let workspace_volumes = context["workspace_volumes"].as_array().unwrap();

        // Should include both (even empty string)
        assert_eq!(workspace_volumes.len(), 2);
    }

    #[test]
    fn test_render_env_example_with_required() {
        let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]

[env]
required = ["DATABASE_URL", "API_KEY"]
optional = ["SENTRY_DSN"]

[env.validation.DATABASE_URL]
pattern = "^postgresql://"
description = "PostgreSQL connection string"
example = "postgresql://user:pass@localhost:5432/db"

[env.validation.API_KEY]
description = "API authentication key"
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_env_example(&manifest).unwrap();

        // Should contain header
        assert!(result.contains("# Auto-generated by airis init"));

        // Should contain required vars section
        assert!(result.contains("# Required environment variables"));
        assert!(result.contains("DATABASE_URL=postgresql://user:pass@localhost:5432/db"));
        assert!(result.contains("API_KEY=your_value_here"));

        // Should contain description as comment
        assert!(result.contains("# PostgreSQL connection string"));

        // Should contain optional vars section
        assert!(result.contains("# Optional environment variables"));
        assert!(result.contains("# SENTRY_DSN="));
    }

    #[test]
    fn test_render_env_example_empty() {
        let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_env_example(&manifest).unwrap();

        // Should only contain header when no env vars defined
        assert!(result.contains("# Auto-generated by airis init"));
        assert!(!result.contains("# Required environment variables"));
        assert!(!result.contains("# Optional environment variables"));
    }

    #[test]
    fn test_render_envrc() {
        let toml_str = r#"
[workspace]
name = "my-awesome-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_envrc(&manifest).unwrap();

        // Should contain header comment
        assert!(result.contains("# Auto-generated by airis init"));
        assert!(result.contains("# Enable with: direnv allow"));

        // Should add .airis/bin to PATH
        assert!(result.contains("export PATH=\"$PWD/.airis/bin:$PATH\""));

        // Should set COMPOSE_PROFILES
        assert!(result.contains("export COMPOSE_PROFILES=\"${COMPOSE_PROFILES:-shell,web}\""));

        // Should set COMPOSE_PROJECT_NAME from workspace name
        assert!(result.contains("export COMPOSE_PROJECT_NAME=\"my-awesome-project\""));
    }

    #[test]
    fn test_resolve_dependencies_catalog_with_colon() {
        let mut deps = IndexMap::new();
        deps.insert("react".to_string(), "catalog:".to_string());
        deps.insert("typescript".to_string(), "^5.0.0".to_string());

        let mut catalog = IndexMap::new();
        catalog.insert("react".to_string(), "^19.2.0".to_string());

        let result = resolve_dependencies(&deps, &catalog).unwrap();

        assert_eq!(result.get("react").unwrap(), "^19.2.0");
        assert_eq!(result.get("typescript").unwrap(), "^5.0.0");
    }

    #[test]
    fn test_resolve_dependencies_catalog_with_key() {
        let mut deps = IndexMap::new();
        deps.insert("my-react".to_string(), "catalog:react".to_string());

        let mut catalog = IndexMap::new();
        catalog.insert("react".to_string(), "^19.2.0".to_string());

        let result = resolve_dependencies(&deps, &catalog).unwrap();

        assert_eq!(result.get("my-react").unwrap(), "^19.2.0");
    }

    #[test]
    fn test_workspace_node_modules_volumes() {
        let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["node_modules:/app/node_modules"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]

[apps.corporate]

[apps.dashboard]
path = "apps/dashboard"

[libs.ui]

[libs.logger]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest).unwrap();

        let workspace_volumes = context["workspace_volumes"].as_array().unwrap();
        let volume_names = context["volume_names"].as_array().unwrap();

        // 1 explicit + 4 auto-generated workspace node_modules
        assert_eq!(workspace_volumes.len(), 5);

        // Check auto-generated volume names and mount paths
        let vol_strs: Vec<String> = workspace_volumes
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        assert!(vol_strs.contains(&"ws_nm_apps_corporate:/app/apps/corporate/node_modules".to_string()));
        assert!(vol_strs.contains(&"ws_nm_apps_dashboard:/app/apps/dashboard/node_modules".to_string()));
        assert!(vol_strs.contains(&"ws_nm_libs_ui:/app/libs/ui/node_modules".to_string()));
        assert!(vol_strs.contains(&"ws_nm_libs_logger:/app/libs/logger/node_modules".to_string()));

        // Volume names should include all
        let name_strs: Vec<String> = volume_names
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        assert!(name_strs.contains(&"ws_nm_apps_corporate".to_string()));
        assert!(name_strs.contains(&"ws_nm_libs_ui".to_string()));
    }

    #[test]
    fn test_workspace_node_modules_no_duplicates() {
        // If user already defines a workspace node_modules volume, don't duplicate it
        let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = ["node_modules:/app/node_modules", "custom_nm:/app/apps/corporate/node_modules"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]

[apps.corporate]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest).unwrap();

        let workspace_volumes = context["workspace_volumes"].as_array().unwrap();

        // Should not add a second volume for apps/corporate/node_modules
        let corporate_nm_count = workspace_volumes
            .iter()
            .filter(|v| v.as_str().unwrap().contains("apps/corporate/node_modules"))
            .count();
        assert_eq!(corporate_nm_count, 1);
    }

    #[test]
    fn test_compose_context_default_volumes_with_apps() {
        // Default volumes (empty volumes array) + apps should auto-add workspace node_modules
        let toml_str = r#"
[workspace]
name = "test-project"
service = "workspace"
image = "node:22-alpine"
workdir = "/app"
volumes = []

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "libs/*"]

[apps.web]

[libs.shared]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest).unwrap();

        let workspace_volumes = context["workspace_volumes"].as_array().unwrap();

        // 10 defaults + 2 workspace node_modules
        assert_eq!(workspace_volumes.len(), 12);

        let vol_strs: Vec<String> = workspace_volumes
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        assert!(vol_strs.contains(&"ws_nm_apps_web:/app/apps/web/node_modules".to_string()));
        assert!(vol_strs.contains(&"ws_nm_libs_shared:/app/libs/shared/node_modules".to_string()));
    }

}
