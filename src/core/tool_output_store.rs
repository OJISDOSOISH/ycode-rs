//! Port of the portable part of `opencode/packages/core/src/tool-output-store.ts`.
//!
//! The disk side is not ported: writing the spill file, the identifier that
//! names it, the retention scan. What is ported is the part the model reads -
//! how an over-long tool output is turned into a head, a marker and a tail -
//! and that part is almost entirely arithmetic on sizes.
//!
//! Four decisions define it, and each is a place where a plausible port gives a
//! different answer:
//!
//! - The budget is measured in BYTES but applied per CHARACTER, adding the
//!   character's own UTF-8 length. A four-byte character is therefore never
//!   half-taken: the loop stops before it rather than truncating it, which is
//!   why an emoji cannot leave a broken sequence behind.
//!
//! - The sample keeps both ends. An odd line budget goes to the head:
//!   `Math.ceil(maxLines / 2)` up top, `Math.floor(maxLines / 2)` at the
//!   bottom. So a budget of 5 lines yields 3 and 2, never 2 and 3, and the
//!   single missing line comes off the tail.
//!
//! - The marker is charged against the budget BEFORE the content is sampled:
//!   `preview(text, maxLines - 4, maxBytes - markerBytes - 4)`. Four lines and
//!   the marker's bytes plus four are reserved, because the marker is inserted
//!   with blank lines around it. Two guards short-circuit the whole thing to
//!   the marker alone when the budget is too small to hold it (`maxLines <= 4`,
//!   or a byte budget that cannot cover the marker and four bytes), because a
//!   preview that exceeded its own budget would be worse than none.
//!
//! - Whether the byte check applies depends on how many LINES the text has.
//!   Under the line limit the whole text is returned with an empty tail; over
//!   it, the head and tail are computed separately even when the byte budget
//!   was never exceeded, so the same text can come back with a tail that is
//!   non-empty purely because it had many lines.
//!
//! One consequence worth spelling out, because it decides whether a tail exists
//! at all: the marker is charged BEFORE the sample, so the line budget the
//! content is sampled against is `maxLines - 4`. A text of exactly that many
//! lines therefore produces NO tail - it fits - and a test that expects one has
//! miscounted the marker rather than found a bug.

use serde::{Deserialize, Serialize};

/// `MAX_LINES`.
pub const MAX_LINES: usize = 2_000;
/// `MAX_BYTES`.
pub const MAX_BYTES: usize = 50 * 1024;
/// `RETENTION`, in days.
pub const RETENTION_DAYS: u64 = 7;
/// `RETENTION`, in milliseconds.
pub const RETENTION_MILLIS: u64 = RETENTION_DAYS * 24 * 60 * 60 * 1_000;
/// `MANAGED_DIRECTORY`.
pub const MANAGED_DIRECTORY: &str = "tool-output";
/// The filename prefix a spill file must carry to be a candidate for cleanup.
pub const FILE_PREFIX: &str = "tool_";

/// Lines reserved by the marker: its own line plus the blank line on each side.
pub const MARKER_LINES: usize = 4;
/// Bytes reserved alongside the marker, matching `MARKER_LINES`.
pub const MARKER_BYTES: usize = 4;

/// The resolved limits, as `limits()` returns them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limits {
    pub max_lines: usize,
    pub max_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_lines: MAX_LINES, max_bytes: MAX_BYTES }
    }
}

/// `StorageError.operation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Encode,
    Write,
}

/// `StorageError`, with its message built the way the getter builds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageError {
    pub operation: Operation,
    /// The cause, already rendered to text: the TS reads `Error.message` when
    /// the cause is an `Error` and falls back to `String(cause)` otherwise, so
    /// the stored form here is the rendered one either way.
    pub cause: String,
}

impl StorageError {
    /// The message getter: `Failed to <operation> tool output[: <detail>]`.
    ///
    /// An empty detail contributes nothing, not even a colon - the template
    /// puts the colon inside the conditional.
    pub fn message(&self) -> String {
        let operation = match self.operation {
            Operation::Encode => "encode",
            Operation::Write => "write",
        };
        if self.cause.is_empty() {
            format!("Failed to {} tool output", operation)
        } else {
            format!("Failed to {} tool output: {}", operation, self.cause)
        }
    }
}

/// `BoundInput`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundInput {
    pub session_id: String,
    pub tool_call_id: String,
    /// The tool output: its structured value plus its content parts.
    pub output: ToolOutput,
}

/// `BoundResult`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundResult {
    pub output: ToolOutput,
    #[serde(rename = "outputPaths")]
    pub output_paths: Vec<String>,
}

/// The slice of `ToolOutput` this module needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolOutput {
    /// The structured value, carried through untouched in every path.
    pub structured: Option<serde_json::Value>,
    pub content: Vec<ContentPart>,
}

/// `ToolOutput` content, keeping only the tag and the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    /// Only the tag matters here: media parts are passed through untouched, so
    /// no other field is modelled.
    #[serde(rename = "file")]
    File,
}

impl ToolOutput {
    /// The text parts concatenated, which is what the budget is applied to.
    pub fn contextual_text(&self) -> Option<String> {
        let texts: Vec<&str> = self
            .content
            .iter()
            .filter_map(|part| match part {
                ContentPart::Text { text } => Some(text.as_str()),
                ContentPart::File => None,
            })
            .collect();
        if texts.is_empty() {
            return None;
        }
        Some(texts.concat())
    }

    /// The non-text parts, which are carried through unchanged.
    pub fn media(&self) -> Vec<ContentPart> {
        self.content
            .iter()
            .filter(|part| matches!(part, ContentPart::File))
            .cloned()
            .collect()
    }
}

/// `lineCount`: one, plus every newline. An empty string counts as ONE line.
pub fn line_count(text: &str) -> usize {
    1 + text.matches('\n').count()
}

/// `takePrefix`: whole characters until the next one would cross the budget.
pub fn take_prefix(input: &str, maximum_bytes: usize) -> String {
    let mut bytes = 0usize;
    let mut content = String::new();
    for c in input.chars() {
        let size = c.len_utf8();
        if bytes + size > maximum_bytes {
            break;
        }
        content.push(c);
        bytes += size;
    }
    content
}

/// `takeSuffix`: whole characters from the end, in their original order.
///
/// Collected backwards and reversed once, rather than inserted at the front of
/// a growing vector: the TS unshifts onto an array too, but Rust's `insert(0)`
/// is quadratic where `unshift` is not, and the budget is bounded by the
/// caller rather than by the input.
pub fn take_suffix(input: &str, maximum_bytes: usize) -> String {
    let mut bytes = 0usize;
    let mut content: Vec<char> = Vec::new();
    for c in input.chars().rev() {
        let size = c.len_utf8();
        if bytes + size > maximum_bytes {
            break;
        }
        content.push(c);
        bytes += size;
    }
    content.reverse();
    content.into_iter().collect()
}

/// `preview`: the head and tail the model sees instead of the whole text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    pub head: String,
    pub tail: String,
}

impl Preview {
    /// Whether the preview carries a tail at all.
    pub fn has_tail(&self) -> bool {
        !self.tail.is_empty()
    }
}

/// `preview`: head first, tail second, with the odd line going to the head.
pub fn preview(text: &str, max_lines: usize, max_bytes: usize) -> Preview {
    let lines: Vec<&str> = text.split('\n').collect();
    // `Math.ceil(maxLines / 2)` and `Math.floor(maxLines / 2)`, written out
    // rather than calling `div_ceil`: this crate has no stated minimum Rust
    // version, and `div_ceil` on integers only stabilised in 1.73. The
    // arithmetic is the contract, so it is spelled out.
    let head_lines = (max_lines + 1) / 2;
    let tail_lines = max_lines / 2;
    let within_lines = lines.len() <= max_lines;

    let sampled = if within_lines {
        text.to_string()
    } else {
        let head = lines[..head_lines.min(lines.len())].join("\n");
        let mut parts = vec![head];
        if tail_lines > 0 {
            let start = lines.len().saturating_sub(tail_lines);
            parts.push(lines[start..].join("\n"));
        }
        parts.join("\n")
    };

    if sampled.len() <= max_bytes {
        if within_lines {
            return Preview { head: sampled, tail: String::new() };
        }
        return Preview {
            head: lines[..head_lines.min(lines.len())].join("\n"),
            tail: if tail_lines > 0 {
                lines[lines.len().saturating_sub(tail_lines)..].join("\n")
            } else {
                String::new()
            },
        };
    }

    let head_bytes = (max_bytes + 1) / 2;
    let tail_bytes = max_bytes / 2;
    Preview { head: take_prefix(&sampled, head_bytes), tail: take_suffix(&sampled, tail_bytes) }
}

/// `boundedPreview`: the marker, charged against the budget, then the sample.
pub fn bounded_preview(text: &str, marker: &str, max_lines: usize, max_bytes: usize) -> String {
    let marker_only: String = take_prefix(marker, max_bytes)
        .split('\n')
        .take(max_lines)
        .collect::<Vec<&str>>()
        .join("\n");
    let marker_bytes = marker.len();
    if max_lines <= MARKER_LINES || max_bytes <= marker_bytes + MARKER_BYTES {
        return marker_only;
    }
    let bounded = preview(text, max_lines - MARKER_LINES, max_bytes - marker_bytes - MARKER_BYTES);
    if bounded.has_tail() {
        format!("{}\n\n{}\n\n{}", bounded.head, marker, bounded.tail)
    } else {
        format!("{}\n\n{}", bounded.head, marker)
    }
}

/// The marker the spill path builds.
pub fn truncation_marker(output_path: &str) -> String {
    format!("... output truncated; full content saved to {} ...", output_path)
}

/// `lineCount` applied to the value that gets measured.
///
/// When the output has NO content at all, the contextual text is the structured
/// value pretty-printed with two-space indentation - and `JSON.stringify` of
/// `undefined` yields `undefined`, which the TS then turns into the string
/// `"undefined"` via its `?? String(...)`.
pub fn contextual_text(output: &ToolOutput) -> String {
    match output.contextual_text() {
        Some(text) => text,
        None => match &output.structured {
            Some(value) => serde_json::to_string_pretty(value).unwrap_or_else(|_| String::from("null")),
            None => "undefined".to_string(),
        },
    }
}

/// Whether a text fits inside the limits untouched.
pub fn fits(contextual: &str, limits: &Limits) -> bool {
    line_count(contextual) <= limits.max_lines && contextual.len() <= limits.max_bytes
}

/// The decision `bound` makes before touching the disk.
///
/// `None` means the output goes back unchanged, which is the common case and
/// must stay cheap. `Some` carries the replacement content and the path it was
/// written to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bound {
    /// Fits: the output is returned as it came in.
    Unchanged,
    /// Too large: the text the model sees, and where the whole thing went.
    Spilled { text: String, path: String },
}

/// `bound`, without the write. The caller supplies the path the spill file took.
pub fn bound_with_path(output: &ToolOutput, limits: &Limits, output_path: &str) -> Bound {
    let contextual = contextual_text(output);
    if fits(&contextual, limits) {
        return Bound::Unchanged;
    }
    let marker = truncation_marker(output_path);
    Bound::Spilled {
        text: bounded_preview(&contextual, &marker, limits.max_lines, limits.max_bytes),
        path: output_path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(lines: usize, bytes: usize) -> Limits {
        Limits { max_lines: lines, max_bytes: bytes }
    }

    fn output(parts: Vec<ContentPart>, structured: Option<serde_json::Value>) -> ToolOutput {
        ToolOutput { structured, content: parts }
    }

    fn text(s: &str) -> ContentPart {
        ContentPart::Text { text: s.to_string() }
    }

    // --- line_count ---

    #[test]
    fn an_empty_string_is_one_line() {
        assert_eq!(line_count(""), 1);
        assert_eq!(line_count("a"), 1);
        assert_eq!(line_count("a\n"), 2);
        assert_eq!(line_count("a\nb"), 2);
        assert_eq!(line_count("\n\n"), 3);
    }

    // --- takePrefix / takeSuffix ---

    #[test]
    fn ascii_costs_one_byte_per_character() {
        // The budget is in bytes, and an ASCII character is one byte, so ten
        // bytes hold ten characters.
        assert_eq!(take_prefix("aaaaaaaaaa", 10), "aaaaaaaaaa");
        assert_eq!(take_prefix("aaaaaaaaaa", 9), "aaaaaaaaa");
        assert_eq!(take_prefix("aaaaaaaaaa", 4), "aaaa");
    }

    #[test]
    fn a_prefix_of_wide_characters_holds_fewer_of_them() {
        // Ten bytes, four per character: two characters fit, the third would
        // cross the budget and stops the loop.
        let wide = "\u{1F600}".repeat(5);
        assert_eq!(take_prefix(&wide, 10), "\u{1F600}\u{1F600}");
        assert_eq!(take_prefix(&wide, 8), "\u{1F600}\u{1F600}");
        assert_eq!(take_prefix(&wide, 12), "\u{1F600}\u{1F600}\u{1F600}");
    }

    #[test]
    fn a_two_byte_character_costs_two() {
        // "é" is two bytes in UTF-8, so five bytes hold two of them.
        let accented = "ééééé";
        assert_eq!(take_prefix(accented, 5), "éé");
        assert_eq!(take_prefix(accented, 4), "éé");
        assert_eq!(take_prefix(accented, 6), "ééé");
    }

    #[test]
    fn a_zero_byte_budget_yields_nothing() {
        assert_eq!(take_prefix("abc", 0), "");
        assert_eq!(take_suffix("abc", 0), "");
    }

    #[test]
    fn a_suffix_keeps_the_original_order() {
        assert_eq!(take_suffix("abcdef", 6), "abcdef");
        assert_eq!(take_suffix("abcdef", 3), "def");
    }

    #[test]
    fn a_multi_byte_character_is_never_split() {
        // Each emoji is four bytes; a budget of five fits one and stops, rather
        // than emitting a broken sequence.
        let emojis = "\u{1F600}\u{1F600}\u{1F600}";
        assert_eq!(take_prefix(emojis, 5), "\u{1F600}");
        assert_eq!(take_prefix(emojis, 8), "\u{1F600}\u{1F600}");
        assert_eq!(take_suffix(emojis, 5), "\u{1F600}");
    }

    // --- preview ---

    #[test]
    fn a_short_text_is_returned_whole_with_no_tail() {
        let p = preview("a\nb\nc", 10, 1_000);
        assert_eq!(p.head, "a\nb\nc");
        assert_eq!(p.tail, "");
        assert!(!p.has_tail());
    }

    #[test]
    fn an_odd_line_budget_gives_the_extra_line_to_the_head() {
        // Five lines kept out of nine: three up top, two at the bottom.
        let text = "1\n2\n3\n4\n5\n6\n7\n8\n9";
        let p = preview(text, 5, 10_000);
        assert_eq!(p.head, "1\n2\n3");
        assert_eq!(p.tail, "8\n9");
    }

    #[test]
    fn an_even_line_budget_splits_evenly() {
        let text = "1\n2\n3\n4\n5\n6";
        let p = preview(text, 4, 10_000);
        assert_eq!(p.head, "1\n2");
        assert_eq!(p.tail, "5\n6");
    }

    #[test]
    fn many_lines_produce_a_tail_even_when_the_bytes_fit() {
        // The byte check passes, but the line count does not, so head and tail
        // are still computed apart.
        let text = "1\n2\n3\n4\n5";
        let p = preview(text, 4, 10_000);
        assert_eq!(p.head, "1\n2");
        assert_eq!(p.tail, "4\n5");
    }

    #[test]
    fn a_one_line_budget_keeps_the_head_only() {
        let text = "1\n2\n3";
        let p = preview(text, 1, 10_000);
        assert_eq!(p.head, "1");
        assert_eq!(p.tail, "", "floor(1/2) is zero, so there is no tail");
    }

    #[test]
    fn a_byte_budget_that_is_exceeded_cuts_both_ends_by_bytes() {
        let text = "aaaa\nbbbb\ncccc\ndddd";
        let p = preview(text, 100, 8);
        assert_eq!(p.head, "aaaa", "ceil(8/2) = 4 bytes at the head");
        assert_eq!(p.tail, "dddd", "floor(8/2) = 4 bytes at the tail");
    }

    #[test]
    fn the_head_gets_the_odd_byte_of_an_odd_budget() {
        let p = preview("abcdefghij", 100, 7);
        assert_eq!(p.head, "abcd", "ceil(7/2) = 4 bytes from the front");
        assert_eq!(p.tail, "defghij", "floor(7/2) = 3 bytes, taken from the END");
    }

    #[test]
    fn the_suffix_is_taken_from_the_end_not_the_front() {
        // The same budget read from the other end: the bytes are the LAST ones,
        // not the ones after the head. This is the pair that catches a port
        // which reuses the head rule for the tail.
        let p = preview("abcdefghij", 100, 7);
        assert_eq!(p.head, "abcd");
        assert_eq!(p.tail, "hij", "the last three characters");
        assert_eq!(p.tail, take_suffix("abcdefghij", 3));
    }

    // --- boundedPreview ---

    #[test]
    fn a_budget_too_small_for_the_marker_yields_the_marker_alone() {
        let marker = truncation_marker("/data/tool_abc");
        // maxLines <= 4 short-circuits.
        assert_eq!(bounded_preview("a\nb\nc\nd\ne", &marker, 4, 10_000), marker);
    }

    #[test]
    fn a_byte_budget_that_cannot_hold_the_marker_yields_the_marker_alone() {
        let marker = truncation_marker("/x");
        // markerBytes + 4 is the threshold; at or below it, marker only.
        let at_threshold = marker.len() + MARKER_BYTES;
        assert_eq!(bounded_preview("body", &marker, 100, at_threshold), marker);
        assert_eq!(bounded_preview("body", &marker, 100, at_threshold - 1), marker);
    }

    #[test]
    fn a_long_marker_is_itself_truncated_to_the_budget() {
        let marker = "m".repeat(50);
        let out = bounded_preview("body", &marker, 100, 10);
        assert_eq!(out.len(), 10);
    }

    #[test]
    fn the_marker_appears_between_the_head_and_the_tail() {
        let marker = truncation_marker("/data/tool_abc");
        // Twenty lines against a twelve-line budget: the marker takes four, so
        // only eight lines remain to sample, and 20 > 8 means there IS a tail.
        // Fewer lines than that would legitimately produce none.
        let text: Vec<String> = (1..=20).map(|n| n.to_string()).collect();
        let text = text.join("\n");
        let out = bounded_preview(&text, &marker, 12, 10_000);
        assert!(out.contains(&marker), "{}", out);
        let (head, rest) = out.split_once(&marker).unwrap();
        let tail = rest.trim_start_matches("\n\n");
        assert!(head.trim().contains('\n'), "several lines in the head");
        assert!(head.contains("1"), "the head starts at the beginning");
        assert!(tail.contains("20"), "the tail ends at the end");
        assert!(!tail.contains("1\n"), "the middle is gone");
    }

    #[test]
    fn a_body_that_fits_the_reduced_budget_has_no_tail() {
        let marker = truncation_marker("/x");
        // Eight lines against a twelve-line budget: the marker takes four, the
        // remaining eight fit exactly, so there is no tail to show.
        let text = (1..=8).map(|n| n.to_string()).collect::<Vec<String>>().join("\n");
        let out = bounded_preview(&text, &marker, 12, 10_000);
        assert!(out.ends_with(&marker));
        assert!(out.contains('\n'));
    }

    #[test]
    fn a_preview_without_a_tail_puts_the_marker_last() {
        let marker = truncation_marker("/x");
        // One content line under the reduced budget: no tail branch.
        let out = bounded_preview("only", &marker, 12, 1_000);
        assert_eq!(out, format!("only\n\n{}", marker));
        assert!(!out.contains(&format!("\n\n{}\n\n", marker)), "no blank lines after it");
    }

    // --- contextual_text ---

    #[test]
    fn text_parts_are_concatenated() {
        let o = output(vec![text("a"), ContentPart::File, text("b")], None);
        assert_eq!(contextual_text(&o), "ab");
    }

    #[test]
    fn no_content_falls_back_to_the_structured_value() {
        let o = output(vec![], Some(serde_json::json!({ "a": 1 })));
        assert_eq!(contextual_text(&o), "{\n  \"a\": 1\n}");
    }

    #[test]
    fn neither_content_nor_structured_gives_the_word_undefined() {
        // JSON.stringify(undefined) is undefined, and String(undefined) is the
        // five-letter word - not an empty string and not "null".
        let o = output(vec![], None);
        assert_eq!(contextual_text(&o), "undefined");
    }

    #[test]
    fn media_parts_are_the_non_text_ones() {
        let o = output(vec![text("a"), ContentPart::File, ContentPart::File], None);
        assert_eq!(o.media().len(), 2);
    }

    // --- fits / bound ---

    #[test]
    fn a_small_output_is_left_unchanged() {
        let o = output(vec![text("short")], None);
        assert!(fits(&contextual_text(&o), &limits(10, 100)));
        assert_eq!(bound_with_path(&o, &limits(10, 100), "/x"), Bound::Unchanged);
    }

    #[test]
    fn an_output_over_the_line_budget_is_spilled() {
        let o = output(vec![text("1\n2\n3\n4\n5")], None);
        assert!(!fits(&contextual_text(&o), &limits(2, 10_000)));
        match bound_with_path(&o, &limits(2, 10_000), "/data/tool_1") {
            Bound::Spilled { text, path } => {
                assert_eq!(path, "/data/tool_1");
                assert!(text.contains("truncated"));
            }
            other => panic!("expected a spill, got {:?}", other),
        }
    }

    #[test]
    fn an_output_over_the_byte_budget_is_spilled() {
        let o = output(vec![text(&"x".repeat(500))], None);
        assert!(!fits(&contextual_text(&o), &limits(1_000, 100)));
    }

    #[test]
    fn the_marker_names_the_file_the_output_went_to() {
        let m = truncation_marker("/data/tool-output/tool_68b9");
        assert_eq!(m, "... output truncated; full content saved to /data/tool-output/tool_68b9 ...");
    }

    // --- StorageError ---

    #[test]
    fn an_error_message_without_a_cause_has_no_colon() {
        let e = StorageError { operation: Operation::Write, cause: String::new() };
        assert_eq!(e.message(), "Failed to write tool output");
    }

    #[test]
    fn an_error_message_with_a_cause_names_both() {
        let e = StorageError { operation: Operation::Encode, cause: "bad json".into() };
        assert_eq!(e.message(), "Failed to encode tool output: bad json");
    }

    #[test]
    fn the_error_serialises_its_operation_in_lower_case() {
        let e = StorageError { operation: Operation::Write, cause: "x".into() };
        assert_eq!(serde_json::to_value(&e).unwrap()["operation"], serde_json::json!("write"));
    }

    #[test]
    fn the_limits_default_to_the_source_values() {
        let d = Limits::default();
        assert_eq!(d.max_lines, MAX_LINES);
        assert_eq!(d.max_bytes, MAX_BYTES);
        assert_eq!(RETENTION_DAYS, 7);
        assert_eq!(RETENTION_MILLIS, 604_800_000);
        assert_eq!(MANAGED_DIRECTORY, "tool-output");
        assert_eq!(FILE_PREFIX, "tool_");
    }
}