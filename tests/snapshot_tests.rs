use hashline_tools::*;
use regex::Regex;
use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

fn create_test_file(content: &str) -> (NamedTempFile, String) {
    let mut temp_file = NamedTempFile::new().unwrap();
    write!(temp_file, "{}", content).unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();
    (temp_file, path)
}

fn normalize_edit_output(result: &str) -> String {
    // Replace temp file paths with a placeholder
    let re = Regex::new(r"/tmp/\.tmp\w+").unwrap();
    re.replace_all(result, "<TEMP_FILE>").to_string()
}

#[test]
fn snapshot_cmd_read_simple_file() {
    let (_temp_file, path) = create_test_file("line 1\nline 2\nline 3\n");
    let result = cmd_read(&path, None, None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_cmd_read_empty_file() {
    let (_temp_file, path) = create_test_file("");
    let result = cmd_read(&path, None, None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_cmd_read_with_offset() {
    let mut temp_file = NamedTempFile::new().unwrap();
    for i in 1..=20 {
        writeln!(temp_file, "line {}", i).unwrap();
    }
    let path = temp_file.path().to_str().unwrap().to_string();
    let result = cmd_read(&path, Some(5), Some(5)).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_cmd_read_with_trailing_content() {
    let mut temp_file = NamedTempFile::new().unwrap();
    for i in 1..=50 {
        writeln!(temp_file, "line {}", i).unwrap();
    }
    let path = temp_file.path().to_str().unwrap().to_string();
    let result = cmd_read(&path, None, None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_cmd_read_single_line_file() {
    let (_temp_file, path) = create_test_file("only line\n");
    let result = cmd_read(&path, None, None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_cmd_read_unicode() {
    let (_temp_file, path) = create_test_file("Hello 世界\n🎉 Emoji test\n\n");
    let result = cmd_read(&path, None, None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_cmd_read_windows_line_endings() {
    let mut temp_file = NamedTempFile::new().unwrap();
    write!(temp_file, "line 1\r\nline 2\r\nline 3\r\n").unwrap();
    let path = temp_file.path().to_str().unwrap().to_string();
    let result = cmd_read(&path, None, None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_cmd_read_file_with_only_newlines() {
    let (_temp_file, path) = create_test_file("\n\n\n");
    let result = cmd_read(&path, None, None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_cmd_read_file_with_only_whitespace_lines() {
    let (_temp_file, path) = create_test_file("   \n  \n\t\n");
    let result = cmd_read(&path, None, None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_cmd_read_offset_beyond_file() {
    let (_temp_file, path) = create_test_file("line 1\nline 2\n");
    let result = cmd_read(&path, Some(100), None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_compute_line_hash_determinism() {
    // Hash should be deterministic for same input
    let hash1 = compute_line_hash(1, "test line");
    let hash2 = compute_line_hash(1, "test line");
    assert_eq!(hash1, hash2);
    // Hash should be 2 characters
    assert_eq!(hash1.len(), 2);
}

#[test]
fn snapshot_compute_line_hash_edge_cases() {
    // Empty line
    let hash1 = compute_line_hash(1, "");
    assert_eq!(hash1.len(), 2);
    
    // Line with only whitespace (should use line number as seed)
    let hash2 = compute_line_hash(1, "   \t\n");
    let hash3 = compute_line_hash(2, "   \t\n");
    // Both should be 2 characters
    assert_eq!(hash2.len(), 2);
    assert_eq!(hash3.len(), 2);
    
    // Same content, different line numbers (non-whitespace)
    let hash4 = compute_line_hash(1, "content");
    let hash5 = compute_line_hash(2, "content");
    // Same content should produce same hash regardless of line number (when it has alphanumeric)
    assert_eq!(hash4, hash5);
}

#[test]
fn snapshot_format_line_tag() {
    let tag = format_line_tag(42, "test content");
    // Format should be "LINE#HASH"
    assert!(tag.contains('#'));
    let parts: Vec<&str> = tag.split('#').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "42");
    assert_eq!(parts[1].len(), 2);
}

#[test]
fn snapshot_apply_hashline_edits_replace_single() {
    let content = "first\nsecond\nthird\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: compute_line_hash(2, "second") },
            end: None,
            lines: vec!["replaced".to_string()],
        }
    ];
    let (result, first_changed) = apply_hashline_edits(content, &edits).unwrap();
    let output = format!("Result:\n{}\n\nFirst changed line: {:?}", result, first_changed);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_apply_hashline_edits_replace_range() {
    let content = "first\nsecond\nthird\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: compute_line_hash(1, "first") },
            end: Some(AnchorRef { line: 3, hash: compute_line_hash(3, "third") }),
            lines: vec!["replaced".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_hashline_edits_append() {
    let content = "first\nsecond\n";
    let edits = vec![
        HashlineEdit::Append {
            pos: Some(AnchorRef { line: 1, hash: compute_line_hash(1, "first") }),
            lines: vec!["inserted".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_hashline_edits_append_eof() {
    let content = "first\nsecond\n";
    let edits = vec![
        HashlineEdit::Append {
            pos: None,
            lines: vec!["at eof".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_hashline_edits_prepend() {
    let content = "first\nsecond\n";
    let edits = vec![
        HashlineEdit::Prepend {
            pos: Some(AnchorRef { line: 2, hash: compute_line_hash(2, "second") }),
            lines: vec!["before".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_hashline_edits_prepend_bof() {
    let content = "first\nsecond\n";
    let edits = vec![
        HashlineEdit::Prepend {
            pos: None,
            lines: vec!["at bof".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_hashline_edits_empty_content() {
    let content = "";
    let edits = vec![
        HashlineEdit::Append {
            pos: None,
            lines: vec!["new line".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_hashline_edits_empty_new_text() {
    let content = "first\nsecond\nthird\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: compute_line_hash(2, "second") },
            end: None,
            lines: vec![],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_hashline_edits_to_empty_file() {
    let content = "";
    let edits = vec![
        HashlineEdit::Append {
            pos: None,
            lines: vec!["line 1".to_string(), "line 2".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_hashline_edits_single_line() {
    let content = "only\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: compute_line_hash(1, "only") },
            end: None,
            lines: vec!["modified".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_hashline_edits_multiple_operations() {
    let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
    let edits = vec![
        HashlineEdit::Append {
            pos: Some(AnchorRef { line: 1, hash: compute_line_hash(1, "line 1") }),
            lines: vec!["new line 1.5".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 5, hash: compute_line_hash(5, "line 5") },
            end: None,
            lines: vec!["modified line 5".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_parse_anchor_valid() {
    let result = parse_anchor("5#ab");
    assert_eq!(result, Some((5, "ab".to_string())));
}

#[test]
fn snapshot_parse_anchor_old_format() {
    // Should still support old format for backward compatibility
    let result = parse_anchor("5:abc1");
    assert_eq!(result, Some((5, "abc1".to_string())));
}

#[test]
fn snapshot_parse_anchor_invalid_formats() {
    assert_eq!(parse_anchor("invalid"), None);
    assert_eq!(parse_anchor(""), None);
    assert_eq!(parse_anchor("abc#def"), None); // non-numeric line
}

#[test]
fn snapshot_hashline_mismatch_error() {
    let content = "first\nsecond\nthird\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: "ZZ".to_string() }, // Wrong hash
            end: None,
            lines: vec!["replaced".to_string()],
        }
    ];
    let result = apply_hashline_edits(content, &edits);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("changed since last read"));
}

#[test]
fn snapshot_hashline_line_out_of_range() {
    let content = "first\nsecond\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 10, hash: "AB".to_string() },
            end: None,
            lines: vec!["replaced".to_string()],
        }
    ];
    let result = apply_hashline_edits(content, &edits);
    assert!(result.is_err());
}

#[test]
fn snapshot_hashline_append_after_last_line() {
    let content = "first\nsecond\n";
    let edits = vec![
        HashlineEdit::Append {
            pos: Some(AnchorRef { line: 2, hash: compute_line_hash(2, "second") }),
            lines: vec!["third".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_hashline_edits_with_special_characters() {
    let content = "line with \t tabs\nline with unicode: 你好\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: compute_line_hash(1, "line with \t tabs") },
            end: None,
            lines: vec!["replaced".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_hashline_edits_no_changes() {
    let content = "first\nsecond\nthird\n";
    let edits: Vec<HashlineEdit> = vec![];
    let (result, first_changed) = apply_hashline_edits(content, &edits).unwrap();
    assert!(result.starts_with("first"));
    assert_eq!(first_changed, None);
}

#[test]
fn snapshot_apply_hashline_edits_replace_lines_range_mismatch() {
    let content = "line 1\nline 2\nline 3\n";
    // Test that start line must be <= end line
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 3, hash: compute_line_hash(3, "line 3") },
            end: Some(AnchorRef { line: 1, hash: compute_line_hash(1, "line 1") }),
            lines: vec!["replaced".to_string()],
        }
    ];
    let result = apply_hashline_edits(content, &edits);
    assert!(result.is_err());
}

#[test]
fn snapshot_apply_hashline_edits_deduplication() {
    let content = "first\nsecond\n";
    let edit = HashlineEdit::Replace {
        pos: AnchorRef { line: 1, hash: compute_line_hash(1, "first") },
        end: None,
        lines: vec!["replaced".to_string()],
    };
    // Duplicate edits should be deduplicated
    let edits = vec![edit.clone(), edit];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    // Should only apply once
    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(lines.len(), 2);
}

#[test]
fn snapshot_apply_hashline_edits_noop_detection() {
    let content = "first\nsecond\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: compute_line_hash(1, "first") },
            end: None,
            lines: vec!["first".to_string()], // Same content
        }
    ];
    let (result, first_changed) = apply_hashline_edits(content, &edits).unwrap();
    // Should be detected as no-op but still applied
    assert!(result.starts_with("first"));
    // first_changed should still be set to the edit position
    assert_eq!(first_changed, Some(1));
}

#[test]
fn test_multiple_edits_applied_bottom_up() {
    // Test that edits are applied bottom-up (highest line first)
    // If prepend at line 1 is applied before replace at line 2,
    // the replace position would shift and fail
    let content = "line 1\nline 2\nline 3\n";
    let h1 = compute_line_hash(1, "line 1");
    let h2 = compute_line_hash(2, "line 2");
    let h3 = compute_line_hash(3, "line 3");

    let edits = vec![
        HashlineEdit::Prepend {
            pos: Some(AnchorRef { line: 1, hash: h1.clone() }),
            lines: vec!["prepended".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: h2.clone() },
            end: None,
            lines: vec!["replaced".to_string()],
        },
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();

    // If sorted correctly: prepend at 1, then replace at 2 (now at 3)
    // If sorted wrong: prepend shifts line 2 to 3, replace fails or corrupts
    let expected = "prepended\nline 1\nreplaced\nline 3\n";
    assert_eq!(result, expected, "Edits should be applied bottom-up so line numbers stay valid");
}

#[test]
fn test_three_edits_bottom_up() {
    // Three edits at different positions - must process in reverse order
    let content = "a\nb\nc\nd\ne\n";
    let h1 = compute_line_hash(1, "a");
    let h2 = compute_line_hash(2, "b");
    let h4 = compute_line_hash(4, "d");

    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: h1.clone() },
            end: None,
            lines: vec!["A".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 4, hash: h4.clone() },
            end: None,
            lines: vec!["D".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: h2.clone() },
            end: None,
            lines: vec!["B".to_string()],
        },
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();

    // All three lines should be replaced correctly
    assert!(!result.contains("a"));
    assert!(!result.contains("b"));
    assert!(!result.contains("d"));
    assert!(result.contains("A"));
    assert!(result.contains("B"));
    assert!(result.contains("D"));
}

