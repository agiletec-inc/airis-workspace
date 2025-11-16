use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

const PRE_COMMIT_HOOK: &str = include_str!("../../hooks/pre-commit");

/// Install Git hooks for version auto-increment
pub fn install() -> Result<()> {
    let git_dir = Path::new(".git");

    if !git_dir.exists() {
        eprintln!("❌ Not a git repository. Skipping hook installation.");
        return Ok(());
    }

    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)
        .with_context(|| "Failed to create .git/hooks directory")?;

    let pre_commit_path = hooks_dir.join("pre-commit");

    // Write pre-commit hook
    fs::write(&pre_commit_path, PRE_COMMIT_HOOK)
        .with_context(|| "Failed to write pre-commit hook")?;

    // Make executable (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&pre_commit_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&pre_commit_path, perms)?;
    }

    println!("✅ Git hooks installed successfully!");
    println!(
        "   {} → {}",
        "pre-commit".green(),
        pre_commit_path.display()
    );
    println!("\n💡 Version will auto-bump on every commit based on commit message.");

    Ok(())
}
