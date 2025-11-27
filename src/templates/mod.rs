use anyhow::{Context, Result};
use handlebars::Handlebars;
use indexmap::IndexMap;
use serde_json::json;
use std::process::Command;

use crate::commands::sync_deps::resolve_version;
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
        hbs.register_template_string("dockerfile_dev", DOCKERFILE_DEV_TEMPLATE)?;
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
            .split(|c: char| c == '-' || c == '_')
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

    pub fn render_dockerfile_dev(&self, manifest: &Manifest) -> Result<String> {
        let data = self.prepare_dockerfile_dev_data(manifest)?;
        self.hbs
            .render("dockerfile_dev", &data)
            .context("Failed to render Dockerfile.dev")
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

    fn prepare_dockerfile_dev_data(&self, manifest: &Manifest) -> Result<serde_json::Value> {
        Ok(json!({
            "workspace_image": manifest.workspace.image,
            "workdir": manifest.workspace.workdir,
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
                    "command": svc.command,
                    "volumes": svc.volumes,
                    "env": svc.env,
                })
            })
            .collect();

        // Get proxy network from orchestration.networks config
        let proxy_network = manifest
            .orchestration
            .networks
            .as_ref()
            .and_then(|n| n.proxy.clone())
            .unwrap_or_else(|| "coolify".to_string());

        let default_external = manifest
            .orchestration
            .networks
            .as_ref()
            .map(|n| n.default_external)
            .unwrap_or(false);

        Ok(json!({
            "project": manifest.workspace.name,
            "workspace_service": manifest.workspace.service,
            "workspace_image": manifest.workspace.image,
            "workdir": manifest.workspace.workdir,
            "services": services,
            "proxy_network": proxy_network,
            "default_external": default_external,
        }))
    }

    // Note: prepare_cargo_toml_data removed - Cargo.toml is source of truth for Rust projects
}

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

const DOCKERFILE_DEV_TEMPLATE: &str = r#"FROM node:24-bookworm

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      build-essential \
      ca-certificates \
      git \
      curl \
      openssh-client \
      python3 \
      pkg-config \
      tini && \
    rm -rf /var/lib/apt/lists/*

# Ensure dedicated app user exists
RUN set -eux; \
    if ! id -u app >/dev/null 2>&1; then \
      useradd -m -s /bin/bash app; \
    fi; \
    chown -R app:app /home/app

# Pre-create common build directories to prevent root-owned creation by Docker volumes
RUN mkdir -p {{workdir}}/{node_modules,.next,dist,build,out,.swc,.cache,.turbo} && \
    chown -R app:app {{workdir}}

WORKDIR {{workdir}}
USER app

ENTRYPOINT ["tini","--"]
CMD ["sleep","infinity"]
"#;

const DOCKER_COMPOSE_TEMPLATE: &str = r#"# ============================================================
# {{project}} Workspace - Monorepo Dev Shell
# ============================================================
# Provides a single container with pnpm/node toolchain for running
# monorepo commands (`airis shell`, `airis install`, etc).
# ============================================================
# Generated by `airis init` - DO NOT EDIT MANUALLY
# Source of truth: manifest.toml
# ============================================================

services:
  {{workspace_service}}:
    container_name: {{project}}-{{workspace_service}}
    build:
      context: .
      dockerfile: Dockerfile.dev
    volumes:
      # ソースコード: :delegated で macOS I/O を最適化
      - ./:{{workdir}}:delegated
      # 依存・ビルド: named volume で完全隔離（bind mount しない）
      - node_modules:{{workdir}}/node_modules
      - next_build:{{workdir}}/.next
      - dist_build:{{workdir}}/dist
      - build_output:{{workdir}}/build
      - out_export:{{workdir}}/out
      - turbo_cache:{{workdir}}/.turbo
      - swc_cache:{{workdir}}/.swc
      - cache_dir:{{workdir}}/.cache
    working_dir: {{workdir}}
    expose:
      - "3000"
    environment:
      CHOKIDAR_USEPOLLING: "true"
      CHOKIDAR_INTERVAL: "200"
      WATCHPACK_POLLING: "true"
      NODE_ENV: development
    extra_hosts:
      - "host.docker.internal:host-gateway"
    command: sleep infinity
    networks:
      - default
    labels:
      - "traefik.enable=false"
    develop:
      watch:
        - path: .
          action: sync
          target: {{workdir}}
          ignore:
            - node_modules/
            - .next/
            - dist/
            - .turbo/

{{#each services}}
  {{name}}:
    image: {{image}}
{{#if port}}
    ports:
      - "{{port}}:{{port}}"
{{/if}}
{{#if command}}
    command: {{command}}
{{/if}}
{{#if volumes}}
    volumes:
{{#each volumes}}
      - {{this}}
{{/each}}
{{/if}}
{{#if env}}
    environment:
{{#each env}}
      {{@key}}: "{{this}}"
{{/each}}
{{/if}}

{{/each}}

networks:
  default:
    name: {{project}}_default
    external: {{default_external}}
  {{proxy_network}}:
    external: true

volumes:
  node_modules:   # ルートnode_modules（ホストに出ない）
  next_build:
  dist_build:
  build_output:
  out_export:
  turbo_cache:
  swc_cache:
  cache_dir:
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
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Setup pnpm
        uses: pnpm/action-setup@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '22'
          cache: 'pnpm'

      - name: Install dependencies
        run: pnpm install

      - name: Run tests
        run: pnpm test

      - name: Build
        run: pnpm build
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
          echo "✅ Created tag v${VERSION} for release build"

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
