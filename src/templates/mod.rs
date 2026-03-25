use anyhow::{Context, Result};
use handlebars::Handlebars;
use indexmap::IndexMap;
use serde_json::json;
use crate::version_resolver::resolve_all_action_versions;
use crate::manifest::{ActionsVersions, MANIFEST_FILE, Manifest};

/// Resolved GitHub Actions versions with full action references (e.g., "actions/checkout@v6")
struct ResolvedActions {
    checkout: String,
    pnpm: String,
    setup_node: String,
    cache: String,
    doppler: String,
}

impl ResolvedActions {
    fn from_manifest(actions: &ActionsVersions) -> Result<Self> {
        let resolved = resolve_all_action_versions(actions)?;
        Ok(ResolvedActions {
            checkout: format!("actions/checkout@{}", resolved.checkout),
            pnpm: format!("pnpm/action-setup@{}", resolved.pnpm),
            setup_node: format!("actions/setup-node@{}", resolved.setup_node),
            cache: format!("actions/cache@{}", resolved.cache),
            doppler: format!("dopplerhq/cli-action@{}", resolved.doppler),
        })
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
        let conventions = crate::conventions::framework_defaults(framework);
        let variant = deploy.variant.as_deref().unwrap_or(match framework {
            "nextjs" => "nextjs",
            _ => "node",
        });
        let path = app.path.as_deref().unwrap_or(&app.name);
        let scope = app.scope.as_deref().unwrap_or("@workspace");
        let port = deploy.port.unwrap_or(conventions.port);

        let entrypoint = deploy.entrypoint.clone().unwrap_or_else(|| {
            format!("{}/{}", path, conventions.entrypoint)
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

    /// Render root package.json in hybrid mode.
    ///
    /// Managed fields: name, version, private, type, packageManager, workspaces.
    /// All other fields (dependencies, scripts, engines, pnpm config) are user-managed.
    pub fn render_package_json(
        &self,
        manifest: &Manifest,
        _resolved_catalog: &IndexMap<String, String>,
    ) -> Result<String> {
        let managed_fields = ["name", "version", "private", "type", "packageManager", "workspaces"];

        // Read existing package.json if it exists
        let package_json_path = std::path::Path::new("package.json");
        let mut package_json = if package_json_path.exists() {
            let existing = std::fs::read_to_string(package_json_path)
                .context("Failed to read existing root package.json")?;
            serde_json::from_str::<serde_json::Value>(&existing)
                .context("Failed to parse existing root package.json")?
        } else {
            serde_json::json!({})
        };

        let obj = package_json.as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Root package.json is not a JSON object"))?;

        // Update only managed fields
        obj.insert("name".to_string(), serde_json::json!(manifest.workspace.name));
        obj.insert("version".to_string(), serde_json::json!("0.0.0"));
        obj.insert("private".to_string(), serde_json::json!(true));
        obj.insert("type".to_string(), serde_json::json!("module"));
        obj.insert("packageManager".to_string(), serde_json::json!(manifest.workspace.package_manager));

        if !manifest.packages.workspaces.is_empty() {
            obj.insert("workspaces".to_string(), serde_json::to_value(&manifest.packages.workspaces)?);
        }

        // Update generation marker
        obj.insert("_generated".to_string(), serde_json::json!({
            "by": "airis gen",
            "managed_fields": managed_fields,
            "warning": "Only the fields listed in managed_fields are updated by airis gen. Everything else is yours."
        }));

        // Serialize to pretty JSON
        let content = serde_json::to_string_pretty(&package_json)
            .context("Failed to serialize package.json")?;
        Ok(format!("{content}\n"))
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

    /// Generate .github/workflows/ci.yml from manifest v2
    pub fn render_ci_workflow(&self, manifest: &Manifest) -> Result<String> {
        // Infrastructure-only: no Node.js workspace
        if !manifest.has_workspace() {
            return self.render_infra_ci_workflow(manifest);
        }

        let ci = &manifest.ci;
        let a = ResolvedActions::from_manifest(&ci.actions)?;
        let checkout = &a.checkout;
        let pnpm_action = &a.pnpm;
        let setup_node = &a.setup_node;
        let node_version = manifest.node_version();
        let runner = ci.runner.as_deref().unwrap_or("ubuntu-latest");
        let affected_flag = if ci.affected { " --affected" } else { "" };

        // Runner YAML: "self-hosted, linux" → [self-hosted, linux]
        let runner_yaml = if runner.contains(',') {
            format!("[{}]", runner)
        } else {
            runner.to_string()
        };

        // pnpm store step
        let pnpm_store_step = if let Some(ref store_path) = ci.pnpm_store_path {
            format!(
                "      - name: Configure pnpm store\n        run: pnpm config set store-dir {}",
                store_path
            )
        } else {
            format!("      - name: Cache pnpm store\n        uses: {}\n        with:\n          path: ~/.pnpm-store\n          key: ${{{{ runner.os }}}}-pnpm-${{{{ hashFiles('pnpm-lock.yaml') }}}}\n          restore-keys: ${{{{ runner.os }}}}-pnpm-", a.cache)
        };

        // Determine CI branch and PR target from profiles
        let deploy_profiles = manifest.deploy_profiles();
        let ci_branch = deploy_profiles
            .iter()
            .find(|(name, p)| p.effective_role(name) == "staging")
            .map(|(_, p)| p.branch.as_deref().unwrap_or("stg"))
            .unwrap_or(&ci.auto_merge.from);
        let pr_target = deploy_profiles
            .iter()
            .find(|(name, p)| p.effective_role(name) == "production")
            .map(|(_, p)| p.branch.as_deref().unwrap_or("main"))
            .unwrap_or(&ci.auto_merge.to);

        let concurrency = if ci.concurrency_cancel {
            "\nconcurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}\n  cancel-in-progress: true\n"
        } else {
            ""
        };

        // Build CI job (same structure for lint, typecheck, test)
        let build_job = |task: &str, timeout: u8| -> String {
            format!(
                "  {}:\n    runs-on: {}\n    timeout-minutes: {}\n    steps:\n      - uses: {}\n        with:\n          fetch-depth: 2\n      - uses: {}\n      - uses: {}\n        with:\n          node-version: '{}'\n{}\n      - run: pnpm install --frozen-lockfile\n      - run: pnpm turbo run {}{}",
                task, runner_yaml, timeout, checkout, pnpm_action, setup_node, node_version, pnpm_store_step, task, affected_flag
            )
        };

        let job_blocks: Vec<String> = ci.jobs.iter()
            .map(|(task, timeout)| build_job(task, *timeout))
            .collect();

        Ok(format!(
            "# Auto-generated by airis gen — DO NOT EDIT\n# Change manifest.toml [ci] and [profile] sections instead.\n\nname: CI\n\non:\n  push:\n    branches: [{}]\n  pull_request:\n    branches: [{}]\n{}\njobs:\n{}\n",
            ci_branch,
            pr_target,
            concurrency,
            job_blocks.join("\n\n"),
        ))
    }

    /// Generate CI workflow for infrastructure-only repos (no Node.js)
    fn render_infra_ci_workflow(&self, manifest: &Manifest) -> Result<String> {
        let ci = &manifest.ci;
        let a = ResolvedActions::from_manifest(&ci.actions)?;
        let checkout = &a.checkout;
        let runner = ci.runner.as_deref().unwrap_or("ubuntu-latest");
        let runner_yaml = if runner.contains(',') {
            format!("[{}]", runner)
        } else {
            runner.to_string()
        };

        let deploy_profiles = manifest.deploy_profiles();
        let pr_target = deploy_profiles
            .iter()
            .find(|(name, p)| p.effective_role(name) == "production")
            .map(|(_, p)| p.branch.as_deref().unwrap_or("main"))
            .unwrap_or("main");

        Ok(format!(
            "# Auto-generated by airis gen — DO NOT EDIT\n# Change manifest.toml [ci] and [profile] sections instead.\n\nname: CI\n\non:\n  pull_request:\n    branches: [{pr_target}]\n\njobs:\n  validate:\n    runs-on: {runner_yaml}\n    timeout-minutes: 5\n    steps:\n      - uses: {checkout}\n      - name: Validate compose\n        run: docker compose config --quiet\n"
        ))
    }

    /// Generate .github/workflows/deploy.yml from manifest v2
    pub fn render_deploy_workflow(&self, manifest: &Manifest) -> Result<String> {
        let ci = &manifest.ci;
        let a = ResolvedActions::from_manifest(&ci.actions)?;
        let checkout = &a.checkout;
        let pnpm_action = &a.pnpm;
        let setup_node = &a.setup_node;
        let doppler_action = &a.doppler;
        let node_version = manifest.node_version();
        let runner = ci.runner.as_deref().unwrap_or("ubuntu-latest");
        let worker_runner = ci.worker_runner.as_deref().unwrap_or("ubuntu-latest");

        let runner_yaml = if runner.contains(',') {
            format!("[{}]", runner)
        } else {
            runner.to_string()
        };

        // Collect deploy branches from profiles
        let deploy_profiles = manifest.deploy_profiles();
        let branches: Vec<&str> = deploy_profiles
            .iter()
            .filter_map(|(_, p)| p.branch.as_deref())
            .collect();
        let branches_yaml = branches.join(", ");

        // Determine main branch (production profile)
        let main_branch = deploy_profiles
            .iter()
            .find(|(name, p)| p.effective_role(name) == "production")
            .and_then(|(_, p)| p.branch.as_deref())
            .unwrap_or("main");

        // Build doppler token expression from profiles
        let doppler_token_expr = {
            let doppler_profiles: Vec<_> = deploy_profiles
                .iter()
                .filter_map(|(_, p)| p.env_source.doppler_config())
                .collect();
            if doppler_profiles.len() >= 2 {
                let parts: Vec<String> = doppler_profiles
                    .iter()
                    .map(|d| format!("needs.prepare.outputs.doppler_config == '{}' && secrets.{}", d.config, d.secret))
                    .collect();
                format!("${{{{ {} || {} }}}}", parts[0], parts[1])
            } else if let Some(d) = doppler_profiles.first() {
                format!("${{{{ secrets.{} }}}}", d.secret)
            } else {
                "${{ secrets.DOPPLER_TOKEN }}".to_string()
            }
        };
        let doppler_config_expr = "${{ needs.prepare.outputs.doppler_config }}";

        // Separate docker and worker apps
        let docker_apps: Vec<&crate::manifest::ProjectDefinition> = manifest
            .app
            .iter()
            .filter(|a| {
                a.deploy
                    .as_ref()
                    .is_some_and(|d| d.enabled && !a.is_worker_deploy())
            })
            .collect();
        let worker_apps: Vec<&crate::manifest::ProjectDefinition> = manifest
            .app
            .iter()
            .filter(|a| a.deploy.as_ref().is_some_and(|d| d.enabled) && a.is_worker_deploy())
            .collect();

        // Infrastructure-only: no apps to deploy, just docker compose up
        if docker_apps.is_empty() && worker_apps.is_empty() {
            return self.render_infra_deploy_workflow(manifest);
        }

        let all_deploy_apps: Vec<&crate::manifest::ProjectDefinition> = manifest
            .app
            .iter()
            .filter(|a| a.deploy.as_ref().is_some_and(|d| d.enabled))
            .collect();

        // --- Prepare job ---
        let mut prepare_outputs = Vec::new();
        let mut change_detections = Vec::new();
        let mut dispatch_outputs = Vec::new();

        for app in &all_deploy_apps {
            let snake = app.name.replace('-', "_");
            let path = app.path.as_deref().unwrap_or(&app.name);
            prepare_outputs.push(format!(
                "      {}: ${{{{ steps.check.outputs.{} }}}}",
                snake, snake
            ));
            change_detections.push(format!(
                "            echo \"{}=$(echo \"$CHANGED\" | grep -qE '^{}/' && echo true || echo $LIBS_CHANGED)\" >> $GITHUB_OUTPUT",
                snake, path
            ));
            dispatch_outputs.push(format!(
                "            echo \"{}=true\" >> $GITHUB_OUTPUT",
                snake
            ));
        }

        // --- Docker deploy jobs ---
        let mut docker_jobs = Vec::new();
        let mut generated_app_names: Vec<String> = Vec::new(); // Track actually generated jobs
        for app in &docker_apps {
            let deploy = app.deploy.as_ref().unwrap();
            let snake = app.name.replace('-', "_");
            let kebab = &app.name;

            // Host rule for health check (v2: host, v1 compat: host_rule)
            let host_raw = deploy
                .host
                .as_deref()
                .or(deploy.host_rule.as_deref())
                .unwrap_or("");
            if host_raw.is_empty() {
                continue;
            }

            // Convert host template to doppler expansion for deploy
            // v2: {profile.domain} → $(doppler secrets get CORPORATE_DOMAIN)
            // v1: ${CORPORATE_DOMAIN} → $(doppler secrets get CORPORATE_DOMAIN)
            let health_domain = if host_raw.contains("{profile.domain}") {
                let prefix = host_raw.replace("{profile.domain}", "");
                if prefix.is_empty() {
                    format!(
                        "$(doppler secrets get CORPORATE_DOMAIN --plain -c {})",
                        doppler_config_expr
                    )
                } else {
                    format!(
                        "{}$(doppler secrets get CORPORATE_DOMAIN --plain -c {})",
                        prefix, doppler_config_expr
                    )
                }
            } else if host_raw.contains("${CORPORATE_DOMAIN}") {
                // v1 compat: ${CORPORATE_DOMAIN} → doppler expansion
                let prefix = host_raw.replace("${CORPORATE_DOMAIN}", "");
                if prefix.is_empty() {
                    format!(
                        "$(doppler secrets get CORPORATE_DOMAIN --plain -c {})",
                        doppler_config_expr
                    )
                } else {
                    format!(
                        "{}$(doppler secrets get CORPORATE_DOMAIN --plain -c {})",
                        prefix, doppler_config_expr
                    )
                }
            } else {
                host_raw.to_string()
            };

            let timeout = deploy.timeout.unwrap_or(15);
            let retries = deploy.health_retries.unwrap_or(6);
            let interval = deploy.health_retry_interval.unwrap_or(10);
            let retry_seq = (1..=retries).map(|i| i.to_string()).collect::<Vec<_>>().join(" ");

            generated_app_names.push(kebab.to_string());
            docker_jobs.push(format!(
                "  deploy-{kebab}:\n    name: Deploy {kebab}\n    runs-on: {runner_yaml}\n    concurrency:\n      group: deploy-{kebab}-${{{{ github.ref }}}}\n      cancel-in-progress: true\n    needs: prepare\n    if: needs.prepare.outputs.{snake} == 'true'\n    timeout-minutes: {timeout}\n    steps:\n      - uses: {checkout}\n      - uses: {doppler_action}\n      - name: Deploy\n        env:\n          DOPPLER_TOKEN: {doppler_token_expr}\n        run: |\n          doppler run -c {doppler_config_expr} -- docker compose -f deploy/compose.yml --profile {kebab} up -d --build --force-recreate\n      - name: Health Check\n        env:\n          DOPPLER_TOKEN: {doppler_token_expr}\n        run: |\n          DOMAIN=\"{health_domain}\"\n          for i in {retry_seq}; do\n            sleep {interval}\n            curl -sf \"https://$DOMAIN{health_path}\" && echo \"{kebab} health check passed\" && exit 0 || echo \"Attempt $i failed, retrying...\"\n          done\n          echo \"Health check failed after {retries} attempts\"; exit 1",
                health_path = deploy.health_path,
            ));
        }

        // --- Worker deploy jobs ---
        let pnpm_store_step = if let Some(ref store_path) = ci.pnpm_store_path {
            format!(
                "      - name: Configure pnpm store\n        run: pnpm config set store-dir {}",
                store_path
            )
        } else {
            format!("      - name: Cache pnpm store\n        uses: {}\n        with:\n          path: ~/.pnpm-store\n          key: ${{{{ runner.os }}}}-pnpm-${{{{ hashFiles('pnpm-lock.yaml') }}}}\n          restore-keys: ${{{{ runner.os }}}}-pnpm-", a.cache)
        };

        let mut worker_jobs = Vec::new();
        for app in &worker_apps {
            let snake = app.name.replace('-', "_");
            let kebab = &app.name;
            let path = app.path.as_deref().unwrap_or(&app.name);

            let deploy = app.deploy.as_ref().unwrap();
            let timeout = deploy.timeout.unwrap_or(10);
            let health_path = &deploy.health_path;
            let workers_domain = deploy.workers_domain.as_deref()
                .ok_or_else(|| anyhow::anyhow!(
                    "app '{}': deploy_target=worker requires workers_domain (e.g., 'myorg.workers.dev')",
                    app.name
                ))?;

            generated_app_names.push(kebab.to_string());
            worker_jobs.push(format!(
                "  deploy-{kebab}:\n    name: Deploy {kebab}\n    runs-on: {worker_runner}\n    concurrency:\n      group: deploy-{kebab}-${{{{ github.ref }}}}\n      cancel-in-progress: true\n    needs: prepare\n    if: needs.prepare.outputs.{snake} == 'true'\n    timeout-minutes: {timeout}\n    steps:\n      - uses: {checkout}\n      - uses: {doppler_action}\n      - uses: {pnpm_action}\n      - uses: {setup_node}\n        with:\n          node-version: '{node_version}'\n{pnpm_store_step}\n      - name: Install dependencies\n        run: pnpm install --frozen-lockfile\n      - name: Deploy to Cloudflare Workers\n        env:\n          DOPPLER_TOKEN: {doppler_token_expr}\n        run: |\n          cd {path}\n          export CLOUDFLARE_API_TOKEN=$(doppler secrets get CLOUDFLARE_API_TOKEN --plain -c {doppler_config_expr})\n          if [ \"{doppler_config_expr}\" = \"prd\" ]; then\n            pnpm wrangler deploy\n          else\n            pnpm wrangler deploy --env staging\n          fi\n      - name: Health Check\n        run: |\n          sleep 5\n          if [ \"{doppler_config_expr}\" = \"prd\" ]; then\n            URL=\"https://{kebab}-production.{workers_domain}{health_path}\"\n          else\n            URL=\"https://{kebab}.{workers_domain}{health_path}\"\n          fi\n          curl -sf \"$URL\" && echo \"{kebab} health check passed\" || {{ echo \"Health check failed\"; exit 1; }}",
            ));
        }

        // --- Notify job (only reference actually generated jobs) ---
        let notify_needs: Vec<String> = std::iter::once("prepare".to_string())
            .chain(generated_app_names.iter().map(|name| format!("deploy-{}", name)))
            .collect();

        let notify_rows: Vec<String> = generated_app_names
            .iter()
            .map(|name| {
                format!(
                    "          echo \"| {} | ${{{{ needs.deploy-{}.result || 'skipped' }}}} |\" >> $GITHUB_STEP_SUMMARY",
                    name, name
                )
            })
            .collect();

        // Assemble all jobs
        let all_jobs: Vec<String> = docker_jobs
            .into_iter()
            .chain(worker_jobs)
            .collect();

        Ok(format!(
            "# Auto-generated by airis gen — DO NOT EDIT\n# Change manifest.toml [ci], [profile], and [app.deploy] sections instead.\n\nname: Deploy\n\non:\n  push:\n    branches: [{branches_yaml}]\n  workflow_dispatch:\n\njobs:\n  prepare:\n    name: Prepare\n    runs-on: {runner_yaml}\n    outputs:\n{prepare_outputs}\n      doppler_config: ${{{{ steps.env.outputs.doppler_config }}}}\n      branch: ${{{{ steps.env.outputs.branch }}}}\n    steps:\n      - uses: {checkout}\n        with:\n          fetch-depth: 2\n      - name: Set environment\n        id: env\n        run: |\n          BRANCH=\"${{{{ github.ref_name }}}}\"\n          echo \"branch=$BRANCH\" >> $GITHUB_OUTPUT\n          if [ \"$BRANCH\" = \"{main_branch}\" ]; then\n            echo \"doppler_config=prd\" >> $GITHUB_OUTPUT\n          else\n            echo \"doppler_config=stg\" >> $GITHUB_OUTPUT\n          fi\n      - name: Detect changes\n        id: check\n        run: |\n          if [ \"${{{{ github.event_name }}}}\" = \"workflow_dispatch\" ]; then\n{dispatch_outputs}\n          else\n            BEFORE=\"${{{{ github.event.before }}}}\"\n            AFTER=\"${{{{ github.sha }}}}\"\n            if [ \"$BEFORE\" = \"0000000000000000000000000000000000000000\" ] || ! git cat-file -e \"$BEFORE\" 2>/dev/null; then\n              BEFORE=\"HEAD~1\"\n            fi\n            CHANGED=$(git diff --name-only \"$BEFORE\" \"$AFTER\" 2>/dev/null || echo \"\")\n            echo \"Changed files:\"\n            echo \"$CHANGED\"\n            LIBS_CHANGED=$(echo \"$CHANGED\" | grep -qE '^(libs|deploy)/' && echo true || echo false)\n{change_detections}\n          fi\n\n{all_jobs}\n\n  notify:\n    name: Notify\n    runs-on: {runner_yaml}\n    needs: [{notify_needs}]\n    if: always()\n    steps:\n      - name: Summary\n        run: |\n          echo \"## Deploy Summary\" >> $GITHUB_STEP_SUMMARY\n          echo \"| App | Status |\" >> $GITHUB_STEP_SUMMARY\n          echo \"|-----|--------|\" >> $GITHUB_STEP_SUMMARY\n{notify_rows}\n          echo \"\" >> $GITHUB_STEP_SUMMARY\n          echo \"**Branch:** ${{{{ needs.prepare.outputs.branch }}}}\" >> $GITHUB_STEP_SUMMARY\n          echo \"**Environment:** ${{{{ needs.prepare.outputs.doppler_config }}}}\" >> $GITHUB_STEP_SUMMARY\n",
            prepare_outputs = prepare_outputs.join("\n"),
            dispatch_outputs = dispatch_outputs.join("\n"),
            change_detections = change_detections.join("\n"),
            all_jobs = all_jobs.join("\n\n"),
            notify_needs = notify_needs.join(", "),
            notify_rows = notify_rows.join("\n"),
        ))
    }

    /// Generate deploy workflow for infrastructure-only repos (no apps)
    fn render_infra_deploy_workflow(&self, manifest: &Manifest) -> Result<String> {
        let ci = &manifest.ci;
        let a = ResolvedActions::from_manifest(&ci.actions)?;
        let checkout = &a.checkout;
        let doppler_action = &a.doppler;
        let runner = ci.runner.as_deref().unwrap_or("ubuntu-latest");
        let runner_yaml = if runner.contains(',') {
            format!("[{}]", runner)
        } else {
            runner.to_string()
        };

        let deploy_profiles = manifest.deploy_profiles();
        let branches: Vec<&str> = deploy_profiles
            .iter()
            .filter_map(|(_, p)| p.branch.as_deref())
            .collect();
        let branches_yaml = branches.join(", ");
        let project_id = &manifest.project.id;

        // Doppler token from profile
        let doppler_secret = deploy_profiles
            .iter()
            .find_map(|(_, p)| p.env_source.doppler_config())
            .map(|d| d.secret.as_str())
            .unwrap_or("DOPPLER_TOKEN");

        let network_name = manifest.orchestration
            .networks.as_ref()
            .and_then(|n| n.proxy.as_deref())
            .unwrap_or("proxy");

        Ok(format!(
            "# Auto-generated by airis gen — DO NOT EDIT\n# Change manifest.toml [ci] and [profile] sections instead.\n\nname: Deploy\n\non:\n  push:\n    branches: [{branches_yaml}]\n  workflow_dispatch:\n\nconcurrency:\n  group: deploy-{project_id}\n  cancel-in-progress: false\n\njobs:\n  deploy:\n    runs-on: {runner_yaml}\n    steps:\n      - uses: {checkout}\n      - uses: {doppler_action}\n      - name: Ensure proxy network\n        run: docker network create {network_name} 2>/dev/null || true\n      - name: Deploy\n        env:\n          DOPPLER_TOKEN: ${{{{ secrets.{doppler_secret} }}}}\n        run: doppler run -- docker compose up -d --pull always --remove-orphans\n      - name: Show status\n        run: docker compose ps\n"
        ))
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
            "is_pnpm": pm_bin == "pnpm",
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

    /// Render tsconfig.base.json — shared compilerOptions only (no baseUrl/paths).
    pub fn render_tsconfig_base(&self, manifest: &Manifest) -> Result<String> {
        let mut compiler_options = serde_json::Map::new();

        // Derive ES target from workspace.image Node version
        let es_target = crate::conventions::parse_node_version_from_image(&manifest.workspace.image)
            .map(crate::conventions::node_version_to_es_target)
            .unwrap_or("ES2023");

        // Defaults
        let defaults: &[(&str, serde_json::Value)] = &[
            ("target", json!(es_target)),
            ("module", json!("ESNext")),
            ("moduleResolution", json!("bundler")),
            ("lib", json!([es_target])),
            ("strict", json!(true)),
            ("esModuleInterop", json!(true)),
            ("skipLibCheck", json!(true)),
            ("forceConsistentCasingInFileNames", json!(true)),
            ("resolveJsonModule", json!(true)),
            ("isolatedModules", json!(true)),
            ("types", json!(["node"])),
        ];
        for (key, value) in defaults {
            compiler_options.insert((*key).to_string(), value.clone());
        }

        // Merge user-specified compilerOptions from [typescript.compiler_options]
        for (key, value) in &manifest.typescript.compiler_options {
            compiler_options.insert(key.clone(), toml_value_to_json(value));
        }

        let tsconfig = json!({
            "_generated": "DO NOT EDIT — regenerated by airis gen from manifest.toml [typescript]",
            "compilerOptions": serde_json::Value::Object(compiler_options),
        });

        let content = serde_json::to_string_pretty(&tsconfig)
            .context("Failed to serialize tsconfig.base.json")?;
        Ok(format!("{content}\n"))
    }

    /// Render root tsconfig.json — IDE config with baseUrl + paths + ignoreDeprecations.
    ///
    /// `workspace_paths` is a list of (package_name, relative_path) pairs auto-discovered
    /// from workspace patterns. `ts_major` controls whether `ignoreDeprecations` is added.
    pub fn render_tsconfig_root(
        &self,
        manifest: &Manifest,
        workspace_paths: &[(String, String)],
        ts_major: u32,
    ) -> Result<String> {
        let mut paths = serde_json::Map::new();

        // Auto-generated paths from workspace discovery
        for (pkg_name, rel_path) in workspace_paths {
            paths.insert(
                pkg_name.clone(),
                json!([format!("{}/src", rel_path)]),
            );
        }

        // Merge user-specified paths from [typescript.paths]
        for (alias, target) in &manifest.typescript.paths {
            paths.insert(alias.clone(), json!([target]));
        }

        let mut compiler_options = serde_json::Map::new();
        compiler_options.insert("noEmit".to_string(), json!(true));
        compiler_options.insert("baseUrl".to_string(), json!("."));

        if !paths.is_empty() {
            compiler_options.insert("paths".to_string(), serde_json::Value::Object(paths));
        }

        if ts_major >= 6 {
            compiler_options.insert("ignoreDeprecations".to_string(), json!("6.0"));
        }

        // Build include patterns from workspace patterns
        let workspace_patterns = if !manifest.packages.workspaces.is_empty() {
            &manifest.packages.workspaces
        } else {
            &manifest.workspace.workspaces
        };

        let mut include: Vec<String> = Vec::new();
        for pattern in workspace_patterns {
            if pattern.starts_with('!') {
                continue;
            }
            // Convert glob pattern to ts include pattern
            // "apps/*" → "apps/**/*.ts", "apps/**/*.tsx"
            // "products/**" → "products/**/*.ts", "products/**/*.tsx"
            let base = pattern.trim_end_matches('*').trim_end_matches('/');
            include.push(format!("{}/**/*.ts", base));
            include.push(format!("{}/**/*.tsx", base));
        }
        if include.is_empty() {
            include.push("**/*.ts".to_string());
            include.push("**/*.tsx".to_string());
        }

        let tsconfig = json!({
            "_generated": "DO NOT EDIT — regenerated by airis gen from manifest.toml [typescript]",
            "extends": "./tsconfig.base.json",
            "compilerOptions": serde_json::Value::Object(compiler_options),
            "include": include,
            "exclude": [
                "node_modules",
                "**/node_modules",
                "dist",
                "**/dist",
                ".next",
                "**/.next",
                "coverage",
            ],
        });

        let content = serde_json::to_string_pretty(&tsconfig)
            .context("Failed to serialize tsconfig.json")?;
        Ok(format!("{content}\n"))
    }
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

{{#if is_pnpm}}
# Fetch dependencies first (lockfile-only layer, maximizes Docker cache hits)
COPY pnpm-lock.yaml pnpm-workspace.yaml .npmrc* package.json ./
RUN --mount=type=cache,id=pnpm,target=/pnpm/store {{pm_bin}} fetch

# Then copy source and install from cache (no network needed)
COPY . .
RUN --mount=type=cache,id=pnpm,target=/pnpm/store {{pm_bin}} install --offline
RUN chown -R app:app {{workdir}}
USER app
{{else}}
COPY . .
RUN {{pm_bin}} install
RUN chown -R app:app {{workdir}}
USER app
{{/if}}

ENTRYPOINT ["tini","--"]
"#;

const DOCKER_COMPOSE_TEMPLATE: &str = r#"# ============================================================
# {{project}}
# ============================================================
# Generated by `airis gen` - DO NOT EDIT MANUALLY
# Source of truth: manifest.toml
#
# Use `airis up` to start. Profiles control which services run.
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

/// Convert a TOML value to a serde_json value for tsconfig generation.
fn toml_value_to_json(value: &toml::Value) -> serde_json::Value {
    match value {
        toml::Value::String(s) => json!(s),
        toml::Value::Integer(i) => json!(i),
        toml::Value::Float(f) => json!(f),
        toml::Value::Boolean(b) => json!(b),
        toml::Value::Array(a) => {
            serde_json::Value::Array(a.iter().map(toml_value_to_json).collect())
        }
        toml::Value::Table(t) => {
            let map: serde_json::Map<String, serde_json::Value> = t
                .iter()
                .map(|(k, v)| (k.clone(), toml_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        toml::Value::Datetime(d) => json!(d.to_string()),
    }
}

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
package_manager = "pnpm@10.22.0"
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

        // Dockerfile should use pnpm fetch + install --offline pattern
        assert!(result.contains("pnpm fetch"));
        assert!(result.contains("pnpm install --offline"));
        // Should use BuildKit cache mount for pnpm store
        assert!(result.contains("--mount=type=cache,id=pnpm,target=/pnpm/store"));
        // Should NOT contain sleep infinity
        assert!(!result.contains("sleep infinity"));
        // Should contain COPY
        assert!(result.contains("COPY . ."));
        // Lockfile should be copied before source for cache optimization
        assert!(result.contains("COPY pnpm-lock.yaml"));
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

        // Non-pnpm uses simple install (no fetch + offline pattern)
        assert!(result.contains("RUN bun install"));
        assert!(!result.contains("bun fetch"));
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

    #[test]
    fn test_ci_workflow_custom_jobs() {
        let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true
runner = "self-hosted, linux"

[ci.jobs]
lint = 10
typecheck = 10
test = 20
e2e = 30

[profile.stg]
branch = "stg"
domain = "stg.example.com"

[profile.prd]
branch = "main"
domain = "example.com"
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_ci_workflow(&manifest).unwrap();

        assert!(result.contains("runs-on: [self-hosted, linux]"), "runner should be self-hosted array");
        assert!(result.contains("  lint:"), "should have lint job");
        assert!(result.contains("  typecheck:"), "should have typecheck job");
        assert!(result.contains("  test:"), "should have test job");
        assert!(result.contains("  e2e:"), "should have e2e job");
        assert!(result.contains("timeout-minutes: 30"), "e2e should have 30min timeout");
        assert!(result.contains("timeout-minutes: 20"), "test should have 20min timeout");
        assert!(result.contains("pnpm turbo run e2e"), "e2e job should run turbo e2e");
    }

    #[test]
    fn test_ci_workflow_default_jobs() {
        let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true

[profile.stg]
branch = "stg"
domain = "stg.example.com"

[profile.prd]
branch = "main"
domain = "example.com"
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_ci_workflow(&manifest).unwrap();

        assert!(result.contains("  lint:"), "should have lint job");
        assert!(result.contains("  typecheck:"), "should have typecheck job");
        assert!(result.contains("  test:"), "should have test job");
        assert!(!result.contains("  e2e:"), "should NOT have e2e job by default");
    }

    #[test]
    fn test_profile_effective_role() {
        use crate::manifest::ProfileSection;
        let default = ProfileSection::default();

        // Name-based inference
        assert_eq!(default.effective_role("prd"), "production");
        assert_eq!(default.effective_role("prod"), "production");
        assert_eq!(default.effective_role("production"), "production");
        assert_eq!(default.effective_role("local"), "local");
        assert_eq!(default.effective_role("dev"), "local");
        assert_eq!(default.effective_role("stg"), "staging");
        assert_eq!(default.effective_role("staging"), "staging");
        assert_eq!(default.effective_role("preview"), "staging");

        // Explicit role overrides name
        let mut custom = ProfileSection::default();
        custom.role = Some("production".to_string());
        assert_eq!(custom.effective_role("stg"), "production");
    }

    #[test]
    fn test_profile_role_in_ci_workflow() {
        let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true

[profile.staging]
branch = "develop"
domain = "stg.example.com"
role = "staging"

[profile.live]
branch = "release"
domain = "example.com"
role = "production"
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_ci_workflow(&manifest).unwrap();

        assert!(result.contains("branches: [develop]"), "CI should use staging branch 'develop'");
        assert!(result.contains("branches: [release]"), "PR target should use production branch 'release'");
    }

    #[test]
    fn test_notify_job_uses_ci_runner() {
        let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true
runner = "self-hosted, linux"

[[app]]
name = "my-app"
path = "apps/my-app"
framework = "nextjs"

[app.deploy]
enabled = true
port = 3000
health_path = "/health"
host = "{profile.domain}"

[profile.stg]
branch = "stg"
domain = "stg.example.com"
env_source = { doppler = { config = "stg", secret = "DOPPLER_TOKEN_STG" } }

[profile.prd]
branch = "main"
domain = "example.com"
env_source = { doppler = { config = "prd", secret = "DOPPLER_TOKEN_PRD" } }
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_deploy_workflow(&manifest).unwrap();

        // Notify job should use the same runner as other jobs
        let notify_section = result.find("  notify:").expect("should have notify job");
        let after_notify = &result[notify_section..];
        assert!(after_notify.contains("runs-on: [self-hosted, linux]"), "notify should use ci.runner, not ubuntu-latest");
        assert!(!after_notify.contains("runs-on: ubuntu-latest"), "notify should NOT use ubuntu-latest");
    }

    #[test]
    fn test_docker_deploy_custom_timeout_and_retries() {
        let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true

[[app]]
name = "my-api"
path = "apps/my-api"
framework = "node"

[app.deploy]
enabled = true
port = 3000
health_path = "/healthz"
host = "{profile.domain}"
timeout = 20
health_retries = 10
health_retry_interval = 15

[profile.stg]
branch = "stg"
domain = "stg.example.com"
env_source = { doppler = { config = "stg", secret = "DOPPLER_TOKEN_STG" } }

[profile.prd]
branch = "main"
domain = "example.com"
env_source = { doppler = { config = "prd", secret = "DOPPLER_TOKEN_PRD" } }
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_deploy_workflow(&manifest).unwrap();

        let deploy_section = result.find("deploy-my-api:").expect("should have deploy job");
        let after_deploy = &result[deploy_section..];
        assert!(after_deploy.contains("timeout-minutes: 20"), "should use custom timeout");
        assert!(after_deploy.contains("for i in 1 2 3 4 5 6 7 8 9 10;"), "should have 10 retries");
        assert!(after_deploy.contains("sleep 15"), "should use custom retry interval");
        assert!(after_deploy.contains("after 10 attempts"), "error message should reflect retry count");
    }

    #[test]
    fn test_worker_deploy_custom_domain() {
        let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true

[[app]]
name = "my-worker"
path = "apps/my-worker"
framework = "node"

[app.deploy]
enabled = true
deploy_target = "worker"
health_path = "/health"
workers_domain = "myorg.workers.dev"

[profile.stg]
branch = "stg"
domain = "stg.example.com"
env_source = { doppler = { config = "stg", secret = "DOPPLER_TOKEN_STG" } }

[profile.prd]
branch = "main"
domain = "example.com"
env_source = { doppler = { config = "prd", secret = "DOPPLER_TOKEN_PRD" } }
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_deploy_workflow(&manifest).unwrap();

        assert!(result.contains("my-worker-production.myorg.workers.dev/health"), "production URL should use workers_domain and health_path");
        assert!(result.contains("my-worker.myorg.workers.dev/health"), "staging URL should use workers_domain and health_path");
        assert!(!result.contains("agiletec"), "should NOT contain hardcoded agiletec domain");
    }

    #[test]
    fn test_worker_deploy_missing_domain_errors() {
        let toml_str = r#"
[project]
id = "test-project"

[workspace]
package_manager = "pnpm"
members = ["apps/*"]

[ci]
enabled = true

[[app]]
name = "my-worker"
path = "apps/my-worker"
framework = "node"

[app.deploy]
enabled = true
deploy_target = "worker"
health_path = "/health"

[profile.stg]
branch = "stg"
domain = "stg.example.com"
env_source = { doppler = { config = "stg", secret = "DOPPLER_TOKEN_STG" } }

[profile.prd]
branch = "main"
domain = "example.com"
env_source = { doppler = { config = "prd", secret = "DOPPLER_TOKEN_PRD" } }
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_deploy_workflow(&manifest);
        assert!(result.is_err(), "should error when workers_domain is missing for worker deploy");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("workers_domain"), "error should mention workers_domain");
    }

    #[test]
    fn test_infra_deploy_custom_network() {
        let toml_str = r#"
[project]
id = "test-project"

[ci]
enabled = true

[profile.prd]
branch = "main"
domain = "example.com"
env_source = { doppler = { config = "prd", secret = "DOPPLER_TOKEN" } }

[orchestration.networks]
proxy = "traefik-public"
"#;
        let manifest: Manifest = toml::from_str(toml_str).unwrap();
        let engine = TemplateEngine::new().unwrap();
        let result = engine.render_deploy_workflow(&manifest).unwrap();

        assert!(result.contains("docker network create traefik-public"), "should use custom network name from orchestration.networks.proxy");
        assert!(!result.contains("docker network create proxy"), "should NOT use hardcoded 'proxy' network");
    }

}
