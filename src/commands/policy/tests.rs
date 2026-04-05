//! Tests for the policy command

use super::checkers::{check_forbidden_files, check_required_env};
use super::*;

#[test]
fn test_policy_config_default() {
    let config = PolicyConfig::default();
    assert!(!config.gates.require_clean_git);
    assert!(config.gates.require_env.is_empty());
    assert!(config.gates.forbid_files.is_empty());
}

#[test]
fn test_policy_template() {
    let template = PolicyConfig::template();
    assert!(template.contains("[gates]"));
    assert!(template.contains("require_clean_git"));
    assert!(template.contains("[security]"));
    assert!(template.contains("scan_secrets"));
}

#[test]
fn test_forbidden_files_check() {
    let temp = tempfile::tempdir().unwrap();
    let forbidden_file = temp.path().join(".env.local");
    std::fs::write(&forbidden_file, "SECRET=123").unwrap();

    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    // Use absolute path to avoid thread-safety issues with set_current_dir
    let abs_path = forbidden_file.to_string_lossy().to_string();
    check_forbidden_files(std::slice::from_ref(&abs_path), &mut result).unwrap();

    assert!(!result.violations.is_empty());
    assert!(result.violations[0].message.contains(".env.local"));
}

#[test]
fn test_required_env_missing() {
    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    check_required_env(&["DEFINITELY_NOT_SET_12345".to_string()], &mut result);

    assert!(!result.violations.is_empty());
    assert!(
        result.violations[0]
            .message
            .contains("DEFINITELY_NOT_SET_12345")
    );
}

#[test]
fn test_severity_enum() {
    assert_eq!(Severity::Error, Severity::Error);
    assert_ne!(Severity::Error, Severity::Warning);
}

// ── Mock pattern checker tests ──

#[test]
fn test_mock_pattern_detects_violation() {
    use super::checkers::check_mock_patterns;

    let temp = tempfile::tempdir().unwrap();
    let test_file = temp.path().join("api.test.ts");
    std::fs::write(
        &test_file,
        r#"
import { describe, it, vi } from 'vitest';

vi.mock('../lib/supabase', () => ({
    createClient: () => ({ from: () => ({}) }),
}));
"#,
    )
    .unwrap();

    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    let patterns = vec![r"vi\.mock.*supabase".to_string()];
    check_mock_patterns(&patterns, Some(temp.path().to_str().unwrap()), &mut result).unwrap();

    assert!(
        !result.violations.is_empty(),
        "Expected violation for vi.mock supabase"
    );
    assert_eq!(result.violations[0].rule, "testing.forbidden_patterns");
    assert!(result.violations[0].message.contains("api.test.ts"));
}

#[test]
fn test_mock_pattern_ignores_non_test_files() {
    use super::checkers::check_mock_patterns;

    let temp = tempfile::tempdir().unwrap();
    // Not a test file — should be skipped
    let src_file = temp.path().join("api.ts");
    std::fs::write(&src_file, r#"vi.mock('../lib/supabase', () => ({}));"#).unwrap();

    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    let patterns = vec![r"vi\.mock.*supabase".to_string()];
    check_mock_patterns(&patterns, Some(temp.path().to_str().unwrap()), &mut result).unwrap();

    assert!(
        result.violations.is_empty(),
        "Non-test files should not be scanned"
    );
}

#[test]
fn test_mock_pattern_clean_test_passes() {
    use super::checkers::check_mock_patterns;

    let temp = tempfile::tempdir().unwrap();
    let test_file = temp.path().join("api.integration.ts");
    std::fs::write(
        &test_file,
        r#"
import { createClient } from '@workspace/database';
const supabase = createClient(process.env.SUPABASE_URL!);
"#,
    )
    .unwrap();

    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    let patterns = vec![r"vi\.mock.*supabase".to_string()];
    check_mock_patterns(&patterns, Some(temp.path().to_str().unwrap()), &mut result).unwrap();

    assert!(result.violations.is_empty(), "Clean test file should pass");
}

#[test]
fn test_mock_pattern_multiple_patterns() {
    use super::checkers::check_mock_patterns;

    let temp = tempfile::tempdir().unwrap();
    let test_file = temp.path().join("service.spec.ts");
    std::fs::write(
        &test_file,
        r#"
jest.mock('../database', () => ({ query: jest.fn() }));
"#,
    )
    .unwrap();

    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    let patterns = vec![
        r"vi\.mock.*supabase".to_string(),
        r"jest\.mock.*database".to_string(),
    ];
    check_mock_patterns(&patterns, Some(temp.path().to_str().unwrap()), &mut result).unwrap();

    assert_eq!(result.violations.len(), 1);
    assert!(
        result.violations[0]
            .message
            .contains("jest\\.mock.*database")
    );
}

// ── Type enforcement checker tests ──

#[test]
fn test_type_enforcement_detects_missing_import() {
    use super::checkers::check_type_enforcement;

    let temp = tempfile::tempdir().unwrap();
    // Integration test that touches DB but doesn't import generated types
    let test_file = temp.path().join("users.integration.ts");
    std::fs::write(
        &test_file,
        r#"
import { describe, it } from 'vitest';
import { createClient } from '@supabase/supabase-js';

const supabase = createClient(process.env.SUPABASE_URL!, process.env.SUPABASE_KEY!);

describe('users', () => {
    it('should create user', async () => {
        const { data } = await supabase.from('users').insert({ name: 'test' });
    });
});
"#,
    )
    .unwrap();

    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    let required = vec![r"from.*@workspace/database".to_string()];
    check_type_enforcement(
        "libs/database/src/types.ts",
        &required,
        Some(temp.path().to_str().unwrap()),
        &mut result,
    )
    .unwrap();

    assert!(
        !result.violations.is_empty(),
        "Should detect missing generated type import"
    );
    assert_eq!(result.violations[0].rule, "testing.type_enforcement");
}

#[test]
fn test_type_enforcement_passes_with_import() {
    use super::checkers::check_type_enforcement;

    let temp = tempfile::tempdir().unwrap();
    let test_file = temp.path().join("users.integration.ts");
    std::fs::write(
        &test_file,
        r#"
import { describe, it } from 'vitest';
import { Database } from '@workspace/database';
import { createClient } from '@supabase/supabase-js';

const supabase = createClient<Database>(process.env.SUPABASE_URL!, process.env.SUPABASE_KEY!);

describe('users', () => {
    it('should create user', async () => {
        const { data } = await supabase.from('users').insert({ name: 'test' });
    });
});
"#,
    )
    .unwrap();

    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    let required = vec![r"from.*@workspace/database".to_string()];
    check_type_enforcement(
        "libs/database/src/types.ts",
        &required,
        Some(temp.path().to_str().unwrap()),
        &mut result,
    )
    .unwrap();

    assert!(
        result.violations.is_empty(),
        "Should pass when generated types are imported"
    );
}

#[test]
fn test_type_enforcement_skips_non_db_tests() {
    use super::checkers::check_type_enforcement;

    let temp = tempfile::tempdir().unwrap();
    // Test that doesn't touch DB — should not be flagged
    let test_file = temp.path().join("utils.spec.ts");
    std::fs::write(
        &test_file,
        r#"
import { describe, it } from 'vitest';
import { formatDate } from '../utils';

describe('formatDate', () => {
    it('formats correctly', () => {
        expect(formatDate(new Date())).toBeDefined();
    });
});
"#,
    )
    .unwrap();

    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    let required = vec![r"from.*@workspace/database".to_string()];
    check_type_enforcement(
        "libs/database/src/types.ts",
        &required,
        Some(temp.path().to_str().unwrap()),
        &mut result,
    )
    .unwrap();

    assert!(
        result.violations.is_empty(),
        "Non-DB tests should not require type imports"
    );
}

// ── Banned env vars checker tests ──

#[test]
fn test_banned_env_vars_detects_violation() {
    use super::checkers::check_banned_env_vars;

    let temp = tempfile::tempdir().unwrap();
    let src_file = temp.path().join("client.ts");
    std::fs::write(
        &src_file,
        r#"
const supabase = createClient(
    process.env.SUPABASE_URL!,
    process.env.SUPABASE_SERVICE_ROLE_KEY!,
);
"#,
    )
    .unwrap();

    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    let banned = vec!["SUPABASE_SERVICE_ROLE_KEY".to_string()];
    check_banned_env_vars(
        &banned,
        &[],
        Some(temp.path().to_str().unwrap()),
        &mut result,
    )
    .unwrap();

    assert!(
        !result.violations.is_empty(),
        "Should detect banned env var"
    );
    assert_eq!(result.violations[0].rule, "policy.security.banned_env_vars");
    assert!(
        result.violations[0]
            .message
            .contains("SUPABASE_SERVICE_ROLE_KEY")
    );
}

#[test]
fn test_banned_env_vars_skips_allowed_paths() {
    use super::checkers::check_banned_env_vars;

    let temp = tempfile::tempdir().unwrap();
    let functions_dir = temp.path().join("supabase/functions");
    std::fs::create_dir_all(&functions_dir).unwrap();
    let src_file = functions_dir.join("webhook.ts");
    std::fs::write(
        &src_file,
        r#"
const supabase = createClient(url, process.env.SUPABASE_SERVICE_ROLE_KEY!);
"#,
    )
    .unwrap();

    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    let banned = vec!["SUPABASE_SERVICE_ROLE_KEY".to_string()];
    let allowed = vec!["supabase/functions/*".to_string()];
    check_banned_env_vars(
        &banned,
        &allowed,
        Some(temp.path().to_str().unwrap()),
        &mut result,
    )
    .unwrap();

    assert!(
        result.violations.is_empty(),
        "Allowed paths should be skipped"
    );
}

#[test]
fn test_banned_env_vars_clean_code_passes() {
    use super::checkers::check_banned_env_vars;

    let temp = tempfile::tempdir().unwrap();
    let src_file = temp.path().join("client.ts");
    std::fs::write(
        &src_file,
        r#"
const supabase = createClient(
    process.env.NEXT_PUBLIC_SUPABASE_URL!,
    process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY!,
);
"#,
    )
    .unwrap();

    let mut result = PolicyResult {
        passed: true,
        ..Default::default()
    };

    let banned = vec![
        "SUPABASE_SERVICE_ROLE_KEY".to_string(),
        "SUPABASE_SECRET_KEY".to_string(),
    ];
    check_banned_env_vars(
        &banned,
        &[],
        Some(temp.path().to_str().unwrap()),
        &mut result,
    )
    .unwrap();

    assert!(
        result.violations.is_empty(),
        "Clean code using anon key should pass"
    );
}
