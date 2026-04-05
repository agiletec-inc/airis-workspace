//! Test scan command: categorize test files and detect policy violations.
//!
//! Walks the workspace to classify each test file (MOCK, STRUCTURAL,
//! INTEGRATION, PURE), then runs forbidden-pattern and type-enforcement
//! checks from the policy checkers.

use anyhow::{bail, Result};
use colored::Colorize;
use std::path::Path;
use walkdir::WalkDir;

use crate::manifest::{Manifest, MANIFEST_FILE};
use super::policy::checkers::{check_mock_patterns, check_type_enforcement};
use super::policy::{PolicyResult, PolicyViolation, Severity};

/// Test file category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestCategory {
    Mock,
    Structural,
    Integration,
    Pure,
}

impl TestCategory {
    fn color(self, s: &str) -> colored::ColoredString {
        match self {
            Self::Mock => s.red(),
            Self::Structural => s.yellow(),
            Self::Integration => s.cyan(),
            Self::Pure => s.green(),
        }
    }
}

/// Classify a test file by its content.
fn categorize(file_name: &str, content: &str) -> TestCategory {
    // INTEGRATION: filename marker or real DB connection patterns
    if file_name.contains(".integration.") {
        return TestCategory::Integration;
    }
    let integration_indicators = [
        "createClient(",
        "supabase.from(",
        "pg.connect(",
        "new Pool(",
        "DATABASE_URL",
        "SUPABASE_URL",
    ];
    if integration_indicators
        .iter()
        .any(|pat| content.contains(pat))
    {
        return TestCategory::Integration;
    }

    // MOCK: mocking utilities
    let mock_indicators = ["vi.mock(", "vi.fn().mockReturnValue", "jest.mock("];
    if mock_indicators.iter().any(|pat| content.contains(pat)) {
        return TestCategory::Mock;
    }

    // STRUCTURAL: reads files and asserts on content
    if content.contains("readFileSync") && content.contains("toContain") {
        return TestCategory::Structural;
    }

    TestCategory::Pure
}

/// Returns true if a path component indicates we should skip this entry.
fn should_skip(path_str: &str) -> bool {
    path_str.contains("node_modules")
        || path_str.contains("/.git/")
        || path_str.contains("/target/")
        || path_str.contains("/dist/")
        || path_str.contains("__ref/")
}

/// Returns true if the file name looks like a test file.
fn is_test_file(name: &str) -> bool {
    name.contains(".test.") || name.contains(".spec.") || name.contains(".integration.")
}

/// Entry point for `airis test --scan`.
pub fn run() -> Result<()> {
    println!("{}", "==================================".bright_blue());
    println!("{}", "airis test --scan".bright_blue().bold());
    println!("{}", "==================================".bright_blue());
    println!();

    // --- Phase 1: Categorize test files ---
    println!(
        "{}",
        "Phase 1: Categorize test files".bright_blue().bold()
    );

    let mut mock_count: usize = 0;
    let mut structural_count: usize = 0;
    let mut integration_count: usize = 0;
    let mut pure_count: usize = 0;

    for entry in WalkDir::new(".")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let path_str = path.to_string_lossy();

        if should_skip(&path_str) {
            continue;
        }

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !is_test_file(file_name) {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let category = categorize(file_name, &content);
        match category {
            TestCategory::Mock => mock_count += 1,
            TestCategory::Structural => structural_count += 1,
            TestCategory::Integration => integration_count += 1,
            TestCategory::Pure => pure_count += 1,
        }
    }

    let total = mock_count + structural_count + integration_count + pure_count;

    println!();
    println!(
        "  {} {:>4}",
        TestCategory::Pure.color("PURE         "),
        pure_count
    );
    println!(
        "  {} {:>4}",
        TestCategory::Mock.color("MOCK         "),
        mock_count
    );
    println!(
        "  {} {:>4}",
        TestCategory::Structural.color("STRUCTURAL   "),
        structural_count
    );
    println!(
        "  {} {:>4}",
        TestCategory::Integration.color("INTEGRATION  "),
        integration_count
    );
    println!("  {}", "─────────────────────".dimmed());
    println!("  TOTAL              {:>4}", total);
    println!();

    // --- Phase 2: Policy violation scan ---
    println!(
        "{}",
        "Phase 2: Policy violation scan".bright_blue().bold()
    );

    let manifest_path = Path::new(MANIFEST_FILE);
    if !manifest_path.exists() {
        println!(
            "  {}",
            "skipped (manifest.toml not found)".dimmed()
        );
        println!();
        println!("{}", "==================================".bright_blue());
        return Ok(());
    }

    let manifest = Manifest::load(manifest_path)?;
    let testing = &manifest.policy.testing;

    let mut result = PolicyResult {
        passed: true,
        violations: Vec::new(),
        warnings: Vec::new(),
    };

    // Forbidden mock patterns
    if !testing.forbidden_patterns.is_empty() {
        check_mock_patterns(&testing.forbidden_patterns, None, &mut result)?;
    } else {
        println!(
            "  {}",
            "forbidden_patterns: skipped (none configured)".dimmed()
        );
    }

    // Type enforcement
    if let Some(te) = &testing.type_enforcement {
        check_type_enforcement(
            &te.generated_types_path,
            &te.required_imports,
            None,
            &mut result,
        )?;
    } else {
        println!(
            "  {}",
            "type_enforcement: skipped (not configured)".dimmed()
        );
    }

    // --- Results ---
    println!();
    if result.violations.is_empty() {
        println!("{}", "All checks passed!".green().bold());
    } else {
        let errors: Vec<&PolicyViolation> = result
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Error)
            .collect();
        let warnings: Vec<&PolicyViolation> = result
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Warning)
            .collect();

        if !errors.is_empty() {
            println!(
                "{}",
                format!("{} violation(s):", errors.len()).red().bold()
            );
            for v in &errors {
                println!("   {} {}", "x".red(), v.message);
            }
            result.passed = false;
        }

        if !warnings.is_empty() {
            println!(
                "{}",
                format!("{} warning(s):", warnings.len()).yellow()
            );
            for v in &warnings {
                println!("   {} {}", "!".yellow(), v.message);
            }
        }
    }

    println!("{}", "==================================".bright_blue());

    if !result.passed {
        bail!("Test scan found policy violations. Fix them before proceeding.");
    }

    Ok(())
}
