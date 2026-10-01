//! Port of the portable part of `opencode/packages/core/src/tool/tool.ts`.
//!
//! `Tool.make` is generic machinery over Effect schemas, and the schema-to-JSON
//! step is Effect's own code generation, so neither is ported. What is ported
//! is the machinery's two decisions that outlive those pieces: which names may
//! be registered at all, and how a tool's model content is assembled once its
//! output exists.
//!
//! `validateName` is the gate every registration passes, and the regex is the
//! contract. It is anchored, case sensitive, and the quantifier on the tail is
//! `{0,63}` - so the whole name is 1 to 64 characters, starting with a letter.
//! A leading digit, a leading underscore, a dot, or a 65th character all fail.
//! That last one is easy to get wrong: `{0,63}` after the first character is
//! exactly 64 total, not 63, and not unbounded.
//!
//! The content assembly has a three-way fallback that is worth stating, because
//! the empty list is a real outcome and not an oversight:
//!
//! - a tool with `toModelOutput` uses it, and a file part becomes a `data:`
//!   URI carrying the mime and the base64 payload;
//! - a tool without it falls back to the output itself, but ONLY when that
//!   output is a string;
//! - a tool without it whose output is not a string produces NO content at
//!   all. The result is then invisible to the model, which is the source's
//!   behaviour and the reason `toModelOutput` exists on most tools.

use serde::{Deserialize, Serialize};

/// The registration name pattern: a letter, then up to 63 of letter/digit/`_`/`-`.
pub const NAME_PATTERN: &str = "^[A-Za-z][A-Za-z0-9_-]{0,63}$";

/// The longest name the pattern accepts: the first character plus 63.
pub const MAX_NAME_LENGTH: usize = 64;

/// `validateName`, as a predicate.
pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    let tail = chars.as_str();
    if tail.len() > 63 {
        return false;
    }
    tail.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// The registration failure text, verbatim.
pub fn invalid_name_message(name: &str) -> String {
    format!("Invalid tool name: {}", name)
}

/// The input failure text, verbatim.
pub fn invalid_input_message(reason: &str) -> String {
    format!("Invalid tool input: {}", reason)
}

/// The output failure text, verbatim.
pub fn invalid_output_message(reason: &str) -> String {
    format!("Tool returned an invalid value for its output schema: {}", reason)
}

/// `runtimeOf`'s failure, verbatim.
pub const ERR_NOT_A_TOOL: &str = "Invalid Core Tool value";

/// `Tool.Content`: what a tool hands back to the model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
    /// Note `data` here. The file part of a tool's own content carries `data`,
    /// while the content contract in `schema::llm` carries `uri`; the same
    /// object gets both only after `to_model_content` builds the data URI.
    #[serde(rename = "file")]
    File {
        data: String,
        mime: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

/// The settled content the model receives.
///
/// A file part arrives here as a `data:` URI, which is the one place the two
/// contracts meet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SettledContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "file")]
    File {
        uri: String,
        mime: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

/// `data:${mime};base64,${data}`.
pub fn data_uri(mime: &str, data: &str) -> String {
    format!("data:{};base64,{}", mime, data)
}

/// Convert one `Tool.Content` part into its settled form.
pub fn settle_part(part: &Content) -> SettledContent {
    match part {
        Content::Text { text } => SettledContent::Text { text: text.clone() },
        Content::File { data, mime, name } => SettledContent::File {
            uri: data_uri(mime, data),
            mime: mime.clone(),
            name: name.clone(),
        },
    }
}

/// The three-way fallback, with the string output as the middle case.
///
/// `output` is the already-encoded output. When the tool has no
/// `toModelOutput`, only a string output reaches the model.
pub fn content_from_output(to_model_output: Option<&[Content]>, output: Option<&str>) -> Vec<SettledContent> {
    if let Some(parts) = to_model_output {
        return parts.iter().map(settle_part).collect();
    }
    match output {
        Some(text) => vec![SettledContent::Text { text: text.to_string() }],
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_plain_letter_is_a_valid_name() {
        assert!(is_valid_name("a"));
        assert!(is_valid_name("read"));
        assert!(is_valid_name("apply_patch"));
    }

    #[test]
    fn digits_underscores_and_dashes_are_allowed_after_the_first_character() {
        assert!(is_valid_name("a1"));
        assert!(is_valid_name("a_b"));
        assert!(is_valid_name("a-b"));
        assert!(is_valid_name("Read2_x-y"));
    }

    #[test]
    fn the_first_character_must_be_a_letter() {
        assert!(!is_valid_name("1abc"));
        assert!(!is_valid_name("_abc"));
        assert!(!is_valid_name("-abc"));
        assert!(!is_valid_name(".abc"));
    }

    #[test]
    fn the_empty_name_is_refused() {
        assert!(!is_valid_name(""));
    }

    #[test]
    fn a_dot_anywhere_is_refused() {
        assert!(!is_valid_name("a.b"));
        assert!(!is_valid_name("read.write"));
    }

    #[test]
    fn the_pattern_allows_sixty_four_characters_and_no_more() {
        let sixty_four = "a".repeat(MAX_NAME_LENGTH);
        let sixty_five = "a".repeat(MAX_NAME_LENGTH + 1);
        assert_eq!(sixty_four.len(), 64);
        assert!(is_valid_name(&sixty_four), "64 is inside the quantifier");
        assert!(!is_valid_name(&sixty_five), "65 is outside it");
    }

    #[test]
    fn upper_case_letters_are_letters_too() {
        assert!(is_valid_name("ABC"));
        assert!(is_valid_name("TodoWrite"));
        assert!(is_valid_name("aB1_"));
    }

    #[test]
    fn a_non_ascii_letter_does_not_pass_as_alphabetic() {
        // `is_ascii_alphabetic` is the rule; JS `\w` without the `u` flag would
        // also exclude it, so the two agree here.
        assert!(!is_valid_name("é"));
        assert!(!is_valid_name("aé"));
    }

    #[test]
    fn the_failure_texts_are_verbatim() {
        assert_eq!(invalid_name_message("a.b"), "Invalid tool name: a.b");
        assert_eq!(invalid_input_message("missing path"), "Invalid tool input: missing path");
        assert_eq!(
            invalid_output_message("bad field"),
            "Tool returned an invalid value for its output schema: bad field"
        );
        assert_eq!(ERR_NOT_A_TOOL, "Invalid Core Tool value");
    }

    #[test]
    fn a_file_part_becomes_a_data_uri() {
        let part = Content::File { data: "AAA".into(), mime: "image/png".into(), name: Some("i.png".into()) };
        assert_eq!(data_uri("image/png", "AAA"), "data:image/png;base64,AAA");
        assert_eq!(
            settle_part(&part),
            SettledContent::File {
                uri: "data:image/png;base64,AAA".into(),
                mime: "image/png".into(),
                name: Some("i.png".into()),
            }
        );
    }

    #[test]
    fn a_text_part_passes_through_unchanged() {
        let part = Content::Text { text: "hello".into() };
        assert_eq!(settle_part(&part), SettledContent::Text { text: "hello".into() });
    }

    #[test]
    fn the_formatter_wins_over_the_string_output() {
        let parts = vec![Content::Text { text: "formatted".into() }];
        let settled = content_from_output(Some(&parts), Some("raw"));
        assert_eq!(settled, vec![SettledContent::Text { text: "formatted".into() }]);
    }

    #[test]
    fn without_a_formatter_a_string_output_is_the_content() {
        assert_eq!(
            content_from_output(None, Some("raw")),
            vec![SettledContent::Text { text: "raw".into() }]
        );
    }

    #[test]
    fn without_a_formatter_and_without_a_string_output_there_is_no_content() {
        assert!(content_from_output(None, None).is_empty());
    }

    #[test]
    fn an_empty_formatter_result_stays_empty() {
        assert!(content_from_output(Some(&[]), Some("raw")).is_empty());
    }

    #[test]
    fn the_two_file_contracts_use_different_field_names() {
        let part = Content::File { data: "AAA".into(), mime: "image/png".into(), name: None };
        let v = json!(part);
        assert_eq!(v["data"], json!("AAA"));
        assert!(v.get("uri").is_none(), "the tool content carries data");

        let settled = SettledContent::File {
            uri: "data:image/png;base64,AAA".into(),
            mime: "image/png".into(),
            name: None,
        };
        let v = json!(settled);
        assert!(v.get("uri").is_some());
        assert!(v.get("name").is_none(), "an absent name is dropped, not null");
    }
}