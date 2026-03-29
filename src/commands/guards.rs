use std::fs;
use std::io::BufRead;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use colored::Colorize;

use crate::manifest::{GlobalConfig, MANIFEST_FILE, Manifest, Mode};

/// Additional commands blocked in strict mode
const STRICT_MODE_DENY: &[&str] = &[
    "cargo", "rustc", "python", "python3", "pip", "pip3", "uv", "go", "java", "javac", "gradle",
    "mvn",
];

/// Commands allowed in hybrid mode (local builds)
const HYBRID_MODE_ALLOW: &[&str] = &["cargo", "rustc", "python", "python3", "pip", "pip3", "uv"];

const GUARDS_DIR: &str = ".airis/bin";

/// Install command guards from manifest.toml
pub fn install() -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);

    if !manifest_path.exists() {
        anyhow::bail!("manifest.toml not found. Run `airis init` first.");
    }

    let manifest = Manifest::load(manifest_path)?;

    // Collect effective deny list based on mode
    let mut effective_deny: Vec<String> = manifest.guards.deny.clone();

    // Apply mode-specific guards
    match &manifest.mode {
        Mode::DockerFirst => {
            // Standard mode: use guards as configured
            println!("{}", "Mode: docker-first (standard guards)".dimmed());
        }
        Mode::Hybrid => {
            // Hybrid mode: allow local toolchains (cargo, python, etc.)
            println!("{}", "Mode: hybrid (allowing local toolchains)".yellow());
            effective_deny.retain(|cmd| !HYBRID_MODE_ALLOW.contains(&cmd.as_str()));
        }
        Mode::Strict => {
            // Strict mode: block additional commands
            println!("{}", "Mode: strict (maximum restrictions)".bright_red());
            for cmd in STRICT_MODE_DENY {
                if !effective_deny.contains(&cmd.to_string()) {
                    effective_deny.push(cmd.to_string());
                }
            }
        }
    }

    if effective_deny.is_empty()
        && manifest.guards.wrap.is_empty()
        && manifest.guards.deny_with_message.is_empty()
    {
        println!(
            "{}",
            "⚠️  No guards to install (all commands allowed in this mode)".yellow()
        );
        return Ok(());
    }

    println!("{}", "🛡️  Installing command guards...".bright_blue());
    println!();

    let guards_dir = PathBuf::from(GUARDS_DIR);
    fs::create_dir_all(&guards_dir).with_context(|| format!("Failed to create {}", GUARDS_DIR))?;

    let mut installed_count = 0;

    // Install deny guards (mode-adjusted)
    for cmd in &effective_deny {
        install_deny_guard(&guards_dir, cmd, None)?;
        installed_count += 1;
        println!("   {} {}", "✓".green(), format!("{} (deny)", cmd).dimmed());
    }

    // Install wrap guards
    for (cmd, wrapper) in &manifest.guards.wrap {
        install_wrap_guard(&guards_dir, cmd, wrapper)?;
        installed_count += 1;
        println!(
            "   {} {}",
            "✓".green(),
            format!("{} → {}", cmd, wrapper).dimmed()
        );
    }

    // Install deny with message guards
    for (cmd, message) in &manifest.guards.deny_with_message {
        install_deny_guard(&guards_dir, cmd, Some(message))?;
        installed_count += 1;
        println!(
            "   {} {}",
            "✓".green(),
            format!("{} (deny with message)", cmd).dimmed()
        );
    }

    println!();
    println!(
        "{}",
        format!("✅ {} command guards installed", installed_count).green()
    );
    println!();
    println!("{}", "To activate guards:".bright_yellow());
    println!("  export PATH=\"$PWD/{}:$PATH\"", GUARDS_DIR);
    println!();
    println!("{}", "Or use airis shell:".bright_yellow());
    println!("  airis shell");

    Ok(())
}

/// Ensure a guard script path is safe to write to.
///
/// Rejects symlinks and non-regular files to prevent symlink attacks where
/// an attacker replaces a guard script with a symlink to a malicious binary.
/// Returns the validated path, or an error if the path is unsafe.
fn ensure_safe_guard_path(path: &Path) -> Result<()> {
    // Check symlink BEFORE following it (lstat, not stat)
    if path.is_symlink() {
        let target = fs::read_link(path)
            .map(|t| t.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        // Remove the symlink to prevent execution
        fs::remove_file(path)
            .with_context(|| format!("Failed to remove symlink at {}", path.display()))?;
        eprintln!(
            "🚨 {} Symlink detected at {} → {}. Removed for security.",
            "SECURITY".red().bold(),
            path.display(),
            target
        );
    } else if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        let file_type = metadata.file_type();
        if !file_type.is_file() {
            bail!(
                "Guard path {} is not a regular file (type: {:?}). Refusing to overwrite.",
                path.display(),
                file_type
            );
        }
    }
    Ok(())
}

fn install_deny_guard(guards_dir: &Path, cmd: &str, custom_message: Option<&String>) -> Result<()> {
    let script_path = guards_dir.join(cmd);
    ensure_safe_guard_path(&script_path)?;

    let message = if let Some(msg) = custom_message {
        msg.clone()
    } else {
        format!(
            "'{}' is denied by manifest.toml [guards.deny].\n\nUse Docker-first workflow instead:\n  airis shell     # Enter container\n  {}              # Run inside container",
            cmd, cmd
        )
    };

    let content = format!(
        r#"#!/usr/bin/env bash
# Auto-generated by airis guards install
# DO NOT EDIT - managed by manifest.toml [guards] section

echo "❌ ERROR: {}"
exit 1
"#,
        message.replace("\"", "\\\"")
    );

    fs::write(&script_path, content)
        .with_context(|| format!("Failed to write guard script for '{}'", cmd))?;

    make_executable(&script_path)?;

    Ok(())
}

fn install_wrap_guard(guards_dir: &Path, cmd: &str, wrapper: &str) -> Result<()> {
    let script_path = guards_dir.join(cmd);
    ensure_safe_guard_path(&script_path)?;

    let content = format!(
        r#"#!/usr/bin/env bash
# Auto-generated by airis guards install
# DO NOT EDIT - managed by manifest.toml [guards.wrap] section

# Wrapper: {} → {}
exec {} "$@"
"#,
        cmd, wrapper, wrapper
    );

    fs::write(&script_path, content)
        .with_context(|| format!("Failed to write guard script for '{}'", cmd))?;

    make_executable(&script_path)?;

    Ok(())
}

fn make_executable(_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(_path)?.permissions();
        perms.set_mode(0o755); // rwxr-xr-x
        fs::set_permissions(_path, perms)?;
    }
    Ok(())
}

/// Check if running inside Docker container (respects mode)
pub fn check_docker() -> Result<()> {
    // Load manifest to check mode
    let manifest_path = Path::new(MANIFEST_FILE);
    let mode = if manifest_path.exists() {
        Manifest::load(manifest_path)
            .map(|m| m.mode)
            .unwrap_or_default()
    } else {
        Mode::default()
    };

    // Hybrid mode allows host execution
    if matches!(mode, Mode::Hybrid) {
        println!("{}", "✅ Hybrid mode: host execution allowed".green());
        return Ok(());
    }

    println!("{}", "🔍 Checking Docker environment...".bright_blue());

    // Check DOCKER_CONTAINER environment variable
    if std::env::var("DOCKER_CONTAINER").unwrap_or_default() == "true" {
        println!(
            "{}",
            "✅ Running inside Docker container (DOCKER_CONTAINER=true)".green()
        );
        return Ok(());
    }

    // Check for /.dockerenv file
    if Path::new("/.dockerenv").exists() {
        println!(
            "{}",
            "✅ Running inside Docker container (/.dockerenv exists)".green()
        );
        return Ok(());
    }

    // Check /proc/1/cgroup for Docker
    if let Ok(content) = fs::read_to_string("/proc/1/cgroup")
        && (content.contains("docker") || content.contains("containerd"))
    {
        println!(
            "{}",
            "✅ Running inside Docker container (cgroup detected)".green()
        );
        return Ok(());
    }

    // Check for CI environment
    if std::env::var("CI").unwrap_or_default() == "true"
        || std::env::var("GITHUB_ACTIONS").unwrap_or_default() == "true"
        || std::env::var("GITLAB_CI").unwrap_or_default() == "true"
    {
        println!("{}", "✅ Running in CI environment".green());
        return Ok(());
    }

    // Not in Docker - show error
    println!();
    println!("{}", "=".repeat(70).red());
    println!(
        "{}",
        "❌ CRITICAL ERROR: Not running inside Docker container"
            .red()
            .bold()
    );
    println!("{}", "=".repeat(70).red());
    println!();
    println!("{}", "【問題】".bright_yellow());
    println!("  Mac ホスト上で実行しようとしています。");
    println!("  Docker-First開発では、全てのコマンドはDocker内で実行する必要があります。");
    println!();
    println!("{}", "【正しい使用方法】".bright_yellow());
    println!(
        "  1. {} # Docker ワークスペースに入る",
        "airis shell".cyan()
    );
    println!("  2. コマンドを実行");
    println!();
    println!("{}", "【または】".bright_yellow());
    println!(
        "  {} # コンテナ内で直接実行",
        "airis exec workspace <command>".cyan()
    );
    println!();
    println!("{}", "【ヒント】".bright_yellow());
    println!(
        "  ホストでの実行を許可するには、manifest.toml で mode = \"hybrid\" を設定してください"
    );
    println!();
    println!("{}", "=".repeat(70).red());

    anyhow::bail!("Not running inside Docker container");
}

/// Show guard status
pub fn status() -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);

    if !manifest_path.exists() {
        println!("{}", "⚠️  manifest.toml not found".yellow());
        return Ok(());
    }

    let manifest = Manifest::load(manifest_path)?;
    let guards_dir = PathBuf::from(GUARDS_DIR);

    println!("{}", "🛡️  Guard Status".bright_blue());
    println!();

    // Check if guards directory exists
    if !guards_dir.exists() {
        println!(
            "{}",
            "Guards not installed. Run: airis guards install".yellow()
        );
        return Ok(());
    }

    // Show deny guards
    if !manifest.guards.deny.is_empty() {
        println!("{}", "Deny guards:".bright_yellow());
        for cmd in &manifest.guards.deny {
            let guard_path = guards_dir.join(cmd);
            if guard_path.is_symlink() {
                println!("  {} {} (SYMLINK — security risk!)", "🚨".red(), cmd);
            } else if guard_path.exists() {
                println!("  {} {}", "✓".green(), cmd);
            } else {
                println!("  {} {}", "✗".red(), cmd);
            }
        }
        println!();
    }

    // Show wrap guards
    if !manifest.guards.wrap.is_empty() {
        println!("{}", "Wrap guards:".bright_yellow());
        for (cmd, wrapper) in &manifest.guards.wrap {
            let guard_path = guards_dir.join(cmd);
            if guard_path.is_symlink() {
                println!(
                    "  {} {} → {} (SYMLINK — security risk!)",
                    "🚨".red(),
                    cmd,
                    wrapper.dimmed()
                );
            } else if guard_path.exists() {
                println!("  {} {} → {}", "✓".green(), cmd, wrapper.dimmed());
            } else {
                println!("  {} {} → {}", "✗".red(), cmd, wrapper.dimmed());
            }
        }
        println!();
    }

    // Show deny with message
    if !manifest.guards.deny_with_message.is_empty() {
        println!("{}", "Deny with message:".bright_yellow());
        for (cmd, _) in &manifest.guards.deny_with_message {
            let guard_path = guards_dir.join(cmd);
            let status = if guard_path.exists() {
                "✓".green()
            } else {
                "✗".red()
            };
            println!("  {} {}", status, cmd);
        }
    }

    Ok(())
}

/// Uninstall local guards
pub fn uninstall() -> Result<()> {
    let guards_dir = PathBuf::from(GUARDS_DIR);

    if !guards_dir.exists() {
        println!(
            "{}",
            "⚠️  Guards not installed (no .airis/bin directory)".yellow()
        );
        return Ok(());
    }

    println!("{}", "🗑️  Uninstalling local guards...".bright_blue());
    println!();

    let mut removed_count = 0;

    // Remove only files that have the airis guard marker
    for entry in fs::read_dir(&guards_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && is_airis_guard(&path)? {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            fs::remove_file(&path)?;
            println!(
                "   {} {}",
                "✓".green(),
                format!("Removed {}", name).dimmed()
            );
            removed_count += 1;
        }
    }

    // Remove directory if empty
    if guards_dir.read_dir()?.next().is_none() {
        fs::remove_dir(&guards_dir)?;
        println!(
            "   {} {}",
            "✓".green(),
            "Removed .airis/bin directory".dimmed()
        );
    }

    println!();
    println!(
        "{}",
        format!("✅ {} guard(s) uninstalled", removed_count).green()
    );

    Ok(())
}

// =============================================================================
// Global Guards (~/.airis/bin)
// =============================================================================

const GLOBAL_GUARD_MARKER: &str = "Auto-generated by airis guards --global install";

/// Install global guards to ~/.airis/bin
pub fn install_global() -> Result<()> {
    println!("{}", "🌍 Installing global guards...".bright_blue());
    println!();

    // Load or create global config
    let config = GlobalConfig::load()?;
    let config_path = GlobalConfig::config_path()?;
    let bin_dir = GlobalConfig::bin_dir()?;

    // Create directories
    fs::create_dir_all(&bin_dir).with_context(|| format!("Failed to create {:?}", bin_dir))?;

    // Save default config if it doesn't exist
    if !config_path.exists() {
        config.save()?;
        println!(
            "   {} {}",
            "✓".green(),
            format!("Created {}", config_path.display()).dimmed()
        );
    }

    let mut installed_count = 0;

    // Install guard scripts for each denied command
    for cmd in &config.guards.deny {
        install_global_guard(&bin_dir, cmd)?;
        installed_count += 1;
        println!(
            "   {} {}",
            "✓".green(),
            format!("{} (global guard)", cmd).dimmed()
        );
    }

    // Auto-add PATH to shell profiles
    let path_line = "export PATH=\"$HOME/.airis/bin:$PATH\"  # airis guards";
    let mut path_added = false;

    for rc_file in &[".zshrc", ".bashrc"] {
        let home = dirs::home_dir().context("Failed to detect home directory")?;
        let rc_path = home.join(rc_file);

        if !rc_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&rc_path)
            .with_context(|| format!("Failed to read ~/{}", rc_file))?;

        if content.contains(".airis/bin") {
            println!(
                "   {} {}",
                "✓".green(),
                format!("~/{} already has PATH entry", rc_file).dimmed()
            );
            continue;
        }

        // Append PATH entry
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&rc_path)
            .with_context(|| format!("Failed to open ~/{} for writing", rc_file))?;

        use std::io::Write;
        writeln!(file)?;
        writeln!(file, "{}", path_line)?;

        println!(
            "   {} {}",
            "✓".green(),
            format!("Added PATH to ~/{}", rc_file).cyan()
        );
        path_added = true;
    }

    println!();
    println!(
        "{}",
        format!("✅ {} global guard(s) installed", installed_count).green()
    );
    println!();
    println!("{}", "📁 Config:".bright_yellow());
    println!("   {}", config_path.display());

    if path_added {
        println!();
        println!("{}", "🔧 Reload your shell to activate:".bright_yellow());
        println!("   {}", "source ~/.zshrc".cyan());
    }

    Ok(())
}

fn install_global_guard(bin_dir: &Path, cmd: &str) -> Result<()> {
    let script_path = bin_dir.join(cmd);
    ensure_safe_guard_path(&script_path)?;

    // Global guard script that:
    // 1. Inside Docker/CI: pass through to real command
    // 2. On host: always block (Docker-First enforcement)
    let content = format!(
        r#"#!/usr/bin/env bash
# {GLOBAL_GUARD_MARKER}
# DO NOT EDIT - managed by airis global guards

# Docker container: allow
if [[ "${{DOCKER_CONTAINER:-}}" == "true" ]] || [[ -f /.dockerenv ]]; then
    REAL_CMD=$(PATH=$(echo "$PATH" | tr ':' '\n' | grep -v '\.airis/bin' | tr '\n' ':' | sed 's/:$//') which {cmd} 2>/dev/null)
    if [[ -n "$REAL_CMD" && -x "$REAL_CMD" ]]; then
        exec "$REAL_CMD" "$@"
    fi
    exit 127
fi

# CI environment: allow
if [[ "${{CI:-}}" == "true" ]] || [[ -n "${{GITHUB_ACTIONS:-}}" ]] || [[ -n "${{GITLAB_CI:-}}" ]]; then
    REAL_CMD=$(PATH=$(echo "$PATH" | tr ':' '\n' | grep -v '\.airis/bin' | tr '\n' ':' | sed 's/:$//') which {cmd} 2>/dev/null)
    if [[ -n "$REAL_CMD" && -x "$REAL_CMD" ]]; then
        exec "$REAL_CMD" "$@"
    fi
    exit 127
fi

# Host: always block
echo "❌ '{cmd}' is blocked on this machine (Docker-First)."
echo ""
echo "Use airis commands instead:"
echo "  airis up      # Start all services (docker compose up + build)"
echo "  airis shell   # Enter container shell"
echo ""
echo "To manage guards:"
echo "  airis guards --global status     # Check status"
echo "  airis guards --global uninstall  # Remove guards"
exit 1
"#,
        GLOBAL_GUARD_MARKER = GLOBAL_GUARD_MARKER,
        cmd = cmd
    );

    fs::write(&script_path, content)
        .with_context(|| format!("Failed to write global guard script for '{}'", cmd))?;

    make_executable(&script_path)?;

    Ok(())
}

/// Show global guards status
pub fn status_global() -> Result<()> {
    let config_path = GlobalConfig::config_path()?;
    let bin_dir = GlobalConfig::bin_dir()?;

    println!("{}", "🌍 Global Guard Status".bright_blue());
    println!();

    // Check config file
    println!("{}", "Config file:".bright_yellow());
    if config_path.exists() {
        println!("   {} {}", "✓".green(), config_path.display());
    } else {
        println!(
            "   {} {} (will use defaults)",
            "✗".yellow(),
            config_path.display()
        );
    }
    println!();

    // Check bin directory
    println!("{}", "Guard directory:".bright_yellow());
    if bin_dir.exists() {
        println!("   {} {}", "✓".green(), bin_dir.display());
    } else {
        println!("   {} {} (not created)", "✗".yellow(), bin_dir.display());
        println!();
        println!(
            "{}",
            "Run 'airis guards --global install' to install guards".yellow()
        );
        return Ok(());
    }
    println!();

    // Load config and show guards
    let config = GlobalConfig::load()?;

    println!("{}", "Installed guards:".bright_yellow());
    for cmd in &config.guards.deny {
        let guard_path = bin_dir.join(cmd);
        let status = if guard_path.exists() && is_global_guard(&guard_path)? {
            "✓".green()
        } else if guard_path.exists() {
            "?".yellow() // File exists but not our guard
        } else {
            "✗".red()
        };
        println!("   {} {}", status, cmd);
    }
    println!();

    // Check PATH
    println!("{}", "PATH check:".bright_yellow());
    let path_var = std::env::var("PATH").unwrap_or_default();
    let bin_dir_str = bin_dir.to_string_lossy();
    if path_var.contains(&*bin_dir_str) {
        println!("   {} ~/.airis/bin is in PATH", "✓".green());
    } else {
        println!("   {} ~/.airis/bin is NOT in PATH", "✗".red());
        println!();
        println!("{}", "Add to your ~/.zshrc or ~/.bashrc:".bright_yellow());
        println!("   {}", "export PATH=\"$HOME/.airis/bin:$PATH\"".cyan());
    }

    Ok(())
}

/// Uninstall global guards
pub fn uninstall_global() -> Result<()> {
    let bin_dir = GlobalConfig::bin_dir()?;

    if !bin_dir.exists() {
        println!(
            "{}",
            "⚠️  Global guards not installed (no ~/.airis/bin directory)".yellow()
        );
        return Ok(());
    }

    println!("{}", "🗑️  Uninstalling global guards...".bright_blue());
    println!();

    let mut removed_count = 0;

    // Remove only files that have the global guard marker
    for entry in fs::read_dir(&bin_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && is_global_guard(&path)? {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            fs::remove_file(&path)?;
            println!(
                "   {} {}",
                "✓".green(),
                format!("Removed {}", name).dimmed()
            );
            removed_count += 1;
        }
    }

    // Remove directory if empty
    if bin_dir.read_dir()?.next().is_none() {
        fs::remove_dir(&bin_dir)?;
        println!(
            "   {} {}",
            "✓".green(),
            "Removed ~/.airis/bin directory".dimmed()
        );
    }

    println!();
    println!(
        "{}",
        format!("✅ {} global guard(s) uninstalled", removed_count).green()
    );

    Ok(())
}

/// Verify global guards are properly installed and active
pub fn verify_global() -> Result<()> {
    let bin_dir = GlobalConfig::bin_dir()?;
    let config = GlobalConfig::load()?;

    println!("{}", "🔍 Guard Verification".bright_blue());
    println!();

    let mut all_ok = true;

    // 1. Check ~/.airis/bin/ directory exists
    if bin_dir.exists() {
        println!("  {} ~/.airis/bin/ exists", "✓".green());
    } else {
        println!("  {} ~/.airis/bin/ does not exist", "✗".red());
        all_ok = false;
    }

    // 2. Check each guard script exists, is not a symlink, and has correct marker
    for cmd in &config.guards.deny {
        let guard_path = bin_dir.join(cmd);
        if guard_path.is_symlink() {
            let target = fs::read_link(&guard_path)
                .map(|t| t.display().to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            println!(
                "  {} {} is a SYMLINK → {} (security risk!)",
                "🚨".red(),
                cmd,
                target.red()
            );
            all_ok = false;
        } else if guard_path.exists() {
            if is_global_guard(&guard_path)? {
                println!("  {} {} guard installed", "✓".green(), cmd);
            } else {
                println!("  {} {} exists but is NOT an airis guard", "✗".red(), cmd);
                all_ok = false;
            }
        } else {
            println!("  {} {} guard not installed", "✗".red(), cmd);
            all_ok = false;
        }
    }

    // 3. Check PATH contains ~/.airis/bin
    let path_var = std::env::var("PATH").unwrap_or_default();
    let bin_dir_str = bin_dir.to_string_lossy();
    if path_var.contains(&*bin_dir_str) {
        println!("  {} ~/.airis/bin is in PATH", "✓".green());
    } else {
        println!("  {} ~/.airis/bin is NOT in PATH", "✗".red());
        all_ok = false;
    }

    // 4. Check which <cmd> points to guard script
    for cmd in &config.guards.deny {
        let output = std::process::Command::new("which").arg(cmd).output();

        match output {
            Ok(out) if out.status.success() => {
                let resolved = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if resolved.contains(".airis/bin") {
                    println!(
                        "  {} which {} → {} (guarded)",
                        "✓".green(),
                        cmd,
                        resolved.dimmed()
                    );
                } else {
                    println!(
                        "  {} which {} → {} (NOT guarded)",
                        "✗".red(),
                        cmd,
                        resolved.yellow()
                    );
                    all_ok = false;
                }
            }
            _ => {
                println!("  {} which {} → not found", "?".yellow(), cmd);
            }
        }
    }

    println!();
    if all_ok {
        println!("{}", "✅ All guards are active!".green().bold());
    } else {
        println!(
            "{}",
            "⚠️  Some guards are not properly configured.".yellow()
        );
        println!();
        println!("Run to fix:");
        println!("  {}", "airis guards --global install".cyan());
        println!("  {}", "source ~/.zshrc".cyan());
    }

    Ok(())
}

/// Check if a file is an airis local guard (contains our marker)
fn is_airis_guard(path: &Path) -> Result<bool> {
    let file = fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);

    // Check first 5 lines for the marker
    for line in reader.lines().take(5).flatten() {
        if line.contains("Auto-generated by airis guards install") {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Check if a file is an airis global guard (contains our marker)
fn is_global_guard(path: &Path) -> Result<bool> {
    let file = fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);

    // Check first 5 lines for the marker
    for line in reader.lines().take(5).flatten() {
        if line.contains(GLOBAL_GUARD_MARKER) {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_mode_deny_list() {
        // Strict mode should block cargo, python, etc.
        assert!(STRICT_MODE_DENY.contains(&"cargo"));
        assert!(STRICT_MODE_DENY.contains(&"python"));
        assert!(STRICT_MODE_DENY.contains(&"go"));
    }

    #[test]
    fn test_hybrid_mode_allow_list() {
        // Hybrid mode should allow cargo, python, etc.
        assert!(HYBRID_MODE_ALLOW.contains(&"cargo"));
        assert!(HYBRID_MODE_ALLOW.contains(&"python"));
    }

    #[test]
    fn test_hybrid_mode_filters_deny_list() {
        // Simulate hybrid mode filtering
        let deny_list = vec![
            "npm".to_string(),
            "yarn".to_string(),
            "cargo".to_string(),
            "python".to_string(),
        ];

        let filtered: Vec<String> = deny_list
            .into_iter()
            .filter(|cmd| !HYBRID_MODE_ALLOW.contains(&cmd.as_str()))
            .collect();

        // npm and yarn should remain denied
        assert!(filtered.contains(&"npm".to_string()));
        assert!(filtered.contains(&"yarn".to_string()));
        // cargo and python should be allowed
        assert!(!filtered.contains(&"cargo".to_string()));
        assert!(!filtered.contains(&"python".to_string()));
    }

    #[test]
    fn test_strict_mode_adds_to_deny_list() {
        // Simulate strict mode adding commands
        let mut deny_list: Vec<String> = vec!["npm".to_string(), "yarn".to_string()];

        for cmd in STRICT_MODE_DENY {
            if !deny_list.contains(&cmd.to_string()) {
                deny_list.push(cmd.to_string());
            }
        }

        // Should now contain both original and strict mode commands
        assert!(deny_list.contains(&"npm".to_string()));
        assert!(deny_list.contains(&"cargo".to_string()));
        assert!(deny_list.contains(&"python".to_string()));
    }

    #[test]
    fn test_global_config_default() {
        let config = GlobalConfig::default();
        assert_eq!(config.version, 1);
        assert!(config.guards.deny.contains(&"npm".to_string()));
        assert!(config.guards.deny.contains(&"yarn".to_string()));
        assert!(config.guards.deny.contains(&"pnpm".to_string()));
        assert!(config.guards.deny.contains(&"bun".to_string()));
        assert!(config.guards.deny.contains(&"npx".to_string()));
    }

    #[test]
    fn test_global_config_paths() {
        // Just verify that path functions don't panic
        let config_path = GlobalConfig::config_path();
        assert!(config_path.is_ok());

        let bin_dir = GlobalConfig::bin_dir();
        assert!(bin_dir.is_ok());

        // Verify paths contain expected components
        let config_path = config_path.unwrap();
        assert!(config_path.to_string_lossy().contains(".airis"));
        assert!(config_path.to_string_lossy().contains("global-config.toml"));

        let bin_dir = bin_dir.unwrap();
        assert!(bin_dir.to_string_lossy().contains(".airis"));
        assert!(bin_dir.to_string_lossy().ends_with("bin"));
    }

    #[test]
    fn test_global_config_serialization() {
        let config = GlobalConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();

        // Verify it can be deserialized back
        let parsed: GlobalConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.version, config.version);
        assert_eq!(parsed.guards.deny.len(), config.guards.deny.len());
    }

    #[test]
    fn test_is_airis_guard_marker() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let guard_file = dir.path().join("npm");

        // Write a file with the guard marker
        let mut f = fs::File::create(&guard_file).unwrap();
        writeln!(f, "#!/usr/bin/env bash").unwrap();
        writeln!(f, "# Auto-generated by airis guards install").unwrap();
        writeln!(f, "echo 'blocked'").unwrap();

        assert!(is_airis_guard(&guard_file).unwrap());

        // Write a file without the marker
        let other_file = dir.path().join("other");
        let mut f = fs::File::create(&other_file).unwrap();
        writeln!(f, "#!/usr/bin/env bash").unwrap();
        writeln!(f, "echo 'hello'").unwrap();

        assert!(!is_airis_guard(&other_file).unwrap());
    }

    #[test]
    fn test_is_global_guard_marker() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let guard_file = dir.path().join("npm");

        // Write a file with the global guard marker
        let mut f = fs::File::create(&guard_file).unwrap();
        writeln!(f, "#!/usr/bin/env bash").unwrap();
        writeln!(f, "# {}", GLOBAL_GUARD_MARKER).unwrap();
        writeln!(f, "echo 'blocked'").unwrap();

        assert!(is_global_guard(&guard_file).unwrap());

        // Write a file with the local guard marker (should not match)
        let local_file = dir.path().join("local");
        let mut f = fs::File::create(&local_file).unwrap();
        writeln!(f, "#!/usr/bin/env bash").unwrap();
        writeln!(f, "# Auto-generated by airis guards install").unwrap();
        writeln!(f, "echo 'blocked'").unwrap();

        assert!(!is_global_guard(&local_file).unwrap());
    }

    #[test]
    fn test_install_deny_guard_default_message() {
        let dir = tempfile::tempdir().unwrap();
        install_deny_guard(dir.path(), "npm", None).unwrap();

        let script_path = dir.path().join("npm");
        assert!(script_path.exists());

        let content = fs::read_to_string(&script_path).unwrap();

        // Should contain the shebang
        assert!(content.starts_with("#!/usr/bin/env bash"));
        // Should contain the auto-generated marker
        assert!(content.contains("Auto-generated by airis guards install"));
        // Should contain the default error message referencing the command
        assert!(content.contains("npm"));
        assert!(content.contains("guards.deny"));
        assert!(content.contains("airis shell"));
        // Should exit with code 1
        assert!(content.contains("exit 1"));
    }

    #[test]
    fn test_install_deny_guard_with_custom_message() {
        let dir = tempfile::tempdir().unwrap();
        let custom_msg = "Please use 'airis install' instead of yarn.".to_string();
        install_deny_guard(dir.path(), "yarn", Some(&custom_msg)).unwrap();

        let script_path = dir.path().join("yarn");
        assert!(script_path.exists());

        let content = fs::read_to_string(&script_path).unwrap();

        // Should contain the custom message, not the default
        assert!(content.contains("Please use 'airis install' instead of yarn."));
        // Should NOT contain the default message
        assert!(!content.contains("guards.deny"));
        // Should still exit with code 1
        assert!(content.contains("exit 1"));
    }

    #[test]
    fn test_install_wrap_guard() {
        let dir = tempfile::tempdir().unwrap();
        let wrapper = "docker compose exec workspace pnpm";
        install_wrap_guard(dir.path(), "pnpm", wrapper).unwrap();

        let script_path = dir.path().join("pnpm");
        assert!(script_path.exists());

        let content = fs::read_to_string(&script_path).unwrap();

        // Should contain the shebang
        assert!(content.starts_with("#!/usr/bin/env bash"));
        // Should contain the auto-generated marker for wrap section
        assert!(content.contains("guards.wrap"));
        // Should contain the exec wrapper line with "$@" for argument forwarding
        assert!(content.contains(&format!("exec {} \"$@\"", wrapper)));
        // Should contain the comment showing the mapping
        assert!(content.contains(&format!("# Wrapper: pnpm → {}", wrapper)));
        // Should NOT contain "exit 1" (wrap lets through)
        assert!(!content.contains("exit 1"));
    }

    #[test]
    fn test_script_permissions() {
        let dir = tempfile::tempdir().unwrap();

        // Test deny guard permissions
        install_deny_guard(dir.path(), "npm", None).unwrap();
        let perms = fs::metadata(dir.path().join("npm")).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o755);

        // Test wrap guard permissions
        install_wrap_guard(dir.path(), "pnpm", "docker compose exec workspace pnpm").unwrap();
        let perms = fs::metadata(dir.path().join("pnpm")).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o755);
    }

    #[test]
    fn test_make_executable_sets_755() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_script");

        // Create a file with default permissions
        fs::write(&file_path, "#!/usr/bin/env bash\necho hello").unwrap();

        // Verify it is not yet 755 (default is typically 644)
        let perms_before = fs::metadata(&file_path).unwrap().permissions();
        assert_ne!(perms_before.mode() & 0o777, 0o755);

        // Apply make_executable
        make_executable(&file_path).unwrap();

        let perms_after = fs::metadata(&file_path).unwrap().permissions();
        assert_eq!(perms_after.mode() & 0o777, 0o755);
    }

    #[test]
    fn test_full_install_with_tempdir() {
        let _guard = crate::test_lock::DIR_LOCK.lock().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let result = std::panic::catch_unwind(|| {
            // Write a minimal manifest.toml with deny, wrap, and deny_with_message entries
            let manifest_content = r#"
[workspace]
name = "test-workspace"

[guards]
deny = ["npm", "yarn"]

[guards.wrap]
pnpm = "docker compose exec workspace pnpm"

[guards.deny_with_message]
bun = "bun is not supported. Use pnpm via airis."
"#;
            fs::write("manifest.toml", manifest_content).unwrap();

            // Run install
            install().unwrap();

            // Verify guards directory was created
            let guards_dir = PathBuf::from(GUARDS_DIR);
            assert!(guards_dir.exists(), ".airis/bin should exist");

            // Verify deny guards
            let npm_script = guards_dir.join("npm");
            assert!(npm_script.exists(), "npm guard should exist");
            let npm_content = fs::read_to_string(&npm_script).unwrap();
            assert!(npm_content.contains("exit 1"));
            assert!(npm_content.contains("guards.deny"));
            assert_eq!(
                fs::metadata(&npm_script).unwrap().permissions().mode() & 0o777,
                0o755
            );

            let yarn_script = guards_dir.join("yarn");
            assert!(yarn_script.exists(), "yarn guard should exist");
            let yarn_content = fs::read_to_string(&yarn_script).unwrap();
            assert!(yarn_content.contains("exit 1"));

            // Verify wrap guard
            let pnpm_script = guards_dir.join("pnpm");
            assert!(pnpm_script.exists(), "pnpm wrap guard should exist");
            let pnpm_content = fs::read_to_string(&pnpm_script).unwrap();
            assert!(pnpm_content.contains("exec docker compose exec workspace pnpm \"$@\""));
            assert!(!pnpm_content.contains("exit 1"));

            // Verify deny_with_message guard
            let bun_script = guards_dir.join("bun");
            assert!(
                bun_script.exists(),
                "bun deny_with_message guard should exist"
            );
            let bun_content = fs::read_to_string(&bun_script).unwrap();
            assert!(bun_content.contains("bun is not supported. Use pnpm via airis."));
            assert!(bun_content.contains("exit 1"));
            // Should NOT contain the default message
            assert!(!bun_content.contains("guards.deny"));
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    // ── symlink attack protection tests ──

    #[cfg(unix)]
    #[test]
    fn test_ensure_safe_guard_path_removes_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("malicious");
        fs::write(&target, "#!/bin/bash\necho pwned").unwrap();

        let guard_path = dir.path().join("npm");
        std::os::unix::fs::symlink(&target, &guard_path).unwrap();
        assert!(guard_path.is_symlink());

        // ensure_safe_guard_path should remove the symlink
        ensure_safe_guard_path(&guard_path).unwrap();
        assert!(!guard_path.exists(), "symlink should have been removed");
        // Target should still exist (not the symlink's target)
        assert!(target.exists(), "symlink target should not be removed");
    }

    #[cfg(unix)]
    #[test]
    fn test_install_deny_guard_replaces_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("malicious");
        fs::write(&target, "#!/bin/bash\necho pwned").unwrap();

        let guard_path = dir.path().join("npm");
        std::os::unix::fs::symlink(&target, &guard_path).unwrap();

        // install_deny_guard should succeed (removes symlink, writes real file)
        install_deny_guard(dir.path(), "npm", None).unwrap();

        assert!(!guard_path.is_symlink(), "should no longer be a symlink");
        assert!(guard_path.is_file(), "should be a regular file");
        let content = fs::read_to_string(&guard_path).unwrap();
        assert!(content.contains("Auto-generated by airis guards install"));
    }

    #[cfg(unix)]
    #[test]
    fn test_install_wrap_guard_replaces_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("malicious");
        fs::write(&target, "#!/bin/bash\necho pwned").unwrap();

        let guard_path = dir.path().join("pnpm");
        std::os::unix::fs::symlink(&target, &guard_path).unwrap();

        install_wrap_guard(dir.path(), "pnpm", "docker compose exec workspace pnpm").unwrap();

        assert!(!guard_path.is_symlink());
        let content = fs::read_to_string(&guard_path).unwrap();
        assert!(content.contains("guards.wrap"));
    }

    #[test]
    fn test_ensure_safe_guard_path_allows_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("npm");
        fs::write(&path, "existing content").unwrap();

        // Should succeed for regular files
        ensure_safe_guard_path(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_ensure_safe_guard_path_allows_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent");

        // Should succeed for non-existent paths
        ensure_safe_guard_path(&path).unwrap();
    }
}
