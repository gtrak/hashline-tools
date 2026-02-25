use hashline_tools::*;

#[test]
fn test_replace_single_line_no_duplicate() {
    let content = "line 1\nline 2\nline 3\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: compute_line_hash(2, "line 2") },
            end: None,
            lines: vec!["REPLACED".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    
    let count = result.matches("line 2").count();
    assert_eq!(count, 0, "Old content 'line 2' should not appear after replacement. Got:\n{}", result);
    
    let line_count = result.lines().count();
    assert_eq!(line_count, 3, "Should have exactly 3 lines. Got:\n{}", result);
}

#[test]
fn test_replace_range_no_duplicate() {
    let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: compute_line_hash(2, "line 2") },
            end: Some(AnchorRef { line: 4, hash: compute_line_hash(4, "line 4") }),
            lines: vec!["replaced".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    
    let line_count = result.lines().count();
    assert_eq!(line_count, 3, "Should have exactly 3 lines after range replacement. Got {} lines:\n{}", line_count, result);
    
    assert_eq!(result.matches("line 1").count(), 1);
    assert_eq!(result.matches("line 5").count(), 1);
}

#[test]
fn test_append_after_no_duplicate() {
    let content = "line 1\nline 2\n";
    let edits = vec![
        HashlineEdit::Append {
            pos: Some(AnchorRef { line: 2, hash: compute_line_hash(2, "line 2") }),
            lines: vec!["line 3".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    
    let line_count = result.lines().count();
    assert_eq!(line_count, 3, "Should have exactly 3 lines. Got {} lines:\n{}", line_count, result);
}

#[test]
fn test_multiple_edits_no_duplicate() {
    let content = "line 1\nline 2\nline 3\nline 4\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: compute_line_hash(2, "line 2") },
            end: None,
            lines: vec!["modified 2".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 4, hash: compute_line_hash(4, "line 4") },
            end: None,
            lines: vec!["modified 4".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    
    let line_count = result.lines().count();
    assert_eq!(line_count, 4, "Should have exactly 4 lines. Got {} lines:\n{}", line_count, result);
    
    assert_eq!(result.matches("line 2").count(), 0, "Should not contain 'line 2'");
    assert_eq!(result.matches("line 4").count(), 0, "Should not contain 'line 4'");
}

#[test]
fn test_replace_preserves_all_lines() {
    let content = "fn test() {\n    // comment\n}\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: compute_line_hash(2, "    // comment") },
            end: None,
            lines: vec!["    // modified comment".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    
    // Should still have 3 lines
    let line_count = result.lines().count();
    assert_eq!(line_count, 3, "Should have exactly 3 lines after replace. Got {} lines:\n{}", line_count, result);
    
    // Should contain the closing brace
    assert!(result.contains("}"), "Result should contain closing brace. Got:\n{}", result);
}

#[test]
fn test_replace_preserves_trailing_newline() {
    let content = "fn test() {\n    // comment\n}\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: compute_line_hash(2, "    // comment") },
            end: None,
            lines: vec!["    // modified".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    
    // Should preserve trailing newline
    assert!(result.ends_with("\n"), "Result should end with newline. Got: {:?}", result.as_bytes());
    
    // Should still have 3 lines (when split by \n)
    let lines: Vec<&str> = result.split('\n').collect();
    assert_eq!(lines.len(), 4, "Split by newline should have 4 elements (last is empty). Got {}: {:?}", lines.len(), lines);
}

#[test]
fn test_overlapping_edits_rejected() {
    let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
    
    // Two replace operations that overlap - one replaces lines 2-4, another replaces line 3
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: compute_line_hash(2, "line 2") },
            end: Some(AnchorRef { line: 4, hash: compute_line_hash(4, "line 4") }),
            lines: vec!["replaced range".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 3, hash: compute_line_hash(3, "line 3") },
            end: None,
            lines: vec!["replaced single".to_string()],
        },
    ];
    
    let result = apply_hashline_edits(content, &edits);
    
    // Should fail with overlapping error
    assert!(result.is_err(), "Overlapping edits should be rejected");
    let error = result.unwrap_err().to_string();
    assert!(error.contains("Overlapping edits detected"), "Error should mention overlapping edits. Got: {}", error);
    assert!(error.contains("lines 2-4"), "Error should mention first range. Got: {}", error);
    assert!(error.contains("lines 3-3"), "Error should mention second range. Got: {}", error);
}

#[test]
fn test_non_overlapping_edits_succeed() {
    let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
    
    // Two replace operations that don't overlap
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: compute_line_hash(1, "line 1") },
            end: Some(AnchorRef { line: 2, hash: compute_line_hash(2, "line 2") }),
            lines: vec!["first range".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 4, hash: compute_line_hash(4, "line 4") },
            end: Some(AnchorRef { line: 5, hash: compute_line_hash(5, "line 5") }),
            lines: vec!["second range".to_string()],
        },
    ];
    
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    
    // Should succeed - lines 1-2 and 4-5 are replaced, line 3 unchanged
    assert!(result.contains("first range"));
    assert!(result.contains("second range"));
    assert!(result.contains("line 3"));
    assert!(!result.contains("line 1"));
    assert!(!result.contains("line 2"));
    assert!(!result.contains("line 4"));
    assert!(!result.contains("line 5"));
}

#[test]
fn test_adjacent_edits_succeed() {
    let content = "line 1\nline 2\nline 3\nline 4\n";
    
    // Two replace operations that are adjacent but not overlapping
    // Replacing lines 1-2 and lines 3-4 - line 2 ends at 2, line 3 starts at 3
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: compute_line_hash(1, "line 1") },
            end: Some(AnchorRef { line: 2, hash: compute_line_hash(2, "line 2") }),
            lines: vec!["first".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 3, hash: compute_line_hash(3, "line 3") },
            end: Some(AnchorRef { line: 4, hash: compute_line_hash(4, "line 4") }),
            lines: vec!["second".to_string()],
        },
    ];
    
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    
    // Should succeed - adjacent edits are fine
    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "first");
    assert_eq!(lines[1], "second");
}
