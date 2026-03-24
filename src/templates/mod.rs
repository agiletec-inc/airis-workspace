use anyhow::{Context, Result};
use handlebars::Handlebars;
use indexmap::IndexMap;
use serde_json::json;
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
                .with_context(|| format!(
                    "'{}' uses catalog: but is not defined in [packages.catalog]",
                    package
                ))?
        } else if let Some(catalog_key) = version_spec.strip_prefix("catalog:") {
            // "catalog:key" → look up specific key
            resolved_catalog
                .get(catalog_key)
                .cloned()
                .with_context(|| format!(
                    "'{}' references catalog key '{}' which is not defined in [packages.catalog]",
                    package, catalog_key
                ))?
        } else if version_spec == "latest" || version_spec == "lts" {
            // Resolve from npm registry
            resolve_version(package, version_spec)
                .with_context(|| format!(
                    "Failed to resolve {} for '{}' from npm registry",
                    version_spec, package
                ))?
        } else {
            // Use as-is (specific version)
            version_spec.clone()
        };

        resolved.insert(package.clone(), resolved_version);
    }

    Ok(resolved)
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
        hbs.register_template_string("service_dockerfile", SERVICE_DOCKERFILE_TEMPLATE)?;

        Ok(TemplateEngine { hbs })
    }

    /// Render a production Dockerfile for a service using turbo prune pattern.
    pub fn render_service_dockerfile(
        &self,
        app: &crate::manifest::ProjectDefinition,
        pnpm_version: &str,
    ) -> Result<String> {
        let deploy = app.deploy.as_ref()
            .context("deploy config is required for service Dockerfile generation")?;

        let framework = app.framework.as_deref().unwrap_or("node");
        let variant = deploy.variant.as_deref().unwrap_or(match framework {
            "nextjs" => "nextjs",
            _ => "node",
        });
        let path = app.path.as_deref().unwrap_or(&app.name);
        let scope = app.scope.as_deref().unwrap_or("@agiletec");
        let port = deploy.port.unwrap_or(3000);

        let entrypoint = deploy.entrypoint.clone().unwrap_or_else(|| {
            match variant {
                "nextjs" => format!("{}/server.js", path),
                _ => format!("{}/dist/index.js", path),
            }
        });

        // Pre-compute build arg lines to avoid Handlebars brace escaping issues
        let build_args_lines: Vec<String> = deploy.build_args.iter()
            .flat_map(|arg| vec![
                format!("ARG {}", arg),
                format!("ENV {}=${{{}}}", arg, arg),
            ])
            .collect();

        let data = json!({
            "scope": scope,
            "name": app.name,
            "path": path,
            "variant": variant,
            "is_nextjs": variant == "nextjs",
            "is_node": variant == "node" || variant == "worker",
            "is_worker": variant == "worker",
            "pnpm_version": pnpm_version,
            "port": port,
            "entrypoint": entrypoint,
            "health_path": deploy.health_path,
            "health_interval": deploy.health_interval,
            "build_args_lines": build_args_lines,
            "extra_apk": deploy.extra_apk,
        });

        self.hbs
            .render("service_dockerfile", &data)
            .context("Failed to render service Dockerfile")
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
        let data = self.prepare_docker_compose_data(manifest, ".")?;
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

    fn prepare_docker_compose_data(&self, manifest: &Manifest, root: &str) -> Result<serde_json::Value> {
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

        // Auto-generate artifact volumes for each workspace (apps/libs/products/...)
        // This prevents container-generated artifacts from leaking to the host via bind mount
        // Source of truth: [workspace.clean] — recursive dirs + clean dirs
        let mut artifact_dirs: Vec<&str> = Vec::new();
        for d in &manifest.workspace.clean.recursive {
            artifact_dirs.push(d.as_str());
        }
        for d in &manifest.workspace.clean.dirs {
            // Skip file entries (e.g., "pnpm-lock.yaml") — has extension but doesn't start with dot
            if d.contains('.') && !d.starts_with('.') { continue; }
            // Skip duplicates already in recursive list
            if artifact_dirs.contains(&d.as_str()) { continue; }
            artifact_dirs.push(d.as_str());
        }
        let mut workspace_volumes = workspace_volumes;
        for ws_path in manifest.all_workspace_paths_in(root) {
            for artifact in &artifact_dirs {
                let safe_name = artifact.replace('.', "");
                let vol_name = format!("ws_{}_{}", safe_name, ws_path.replace('/', "_"));
                let mount = format!("{}:{}/{}/{}", vol_name, workdir, ws_path, artifact);
                if !workspace_volumes.iter().any(|v| v.contains(&format!("{}/{}", ws_path, artifact))) {
                    workspace_volumes.push(mount);
                }
            }
        }

        // Build base volumes list (bind mount + workspace volumes) for x-app-base
        let mut base_volumes = vec![format!("./:{}:delegated", workdir)];
        base_volumes.extend(workspace_volumes.clone());

        // Build services, merging base volumes when a service uses extends + own volumes
        // YAML merge key (<<: *app-base) is overridden when a service defines its own volumes:
        // so we prepend base volumes to prevent the override from losing them.
        let services: Vec<serde_json::Value> = manifest
            .service
            .iter()
            .map(|(name, svc)| {
                let merged_volumes = if svc.extends.is_some() && !svc.volumes.is_empty() {
                    // Merge: base volumes first, then service-specific volumes
                    let mut merged = base_volumes.clone();
                    for v in &svc.volumes {
                        if !merged.contains(v) {
                            merged.push(v.clone());
                        }
                    }
                    merged
                } else {
                    svc.volumes.clone()
                };

                // Extract internal port: explicit port > ports mapping > default 3000
                let internal_port = svc.port.unwrap_or_else(|| {
                    svc.ports.first()
                        .and_then(|p| p.split(':').last())
                        .and_then(|p| p.parse::<u16>().ok())
                        .unwrap_or(3000)
                });

                json!({
                    "name": name,
                    "image": svc.image,
                    "port": internal_port,
                    "ports": svc.ports,
                    "command": svc.command,
                    "volumes": merged_volumes,
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
                    "devices": svc.devices,
                    "runtime": svc.runtime,
                    "gpu": svc.gpu,
                    "health_path": svc.health_path,
                    "network_mode": svc.network_mode,
                    "labels": svc.labels,
                    "networks": svc.networks,
                })
            })
            .collect();

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

        let network_defs = manifest
            .orchestration
            .networks
            .as_ref()
            .map(|n| &n.define)
            .filter(|d| !d.is_empty());

        Ok(json!({
            "project": manifest.workspace.name,
            "workspace_image": manifest.workspace.image,
            "workdir": manifest.workspace.workdir,
            "services": services,
            "proxy_network": proxy_network,
            "default_external": default_external,
            "workspace_volumes": workspace_volumes,
            "volume_names": volume_names,
            "network_defs": network_defs,
        }))
    }

    // Note: prepare_cargo_toml_data removed - Cargo.toml is source of truth for Rust projects
}

const NPMRC_TEMPLATE: &str = "\
# Auto-generated by airis init
# DO NOT EDIT — regenerate with: airis gen
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
# Generated by `airis gen` - DO NOT EDIT MANUALLY
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
{{#if gpu}}
    deploy:
      resources:
        reservations:
          devices:
            - driver: {{gpu.driver}}
              count: {{gpu.count}}
              capabilities: [{{#each gpu.capabilities}}{{this}}{{#unless @last}}, {{/unless}}{{/each}}]
{{else if deploy}}
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
{{#if runtime}}
    runtime: {{runtime}}
{{/if}}
{{#if devices}}
    devices:
{{#each devices}}
      - {{this}}
{{/each}}
{{/if}}
{{#if network_mode}}
    network_mode: {{network_mode}}
{{/if}}
{{#if labels}}
    labels:
{{#each labels}}
      - "{{this}}"
{{/each}}
{{/if}}
{{#if networks}}
    networks:
{{#each networks}}
      - {{this}}
{{/each}}
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
{{#if health_path}}
    healthcheck:
      test: ["CMD-SHELL", "node -e \"require('http').request({hostname:'localhost',port:{{port}},path:'{{health_path}}',timeout:5000},(r)=>{process.exit(r.statusCode===200?0:1)}).on('error',()=>process.exit(1)).end()\""]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
{{/if}}

{{/each}}

{{#if network_defs}}
networks:
  default:
    name: {{project}}_default
    external: {{default_external}}
{{#each network_defs}}
  {{@key}}:
    external: {{this.external}}
{{#if this.name}}
    name: {{this.name}}
{{/if}}
{{/each}}
{{else}}
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
{{/if}}

volumes:
{{#each volume_names}}
  {{this}}:
{{/each}}
"#;

// CI/CD workflows (ci.yml, release.yml) are project-owned — not generated.
// See git history for rationale.

const SERVICE_DOCKERFILE_TEMPLATE: &str = r#"# Auto-generated by airis gen
# DO NOT EDIT - change manifest.toml [app.deploy] instead.
#
# Variant: {{variant}} | Package: {{scope}}/{{name}}

# ============================================
# Base stage - pnpm environment setup
# ============================================
FROM node:24-alpine AS base
ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN apk add --no-cache libc6-compat{{#each extra_apk}} {{this}}{{/each}}
RUN corepack enable && corepack prepare pnpm@{{pnpm_version}} --activate

# ============================================
# Pruner stage - extract only needed packages
# ============================================
FROM base AS pruner
WORKDIR /app
RUN pnpm add -g turbo
COPY . .
RUN turbo prune {{scope}}/{{name}} --docker

# ============================================
# Builder stage - install deps and build
# ============================================
FROM base AS builder
WORKDIR /app

# Install dependencies from pruned lockfile
COPY --from=pruner /app/out/json/ .
RUN --mount=type=cache,id=pnpm,target=/pnpm/store pnpm install --frozen-lockfile

# Copy source code and build
COPY --from=pruner /app/out/full/ .
COPY --from=pruner /app/tsconfig.base.json ./
{{#each build_args_lines}}
{{{this}}}
{{/each}}
RUN pnpm turbo run build --filter={{scope}}/{{name}}
{{#if is_node}}
# Generate flat node_modules with pnpm deploy (resolves workspace symlink issues)
RUN pnpm deploy --legacy --filter={{scope}}/{{name}} --prod /app/deploy
{{/if}}

# ============================================
# Production stage - minimal runtime image
# ============================================
FROM node:24-alpine AS production
WORKDIR /app

RUN apk add --no-cache libc6-compat wget

{{#if is_nextjs}}
# Copy Next.js standalone output
COPY --from=builder /app/{{path}}/.next/standalone ./
COPY --from=builder /app/{{path}}/.next/static ./{{path}}/.next/static
COPY --from=builder /app/{{path}}/public ./{{path}}/public
{{else}}
# Copy built output and flat node_modules from pnpm deploy
COPY --from=builder /app/{{path}}/dist ./{{path}}/dist
COPY --from=builder /app/deploy/package.json ./{{path}}/
COPY --from=builder /app/deploy/node_modules ./{{path}}/node_modules
{{/if}}

# Create non-root user
RUN addgroup -g 1001 -S nodejs && adduser -S nodejs -u 1001
USER nodejs

ENV NODE_ENV=production
{{#unless is_worker}}
ENV PORT={{port}}

EXPOSE {{port}}

HEALTHCHECK --interval={{health_interval}} --timeout=10s --start-period=30s --retries=3 \
  CMD wget -q --spider http://localhost:{{port}}{{health_path}} || exit 1
{{/unless}}

CMD ["node", "{{entrypoint}}"]
"#;

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
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

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
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

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
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

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
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

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
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

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
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

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
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

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
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

        let workspace_volumes = context["workspace_volumes"].as_array().unwrap();
        let volume_names = context["volume_names"].as_array().unwrap();

        // 1 explicit + 4 workspaces × 10 artifact dirs
        assert_eq!(workspace_volumes.len(), 41);

        // Check auto-generated volume names and mount paths
        let vol_strs: Vec<String> = workspace_volumes
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        assert!(vol_strs.contains(&"ws_node_modules_apps_corporate:/app/apps/corporate/node_modules".to_string()));
        assert!(vol_strs.contains(&"ws_turbo_apps_corporate:/app/apps/corporate/.turbo".to_string()));
        assert!(vol_strs.contains(&"ws_dist_apps_corporate:/app/apps/corporate/dist".to_string()));
        assert!(vol_strs.contains(&"ws_next_apps_corporate:/app/apps/corporate/.next".to_string()));
        assert!(vol_strs.contains(&"ws_node_modules_apps_dashboard:/app/apps/dashboard/node_modules".to_string()));
        assert!(vol_strs.contains(&"ws_node_modules_libs_ui:/app/libs/ui/node_modules".to_string()));
        assert!(vol_strs.contains(&"ws_node_modules_libs_logger:/app/libs/logger/node_modules".to_string()));

        // Volume names should include all
        let name_strs: Vec<String> = volume_names
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        assert!(name_strs.contains(&"ws_node_modules_apps_corporate".to_string()));
        assert!(name_strs.contains(&"ws_turbo_libs_ui".to_string()));
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
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

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
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

        let workspace_volumes = context["workspace_volumes"].as_array().unwrap();

        // 10 defaults + 2 workspaces × 10 artifact dirs
        assert_eq!(workspace_volumes.len(), 30);

        let vol_strs: Vec<String> = workspace_volumes
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();

        assert!(vol_strs.contains(&"ws_node_modules_apps_web:/app/apps/web/node_modules".to_string()));
        assert!(vol_strs.contains(&"ws_turbo_apps_web:/app/apps/web/.turbo".to_string()));
        assert!(vol_strs.contains(&"ws_dist_apps_web:/app/apps/web/dist".to_string()));
        assert!(vol_strs.contains(&"ws_next_apps_web:/app/apps/web/.next".to_string()));
        assert!(vol_strs.contains(&"ws_node_modules_libs_shared:/app/libs/shared/node_modules".to_string()));
    }

    #[test]
    fn test_glob_expansion_adds_products_workspaces() {
        // Test that packages.workspaces glob patterns are expanded via filesystem
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Create directories matching "products/*" glob with package.json
        std::fs::create_dir_all(root.join("products/sales-agent")).unwrap();
        std::fs::write(root.join("products/sales-agent/package.json"), "{}").unwrap();
        std::fs::create_dir_all(root.join("products/bidalert")).unwrap();
        std::fs::write(root.join("products/bidalert/package.json"), "{}").unwrap();

        let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"
volumes = ["node_modules:/app/node_modules"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["products/*"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let paths = manifest.all_workspace_paths_in(root.to_str().unwrap());

        // Should contain the two products directories
        assert!(paths.contains(&"products/sales-agent".to_string()));
        assert!(paths.contains(&"products/bidalert".to_string()));
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_glob_expansion_skips_exclude_patterns() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("apps/web")).unwrap();
        std::fs::write(root.join("apps/web/package.json"), "{}").unwrap();

        let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = ["apps/*", "!apps/internal"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let paths = manifest.all_workspace_paths_in(root.to_str().unwrap());

        // Should contain apps/web from glob, exclude pattern should be skipped
        assert!(paths.contains(&"apps/web".to_string()));
        assert!(!paths.contains(&"!apps/internal".to_string()));
    }

    #[test]
    fn test_extends_with_volumes_merges_base_volumes() {
        // When a service uses extends + own volumes, base volumes should be included
        let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"
volumes = ["node_modules:/app/node_modules"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = []

[service.sales-agent]
image = "node:22-alpine"
extends = "app-base"
command = "pnpm dev"
volumes = ["sales_data:/app/data"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

        let services = context["services"].as_array().unwrap();
        let svc = &services[0];
        let volumes = svc["volumes"].as_array().unwrap();
        let vol_strs: Vec<String> = volumes.iter().map(|v| v.as_str().unwrap().to_string()).collect();

        // Should contain base bind mount
        assert!(vol_strs.contains(&"./:/app:delegated".to_string()));
        // Should contain base workspace volumes
        assert!(vol_strs.contains(&"node_modules:/app/node_modules".to_string()));
        // Should contain service-specific volume
        assert!(vol_strs.contains(&"sales_data:/app/data".to_string()));
    }

    #[test]
    fn test_extends_without_volumes_keeps_original() {
        // When a service uses extends but no own volumes, volumes should be empty (inherits from YAML merge)
        let toml_str = r#"
[workspace]
name = "test-project"
image = "node:22-alpine"
workdir = "/app"
volumes = ["node_modules:/app/node_modules"]

[commands]
dev = "pnpm dev"

[versioning]
strategy = "manual"

[packages]
workspaces = []

[service.frontend]
image = "node:22-alpine"
extends = "app-base"
command = "pnpm dev"
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let context = engine.prepare_docker_compose_data(&manifest, "/nonexistent").unwrap();

        let services = context["services"].as_array().unwrap();
        let svc = &services[0];
        let volumes = svc["volumes"].as_array().unwrap();

        // No own volumes → should be empty (YAML merge handles it)
        assert_eq!(volumes.len(), 0);
    }

    #[test]
    fn test_compose_infra_service() {
        let toml_str = r#"
version = 1
mode = "docker-first"
[workspace]
name = "infra-test"
workdir = "/app"

[service.tunnel]
image = "cloudflare/cloudflared:latest"
network_mode = "host"

[service.app]
image = "myapp:latest"
networks = ["default", "proxy"]
labels = [
  "traefik.enable=true",
  "traefik.http.routers.app.rule=Host(`app.example.com`)",
]

[orchestration.networks.define.proxy]
external = true
name = "proxy"
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_docker_compose(&manifest).unwrap();

        // network_mode
        assert!(result.contains("network_mode: host"), "missing network_mode");
        // labels
        assert!(result.contains("traefik.enable=true"), "missing labels");
        assert!(result.contains("traefik.http.routers.app.rule=Host(`app.example.com`)"), "missing router label");
        // service networks
        assert!(result.contains("- default"), "missing service network default");
        assert!(result.contains("- proxy"), "missing service network proxy");
        // top-level networks section (data-driven)
        assert!(result.contains("external: true"), "missing external in network_defs");
        assert!(result.contains("name: proxy"), "missing name in network_defs");
        // should NOT contain hardcoded traefik network
        assert!(!result.contains("traefik_default"), "should not have hardcoded traefik network");
    }

    #[test]
    fn test_compose_gpu_service() {
        let toml_str = r#"
version = 1
mode = "docker-first"
[workspace]
name = "gpu-test"
workdir = "/app"

[service.ml]
image = "nvidia/cuda:12.6"
runtime = "nvidia"
devices = ["/dev/dri:/dev/dri"]

[service.ml.gpu]
driver = "nvidia"
count = "all"
capabilities = ["gpu"]
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_docker_compose(&manifest).unwrap();

        assert!(result.contains("runtime: nvidia"), "missing runtime");
        assert!(
            result.contains("- /dev/dri:/dev/dri"),
            "missing devices"
        );
        assert!(result.contains("driver: nvidia"), "missing gpu driver");
        assert!(result.contains("count: all"), "missing gpu count");
        assert!(
            result.contains("capabilities: [gpu]"),
            "missing gpu capabilities"
        );
        // ml service should have deploy.resources, not deploy.replicas
        // (x-app-base may have replicas, but the service itself should not)
        let ml_section = result.split("  ml:").nth(1).unwrap();
        assert!(
            ml_section.contains("resources:"),
            "ml service should have deploy.resources"
        );
        assert!(
            !ml_section.contains("replicas:"),
            "ml service should not have replicas when gpu is set"
        );
    }

    #[test]
    fn test_compose_gpu_defaults() {
        let toml_str = r#"
version = 1
mode = "docker-first"
[workspace]
name = "gpu-test"
workdir = "/app"

[service.ml]
image = "nvidia/cuda:12.6"
gpu = {}
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let svc = &manifest.service["ml"];
        let gpu = svc.gpu.as_ref().unwrap();

        assert_eq!(gpu.driver, "nvidia");
        assert_eq!(gpu.count, "all");
        assert_eq!(gpu.capabilities, vec!["gpu".to_string()]);
    }

}
