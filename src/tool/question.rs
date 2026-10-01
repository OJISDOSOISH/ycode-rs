//! Port of the portable part of `opencode/packages/core/src/tool/question.ts`.
//!
//! The question contract (`Info`) and the answer type (`Answer`) already exist
//! in `crate::question`, so this module re-exports them rather than writing a
//! second copy of the same shapes.
//!
//! What is ported is `toModelOutput`, and its formatting is unusual in three
//! ways worth pinning:
//!
//! - questions and answers are paired BY INDEX, not by identity, so a missing
//!   or empty answer at position `i` prints `Unanswered`;
//! - an empty list of answers is `Unanswered` as well, because the TS tests
//!   `answers[index]?.length`, and zero length is falsy;
//! - the question text goes inside double quotes with NO escaping, so a
//!   question containing a quote produces a malformed line. That is the
//!   source's behaviour, not an oversight of the port.
//!
//! Answers beyond the number of questions are ignored, and a question list
//! that is empty yields a sentence with an empty list between the colons.

use serde::{Deserialize, Serialize};

pub use crate::question::{Answer, Info as Prompt};

/// Tool name.
pub const NAME: &str = "question";

/// `QuestionTool.description`, verbatim: the model reads this text.
pub const DESCRIPTION: &str = concat!(
    "Use this tool when you need to ask the user questions during execution. ",
    "This allows you to:\n",
    "1. Gather user preferences or requirements\n",
    "2. Clarify ambiguous instructions\n",
    "3. Get decisions on implementation choices as you work\n",
    "4. Offer choices to the user about what direction to take.\n",
    "\n",
    "Usage notes:\n",
    "- When `custom` is enabled (default), a \"Type your own answer\" option is added ",
    "automatically; don't include \"Other\" or catch-all options\n",
    "- Answers are returned as arrays of labels; set `multiple: true` to allow ",
    "selecting more than one\n",
    "- If you recommend a specific option, make that the first option in the list ",
    "and add \"(Recommended)\" at the end of the label",
);

/// Printed in place of the answer when a question was not answered.
pub const UNANSWERED: &str = "Unanswered";

/// `QuestionTool.Input`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Input {
    pub questions: Vec<Prompt>,
}

/// `QuestionTool.Output`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Output {
    pub answers: Vec<Answer>,
}

/// `toModelOutput`: one `"question"="answer"` pair per question, comma separated.
pub fn to_model_output(questions: &[Prompt], answers: &[Answer]) -> String {
    let formatted: Vec<String> = questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let answer = match answers.get(index) {
                Some(a) if !a.is_empty() => a.join(", "),
                _ => UNANSWERED.to_string(),
            };
            format!("\"{}\"=\"{}\"", question.question, answer)
        })
        .collect();
    format!(
        "User has answered your questions: {}. You can now continue with the user's answers in mind.",
        formatted.join(", ")
    )
}

/// The tool's model content: a single text block.
pub fn to_model_content(input: &Input, output: &Output) -> Vec<(String, String)> {
    vec![("text".to_string(), to_model_output(&input.questions, &output.answers))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::question::Option_;

    fn prompt(question: &str) -> Prompt {
        Prompt {
            question: question.to_string(),
            header: String::new(),
            options: vec![Option_ {
                label: "yes".to_string(),
                description: String::new(),
            }],
            multiple: None,
            custom: None,
        }
    }

    #[test]
    fn a_single_answer_is_rendered_verbatim() {
        let out = to_model_output(&[prompt("Ship it?")], &[vec!["yes".to_string()]]);
        assert_eq!(
            out,
            "User has answered your questions: \"Ship it?\"=\"yes\". \
             You can now continue with the user's answers in mind."
        );
    }

    #[test]
    fn several_answers_are_joined_with_a_comma_and_a_space() {
        let out = to_model_output(&[prompt("Pick")], &[vec!["a".into(), "b".into()]]);
        assert!(out.contains("\"Pick\"=\"a, b\""), "{}", out);
    }

    #[test]
    fn questions_are_paired_by_index_not_by_identity() {
        let out = to_model_output(
            &[prompt("first"), prompt("second")],
            &[vec!["one".into()], vec!["two".into()]],
        );
        assert!(out.contains("\"first\"=\"one\""), "{}", out);
        assert!(out.contains("\"second\"=\"two\""), "{}", out);
    }

    #[test]
    fn a_missing_answer_becomes_unanswered() {
        let out = to_model_output(&[prompt("q")], &[]);
        assert!(out.contains("\"q\"=\"Unanswered\""), "{}", out);
    }

    #[test]
    fn an_empty_answer_also_becomes_unanswered() {
        // The TS tests `answers[index]?.length`, so a zero-length array is
        // falsy and prints the placeholder, not an empty string.
        let out = to_model_output(&[prompt("q")], &[vec![]]);
        assert!(out.contains("\"q\"=\"Unanswered\""), "{}", out);
    }

    #[test]
    fn only_the_missing_position_is_unanswered() {
        let out = to_model_output(
            &[prompt("a"), prompt("b")],
            &[vec!["x".into()], vec![]],
        );
        assert!(out.contains("\"a\"=\"x\""), "{}", out);
        assert!(out.contains("\"b\"=\"Unanswered\""), "{}", out);
    }

    #[test]
    fn surplus_answers_are_ignored() {
        let out = to_model_output(
            &[prompt("only")],
            &[vec!["used".into()], vec!["extra".into()]],
        );
        assert!(!out.contains("extra"), "an answer with no question is dropped: {}", out);
    }

    #[test]
    fn the_quote_in_a_question_is_not_escaped() {
        // Recorded deliberately: the source does not escape, so a question
        // containing a double quote yields a malformed line.
        let out = to_model_output(&[prompt("say \"hi\"?")], &[vec!["yes".into()]]);
        assert!(out.contains("\"say \"hi\"?\"=\"yes\""), "{}", out);
    }

    #[test]
    fn no_question_yields_an_empty_list_between_the_colons() {
        let out = to_model_output(&[], &[]);
        assert_eq!(
            out,
            "User has answered your questions: . \
             You can now continue with the user's answers in mind."
        );
    }

    #[test]
    fn the_description_keeps_its_usage_notes() {
        assert!(DESCRIPTION.contains("Type your own answer"));
        assert!(DESCRIPTION.contains("multiple: true"));
        assert!(DESCRIPTION.contains("(Recommended)"));
    }

    #[test]
    fn the_contract_wraps_a_list_of_answers() {
        let v = serde_json::to_value(Output { answers: vec![vec!["a".into()]] }).unwrap();
        assert_eq!(v, serde_json::json!({ "answers": [["a"]] }));
    }

    #[test]
    fn the_tool_hands_the_model_one_text_block() {
        let input = Input { questions: vec![prompt("q")] };
        let output = Output { answers: vec![vec!["a".into()]] };
        let blocks = to_model_content(&input, &output);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "text");
        assert!(blocks[0].1.starts_with("User has answered your questions:"));
    }
}