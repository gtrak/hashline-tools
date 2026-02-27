use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use xxhash_rust::xxh32::xxh32;

// ═══════════════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════════════

const NIBBLE_STR: &str = "ZPMQVRWSNKTXJBYH";
const HASH_SEED: u32 = 0;

// ═══════════════════════════════════════════════════════════════════════════
// Hash Computation
// ═══════════════════════════════════════════════════════════════════════════

/// Compute a short 2-character hash of a single line using xxHash32.
/// Uses whitespace-normalized line. Creates a hash chain where each line's hash
/// depends on the previous line's hash, ensuring that any change invalidates
/// all subsequent line hashes.
pub fn compute_line_hash(line_num: usize, line: &str, prev_hash: Option<&str>) -> String {
    // Remove trailing carriage return
    let line = if line.ends_with('\r') {
        &line[..line.len() - 1]
    } else {
        line
    };
    
    // Normalize: remove all whitespace
    let normalized: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    
    // Check if line has significant characters (alphanumeric)
    let has_significant = normalized.chars().any(|c| c.is_alphanumeric());
    
    // Build seed from previous hash (if any) or use defaults
    let seed = if let Some(prev) = prev_hash {
        // Convert previous 2-char hash to u32 seed
        let mut seed_val = 0u32;
        for c in prev.chars() {
            seed_val = seed_val.wrapping_mul(256).wrapping_add(c as u32);
        }
        seed_val
    } else if has_significant {
        HASH_SEED
    } else {
        line_num as u32
    };
    
    // Compute xxHash32 and take lower 8 bits
    let hash = xxh32(normalized.as_bytes(), seed) & 0xff;
    
    // Convert to 2-char hash using NIBBLE_STR
    let high = (hash >> 4) as usize;
    let low = (hash & 0x0f) as usize;
    
    format!(
        "{}{}",
        NIBBLE_STR.chars().nth(high).unwrap(),
        NIBBLE_STR.chars().nth(low).unwrap()
    )
}


// ═══════════════════════════════════════════════════════════════════════════
// Anchor Parsing
// ═══════════════════════════════════════════════════════════════════════════

/// Parse a line reference like "5#ab" into structured form.
/// Also accepts "5:abc" (old format) for backward compatibility.
pub fn parse_anchor(anchor: &str) -> Option<(usize, String)> {
    // Try new format: "LINE#HASH" (e.g., "5#ab")
    let parts: Vec<&str> = anchor.splitn(2, '#').collect();
    if parts.len() == 2 {
        let line_num = parts[0].parse::<usize>().ok()?;
        let hash = parts[1].to_string();
        return Some((line_num, hash));
    }
    
    // Try old format: "LINE:HASH" (e.g., "5:abc1")
    let parts: Vec<&str> = anchor.splitn(2, ':').collect();
    if parts.len() == 2 {
        let line_num = parts[0].parse::<usize>().ok()?;
        let hash = parts[1].to_string();
        return Some((line_num, hash));
    }
    
    None
}

// ═══════════════════════════════════════════════════════════════════════════
// Hashline Edit Types
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AnchorRef {
    pub line: usize,
    pub hash: String,
}

impl<'de> Deserialize<'de> for AnchorRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        
        // Parse format: "LINE#HASH" (e.g., "8#RT")
        let parts: Vec<&str> = s.splitn(2, '#').collect();
        if parts.len() != 2 {
            return Err(serde::de::Error::custom(
                format!("Invalid anchor format '{}', expected format 'LINE#HASH' (e.g., '8#RT')", s)
            ));
        }
        
        let line = parts[0].parse::<usize>()
            .map_err(|_| serde::de::Error::custom(
                format!("Invalid line number '{}' in anchor '{}', expected format 'LINE#HASH' (e.g., '8#RT')", parts[0], s)
            ))?;
        
        let hash = parts[1].to_string();
        
        Ok(AnchorRef { line, hash })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "op")]
pub enum HashlineEdit {
    #[serde(rename = "replace")]
    Replace {
        pos: AnchorRef,
        #[serde(skip_serializing_if = "Option::is_none")]
        end: Option<AnchorRef>,
        lines: Vec<String>,
    },
    #[serde(rename = "append")]
    Append {
        #[serde(skip_serializing_if = "Option::is_none")]
        pos: Option<AnchorRef>,
        lines: Vec<String>,
    },
    #[serde(rename = "prepend")]
    Prepend {
        #[serde(skip_serializing_if = "Option::is_none")]
        pos: Option<AnchorRef>,
        lines: Vec<String>,
    },
}

/// A hash mismatch found during validation
#[derive(Debug)]
pub struct HashMismatch {
    pub line: usize,
    pub expected: String,
    pub actual: String,
}

/// Error thrown when hashline references have stale hashes
#[derive(Debug)]
pub struct HashlineMismatchError {
    pub mismatches: Vec<HashMismatch>,
    pub file_lines: Vec<String>,
}

impl std::fmt::Display for HashlineMismatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mismatch_set: std::collections::HashSet<usize> = 
            self.mismatches.iter().map(|m| m.line).collect();
        
        writeln!(f, "{} line{} have changed since last read. Use the updated LINE#ID references shown below (>>> marks changed lines).",
            self.mismatches.len(),
            if self.mismatches.len() > 1 { "s" } else { "" }
        )?;
        writeln!(f)?;
        
        // Collect lines to display (mismatch lines + 2 context)
        let mut display_lines: Vec<usize> = Vec::new();
        for m in &self.mismatches {
            let lo = m.line.saturating_sub(2).max(1);
            let hi = (m.line + 2).min(self.file_lines.len());
            for i in lo..=hi {
                if !display_lines.contains(&i) {
                    display_lines.push(i);
                }
            }
        }
        display_lines.sort();
        
        let mut prev_line = 0usize;
        
        // Pre-compute all cumulative hashes for the file
        let mut prev_hash: Option<&str> = None;
        let mut cumulative_hashes: Vec<String> = Vec::new();
        for (i, line) in self.file_lines.iter().enumerate() {
            let line_num = i + 1;
            let hash_str = compute_line_hash(line_num, line, prev_hash);
            cumulative_hashes.push(hash_str.clone());
            prev_hash = Some(&cumulative_hashes[i]);
        }
        
        for line_num in display_lines {
            if prev_line != 0 && line_num > prev_line + 1 {
                writeln!(f, "    ...")?;
            }
            prev_line = line_num;
            
            let text = &self.file_lines[line_num - 1];
            let hash = &cumulative_hashes[line_num - 1];
            
            if mismatch_set.contains(&line_num) {
                writeln!(f, ">>> {}#{}:{}", line_num, hash, text)?;
            } else {
                writeln!(f, "    {}#{}:{}", line_num, hash, text)?;
            }
        }
        
        Ok(())
    }
}

impl std::error::Error for HashlineMismatchError {}

// ═══════════════════════════════════════════════════════════════════════════
// Hashline Edit Application
// ═══════════════════════════════════════════════════════════════════════════

/// Apply an array of hashline edits to file content.
/// Edits are sorted bottom-up and validated before application.
pub fn apply_hashline_edits(
    content: &str,
    edits: &[HashlineEdit],
) -> Result<(String, Option<usize>), Box<dyn std::error::Error>> {
    if edits.is_empty() {
        return Ok((content.to_string(), None));
    }
    
    // Track if original content ends with newline
    let ends_with_newline = content.ends_with('\n');

    let mut file_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let _original_file_lines = file_lines.clone();
    let mut first_changed_line: Option<usize> = None;
    
    // Pre-validate: collect all hash mismatches and check for invalid ranges
    let mut mismatches: Vec<HashMismatch> = Vec::new();
    let mut validation_errors: Vec<String> = Vec::new();
    
    for edit in edits {
        match edit {
            HashlineEdit::Replace { pos, end, .. } => {
                // Check if start line > end line
                if let Some(end_ref) = end {
                    if pos.line > end_ref.line {
                        validation_errors.push(format!(
                            "Range start line {} must be <= end line {}",
                            pos.line, end_ref.line
                        ));
                    }
                }
                validate_anchor_ref(pos, &file_lines, &mut mismatches, &mut validation_errors);
                if let Some(end_ref) = end {
                    validate_anchor_ref(end_ref, &file_lines, &mut mismatches, &mut validation_errors);
                }
            }
            HashlineEdit::Append { pos, .. } => {
                if let Some(ref_pos) = pos {
                    validate_anchor_ref(ref_pos, &file_lines, &mut mismatches, &mut validation_errors);
                }
            }
            HashlineEdit::Prepend { pos, .. } => {
                if let Some(ref_pos) = pos {
                    validate_anchor_ref(ref_pos, &file_lines, &mut mismatches, &mut validation_errors);
                }
            }
        }
    }
    
    if !validation_errors.is_empty() {
        return Err(validation_errors.join("\n").into());
    }
    
    if !mismatches.is_empty() {
        return Err(Box::new(HashlineMismatchError {
            mismatches,
            file_lines,
        }));
    }
    
    // Deduplicate edits targeting same location with same content
    let edits = deduplicate_edits(edits, &file_lines);
    
    // Check for overlapping edits
    let mut overlapping: Vec<String> = Vec::new();
    let file_len = file_lines.len();
    
    // Helper: get the line range affected by an edit
    fn get_edit_range(edit: &HashlineEdit, file_len: usize) -> Option<(usize, usize)> {
        match edit {
            HashlineEdit::Replace { pos, end, .. } => {
                let end_line = end.as_ref().map(|e| e.line).unwrap_or(pos.line);
                Some((pos.line, end_line))
            }
            HashlineEdit::Append { pos, lines } => {
                if lines.is_empty() { return None; }
                let ref_line = pos.as_ref().map(|p| p.line).unwrap_or(file_len);
                // Append inserts after ref_line, so range is [ref_line+1, ref_line+lines.len()]
                Some((ref_line + 1, ref_line + lines.len()))
            }
            HashlineEdit::Prepend { pos, lines } => {
                if lines.is_empty() { return None; }
                let ref_line = pos.as_ref().map(|p| p.line).unwrap_or(1);
                // Prepend inserts before ref_line, so range is [ref_line, ref_line+lines.len()-1]
                Some((ref_line, ref_line + lines.len() - 1))
            }
        }
    }
    
    // Check if any two edits have overlapping ranges
    for i in 0..edits.len() {
        let range_i = match get_edit_range(&edits[i], file_len) {
            Some(r) => r,
            None => continue,
        };
        for j in (i + 1)..edits.len() {
            let range_j = match get_edit_range(&edits[j], file_len) {
                Some(r) => r,
                None => continue,
            };
            
            // Check if ranges overlap (intervals intersect)
            let intervals_overlap = !(range_i.1 < range_j.0 || range_j.1 < range_i.0);
            
            
            // Special case: Append and Prepend at same ref line are conceptually at the same position
            // even if their intervals don't overlap (prepend inserts before, append inserts after)
            let same_ref_line = match (&edits[i], &edits[j]) {
                (HashlineEdit::Append { pos: pos_a, .. }, HashlineEdit::Prepend { pos: pos_b, .. }) |
                (HashlineEdit::Prepend { pos: pos_a, .. }, HashlineEdit::Append { pos: pos_b, .. }) => {
                    let ref_a = pos_a.as_ref().map(|p| p.line).unwrap_or(file_len);
                    let ref_b = pos_b.as_ref().map(|p| p.line).unwrap_or(1);
                    ref_a == ref_b && pos_a.is_some() && pos_b.is_some()
                }
                _ => false,
            };
            
            if intervals_overlap || same_ref_line {
                let op_i = match &edits[i] {
                    HashlineEdit::Replace { .. } => "replace",
                    HashlineEdit::Append { .. } => "append",
                    HashlineEdit::Prepend { .. } => "prepend",
                };
                let op_j = match &edits[j] {
                    HashlineEdit::Replace { .. } => "replace",
                    HashlineEdit::Append { .. } => "append",
                    HashlineEdit::Prepend { .. } => "prepend",
                };
                overlapping.push(format!(
                    "  - {} at lines {}-{} overlaps with {} at lines {}-{}",
                    op_i, range_i.0, range_i.1, op_j, range_j.0, range_j.1
                ));
            }
        }
    }
    
    if !overlapping.is_empty() {
        return Err(format!(
            "Overlapping edits detected. Combine overlapping edits into a single operation:\n{}",
            overlapping.join("\n")
        ).into());
    }
    
    
    // Sort edits bottom-up (highest line first)
    let mut annotated: Vec<(usize, usize, HashlineEdit)> = edits.into_iter()
        .enumerate()
        .map(|(idx, edit)| {
            let (sort_line, _precedence) = match &edit {
                HashlineEdit::Replace { pos, end, .. } => {
                    let end_line = end.as_ref().map(|e| e.line).unwrap_or(pos.line);
                    (end_line, 0)
                }
                HashlineEdit::Append { pos, .. } => {
                    (pos.as_ref().map(|p| p.line).unwrap_or(file_lines.len()), 1)
                }
                HashlineEdit::Prepend { pos, .. } => {
                    (pos.as_ref().map(|p| p.line).unwrap_or(0), 2)
                }
            };
            (idx, sort_line, edit)
        })
        .collect();
    
    // Sort by line descending, then by precedence, then by original index
    annotated.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.0.cmp(&a.0))
    });
    
    // Apply edits
    for (_idx, _, edit) in annotated {
        match edit {
            HashlineEdit::Replace { pos, end, lines } => {
                if let Some(end_ref) = end {
                    // Replace range
                    let count = end_ref.line - pos.line + 1;
                    file_lines.splice(pos.line - 1..pos.line - 1 + count, lines.clone());
                } else {
                    // Replace single line
                    file_lines.splice(pos.line - 1..pos.line, lines.clone());
                }
                track_first_changed(&mut first_changed_line, pos.line);
            }
            HashlineEdit::Append { pos, lines } => {
                if lines.is_empty() {
                    continue;
                }
                if let Some(ref_pos) = pos {
                    // Insert after specified line
                    file_lines.splice(ref_pos.line..ref_pos.line, lines.clone());
                    track_first_changed(&mut first_changed_line, ref_pos.line + 1);
                } else {
                    // Append at end of file
                    if file_lines.len() == 1 && file_lines[0].is_empty() {
                        file_lines.clear();
                    }
                    let start_idx = file_lines.len();
                    file_lines.extend(lines.clone());
                    track_first_changed(&mut first_changed_line, start_idx + 1);
                }
            }
            HashlineEdit::Prepend { pos, lines } => {
                if lines.is_empty() {
                    continue;
                }
                if let Some(ref_pos) = pos {
                    // Insert before specified line
                    file_lines.splice(ref_pos.line - 1..ref_pos.line - 1, lines.clone());
                    track_first_changed(&mut first_changed_line, ref_pos.line);
                } else {
                    // Prepend at start of file
                    if file_lines.len() == 1 && file_lines[0].is_empty() {
                        file_lines.clear();
                    }
                    file_lines.splice(0..0, lines.clone());
                    track_first_changed(&mut first_changed_line, 1);
                }
            }
        }
    }
    
    let result = file_lines.join("\n");
    // Restore trailing newline if it existed in original
    if ends_with_newline && !result.is_empty() && !result.ends_with('\n') {
        return Ok((result + "\n", first_changed_line));
    }
    Ok((result, first_changed_line))
}

fn validate_anchor_ref(
    anchor: &AnchorRef,
    file_lines: &[String],
    mismatches: &mut Vec<HashMismatch>,
    validation_errors: &mut Vec<String>,
) {
    if anchor.line < 1 {
        validation_errors.push(format!("Line {} must be >= 1", anchor.line));
        return;
    }
    if anchor.line > file_lines.len() {
        validation_errors.push(format!(
            "Line {} does not exist (file has {} lines)",
            anchor.line, file_lines.len()
        ));
        return;
    }
    
    // Compute cumulative hashes up to the anchor line
    let mut prev_hash: Option<&str> = None;
    let mut cumulative_hashes: Vec<String> = Vec::new();
    for (i, line) in file_lines.iter().enumerate() {
        let line_num = i + 1;
        let hash_str = compute_line_hash(line_num, line, prev_hash);
        cumulative_hashes.push(hash_str.clone());
        prev_hash = Some(&cumulative_hashes[i]);
        if line_num == anchor.line {
            break;
        }
    }
    
    let actual_hash = &cumulative_hashes[anchor.line - 1];
    if *actual_hash != anchor.hash {
        mismatches.push(HashMismatch {
            line: anchor.line,
            expected: anchor.hash.clone(),
            actual: actual_hash.to_string(),
        });
    }
}

fn deduplicate_edits(edits: &[HashlineEdit], _file_lines: &[String]) -> Vec<HashlineEdit> {
    let mut seen = std::collections::HashMap::new();
    let mut result = Vec::new();
    
    for (i, edit) in edits.iter().enumerate() {
        let key = match edit {
            HashlineEdit::Replace { pos, end, lines } => {
                let line_key = match end {
                    Some(end_ref) => format!("r:{}:{}", pos.line, end_ref.line),
                    None => format!("s:{}", pos.line),
                };
                format!("{}:{}", line_key, lines.join("\n"))
            }
            HashlineEdit::Append { pos, lines } => {
                let line_key = pos.as_ref().map(|p| format!("i:{}", p.line))
                    .unwrap_or_else(|| "ieof".to_string());
                format!("{}:{}", line_key, lines.join("\n"))
            }
            HashlineEdit::Prepend { pos, lines } => {
                let line_key = pos.as_ref().map(|p| format!("ib:{}", p.line))
                    .unwrap_or_else(|| "ibef".to_string());
                format!("{}:{}", line_key, lines.join("\n"))
            }
        };
        
        if !seen.contains_key(&key) {
            seen.insert(key, i);
            result.push(edit.clone());
        }
    }
    
    result
}

fn track_first_changed(first: &mut Option<usize>, line: usize) {
    if first.is_none() || line < first.unwrap() {
        *first = Some(line);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Commands
// ═══════════════════════════════════════════════════════════════════════════

pub fn cmd_read(file_path: &str, offset: Option<usize>, limit: Option<usize>) -> Result<String, String> {
    let content = fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;
    let lines: Vec<&str> = content.lines().collect();
    let start = offset.unwrap_or(0);
    let count = limit.unwrap_or(2000);
    let total_lines = lines.len();
    let end = (start + count).min(total_lines);
    
    if start >= total_lines {
        return Ok("<file>\n(End of file - 0 lines)\n</file>".to_string());
    }
    let mut prev_hash: Option<&str> = None;
    let mut cumulative_hashes: Vec<String> = Vec::new();
    
    // Compute cumulative hashes from line 1 up to the end of the requested range
    for (i, line) in lines.iter().enumerate() {
        let line_num = i + 1;
        let hash = compute_line_hash(line_num, line, prev_hash);
        cumulative_hashes.push(hash.clone());
        prev_hash = Some(&cumulative_hashes[i]);
    }
    
    
    let output: String = lines[start..end]
        .iter().enumerate()
        .map(|(i, line)| { 
            let line_num = start + i + 1; 
            let hash = &cumulative_hashes[line_num - 1];
            format!("{}#{}:{}", line_num, hash, line) 
        })
        .collect::<Vec<_>>().join("\n");
    
    let end_msg = if end < total_lines {
        format!("\n\n(File has more lines. Use 'offset' parameter to read beyond line {})", end)
    } else {
        format!("\n\n(End of file - {} total lines)", total_lines)
    };
    
    Ok(format!("<file>\n{}{}\n</file>", output, end_msg))
}

pub fn cmd_edit(file_path: &str, edits_json: &str) -> Result<String, String> {
    let content = fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;
    
    let hashline_edits: Vec<HashlineEdit> = serde_json::from_str(edits_json)
        .map_err(|e| format!("Failed to parse edits: {}", e))?;
    
    apply_hashline_cmd(&content, file_path, &hashline_edits)
}

fn apply_hashline_cmd(content: &str, file_path: &str, edits: &[HashlineEdit]) -> Result<String, String> {
    match apply_hashline_edits(content, edits) {
        Ok((new_content, first_changed)) => {
            if new_content == content {
                return Ok("No changes made".to_string());
            }
            
            fs::write(file_path, &new_content).map_err(|e| format!("Failed to write file: {}", e))?;
            
            let first_changed_line = first_changed.unwrap_or(1);
            let first_line_msg = format!(" (first change at line {})", first_changed_line);
            
            // Generate hash-aware diff
            let diff_output = generate_hash_aware_diff(content, &new_content, first_changed_line);
            
            Ok(format!("Edit applied successfully{}.\n\n<diff>\n--- {}\n+++ {}\n{}\n</diff>",
                first_line_msg, file_path, file_path, diff_output))
        }
        Err(e) => {
            if let Some(mismatch_err) = e.downcast_ref::<HashlineMismatchError>() {
                Err(format!("Hash mismatch error:\n{}", mismatch_err))
            } else {
                Err(format!("Edit failed: {}", e))
            }
        }
    }
}

fn generate_hash_aware_diff(old_content: &str, new_content: &str, first_changed_line: usize) -> String {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();
    let total_new_lines = new_lines.len();
    
    // Compute cumulative hashes for all new lines
    let mut prev_hash: Option<&str> = None;
    let mut new_line_hashes: Vec<String> = Vec::new();
    for (i, line) in new_lines.iter().enumerate() {
        let line_num = i + 1;
        let hash_str = compute_line_hash(line_num, line, prev_hash);
        new_line_hashes.push(hash_str.clone());
        prev_hash = Some(&new_line_hashes[i]);
    }
    
    // Use similar to get changes
    let diff = similar::TextDiff::from_lines(old_content, new_content);
    
    // Collect all changed line numbers (in new file)
    let mut changed_new_lines: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut deleted_old_lines: std::collections::HashSet<usize> = std::collections::HashSet::new();
    
    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Insert => {
                if let Some(new_index) = change.new_index() {
                    changed_new_lines.insert(new_index + 1); // 1-indexed
                }
            }
            similar::ChangeTag::Delete => {
                if let Some(old_index) = change.old_index() {
                    deleted_old_lines.insert(old_index + 1); // 1-indexed
                }
            }
            similar::ChangeTag::Equal => {}
        }
    }
    
    // Calculate display range: ±5 lines around changes
    let mut display_ranges: Vec<(usize, usize)> = Vec::new();
    for &line in &changed_new_lines {
        let start = line.saturating_sub(5).max(1);
        let end = (line + 5).min(total_new_lines);
        display_ranges.push((start, end));
    }
    
    // Merge overlapping ranges
    display_ranges.sort_by_key(|r| r.0);
    let mut merged_ranges: Vec<(usize, usize)> = Vec::new();
    for (start, end) in display_ranges {
        if let Some(last) = merged_ranges.last_mut() {
            if start <= last.1 + 1 {
                last.1 = last.1.max(end);
            } else {
                merged_ranges.push((start, end));
            }
        } else {
            merged_ranges.push((start, end));
        }
    }
    
    // If no merged ranges, show context around first_changed_line
    if merged_ranges.is_empty() {
        let start = first_changed_line.saturating_sub(5).max(1);
        let end = (first_changed_line + 5).min(total_new_lines);
        merged_ranges.push((start, end));
    }
    
    // Build output
    let mut output_lines: Vec<String> = Vec::new();
    let mut prev_end: usize = 0;
    
    for (range_start, range_end) in merged_ranges {
        // Add ellipsis if there is a gap
        if prev_end > 0 && range_start > prev_end + 1 {
            output_lines.push("...".to_string());
        }
        
        for line_num in range_start..=range_end {
            let new_line_content = new_lines[line_num - 1];
            let new_hash = &new_line_hashes[line_num - 1];
            
            // Check if this line was deleted in old version
            let was_deleted = deleted_old_lines.contains(&line_num);
            
            // Check if this line was inserted (new)
            let was_inserted = changed_new_lines.contains(&line_num);
            
            if was_deleted {
                // Show old content as deleted
                let old_content = if line_num <= old_lines.len() {
                    old_lines[line_num - 1]
                } else {
                    ""
                };
                output_lines.push(format!("-{}#  :{}", line_num, old_content));
            }
            
            if was_inserted || !was_deleted {
                // Show new content with hash
                let sign = if was_inserted { "+" } else { " " };
                output_lines.push(format!("{}{}#{}:{}", sign, line_num, new_hash, new_line_content));
            }
        }
        
        prev_end = range_end;
    }
    
    // Add note about invalidated hashes
    output_lines.push("".to_string());
    output_lines.push("Note: Lines after edited regions have stale hashes. Use hashread to refresh.".to_string());
    
    output_lines.join("\n")
}


// ═══════════════════════════════════════════════════════════════════════════
// CLI
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Parser)]
#[command(name = "hashline-tools")]
#[command(about = "Hashline tools for opencode")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Read { 
        file_path: String, 
        #[arg(long)] offset: Option<usize>, 
        #[arg(long)] limit: Option<usize> 
    },
    Edit { 
        file_path: String, 
        #[arg(long)] edits: Option<String>, 
        #[arg(long)] edits_stdin: bool 
    },
}