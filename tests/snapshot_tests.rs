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
fn snapshot_cmd_edit_set_line() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    let content = fs::read_to_string(temp_file.path()).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let hash = compute_line_hash(lines[1]);
    
    let edits = format!(r#"[{{"type":"set_line","anchor":"2:{}","new_text":"replaced"}}]"#, hash);
    let result = cmd_edit(&path, &edits).unwrap();
    insta::assert_snapshot!(normalize_edit_output(&result));
}

#[test]
fn snapshot_cmd_edit_replace_lines() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    let content = fs::read_to_string(temp_file.path()).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let hash1 = compute_line_hash(lines[0]);
    let hash3 = compute_line_hash(lines[2]);
    
    let edits = format!(r#"[{{"type":"replace_lines","start_anchor":"1:{}","end_anchor":"3:{}","new_text":"replaced"}}]"#, hash1, hash3);
    let result = cmd_edit(&path, &edits).unwrap();
    insta::assert_snapshot!(normalize_edit_output(&result));
}

#[test]
fn snapshot_cmd_edit_insert_after() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    let content = fs::read_to_string(temp_file.path()).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let hash = compute_line_hash(lines[0]);
    
    let edits = format!(r#"[{{"type":"insert_after","anchor":"1:{}","text":"inserted"}}]"#, hash);
    let result = cmd_edit(&path, &edits).unwrap();
    insta::assert_snapshot!(normalize_edit_output(&result));
}

#[test]
fn snapshot_cmd_edit_replace_fuzzy() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    
    let edits = r#"[{"type":"replace","old_text":"second","new_text":"modified","all":false}]"#;
    let result = cmd_edit(&path, edits).unwrap();
    insta::assert_snapshot!(normalize_edit_output(&result));
}

#[test]
fn snapshot_cmd_edit_replace_all() {
    let (temp_file, path) = create_test_file("alpha\nbeta\ngamma\n");
    
    let edits = r#"[{"type":"replace","old_text":"a","new_text":"@","all":true}]"#;
    let result = cmd_edit(&path, edits).unwrap();
    insta::assert_snapshot!(normalize_edit_output(&result));
}

#[test]
fn snapshot_cmd_edit_no_changes() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    
    let edits = r#"[{"type":"replace","old_text":"nonexistent","new_text":"modified","all":false}]"#;
    let result = cmd_edit(&path, edits);
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_cmd_edit_hash_mismatch() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    
    let edits = r#"[{"type":"set_line","anchor":"2:zzzz","new_text":"test"}]"#;
    let result = cmd_edit(&path, edits);
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_cmd_edit_multiple_operations() {
    let (temp_file, path) = create_test_file("line 1\nline 2\nline 3\nline 4\nline 5\n");
    let content = fs::read_to_string(temp_file.path()).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let hash1 = compute_line_hash(lines[0]);
    let hash5 = compute_line_hash(lines[4]);
    
    let edits = format!(r#"[{{"type":"insert_after","anchor":"1:{}","text":"new line 1.5"}},{{"type":"set_line","anchor":"5:{}","new_text":"modified line 5"}}]"#, hash1, hash5);
    let result = cmd_edit(&path, &edits).unwrap();
    insta::assert_snapshot!(normalize_edit_output(&result));
}

#[test]
fn snapshot_parse_edits_flat_format() {
    let json = r#"[{"type":"set_line","anchor":"1:abcd","new_text":"test"}]"#;
    let result = parse_edits(json).unwrap();
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_parse_edits_nested_format() {
    let json = r#"[{"set_line":{"anchor":"1:abcd","new_text":"test"}}]"#;
    let result = parse_edits(json).unwrap();
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_parse_edits_multiple() {
    let json = r#"[{"type":"set_line","anchor":"1:abcd","new_text":"line1"},{"type":"insert_after","anchor":"2:efgh","text":"line2"}]"#;
    let result = parse_edits(json).unwrap();
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_normalize_whitespace() {
    let test_cases = vec![
        "hello   world",
        "  leading",
        "trailing  ",
        "  both  ",
        "\ttab\tspace",
        "\nnewline\n",
    ];
    
    let results: Vec<String> = test_cases
        .iter()
        .map(|s| normalize_whitespace(s))
        .collect();
    insta::assert_snapshot!(format!("{:#?}", results));
}

#[test]
fn snapshot_normalize_line() {
    let test_cases = vec![
        "hello\r",
        "hello  world\r",
        "hello\n",
        "  hello  world  ",
    ];
    
    let results: Vec<String> = test_cases
        .iter()
        .map(|s| normalize_line(s))
        .collect();
    insta::assert_snapshot!(format!("{:#?}", results));
}

#[test]
fn snapshot_levenshtein_examples() {
    let test_cases = vec![
        ("kitten", "sitting"),
        ("hello", "world"),
        ("", ""),
        ("a", ""),
        ("abc", "abc"),
    ];
    
    let results: Vec<(String, String, usize)> = test_cases
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string(), levenshtein(a, b)))
        .collect();
    insta::assert_snapshot!(format!("{:#?}", results));
}

#[test]
fn snapshot_similarity_examples() {
    let test_cases = vec![
        ("hello", "hello"),
        ("hello", "hellp"),
        ("abc", "xyz"),
        ("", ""),
        ("a", "b"),
    ];
    
    let results: Vec<(String, String, f64)> = test_cases
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string(), similarity(a, b)))
        .collect();
    insta::assert_snapshot!(format!("{:#?}", results));
}

#[test]
fn snapshot_compute_line_hash_determinism() {
    let lines = vec!["hello world", "  hello world  ", "hello\tworld"];
    let hashes: Vec<String> = lines.iter().map(|l| compute_line_hash(l)).collect();
    insta::assert_snapshot!(format!("{:#?}", hashes));
}

#[test]
fn snapshot_to_base36_range() {
    let hash_mod: u64 = 36 * 36 * 36 * 36;
    let values = vec![0, 1, 35, 36, 1295, 1296, hash_mod - 1, hash_mod / 2];
    let results: Vec<String> = values.iter().map(|&v| to_base36(v)).collect();
    insta::assert_snapshot!(format!("{:#?}", results));
}

#[test]
fn snapshot_find_fuzzy_match_exact() {
    let content = "exact match here";
    let result = find_fuzzy_match(content, "exact match");
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_find_fuzzy_match_normalized() {
    let content = "  match  here  ";
    let result = find_fuzzy_match(content, "match here");
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_find_fuzzy_match_similar() {
    let content = "hello world here";
    let result = find_fuzzy_match(content, "hello wrld");
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_find_fuzzy_match_no_match() {
    let content = "completely different content";
    let result = find_fuzzy_match(content, "xyzzy");
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_fuzzy_match_multiple_candidates() {
    let content = "hello\nhello\nhello";
    let result = find_fuzzy_match(content, "hell");
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_cmd_edit_with_special_characters() {
    let (temp_file, path) = create_test_file("hello \"world\"\nsecond 'line'\nthird\n");
    
    let edits = r#"[{"type":"replace","old_text":"hello \"world\"","new_text":"replaced","all":false}]"#;
    let result = cmd_edit(&path, edits).unwrap();
    insta::assert_snapshot!(normalize_edit_output(&result));
}

#[test]
fn snapshot_cmd_read_unicode() {
    let (temp_file, path) = create_test_file("café\nnaïve\nüber\n");
    let result = cmd_read(&path, None, None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_edits_empty_content() {
    let content = "";
    let edits = r#"[]"#;
    let result = apply_edits(content, edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_apply_edits_single_line() {
    let content = "single line";
    let edits = r#"[{"type":"replace","old_text":"single","new_text":"only","all":false}]"#;
    let result = apply_edits(content, edits).unwrap();
    insta::assert_snapshot!(result);
}


// ===== Edge Case Tests =====

#[test]
fn snapshot_parse_anchor_invalid_formats() {
    let cases = vec![
        ("no_colon", "Missing colon"),
        (":only_hash", "Line 0 (invalid)"),
        ("abc:hash", "Non-numeric line"),
        ("1:", "Empty hash"),
        ("1", "No hash at all"),
    ];
    
    let results: Vec<(String, Option<(usize, String)>)> = cases
        .into_iter()
        .map(|(input, desc)| (desc.to_string(), parse_anchor(input)))
        .collect();
    insta::assert_snapshot!(format!("{:#?}", results));
}


#[test]
fn snapshot_cmd_read_offset_beyond_file() {
    let (_temp_file, path) = create_test_file("line 1\nline 2\nline 3\n");
    let result = cmd_read(&path, Some(100), None).unwrap();
    insta::assert_snapshot!(result);
}
#[test]
fn snapshot_cmd_read_file_with_only_whitespace_lines() {
    let (_temp_file, path) = create_test_file("  \n\t\n   \n");
    let result = cmd_read(&path, None, None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_cmd_edit_line_number_zero() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    let content = fs::read_to_string(temp_file.path()).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let hash = compute_line_hash(lines[0]);
    
    let edits = format!(r#"[{{"type":"set_line","anchor":"0:{}","new_text":"replaced"}}]"#, hash);
    let result = cmd_edit(&path, &edits);
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_cmd_edit_line_beyond_file() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    let content = fs::read_to_string(temp_file.path()).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let hash = compute_line_hash(lines[0]);
    
    let edits = format!(r#"[{{"type":"set_line","anchor":"100:{}","new_text":"replaced"}}]"#, hash);
    let result = cmd_edit(&path, &edits);
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_cmd_edit_set_line_empty_new_text() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    let content = fs::read_to_string(temp_file.path()).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let hash = compute_line_hash(lines[1]);
    
    let edits = format!(r#"[{{"type":"set_line","anchor":"2:{}","new_text":""}}]"#, hash);
    let result = cmd_edit(&path, &edits).unwrap();
    insta::assert_snapshot!(normalize_edit_output(&result));
}

#[test]
fn snapshot_cmd_edit_replace_lines_empty_new_text() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    let content = fs::read_to_string(temp_file.path()).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let hash1 = compute_line_hash(lines[0]);
    let hash3 = compute_line_hash(lines[2]);
    
    let edits = format!(r#"[{{"type":"replace_lines","start_anchor":"1:{}","end_anchor":"3:{}","new_text":""}}]"#, hash1, hash3);
    let result = cmd_edit(&path, &edits).unwrap();
    insta::assert_snapshot!(normalize_edit_output(&result));
}

#[test]
fn snapshot_cmd_edit_insert_after_last_line() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    let content = fs::read_to_string(temp_file.path()).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let hash = compute_line_hash(lines[2]);
    
    let edits = format!(r#"[{{"type":"insert_after","anchor":"3:{}","text":"fourth"}}]"#, hash);
    let result = cmd_edit(&path, &edits).unwrap();
    insta::assert_snapshot!(normalize_edit_output(&result));
}

#[test]
fn snapshot_cmd_edit_replace_all_with_empty_string() {
    let (temp_file, path) = create_test_file("aaa\naaa\naaa\n");
    
    let edits = r#"[{"type":"replace","old_text":"aaa","new_text":"","all":true}]"#;
    let result = cmd_edit(&path, edits).unwrap();
    insta::assert_snapshot!(normalize_edit_output(&result));
}

#[test]
fn snapshot_parse_edits_invalid_json() {
    let json = r#"not valid json"#;
    let result = parse_edits(json);
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_parse_edits_empty_array() {
    let json = r#"[]"#;
    let result = parse_edits(json).unwrap();
    insta::assert_snapshot!(format!("{:?}", result));
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
fn snapshot_apply_edits_empty_new_text_fuzzy_replace() {
    let content = "delete this line";
    let edits = r#"[{"type":"replace","old_text":"this","new_text":"","all":false}]"#;
    let result = apply_edits(content, edits).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_cmd_read_single_line_file() {
    let (_temp_file, path) = create_test_file("only one line");
    let result = cmd_read(&path, None, None).unwrap();
    insta::assert_snapshot!(result);
}

#[test]
fn snapshot_levenshtein_same_string() {
    let result = levenshtein("identical", "identical");
    insta::assert_snapshot!(format!("{}", result));
}

#[test]
fn snapshot_similarity_edge_cases() {
    let cases = vec![
        ("a", "ab"),
        ("ab", "a"),
        ("abc", "def"),
        ("case", "CaSe"),
    ];
    
    let results: Vec<(String, String, f64)> = cases
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string(), similarity(a, b)))
        .collect();
    insta::assert_snapshot!(format!("{:#?}", results));
}

#[test]
fn snapshot_normalize_whitespace_only() {
    let cases = vec![
        "   ",
        "\t\t\t",
        "\n\n",
        " \t\n \t\n",
    ];
    
    let results: Vec<String> = cases
        .iter()
        .map(|s| normalize_whitespace(s))
        .collect();
    insta::assert_snapshot!(format!("{:#?}", results));
}

#[test]
fn snapshot_compute_line_hash_edge_cases() {
    let cases = vec![
        "",
        " ",
        "\t",
        "\n",
        "\r\n",
        "a",
        "aa",
        "very long line with many characters to test hashing",
    ];
    
    let results: Vec<(String, String)> = cases
        .iter()
        .map(|s| (format!("{:?}", s), compute_line_hash(s)))
        .collect();
    insta::assert_snapshot!(format!("{:#?}", results));
}

#[test]
fn snapshot_apply_edits_to_empty_file() {
    let content = "";
    let edits = r#"[{"type":"replace","old_text":"x","new_text":"y","all":false}]"#;
    let result = apply_edits(content, edits);
    insta::assert_snapshot!(format!("{:?}", result));
}

#[test]
fn snapshot_cmd_edit_replace_lines_range_mismatch() {
    let (temp_file, path) = create_test_file("first\nsecond\nthird\n");
    let content = fs::read_to_string(temp_file.path()).unwrap();
    let lines: Vec<&str> = content.lines().collect();
    let hash1 = compute_line_hash(lines[0]);
    
    // Start > end should fail
    let edits = format!(r#"[{{"type":"replace_lines","start_anchor":"3:{}","end_anchor":"1:{}","new_text":"replaced"}}]"#, hash1, hash1);
    let result = cmd_edit(&path, &edits);
    insta::assert_snapshot!(format!("{:?}", result));
}
