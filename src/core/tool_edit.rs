//! Port of the portable part of `opencode/packages/core/src/tool/edit.ts`.
//!
//! The exact-edit tool. What is ported is the string handling and the model
//! output, not the file mutation or the permissions.
//!
//! The string handling is where the behaviour lives, and every piece of it is
//! pinned by a test below:
//!
//! - line endings are normalised for matching, but the file's original ending
//!   is detected first and restored on write, so an edit does not silently
//!   rewrite CRLF files as LF;
//! - a UTF-8 BOM is split off before matching and put back after, so it
//!   survives an edit;
//! - an EMPTY `oldString` is refused by the tool (`content.length + 1`
//! occurrences is the counter's way of saying "matches everywhere", never a
//!   real count), and identical old/new strings are refused too;
//! - the diff preview shows at most six lines per side, truncates any line
//!   over 240 characters, and appends a `+...` / `-...` marker when lines were
//!   dropped.

use serde::{Deserialize, Serialize};

use crate::schema::file_diff::Info as FileDiffInfo;

/// Tool name.
pub const NAME: &str = "edit";

/// Lines shown per side in the preview.
pub const PREVIEW_LINES: usize = 6;
/// Characters shown per line before truncation.
pub const PREVIEW_LINE_CHARS: usize = 240;

/// `EditTool.Input`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Input {
    pub path: String,
    #[serde(rename = "oldString")]
    pub old_string: String,
    #[serde(rename = "newString")]
    pub new_string: String,
    #[serde(rename = "replaceAll", skip_serializing_if = "Option::is_none")]
    pub replace_all: Option<bool>,
}

/// `EditTool.Output`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub files: Vec<FileDiffInfo>,
    pub replacements: f64,
}

/// The two refusals the tool returns before touching the file, verbatim.
pub const ERR_IDENTICAL: &str = "No changes to apply: oldString and newString are identical.";
pub const ERR_EMPTY_OLD: &str =
    "oldString must not be empty. Use write to create or overwrite a file.";

/// The pre-flight checks, in the order the TS performs them.
pub fn preflight(input: &Input) -> Result<(), &'static str> {
    if input.old_string == input.new_string {
        return Err(ERR_IDENTICAL);
    }
    if input.old_string.is_empty() {
        return Err(ERR_EMPTY_OLD);
    }
    Ok(())
}

/// `normalizeLineEndings`.
pub fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// `detectLineEnding`: CRLF if the text contains one anywhere, else LF.
pub fn detect_line_ending(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// `convertToLineEnding`: normalise, then restore the target ending.
pub fn convert_to_line_ending(text: &str, ending: &str) -> String {
    let normalized = normalize_line_endings(text);
    if ending == "\n" {
        normalized
    } else {
        normalized.replace('\n', "\r\n")
    }
}

/// `splitBom`.
pub fn split_bom(text: &str) -> (bool, &str) {
    match text.strip_prefix('\u{feff}') {
        Some(rest) => (true, rest),
        None => (false, text),
    }
}

/// `joinBom`.
pub fn join_bom(text: &str, bom: bool) -> String {
    if bom {
        format!("\u{feff}{}", text)
    } else {
        text.to_string()
    }
}

/// The UTF-8 BOM as bytes, for the decode side.
pub const UTF8_BOM: [u8; 3] = [0xef, 0xbb, 0xbf];

/// `countOccurrences`, including the empty-search case the TS defines.
pub fn count_occurrences(content: &str, search: &str) -> usize {
    if search.is_empty() {
        return content.chars().count() + 1;
    }
    let mut count = 0usize;
    let mut offset = 0usize;
    while let Some(found) = content[offset..].find(search) {
        count += 1;
        offset += found + search.len();
    }
    count
}

/// `previewLines`: at most `PREVIEW_LINES` lines, each cut at 240 chars.
pub fn preview_lines(value: &str, prefix: char) -> Vec<String> {
    let normalized = normalize_line_endings(value);
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut shown: Vec<String> = lines
        .iter()
        .take(PREVIEW_LINES)
        .map(|line| {
            if line.chars().count() > PREVIEW_LINE_CHARS {
                let cut: String = line.chars().take(PREVIEW_LINE_CHARS).collect();
                format!("{}{}...", prefix, cut)
            } else {
                format!("{}{}", prefix, line)
            }
        })
        .collect();
    if lines.len() > shown.len() {
        shown.push(format!("{}...", prefix));
    }
    shown
}

/// `toModelOutput`: a diff block between the removed and added lines.
pub fn to_model_output(output: &Output, old_string: &str, new_string: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    parts.push(format!(
        "Edited file successfully: {}",
        output.files.first().and_then(|f| f.file.as_deref()).unwrap_or("undefined")
    ));
    parts.push(format!("Replacements: {}", output.replacements));
    parts.push("```diff".to_string());
    parts.extend(preview_lines(old_string, '-'));
    parts.extend(preview_lines(new_string, '+'));
    parts.push("```".to_string());
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(old: &str, new: &str) -> Input {
        Input {
            path: "a.ts".into(),
            old_string: old.into(),
            new_string: new.into(),
            replace_all: None,
        }
    }

    #[test]
    fn identical_strings_are_refused_before_the_empty_check() {
        assert_eq!(preflight(&input("x", "x")), Err(ERR_IDENTICAL));
    }

    #[test]
    fn an_empty_old_string_is_refused() {
        assert_eq!(preflight(&input("", "y")), Err(ERR_EMPTY_OLD));
    }

    #[test]
    fn a_real_edit_passes_the_preflight() {
        assert_eq!(preflight(&input("a", "b")), Ok(()));
    }

    #[test]
    fn crlf_is_detected_and_restored() {
        assert_eq!(detect_line_ending("a\r\nb"), "\r\n");
        assert_eq!(detect_line_ending("a\nb"), "\n");
        assert_eq!(normalize_line_endings("a\r\nb\rc"), "a\nb\rc");
        assert_eq!(convert_to_line_ending("a\nb", "\r\n"), "a\r\nb");
        assert_eq!(convert_to_line_ending("a\r\nb", "\n"), "a\nb");
    }

    #[test]
    fn a_bom_survives_a_round_trip() {
        let (bom, text) = split_bom("\u{feff}hello");
        assert!(bom);
        assert_eq!(text, "hello");
        assert_eq!(join_bom(text, bom), "\u{feff}hello");
        let (bom, text) = split_bom("hello");
        assert!(!bom);
        assert_eq!(join_bom(text, bom), "hello");
    }

    #[test]
    fn occurrences_are_counted_without_overlapping() {
        assert_eq!(count_occurrences("aaaa", "aa"), 2, "the scan resumes after the match");
        assert_eq!(count_occurrences("abc", "z"), 0);
        assert_eq!(count_occurrences("aaa", "a"), 3);
    }

    #[test]
    fn an_empty_search_counts_length_plus_one() {
        // The TS returns content.length + 1 here; it is a guard against
        // matching everywhere, never a real count.
        assert_eq!(count_occurrences("abc", ""), 4);
        assert_eq!(count_occurrences("", ""), 1);
    }

    #[test]
    fn the_preview_stops_at_six_lines_and_marks_the_cut() {
        let long = "1\n2\n3\n4\n5\n6\n7\n8";
        let shown = preview_lines(long, '-');
        assert_eq!(shown.len(), 7);
        assert_eq!(shown[0], "-1");
        assert_eq!(shown[6], "-...");
    }

    #[test]
    fn a_short_value_is_not_marked() {
        assert_eq!(preview_lines("a\nb", '+'), vec!["+a".to_string(), "+b".to_string()]);
    }

    #[test]
    fn a_long_line_is_cut_at_240_characters_with_an_ellipsis() {
        let long = "x".repeat(300);
        let shown = preview_lines(&long, '+');
        let body = shown[0].strip_prefix('+').unwrap();
        assert!(body.ends_with("..."));
        assert_eq!(body.chars().count(), 241);
    }

    #[test]
    fn the_model_output_has_the_diff_fence_and_both_sides() {
        let out = Output {
            files: vec![FileDiffInfo {
                file: Some("src/a.ts".into()),
                patch: None,
                additions: 1.0,
                deletions: 1.0,
                status: None,
            }],
            replacements: 1.0,
        };
        let text = to_model_output(&out, "old", "new");
        assert_eq!(
            text,
            "Edited file successfully: src/a.ts\nReplacements: 1\n```diff\n-old\n+new\n```"
        );
    }

    #[test]
    fn a_missing_file_name_becomes_undefined_as_in_the_ts() {
        let out = Output { files: vec![], replacements: 0.0 };
        assert!(to_model_output(&out, "", "x").starts_with("Edited file successfully: undefined"));
    }

    #[test]
    fn the_input_keeps_its_camel_case_names() {
        let v = serde_json::to_value(Input {
            path: "a".into(),
            old_string: "x".into(),
            new_string: "y".into(),
            replace_all: Some(true),
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "path": "a", "oldString": "x", "newString": "y", "replaceAll": true })
        );
        let v = serde_json::to_value(input("x", "y")).unwrap();
        assert!(v.get("replaceAll").is_none());
    }
}