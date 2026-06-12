//! CLI integration tests
//!
//! Tests the airis CLI commands end-to-end

use assert_cmd::Command;
use predicates::prelude::*;

fn airis() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("airis-workspace").unwrap()
}

#[test]
fn test_version_flag() {
    airis()
        .arg("-V")
        .assert()
        .success()
        .stdout(predicate::str::contains("airis"));
}

#[test]
fn test_help_flag() {
    airis()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "A workspace orchestrator for monorepos.",
        ));
}

#[test]
fn test_clean_dry_run() {
    airis().args(["clean", "--dry-run"]).assert().success();
}

#[test]
fn test_policy_help() {
    airis()
        .args(["policy", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("check"))
        .stdout(predicate::str::contains("enforce"));
}

#[test]
fn test_policy_check_no_config() {
    // Should succeed with default config (no policies.toml)
    airis().args(["policy", "check"]).assert().success();
}
