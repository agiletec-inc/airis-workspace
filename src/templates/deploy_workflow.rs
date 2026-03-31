use super::TemplateEngine;
use super::actions::ResolvedActions;
use super::deploy_env::resolve_deploy_env;
use crate::manifest::Manifest;
use anyhow::Result;

impl TemplateEngine {
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
                    .map(|d| {
                        format!(
                            "needs.prepare.outputs.doppler_config == '{}' && secrets.{}",
                            d.config, d.secret
                        )
                    })
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
            let Some(deploy) = app.deploy.as_ref() else {
                continue;
            };
            let snake = app.name.replace('-', "_");
            let kebab = &app.name;

            // Resolve env_groups + explicit env into a single list for deploy compose
            let _resolved_env = resolve_deploy_env(deploy, manifest);

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
            let retry_seq = (1..=retries)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(" ");

            generated_app_names.push(kebab.to_string());
            docker_jobs.push(format!(
                "  deploy-{kebab}:\n    name: Deploy {kebab}\n    runs-on: {runner_yaml}\n    concurrency:\n      group: deploy-{kebab}-${{{{ github.ref }}}}\n      cancel-in-progress: true\n    needs: prepare\n    if: needs.prepare.outputs.{snake} == 'true'\n    timeout-minutes: {timeout}\n    steps:\n      - uses: {checkout}\n      - uses: {doppler_action}\n      - name: Deploy\n        env:\n          DOPPLER_TOKEN: {doppler_token_expr}\n        run: |\n          doppler run -c {doppler_config_expr} -- docker compose -f deploy/compose.yml --profile {kebab} up -d --build --force-recreate\n      - name: Health Check\n        env:\n          DOPPLER_TOKEN: {doppler_token_expr}\n        run: |\n          DOMAIN=\"{health_domain}\"\n          for i in {retry_seq}; do\n            sleep {interval}\n            curl -sf \"https://$DOMAIN{health_path}\" && echo \"{kebab} health check passed\" && exit 0 || echo \"Attempt $i failed, retrying...\"\n          done\n          echo \"Health check failed after {retries} attempts\"; exit 1",
                health_path = deploy.health_path.as_deref().unwrap_or("/health"),
            ));
        }

        // --- Worker deploy jobs ---
        let pnpm_store_step = if let Some(ref store_path) = ci.pnpm_store_path {
            format!(
                "      - name: Configure pnpm store\n        run: pnpm config set store-dir {}",
                store_path
            )
        } else {
            format!(
                "      - name: Cache pnpm store\n        uses: {}\n        with:\n          path: ~/.pnpm-store\n          key: ${{{{ runner.os }}}}-pnpm-${{{{ hashFiles('pnpm-lock.yaml') }}}}\n          restore-keys: ${{{{ runner.os }}}}-pnpm-",
                a.cache
            )
        };

        let mut worker_jobs = Vec::new();
        for app in &worker_apps {
            let snake = app.name.replace('-', "_");
            let kebab = &app.name;
            let path = app.path.as_deref().unwrap_or(&app.name);

            let Some(deploy) = app.deploy.as_ref() else {
                continue;
            };
            let timeout = deploy.timeout.unwrap_or(10);
            let health_path = deploy.health_path.as_deref().unwrap_or("/health");
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
            .chain(
                generated_app_names
                    .iter()
                    .map(|name| format!("deploy-{}", name)),
            )
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
        let all_jobs: Vec<String> = docker_jobs.into_iter().chain(worker_jobs).collect();

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
    pub(super) fn render_infra_deploy_workflow(&self, manifest: &Manifest) -> Result<String> {
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

        let network_name = manifest
            .orchestration
            .networks
            .as_ref()
            .and_then(|n| n.proxy.as_deref())
            .unwrap_or("proxy");

        Ok(format!(
            "# Auto-generated by airis gen — DO NOT EDIT\n# Change manifest.toml [ci] and [profile] sections instead.\n\nname: Deploy\n\non:\n  push:\n    branches: [{branches_yaml}]\n  workflow_dispatch:\n\nconcurrency:\n  group: deploy-{project_id}\n  cancel-in-progress: false\n\njobs:\n  deploy:\n    runs-on: {runner_yaml}\n    steps:\n      - uses: {checkout}\n      - uses: {doppler_action}\n      - name: Ensure proxy network\n        run: docker network create {network_name} 2>/dev/null || true\n      - name: Deploy\n        env:\n          DOPPLER_TOKEN: ${{{{ secrets.{doppler_secret} }}}}\n        run: doppler run -- docker compose up -d --pull always --remove-orphans\n      - name: Show status\n        run: docker compose ps\n"
        ))
    }
}
