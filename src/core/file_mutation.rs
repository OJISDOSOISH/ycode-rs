//! Port of the portable part of
//! `opencode/packages/core/src/file-mutation.ts`.
//!
//! Not ported: the service. Every method here serialises on a per-canonical-path
//! lock and talks to the filesystem, and none of that survives without a process
//! and a disk. What is ported is the BOM and byte logic, and the rule that says
//! whether a write reports `existed` - which is model-visible, because it is
//! what the write tool prints as `Wrote` or `Created`.
//!
//! # Two `splitBom`s, and they do not agree
//!
//! This file's helpers and the ones in `tool::edit` have the same names and
//! DIFFERENT behaviour, in the same TypeScript codebase:
//!
//! | | strips | when restoring |
//! |---|---|---|
//! | `tool/edit.ts` | exactly ONE leading BOM (`startsWith` + `slice(1)`) | prepends without stripping |
//! | `file-mutation.ts` | ALL leading BOMs (`replace(/^\uFEFF+/, "")`) | strips all, then prepends one |
//!
//! They differ only for text that begins with two or more BOMs, which is
//! pathological input - and that is exactly the input where the two answers
//! differ, so the difference is invisible in normal use and real when it
//! matters. Both ports keep their own file's behaviour; neither was "corrected"
//! to match the other, because each is a faithful port of a different function.
//!
//! # The `existed` rules
//!
//! Four operations report `existed`, and they do NOT all mean the same thing:
//!
//! - `create` always reports `false`, even when the file was already there -
//!   because the write is conditional on it NOT being there, so reaching the
//!   return means it was absent a moment ago.
//! - `writeIfUnchanged` always reports `true`, because it just read the target
//!   successfully; there is no existence question left to ask.
//! - `write` and `remove` report what the filesystem said.
//!
//! Collapsing these into one helper would be the obvious simplification and it
//! would be wrong: it would make a create report `true` after overwriting, which
//! is the one thing `create` exists to prevent.

use serde::{Deserialize, Serialize};

use super::location_mutation::Target;

/// The UTF-8 BOM, as bytes.
pub const UTF8_BOM: [u8; 3] = [0xef, 0xbb, 0xbf];
/// The BOM as a character.
pub const BOM_CHAR: char = '\u{feff}';

/// `StaleContentError` and `TargetExistsError`, as one internally tagged enum.
///
/// `Schema.TaggedErrorClass("FileMutation.StaleContentError", { path })` takes
/// the tag as its first argument and the fields as one object, and the wire shape
/// is those two merged FLAT: `_tag` alongside the fields. That is exactly what
/// `#[serde(tag = "_tag")]` on an enum produces, so that is what this is.
///
/// The first draft modelled each error as a STRUCT with a `tag` field and
/// `#[serde(tag = "_tag")]` on the struct. Serde accepts that attribute on a
/// struct and ignores it - `tag` only means anything on an enum - so the tag
/// never reached the wire under that key. `repository.rs` and
/// `repository_cache.rs` already used the enum form, which is how the odd one out
/// became visible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag")]
pub enum FileMutationError {
    #[serde(rename = "FileMutation.StaleContentError")]
    StaleContent {
        /// The path whose on-disk content no longer matched.
        path: String,
    },
    #[serde(rename = "FileMutation.TargetExistsError")]
    TargetExists {
        /// The path that already exists.
        path: String,
    },
}

impl FileMutationError {
    /// The tag as it appears on the wire, namespace included.
    ///
    /// Exposed because the tag is the part callers match on, and a caller that
    /// has to re-spell `"FileMutation.StaleContentError"` in order to compare
    /// against it is a caller that can get it wrong.
    pub fn tag(&self) -> &'static str {
        match self {
            FileMutationError::StaleContent { .. } => "FileMutation.StaleContentError",
            FileMutationError::TargetExists { .. } => "FileMutation.TargetExistsError",
        }
    }

    /// The path the error is about.
    pub fn path(&self) -> &str {
        match self {
            FileMutationError::StaleContent { path } => path,
            FileMutationError::TargetExists { path } => path,
        }
    }
}

/// `WriteResult.operation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriteOperation {
    #[serde(rename = "write")]
    Write,
}

/// `RemoveResult.operation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoveOperation {
    #[serde(rename = "remove")]
    Remove,
}

/// `WriteResult`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteResult {
    pub operation: WriteOperation,
    pub target: String,
    pub resource: String,
    pub existed: bool,
}

/// `RemoveResult`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoveResult {
    pub operation: RemoveOperation,
    pub target: String,
    pub resource: String,
    pub existed: bool,
}

/// `writeResult`.
pub fn write_result(target: &Target, existed: bool) -> WriteResult {
    WriteResult {
        operation: WriteOperation::Write,
        target: target.canonical.clone(),
        resource: target.resource.clone(),
        existed,
    }
}

/// `removeResult`.
pub fn remove_result(target: &Target, existed: bool) -> RemoveResult {
    RemoveResult {
        operation: RemoveOperation::Remove,
        target: target.canonical.clone(),
        resource: target.resource.clone(),
        existed,
    }
}

/// `splitBom` as THIS file defines it: strips EVERY leading BOM.
///
/// The boolean records whether any were there, which is what the caller needs
/// to decide whether to restore one.
pub fn split_bom(text: &str) -> (bool, &str) {
    let stripped = text.trim_start_matches(BOM_CHAR);
    (stripped.len() != text.len(), stripped)
}

/// `joinBom` as THIS file defines it: strip all leading BOMs, then prepend one
/// if asked.
///
/// The strip on the way in is what makes "at most one BOM" true no matter how
/// many the caller passed.
pub fn join_bom(text: &str, bom: bool) -> String {
    let stripped = split_bom(text).1;
    if bom {
        format!("{}{}", BOM_CHAR, stripped)
    } else {
        stripped.to_string()
    }
}

/// `hasUtf8Bom`: a short or empty buffer is not a BOM.
pub fn has_utf8_bom(content: &[u8]) -> bool {
    content.len() >= 3 && content[0] == UTF8_BOM[0] && content[1] == UTF8_BOM[1] && content[2] == UTF8_BOM[2]
}

/// `sameBytes`: equal length and equal content.
pub fn same_bytes(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len() && left.iter().zip(right.iter()).all(|(a, b)| a == b)
}

/// The BOM decision for `writeTextPreservingBom`.
///
/// The existing file's BOM wins over the incoming text's, but either one keeps
/// a BOM: `hasUtf8Bom(current) || next.bom`. That is an OR, so a text that
/// arrives without a BOM does not strip one an existing file already had.
pub fn preserve_bom(current: Option<&[u8]>, next_bom: bool) -> bool {
    current.map(has_utf8_bom).unwrap_or(false) || next_bom
}

/// The content `writeTextPreservingBom` would write, given what is on disk.
pub fn text_preserving_bom(existing: Option<&[u8]>, incoming: &str) -> String {
    let (next_bom, next_text) = split_bom(incoming);
    join_bom(next_text, preserve_bom(existing, next_bom))
}

/// The content `writeIfUnchanged` would write. No BOM handling at all: the
/// caller supplies exact bytes and the comparison is on them.
pub fn conditional_content(incoming: &str) -> &str {
    incoming
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Target {
        Target {
            canonical: "/repo/a.ts".into(),
            resource: "a.ts".into(),
            external_directory: None,
        }
    }

    // --- the BOM helpers, and how they differ from edit's ---

    #[test]
    fn split_bom_strips_every_leading_bom() {
        assert_eq!(split_bom("\u{feff}a"), (true, "a"));
        assert_eq!(split_bom("\u{feff}\u{feff}\u{feff}a"), (true, "a"), "all of them, not just the first");
        assert_eq!(split_bom("a"), (false, "a"));
        assert_eq!(split_bom(""), (false, ""));
    }

    #[test]
    fn split_bom_leaves_an_interior_bom_alone() {
        assert_eq!(split_bom("a\u{feff}b"), (false, "a\u{feff}b"));
    }

    #[test]
    fn join_bom_emits_at_most_one() {
        assert_eq!(join_bom("a", true), "\u{feff}a");
        assert_eq!(join_bom("a", false), "a");
        assert_eq!(
            join_bom("\u{feff}\u{feff}a", true),
            "\u{feff}a",
            "the incoming ones are stripped first"
        );
        assert_eq!(join_bom("\u{feff}a", false), "a", "and dropped when none is wanted");
    }

    #[test]
    fn this_file_splits_all_where_edit_splits_one() {
        // The two same-named helpers in the TS, side by side. They differ only
        // for two or more BOMs, which is why the difference is invisible in
        // normal use and real in that case.
        let doubled = "\u{feff}\u{feff}a";
        assert_eq!(split_bom(doubled).1, "a", "this file strips all");
        assert_eq!(
            doubled.strip_prefix('\u{feff}'),
            Some("\u{feff}a"),
            "edit strips exactly one, so this is the difference"
        );
    }

    #[test]
    fn a_utf8_bom_is_three_bytes_and_a_short_buffer_is_not_one() {
        assert!(has_utf8_bom(&[0xef, 0xbb, 0xbf]));
        assert!(has_utf8_bom(&[0xef, 0xbb, 0xbf, 0x61]), "trailing bytes are fine");
        assert!(!has_utf8_bom(&[0xef, 0xbb]), "two bytes is not three");
        assert!(!has_utf8_bom(&[]));
        assert!(!has_utf8_bom(&[0xef, 0xbb, 0xbe]), "one byte differs");
    }

    // --- byte comparison ---

    #[test]
    fn same_bytes_compares_length_first() {
        assert!(same_bytes(b"abc", b"abc"));
        assert!(!same_bytes(b"abc", b"ab"), "a prefix is not equal");
        assert!(!same_bytes(b"ab", b"abc"), "nor is a superstring");
        assert!(same_bytes(b"", b""));
    }

    #[test]
    fn same_bytes_rejects_a_single_differing_byte() {
        assert!(!same_bytes(b"abc", b"abd"));
        assert!(same_bytes(&[0u8; 16], &[0u8; 16]));
    }

    // --- the BOM decision ---

    #[test]
    fn an_existing_bom_survives_text_that_has_none() {
        assert!(preserve_bom(Some(&[0xef, 0xbb, 0xbf, b'a']), false), "the file's BOM wins");
    }

    #[test]
    fn incoming_text_can_introduce_a_bom() {
        assert!(preserve_bom(None, true));
        assert!(preserve_bom(Some(b"plain"), true));
    }

    #[test]
    fn neither_one_means_no_bom() {
        assert!(!preserve_bom(None, false));
        assert!(!preserve_bom(Some(b"plain"), false));
    }

    #[test]
    fn the_written_content_keeps_a_bom_exactly_once() {
        let existing = [0xef, 0xbb, 0xbf, b'o', b'l', b'd'];
        let out = text_preserving_bom(Some(&existing), "new");
        assert_eq!(out, "\u{feff}new");
    }

    #[test]
    fn text_without_a_bom_does_not_gain_one_when_the_file_had_none() {
        assert_eq!(text_preserving_bom(Some(b"old"), "new"), "new");
        assert_eq!(text_preserving_bom(None, "new"), "new");
    }

    #[test]
    fn doubled_incoming_boms_collapse_to_one() {
        assert_eq!(text_preserving_bom(None, "\u{feff}\u{feff}a"), "\u{feff}a");
    }

    #[test]
    fn a_conditional_write_touches_no_bom() {
        assert_eq!(conditional_content("\u{feff}a"), "\u{feff}a", "exact bytes, no normalisation");
    }

    // --- the results ---

    #[test]
    fn a_write_result_carries_the_canonical_target_and_the_resource() {
        let r = write_result(&target(), true);
        assert_eq!(r.target, "/repo/a.ts");
        assert_eq!(r.resource, "a.ts");
        assert_eq!(r.operation, WriteOperation::Write);
        assert!(r.existed);
    }

    #[test]
    fn a_remove_result_is_the_same_shape_with_the_other_operation() {
        let r = remove_result(&target(), false);
        assert_eq!(r.operation, RemoveOperation::Remove);
        assert!(!r.existed);
        assert_eq!(
            serde_json::to_value(&r).unwrap()["operation"],
            serde_json::json!("remove")
        );
    }

    #[test]
    fn the_operation_literals_are_write_and_remove() {
        assert_eq!(serde_json::to_value(WriteOperation::Write).unwrap(), serde_json::json!("write"));
        assert_eq!(serde_json::to_value(RemoveOperation::Remove).unwrap(), serde_json::json!("remove"));
    }

    #[test]
    fn the_errors_serialise_under_their_tags() {
        // The tag is namespaced in the TypeScript - `FileMutation.StaleContentError`,
        // with a dot - and the fields sit BESIDE it on the same object, not under
        // it. Both halves are asserted, because a tag alone would pass even if
        // the fields were nested one level too deep.
        let e = FileMutationError::StaleContent { path: "/a".into() };
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            serde_json::json!({ "_tag": "FileMutation.StaleContentError", "path": "/a" })
        );
        let e = FileMutationError::TargetExists { path: "/b".into() };
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            serde_json::json!({ "_tag": "FileMutation.TargetExistsError", "path": "/b" })
        );
    }

    #[test]
    fn an_error_round_trips_through_its_tag() {
        // The direction that matters: a caller reading our output has to get the
        // same variant back. A shape that only serialises correctly would pass the
        // test above and still break every consumer.
        for e in [
            FileMutationError::StaleContent { path: "/a".into() },
            FileMutationError::TargetExists { path: "/b".into() },
        ] {
            let text = serde_json::to_string(&e).unwrap();
            let back: FileMutationError = serde_json::from_str(&text).unwrap();
            assert_eq!(back, e, "round trip of {}", text);
            assert_eq!(back.tag(), e.tag());
            assert_eq!(back.path(), e.path());
        }
    }

    #[test]
    fn the_bom_constant_matches_the_character() {
        assert_eq!(UTF8_BOM.len(), 3);
        assert_eq!(BOM_CHAR as u32, 0xfeff);
    }
}