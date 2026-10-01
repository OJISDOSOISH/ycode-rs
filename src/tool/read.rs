//! Port of the portable part of `opencode/packages/core/src/tool/read.ts`.
//!
//! The tool service itself is Effect and not ported. What is ported is the
//! tool's wire contract - its name, its input schema - and `toModelOutput`,
//! which is the part that decides what the model actually sees.
//!
//! That function is small and exacting, and the tests pin its behaviour: it
//! returns NOTHING unless the content is base64 AND the mime is one of the four
//! supported image types. A text page, a directory listing, or a base64 payload
//! with any other mime all produce an empty list - the tool result stays out of
//! the model's context rather than being pushed as text.

use serde::{Deserialize, Serialize};

use super::read_filesystem::is_supported_image_mime;

/// Tool name, `export const name = "read"`.
pub const NAME: &str = "read";

/// The read tool input: a path plus optional paging.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Input {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

/// `FileSystem.Content.encoding`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Encoding {
    #[serde(rename = "utf8")]
    Utf8,
    #[serde(rename = "base64")]
    Base64,
}

/// The slice of `FileSystem.Content` the mapping inspects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Content {
    pub content: String,
    pub encoding: Encoding,
    pub mime: String,
}

/// `Tool.Content`: what the tool hands back to the model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ModelContent {
    #[serde(rename = "text")]
    Text { text: String },
    /// Note `data`, not `uri`: the tool content contract and the tool-result
    /// content contract in `schema::llm` are two different shapes in the TS.
    #[serde(rename = "file")]
    File {
        data: String,
        mime: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
}

/// The exact text the TS puts in front of an image.
pub const IMAGE_READ_OK: &str = "Image read successfully";

/// `toModelOutput`: empty unless base64 and a supported image mime.
pub fn to_model_output(input: &Input, content: Option<&Content>) -> Vec<ModelContent> {
    let Some(content) = content else { return Vec::new() };
    if content.encoding != Encoding::Base64 {
        return Vec::new();
    }
    if !is_supported_image_mime(&content.mime) {
        return Vec::new();
    }
    vec![
        ModelContent::Text { text: IMAGE_READ_OK.to_string() },
        ModelContent::File {
            data: content.content.clone(),
            mime: content.mime.clone(),
            name: Some(input.path.clone()),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base64_image() -> Content {
        Content {
            content: "AAA".into(),
            encoding: Encoding::Base64,
            mime: "image/png".into(),
        }
    }

    #[test]
    fn a_supported_base64_image_becomes_a_text_then_a_file() {
        let input = Input { path: "img.png".into(), offset: None, limit: None };
        let out = to_model_output(&input, Some(&base64_image()));
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], ModelContent::Text { text: IMAGE_READ_OK.to_string() });
        match &out[1] {
            ModelContent::File { data, mime, name } => {
                assert_eq!(data, "AAA");
                assert_eq!(mime, "image/png");
                assert_eq!(name.as_deref(), Some("img.png"), "the name is the input path");
            }
            other => panic!("expected a file content, got {:?}", other),
        }
    }

    #[test]
    fn a_utf8_payload_produces_nothing() {
        let input = Input { path: "a.txt".into(), offset: None, limit: None };
        let text = Content {
            content: "hello".into(),
            encoding: Encoding::Utf8,
            mime: "text/plain".into(),
        };
        assert!(to_model_output(&input, Some(&text)).is_empty());
    }

    #[test]
    fn base64_with_an_unsupported_mime_produces_nothing() {
        let input = Input { path: "a.bin".into(), offset: None, limit: None };
        let pdf = Content {
            content: "JVBER".into(),
            encoding: Encoding::Base64,
            mime: "application/pdf".into(),
        };
        assert!(to_model_output(&input, Some(&pdf)).is_empty());
    }

    #[test]
    fn a_page_or_listing_produces_nothing() {
        let input = Input { path: "a".into(), offset: None, limit: None };
        assert!(to_model_output(&input, None).is_empty());
    }

    #[test]
    fn every_supported_mime_is_accepted() {
        let input = Input { path: "i".into(), offset: None, limit: None };
        for mime in ["image/jpeg", "image/png", "image/gif", "image/webp"] {
            let c = Content {
                content: "A".into(),
                encoding: Encoding::Base64,
                mime: mime.into(),
            };
            assert_eq!(to_model_output(&input, Some(&c)).len(), 2, "{}", mime);
        }
    }

    #[test]
    fn the_input_omits_absent_paging() {
        let v = serde_json::to_value(Input { path: "a".into(), offset: None, limit: None }).unwrap();
        assert_eq!(v, json!({ "path": "a" }));
        let v = serde_json::to_value(Input { path: "a".into(), offset: Some(1), limit: Some(20) }).unwrap();
        assert_eq!(v, json!({ "path": "a", "offset": 1, "limit": 20 }));
    }

    #[test]
    fn the_file_content_uses_data_and_omits_an_absent_name() {
        let c = ModelContent::File { data: "AAA".into(), mime: "image/png".into(), name: None };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v, json!({ "type": "file", "data": "AAA", "mime": "image/png" }));
        assert!(v.get("uri").is_none(), "this contract uses data, not uri");
    }
}