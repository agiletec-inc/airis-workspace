use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

const PRE_COMMIT_HOOK: &str = include_str!("../../hooks/pre-commit");
const PRE_PUSH_HOOK: &str = include_str!("../../hooks/pre-push");

/// Install a single Git hook file with executable permissions
fn install_hook(hooks_dir: &Path, name: &str, content: &str) -> Result<()> {
    let hook_path = hooks_dir.join(name);

    fs::write(&hook_path, content)
        .with_context(|| format!("Failed to write {} hook", name))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)?;
    }

    Ok(())
}

/// Install Git hooks (pre-commit + pre-push)
pub fn install() -> Result<()> {
    let git_dir = Path::new(".git");

    if !git_dir.exists() {
        eprintln!("❌ Not a git repository. Skipping hook installation.");
        return Ok(());
    }

    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)
        .with_context(|| "Failed to create .git/hooks directory")?;

    install_hook(&hooks_dir, "pre-commit", PRE_COMMIT_HOOK)?;
    install_hook(&hooks_dir, "pre-push", PRE_PUSH_HOOK)?;

    println!("✅ Git hooks installed successfully!");
    println!(
        "   {} → {}",
        "pre-commit".green(),
        hooks_dir.join("pre-commit").display()
    );
    println!(
        "   {} → {}",
        "pre-push".green(),
        hooks_dir.join("pre-push").display()
    );
    println!("\n💡 pre-commit: version auto-bump + .env / node_modules guard");
    println!("💡 pre-push:   lint / typecheck / build (commented out — enable as needed)");

    Ok(())
}
