//! Port of the portable part of `opencode/packages/core/src/tool/write.ts`.
//!
//! The write tool's contract, and the one line that decides what the model
//! reads back. `toModelOutput` is a single template with a conditional word:
//! an existing file reports "Wrote", a new one "Created". That word is the
//! only signal the model gets about whether the write created or replaced
//! anything, so it is worth a test per branch rather than a sample.
//!
//! `existed` is required on the output: the TS has no default for it, and
//! guessing "created" when the field is missing would silently misreport.

use serde::{Deserialize, Serialize};

/// Tool name.
pub const NAME: &str = "write";

/// `WriteTool.Input`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Input {
    pub path: String,
    pub content: String,
}

/// `WriteTool.Output`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Output {
    pub operation: WriteOperation,
    pub target: String,
    pub resource: String,
    pub existed: bool,
}

/// `Schema.Literal("write")` on `operation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriteOperation {
    #[serde(rename = "write")]
    Write,
}

/// The two words the model sees, and the exact sentence around them.
pub fn to_model_output(output: &Output) -> String {
    format!(
        "{} file successfully: {}",
        if output.existed { "Wrote" } else { "Created" },
        output.resource
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn output(existed: bool) -> Output {
        Output {
            operation: WriteOperation::Write,
            target: "/repo/a.ts".into(),
            resource: "src/a.ts".into(),
            existed,
        }
    }

    #[test]
    fn an_existing_file_reports_wrote() {
        assert_eq!(
            to_model_output(&output(true)),
            "Wrote file successfully: src/a.ts"
        );
    }

    #[test]
    fn a_new_file_reports_created() {
        assert_eq!(
            to_model_output(&output(false)),
            "Created file successfully: src/a.ts"
        );
    }

    #[test]
    fn the_output_carries_its_operation_literal_and_its_boolean() {
        let v = serde_json::to_value(output(true)).unwrap();
        assert_eq!(
            v,
            json!({
                "operation": "write",
                "target": "/repo/a.ts",
                "resource": "src/a.ts",
                "existed": true
            })
        );
    }

    #[test]
    fn existed_false_is_written_not_dropped() {
        let v = serde_json::to_value(output(false)).unwrap();
        assert_eq!(v["existed"], json!(false));
        assert!(v.get("existed").is_some());
    }

    #[test]
    fn an_output_without_existed_does_not_deserialise() {
        let raw = json!({
            "operation": "write", "target": "/a", "resource": "a"
        });
        assert!(serde_json::from_value::<Output>(raw).is_err());
    }

    #[test]
    fn the_input_is_the_path_and_the_content() {
        let v = serde_json::to_value(Input { path: "a".into(), content: "x".into() }).unwrap();
        assert_eq!(v, json!({ "path": "a", "content": "x" }));
    }
}