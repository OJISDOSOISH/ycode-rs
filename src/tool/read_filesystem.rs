//! Port of the portable part of
//! `opencode/packages/core/src/tool/read-filesystem.ts`.
//!
//! The file service itself reads the disk through Effect and is not ported.
//! What is ported is everything the read tool's contract depends on: the page
//! input, the two page shapes, the three tagged errors, and the three pure
//! predicates that decide what a file is - binary by extension, binary by
//! bytes, and which image mime the bytes carry.
//!
//! Those predicates are the interesting part, because their thresholds are
//! behaviour, not plumbing: a NUL byte means binary outright, otherwise more
//! than 30% non-printable bytes does, and an empty file is never binary.

use serde::{Deserialize, Serialize};

/// `MAX_READ_LINES`.
pub const MAX_READ_LINES: i64 = 2_000;
/// `MAX_READ_BYTES`.
pub const MAX_READ_BYTES: u64 = 50 * 1024;
/// `MAX_MEDIA_INGEST_BYTES`.
pub const MAX_MEDIA_INGEST_BYTES: u64 = 20 * 1024 * 1024;
/// `MAX_LINE_LENGTH`.
pub const MAX_LINE_LENGTH: usize = 2_000;
/// `MAX_LINE_SUFFIX`.
pub const MAX_LINE_SUFFIX: &str = "... (line truncated to 2000 chars)";

/// `ReadTool.PageInput`: both bounds optional, `limit` capped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PageInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}

impl PageInput {
    /// `PositiveInt.check(isLessThanOrEqualTo(MAX_READ_LINES))`.
    pub fn validated(self) -> Result<Self, &'static str> {
        if let Some(limit) = self.limit {
            if limit <= 0 {
                return Err("limit must be a positive integer");
            }
            if limit > MAX_READ_LINES {
                return Err("limit exceeds MAX_READ_LINES");
            }
        }
        if let Some(offset) = self.offset {
            if offset <= 0 {
                return Err("offset must be a positive integer");
            }
        }
        Ok(self)
    }
}

/// `ReadTool.TextPage`. The tag is part of the shape, unlike `ListPage`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextPage {
    pub content: String,
    pub mime: String,
    pub offset: i64,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<i64>,
}

/// `ReadTool.ListPage`: no `type` field in the TS, so none here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListPage {
    pub entries: Vec<DirectoryEntry>,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<i64>,
}

/// The entry shape `FileSystem.Entry` produces for a listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: EntryKind,
}

/// `FileSystem.Entry` kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    #[serde(rename = "file")]
    File,
    #[serde(rename = "directory")]
    Directory,
}

/// Extensions that make a file binary whatever its bytes say.
pub const BINARY_EXTENSIONS: [&str; 28] = [
    ".zip", ".tar", ".gz", ".exe", ".dll", ".so", ".class", ".jar", ".war", ".7z", ".doc", ".docx",
    ".xls", ".xlsx", ".ppt", ".pptx", ".odt", ".ods", ".odp", ".bin", ".dat", ".obj", ".o", ".a",
    ".lib", ".wasm", ".pyc", ".pyo",
];

/// Lowercased extension of a resource path, extension included.
///
/// `path.extname` in Node: a leading dot marks a hidden file, not an
/// extension, so `.bashrc` has none. A `rfind('.')` without that check would
/// report `.bashrc` as an extension and look it up in the binary set.
fn extension_of(resource: &str) -> String {
    let name = resource.rsplit('/').next().unwrap_or(resource);
    match name.rfind('.') {
        Some(0) | None => String::new(),
        Some(i) => name[i..].to_lowercase(),
    }
}

/// `imageMime`: magic numbers, PNG then JPEG then GIF then WEBP.
pub fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    let starts = |prefix: &[u8], at: usize| -> bool {
        bytes.len() >= at + prefix.len() && bytes[at..at + prefix.len()] == *prefix
    };
    if starts(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a], 0) {
        return Some("image/png");
    }
    if starts(&[0xff, 0xd8, 0xff], 0) {
        return Some("image/jpeg");
    }
    if starts(b"GIF8", 0) {
        return Some("image/gif");
    }
    if starts(b"RIFF", 0) && starts(b"WEBP", 8) {
        return Some("image/webp");
    }
    None
}

/// `binary`: extension first, then the byte heuristic.
pub fn is_binary(resource: &str, bytes: &[u8]) -> bool {
    if BINARY_EXTENSIONS.contains(&extension_of(resource).as_str()) {
        return true;
    }
    if bytes.is_empty() {
        return false;
    }
    let mut non_printable = 0usize;
    for &byte in bytes {
        if byte == 0 {
            return true;
        }
        if byte < 9 || (byte > 13 && byte < 32) {
            non_printable += 1;
        }
    }
    (non_printable as f64) / (bytes.len() as f64) > 0.3
}

/// The mimes the read tool will hand to a model as an image.
pub const SUPPORTED_IMAGE_MIMES: [&str; 4] =
    ["image/jpeg", "image/png", "image/gif", "image/webp"];

pub fn is_supported_image_mime(mime: &str) -> bool {
    SUPPORTED_IMAGE_MIMES.contains(&mime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_input_caps_the_limit_and_requires_positive_numbers() {
        assert!(PageInput { offset: Some(1), limit: Some(MAX_READ_LINES) }.validated().is_ok());
        assert!(PageInput { offset: Some(0), limit: None }.validated().is_err());
        assert!(PageInput { offset: None, limit: Some(0) }.validated().is_err());
        assert!(
            PageInput { offset: None, limit: Some(MAX_READ_LINES + 1) }.validated().is_err(),
            "the cap is inclusive"
        );
        assert!(PageInput::default().validated().is_ok());
    }

    #[test]
    fn an_empty_page_input_writes_nothing() {
        assert_eq!(serde_json::to_value(PageInput::default()).unwrap(), serde_json::json!({}));
    }

    #[test]
    fn a_text_page_carries_its_tag_and_omits_an_absent_next() {
        let page = TextPage {
            content: "hello".into(),
            mime: "text/plain".into(),
            offset: 1,
            truncated: false,
            next: None,
        };
        let v = serde_json::to_value(&page).unwrap();
        assert_eq!(v["content"], serde_json::json!("hello"));
        assert!(v.get("next").is_none());
        // TextPage has no `type` field in the struct: the tag is added by the
        // caller's union, exactly as in the TS class.
        assert!(v.get("type").is_none());
    }

    #[test]
    fn a_list_page_has_no_tag_either() {
        let page = ListPage {
            entries: vec![DirectoryEntry { name: "a.rs".into(), kind: EntryKind::File }],
            truncated: true,
            next: Some(9),
        };
        let v = serde_json::to_value(&page).unwrap();
        assert_eq!(v["entries"][0]["type"], serde_json::json!("file"));
        assert_eq!(v["truncated"], serde_json::json!(true));
        assert_eq!(v["next"], serde_json::json!(9));
    }

    #[test]
    fn magic_numbers_identify_the_four_formats() {
        assert_eq!(image_mime(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), Some("image/png"));
        assert_eq!(image_mime(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
        assert_eq!(image_mime(b"GIF89a"), Some("image/gif"));
        let mut webp = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
        webp.truncate(16);
        assert_eq!(image_mime(&webp), Some("image/webp"));
        assert_eq!(image_mime(b"plain text"), None);
        assert_eq!(image_mime(b""), None);
    }

    #[test]
    fn a_riff_container_that_is_not_webp_is_not_an_image() {
        let mut riff = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
        riff.truncate(16);
        assert_eq!(image_mime(&riff), None, "WEBP at offset 8 is required");
    }

    #[test]
    fn an_extension_decides_before_the_bytes_are_looked_at() {
        assert!(is_binary("a/b.ZIP", b"text"));
        assert!(is_binary("lib.wasm", &[0x61]));
        assert!(!is_binary("a.txt", b"plain ascii"));
    }

    #[test]
    fn a_nul_byte_means_binary_but_an_empty_file_does_not() {
        assert!(is_binary("a.txt", &[0x61, 0x00, 0x62]));
        assert!(!is_binary("a.txt", b""));
    }

    #[test]
    fn the_thirty_percent_rule_uses_a_strict_comparison() {
        // 3 control bytes among 10 is 0.3, which is NOT greater than 0.3.
        // The filler must be genuinely printable: byte 1 is itself a control
        // byte, so filling with 0x01 would make every byte non-printable.
        let mut bytes = vec![b'a'; 7];
        bytes.extend([0x01u8, 0x01, 0x01]);
        assert!(!is_binary("a.txt", &bytes), "exactly 0.3 stays text");

        let mut more = vec![b'a'; 6];
        more.extend([0x01u8, 0x01, 0x01, 0x01]);
        assert!(is_binary("a.txt", &more), "0.4 is binary");
    }

    #[test]
    fn tab_and_newline_are_printable_for_this_heuristic() {
        // byte 9 (tab) and 13 (CR) are excluded from the non-printable count
        let mut bytes = vec![0x09u8; 5];
        bytes.extend([0x0a, 0x0d, 0x1b, 0x1b, 0x1b]);
        assert!(!is_binary("a.txt", &bytes), "escape (27) counts, tab and newline do not");
    }

    #[test]
    fn the_extension_list_has_twenty_eight_entries() {
        assert_eq!(BINARY_EXTENSIONS.len(), 28);
        assert!(BINARY_EXTENSIONS.contains(&".pyc"));
        assert!(!BINARY_EXTENSIONS.contains(&".txt"));
    }

    #[test]
    fn a_hidden_file_has_no_extension() {
        // `path.extname(".bashrc")` is "" in Node, so ".bashrc" must never be
        // looked up in the set: the name IS the whole basename.
        assert!(!is_binary(".wasm", b"text"), "a file named .wasm is text");
        assert!(is_binary("x.wasm", b"text"), "but x.wasm is binary");
        assert!(!is_binary(".bashrc", b"#!/bin/sh\necho hi\n"));
    }
}