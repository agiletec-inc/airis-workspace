use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate native hooks (.airis/hooks/pre-commit, .airis/hooks/pre-push) for `airis hooks install`.
pub(super) fn generate_native_hooks() -> Result<()> {
    let hooks_dir = Path::new(".airis/hooks");

    fs::create_dir_all(hooks_dir).context("Failed to create .airis/hooks directory")?;

    let pre_commit_content = crate::commands::hooks::pre_commit_script();
    let pre_push_content = crate::commands::hooks::pre_push_script();

    let pre_commit_path = hooks_dir.join("pre-commit");
    let pre_push_path = hooks_dir.join("pre-push");

    fs::write(&pre_commit_path, pre_commit_content)
        .with_context(|| "Failed to write .airis/hooks/pre-commit")?;
    fs::write(&pre_push_path, pre_push_content)
        .with_context(|| "Failed to write .airis/hooks/pre-push")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&pre_commit_path, fs::Permissions::from_mode(0o755))
            .with_context(|| "Failed to set .airis/hooks/pre-commit permissions")?;
        fs::set_permissions(&pre_push_path, fs::Permissions::from_mode(0o755))
            .with_context(|| "Failed to set .airis/hooks/pre-push permissions")?;
    }

    println!(
        "   {} Updated .airis/hooks/ implementation",
        "🔒".green()
    );

    Ok(())
}
