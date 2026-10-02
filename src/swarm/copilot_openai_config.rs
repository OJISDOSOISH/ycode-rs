//! Port of `packages/core/src/github-copilot/responses/openai-config.ts`.
//!
//! The source is an 18-line, compile-time-only object type with six members:
//!
//! ```ts
//! export type OpenAIConfig = {
//!   provider: string
//!   url: (options: { modelId: string; path: string }) => string
//!   headers: () => Record<string, string | undefined>
//!   fetch?: FetchFunction
//!   generateId?: () => string
//!   fileIdPrefixes?: readonly string[]
//! }
//! ```
//!
//! Three of those members are functions and one is an imported function type.
//! Only two members carry data: `provider` and `fileIdPrefixes`.
//!
//! ## This config carries no credential
//!
//! There is no `apiKey`, no `baseURL` and no `organizationID` field in this
//! source, and none may be invented here. Whatever authentication exists is
//! produced by the `headers()` closure, whose body lives in another file and
//! is not part of this contract. This module therefore models the SHAPE of a
//! header record and never materialises, logs, or embeds a token; the test
//! values it builds are assembled in-test from an obviously fake construction
//! and are marked as such.
//!
//! ## Trap 1: the names are camelCase
//!
//! `generateId` is not `generateID`, not `generate_id`. `fileIdPrefixes` is not
//! `fileIDPrefixes`, not `file_id_prefixes`. The nested `url` argument object
//! carries `modelId`, again with a capital `I` and a lowercase `d`. Every one
//! of these is spelled out with an explicit `#[serde(rename = ...)]` so the
//! casing is visible in this file, and the tests both serialise the camelCase
//! form and reject the snake_case form on read.
//!
//! ## Trap 2: truthiness and nullity are different operators
//!
//! - `convert-to-openai-responses-input.ts:17` writes `if (!prefixes) return
//!   false`. That is a TRUTHINESS test on the list: only a nullish list is
//!   skipped, an empty array is truthy and reaches `.some()`, which then
//!   returns `false` anyway. `is_file_id` keeps both steps distinct. Note that
//!   an empty prefix `""` is itself truthy, and `data.startsWith("")` is true
//!   for every string, so `fileIdPrefixes: [""]` marks every id as a file id.
//! - `openai-responses-language-model.ts:596` writes
//!   `this.config.generateId?.() ?? generateId()`. Optional chaining and the
//!   coalescent both test NULLITY, so a generator that returns `""` produces
//!   `""`, never the fallback. `next_generated_id` is that function;
//!   `next_generated_id_if_truthy` is the wrong one, kept separate so the
//!   difference cannot be lost.
//!
//! ## JSON shape
//!
//! A JavaScript function has no JSON form: `JSON.stringify` drops it. The
//! serialisable surface of this type is therefore exactly `provider` and
//! `fileIdPrefixes`, and the four function members are skipped rather than
//! given an invented encoding. On the reading side, `parse_open_ai_config`
//! rejects an explicit `null` instead of silently turning it into `None`, and
//! reports an unknown or misspelled key instead of dropping it the way a plain
//! `#[serde(default)]` derive does.
//!
//! ## Character boundaries
//!
//! `data.startsWith(prefix)` compares bytes but never splits a character, so
//! multi-byte identifiers are safe. No byte-index expression such as
//! `&s[..n]` is used anywhere in this file.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The six member names of the source type, in source order.
///
/// Casing is the contract, so the spellings are frozen here rather than
/// derived from the Rust identifiers.
pub const CONFIG_FIELD_NAMES: &'static [&'static str] =
    &["provider", "url", "headers", "fetch", "generateId", "fileIdPrefixes"];

/// The members that survive `JSON.stringify`, because the others are functions.
pub const WIRE_FIELD_NAMES: &'static [&'static str] = &["provider", "fileIdPrefixes"];

/// The members that are functions, directly or through an imported type, and
/// therefore have no JSON representation at all.
pub const FUNCTION_FIELD_NAMES: &'static [&'static str] =
    &["url", "headers", "fetch", "generateId"];

/// `Record<string, string | undefined>`, the return type of `headers()`.
///
/// A `Vec` of pairs, and not a `BTreeMap`, for three reasons that all matter
/// at the exchange boundary: a JavaScript object keeps INSERTION order, which
/// is observable when headers are serialised; a map keyed by `String` cannot
/// express the `undefined` value, which is a legal value of this record and is
/// not the same state as an absent key; and `undefined` must not be confused
/// with `""`, which is a present key with an empty value.
pub type OpenAiConfigHeaders = Vec<(String, Option<String>)>;

/// Placeholder for `FetchFunction`, imported from `@ai-sdk/provider-utils`.
///
/// The signature is declared in another package and is deliberately not
/// redeclared here: this file only asserts that the member is OPTIONAL. The
/// placeholder is a unit struct, so it cannot be mistaken for an
/// implementation and cannot smuggle in invented behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct OpaqueFetchFunction;

impl OpaqueFetchFunction {
    /// Where the real signature lives.
    pub const SOURCE: &'static str = "FetchFunction (@ai-sdk/provider-utils)";

    /// The imported type this placeholder stands for.
    pub fn name(&self) -> &'static str {
        Self::SOURCE
    }
}

/// The argument of `url`: `{ modelId: string; path: string }`.
///
/// `modelId` is the trap-1 spelling of this family. `deny_unknown_fields`
/// turns a snake_case slip into a read error instead of a silent loss; the
/// cost is that the type is marginally stricter than TypeScript for values
/// that are not object literals, which is an acceptable trade at this boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenAiConfigUrlOptions {
    /// `modelId`, capital `I` and lowercase `d`.
    #[serde(rename = "modelId")]
    pub model_id: String,

    /// `path`. The consumer passes `"/responses"` here.
    pub path: String,
}

impl OpenAiConfigUrlOptions {
    /// Builds the argument object.
    pub fn new(model_id: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            model_id: model_id.into(),
            path: path.into(),
        }
    }
}

/// The port of `OpenAIConfig`.
///
/// `provider` is required and is a plain string: `""` is a valid value and is
/// never filtered. `fetch`, `generateId` and `fileIdPrefixes` are optional, and
/// absence is `None`, never an empty value.
///
/// The four function members are held as closures and are skipped on
/// serialisation, exactly as `JSON.stringify` skips them. That is why this
/// type derives `Serialize` but not `Deserialize`: a function cannot be
/// rebuilt from JSON, so the reading path is `parse_open_ai_config`, which
/// takes the data members from a `Value` and the behaviours from the caller.
#[derive(Serialize)]
pub struct OpenAiConfig {
    /// `provider: string`. Returned verbatim by
    /// `openai-responses-language-model.ts:149`.
    #[serde(rename = "provider")]
    pub provider: String,

    /// `url: (options: { modelId: string; path: string }) => string`. Required.
    #[serde(rename = "url", skip_serializing)]
    pub url: Box<dyn Fn(&OpenAiConfigUrlOptions) -> String>,

    /// `headers: () => Record<string, string | undefined>`. Required.
    #[serde(rename = "headers", skip_serializing)]
    pub headers: Box<dyn Fn() -> OpenAiConfigHeaders>,

    /// `fetch?: FetchFunction`. Optional, and see `OpaqueFetchFunction`.
    #[serde(rename = "fetch", skip_serializing)]
    pub fetch: Option<OpaqueFetchFunction>,

    /// `generateId?: () => string`. Optional.
    #[serde(rename = "generateId", skip_serializing)]
    pub generate_id: Option<Box<dyn Fn() -> String>>,

    /// `fileIdPrefixes?: readonly string[]`. Optional. When absent, all file
    /// data is treated as base64 content.
    #[serde(rename = "fileIdPrefixes", skip_serializing_if = "Option::is_none")]
    pub file_id_prefixes: Option<Vec<String>>,
}

impl OpenAiConfig {
    /// Builds the three required members.
    pub fn new(
        provider: impl Into<String>,
        url: Box<dyn Fn(&OpenAiConfigUrlOptions) -> String>,
        headers: Box<dyn Fn() -> OpenAiConfigHeaders>,
    ) -> Self {
        Self {
            provider: provider.into(),
            url,
            headers,
            fetch: None,
            generate_id: None,
            file_id_prefixes: None,
        }
    }

    /// Sets the optional `generateId`.
    pub fn with_generate_id(mut self, generate_id: Box<dyn Fn() -> String>) -> Self {
        self.generate_id = Some(generate_id);
        self
    }

    /// Sets the optional `fetch`.
    pub fn with_fetch(mut self, fetch: OpaqueFetchFunction) -> Self {
        self.fetch = Some(fetch);
        self
    }

    /// Sets the optional `fileIdPrefixes`.
    pub fn with_file_id_prefixes<S, I>(mut self, prefixes: I) -> Self
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        self.file_id_prefixes = Some(prefixes.into_iter().map(Into::into).collect());
        self
    }

    /// Calls `url(...)`.
    pub fn build_url(&self, model_id: &str, path: &str) -> String {
        (self.url)(&OpenAiConfigUrlOptions::new(model_id, path))
    }

    /// Calls `headers()`.
    pub fn build_headers(&self) -> OpenAiConfigHeaders {
        (self.headers)()
    }
}

impl fmt::Debug for OpenAiConfig {
    /// Prints the member names and the data members only.
    ///
    /// The two closures are described by their signature and never called, so
    /// this implementation cannot leak a header value into a log.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiConfig")
            .field("provider", &self.provider)
            .field("url", &"<fn(&OpenAiConfigUrlOptions) -> String>")
            .field("headers", &"<fn() -> OpenAiConfigHeaders>")
            .field("fetch", &self.fetch)
            .field(
                "generateId",
                &self.generate_id.as_ref().map(|_| "<fn() -> String>"),
            )
            .field("fileIdPrefixes", &self.file_id_prefixes)
            .finish()
    }
}

/// Why a `Value` is not an `OpenAIConfig`.
///
/// The source validates nothing, so these variants are PORTING guards, not
/// runtime validation of a provider response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAiConfigError {
    /// The payload root is not a JSON object.
    RootNotAnObject,

    /// A key is not one of the six member names.
    UnknownKey {
        /// The key as written in the JSON.
        received: String,
        /// The accepted names, in source order.
        known: &'static [&'static str],
    },

    /// A required member is absent. Only `provider` can hit this.
    MissingRequiredField {
        /// The missing member name.
        field: &'static str,
    },

    /// The member is present and explicitly `null`. Neither `null` nor `""` is
    /// allowed by the TypeScript types, so this is not read as an absence.
    ExplicitNull {
        /// The offending member name.
        field: &'static str,
    },

    /// The member is present with a value of the wrong JSON kind.
    FieldHasWrongType {
        /// The offending member name.
        field: &'static str,
        /// The TypeScript type, as a human description.
        expected: &'static str,
    },

    /// A function member was found in JSON. No JSON value can be a function,
    /// so `JSON.stringify` could never have produced it.
    FunctionFieldInJson {
        /// The offending member name.
        field: &'static str,
    },
}

impl OpenAiConfigError {
    /// The path of the offending member, empty string at the root.
    pub fn path(&self) -> &str {
        match self {
            OpenAiConfigError::RootNotAnObject => "",
            OpenAiConfigError::UnknownKey { received, .. } => received.as_str(),
            OpenAiConfigError::MissingRequiredField { field }
            | OpenAiConfigError::ExplicitNull { field }
            | OpenAiConfigError::FieldHasWrongType { field, .. }
            | OpenAiConfigError::FunctionFieldInJson { field } => field,
        }
    }
}

/// Reads the data members of an `OpenAIConfig` out of a `Value`.
///
/// The behaviours cannot come from JSON, so the caller injects `url` and
/// `headers`; `fetch` and `generateId` come back as `None` because a function
/// member is simply not there. Unknown keys are rejected instead of dropped,
/// which is the only way a `file_id_prefixes` slip becomes visible: a plain
/// `#[serde(default)]` derive accepts that key and throws the data away.
pub fn parse_open_ai_config(
    value: &Value,
    url: Box<dyn Fn(&OpenAiConfigUrlOptions) -> String>,
    headers: Box<dyn Fn() -> OpenAiConfigHeaders>,
) -> Result<OpenAiConfig, OpenAiConfigError> {
    let object = value
        .as_object()
        .ok_or(OpenAiConfigError::RootNotAnObject)?;

    for key in object.keys() {
        let known = CONFIG_FIELD_NAMES
            .iter()
            .any(|member| *member == key.as_str());
        if !known {
            return Err(OpenAiConfigError::UnknownKey {
                received: key.clone(),
                known: CONFIG_FIELD_NAMES,
            });
        }
    }

    for function_field in FUNCTION_FIELD_NAMES {
        if let Some(found) = object.get(*function_field) {
            return Err(if found.is_null() {
                OpenAiConfigError::ExplicitNull {
                    field: function_field,
                }
            } else {
                OpenAiConfigError::FunctionFieldInJson {
                    field: function_field,
                }
            });
        }
    }

    let provider = match object.get("provider") {
        None => {
            return Err(OpenAiConfigError::MissingRequiredField {
                field: "provider",
            })
        }
        Some(Value::String(provider)) => provider.clone(),
        Some(Value::Null) => {
            return Err(OpenAiConfigError::ExplicitNull {
                field: "provider",
            })
        }
        Some(_) => {
            return Err(OpenAiConfigError::FieldHasWrongType {
                field: "provider",
                expected: "string",
            })
        }
    };

    let file_id_prefixes = match object.get("fileIdPrefixes") {
        None => None,
        Some(Value::Null) => {
            return Err(OpenAiConfigError::ExplicitNull {
                field: "fileIdPrefixes",
            })
        }
        Some(Value::Array(items)) => {
            let mut prefixes = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::String(prefix) => prefixes.push(prefix.clone()),
                    _ => {
                        return Err(OpenAiConfigError::FieldHasWrongType {
                            field: "fileIdPrefixes",
                            expected: "array of strings",
                        })
                    }
                }
            }
            Some(prefixes)
        }
        Some(_) => {
            return Err(OpenAiConfigError::FieldHasWrongType {
                field: "fileIdPrefixes",
                expected: "array of strings",
            })
        }
    };

    Ok(OpenAiConfig {
        provider,
        url,
        headers,
        fetch: None,
        generate_id: None,
        file_id_prefixes,
    })
}

/// Looks up a header, keeping "absent" and "present but undefined" apart.
///
/// Mirrors `headers[name]`: `None` means the key is not in the record, and
/// `Some(None)` means the key is there with the `undefined` value that
/// `Record<string, string | undefined>` allows. Dropping `undefined` values is
/// the job of `combineHeaders` in the consumer, not of this file.
pub fn header_value<'a>(headers: &'a OpenAiConfigHeaders, name: &str) -> Option<&'a Option<String>> {
    headers
        .iter()
        .find(|(key, _)| key.as_str() == name)
        .map(|(_, value)| value)
}

/// `isFileId(data, prefixes)` from `convert-to-openai-responses-input.ts:16`.
///
/// `if (!prefixes) return false` is a TRUTHINESS test on the list, so only a
/// nullish list is short-circuited; an empty list is truthy and reaches
/// `.some()`, which is false anyway. Both steps are kept because merging them
/// would hide the difference on the next edit.
///
/// `startsWith` never splits a character, so a multi-byte prefix or
/// identifier is safe here. An empty prefix is truthy in JavaScript and
/// `data.startsWith("")` is true for every string, including `""`.
pub fn is_file_id(data: &str, prefixes: Option<&[String]>) -> bool {
    match prefixes {
        None => false,
        Some(prefixes) => prefixes.iter().any(|prefix| data.starts_with(prefix.as_str())),
    }
}

/// `this.config.generateId?.() ?? generateId()`.
///
/// Both operators test NULLITY, so a generator returning `""` yields `""` and
/// never reaches the fallback. The fallback is supplied by the caller because
/// `generateId()` itself comes from `@ai-sdk/provider-utils` and is not
/// redeclared here.
pub fn next_generated_id<F>(config: &OpenAiConfig, fallback: F) -> String
where
    F: FnOnce() -> String,
{
    match &config.generate_id {
        Some(generate) => generate(),
        None => fallback(),
    }
}

/// What a port by TRUTHINESS would produce, never called by the model.
///
/// Kept as a separate function so the two cannot be confused: here an empty
/// generator result is falsy and falls back, which is exactly the bug that the
/// coalescent form above avoids.
pub fn next_generated_id_if_truthy<F>(config: &OpenAiConfig, fallback: F) -> String
where
    F: FnOnce() -> String,
{
    match &config.generate_id {
        Some(generate) => {
            let id = generate();
            if id.is_empty() {
                fallback()
            } else {
                id
            }
        }
        None => fallback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_url(options: &OpenAiConfigUrlOptions) -> String {
        // `example.invalid` is a reserved name and nothing here opens a socket.
        format!(
            "https://example.invalid{}?modelId={}",
            options.path, options.model_id
        )
    }

    fn empty_headers() -> OpenAiConfigHeaders {
        Vec::new()
    }

    fn sample_config() -> OpenAiConfig {
        OpenAiConfig::new("copilot", Box::new(sample_url), Box::new(empty_headers))
            .with_file_id_prefixes(["file-"])
    }

    #[test]
    fn member_names_are_exactly_the_source_spelling() {
        assert_eq!(
            CONFIG_FIELD_NAMES,
            ["provider", "url", "headers", "fetch", "generateId", "fileIdPrefixes"].as_slice()
        );
        let has = |name: &str| CONFIG_FIELD_NAMES.iter().any(|member| *member == name);
        assert!(has("generateId"));
        assert!(!has("generateID"));
        assert!(!has("generate_id"));
        assert!(has("fileIdPrefixes"));
        assert!(!has("fileIDPrefixes"));
        assert!(!has("file_id_prefixes"));
        assert_eq!(WIRE_FIELD_NAMES, ["provider", "fileIdPrefixes"].as_slice());
        assert_eq!(
            FUNCTION_FIELD_NAMES,
            ["url", "headers", "fetch", "generateId"].as_slice()
        );
        for member in WIRE_FIELD_NAMES {
            assert!(
                !FUNCTION_FIELD_NAMES.contains(member),
                "a member cannot be both data and a function"
            );
        }
    }

    #[test]
    fn the_wire_form_keeps_camel_case_and_drops_function_members() {
        let value = serde_json::to_value(sample_config()).unwrap();
        let object = value.as_object().unwrap();
        assert_eq!(object.len(), WIRE_FIELD_NAMES.len());
        assert!(object.contains_key("provider"));
        assert!(object.contains_key("fileIdPrefixes"));
        for function_member in FUNCTION_FIELD_NAMES {
            assert!(
                !object.contains_key(*function_member),
                "a function has no JSON form, like JSON.stringify drops it"
            );
        }
        assert!(!object.contains_key("file_id_prefixes"));
        assert!(!object.contains_key("generate_id"));
        assert_eq!(object["provider"], json!("copilot"));
        assert_eq!(object["fileIdPrefixes"], json!(["file-"]));
    }

    #[test]
    fn url_options_serialise_with_model_id_casing() {
        let options = OpenAiConfigUrlOptions::new("gpt-5", "/responses");
        let value = serde_json::to_value(&options).unwrap();
        assert_eq!(
            value,
            json!({ "modelId": "gpt-5", "path": "/responses" })
        );
        let text = serde_json::to_string(&options).unwrap();
        assert!(text.contains("\"modelId\""), "got {}", text);
        assert!(!text.contains("model_id"), "got {}", text);
    }

    #[test]
    fn url_options_reject_the_snake_case_form_on_read() {
        let error = serde_json::from_str::<OpenAiConfigUrlOptions>(
            r#"{"model_id":"gpt-5","path":"/responses"}"#,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("modelId"),
            "the error must name the expected spelling, got {}",
            error
        );
        let round_trip: OpenAiConfigUrlOptions =
            serde_json::from_str(r#"{"modelId":"gpt-5","path":"/responses"}"#).unwrap();
        assert_eq!(round_trip.model_id, "gpt-5");
        assert_eq!(round_trip.path, "/responses");
    }

    #[test]
    fn the_reader_accepts_the_exact_camel_case_key() {
        let parsed = parse_open_ai_config(
            &json!({ "provider": "copilot", "fileIdPrefixes": ["file-", "assistant-"] }),
            Box::new(sample_url),
            Box::new(empty_headers),
        )
        .unwrap();
        assert_eq!(parsed.provider, "copilot");
        assert_eq!(
            parsed.file_id_prefixes,
            Some(vec![String::from("file-"), String::from("assistant-")])
        );
        assert_eq!(
            parsed.build_url("gpt-5", "/responses"),
            "https://example.invalid/responses?modelId=gpt-5"
        );
    }

    #[test]
    fn the_reader_rejects_the_snake_case_form() {
        for (payload, wrong_key) in [
            (
                json!({ "provider": "copilot", "file_id_prefixes": ["file-"] }),
                "file_id_prefixes",
            ),
            (
                json!({ "provider": "copilot", "file_id_prefix": ["file-"] }),
                "file_id_prefix",
            ),
            (
                json!({ "provider": "copilot", "fileIDPrefixes": ["file-"] }),
                "fileIDPrefixes",
            ),
            (
                json!({ "provider": "copilot", "generate_id": "x" }),
                "generate_id",
            ),
        ] {
            let error =
                parse_open_ai_config(&payload, Box::new(sample_url), Box::new(empty_headers))
                    .unwrap_err();
            assert_eq!(
                error,
                OpenAiConfigError::UnknownKey {
                    received: String::from(wrong_key),
                    known: CONFIG_FIELD_NAMES,
                },
                "the snake_case form must be rejected on read, not dropped"
            );
            assert_eq!(error.path(), wrong_key);
        }
    }

    #[test]
    fn the_reader_reports_a_function_member_found_in_json() {
        for function_member in FUNCTION_FIELD_NAMES {
            let mut payload = json!({ "provider": "copilot" });
            payload[*function_member] = json!("not a function");
            let error = parse_open_ai_config(
                &payload,
                Box::new(sample_url),
                Box::new(empty_headers),
            )
            .unwrap_err();
            assert_eq!(
                error,
                OpenAiConfigError::FunctionFieldInJson {
                    field: *function_member
                }
            );
            assert_eq!(error.path(), *function_member);
        }
    }

    #[test]
    fn an_explicit_null_is_rejected_and_not_read_as_an_absence() {
        // A plain `Option` would read this as `None` in silence.
        for payload in [
            json!({ "provider": null }),
            json!({ "provider": "copilot", "fileIdPrefixes": null }),
        ] {
            let error = parse_open_ai_config(
                &payload,
                Box::new(sample_url),
                Box::new(empty_headers),
            )
            .unwrap_err();
            assert!(matches!(error, OpenAiConfigError::ExplicitNull { .. }));
        }
        assert_eq!(
            parse_open_ai_config(
                &json!({ "provider": 7 }),
                Box::new(sample_url),
                Box::new(empty_headers),
            )
            .unwrap_err(),
            OpenAiConfigError::FieldHasWrongType {
                field: "provider",
                expected: "string",
            }
        );
        assert_eq!(
            parse_open_ai_config(
                &json!({ "fileIdPrefixes": ["file-"] }),
                Box::new(sample_url),
                Box::new(empty_headers),
            )
            .unwrap_err(),
            OpenAiConfigError::MissingRequiredField {
                field: "provider"
            }
        );
        assert_eq!(
            parse_open_ai_config(
                &json!({ "provider": "copilot", "fileIdPrefixes": ["file-", 3] }),
                Box::new(sample_url),
                Box::new(empty_headers),
            )
            .unwrap_err(),
            OpenAiConfigError::FieldHasWrongType {
                field: "fileIdPrefixes",
                expected: "array of strings",
            }
        );
        assert_eq!(
            parse_open_ai_config(
                &json!([]),
                Box::new(sample_url),
                Box::new(empty_headers),
            )
            .unwrap_err(),
            OpenAiConfigError::RootNotAnObject
        );
    }

    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct NaiveConfigProbe {
        provider: Option<String>,
        #[serde(rename = "fileIdPrefixes")]
        file_id_prefixes: Option<Vec<String>>,
    }

    #[test]
    fn a_naive_derive_silently_loses_both_the_misspelled_key_and_the_null() {
        // This is why the strict reader exists. Nothing below fails, and the
        // data is simply gone.
        let missed: NaiveConfigProbe =
            serde_json::from_str(r#"{"provider":"copilot","file_id_prefixes":["file-"]}"#)
                .unwrap();
        assert_eq!(missed.provider.as_deref(), Some("copilot"));
        assert_eq!(missed.file_id_prefixes, None);

        let nulled: NaiveConfigProbe =
            serde_json::from_str(r#"{"provider":"copilot","fileIdPrefixes":null}"#).unwrap();
        assert_eq!(nulled.file_id_prefixes, None);

        let correct: NaiveConfigProbe =
            serde_json::from_str(r#"{"provider":"copilot","fileIdPrefixes":["file-"]}"#).unwrap();
        assert_eq!(
            correct.file_id_prefixes,
            Some(vec![String::from("file-")]),
            "the correct spelling is the only one that carries data"
        );
    }

    #[test]
    fn provider_is_a_string_and_the_empty_string_survives() {
        let parsed = parse_open_ai_config(
            &json!({ "provider": "" }),
            Box::new(sample_url),
            Box::new(empty_headers),
        )
        .unwrap();
        assert_eq!(parsed.provider, "");
        // `provider: string` has no truthiness filter: the empty string is the
        // value that reaches the model, so it arrives EMPTY rather than absent.
        assert!(parsed.provider.is_empty(), "still a value, not an absence");
        let value = serde_json::to_value(parsed).unwrap();
        assert_eq!(value, json!({ "provider": "" }));
    }

    #[test]
    fn an_absent_prefix_list_and_an_empty_one_stay_distinct() {
        let absent: Option<Vec<String>> = None;
        let empty: Option<Vec<String>> = Some(Vec::new());
        // `skip_serializing_if` is a FIELD attribute: it fires when the struct
        // is serialised, never when a bare `Option` is, and a bare `None`
        // serialises to `null` by definition. The absent/empty distinction is
        // therefore read off `OpenAiConfig`, where the field actually lives.
        let sans_prefixes =
            OpenAiConfig::new("copilot", Box::new(sample_url), Box::new(empty_headers));
        let avec_prefixes =
            OpenAiConfig::new("copilot", Box::new(sample_url), Box::new(empty_headers))
                .with_file_id_prefixes(Vec::<String>::new());
        let json_absent = serde_json::to_value(&sans_prefixes).unwrap();
        let json_vide = serde_json::to_value(&avec_prefixes).unwrap();
        assert!(
            json_absent.get("fileIdPrefixes").is_none(),
            "None is skipped, it does not become a null on the wire"
        );
        assert_eq!(json_vide["fileIdPrefixes"], json!([]));

        assert!(!is_file_id("file-abc", absent.as_deref()));
        assert!(!is_file_id("file-abc", empty.as_deref()));
        assert!(is_file_id("file-abc", Some(&[String::from("file-")])));

        let parsed = parse_open_ai_config(
            &json!({ "provider": "copilot", "fileIdPrefixes": [] }),
            Box::new(sample_url),
            Box::new(empty_headers),
        )
        .unwrap();
        assert_eq!(parsed.file_id_prefixes, Some(Vec::new()));
        assert_ne!(parsed.file_id_prefixes, None);
    }

    #[test]
    fn an_empty_prefix_matches_every_id() {
        // `""` is truthy in JavaScript and `"anything".startsWith("")` is true,
        // so a filter that dropped falsy prefixes would answer the opposite.
        let prefixes = vec![String::new()];
        assert!(is_file_id("file-abc", Some(&prefixes)));
        assert!(is_file_id("", Some(&prefixes)));
        assert!(is_file_id("é", Some(&prefixes)));
        let filtered_out = vec![String::new(), String::from("file-")];
        assert!(is_file_id("assistant-1", Some(&filtered_out)));
        assert!(!is_file_id("assistant-1", Some(&[String::from("file-")])));
    }

    #[test]
    fn prefix_matching_never_splits_a_multi_byte_character() {
        // `startsWith` compares bytes without cutting a character in half; a
        // byte-index expression such as `&data[..1]` would panic here.
        let data = "fichier-é-1";
        assert!(is_file_id(data, Some(&[String::from("fichier-é")])));
        assert!(is_file_id(data, Some(&[String::from("fichier")])));
        // "fichie" is the first six CHARACTERS of the identifier and also its
        // first six BYTES, so `data.startsWith("fichie")` is true. A prefix
        // that would split a character cannot be spelled in a Rust `String`
        // at all; the negative cases below are prefixes that differ instead.
        assert!(is_file_id(data, Some(&[String::from("fichie")])));
        assert!(!is_file_id(data, Some(&[String::from("fichier-é-2")])));

        let han = "\u{6587}\u{4ef6}-abc";
        assert!(is_file_id(han, Some(&[String::from("\u{6587}\u{4ef6}")])));
        assert!(!is_file_id(han, Some(&[String::from("\u{6587}")])));

        // The prefix is longer in bytes than the shorter candidate and is still
        // compared without a boundary check of our own.
        assert!(han.len() > han.chars().count());
        assert!(is_file_id(han, Some(&[String::from(han)])));
    }

    #[test]
    fn an_optional_generate_id_keeps_an_empty_result() {
        // `this.config.generateId?.() ?? generateId()`: `""` is not nullish.
        let config = OpenAiConfig::new("copilot", Box::new(sample_url), Box::new(empty_headers))
            .with_generate_id(Box::new(|| String::new()));
        assert_eq!(
            next_generated_id(&config, || String::from("fallback")),
            ""
        );
        let real = OpenAiConfig::new("copilot", Box::new(sample_url), Box::new(empty_headers))
            .with_generate_id(Box::new(|| String::from("id_abc123")));
        assert_eq!(
            next_generated_id(&real, || String::from("fallback")),
            "id_abc123"
        );
    }

    #[test]
    fn the_truthiness_variant_is_a_separate_function_that_drops_it() {
        let config = OpenAiConfig::new("copilot", Box::new(sample_url), Box::new(empty_headers))
            .with_generate_id(Box::new(|| String::new()));
        assert_eq!(
            next_generated_id_if_truthy(&config, || String::from("fallback")),
            "fallback",
            "this is the wrong port and is never called by the model"
        );
        assert_ne!(
            next_generated_id(&config, || String::from("fallback")),
            next_generated_id_if_truthy(&config, || String::from("fallback")),
            "the two functions must not be merged"
        );
    }

    #[test]
    fn an_absent_generate_id_falls_back_once() {
        let config = sample_config();
        assert!(config.generate_id.is_none());
        assert_eq!(
            next_generated_id(&config, || String::from("fallback")),
            "fallback"
        );
        assert_eq!(
            next_generated_id_if_truthy(&config, || String::from("fallback")),
            "fallback"
        );
    }

    #[test]
    fn headers_keep_undefined_apart_from_an_empty_value_and_keep_their_order() {
        // Obviously fake credential SHAPE, assembled in this test from a
        // non-secret construction. No real or plausible token exists here, and
        // no module constant of this file stores any.
        let fake_token = ["Bearer", "not-a-real-token", "built-for-this-test-only"].join(" ");
        let headers: OpenAiConfigHeaders = vec![
            (String::from("X-Fixture-Token"), Some(fake_token.clone())),
            (String::from("X-Empty"), Some(String::new())),
            (String::from("X-Undefined"), None),
        ];
        let config = OpenAiConfig::new("copilot", Box::new(sample_url), Box::new(move || {
            headers.clone()
        }));

        let built = config.build_headers();
        let keys: Vec<&str> = built.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["X-Fixture-Token", "X-Empty", "X-Undefined"],
            "a JavaScript object keeps insertion order"
        );

        let empty = header_value(&built, "X-Empty").expect("key is present");
        assert_eq!(empty.as_deref(), Some(""));
        let undefined = header_value(&built, "X-Undefined").expect("key is present");
        assert!(undefined.is_none(), "`undefined` is a value, not an absence");
        assert_ne!(
            header_value(&built, "X-Empty"),
            header_value(&built, "X-Undefined"),
            "an empty value and undefined are two different states"
        );
        assert_eq!(header_value(&built, "X-Missing"), None);
    }

    #[test]
    fn debug_output_cannot_leak_a_header_value() {
        let fake_token = ["Bearer", "not-a-real-token", "built-for-this-test-only"].join(" ");
        let token_for_closure = fake_token.clone();
        let config =
            OpenAiConfig::new("copilot", Box::new(sample_url), Box::new(move || {
                vec![(String::from("Authorization"), Some(token_for_closure.clone()))]
            }))
            .with_fetch(OpaqueFetchFunction)
            .with_generate_id(Box::new(|| String::from("id_abc123")));
        let rendered = format!("{:?}", config);
        assert!(!rendered.contains("not-a-real-token"), "got {}", rendered);
        assert!(rendered.contains("provider"));
        assert!(rendered.contains("fileIdPrefixes"));
        assert_eq!(OpaqueFetchFunction.name(), OpaqueFetchFunction::SOURCE);
        assert_eq!(config.fetch, Some(OpaqueFetchFunction));
    }

    #[test]
    fn functions_do_not_survive_a_json_round_trip() {
        let config = sample_config()
            .with_fetch(OpaqueFetchFunction)
            .with_generate_id(Box::new(|| String::from("id_abc123")));
        let text = serde_json::to_string(&config).unwrap();
        assert!(!text.contains("generateId"));
        assert!(!text.contains("\"url\""));
        assert!(!text.contains("headers"));
        assert!(!text.contains("fetch"));

        let parsed = parse_open_ai_config(
            &serde_json::from_str::<Value>(&text).unwrap(),
            Box::new(sample_url),
            Box::new(empty_headers),
        )
        .unwrap();
        assert_eq!(parsed.provider, "copilot");
        assert_eq!(
            parsed.file_id_prefixes,
            Some(vec![String::from("file-")]),
            "the two data members come back"
        );
        assert!(parsed.fetch.is_none(), "a function cannot be rebuilt");
        assert!(parsed.generate_id.is_none(), "a function cannot be rebuilt");
    }
}
