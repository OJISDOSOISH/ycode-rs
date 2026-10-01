//! Port of the portable part of `opencode/packages/core/src/tool/todowrite.ts`.
//!
//! The todo contract itself already exists in `session::todo` (`SessionTodo.Info`
//! in the TS), so this module re-exports it rather than writing a third copy
//! of `{content, status, priority}`.
//!
//! Note that `status` and `priority` are plain STRINGS in the schema, not
//! closed enums: the TS documents the expected values in prose ("pending,
//! in_progress, completed, cancelled" and "high, medium, low") but does not
//! enforce them. Typing them as enums here would reject configurations the
//! original accepts, so they stay strings.
//!
//! `toModelOutput` is `JSON.stringify(todos, null, 2)`: two-space indentation,
//! no trailing newline, which is what the model sees.

use serde::{Deserialize, Serialize};

pub use super::session::todo::Info;

/// Tool name.
pub const NAME: &str = "todowrite";

/// `TodoWriteTool.Input`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Input {
    pub todos: Vec<Info>,
}

/// `TodoWriteTool.Output`: the list as stored, not as submitted.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Output {
    pub todos: Vec<Info>,
}

/// `JSON.stringify(output.todos, null, 2)`, faithfully: pretty, no trailing newline.
pub fn to_model_output(output: &Output) -> String {
    serde_json::to_string_pretty(&output.todos).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn todo(content: &str, status: &str, priority: &str) -> Info {
        Info {
            content: content.to_string(),
            status: status.to_string(),
            priority: priority.to_string(),
        }
    }

    #[test]
    fn the_input_is_a_list_of_todos() {
        let input = Input { todos: vec![todo("a", "pending", "high")] };
        let v = serde_json::to_value(&input).unwrap();
        assert_eq!(
            v,
            json!({ "todos": [{ "content": "a", "status": "pending", "priority": "high" }] })
        );
    }

    #[test]
    fn the_model_output_is_pretty_printed_with_two_spaces() {
        let output = Output { todos: vec![todo("a", "pending", "high")] };
        let text = to_model_output(&output);
        assert!(text.contains("\n  "), "two-space indentation expected:\n{}", text);
        assert!(!text.ends_with('\n'), "JSON.stringify adds no trailing newline");
        // and it must parse back to the same list
        let back: Vec<Info> = serde_json::from_str(&text).unwrap();
        assert_eq!(back, output.todos);
    }

    #[test]
    fn an_empty_list_still_serialises_as_an_array() {
        assert_eq!(to_model_output(&Output::default()), "[]");
    }

    #[test]
    fn status_and_priority_stay_free_strings() {
        // The schema does not enforce the documented values; a port that did
        // would refuse configurations the original accepts.
        let t = todo("a", "in-progress", "urgent");
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["status"], json!("in-progress"));
        assert_eq!(v["priority"], json!("urgent"));
    }

    #[test]
    fn the_output_round_trips() {
        let output = Output {
            todos: vec![todo("a", "pending", "low"), todo("b", "completed", "medium")],
        };
        let back: Output =
            serde_json::from_str(&serde_json::to_string(&output).unwrap()).unwrap();
        assert_eq!(back, output);
    }
}