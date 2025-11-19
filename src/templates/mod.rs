use anyhow::{Context, Result};
use handlebars::Handlebars;
use indexmap::IndexMap;
use serde_json::json;
use std::process::Command;

use crate::manifest::{MANIFEST_FILE, Manifest};

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

        hbs.register_template_string("justfile", JUSTFILE_TEMPLATE)?;
        hbs.register_template_string("package_json", PACKAGE_JSON_TEMPLATE)?;
        hbs.register_template_string("pnpm_workspace", PNPM_WORKSPACE_TEMPLATE)?;
        hbs.register_template_string("docker_compose", DOCKER_COMPOSE_TEMPLATE)?;
        hbs.register_template_string("ci_yml", CI_YML_TEMPLATE)?;
        hbs.register_template_string("release_yml", RELEASE_YML_TEMPLATE)?;

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

        // Convert binary_name to PascalCase for Ruby class name
        let formula_class = binary_name
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

    pub fn render_justfile(&self, manifest: &Manifest) -> Result<String> {
        let data = self.prepare_justfile_data(manifest)?;
        self.hbs
            .render("justfile", &data)
            .context("Failed to render justfile")
    }

    pub fn render_package_json(&self, manifest: &Manifest) -> Result<String> {
        let data = self.prepare_package_json_data(manifest)?;
        self.hbs
            .render("package_json", &data)
            .context("Failed to render package.json")
    }

    pub fn render_pnpm_workspace(
        &self,
        manifest: &Manifest,
        resolved_catalog: &IndexMap<String, String>,
    ) -> Result<String> {
        let data = self.prepare_pnpm_workspace_data(manifest, resolved_catalog)?;
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

    fn prepare_justfile_data(&self, manifest: &Manifest) -> Result<serde_json::Value> {
        Ok(json!({
            "project": manifest.workspace.name,
            "workspace": manifest.workspace.service,
            "manifest": MANIFEST_FILE,
            "has_verify_rule": manifest.rule.contains_key("verify"),
            "has_ci_rule": manifest.rule.contains_key("ci"),
        }))
    }

    fn prepare_package_json_data(&self, manifest: &Manifest) -> Result<serde_json::Value> {
        let root = &manifest.packages.root;
        Ok(json!({
            "name": manifest.workspace.name,
            "package_manager": manifest.workspace.package_manager,
            "dependencies": root.dependencies,
            "dev_dependencies": root.dev_dependencies,
            "optional_dependencies": root.optional_dependencies,
            "scripts": root.scripts,
            "engines": root.engines,
            "has_engines": !root.engines.is_empty(),
            "has_optional_deps": !root.optional_dependencies.is_empty(),
            "has_pnpm_config": !root.pnpm.overrides.is_empty()
                || !root.pnpm.peer_dependency_rules.ignore_missing.is_empty()
                || !root.pnpm.only_built_dependencies.is_empty()
                || !root.pnpm.allowed_scripts.is_empty(),
            "pnpm": root.pnpm,
        }))
    }

    fn prepare_pnpm_workspace_data(
        &self,
        manifest: &Manifest,
        resolved_catalog: &IndexMap<String, String>,
    ) -> Result<serde_json::Value> {
        let packages = if manifest.packages.workspaces.is_empty() {
            manifest
                .dev
                .autostart
                .iter()
                .map(|name| format!("apps/{}", name))
                .collect()
        } else {
            manifest.packages.workspaces.clone()
        };

        Ok(json!({
            "packages": packages,
            "catalog": resolved_catalog,
            "has_catalog": !resolved_catalog.is_empty(),
            "manifest": MANIFEST_FILE,
        }))
    }

    fn prepare_docker_compose_data(&self, manifest: &Manifest) -> Result<serde_json::Value> {
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

        let named_volumes: Vec<String> = manifest
            .workspace
            .volumes
            .iter()
            .filter_map(|mount| mount.split(':').next().map(|v| v.to_string()))
            .collect();

        Ok(json!({
            "project": manifest.workspace.name,
            "workspace_service": manifest.workspace.service,
            "workspace_image": manifest.workspace.image,
            "workdir": manifest.workspace.workdir,
            "extra_mounts": manifest.workspace.volumes,
            "depends_on": Vec::<String>::new(),
            "services": services,
            "named_volumes": named_volumes,
        }))
    }
}

const JUSTFILE_TEMPLATE: &str = r#"# Auto-generated by airis init
# DO NOT EDIT - change manifest.toml instead.

project := "{{project}}"
manifest := "{{manifest}}"
workspace := "{{workspace}}"

set shell := ["bash", "-c"]

default:
    @echo "Use 'airis <command>' instead of 'just <command>'"
    @echo ""
    @echo "Available commands:"
    @echo "  airis up       - Start Docker containers"
    @echo "  airis down     - Stop Docker containers"
    @echo "  airis dev      - Run dev servers"
    @echo "  airis build    - Build all apps"
    @echo "  airis test     - Run tests"
    @echo "  airis shell    - Enter workspace container"
    @echo "  airis install  - Install dependencies"

up:
    @echo "⚠️  Use 'airis up' instead"

down:
    @echo "🧹 Stopping containers..."
    docker compose down --remove-orphans

workspace:
    docker compose exec -it {{workspace}} sh

install:
    docker compose exec {{workspace}} pnpm install

logs:
    docker compose logs -f

ps:
    docker compose ps

clean:
    rm -rf ./node_modules ./dist ./.next ./build ./target
    find . -name ".DS_Store" -delete 2>/dev/null || true

dev-all:
	apps="$(airis manifest dev-apps)"
	if [ -z "$$apps" ]; then \
		echo "⚠️  No dev apps defined in manifest.toml (.dev.apps)"; \
		exit 0; \
	fi
	echo "$$apps" | while read -r app; do \
		[ -z "$$app" ] && continue; \
		echo "▶️  docker compose exec {{workspace}} pnpm --filter $$app dev"; \
		docker compose exec {{workspace}} pnpm --filter "$$app" dev & \
	done
	wait

{{#if has_verify_rule}}
verify:
	cmds="$(airis manifest rule verify)"
	if [ -z "$$cmds" ]; then \
		echo "⚠️  [rule.verify] is empty in manifest.toml"; \
		exit 1; \
	fi
	echo "$$cmds" | while read -r cmd; do \
		[ -z "$$cmd" ] && continue; \
		echo ">> $$cmd"; \
		eval "$$cmd"; \
	done
{{/if}}

{{#if has_ci_rule}}
ci:
	cmds="$(airis manifest rule ci)"
	if [ -z "$$cmds" ]; then \
		echo "⚠️  [rule.ci] is empty in manifest.toml"; \
		exit 1; \
	fi
	echo "$$cmds" | while read -r cmd; do \
		[ -z "$$cmd" ] && continue; \
		echo ">> $$cmd"; \
		eval "$$cmd"; \
	done
{{/if}}

[private]
guard tool:
    @echo "❌ ERROR: '\{{tool}}' は直接使えません"
    @echo ""
    @echo "Docker-first ルールに従い、以下を利用してください:"
    @echo "  airis dev      # 開発サーバーを起動"
    @echo "  airis shell    # workspace コンテナに入る"
    @exit 1

pnpm *args:
    @echo "❌ Use 'airis install' or 'airis shell' instead"
    @exit 1

npm *args:
    @echo "❌ Use 'airis install' or 'airis shell' instead"
    @exit 1

yarn *args:
    @echo "❌ Use 'airis install' or 'airis shell' instead"
    @exit 1
"#;

const PACKAGE_JSON_TEMPLATE: &str = r#"{
  "name": "{{name}}",
  "version": "0.0.0",
  "private": true,
  "type": "module",
{{#if has_engines}}
  "engines": {
{{#each engines}}
    "{{@key}}": "{{this}}"{{#unless @last}},{{/unless}}
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
    }{{#if pnpm.peer_dependency_rules.ignore_missing}},{{/if}}{{#if pnpm.only_built_dependencies}},{{/if}}{{#if pnpm.allowed_scripts}},{{/if}}
{{/if}}
{{#if pnpm.peer_dependency_rules.ignore_missing}}
    "peerDependencyRules": {
      "ignoreMissing": [
{{#each pnpm.peer_dependency_rules.ignore_missing}}
        "{{this}}"{{#unless @last}},{{/unless}}
{{/each}}
      ]{{#if pnpm.peer_dependency_rules.allowed_versions}},{{/if}}
{{#if pnpm.peer_dependency_rules.allowed_versions}}
      "allowedVersions": {
{{#each pnpm.peer_dependency_rules.allowed_versions}}
        "{{@key}}": "{{this}}"{{#unless @last}},{{/unless}}
{{/each}}
      }
{{/if}}
    }{{#if pnpm.only_built_dependencies}},{{/if}}{{#if pnpm.allowed_scripts}},{{/if}}
{{/if}}
{{#if pnpm.only_built_dependencies}}
    "onlyBuiltDependencies": [
{{#each pnpm.only_built_dependencies}}
      "{{this}}"{{#unless @last}},{{/unless}}
{{/each}}
    ]{{#if pnpm.allowed_scripts}},{{/if}}
{{/if}}
{{#if pnpm.allowed_scripts}}
    "allowedScripts": {
{{#each pnpm.allowed_scripts}}
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
# Catalog versions are resolved and written directly to package.json files.
# This file only defines workspace packages for pnpm/npm workspaces.

packages:
{{#each packages}}
  - "{{this}}"
{{/each}}
"#;

const DOCKER_COMPOSE_TEMPLATE: &str = r#"# Auto-generated by airis init
# DO NOT EDIT - change manifest.toml instead.

name: {{project}}

services:
  {{workspace_service}}:
    container_name: {{project}}-{{workspace_service}}
    image: {{workspace_image}}
    working_dir: {{workdir}}
    volumes:
      - ./:{{workdir}}
{{#each extra_mounts}}
      - {{this}}
{{/each}}
    command: sh -c "corepack enable && corepack prepare pnpm@latest --activate && sleep infinity"
    stdin_open: true
    tty: true
{{#if depends_on}}
    depends_on:
{{#each depends_on}}
      - {{this}}
{{/each}}
{{/if}}

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

{{#if named_volumes}}
volumes:
{{#each named_volumes}}
  {{this}}:
{{/each}}
{{/if}}
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

const RELEASE_YML_TEMPLATE: &str = r#"# Auto-generated by airis init
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

      - name: Determine version bump
        id: version
        run: |
          # Get highest version tag (sort semantically)
          LATEST_TAG=$(git tag -l 'v*' | sort -t. -k1,1n -k2,2n -k3,3n | tail -1)
          if [ -z "$LATEST_TAG" ]; then
            LATEST_TAG="v0.0.0"
          fi
          CURRENT_VERSION=${LATEST_TAG#v}

          echo "📦 Current version: $CURRENT_VERSION (from $LATEST_TAG)"

          # Parse semver
          IFS='.' read -r major minor patch <<< "$CURRENT_VERSION"

          # Get commit messages since last tag
          COMMITS=$(git log ${LATEST_TAG}..HEAD --pretty=format:"%s" 2>/dev/null || git log --pretty=format:"%s")

          # Determine bump type based on conventional commits
          BUMP_TYPE="patch"

          if echo "$COMMITS" | grep -qE '^(feat!:|fix!:|BREAKING CHANGE:)'; then
            BUMP_TYPE="major"
          elif echo "$COMMITS" | grep -qE '^feat(\(.+\))?:'; then
            BUMP_TYPE="minor"
          fi

          # Calculate new version
          if [ "$BUMP_TYPE" = "major" ]; then
            NEW_VERSION="$((major + 1)).0.0"
          elif [ "$BUMP_TYPE" = "minor" ]; then
            NEW_VERSION="${major}.$((minor + 1)).0"
          else
            NEW_VERSION="${major}.${minor}.$((patch + 1))"
          fi

          echo "version=$NEW_VERSION" >> $GITHUB_OUTPUT
          echo "bump_type=$BUMP_TYPE" >> $GITHUB_OUTPUT
          echo "🚀 Bumping $CURRENT_VERSION → $NEW_VERSION ($BUMP_TYPE)"

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

          # Update formula with OrbStack/Docker dependencies
          cat > Formula/{{binary_name}}.rb <<'EOF'
class {{formula_class}} < Formula
  desc "{{description}}"
  homepage "https://github.com/{{repository}}"
  license "MIT"
EOF

          cat >> Formula/{{binary_name}}.rb <<EOF
  url "https://github.com/{{repository}}/releases/download/v${VERSION}/{{binary_name}}-${VERSION}-${ARCH}.tar.gz"
  sha256 "${SHA256}"
  version "${VERSION}"
EOF

          cat >> Formula/{{binary_name}}.rb <<'EOF'

  # Docker backend is required - this is a Docker-first tool
  on_arm do
    depends_on cask: "orbstack"
  end

  on_intel do
    depends_on cask: "docker"
  end

  def install
    bin.install "{{binary_name}}"
  end

  def caveats
    <<~EOS
      Make sure your Docker backend is running before using {{binary_name}}:
        - Apple Silicon: OrbStack (installed as dependency)
        - Intel Mac: Docker Desktop (installed as dependency)
    EOS
  end

  test do
    system "\#{bin}/{{binary_name}}", "--version"
  end
end
EOF

          # Commit and push
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git add Formula/{{binary_name}}.rb
          git commit -m "Update {{binary_name}} to v${VERSION}" || echo "No changes to commit"
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
"#;
