use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::manifest::Manifest;

const BEGIN_BLOCK: &str = "<!-- BEGIN GENERATED airis gen -->";
const END_BLOCK: &str = "<!-- END GENERATED -->";

/// Sync AI tool rules from shared sources to tool-specific targets.
///
/// This implements the Single Source of Truth (SSOT) for AI rules as specified
/// in IDEAL_STATE.md §5. It manages CLAUDE.md, AGENTS.md, GEMINI.md,
/// and individual rule files for Cursor and Claude.
pub fn sync_ai_rules(manifest: &Manifest, generated_paths: &mut Vec<String>) -> Result<()> {
    if manifest.ai.shared_rules.is_empty() {
        return Ok(());
    }

    // 1. Claude Code
    if let Some(claude) = &manifest.ai.claude {
        generate_vendor_target(
            &claude.target,
            &manifest.ai.shared_rules,
            "claude",
            generated_paths,
        )?;
        sync_individual_rules(
            &claude.rules_dir,
            &manifest.ai.shared_rules,
            generated_paths,
        )?;
    }

    // 2. Codex (AGENTS.md)
    if let Some(codex) = &manifest.ai.codex {
        generate_vendor_target(
            &codex.target,
            &manifest.ai.shared_rules,
            "codex",
            generated_paths,
        )?;
    }

    // 3. Gemini (GEMINI.md)
    if let Some(gemini) = &manifest.ai.gemini {
        generate_vendor_target(
            &gemini.target,
            &manifest.ai.shared_rules,
            "gemini",
            generated_paths,
        )?;
    }

    // 4. Cursor (.cursor/rules/)
    if let Some(cursor) = &manifest.ai.cursor {
        sync_individual_rules(
            &cursor.rules_dir,
            &manifest.ai.shared_rules,
            generated_paths,
        )?;
    }

    Ok(())
}

fn resolve_path(path_str: &str) -> Result<PathBuf> {
    if let Some(rest) = path_str.strip_prefix("~/") {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(rest))
    } else if let Some(rest) = path_str.strip_prefix('~') {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(rest))
    } else {
        // Note: We don't strip leading / here because it would break absolute paths
        // used in tests (tempdir). Users should use relative paths in manifest.toml
        // for repo-internal targets (e.g., ".claude/CLAUDE.md" instead of "/.claude/CLAUDE.md").
        Ok(PathBuf::from(path_str))
    }
}

fn generate_vendor_target(
    target_path_str: &str,
    sources: &[String],
    vendor: &str,
    generated_paths: &mut Vec<String>,
) -> Result<()> {
    let target_path = resolve_path(target_path_str)?;

    let generated_content = render_combined_sources(sources, vendor)?;

    let full_content = if target_path.exists() {
        let existing = fs::read_to_string(&target_path)?;
        update_generated_block(&existing, &generated_content)?
    } else {
        format!("{}\n{}\n{}\n", BEGIN_BLOCK, generated_content, END_BLOCK)
    };

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&target_path, full_content)
        .with_context(|| format!("Failed to write {}", target_path.display()))?;
    generated_paths.push(target_path_str.to_string());

    println!(
        "   {} Syncing {} ({} sources)",
        "→".dimmed(),
        target_path_str.cyan(),
        sources.len()
    );

    Ok(())
}

fn render_combined_sources(sources: &[String], _vendor: &str) -> Result<String> {
    let mut lines = Vec::new();

    lines.push("## Shared Rules (Auto-generated)".to_string());
    lines.push("Primary project instructions. Read these first.".to_string());
    lines.push(String::new());

    for source in sources {
        if Path::new(source).exists() {
            let content = fs::read_to_string(source)?;
            lines.push(format!("### Source: {}", source));
            lines.push(String::new());
            lines.push(content);
            lines.push(String::new());
        } else {
            println!(
                "   {} Rule source not found: {}",
                "⚠️".yellow(),
                source.dimmed()
            );
        }
    }

    Ok(lines.join("\n"))
}

fn update_generated_block(existing: &str, new_gen: &str) -> Result<String> {
    let begin_idx = existing.find(BEGIN_BLOCK);
    let end_idx = existing.find(END_BLOCK);

    match (begin_idx, end_idx) {
        (Some(start), Some(end)) => {
            let mut result = existing[..start].to_string();
            result.push_str(BEGIN_BLOCK);
            result.push('\n');
            result.push_str(new_gen);
            result.push('\n');
            result.push_str(END_BLOCK);
            result.push_str(&existing[end + END_BLOCK.len()..]);
            Ok(result)
        }
        _ => {
            // Block not found, prepend it at the top
            let mut result = format!("{}\n{}\n{}\n\n", BEGIN_BLOCK, new_gen, END_BLOCK);
            result.push_str(existing);
            Ok(result)
        }
    }
}

fn sync_individual_rules(
    rules_dir_str: &str,
    sources: &[String],
    generated_paths: &mut Vec<String>,
) -> Result<()> {
    let rules_dir = resolve_path(rules_dir_str)?;

    fs::create_dir_all(&rules_dir)
        .with_context(|| format!("Failed to create {}", rules_dir.display()))?;

    for source in sources {
        let source_path = Path::new(source);
        if !source_path.exists() {
            continue;
        }

        let file_name = source_path.file_name().unwrap();
        let target_path = rules_dir.join(file_name);

        fs::copy(source_path, &target_path)?;
        generated_paths.push(format!(
            "{}/{}",
            rules_dir_str.trim_end_matches('/'),
            file_name.to_string_lossy()
        ));
    }

    println!(
        "   {} Generated rules in {}",
        "→".dimmed(),
        rules_dir_str.cyan()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ClaudeAIConfig;
    use tempfile::tempdir;

    #[test]
    fn test_update_generated_block_new() {
        let existing = "# Manual Header\n\nSome manual content.";
        let new_gen = "## New Rules\nRule 1\nRule 2";
        let updated = update_generated_block(existing, new_gen).unwrap();

        assert!(updated.starts_with(BEGIN_BLOCK));
        assert!(updated.contains(new_gen));
        assert!(updated.contains(END_BLOCK));
        assert!(updated.contains("# Manual Header"));
        assert!(updated.contains("Some manual content."));
    }

    #[test]
    fn test_update_generated_block_existing() {
        let existing = format!(
            "# Header\n\n{}\nOld Rules\n{}\n\n# Footer\nManual content.",
            BEGIN_BLOCK, END_BLOCK
        );
        let new_gen = "New Rules Content";
        let updated = update_generated_block(&existing, new_gen).unwrap();

        assert!(updated.contains("# Header"));
        assert!(updated.contains(BEGIN_BLOCK));
        assert!(updated.contains(new_gen));
        assert!(!updated.contains("Old Rules"));
        assert!(updated.contains(END_BLOCK));
        assert!(updated.contains("# Footer"));
        assert!(updated.contains("Manual content."));
    }

    #[test]
    fn test_sync_ai_rules_idempotent() -> Result<()> {
        let dir = tempdir()?;
        let source_file = dir.path().join("rules.md");
        fs::write(&source_file, "Source rule content")?;

        let target_file = dir.path().join("CLAUDE.md");
        fs::write(&target_file, "# Manual Title\n")?;

        let mut manifest = Manifest::default_with_project("test");
        manifest.ai.shared_rules = vec![source_file.to_string_lossy().to_string()];
        manifest.ai.claude = Some(ClaudeAIConfig {
            target: target_file.to_string_lossy().to_string(),
            rules_dir: dir.path().join("rules").to_string_lossy().to_string(),
        });

        let mut generated_paths = Vec::new();
        sync_ai_rules(&manifest, &mut generated_paths)?;

        let content1 = fs::read_to_string(&target_file)?;
        assert!(content1.contains("Source rule content"));
        assert!(content1.contains("# Manual Title"));

        // Second run with updated source
        fs::write(&source_file, "Updated source rule content")?;
        sync_ai_rules(&manifest, &mut generated_paths)?;

        let content2 = fs::read_to_string(&target_file)?;
        assert!(content2.contains("Updated source rule content"));
        assert!(!content2.contains("Source rule content"));
        assert!(content2.contains("# Manual Title"));

        Ok(())
    }
}
