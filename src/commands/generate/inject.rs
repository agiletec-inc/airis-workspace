use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use std::fs;

use crate::manifest::InjectValue;

use super::backup_file;

/// Scan workspace files for `# airis:inject <key>` markers and replace
/// the next line with the resolved value from `[inject]` in manifest.toml.
///
/// Returns the number of files modified.
pub(super) fn inject_values(
    inject: &IndexMap<String, InjectValue>,
    resolved_catalog: &IndexMap<String, String>,
) -> Result<usize> {
    if inject.is_empty() {
        return Ok(0);
    }

    println!();
    println!(
        "{}",
        "💉 Injecting values from manifest [inject]...".bright_blue()
    );

    // Resolve all inject values to plain strings
    let values = resolve_inject_values(inject, resolved_catalog)?;

    let marker_re = regex::Regex::new(r"#\s*airis:inject\s+([\w.\-]+)")
        .context("Failed to compile inject marker regex")?;

    let mut modified_count = 0;

    // Walk workspace respecting .gitignore
    let walker = walkdir::WalkDir::new(".")
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            // Skip .git directory, node_modules, dist, .next, .turbo
            !matches!(
                name,
                ".git"
                    | "node_modules"
                    | "dist"
                    | ".next"
                    | ".turbo"
                    | ".pnpm"
                    | ".cache"
                    | "coverage"
            )
        });

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();

        // Skip binary files and known non-text
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if matches!(
            ext,
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "ico"
                | "woff"
                | "woff2"
                | "ttf"
                | "eot"
                | "lock"
                | "zst"
                | "tar"
                | "gz"
        ) {
            continue;
        }

        // Quick check: skip files that don't contain our marker
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if !content.contains("airis:inject") {
            continue;
        }

        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        let mut file_modified = false;

        let mut i = 0;
        while i < lines.len().saturating_sub(1) {
            if let Some(caps) = marker_re.captures(&lines[i]) {
                let key = &caps[1];
                if let Some(value) = values.get(key) {
                    // Preserve indentation of the target line
                    let target = &lines[i + 1];
                    let indent_len = target.len() - target.trim_start().len();
                    let indent: String = target.chars().take(indent_len).collect();
                    let new_line = format!("{indent}{value}");

                    if lines[i + 1] != new_line {
                        lines[i + 1] = new_line;
                        file_modified = true;
                    }
                } else {
                    println!(
                        "   {} marker '{}' in {} has no matching [inject] key",
                        "⚠".yellow(),
                        key,
                        path.display()
                    );
                }
            }
            i += 1;
        }

        if file_modified {
            backup_file(path)?;
            fs::write(path, lines.join("\n") + "\n").with_context(|| {
                format!("Failed to write injected values to {}", path.display())
            })?;
            println!("   {} {}", "→".green(), path.display());
            modified_count += 1;
        }
    }

    if modified_count == 0 {
        println!(
            "   {} All injected values are already up to date",
            "✓".green()
        );
    }

    Ok(modified_count)
}

/// Resolve inject values: simple strings pass through, template values
/// have `{version}` replaced with the resolved catalog version.
pub(super) fn resolve_inject_values(
    inject: &IndexMap<String, InjectValue>,
    resolved_catalog: &IndexMap<String, String>,
) -> Result<IndexMap<String, String>> {
    let mut values = IndexMap::new();

    for (key, val) in inject {
        let resolved = match val {
            InjectValue::Simple(s) => s.clone(),
            InjectValue::Template {
                template,
                from_catalog,
            } => {
                let version = resolved_catalog
                    .get(from_catalog.as_str())
                    .cloned()
                    .unwrap_or_else(|| {
                        let stripped = from_catalog.trim_start_matches('@');
                        resolved_catalog.get(stripped).cloned().unwrap_or_default()
                    });
                if version.is_empty() {
                    println!(
                        "   {} inject key '{}': catalog entry '{}' not found, skipping",
                        "⚠".yellow(),
                        key,
                        from_catalog
                    );
                    continue;
                }
                let clean_version = version.trim_start_matches('^').trim_start_matches('~');
                template.replace("{version}", clean_version)
            }
        };
        values.insert(key.clone(), resolved);
    }

    Ok(values)
}
