//! Environment variable validation: check frontend env vars for disallowed public keys

use anyhow::{Result, bail};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Validate frontend environment variables
pub fn validate_env() -> Result<()> {
    validate_env_impl(false)
}

pub fn validate_env_impl(quiet: bool) -> Result<()> {
    if !quiet {
        println!(
            "{}",
            "🔍 Checking frontend environment variables...".bright_blue()
        );
    }

    let allowed_keys = vec![
        "NEXT_PUBLIC_SUPABASE_URL",
        "NEXT_PUBLIC_SUPABASE_ANON_KEY",
        "EXPO_PUBLIC_SUPABASE_URL",
        "EXPO_PUBLIC_SUPABASE_ANON_KEY",
    ];

    let mut disallowed = Vec::new();

    // Check .env files in apps
    let apps_dir = Path::new("apps");
    if apps_dir.exists() {
        for entry in fs::read_dir(apps_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Check various .env files
            for env_file in &[".env", ".env.local", ".env.development"] {
                let env_path = path.join(env_file);
                if env_path.exists() {
                    check_env_file(&env_path, &allowed_keys, &mut disallowed)?;
                }
            }
        }
    }

    if !disallowed.is_empty() {
        if !quiet {
            println!();
            println!("{}", "Disallowed public environment keys detected:".red());
            for item in &disallowed {
                println!("  - {}", item);
            }
            println!();
            println!("Allowed keys: {}", allowed_keys.join(", "));
        }
        bail!(
            "Found {} disallowed public environment keys",
            disallowed.len()
        );
    }

    if !quiet {
        println!("{}", "✅ Environment variables look good.".green());
    }
    Ok(())
}

/// Check a single .env file for disallowed public keys
pub fn check_env_file(path: &Path, allowed: &[&str], disallowed: &mut Vec<String>) -> Result<()> {
    let content = fs::read_to_string(path)?;

    for line in content.lines() {
        // Skip comments and empty lines
        if line.trim().starts_with('#') || line.trim().is_empty() {
            continue;
        }

        // Extract key
        if let Some(key) = line.split('=').next() {
            let key = key.trim();

            // Check if it's a public key
            if (key.starts_with("NEXT_PUBLIC_") || key.starts_with("EXPO_PUBLIC_"))
                && !allowed.contains(&key)
            {
                disallowed.push(format!("{}: {}", path.display(), key));
            }
        }
    }

    Ok(())
}
