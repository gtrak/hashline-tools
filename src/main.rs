use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::cmp;
use std::fs;
use std::hash::Hasher;
use twox_hash::XxHash64;

const HASH_MOD: u64 = 36 * 36 * 36 * 36;
const RADIX: u64 = 36;

// Similarity thresholds for fuzzy matching
const SINGLE_CANDIDATE_SIMILARITY_THRESHOLD: f64 = 0.0;
const MULTIPLE_CANDIDATES_SIMILARITY_THRESHOLD: f64 = 0.3;

/// Compute hash for a line (whitespace normalized)
fn compute_line_hash(line: &str) -> String {
    let normalized: String = normalize_whitespace(line);
    let mut hasher = XxHash64::with_seed(0);
    hasher.write(normalized.as_bytes());
    let hash = hasher.finish() % HASH_MOD;
    to_base36(hash)
}

/// Normalize whitespace: remove all whitespace characters
fn normalize_whitespace(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn to_base36(mut n: u64) -> String {
    let mut chars = Vec::new();
    for _ in 0..4 {
        let rem = (n % RADIX) as u8;
        chars.push(if rem < 10 {
            b'0' + rem
        } else {
            b'a' + rem - 10
        });
        n /= RADIX;
    }
    chars.reverse();
    String::from_utf8(chars).unwrap()
}

/// Levenshtein distance algorithm
fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    if a_len == 0 || b_len == 0 {
        return cmp::max(a_len, b_len);
    }

    let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    for (i, a_char) in a.chars().enumerate() {
        for (j, b_char) in b.chars().enumerate() {
            let cost = if a_char == b_char { 0 } else { 1 };
            matrix[i + 1][j + 1] = cmp::min(
                cmp::min(matrix[i][j + 1] + 1, matrix[i + 1][j] + 1),
                matrix[i][j] + cost,
            );
        }
    }

    matrix[a_len][b_len]
}

/// Calculate similarity between two strings (0.0 to 1.0)
fn similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let max_len = cmp::max(a.len(), b.len()) as f64;
    let distance = levenshtein(a, b) as f64;
    1.0 - (distance / max_len)
}

/// Find best fuzzy match for old_text in content
fn find_fuzzy_match(content: &str, old_text: &str) -> Result<(usize, String), String> {
    // First try exact match
    if let Some(pos) = content.find(old_text) {
        return Ok((pos, old_text.to_string()));
    }

    // Try normalized match (whitespace collapsed)
    let normalized_old = normalize_whitespace(old_text);
    let normalized_content = normalize_whitespace(content);

    if let Some(pos) = normalized_content.find(&normalized_old) {
        // Map position back to original content
        let mut original_pos = 0;
        let mut normalized_pos = 0;
        for (i, c) in content.chars().enumerate() {
            if normalized_pos == pos {
                original_pos = i;
                break;
            }
            if !c.is_whitespace() {
                normalized_pos += 1;
            }
        }
        let end_pos = (original_pos + old_text.len() * 2).min(content.len());
        return Ok((original_pos, content[original_pos..end_pos].to_string()));
    }

    // Fuzzy matching with Levenshtein distance
    let candidates: Vec<(usize, &str)> = content
        .lines()
        .enumerate()
        .map(|(i, line)| (i + 1, line))
        .collect();

    let mut best_matches: Vec<(usize, f64, &str)> = candidates
        .into_iter()
        .map(|(line_num, line)| {
            let sim = similarity(line, old_text);
            (line_num, sim, line)
        })
        .filter(|(_, sim, _)| *sim >= SINGLE_CANDIDATE_SIMILARITY_THRESHOLD)
        .collect();

    best_matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    if best_matches.is_empty() {
        return Err(format!(
            "Could not find \"{}\" in content. The text may have been modified.",
            old_text.chars().take(50).collect::<String>()
        ));
    }

    // Check if we have multiple similar candidates
    let top_similarity = best_matches[0].1;
    let similar_candidates: Vec<_> = best_matches
        .iter()
        .filter(|(_, sim, _)| *sim >= MULTIPLE_CANDIDATES_SIMILARITY_THRESHOLD)
        .collect();

    if similar_candidates.len() > 1 {
        // Multiple candidates - need more context
        let candidates_str = similar_candidates
            .iter()
            .take(3)
            .map(|(line, sim, text)| {
                format!(
                    "  Line {} (similarity {:.0}%): {}",
                    line,
                    sim * 100.0,
                    text.chars().take(50).collect::<String>()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        return Err(format!(
            "Multiple matches found for \"{}\". Provide more context to identify the correct location:\n{}",
            old_text.chars().take(50).collect::<String>(),
            candidates_str
        ));
    }

    // Single best match
    let (_line_num, sim, matched_line) = best_matches[0];
    if sim >= MULTIPLE_CANDIDATES_SIMILARITY_THRESHOLD {
        // Good match - use it
        let pos = content.find(matched_line).unwrap_or(0);
        return Ok((pos, matched_line.to_string()));
    }

    Err(format!(
        "Could not find \"{}\". Best match (similarity {:.0}%): \"{}\"",
        old_text.chars().take(50).collect::<String>(),
        top_similarity * 100.0,
        best_matches[0].2.chars().take(50).collect::<String>()
    ))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum Edit {
    #[serde(rename = "set_line")]
    SetLine { anchor: String, new_text: String },
    #[serde(rename = "replace_lines")]
    ReplaceLines {
        start_anchor: String,
        end_anchor: String,
        new_text: String,
    },
    #[serde(rename = "insert_after")]
    InsertAfter { anchor: String, text: String },
    #[serde(rename = "replace")]
    Replace {
        old_text: String,
        new_text: String,
        all: Option<bool>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum EditInput {
    Flat(Edit),
    NestedSetLine { set_line: SetLineEdit },
    NestedReplaceLines { replace_lines: ReplaceLinesEdit },
    NestedInsertAfter { insert_after: InsertAfterEdit },
    NestedReplace { replace: ReplaceEdit },
}

#[derive(Debug, Deserialize)]
pub struct SetLineEdit {
    anchor: String,
    new_text: String,
}
#[derive(Debug, Deserialize)]
pub struct ReplaceLinesEdit {
    start_anchor: String,
    end_anchor: String,
    new_text: String,
}
#[derive(Debug, Deserialize)]
pub struct InsertAfterEdit {
    anchor: String,
    text: String,
}
#[derive(Debug, Deserialize)]
pub struct ReplaceEdit {
    old_text: String,
    new_text: String,
    all: Option<bool>,
}

fn parse_anchor(anchor: &str) -> Option<(usize, String)> {
    let parts: Vec<&str> = anchor.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }
    let line: usize = parts[0].parse().ok()?;
    let hash = parts[1].to_string();
    Some((line, hash))
}

#[derive(Clone)]
enum Op {
    SetLine(String, String),
    ReplaceLines(String, String, String),
    InsertAfter(String, String),
    Replace(String, String, bool),
}

fn parse_edits(edits_json: &str) -> Result<Vec<Op>, String> {
    let edit_inputs: Vec<EditInput> =
        serde_json::from_str(edits_json).map_err(|e| format!("Failed to parse edits: {}", e))?;
    let mut ops = Vec::new();
    for input in edit_inputs {
        let edit = match input {
            EditInput::Flat(e) => e,
            EditInput::NestedSetLine { set_line } => Edit::SetLine {
                anchor: set_line.anchor,
                new_text: set_line.new_text,
            },
            EditInput::NestedReplaceLines { replace_lines } => Edit::ReplaceLines {
                start_anchor: replace_lines.start_anchor,
                end_anchor: replace_lines.end_anchor,
                new_text: replace_lines.new_text,
            },
            EditInput::NestedInsertAfter { insert_after } => Edit::InsertAfter {
                anchor: insert_after.anchor,
                text: insert_after.text,
            },
            EditInput::NestedReplace { replace } => Edit::Replace {
                old_text: replace.old_text,
                new_text: replace.new_text,
                all: replace.all,
            },
        };
        match edit {
            Edit::SetLine { anchor, new_text } => ops.push(Op::SetLine(anchor, new_text)),
            Edit::ReplaceLines {
                start_anchor,
                end_anchor,
                new_text,
            } => ops.push(Op::ReplaceLines(start_anchor, end_anchor, new_text)),
            Edit::InsertAfter { anchor, text } => ops.push(Op::InsertAfter(anchor, text)),
            Edit::Replace {
                old_text,
                new_text,
                all,
            } => ops.push(Op::Replace(old_text, new_text, all.unwrap_or(false))),
        }
    }
    Ok(ops)
}

fn apply_edits(content: &str, edits_json: &str) -> Result<String, String> {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let ops = parse_edits(edits_json)?;

    // First pass: validate all hash-based edits
    for op in &ops {
        match op {
            Op::SetLine(anchor, _) => {
                if let Some((line, hash)) = parse_anchor(anchor) {
                    if line == 0 || line > lines.len() {
                        return Err(format!("Line {} does not exist", line));
                    }
                    let expected = compute_line_hash(&lines[line - 1]);
                    if hash != expected {
                        return Err(format!(
                            "Hash mismatch at line {}: expected {}, got {}\n\nThe file content has changed since it was read. Please re-read the file using hashread and try again with updated anchors.",
                            line, expected, hash
                        ));
                    }
                }
            }
            Op::ReplaceLines(start_anchor, end_anchor, _) => {
                if let (Some((start, start_hash)), Some((end, end_hash))) =
                    (parse_anchor(start_anchor), parse_anchor(end_anchor))
                {
                    if start == 0
                        || end == 0
                        || start > lines.len()
                        || end > lines.len()
                        || start > end
                    {
                        return Err("Line number out of range".to_string());
                    }
                    let expected_start = compute_line_hash(&lines[start - 1]);
                    let expected_end = compute_line_hash(&lines[end - 1]);
                    if start_hash != expected_start || end_hash != expected_end {
                        return Err(format!(
                            "Hash mismatch in range {}-{}\n\nThe file content has changed since it was read. Please re-read the file using hashread and try again with updated anchors.",
                            start, end
                        ));
                    }
                }
            }
            Op::InsertAfter(anchor, _) => {
                if let Some((line, hash)) = parse_anchor(anchor) {
                    if line == 0 || line > lines.len() {
                        return Err(format!("Line {} does not exist", line));
                    }
                    let expected = compute_line_hash(&lines[line - 1]);
                    if hash != expected {
                        return Err(format!(
                            "Hash mismatch at line {}\n\nThe file content has changed since it was read. Please re-read the file using hashread and try again with updated anchors.",
                            line
                        ));
                    }
                }
            }
            Op::Replace(_, _, _) => {}
        }
    }

    // Separate and sort anchor ops
    let mut anchor_ops: Vec<Op> = Vec::new();
    let mut replace_ops: Vec<Op> = Vec::new();
    for op in ops {
        match op {
            Op::Replace(_, _, _) => replace_ops.push(op),
            _ => anchor_ops.push(op),
        }
    }

    anchor_ops.sort_by(|a, b| {
        let al = match a {
            Op::SetLine(anchor, _) => parse_anchor(anchor).map(|(l, _)| l).unwrap_or(0),
            Op::ReplaceLines(start, _, _) => parse_anchor(start).map(|(l, _)| l).unwrap_or(0),
            Op::InsertAfter(anchor, _) => parse_anchor(anchor).map(|(l, _)| l).unwrap_or(0),
            Op::Replace(_, _, _) => 0,
        };
        let bl = match b {
            Op::SetLine(anchor, _) => parse_anchor(anchor).map(|(l, _)| l).unwrap_or(0),
            Op::ReplaceLines(start, _, _) => parse_anchor(start).map(|(l, _)| l).unwrap_or(0),
            Op::InsertAfter(anchor, _) => parse_anchor(anchor).map(|(l, _)| l).unwrap_or(0),
            Op::Replace(_, _, _) => 0,
        };
        bl.cmp(&al)
    });

    // Apply anchor ops
    for op in anchor_ops {
        match op {
            Op::SetLine(anchor, new_text) => {
                if let Some((line, _)) = parse_anchor(&anchor) {
                    let idx = line - 1;
                    let new_lines: Vec<String> = if new_text.is_empty() {
                        vec![]
                    } else {
                        new_text.lines().map(|s| s.to_string()).collect()
                    };
                    lines.splice(idx..=idx, new_lines);
                }
            }
            Op::ReplaceLines(start_anchor, end_anchor, new_text) => {
                if let (Some((start, _)), Some((end, _))) =
                    (parse_anchor(&start_anchor), parse_anchor(&end_anchor))
                {
                    let new_lines: Vec<String> = if new_text.is_empty() {
                        vec![]
                    } else {
                        new_text.lines().map(|s| s.to_string()).collect()
                    };
                    lines.splice(start - 1..end, new_lines);
                }
            }
            Op::InsertAfter(anchor, text) => {
                if let Some((line, _)) = parse_anchor(&anchor) {
                    let idx = line;
                    let new_lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
                    lines.splice(idx..idx, new_lines);
                }
            }
            _ => {}
        }
    }

    // Apply replace ops with fuzzy matching
    for op in replace_ops {
        if let Op::Replace(old_text, new_text, all) = op {
            if all {
                lines = lines
                    .iter()
                    .map(|l| l.replace(&old_text, &new_text))
                    .collect();
            } else {
                let content_str = lines.join("\n");
                match find_fuzzy_match(&content_str, &old_text) {
                    Ok((pos, matched_text)) => {
                        let new_content = format!(
                            "{}{}{}",
                            &content_str[..pos],
                            new_text,
                            &content_str[pos + matched_text.len()..]
                        );
                        lines = new_content.lines().map(|s| s.to_string()).collect();
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    Ok(lines.join("\n"))
}

#[derive(Parser)]
#[command(name = "hashline-tools")]
#[command(about = "Hashline tools for opencode")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Read {
        file_path: String,
        #[arg(long)]
        offset: Option<usize>,
        #[arg(long)]
        limit: Option<usize>,
    },
    Edit {
        file_path: String,
        #[arg(long)]
        edits: String,
    },
}

fn cmd_read(
    file_path: &str,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<String, String> {
    let content =
        fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;
    let lines: Vec<&str> = content.lines().collect();
    let start = offset.unwrap_or(0);
    let count = limit.unwrap_or(2000);
    let total_lines = lines.len();
    let end = (start + count).min(total_lines);

    if start >= total_lines {
        return Ok("<file>\n(End of file - 0 lines)\n</file>".to_string());
    }

    let output: String = lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let line_num = start + i + 1;
            let hash = compute_line_hash(line);
            format!("{}:{}|{}", line_num, hash, line)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let end_msg = if end < total_lines {
        format!(
            "\n\n(File has more lines. Use 'offset' parameter to read beyond line {})",
            end
        )
    } else {
        format!("\n\n(End of file - {} total lines)", total_lines)
    };

    Ok(format!("<file>\n{}{}\n</file>", output, end_msg))
}

fn cmd_edit(file_path: &str, edits_json: &str) -> Result<String, String> {
    let content =
        fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;
    let new_content = apply_edits(&content, edits_json)?;

    if new_content == content {
        return Ok("No changes made".to_string());
    }

    let diff = similar::TextDiff::from_lines(&content, &new_content)
        .iter_all_changes()
        .map(|change| {
            let sign = match change.tag() {
                similar::ChangeTag::Delete => "-",
                similar::ChangeTag::Insert => "+",
                similar::ChangeTag::Equal => " ",
            };
            format!("{}{}", sign, change)
        })
        .collect::<Vec<_>>()
        .join("");

    fs::write(file_path, &new_content).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(format!(
        "Edit applied successfully.\n\n<diff>\n--- {}\n+++ {}\n{}\n</diff>",
        file_path, file_path, diff
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Read {
            file_path,
            offset,
            limit,
        } => {
            let result = cmd_read(&file_path, offset, limit)?;
            println!("{}", result);
        }
        Commands::Edit { file_path, edits } => {
            let result = cmd_edit(&file_path, &edits)?;
            println!("{}", result);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_compute_line_hash_determinism() {
        let hash1 = compute_line_hash("hello world");
        let hash2 = compute_line_hash("hello world");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_compute_line_hash_different_content() {
        let hash1 = compute_line_hash("hello world");
        let hash2 = compute_line_hash("hello there");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_line_hash_whitespace_normalization() {
        let hash1 = compute_line_hash("hello   world");
        let hash2 = compute_line_hash("hello world");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_parse_anchor_valid() {
        let result = parse_anchor("42:ab12").unwrap();
        assert_eq!(result.0, 42);
        assert_eq!(result.1, "ab12");
    }

    #[test]
    fn test_parse_anchor_invalid() {
        assert!(parse_anchor("42").is_none());
        assert!(parse_anchor("ab:cd").is_none());
        assert!(parse_anchor(":ab12").is_none());
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("a", ""), 1);
        assert_eq!(levenshtein("", "a"), 1);
        assert_eq!(levenshtein("hello", "hello"), 0);
    }

    #[test]
    fn test_similarity() {
        assert!((similarity("hello", "hello") - 1.0).abs() < 0.001);
        assert!((similarity("", "") - 1.0).abs() < 0.001);
        assert!(similarity("hello", "world") < 0.5);
    }

    #[test]
    fn test_fuzzy_match_exact() {
        let content = "line 1\nline 2\nline 3";
        let result = find_fuzzy_match(content, "line 2").unwrap();
        assert_eq!(result.0, 7);
        assert_eq!(result.1, "line 2");
    }

    #[test]
    fn test_fuzzy_match_normalized() {
        let content = "line  1\nline   2\nline 3";
        let result = find_fuzzy_match(content, "line 2").unwrap();
        assert!(result.0 > 0);
    }

    #[test]
    fn test_cmd_read() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "line 1").unwrap();
        writeln!(temp_file, "line 2").unwrap();
        writeln!(temp_file, "line 3").unwrap();

        let result = cmd_read(temp_file.path().to_str().unwrap(), None, None).unwrap();
        assert!(result.contains("1:"));
        assert!(result.contains("|line 1"));
        assert!(result.contains("(End of file"));
    }

    #[test]
    fn test_cmd_edit_replace_lines() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "line 1").unwrap();
        writeln!(temp_file, "line 2").unwrap();
        writeln!(temp_file, "line 3").unwrap();

        let content = fs::read_to_string(temp_file.path()).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        let hash2 = compute_line_hash(lines[1]);

        let edits = format!(
            r#"[{{"type":"replace_lines","start_anchor":"2:{}","end_anchor":"2:{}","new_text":"replaced line"}}]"#,
            hash2, hash2
        );
        let result = cmd_edit(temp_file.path().to_str().unwrap(), &edits).unwrap();

        assert!(result.contains("Edit applied successfully"));

        let new_content = fs::read_to_string(temp_file.path()).unwrap();
        assert!(new_content.contains("replaced line"));
    }

    #[test]
    fn test_cmd_edit_hash_mismatch() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "line 1").unwrap();
        writeln!(temp_file, "line 2").unwrap();

        let edits = r#"[{"type":"set_line","anchor":"2:zzzz","new_text":"test"}]"#;
        let result = cmd_edit(temp_file.path().to_str().unwrap(), edits);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Hash mismatch"));
    }

    #[test]
    fn test_cmd_edit_replace_fuzzy() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "line 1").unwrap();
        writeln!(temp_file, "line 2").unwrap();
        writeln!(temp_file, "line 3").unwrap();

        let edits = r#"[{"type":"replace","old_text":"line 2","new_text":"modified","all":false}]"#;
        let result = cmd_edit(temp_file.path().to_str().unwrap(), edits).unwrap();

        assert!(result.contains("Edit applied successfully"));

        let new_content = fs::read_to_string(temp_file.path()).unwrap();
        assert!(new_content.contains("modified"));
    }

    #[test]
    fn test_cmd_edit_replace_with_whitespace_diff() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "line 1").unwrap();
        writeln!(temp_file, "line  2").unwrap();
        writeln!(temp_file, "line 3").unwrap();

        let edits = r#"[{"type":"replace","old_text":"line 2","new_text":"modified","all":false}]"#;
        let result = cmd_edit(temp_file.path().to_str().unwrap(), edits).unwrap();

        assert!(result.contains("Edit applied successfully"));

        let new_content = fs::read_to_string(temp_file.path()).unwrap();
        assert!(new_content.contains("modified"));
    }

    #[test]
    fn test_cmd_edit_replace_all() {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "line 1").unwrap();
        writeln!(temp_file, "line 2").unwrap();
        writeln!(temp_file, "line 3").unwrap();

        let edits = r#"[{"type":"replace","old_text":"line","new_text":"row","all":true}]"#;
        let _result = cmd_edit(temp_file.path().to_str().unwrap(), edits).unwrap();

        let new_content = fs::read_to_string(temp_file.path()).unwrap();
        assert!(new_content.contains("row 1"));
        assert!(new_content.contains("row 2"));
        assert!(new_content.contains("row 3"));
    }
}
