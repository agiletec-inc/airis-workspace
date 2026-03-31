use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::templates::TemplateEngine;

pub(super) fn generate_git_hooks(_engine: &TemplateEngine) -> Result<()> {
    let husky_dir = Path::new(".husky");
    fs::create_dir_all(husky_dir).context("Failed to create .husky directory")?;

    let pre_commit_content = include_str!("../../../hooks/pre-commit");
    let pre_push_content = include_str!("../../../hooks/pre-push");

    // Pre-commit hook
    let pre_commit_path = husky_dir.join("pre-commit");
    fs::write(&pre_commit_path, pre_commit_content)
        .with_context(|| "Failed to write .husky/pre-commit")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&pre_commit_path, fs::Permissions::from_mode(0o755))
            .with_context(|| "Failed to set .husky/pre-commit permissions")?;
    }

    // Pre-push hook
    let pre_push_path = husky_dir.join("pre-push");
    fs::write(&pre_push_path, pre_push_content)
        .with_context(|| "Failed to write .husky/pre-push")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&pre_push_path, fs::Permissions::from_mode(0o755))
            .with_context(|| "Failed to set .husky/pre-push permissions")?;
    }

    println!(
        "   {} Generated .husky/pre-commit and .husky/pre-push",
        "🔒".green()
    );

    Ok(())
}

/// Generate native hooks (hooks/pre-commit, hooks/pre-push) for `airis hooks install`.
/// Skips if the hooks/ directory already exists (preserves user customizations).
pub(super) fn generate_native_hooks() -> Result<()> {
    let hooks_dir = Path::new("hooks");

    if hooks_dir.exists() {
        println!(
            "   {} hooks/ directory exists, skipping (user customizations preserved)",
            "⏭️".cyan()
        );
        return Ok(());
    }

    fs::create_dir_all(hooks_dir).context("Failed to create hooks/ directory")?;

    let pre_commit_content = include_str!("../../../hooks/pre-commit");
    let pre_push_content = include_str!("../../../hooks/pre-push");

    let pre_commit_path = hooks_dir.join("pre-commit");
    let pre_push_path = hooks_dir.join("pre-push");

    fs::write(&pre_commit_path, pre_commit_content)
        .with_context(|| "Failed to write hooks/pre-commit")?;
    fs::write(&pre_push_path, pre_push_content)
        .with_context(|| "Failed to write hooks/pre-push")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&pre_commit_path, fs::Permissions::from_mode(0o755))
            .with_context(|| "Failed to set hooks/pre-commit permissions")?;
        fs::set_permissions(&pre_push_path, fs::Permissions::from_mode(0o755))
            .with_context(|| "Failed to set hooks/pre-push permissions")?;
    }

    println!(
        "   {} Generated hooks/pre-commit and hooks/pre-push",
        "🔒".green()
    );
    println!(
        "   {} Run `airis hooks install` to install them to .git/hooks/",
        "💡".cyan()
    );

    Ok(())
}
