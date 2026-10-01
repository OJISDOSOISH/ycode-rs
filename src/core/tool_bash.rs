//! Port of the portable part of `opencode/packages/core/src/tool/bash.ts`.
//!
//! The shell boundary itself runs child processes through Effect and is not
//! ported. What is ported is the tool's contract, its bounds, and the one
//! formatter that decides what the model reads.
//!
//! `modelOutput` has an exact shape that is easy to get wrong: warnings come
//! FIRST, without a leading newline, followed by a blank line, then the single
//! sentence. With no warnings the sentence starts the text outright. And the
//! timeout case says "timed out", not "exited with code" - a timeout has no
//! exit code, so reusing the exit sentence would print "undefined".

use serde::{Deserialize, Serialize};

/// Tool name.
pub const NAME: &str = "bash";
/// `DEFAULT_TIMEOUT_MS`: two minutes.
pub const DEFAULT_TIMEOUT_MS: i64 = 2 * 60 * 1_000;
/// `MAX_TIMEOUT_MS`: ten minutes, and the cap on the input.
pub const MAX_TIMEOUT_MS: i64 = 10 * 60 * 1_000;
/// `MAX_CAPTURE_BYTES`: one mebibyte.
pub const MAX_CAPTURE_BYTES: u64 = 1024 * 1024;

/// `BashTool.Input`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    /// Capped by `MAX_TIMEOUT_MS` in the TS; validated by `validated`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i64>,
}

impl Input {
    /// `PositiveInt.check(isLessThanOrEqualTo(MAX_TIMEOUT_MS))`.
    pub fn validated(&self) -> Result<Self, &'static str> {
        match self.timeout {
            None => Ok(self.clone()),
            Some(t) if t <= 0 => Err("timeout must be a positive integer"),
            Some(t) if t > MAX_TIMEOUT_MS => Err("timeout exceeds MAX_TIMEOUT_MS"),
            Some(_) => Ok(self.clone()),
        }
    }

    /// The effective timeout: what is given, else the default.
    pub fn effective_timeout_ms(&self) -> i64 {
        self.timeout.unwrap_or(DEFAULT_TIMEOUT_MS)
    }
}

/// `BashTool.Output`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub output: String,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

/// The timeout sentence, verbatim from the TS.
pub const TIMEOUT_TEXT: &str = "Command timed out before completion.";

/// `modelOutput`: warnings first, then one sentence.
pub fn model_output(output: &Output) -> String {
    let warnings = match &output.warnings {
        Some(w) if !w.is_empty() => {
            let body: Vec<String> = w.iter().map(|x| format!("- {}", x)).collect();
            format!("Warnings:\n{}", body.join("\n"))
        }
        _ => String::new(),
    };
    // trimStart drops the newline the TS template puts before "Warnings",
    // and the separator is present only when there were warnings.
    let prefix = if warnings.is_empty() {
        String::new()
    } else {
        format!("{}\n\n", warnings)
    };
    let sentence = match output.timeout {
        Some(true) => TIMEOUT_TEXT.to_string(),
        _ => format!("Command exited with code {}.", text_of(output.exit)),
    };
    format!("{}{}", prefix, sentence)
}

/// `output.exit` is a number that may be undefined; the template prints
/// `undefined` in that case, and so does this.
fn text_of(exit: Option<i64>) -> String {
    match exit {
        Some(v) => v.to_string(),
        None => "undefined".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base() -> Output {
        Output {
            output: "hello".into(),
            truncated: false,
            exit: Some(0),
            timeout: None,
            warnings: None,
        }
    }

    #[test]
    fn a_plain_command_reports_its_exit_code() {
        assert_eq!(model_output(&base()), "Command exited with code 0.");
    }

    #[test]
    fn a_timeout_does_not_mention_an_exit_code() {
        let mut o = base();
        o.timeout = Some(true);
        assert_eq!(model_output(&o), TIMEOUT_TEXT);
    }

    #[test]
    fn warnings_come_first_then_a_blank_line() {
        let mut o = base();
        o.warnings = Some(vec!["w1".into(), "w2".into()]);
        assert_eq!(model_output(&o), "Warnings:\n- w1\n- w2\n\nCommand exited with code 0.");
    }

    #[test]
    fn an_empty_warning_list_adds_nothing() {
        let mut o = base();
        o.warnings = Some(vec![]);
        assert_eq!(model_output(&o), "Command exited with code 0.");
    }

    #[test]
    fn a_missing_exit_code_prints_undefined_as_the_ts_does() {
        let mut o = base();
        o.exit = None;
        assert_eq!(model_output(&o), "Command exited with code undefined.");
    }

    #[test]
    fn the_timeout_cap_is_enforced() {
        let ok = Input { command: "ls".into(), workdir: None, timeout: Some(MAX_TIMEOUT_MS) };
        assert!(ok.validated().is_ok());
        let over = Input { command: "ls".into(), workdir: None, timeout: Some(MAX_TIMEOUT_MS + 1) };
        assert!(over.validated().is_err());
        let zero = Input { command: "ls".into(), workdir: None, timeout: Some(0) };
        assert!(zero.validated().is_err());
        let none = Input { command: "ls".into(), workdir: None, timeout: None };
        assert_eq!(none.effective_timeout_ms(), DEFAULT_TIMEOUT_MS);
    }

    #[test]
    fn the_output_omits_absent_optionals() {
        let v = serde_json::to_value(&base()).unwrap();
        assert_eq!(v, json!({ "output": "hello", "truncated": false, "exit": 0 }));
        let mut o = base();
        o.timeout = Some(false);
        let v = serde_json::to_value(&o).unwrap();
        assert_eq!(v["timeout"], json!(false), "false is written, not dropped");
    }

    #[test]
    fn the_bounds_are_the_ones_of_the_source() {
        assert_eq!(DEFAULT_TIMEOUT_MS, 120_000);
        assert_eq!(MAX_TIMEOUT_MS, 600_000);
        assert_eq!(MAX_CAPTURE_BYTES, 1_048_576);
    }
}