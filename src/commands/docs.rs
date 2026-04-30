use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::manifest::{DocsMode, DocsVendor, MANIFEST_FILE, Manifest, MockPolicy, TestingSection};

const DEFAULT_SOURCE_FILES: &[&str] = &[
    "docs/ai/PROJECT_RULES.md",
    "docs/ai/WORKFLOW.md",
    "docs/ai/REVIEW.md",
    "docs/ai/STACK.md",
];

const DEFAULT_SKILLS_SOURCE: &str = "docs/ai/playbooks";
const DEFAULT_HOOKS_POLICY: &str = "docs/ai/hooks/HOOKS_POLICY.md";

pub fn wrap(target: &str, force: bool) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    ensure_manifest_exists(manifest_path)?;

    let mut manifest = Manifest::load(manifest_path)?;
    let vendor = vendor_for_target(target)?;

    if !manifest.docs.targets.contains(&target.to_string()) {
        manifest.docs.targets.push(target.to_string());
    }
    if !manifest.docs.vendors.contains(&vendor) {
        manifest.docs.vendors.push(vendor.clone());
    }
    if manifest.docs.sources.is_empty() {
        manifest.docs.sources = DEFAULT_SOURCE_FILES.iter().map(|s| s.to_string()).collect();
    }
    if manifest.docs.skills_source.is_none() {
        manifest.docs.skills_source = Some(DEFAULT_SKILLS_SOURCE.to_string());
    }
    if manifest.docs.hooks_policy.is_none() {
        manifest.docs.hooks_policy = Some(DEFAULT_HOOKS_POLICY.to_string());
    }

    manifest.save(manifest_path)?;
    write_adapter_target(&manifest, target, force)?;

    println!(
        "✅ Generated {} and registered it in [docs]",
        target.green()
    );
    Ok(())
}

pub fn sync(force: bool) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    ensure_manifest_exists(manifest_path)?;

    let manifest = Manifest::load(manifest_path)?;
    let targets = sync_targets(&manifest)?;

    if targets.is_empty() {
        bail!(
            "No documentation targets configured. Use `airis docs wrap AGENTS.md` or add [docs.vendors]/[docs.targets]."
        );
    }

    for target in &targets {
        write_adapter_target(&manifest, target, force)?;
        println!("✅ Synced {}", target.green());
    }

    Ok(())
}

pub fn list() -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    ensure_manifest_exists(manifest_path)?;

    let manifest = Manifest::load(manifest_path)?;
    let sources = effective_sources(&manifest);
    let targets = sync_targets(&manifest)?;

    println!("📄 AI documentation");
    println!("   Mode: {}", format!("{:?}", manifest.docs.mode).yellow());
    println!();

    println!("Sources:");
    if sources.is_empty() {
        println!("   (none configured)");
    } else {
        for source in &sources {
            let status = if Path::new(source).exists() {
                "✅"
            } else {
                "❌"
            };
            println!("   {} {}", status, source.cyan());
        }
    }

    println!();
    println!("Adapters:");
    if targets.is_empty() {
        println!("   (none configured)");
    } else {
        for target in &targets {
            let status = if Path::new(target).exists() {
                "✅"
            } else {
                "❌"
            };
            println!("   {} {}", status, target.cyan());
        }
    }

    if let Some(skills_source) = effective_skills_source(&manifest) {
        println!();
        println!(
            "Skills source: {} {}",
            if Path::new(&skills_source).exists() {
                "✅"
            } else {
                "❌"
            },
            skills_source.cyan()
        );
    }

    if let Some(hooks_policy) = effective_hooks_policy(&manifest) {
        println!(
            "Hooks policy: {} {}",
            if Path::new(&hooks_policy).exists() {
                "✅"
            } else {
                "❌"
            },
            hooks_policy.cyan()
        );
    }

    Ok(())
}

fn ensure_manifest_exists(manifest_path: &Path) -> Result<()> {
    if !manifest_path.exists() {
        bail!(
            "❌ {} not found. Create one (see docs/manifest.md) or ask Claude Code via {}.",
            MANIFEST_FILE.bold(),
            "/airis:init".bold()
        );
    }
    Ok(())
}

fn sync_targets(manifest: &Manifest) -> Result<Vec<String>> {
    if !manifest.docs.targets.is_empty() {
        return Ok(manifest.docs.targets.clone());
    }

    let vendors = effective_vendors(manifest)?;
    let mut targets = Vec::new();
    for vendor in vendors {
        for target in targets_for_vendor(&vendor) {
            if !targets.contains(&target.to_string()) {
                targets.push(target.to_string());
            }
        }
    }

    Ok(targets)
}

fn effective_vendors(manifest: &Manifest) -> Result<Vec<DocsVendor>> {
    if !manifest.docs.vendors.is_empty() {
        return Ok(manifest.docs.vendors.clone());
    }

    if !manifest.docs.targets.is_empty() {
        let mut vendors = Vec::new();
        for target in &manifest.docs.targets {
            let vendor = vendor_for_target(target)?;
            if !vendors.contains(&vendor) {
                vendors.push(vendor);
            }
        }
        return Ok(vendors);
    }

    Ok(vec![
        DocsVendor::Codex,
        DocsVendor::Claude,
        DocsVendor::Gemini,
    ])
}

fn effective_sources(manifest: &Manifest) -> Vec<String> {
    if !manifest.docs.sources.is_empty() {
        return manifest.docs.sources.clone();
    }

    DEFAULT_SOURCE_FILES
        .iter()
        .filter(|path| Path::new(path).exists())
        .map(|path| path.to_string())
        .collect()
}

fn effective_skills_source(manifest: &Manifest) -> Option<String> {
    manifest.docs.skills_source.clone().or_else(|| {
        Path::new(DEFAULT_SKILLS_SOURCE)
            .exists()
            .then(|| DEFAULT_SKILLS_SOURCE.into())
    })
}

fn effective_hooks_policy(manifest: &Manifest) -> Option<String> {
    manifest.docs.hooks_policy.clone().or_else(|| {
        Path::new(DEFAULT_HOOKS_POLICY)
            .exists()
            .then(|| DEFAULT_HOOKS_POLICY.into())
    })
}

fn write_adapter_target(manifest: &Manifest, target: &str, force: bool) -> Result<()> {
    let target_path = Path::new(target);
    handle_existing_file(manifest, target_path, force)?;

    let content = render_adapter(manifest, target)?;
    fs::write(target_path, content).with_context(|| format!("Failed to write {}", target))?;
    Ok(())
}

fn handle_existing_file(manifest: &Manifest, target_path: &Path, force: bool) -> Result<()> {
    if !target_path.exists() {
        return Ok(());
    }

    if force {
        // Caller explicitly opted into overwriting; skip the warn bail and the
        // backup so re-runs are idempotent without leaving `.bak` litter.
        return Ok(());
    }

    match manifest.docs.mode {
        DocsMode::Warn => {
            bail!(
                "⚠️  {} already exists. Refusing to overwrite in [docs.mode = \"warn\"]. Re-run with `--force` to overwrite, or set [docs.mode = \"backup\"] to keep a `.bak` copy.",
                target_path.display()
            );
        }
        DocsMode::Backup => {
            let backup_path = format!("{}.bak", target_path.display());
            fs::copy(target_path, &backup_path)
                .with_context(|| format!("Failed to create backup: {}", backup_path))?;
            println!(
                "📦 Backed up {} → {}",
                target_path.display().to_string().cyan(),
                backup_path.yellow()
            );
        }
    }

    Ok(())
}

fn render_adapter(manifest: &Manifest, target: &str) -> Result<String> {
    let sources = effective_sources(manifest);
    if sources.is_empty() {
        bail!("No shared AI docs found. Add [docs.sources] or create docs/ai/PROJECT_RULES.md.");
    }

    let skills_source = effective_skills_source(manifest);
    let hooks_policy = effective_hooks_policy(manifest);

    let testing = &manifest.policy.testing;

    match target {
        "AGENTS.md" => Ok(render_agents_md(
            &sources,
            skills_source.as_deref(),
            hooks_policy.as_deref(),
            testing,
        )),
        "CLAUDE.md" => Ok(render_claude_md(
            &sources,
            skills_source.as_deref(),
            hooks_policy.as_deref(),
            &manifest.mcp.servers,
            testing,
        )),
        "GEMINI.md" => Ok(render_gemini_md(
            &sources,
            skills_source.as_deref(),
            hooks_policy.as_deref(),
            testing,
        )),
        ".cursorrules" => Ok(render_cursorrules(
            &sources,
            skills_source.as_deref(),
            hooks_policy.as_deref(),
            testing,
        )),
        _ => bail!(
            "❌ Unknown documentation file: {}. Supported: CLAUDE.md, .cursorrules, GEMINI.md, AGENTS.md",
            target.red()
        ),
    }
}

fn render_agents_md(
    sources: &[String],
    skills_source: Option<&str>,
    hooks_policy: Option<&str>,
    testing: &TestingSection,
) -> String {
    let mut lines = vec![
        "# AGENTS.md".to_string(),
        "".to_string(),
        "This file is generated by `airis docs sync`. Do not edit it directly.".to_string(),
        "".to_string(),
        "Primary project instructions live in shared documentation:".to_string(),
    ];
    lines.extend(sources.iter().map(|source| format!("- `{}`", source)));
    lines.push("".to_string());
    lines.push("Always:".to_string());
    lines.push("- Read the shared instructions before major edits.".to_string());
    lines.push("- Treat `manifest.toml` as the machine-readable source of truth for Docker-first workflow, commands, and guards.".to_string());
    lines.push(
        "- Prefer minimal diffs and run the smallest relevant verification before finishing."
            .to_string(),
    );
    if let Some(skills_source) = skills_source {
        lines.push(format!(
            "- Reuse task-specific playbooks from `{}` when the task matches.",
            skills_source
        ));
    }
    if let Some(hooks_policy) = hooks_policy {
        lines.push(format!(
            "- When working with hooks or guard automation, follow `{}` as the portable policy source.",
            hooks_policy
        ));
    }

    let testing_lines = render_testing_policy(testing);
    if !testing_lines.is_empty() {
        lines.push("".to_string());
        lines.push("Testing policy:".to_string());
        lines.extend(testing_lines);
    }

    lines.push("".to_string());
    lines.push(
        "Use MCP servers when available and prefer official documentation for vendor-specific behavior."
            .to_string(),
    );
    lines.push("".to_string());
    lines.join("\n")
}

fn render_claude_md(
    sources: &[String],
    skills_source: Option<&str>,
    hooks_policy: Option<&str>,
    mcp_servers: &[String],
    testing: &TestingSection,
) -> String {
    let mut lines = vec![
        "# CLAUDE.md".to_string(),
        "".to_string(),
        "This file is generated by `airis docs sync`. Keep this file short; the shared docs are the source of truth.".to_string(),
        "".to_string(),
        "Read these files first:".to_string(),
    ];
    lines.extend(sources.iter().map(|source| format!("- `{}`", source)));
    lines.push("".to_string());
    lines.push("Repository rules:".to_string());
    lines.push("- `manifest.toml` remains the source of truth for Docker-first orchestration, command guards, and generated workspace config.".to_string());
    lines.push(
        "- Follow the shared docs for architecture, workflow, and review expectations.".to_string(),
    );
    lines.push("- Keep verification proportional to the change.".to_string());
    if let Some(skills_source) = skills_source {
        lines.push(format!(
            "- Project playbooks and reusable task guidance live under `{}`.",
            skills_source
        ));
    }
    if let Some(hooks_policy) = hooks_policy {
        lines.push(format!(
            "- Hook intent and shared guard policy live in `{}`; Claude-specific hook wiring may extend it but should not contradict it.",
            hooks_policy
        ));
    }

    let testing_lines = render_testing_policy(testing);
    if !testing_lines.is_empty() {
        lines.push("".to_string());
        lines.push("Testing policy:".to_string());
        lines.extend(testing_lines);
    }

    if !mcp_servers.is_empty() {
        lines.push("".to_string());
        lines.push(format!("Active MCP servers: {}", mcp_servers.join(", ")));
    }
    lines.push("".to_string());
    lines.join("\n")
}

fn render_gemini_md(
    sources: &[String],
    skills_source: Option<&str>,
    hooks_policy: Option<&str>,
    testing: &TestingSection,
) -> String {
    let mut lines = vec![
        "# GEMINI.md".to_string(),
        "".to_string(),
        "<!-- Generated by `airis docs sync`. -->".to_string(),
        "".to_string(),
        "Primary project instructions:".to_string(),
    ];
    lines.extend(sources.iter().map(|source| format!("@./{}", source)));
    if let Some(skills_source) = skills_source {
        lines.push("".to_string());
        lines.push("Reusable playbooks:".to_string());
        lines.push(format!("@./{}", skills_source));
    }
    if let Some(hooks_policy) = hooks_policy {
        lines.push("".to_string());
        lines.push("Hook policy:".to_string());
        lines.push(format!("@./{}", hooks_policy));
    }

    let testing_lines = render_testing_policy(testing);
    if !testing_lines.is_empty() {
        lines.push("".to_string());
        lines.push("Testing policy:".to_string());
        lines.extend(testing_lines);
    }

    lines.push("".to_string());
    lines.push(
        "`manifest.toml` remains the machine-readable source of truth for Docker-first workflow, commands, and guards.".to_string(),
    );
    lines.join("\n")
}

fn render_cursorrules(
    sources: &[String],
    skills_source: Option<&str>,
    hooks_policy: Option<&str>,
    testing: &TestingSection,
) -> String {
    let mut lines = vec![
        "# .cursorrules".to_string(),
        "".to_string(),
        "This file is generated by `airis docs sync`.".to_string(),
        "".to_string(),
        "Shared project instructions:".to_string(),
    ];
    lines.extend(sources.iter().map(|source| format!("- `{}`", source)));
    if let Some(skills_source) = skills_source {
        lines.push(format!("- Task playbooks: `{}`", skills_source));
    }
    if let Some(hooks_policy) = hooks_policy {
        lines.push(format!("- Hook policy: `{}`", hooks_policy));
    }

    let testing_lines = render_testing_policy(testing);
    if !testing_lines.is_empty() {
        lines.push("".to_string());
        lines.push("Testing policy:".to_string());
        lines.extend(testing_lines);
    }

    lines.push("".to_string());
    lines.push(
        "`manifest.toml` stays authoritative for Docker-first workflow and guard configuration."
            .to_string(),
    );
    lines.join("\n")
}

fn render_testing_policy(testing: &TestingSection) -> Vec<String> {
    // If testing section is all defaults with no custom rules, emit nothing (backwards-compatible)
    if testing.mock_policy == MockPolicy::Allowed
        && testing.ai_rules.is_empty()
        && testing.forbidden_patterns.is_empty()
        && testing.type_enforcement.is_none()
        && testing.coverage.unit == 0
        && testing.coverage.integration == 0
    {
        return vec![];
    }

    let mut lines = Vec::new();

    match &testing.mock_policy {
        MockPolicy::Forbidden => {
            lines.push("- **Mock policy: forbidden** — Never mock external services (DB, APIs). Use real instances or local emulators.".to_string());
        }
        MockPolicy::UnitOnly => {
            lines.push("- **Mock policy: unit-only** — Mocks allowed in unit tests only. Integration and E2E tests must use real services.".to_string());
        }
        MockPolicy::Allowed => {}
    }

    if !testing.forbidden_patterns.is_empty() {
        lines.push(format!(
            "- Forbidden in integration/e2e test files: `{}`",
            testing.forbidden_patterns.join("`, `")
        ));
    }

    if let Some(te) = &testing.type_enforcement {
        lines.push(format!(
            "- DB-touching tests must import generated types from `{}`.",
            te.generated_types_path
        ));
    }

    if testing.coverage.unit > 0 || testing.coverage.integration > 0 {
        let mut parts = Vec::new();
        if testing.coverage.unit > 0 {
            parts.push(format!("unit >= {}%", testing.coverage.unit));
        }
        if testing.coverage.integration > 0 {
            parts.push(format!("integration >= {}%", testing.coverage.integration));
        }
        lines.push(format!("- Coverage targets: {}", parts.join(", ")));
    }

    for rule in &testing.ai_rules {
        lines.push(format!("- {}", rule));
    }

    lines
}

fn vendor_for_target(target: &str) -> Result<DocsVendor> {
    match target {
        "AGENTS.md" | ".cursorrules" => Ok(DocsVendor::Codex),
        "CLAUDE.md" => Ok(DocsVendor::Claude),
        "GEMINI.md" => Ok(DocsVendor::Gemini),
        _ => bail!("Unsupported target: {}", target),
    }
}

fn targets_for_vendor(vendor: &DocsVendor) -> &'static [&'static str] {
    match vendor {
        DocsVendor::Codex => &["AGENTS.md"],
        DocsVendor::Claude => &["CLAUDE.md"],
        DocsVendor::Gemini => &["GEMINI.md"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static DIR_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn render_gemini_uses_imports() {
        let manifest = manifest_with_docs();
        let rendered = render_adapter(&manifest, "GEMINI.md").unwrap();

        assert!(rendered.contains("@./docs/ai/PROJECT_RULES.md"));
        assert!(rendered.contains("@./docs/ai/playbooks"));
    }

    #[test]
    fn sync_generates_configured_vendor_adapters() {
        let _guard = DIR_LOCK.lock().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let dir = tempdir().unwrap();

        fs::create_dir_all(dir.path().join("docs/ai/playbooks")).unwrap();
        fs::create_dir_all(dir.path().join("docs/ai/hooks")).unwrap();
        fs::write(
            dir.path().join("docs/ai/PROJECT_RULES.md"),
            "# Project rules\n",
        )
        .unwrap();
        fs::write(dir.path().join("docs/ai/WORKFLOW.md"), "# Workflow\n").unwrap();
        fs::write(dir.path().join("docs/ai/REVIEW.md"), "# Review\n").unwrap();
        fs::write(dir.path().join("docs/ai/STACK.md"), "# Stack\n").unwrap();
        fs::write(
            dir.path().join("docs/ai/playbooks/README.md"),
            "# Playbooks\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("docs/ai/hooks/HOOKS_POLICY.md"),
            "# Hooks policy\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("manifest.toml"),
            r#"
version = 1
mode = "docker-first"

[project]
id = "demo"
binary_name = "demo"
description = "demo"
authors = ["demo"]
license = "MIT"
homepage = "https://example.com"
repository = "https://example.com/demo"
keywords = []
categories = []
rust_edition = "2024"

[workspace]
name = "demo"
package_manager = "pnpm@10"
service = "workspace"
image = "node:22"
workdir = "/app"
volumes = []

[catalog]

[workspaces]
apps = []
libs = []

[dev]
apps = []
autostart = []

[docker]
baseImage = "node:22"
workdir = "/app"

[docker.workspace]
service = "workspace"
volumes = []

[just]
output = "justfile"
features = []

[packages]
workspaces = []
app = []

[packages.catalog]

[packages.root.dependencies]

[packages.root.devDependencies]

[packages.root.optionalDependencies]

[packages.root.scripts]

[packages.root.engines]

[packages.root.pnpm]
onlyBuiltDependencies = []

[packages.root.pnpm.overrides]

[packages.root.pnpm.peerDependencyRules]
ignoreMissing = []

[packages.root.pnpm.peerDependencyRules.allowedVersions]

[packages.root.pnpm.allowedScripts]

[guards]
deny = []
forbid = []
danger = []

[guards.wrap]

[guards.deny_with_message]

[orchestration.dev]

[commands]

[remap]

[versioning]
strategy = "manual"
source = "0.1.0"

[docs]
mode = "backup"
vendors = ["codex", "claude", "gemini"]
sources = [
  "docs/ai/PROJECT_RULES.md",
  "docs/ai/WORKFLOW.md",
  "docs/ai/REVIEW.md",
  "docs/ai/STACK.md",
]
skills_source = "docs/ai/playbooks"
hooks_policy = "docs/ai/hooks/HOOKS_POLICY.md"

[ci]
enabled = true
auto_version = true

[ci.auto_merge]
enabled = true
from = "next"
to = "main"
"#,
        )
        .unwrap();

        std::env::set_current_dir(dir.path()).unwrap();
        let result = std::panic::catch_unwind(|| sync(false));
        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap().unwrap();

        assert!(dir.path().join("AGENTS.md").exists());
        assert!(dir.path().join("CLAUDE.md").exists());
        assert!(dir.path().join("GEMINI.md").exists());
    }

    fn manifest_with_docs() -> Manifest {
        let mut manifest = Manifest::default_with_project("demo");
        manifest.docs.sources = DEFAULT_SOURCE_FILES.iter().map(|s| s.to_string()).collect();
        manifest.docs.skills_source = Some(DEFAULT_SKILLS_SOURCE.to_string());
        manifest.docs.hooks_policy = Some(DEFAULT_HOOKS_POLICY.to_string());
        manifest
    }

    // ── render_testing_policy tests ──

    #[test]
    fn testing_policy_empty_when_all_defaults_with_allowed() {
        // Override to Allowed — should produce no output
        let testing = TestingSection {
            mock_policy: MockPolicy::Allowed,
            ..Default::default()
        };
        let lines = render_testing_policy(&testing);
        assert!(
            lines.is_empty(),
            "Expected no output for all-default + Allowed testing section"
        );
    }

    #[test]
    fn testing_policy_forbidden_renders_mock_rule() {
        let testing = TestingSection {
            mock_policy: MockPolicy::Forbidden,
            ..Default::default()
        };
        let lines = render_testing_policy(&testing);
        assert!(!lines.is_empty());
        assert!(lines[0].contains("Mock policy: forbidden"));
    }

    #[test]
    fn testing_policy_unit_only_renders_mock_rule() {
        let testing = TestingSection {
            mock_policy: MockPolicy::UnitOnly,
            ..Default::default()
        };
        let lines = render_testing_policy(&testing);
        assert!(lines[0].contains("Mock policy: unit-only"));
    }

    #[test]
    fn testing_policy_renders_forbidden_patterns() {
        let testing = TestingSection {
            mock_policy: MockPolicy::Allowed,
            forbidden_patterns: vec!["vi\\.mock.*supabase".to_string()],
            ..Default::default()
        };
        let lines = render_testing_policy(&testing);
        assert!(lines.iter().any(|l| l.contains("vi\\.mock.*supabase")));
    }

    #[test]
    fn testing_policy_renders_type_enforcement() {
        use crate::manifest::TypeEnforcement;
        let testing = TestingSection {
            mock_policy: MockPolicy::Allowed,
            type_enforcement: Some(TypeEnforcement {
                generated_types_path: "libs/db/types.ts".to_string(),
                required_imports: vec![],
            }),
            ..Default::default()
        };
        let lines = render_testing_policy(&testing);
        assert!(lines.iter().any(|l| l.contains("libs/db/types.ts")));
    }

    #[test]
    fn testing_policy_renders_coverage_targets() {
        use crate::manifest::TestingCoverage;
        let testing = TestingSection {
            mock_policy: MockPolicy::Allowed,
            coverage: TestingCoverage {
                unit: 80,
                integration: 60,
            },
            ..Default::default()
        };
        let lines = render_testing_policy(&testing);
        assert!(
            lines
                .iter()
                .any(|l| l.contains("unit >= 80%") && l.contains("integration >= 60%"))
        );
    }

    #[test]
    fn testing_policy_renders_custom_ai_rules() {
        let testing = TestingSection {
            mock_policy: MockPolicy::Allowed,
            ai_rules: vec!["Never mock Supabase.".to_string()],
            ..Default::default()
        };
        let lines = render_testing_policy(&testing);
        assert!(lines.iter().any(|l| l.contains("Never mock Supabase.")));
    }

    #[test]
    fn testing_policy_injected_into_claude_md() {
        let mut manifest = manifest_with_docs();
        manifest.policy.testing.mock_policy = MockPolicy::Forbidden;
        manifest.policy.testing.ai_rules = vec!["Use real DB.".to_string()];
        let rendered = render_adapter(&manifest, "CLAUDE.md").unwrap();
        assert!(rendered.contains("Testing policy:"));
        assert!(rendered.contains("Mock policy: forbidden"));
        assert!(rendered.contains("Use real DB."));
    }

    #[test]
    fn testing_policy_injected_into_agents_md() {
        let mut manifest = manifest_with_docs();
        manifest.policy.testing.mock_policy = MockPolicy::UnitOnly;
        let rendered = render_adapter(&manifest, "AGENTS.md").unwrap();
        assert!(rendered.contains("Testing policy:"));
        assert!(rendered.contains("Mock policy: unit-only"));
    }

    #[test]
    fn testing_policy_injected_into_gemini_md() {
        let mut manifest = manifest_with_docs();
        manifest.policy.testing.ai_rules = vec!["No mocks.".to_string()];
        let rendered = render_adapter(&manifest, "GEMINI.md").unwrap();
        assert!(rendered.contains("Testing policy:"));
        assert!(rendered.contains("No mocks."));
    }

    #[test]
    fn testing_policy_not_in_claude_md_when_all_defaults_allowed() {
        let mut manifest = manifest_with_docs();
        manifest.policy.testing.mock_policy = MockPolicy::Allowed;
        let rendered = render_adapter(&manifest, "CLAUDE.md").unwrap();
        assert!(!rendered.contains("Testing policy:"));
    }

    #[test]
    fn handle_existing_file_force_overwrites_in_warn_mode() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("CLAUDE.md");
        fs::write(&target, "stale").unwrap();

        let mut manifest = manifest_with_docs();
        manifest.docs.mode = DocsMode::Warn;

        // Without force: bail.
        assert!(handle_existing_file(&manifest, &target, false).is_err());

        // With force: succeed without leaving a `.bak`.
        handle_existing_file(&manifest, &target, true).unwrap();
        assert!(!dir.path().join("CLAUDE.md.bak").exists());
    }

    #[test]
    fn handle_existing_file_backup_mode_writes_bak() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("CLAUDE.md");
        fs::write(&target, "stale").unwrap();

        let mut manifest = manifest_with_docs();
        manifest.docs.mode = DocsMode::Backup;

        handle_existing_file(&manifest, &target, false).unwrap();
        assert!(dir.path().join("CLAUDE.md.bak").exists());
    }
}
