use anyhow::{Context, Result};
use handlebars::Handlebars;
use indexmap::IndexMap;
use serde_json::json;

use crate::manifest::{MANIFEST_FILE, Manifest};

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

        Ok(TemplateEngine { hbs })
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
    @just --list

up:
    @echo "🚀 Starting workspace + infra from {{manifest}}..."
    docker compose up -d
    just dev-all

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
    @echo "  just dev-all         # manifest.toml のアプリを起動"
    @echo "  just workspace       # workspace コンテナに入る"
    @exit 1

pnpm *args:
    @just guard pnpm

npm *args:
    @just guard npm

yarn *args:
    @just guard yarn
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
    container_name: {{workspace_service}}
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
