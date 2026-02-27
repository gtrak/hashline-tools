use hashline_tools::*;

// Helper function to compute cumulative hashes for a file and get a specific line's hash
fn get_line_hash(content: &str, line_num: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut prev_hash: Option<&str> = None;
    let mut cumulative_hashes: Vec<String> = Vec::new();
    
    for (i, line) in lines.iter().enumerate() {
        let ln = i + 1;
        let hash = compute_line_hash(ln, line, prev_hash);
        cumulative_hashes.push(hash);
        prev_hash = Some(&cumulative_hashes[i]);
    }
    
    cumulative_hashes[line_num - 1].clone()
}

use hashline_tools::*;

#[test]
fn test_replace_single_line_no_duplicate() {
    let content = "line 1\nline 2\nline 3\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: get_line_hash(content, 2) },
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
            pos: AnchorRef { line: 2, hash: get_line_hash(content, 2) },
            end: Some(AnchorRef { line: 4, hash: get_line_hash(content, 4) }),
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
            pos: Some(AnchorRef { line: 2, hash: get_line_hash(content, 2) }),
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
            pos: AnchorRef { line: 2, hash: get_line_hash(content, 2) },
            end: None,
            lines: vec!["modified 2".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 4, hash: get_line_hash(content, 4) },
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
            pos: AnchorRef { line: 2, hash: get_line_hash(content, 2) },
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
            pos: AnchorRef { line: 2, hash: get_line_hash(content, 2) },
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
            pos: AnchorRef { line: 2, hash: get_line_hash(content, 2) },
            end: Some(AnchorRef { line: 4, hash: get_line_hash(content, 4) }),
            lines: vec!["replaced range".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 3, hash: get_line_hash(content, 3) },
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
            pos: AnchorRef { line: 1, hash: get_line_hash(content, 1) },
            end: Some(AnchorRef { line: 2, hash: get_line_hash(content, 2) }),
            lines: vec!["first range".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 4, hash: get_line_hash(content, 4) },
            end: Some(AnchorRef { line: 5, hash: get_line_hash(content, 5) }),
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
            pos: AnchorRef { line: 1, hash: get_line_hash(content, 1) },
            end: Some(AnchorRef { line: 2, hash: get_line_hash(content, 2) }),
            lines: vec!["first".to_string()],
        },
        HashlineEdit::Replace {
            pos: AnchorRef { line: 3, hash: get_line_hash(content, 3) },
            end: Some(AnchorRef { line: 4, hash: get_line_hash(content, 4) }),
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

#[test]
fn test_cumulative_hash_prevents_stale_edit() {
    // Test that cumulative hashes prevent editing with stale anchors
    // This reproduces the bug where editing line N, then trying to edit
    // line N again with the old hash would cause duplication/corruption
    let content = "line 1\nline 2\nline 3\n";
    
    // First edit: replace line 2
    let first_edit = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: get_line_hash(content, 2) },
            end: None,
            lines: vec!["MODIFIED".to_string()],
        }
    ];
    
    let (result, _) = apply_hashline_edits(content, &first_edit).unwrap();
    
    // Verify first edit worked
    assert!(result.contains("MODIFIED"));
    assert!(!result.contains("line 2"));
    
    // Second edit: try to replace line 2 again using the ORIGINAL hash
    // This should FAIL because the hash has changed (cumulative hash dependency)
    let stale_hash = get_line_hash(content, 2); // Get hash from ORIGINAL content
    let second_edit = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: stale_hash },
            end: None,
            lines: vec!["SHOULD_FAIL".to_string()],
        }
    ];
    
    let result2 = apply_hashline_edits(&result, &second_edit);
    
    // Should fail with hash mismatch
    assert!(result2.is_err(), "Second edit with stale hash should fail");
    let error = result2.unwrap_err().to_string();
    assert!(error.contains("have changed since last read"), "Error should mention hash mismatch");
    assert!(error.contains("2#"), "Error should mention line 2");
    
    // Verify the file wasn't corrupted (no duplication)
    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(lines.len(), 3, "Should still have 3 lines, no duplication");
    assert_eq!(lines[0], "line 1");
    assert_eq!(lines[1], "MODIFIED");
    assert_eq!(lines[2], "line 3");
}

#[test]
fn test_cumulative_hash_invalidation_chain() {
    // Test that editing an early line invalidates hashes of ALL subsequent lines
    let content = "line 1\nline 2\nline 3\nline 4\n";
    
    // Get original hashes
    let h1 = get_line_hash(content, 1);
    let h2 = get_line_hash(content, 2);
    let h3 = get_line_hash(content, 3);
    let h4 = get_line_hash(content, 4);
    
    // Edit line 2
    let edit = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: h2.clone() },
            end: None,
            lines: vec!["MODIFIED".to_string()],
        }
    ];
    
    let (result, _) = apply_hashline_edits(content, &edit).unwrap();
    
    // Now try to edit line 3 with its ORIGINAL hash
    // Should fail because line 3's hash depends on line 2's hash (cumulative)
    let result2 = apply_hashline_edits(&result, &vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 3, hash: h3 },
            end: None,
            lines: vec!["should fail".to_string()],
        }
    ]);
    
    assert!(result2.is_err(), "Edit at line 3 with stale hash should fail");
    
    // Similarly, try to edit line 4 with its ORIGINAL hash
    let result3 = apply_hashline_edits(&result, &vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 4, hash: h4 },
            end: None,
            lines: vec!["should fail".to_string()],
        }
    ]);
    
    assert!(result3.is_err(), "Edit at line 4 with stale hash should fail");
    
    // But editing line 1 should work (it's before the change)
    let result4 = apply_hashline_edits(&result, &vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: h1 },
            end: None,
            lines: vec!["line 1 modified".to_string()],
        }
    ]);
    
    assert!(result4.is_ok(), "Edit at line 1 should succeed (before the changed line)");
}

#[test]
fn test_overlapping_replace_and_prepend() {
    // Replace at line N and Prepend at line N both target position N
    let content = "line 1\nline 2\nline 3\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: get_line_hash(content, 1) },
            end: None,
            lines: vec!["replaced".to_string()],
        },
        HashlineEdit::Prepend {
            pos: Some(AnchorRef { line: 1, hash: get_line_hash(content, 1) }),
            lines: vec!["prepended".to_string()],
        }
    ];
    let result = apply_hashline_edits(content, &edits);
    assert!(result.is_err(), "Replace and prepend at same line should overlap");
}

#[test]
fn test_overlapping_append_and_prepend_same_line() {
    // Append and Prepend at same line both reference line N - conceptual overlap
    let content = "line 1\nline 2\nline 3\n";
    let edits = vec![
        HashlineEdit::Append {
            pos: Some(AnchorRef { line: 2, hash: get_line_hash(content, 2) }),
            lines: vec!["appended".to_string()],
        },
        HashlineEdit::Prepend {
            pos: Some(AnchorRef { line: 2, hash: get_line_hash(content, 2) }),
            lines: vec!["prepended".to_string()],
        }
    ];
    let result = apply_hashline_edits(content, &edits);
    assert!(result.is_err(), "Append and prepend at same line should overlap");
}

#[test]
fn test_non_overlapping_replace_and_append() {
    // Replace at line N and Append at line N don't actually overlap
    // (replace affects position N, append affects N+1)
    let content = "line 1\nline 2\nline 3\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: get_line_hash(content, 1) },
            end: None,
            lines: vec!["replaced".to_string()],
        },
        HashlineEdit::Append {
            pos: Some(AnchorRef { line: 1, hash: get_line_hash(content, 1) }),
            lines: vec!["appended".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    assert!(result.contains("replaced"));
    assert!(result.contains("appended"));
}

#[test]
fn test_non_overlapping_replace_range_and_append() {
    // Replace range 2-3 and Append at line 3 don't overlap
    // (replace 2-3 -> [2,3], append at 3 -> [4,4])
    let content = "line 1\nline 2\nline 3\nline 4\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: get_line_hash(content, 2) },
            end: Some(AnchorRef { line: 3, hash: get_line_hash(content, 3) }),
            lines: vec!["replaced".to_string()],
        },
        HashlineEdit::Append {
            pos: Some(AnchorRef { line: 3, hash: get_line_hash(content, 3) }),
            lines: vec!["appended".to_string()],
        }
    ];
    // These don't overlap - append inserts at position 4, replace is at 2-3
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    assert!(result.contains("replaced"));
    assert!(result.contains("appended"));
}

#[test]
fn test_overlapping_replace_range_with_prepend() {
    // Replace range 2-4 and Prepend at line 2 overlap
    let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 2, hash: get_line_hash(content, 2) },
            end: Some(AnchorRef { line: 4, hash: get_line_hash(content, 4) }),
            lines: vec!["replaced".to_string()],
        },
        HashlineEdit::Prepend {
            pos: Some(AnchorRef { line: 2, hash: get_line_hash(content, 2) }),
            lines: vec!["prepended".to_string()],
        }
    ];
    let result = apply_hashline_edits(content, &edits);
    assert!(result.is_err(), "Replace range 2-4 and prepend at line 2 should overlap");
}

#[test]
fn test_non_overlapping_replace_different_lines() {
    // Replace at line 1 and Append at line 3 don't overlap
    let content = "line 1\nline 2\nline 3\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: get_line_hash(content, 1) },
            end: None,
            lines: vec!["replaced 1".to_string()],
        },
        HashlineEdit::Append {
            pos: Some(AnchorRef { line: 3, hash: get_line_hash(content, 3) }),
            lines: vec!["appended".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    assert!(result.contains("replaced 1"));
    assert!(result.contains("appended"));
}

#[test]
fn test_non_overlapping_append_eof_with_replace() {
    // Append at EOF and replace not at last line don't overlap
    let content = "line 1\nline 2\nline 3\n";
    let edits = vec![
        HashlineEdit::Replace {
            pos: AnchorRef { line: 1, hash: get_line_hash(content, 1) },
            end: None,
            lines: vec!["replaced".to_string()],
        },
        HashlineEdit::Append {
            pos: None,
            lines: vec!["appended".to_string()],
        }
    ];
    let (result, _) = apply_hashline_edits(content, &edits).unwrap();
    assert!(result.contains("replaced"));
    assert!(result.contains("appended"));
}
