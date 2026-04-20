use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Generate TypeScript types from Supabase PostgreSQL schema
pub fn run(host: &str, port: &str, database: &str, output: &str) -> Result<()> {
    println!(
        "{}",
        "🔧 Generating TypeScript types from Supabase..."
            .cyan()
            .bold()
    );
    println!("   {} Host: {}:{}", "📍".dimmed(), host, port);
    println!("   {} Database: {}", "💾".dimmed(), database);
    println!("   {} Output: {}", "📂".dimmed(), output);
    println!();

    // Check if output directory exists, create if not
    let output_path = Path::new(output);
    if !output_path.exists() {
        println!("   {} Creating output directory: {}", "📁".dimmed(), output);
        fs::create_dir_all(output_path)
            .with_context(|| format!("Failed to create directory: {}", output))?;
    }

    // Check if Supabase is running
    println!("   {} Checking if Supabase is running...", "🔍".dimmed());
    let pg_ready = Command::new("docker")
        .args([
            "compose",
            "-f",
            "supabase/docker-compose.yml",
            "exec",
            
            "db",
            "pg_isready",
            "-U",
            "postgres",
        ])
        .output();

    if let Ok(output) = pg_ready {
        if !output.status.success() {
            eprintln!("   {} Supabase database is not running!", "❌".red());
            eprintln!("   {} Please start Supabase first: airis up", "💡".yellow());
            anyhow::bail!("Supabase database is not running");
        }
    } else {
        eprintln!("   {} Failed to check Supabase status", "⚠️".yellow());
    }

    println!("   {} Supabase is running", "✅".green());
    println!();

    // Use supabase CLI to generate types
    println!("   {} Generating types with Supabase CLI...", "⚙️".dimmed());

    let status = Command::new("npx")
        .args([
            "supabase",
            "gen",
            "types",
            "typescript",
            "--db-url",
            &format!(
                "postgresql://postgres:postgres@{}:{}/{}",
                host, port, database
            ),
        ])
        .current_dir(".")
        .status()
        .with_context(|| "Failed to run Supabase CLI")?;

    if !status.success() {
        anyhow::bail!("Supabase type generation failed");
    }

    println!();
    println!(
        "{}",
        "✅ TypeScript types generated successfully!".green().bold()
    );
    println!();
    println!("{}", "📝 Next steps:".bright_yellow());
    println!("  1. Check generated types in {}", output);
    println!("  2. Import types in your application");
    println!("  3. Run `airis init` to update workspace configuration");

    Ok(())
}
