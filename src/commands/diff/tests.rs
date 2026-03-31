//! Tests for the diff command

use super::FileStatus;
use super::compute::{compute_unified_diff, format_new_file_diff};

#[test]
fn test_file_status_serialize() {
    assert_eq!(
        serde_json::to_string(&FileStatus::Created).unwrap(),
        "\"created\""
    );
    assert_eq!(
        serde_json::to_string(&FileStatus::Modified).unwrap(),
        "\"modified\""
    );
}

#[test]
fn test_format_new_file_diff() {
    let diff = format_new_file_diff("test.txt", "line1\nline2");
    assert!(diff.contains("+++ test.txt"));
    assert!(diff.contains("+line1"));
    assert!(diff.contains("+line2"));
}

#[test]
fn test_compute_unified_diff_no_changes() {
    let content = "line1\nline2\nline3";
    let (adds, dels, _) = compute_unified_diff("test.txt", content, content);
    assert_eq!(adds, 0);
    assert_eq!(dels, 0);
}

#[test]
fn test_compute_unified_diff_with_changes() {
    let current = "line1\nold\nline3";
    let expected = "line1\nnew\nline3";
    let (adds, dels, diff) = compute_unified_diff("test.txt", current, expected);
    assert_eq!(adds, 1);
    assert_eq!(dels, 1);
    assert!(diff.contains("-old"));
    assert!(diff.contains("+new"));
}
