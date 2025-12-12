//! CLI integration tests
//!
//! Tests the airis CLI commands end-to-end

use assert_cmd::Command;
use predicates::prelude::*;

fn airis() -> Command {
    Command::cargo_bin("airis").unwrap()
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
        .stdout(predicate::str::contains("Docker-first monorepo workspace manager"));
}

#[test]
fn test_build_help() {
    airis()
        .args(["build", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--docker"))
        .stdout(predicate::str::contains("--channel"));
}

#[test]
fn test_build_docker_requires_project() {
    airis()
        .args(["build", "--docker"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--docker requires a project path"));
}

#[test]
fn test_invalid_channel() {
    // This should fail because the project doesn't exist, but we can test channel parsing
    // by checking that valid channels don't cause parse errors
    airis()
        .args(["build", "apps/nonexistent", "--docker", "--channel", "lts"])
        .assert()
        .failure()
        // Should fail due to missing pnpm-lock.yaml, not channel parsing
        .stderr(predicate::str::contains("pnpm-lock.yaml").or(predicate::str::contains("not found")));
}

#[test]
fn test_clean_dry_run() {
    airis()
        .args(["clean", "--dry-run"])
        .assert()
        .success();
}

#[test]
fn test_affected_command() {
    airis()
        .args(["affected", "--base", "HEAD", "--head", "HEAD"])
        .assert()
        .success();
}

#[test]
fn test_bundle_help() {
    airis()
        .args(["bundle", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("image.tar"))
        .stdout(predicate::str::contains("artifact.tar.gz"))
        .stdout(predicate::str::contains("bundle.json"));
}

#[test]
fn test_bundle_requires_project() {
    // Bundle without project argument should fail
    airis()
        .arg("bundle")
        .assert()
        .failure();
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
    airis()
        .args(["policy", "check"])
        .assert()
        .success();
}
