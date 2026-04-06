use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

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
