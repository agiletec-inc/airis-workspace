use anyhow::{bail, Result};
use colored::Colorize;
use std::fs;

use crate::rules::DEFAULT_RULES;

fn rules_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .expect("Could not determine home directory")
        .join(".claude")
        .join("rules")
}

pub fn init() -> Result<()> {
    let dir = rules_dir();
    fs::create_dir_all(&dir)?;

    let mut created = 0;
    let mut updated = 0;

    for rule in DEFAULT_RULES {
        let path = dir.join(rule.filename);
        if path.exists() {
            let existing = fs::read_to_string(&path)?;
            if existing == rule.content {
                println!("  {} {} (unchanged)", "·".dimmed(), rule.filename);
                continue;
            }
            fs::write(&path, rule.content)?;
            println!("  {} {} (updated)", "↻".yellow(), rule.filename);
            updated += 1;
        } else {
            fs::write(&path, rule.content)?;
            println!("  {} {} (created)", "✓".green(), rule.filename);
            created += 1;
        }
    }

    println!();
    println!(
        "{} {} rules initialized ({} created, {} updated) → {}",
        "✓".green().bold(),
        DEFAULT_RULES.len(),
        created,
        updated,
        dir.display()
    );
    Ok(())
}

pub fn list() -> Result<()> {
    let dir = rules_dir();

    println!("{}", "airis-managed rules:".bold());
    for rule in DEFAULT_RULES {
        let path = dir.join(rule.filename);
        let status = if path.exists() {
            "installed".green().to_string()
        } else {
            "not installed".red().to_string()
        };
        println!(
            "  {} {:30} {:30} [{}]",
            "·", rule.filename, rule.description, status
        );
    }

    // Show user rules (non-airis files)
    if dir.exists() {
        let user_rules: Vec<_> = fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.ends_with(".md") && !name.starts_with("airis-")
            })
            .collect();

        if !user_rules.is_empty() {
            println!();
            println!("{}", "User rules:".bold());
            for entry in &user_rules {
                println!("  {} {}", "·", entry.file_name().to_string_lossy());
            }
        }
    }

    Ok(())
}

pub fn show(name: &str) -> Result<()> {
    let dir = rules_dir();

    // Try exact filename first, then with airis- prefix
    let filename = if name.ends_with(".md") {
        name.to_string()
    } else {
        format!("airis-{}.md", name)
    };

    let path = dir.join(&filename);
    if !path.exists() {
        // Try finding by rule name in defaults
        let found = DEFAULT_RULES.iter().find(|r| r.name == name);
        if let Some(rule) = found {
            let rule_path = dir.join(rule.filename);
            if rule_path.exists() {
                let content = fs::read_to_string(&rule_path)?;
                print!("{}", content);
                return Ok(());
            }
        }
        bail!(
            "Rule '{}' not found. Run `airis rules list` to see available rules.",
            name
        );
    }

    let content = fs::read_to_string(&path)?;
    print!("{}", content);
    Ok(())
}

pub fn update() -> Result<()> {
    let dir = rules_dir();
    if !dir.exists() {
        bail!("Rules directory not found. Run `airis rules init` first.");
    }

    let mut updated = 0;
    let mut unchanged = 0;

    for rule in DEFAULT_RULES {
        let path = dir.join(rule.filename);
        if path.exists() {
            let existing = fs::read_to_string(&path)?;
            if existing == rule.content {
                println!("  {} {} (unchanged)", "·".dimmed(), rule.filename);
                unchanged += 1;
                continue;
            }
        }
        fs::write(&path, rule.content)?;
        println!("  {} {} (updated)", "↻".yellow(), rule.filename);
        updated += 1;
    }

    println!();
    println!(
        "{} {} updated, {} unchanged",
        "✓".green().bold(),
        updated,
        unchanged
    );
    Ok(())
}
