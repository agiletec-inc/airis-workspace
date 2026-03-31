//! Display formatting for diff output

use colored::Colorize;

use super::{DiffResult, DiffSummary, FileStatus};

/// Print unified diff output
pub(super) fn print_unified(result: &DiffResult) {
    if result.summary.files_changed == 0 && result.summary.files_created == 0 {
        println!("{}", "✅ No changes detected".green());
        println!("   All generated files are in sync with manifest.toml");
        return;
    }

    println!(
        "{}",
        "📝 Diff Preview (manifest.toml → generated files)"
            .bright_blue()
            .bold()
    );
    println!();

    for file in &result.files {
        match file.status {
            FileStatus::Unchanged => continue,
            FileStatus::Created => {
                println!(
                    "{}",
                    format!("=== {} (new file) ===", file.path).green().bold()
                );
            }
            FileStatus::Modified => {
                println!("{}", format!("=== {} ===", file.path).yellow().bold());
            }
        }

        if let Some(ref diff) = file.diff {
            for line in diff.lines() {
                if line.starts_with('+') && !line.starts_with("+++") {
                    println!("{}", line.green());
                } else if line.starts_with('-') && !line.starts_with("---") {
                    println!("{}", line.red());
                } else if line.starts_with("@@") {
                    println!("{}", line.cyan());
                } else {
                    println!("{}", line);
                }
            }
        }
        println!();
    }

    // Summary
    println!("{}", "─".repeat(50).dimmed());
    print_summary(&result.summary);
}

/// Print statistics only
pub(super) fn print_stat(result: &DiffResult) {
    if result.summary.files_changed == 0 && result.summary.files_created == 0 {
        println!("{}", "✅ No changes".green());
        return;
    }

    println!("{}", "📊 Diff Statistics".bright_blue().bold());
    println!();

    for file in &result.files {
        if file.status == FileStatus::Unchanged {
            continue;
        }

        let status_indicator = match file.status {
            FileStatus::Created => "A".green(),
            FileStatus::Modified => "M".yellow(),
            FileStatus::Unchanged => " ".normal(),
        };

        let changes = format!("+{} -{}", file.additions, file.deletions);
        println!(
            " {} {:40} {}",
            status_indicator,
            file.path,
            changes.dimmed()
        );
    }

    println!();
    print_summary(&result.summary);
}

/// Print summary line
pub(super) fn print_summary(summary: &DiffSummary) {
    let mut parts = Vec::new();

    if summary.files_created > 0 {
        parts.push(
            format!("{} file(s) created", summary.files_created)
                .green()
                .to_string(),
        );
    }
    if summary.files_changed > 0 {
        parts.push(
            format!("{} file(s) modified", summary.files_changed)
                .yellow()
                .to_string(),
        );
    }

    if parts.is_empty() {
        println!("{}", "No changes".dimmed());
    } else {
        let summary_text = format!(
            "{}, {} insertions(+), {} deletions(-)",
            parts.join(", "),
            summary.total_additions,
            summary.total_deletions
        );
        println!("{}", summary_text);
    }
}
