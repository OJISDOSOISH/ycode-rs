//! Port of `github-copilot/chat/openai-compatible-metadata-extractor.ts`.
//!
//! ## What the source actually is
//!
//! Forty-four lines, **zero functions**, zero fields of data. The file exports
//! exactly one thing, `export type MetadataExtractor`, and a TypeScript type is
//! erased at runtime: no `MetadataExtractor` value ever exists, and nothing in
//! this repository ever implements one. A repo-wide search for
//! `createStreamExtractor` returns the declaration itself and the single
//! `config.metadataExtractor?.` call site in
//! `openai-compatible-chat-language-model.ts:316`; a search for
//! `metadataExtractor` finds no producer, so the field is always `undefined` at
//! runtime and both call sites are no-ops today.
//!
//! That is why the behaviour worth porting is **not** an algorithm but a
//! contract, and why the whole risk of a port lives in the *shape* rather than
//! in the maths:
//!
//! ```text
//! extractMetadata: ({ parsedBody }: { parsedBody: unknown })
//!   => Promise<SharedV3ProviderMetadata | undefined>
//!
//! createStreamExtractor: () => {
//!   processChunk(parsedChunk: unknown): void
//!   buildMetadata(): SharedV3ProviderMetadata | undefined
//! }
//! ```
//!
//! Four facts in that block drive every decision below.
//!
//! ## Fact 1: a Rust trait carries no names, so the file has two layers
//!
//! `MetadataExtractor` and `StreamMetadataExtractor` below are the behavioural
//! contract. A trait is invisible at runtime, which means a mistyped member
//! name is invisible too: renaming `processChunk` to `process_chunk` compiles,
//! passes every behavioural test, and breaks the only consumer that reaches an
//! extractor **by reflection over the config object**. So the spelling of the
//! four members and of the two parameter names is data, held in
//! `MetadataExtractorDescriptor`, serialised with the exact TypeScript
//! spellings and read back with `deny_unknown_fields` so the snake_case form
//! is an error rather than a silent `None`.
//!
//! ## Fact 2: the input is `unknown`, and that is a guarantee of *nothing*
//!
//! `parsedBody: unknown` is not a type to validate against, it is the absence
//! of a type. The extractor therefore cannot be written to fail: whatever JSON
//! arrives, it must produce a metadata map or `undefined`. This is the single
//! most important difference from a sibling port in this family
//! (`copilot_response_metadata.rs`), whose input is a `z.object` and whose
//! parser legitimately returns `Err` on a wrong type. Here the only total
//! functions are the two extractors; `ExtractionDiagnosis` is a
//! **porting-control** surface with no counterpart in the source, and it never
//! disagrees with the extraction because both come from the same walk.
//!
//! ## Fact 3: `undefined` and `{}` are the same thing to the only caller
//!
//! Both call sites spread the result into an object literal:
//!
//! ```ts
//! const providerMetadata: SharedV3ProviderMetadata = {
//!   [this.providerOptionsName]: {},
//!   ...(await this.config.metadataExtractor?.extractMetadata?.({ parsedBody: rawResponse })),
//! }
//! ```
//!
//! `{ ...undefined }` is `{}` in JavaScript, and `{ ...{} }` is `{}`. So
//! `None` and `Some(empty)` are observationally identical at every site that
//! exists. The port keeps the `Option` because the declared return type has
//! it, and `merge_extracted_metadata` proves the two forms collapse, so nobody
//! later "fixes" a non-empty-versus-empty difference that cannot be observed.
//!
//! The two optional calls also matter: `metadataExtractor?.extractMetadata?.`.
//! An absent extractor, an extractor with a missing member, and an extractor
//! that returns `undefined` are three different `None`s the caller cannot tell
//! apart, and all three are modelled as one.
//!
//! ## Fact 4: the real filtering in this family is a **truthiness** test
//!
//! The concrete extractors that satisfy this interface are not in the source
//! file, but the only code in the repository that builds `{ copilot: { ... } }`
//! metadata is the chat model's caller, and it reads
//! `choice.message.reasoning_opaque` with `?` and not `??`:
//!
//! ```ts
//! providerMetadata: choice.message.reasoning_opaque
//!   ? { copilot: { reasoningOpaque: choice.message.reasoning_opaque } }
//!   : undefined
//! ```
//!
//! `?` is **truthiness**: `""`, `0` and `false` are dropped. `??` is
//! **nullity**: only `null` and `undefined` are dropped, and `""` survives.
//! `truthiness_filter` is the production path and `nullity_filter` is its
//! comparison twin, never called by production code, so the difference is
//! locked by a test instead of described in a comment. Both are tested on the
//! three falsy JavaScript values and on the two **truthy** empty values, `{}`
//! and `[]`, which is the case a Rust port gets wrong by reaching for
//! `is_empty()`.
//!
//! The two paths read different inner objects, and both spellings are real:
//! the non-streaming body has `choices[0].message`, the streamed chunk has
//! `choices[0].delta` (`openai-compatible-chat-language-model.ts:788`), and
//! `delta` is itself `.nullish()`. `InnerHop` is what keeps one walk shared by
//! both without blurring which path reported a missing inner object.
//!
//! ## The casing trap, spelled out
//!
//! The same field changes spelling across the boundary, and that is the trap
//! this file exists to lock:
//!
//! | position | spelling | style |
//! |---|---|---|
//! | read from the body | `reasoning_opaque` | snake_case |
//! | written to the metadata | `reasoningOpaque` | camelCase |
//! | argument of `extractMetadata` | `parsedBody` | camelCase |
//! | argument of `processChunk` | `parsedChunk` | camelCase |
//! | members | `extractMetadata`, `createStreamExtractor`, `processChunk`, `buildMetadata` | camelCase |
//!
//! None of it is visible at compile time. A port that reads `reasoningOpaque`
//! from the body silently extracts nothing, and a port that writes
//! `reasoning_opaque` into the metadata silently breaks the consumer. Both
//! directions are locked: `reasoning_opaque` and `reasoningOpaque` are the only
//! spellings that serialise, and `reasoning_opaque` is **rejected** when the
//! metadata record is read back.
//!
//! The same applies to the members. `extract_metadata` is not a
//! `MetadataExtractor` key, and it is rejected on read.
//!
//! ## One key, two names, both real
//!
//! The metadata is written under the literal key `copilot`, while the
//! always-present bucket uses `providerOptionsName`, which is
//! `config.provider.split(".")[0].trim()` and is built from a provider string
//! of the shape `${name}.chat`. For `name = "copilot"` the two keys are
//! `"copilot"` and `"copilot"`, and it is tempting to collapse them. They are
//! kept apart: the bucket is derived from configuration, the lifted field is a
//! constant of the wire format, and `".chat"` shows the derivation is not even
//! guaranteed to be non-empty. `provider_options_name` never panics because
//! `split` always yields at least one element, exactly as in JavaScript.
//!
//! ## Open question, deliberately not answered
//!
//! Two agents reviewing this family reached **opposite** conclusions about
//! whether a schema `optional` field accepts an explicit JSON `null`, and
//! neither could settle it because `effect` is not installed on this machine.
//! The question is not answered here either. What this file does is pin the
//! behaviour of *this* port so the answer cannot drift silently:
//! `classify_extraction` distinguishes `FieldAbsent` from `FieldNull`, and the
//! extractor produces the same `None` for both, which is what `?` requires
//! whatever the schema says. The test
//! `an_absent_field_and_an_explicit_null_extract_alike_but_classify_differently`
//! is the lock. If the question is later settled, only the diagnosis arm needs
//! revisiting, and the test says so.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Names, spelled exactly as the source spells them
// ---------------------------------------------------------------------------

/// `extractMetadata`, the first member of the `MetadataExtractor` type.
///
/// A trait method cannot carry this name, so the spelling lives here as data
/// and is checked by `the_member_names_are_camel_case_and_not_snake_case`.
pub const MEMBER_EXTRACT_METADATA: &str = "extractMetadata";

/// `createStreamExtractor`, the second member of the `MetadataExtractor` type.
pub const MEMBER_CREATE_STREAM_EXTRACTOR: &str = "createStreamExtractor";

/// `processChunk`, the first member of the stream extractor.
pub const MEMBER_PROCESS_CHUNK: &str = "processChunk";

/// `buildMetadata`, the second member of the stream extractor.
pub const MEMBER_BUILD_METADATA: &str = "buildMetadata";

/// The named parameter of the argument object: `parsedBody`.
pub const PARAM_PARSED_BODY: &str = "parsedBody";

/// The only parameter of `processChunk`: `parsedChunk`.
pub const PARAM_PARSED_CHUNK: &str = "parsedChunk";

/// The four members, in the order the type declares them.
pub const MEMBERS_METADATA_EXTRACTOR: [&str; 4] = [
    MEMBER_EXTRACT_METADATA,
    MEMBER_CREATE_STREAM_EXTRACTOR,
    MEMBER_PROCESS_CHUNK,
    MEMBER_BUILD_METADATA,
];

/// `copilot`, the key the lifted reasoning field is written under.
///
/// It is a constant of the wire format, not a derivation: the caller writes the
/// literal `{ copilot: { reasoningOpaque } }`.
pub const COPILOT_METADATA_KEY: &str = "copilot";

/// `reasoning_opaque`, read from the body, **snake_case**.
pub const REASONING_OPAQUE_FIELD: &str = "reasoning_opaque";

/// `reasoningOpaque`, written to the metadata, **camelCase**.
pub const REASONING_OPAQUE_METADATA_FIELD: &str = "reasoningOpaque";

/// `choices`, the outer array walked on every read.
pub const CHOICES_FIELD: &str = "choices";

/// `message`, the object inside the first choice of a complete response.
pub const MESSAGE_FIELD: &str = "message";

/// `delta`, the object inside the first choice of a streamed chunk.
pub const DELTA_FIELD: &str = "delta";

/// The suffix the provider id carries, stripped before it becomes a key.
pub const PROVIDER_NAME_SUFFIX: &str = ".";

// ---------------------------------------------------------------------------
// The metadata types
// ---------------------------------------------------------------------------

/// A provider metadata value: `JSONValue`.
///
/// The source returns `SharedV3ProviderMetadata | undefined`, whose values are
/// `JSONValue`. `serde_json::Value` is that type. It is an alias and not a
/// newtype on purpose: the contract is "arbitrary JSON", and wrapping it in a
/// newtype would add a conversion the source does not have.
pub type ProviderMetadataValue = Value;

/// One provider's metadata: `Record<string, JSONValue>`.
pub type ProviderMetadataRecord = BTreeMap<String, ProviderMetadataValue>;

/// The whole `SharedV3ProviderMetadata`: `Record<string, Record<string,
/// JSONValue>>`. The outer key is "a key indicating the provider id", as the
/// source's own documentation says.
pub type ProviderMetadata = BTreeMap<String, ProviderMetadataRecord>;

/// The empty metadata, which is what `undefined` becomes on the call site.
pub fn empty_provider_metadata() -> ProviderMetadata {
    ProviderMetadata::new()
}

/// The lifted Copilot record, typed so its single key can be pinned.
///
/// `serde` on this struct is the lock for the casing trap. There is deliberately
/// **no** `rename_all`: a `snake_case` rename would silently produce
/// `reasoning_opaque` and a `camelCase` one would be correct here by accident,
/// so the explicit `rename` leaves nothing to infer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CopilotMetadataRecord {
    /// Written as `reasoningOpaque`. Never `reasoning_opaque`, never
    /// `reasoningOpaqueId`, and never `reasoningid`.
    #[serde(rename = "reasoningOpaque", skip_serializing_if = "Option::is_none")]
    pub reasoning_opaque: Option<String>,
}

/// Which inner object of the first choice a walk descended into.
///
/// The two are not interchangeable: the non-streaming schema has
/// `message`, the streamed chunk schema has `delta`, and keeping them apart is
/// what lets one walk serve both call sites without a `path` that could be
/// wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InnerHop {
    /// `choices[0].message`, the complete response.
    Message,

    /// `choices[0].delta`, a streamed chunk. `delta` is `.nullish()` in the
    /// source schema, so both an absent and a `null` delta are real inputs.
    Delta,
}

impl InnerHop {
    /// The JSON name of the inner object.
    pub fn name(&self) -> &'static str {
        match self {
            InnerHop::Message => MESSAGE_FIELD,
            InnerHop::Delta => DELTA_FIELD,
        }
    }

    /// The path of the inner object, relative to the root of the payload.
    pub fn path(&self) -> String {
        format!("choices[0].{}", self.name())
    }

    /// The path of the lifted field, relative to the root of the payload.
    pub fn field_path(&self) -> String {
        format!("choices[0].{}.{}", self.name(), REASONING_OPAQUE_FIELD)
    }
}

/// A porting-control classification of a read, with no counterpart in the
/// source.
///
/// The source cannot classify anything: `parsedBody: unknown` gives it nothing
/// to classify against, and the extractor must not fail. These variants exist
/// to make the missing cases **visible** in Rust, where an absent field on a
/// non-`Option` is a compile error and a `null` on an `Option` is a silent
/// `None` — two failures that look nothing alike from inside the language and
/// one identical failure from outside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractionDiagnosis {
    /// The payload is not a JSON object. Code that indexed into it would throw
    /// in JavaScript; here it returns `None`.
    RootNotObject,

    /// `choices` is absent.
    ChoicesAbsent,

    /// `choices` is present and is not an array.
    ChoicesNotArray,

    /// `choices` is an empty array, so there is no first choice.
    ChoicesEmpty,

    /// `choices[0]` is present and is not an object.
    ChoiceNotObject,

    /// The inner object (`message` or `delta`) is absent.
    InnerAbsent(InnerHop),

    /// The inner object is present and is not an object.
    InnerNotObject(InnerHop),

    /// `reasoning_opaque` is absent from the inner object.
    FieldAbsent(InnerHop),

    /// `reasoning_opaque` is present and is `null`.
    FieldNull(InnerHop),

    /// `reasoning_opaque` is present, non-null and **falsy**: `""`, `0` or
    /// `false`. The truthiness filter drops it, the nullity filter keeps it.
    FieldFalsy(InnerHop),

    /// `reasoning_opaque` is present and truthy but is not a string. The
    /// caller's `z.string().nullish()` would have rejected it before the
    /// extractor ever saw it; under `parsedBody: unknown` it reaches the
    /// extractor anyway and is reported as-is.
    FieldWrongType(InnerHop),

    /// `reasoning_opaque` is present, truthy and a string.
    FieldPresent(InnerHop),
}

impl ExtractionDiagnosis {
    /// The path of the field responsible, as zod would report it.
    ///
    /// A `String` and not a `&'static str` because the array index and the
    /// inner hop are built at runtime. The root has no field, hence the empty
    /// string, as in the sibling ports of this family.
    pub fn path(&self) -> String {
        match self {
            ExtractionDiagnosis::RootNotObject => String::new(),
            ExtractionDiagnosis::ChoicesAbsent
            | ExtractionDiagnosis::ChoicesNotArray
            | ExtractionDiagnosis::ChoicesEmpty => CHOICES_FIELD.to_string(),
            ExtractionDiagnosis::ChoiceNotObject => "choices[0]".to_string(),
            ExtractionDiagnosis::InnerAbsent(hop) | ExtractionDiagnosis::InnerNotObject(hop) => {
                hop.path()
            }
            ExtractionDiagnosis::FieldAbsent(hop)
            | ExtractionDiagnosis::FieldNull(hop)
            | ExtractionDiagnosis::FieldFalsy(hop)
            | ExtractionDiagnosis::FieldWrongType(hop)
            | ExtractionDiagnosis::FieldPresent(hop) => hop.field_path(),
        }
    }

    /// Whether the extractor produced metadata for this classification.
    ///
    /// One predicate, so the diagnosis and the extraction cannot drift apart:
    /// every variant that yields `None` is listed here explicitly instead of
    /// being derived from a rule that could be edited on one side only.
    pub fn yields_metadata(&self) -> bool {
        matches!(
            self,
            ExtractionDiagnosis::FieldPresent(_) | ExtractionDiagnosis::FieldWrongType(_)
        )
    }
}

// ---------------------------------------------------------------------------
// The two operators the family actually uses
// ---------------------------------------------------------------------------

/// JavaScript truthiness of a JSON value.
///
/// The two lines that are easy to get wrong in Rust, and both are exercised by
/// `an_empty_object_and_an_empty_array_are_truthy_in_javascript`:
///
/// - `[]` and `{}` are **truthy**. A port that reaches for `is_empty()` treats
///   them as absent and silently drops data.
/// - a number is falsy only when it is exactly `0`, and `0.0` and `-0.0` are
///   the same value in `f64`, which matches JavaScript.
pub fn js_truthy(valeur: &Value) -> bool {
    match valeur {
        // `null` and `undefined`; the absent case is filtered out by the caller.
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => match number.as_f64() {
            Some(flottant) => flottant != 0.0,
            // A number JSON cannot express as `f64` is reported as truthy: the
            // safe direction is to keep data, never to drop it.
            None => true,
        },
        Value::String(texte) => !texte.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// The `?` test, i.e. the production filter of this family.
///
/// `None` covers both ways a value can be missing: the key absent, and the key
/// present with `null`. `value ? value : undefined` cannot tell them apart
/// either.
pub fn truthiness_filter(valeur: Option<&Value>) -> Option<&Value> {
    match valeur {
        Some(valeur) if js_truthy(valeur) => Some(valeur),
        _ => None,
    }
}

/// The `??` test, kept only as the comparison twin of `truthiness_filter`.
///
/// **Never called by any production path.** It exists so the difference
/// between the two operators is tested rather than described: the empty string
/// is the only value on which they disagree, and `truthiness_filter` is the one
/// the source uses.
pub fn nullity_filter(valeur: Option<&Value>) -> Option<&Value> {
    match valeur {
        None | Some(Value::Null) => None,
        Some(valeur) => Some(valeur),
    }
}

// ---------------------------------------------------------------------------
// The walk shared by both paths
// ---------------------------------------------------------------------------

/// Walks `choices[0].<hop>` and returns the `reasoning_opaque` value together
/// with the diagnosis of the walk.
///
/// Both extractors are built on this one function, which is the structural
/// reason they cannot diverge. It has no fallible path: every step that cannot
/// be taken is a `None` in the first element and a diagnosis in the second.
fn walk_to_opaque<'a>(
    parsed: &'a Value,
    hop: InnerHop,
) -> (Option<&'a Value>, ExtractionDiagnosis) {
    let root = match parsed.as_object() {
        Some(object) => object,
        None => return (None, ExtractionDiagnosis::RootNotObject),
    };
    let choices = match root.get(CHOICES_FIELD) {
        Some(Value::Array(choices)) => choices,
        None => return (None, ExtractionDiagnosis::ChoicesAbsent),
        Some(_) => return (None, ExtractionDiagnosis::ChoicesNotArray),
    };
    let first = match choices.first() {
        Some(first) => first,
        None => return (None, ExtractionDiagnosis::ChoicesEmpty),
    };
    let choice = match first.as_object() {
        Some(choice) => choice,
        None => return (None, ExtractionDiagnosis::ChoiceNotObject),
    };
    let inner = match choice.get(hop.name()) {
        Some(Value::Object(inner)) => inner,
        None => return (None, ExtractionDiagnosis::InnerAbsent(hop)),
        Some(_) => return (None, ExtractionDiagnosis::InnerNotObject(hop)),
    };
    match inner.get(REASONING_OPAQUE_FIELD) {
        None => (None, ExtractionDiagnosis::FieldAbsent(hop)),
        Some(Value::Null) => (None, ExtractionDiagnosis::FieldNull(hop)),
        Some(valeur) => {
            if !js_truthy(valeur) {
                (None, ExtractionDiagnosis::FieldFalsy(hop))
            } else if valeur.is_string() {
                (Some(valeur), ExtractionDiagnosis::FieldPresent(hop))
            } else {
                (Some(valeur), ExtractionDiagnosis::FieldWrongType(hop))
            }
        }
    }
}

/// `parsedBody.reasoning_opaque`, lifted through the truthiness filter.
fn lifted_from_body(parsed_body: &Value) -> Option<&Value> {
    walk_to_opaque(parsed_body, InnerHop::Message).0
}

/// `parsedChunk.choices[0].delta.reasoning_opaque`, lifted the same way.
fn lifted_from_chunk(parsed_chunk: &Value) -> Option<&Value> {
    walk_to_opaque(parsed_chunk, InnerHop::Delta).0
}

/// Builds the `{ copilot: { reasoningOpaque } }` map from a lifted value.
///
/// Returns an `Option` and not the bare map so that "nothing to report" stays
/// a single concept all the way to the caller, where `undefined` and `{}`
/// finally collapse.
fn copilot_metadata(valeur: &Value) -> Option<ProviderMetadata> {
    let mut record = ProviderMetadataRecord::new();
    record.insert(REASONING_OPAQUE_METADATA_FIELD.to_string(), valeur.clone());
    let mut metadata = empty_provider_metadata();
    metadata.insert(COPILOT_METADATA_KEY.to_string(), record);
    Some(metadata)
}

// ---------------------------------------------------------------------------
// The traits: the behavioural contract
// ---------------------------------------------------------------------------

/// The object returned by `createStreamExtractor()`.
///
/// `processChunk` returns `()` because the source declares `void`. The
/// duplicate-value **rejection** of the stream path is not a return value but
/// an `InvalidResponseDataError` thrown by the caller, so it is modelled as
/// recorded state plus a message, never as a `Result` on the trait method: a
/// `Result` there would be a signature the source does not have.
pub trait StreamMetadataExtractor {
    /// `processChunk(parsedChunk: unknown): void`.
    ///
    /// A chunk that is not an object, or that carries no `choices`, or that
    /// carries an empty `choices`, or whose `delta` is absent or `null`, is a
    /// no-op: the contract promises no failure, and the accumulator keeps
    /// whatever it already had.
    fn process_chunk(&mut self, parsed_chunk: &Value);

    /// `buildMetadata(): SharedV3ProviderMetadata | undefined`.
    ///
    /// Takes `&self` and not `&mut self`: the source closes over the same
    /// variables for reading and writing, and a build that mutated the
    /// accumulator would not be idempotent.
    fn build_metadata(&self) -> Option<ProviderMetadata>;
}

/// The `MetadataExtractor` type.
pub trait MetadataExtractor {
    /// `extractMetadata({ parsedBody })`, minus the `Promise`.
    ///
    /// Rust has no promise and the source never rejects, so the port is
    /// synchronous and total. There is no `Result`: the `unknown` input gives
    /// the source nothing to fail on, and a fallible signature would be a
    /// guarantee this contract does not make.
    fn extract_metadata(&self, parsed_body: &Value) -> Option<ProviderMetadata>;

    /// `createStreamExtractor()`.
    ///
    /// Takes `&self` and returns a fresh, independent accumulator, as in
    /// JavaScript where each call allocates a fresh closure environment.
    fn create_stream_extractor(&self) -> Box<dyn StreamMetadataExtractor>;
}

// ---------------------------------------------------------------------------
// The descriptor: the names, as data
// ---------------------------------------------------------------------------

/// The signature of `extractMetadata`, as the source writes it.
///
/// The source's arrow function has no runtime representation either, so this
/// is the same exercise one level up: the declaration is data, and data can be
/// asserted on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractMetadataMember {
    /// The destructured parameter of the argument object: `parsedBody`.
    #[serde(rename = "parameterName")]
    pub parameter_name: String,

    /// Its declared type: `unknown`. The whole contract in one word.
    #[serde(rename = "parameterType")]
    pub parameter_type: String,

    /// The declared return type, verbatim: `SharedV3ProviderMetadata |
    /// undefined`.
    #[serde(rename = "returnType")]
    pub return_type: String,
}

/// One member of the object returned by `createStreamExtractor()`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamMember {
    /// The member name, in declaration order: `processChunk`, `buildMetadata`.
    pub name: String,

    /// The positional parameter names. Never skipped, so an empty list
    /// serialises as `[]` and the JSON string of a whole descriptor is stable.
    pub parameters: Vec<String>,

    /// The declared return type, verbatim: `void` for `processChunk`.
    #[serde(rename = "returnType")]
    pub return_type: String,
}

/// The signature of `createStreamExtractor`, as the source writes it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateStreamExtractorMember {
    /// The two members of the returned object, in declaration order.
    pub members: Vec<StreamMember>,
}

/// The whole `MetadataExtractor` declaration, as serialisable data.
///
/// It exists for one reason: a Rust trait is invisible, so a member name
/// rewritten in snake_case would break every reflective consumer without
/// breaking a single behavioural test. `deny_unknown_fields` turns the
/// snake_case spelling into a read error instead of a silent default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataExtractorDescriptor {
    /// `extractMetadata`.
    #[serde(rename = "extractMetadata")]
    pub extract_metadata: ExtractMetadataMember,

    /// `createStreamExtractor`.
    #[serde(rename = "createStreamExtractor")]
    pub create_stream_extractor: CreateStreamExtractorMember,
}

/// The descriptor of the source file, transcribed member by member.
pub fn metadata_extractor_descriptor() -> MetadataExtractorDescriptor {
    MetadataExtractorDescriptor {
        extract_metadata: ExtractMetadataMember {
            parameter_name: PARAM_PARSED_BODY.to_string(),
            parameter_type: "unknown".to_string(),
            return_type: "SharedV3ProviderMetadata | undefined".to_string(),
        },
        create_stream_extractor: CreateStreamExtractorMember {
            members: vec![
                StreamMember {
                    name: MEMBER_PROCESS_CHUNK.to_string(),
                    parameters: vec![PARAM_PARSED_CHUNK.to_string()],
                    return_type: "void".to_string(),
                },
                StreamMember {
                    name: MEMBER_BUILD_METADATA.to_string(),
                    parameters: Vec::new(),
                    return_type: "SharedV3ProviderMetadata | undefined".to_string(),
                },
            ],
        },
    }
}

// ---------------------------------------------------------------------------
// The concrete extractors
// ---------------------------------------------------------------------------

/// The message the stream path throws on a second opaque value.
///
/// Quoted from `openai-compatible-chat-language-model.ts:474`, where it reaches
/// an `InvalidResponseDataError`. It is reproduced here as a string because the
/// port does not model that error type, which belongs to the caller's file.
pub const MULTIPLE_OPAQUE_VALUES_MESSAGE: &str =
    "Multiple reasoning_opaque values received in a single response. \
Only one thinking part per response is supported.";

/// The non-streaming extractor.
///
/// Permissive on purpose: the declared input is `unknown`, so a truthy value is
/// reported whatever its JSON type. The caller's `z.string().nullish()` would
/// have rejected a non-string first, and `ExtractionDiagnosis::FieldWrongType`
/// is where that divergence is recorded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CopilotMetadataExtractor {
    /// The provider options name of the configuration, kept so the always
    /// present bucket can be built by `merge_extracted_metadata`.
    pub provider_options_name: String,
}

impl CopilotMetadataExtractor {
    /// `new(provider)` derives the options name the way the model does.
    pub fn new(provider: &str) -> Self {
        CopilotMetadataExtractor {
            provider_options_name: provider_options_name(provider),
        }
    }
}

impl MetadataExtractor for CopilotMetadataExtractor {
    fn extract_metadata(&self, parsed_body: &Value) -> Option<ProviderMetadata> {
        lifted_from_body(parsed_body).and_then(copilot_metadata)
    }

    fn create_stream_extractor(&self) -> Box<dyn StreamMetadataExtractor> {
        Box::new(CopilotStreamMetadataExtractor::new(
            &self.provider_options_name,
        ))
    }
}

/// The streaming accumulator, the object `createStreamExtractor()` returns.
///
/// The state mirrors the caller: one `let reasoningOpaque: string | undefined`
/// plus a "have I already seen one" flag. The flag is not the same question as
/// the value being truthy, which is why both are kept.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CopilotStreamMetadataExtractor {
    /// The configuration's provider options name, so the accumulator can be
    /// built without a back-reference to its extractor.
    pub provider_options_name: String,

    /// The accumulated value, or `None` if no truthy value was seen.
    pub reasoning_opaque: Option<ProviderMetadataValue>,

    /// Whether a second truthy value arrived in the same response.
    pub duplicate_opaque: bool,
}

impl CopilotStreamMetadataExtractor {
    /// `new(provider)` derives the options name the way the model does.
    pub fn new(provider: &str) -> Self {
        CopilotStreamMetadataExtractor {
            provider_options_name: provider_options_name(provider),
            reasoning_opaque: None,
            duplicate_opaque: false,
        }
    }

    /// The message the caller would throw, or `None` when there was no second
    /// truthy value.
    ///
    /// The first truthy value wins and the accumulator keeps it, because the
    /// source throws instead of overwriting, and throwing is not something a
    /// `void` method can express here.
    pub fn duplicate_message(&self) -> Option<&'static str> {
        if self.duplicate_opaque {
            Some(MULTIPLE_OPAQUE_VALUES_MESSAGE)
        } else {
            None
        }
    }
}

impl StreamMetadataExtractor for CopilotStreamMetadataExtractor {
    fn process_chunk(&mut self, parsed_chunk: &Value) {
        // `if (delta.reasoning_opaque) { if (reasoningOpaque != null) { throw }
        // reasoningOpaque = delta.reasoning_opaque }`
        if let Some(valeur) = lifted_from_chunk(parsed_chunk) {
            if self.reasoning_opaque.is_some() {
                self.duplicate_opaque = true;
            } else {
                self.reasoning_opaque = Some(valeur.clone());
            }
        }
    }

    fn build_metadata(&self) -> Option<ProviderMetadata> {
        self.reasoning_opaque.as_ref().and_then(copilot_metadata)
    }
}

// ---------------------------------------------------------------------------
// The caller-side merge
// ---------------------------------------------------------------------------

/// `config.provider.split(".")[0].trim()`.
///
/// Total, like the JavaScript it mirrors: `split` always yields at least one
/// element, so `.chat` gives the empty string and `""` gives the empty string.
/// Both are legal metadata keys and neither panics. A `provider` with spaces
/// around the first segment is trimmed, as in the source.
pub fn provider_options_name(provider: &str) -> String {
    let premier = provider.split('.').next().unwrap_or("");
    premier.trim().to_string()
}

/// The literal from `openai-compatible-chat-language-model.ts:53`:
/// ``` `${options.name ?? "openai-compatible"}.chat` ```.
///
/// Provided so a test can feed the extractors the same string the real
/// configuration builds, rather than a hand-written one.
pub fn chat_provider_name(name: &str) -> String {
    format!("{}{}chat", name, PROVIDER_NAME_SUFFIX)
}

/// `{ [providerOptionsName]: {}, ...extracted }`.
///
/// The spread of `undefined` and the spread of `{}` are the same object in
/// JavaScript, so `None` and `Some(empty)` produce the same map here. The
/// extracted entries are written **after** the bucket, matching the source
/// order, so a metadata map carrying the bucket key itself overwrites the empty
/// bucket rather than the reverse.
pub fn merge_extracted_metadata(
    provider_options_name: &str,
    extracted: Option<ProviderMetadata>,
) -> ProviderMetadata {
    let mut metadata = empty_provider_metadata();
    metadata.insert(
        provider_options_name.to_string(),
        ProviderMetadataRecord::new(),
    );
    if let Some(extracted) = extracted {
        for (clef, record) in extracted {
            metadata.insert(clef, record);
        }
    }
    metadata
}

/// `classify_extraction`, the porting-control walk over a complete response.
pub fn classify_extraction(parsed_body: &Value) -> ExtractionDiagnosis {
    walk_to_opaque(parsed_body, InnerHop::Message).1
}

/// `classify_stream_chunk`, the same walk over a streamed chunk.
///
/// The two differ only in the inner hop, and both are exposed so a test can
/// assert that a `delta`-shaped payload is not a `message`-shaped payload.
pub fn classify_stream_chunk(parsed_chunk: &Value) -> ExtractionDiagnosis {
    walk_to_opaque(parsed_chunk, InnerHop::Delta).1
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    /// Builds a complete response carrying `reasoning_opaque` at the real path.
    fn body(valeur: Value) -> Value {
        json!({ "choices": [ { "message": { "reasoning_opaque": valeur } } ] })
    }

    /// Builds a streamed chunk carrying `reasoning_opaque` at the real path.
    fn chunk(valeur: Value) -> Value {
        json!({ "choices": [ { "delta": { "reasoning_opaque": valeur } } ] })
    }

    /// The metadata the extractors are expected to produce, built by hand from
    /// the wire spelling rather than from the production helper.
    fn attendu(valeur: &str) -> Option<ProviderMetadata> {
        serde_json::from_str(&format!(
            r#"{{"copilot":{{"reasoningOpaque":"{}"}}}}"#,
            valeur
        ))
        .unwrap()
    }

    /// The extractor under test, configured as the chat model would configure
    /// it for Copilot.
    fn extractor() -> CopilotMetadataExtractor {
        CopilotMetadataExtractor::new(&chat_provider_name("copilot"))
    }

    /// Case 5, the empty input, plus every other root shape a `parsedBody:
    /// unknown` can be handed. The extractor is total, so none may panic.
    #[test]
    fn an_empty_input_extracts_nothing_and_does_not_panic() {
        for racine in [json!({}), json!(null), json!([]), json!(""), json!(0), json!(true)] {
            assert_eq!(
                extractor().extract_metadata(&racine),
                None,
                "root: {}",
                racine
            );
            assert_eq!(
                classify_extraction(&racine),
                ExtractionDiagnosis::RootNotObject
            );
        }
        // An empty object is a valid body that simply carries nothing, and it
        // is diagnosed one step further in than a non-object root.
        assert_eq!(extractor().extract_metadata(&json!({})), None);
        assert_eq!(
            classify_extraction(&json!({})),
            ExtractionDiagnosis::ChoicesAbsent
        );
    }

    /// Case 1: the field is simply not there.
    #[test]
    fn a_field_that_is_absent_extracts_nothing() {
        let sans = json!({ "choices": [ { "message": { "role": "assistant" } } ] });
        assert_eq!(extractor().extract_metadata(&sans), None);
        assert_eq!(
            classify_extraction(&sans),
            ExtractionDiagnosis::FieldAbsent(InnerHop::Message)
        );
        assert_eq!(
            classify_extraction(&sans).path(),
            "choices[0].message.reasoning_opaque"
        );
    }

    /// Case 2: the field is present and explicitly `null`.
    ///
    /// This is the arm the open question touches. The extractor drops it,
    /// because the source filters with `?`; the diagnosis still names it, so
    /// the two never have to be told apart to be tested.
    #[test]
    fn a_field_present_but_null_extracts_nothing() {
        let nul = body(Value::Null);
        assert_eq!(extractor().extract_metadata(&nul), None);
        assert_eq!(
            classify_extraction(&nul),
            ExtractionDiagnosis::FieldNull(InnerHop::Message)
        );
    }

    /// Case 3: the field is present with a type nobody expected.
    ///
    /// Under `parsedBody: unknown` the extractor cannot refuse it, so a truthy
    /// wrong type is reported verbatim and only the diagnosis calls it out.
    #[test]
    fn a_field_present_with_an_unexpected_type_is_reported_not_dropped() {
        for valeur in [json!({}), json!([]), json!(42), json!(true)] {
            let charge = body(valeur.clone());
            assert_eq!(
                extractor().extract_metadata(&charge),
                copilot_metadata(&valeur),
                "valeur: {}",
                valeur
            );
            assert_eq!(
                classify_extraction(&charge),
                ExtractionDiagnosis::FieldWrongType(InnerHop::Message)
            );
            assert!(ExtractionDiagnosis::FieldWrongType(InnerHop::Message).yields_metadata());
        }

        // And the falsy wrong types are dropped by the truthiness filter, not
        // by a type check: `0` and `false` never reach the type branch.
        for valeur in [json!(0), json!(false)] {
            let charge = body(valeur);
            assert_eq!(extractor().extract_metadata(&charge), None);
            assert_eq!(
                classify_extraction(&charge),
                ExtractionDiagnosis::FieldFalsy(InnerHop::Message)
            );
            assert!(!ExtractionDiagnosis::FieldFalsy(InnerHop::Message).yields_metadata());
        }
    }

    /// Case 4: the nested object is there and its inner key is not.
    ///
    /// Four levels of navigation, each of which is an ordinary `None` in Rust
    /// and an ordinary `undefined` in JavaScript, plus the two shapes where the
    /// hop exists but is not an object.
    #[test]
    fn a_nested_object_missing_its_inner_key_extracts_nothing() {
        for (charge, attendue) in [
            (json!({ "choices": [ {} ] }), ExtractionDiagnosis::InnerAbsent(InnerHop::Message)),
            (
                json!({ "choices": [ { "content": "x" } ] }),
                ExtractionDiagnosis::InnerAbsent(InnerHop::Message),
            ),
            (
                json!({ "choices": [ { "message": "texte" } ] }),
                ExtractionDiagnosis::InnerNotObject(InnerHop::Message),
            ),
            (
                json!({ "choices": [ { "message": [] } ] }),
                ExtractionDiagnosis::InnerNotObject(InnerHop::Message),
            ),
            (
                json!({ "choices": "beaucoup" }),
                ExtractionDiagnosis::ChoicesNotArray,
            ),
            (json!({ "choices": [] }), ExtractionDiagnosis::ChoicesEmpty),
            (json!({ "choices": [42] }), ExtractionDiagnosis::ChoiceNotObject),
            (json!([1, 2]), ExtractionDiagnosis::RootNotObject),
        ] {
            assert_eq!(
                extractor().extract_metadata(&charge),
                None,
                "charge: {}",
                charge
            );
            assert_eq!(classify_extraction(&charge), attendue, "charge: {}", charge);
            assert!(!attendue.yields_metadata());
        }
    }

    /// The lock for the open question: the two failures are classified
    /// differently and extract identically, and that is asserted rather than
    /// asserted-about. If a later investigation settles whether an optional
    /// schema field accepts an explicit `null`, only the diagnosis arm changes.
    #[test]
    fn an_absent_field_and_an_explicit_null_extract_alike_but_classify_differently() {
        let absent = json!({ "choices": [ { "message": {} } ] });
        let nul = body(Value::Null);

        assert_eq!(extractor().extract_metadata(&absent), None);
        assert_eq!(extractor().extract_metadata(&nul), None);
        assert_eq!(
            extractor().extract_metadata(&absent),
            extractor().extract_metadata(&nul)
        );

        assert_ne!(
            classify_extraction(&absent),
            classify_extraction(&nul),
            "the two nullity sources must stay distinguishable in the diagnosis"
        );
        assert_eq!(
            classify_extraction(&absent),
            ExtractionDiagnosis::FieldAbsent(InnerHop::Message)
        );
        assert_eq!(
            classify_extraction(&nul),
            ExtractionDiagnosis::FieldNull(InnerHop::Message)
        );
        // Same path, so a reporter points at the right field either way.
        assert_eq!(
            classify_extraction(&absent).path(),
            classify_extraction(&nul).path()
        );
    }

    /// The happy path, spelled out: a non-empty string is lifted, renamed and
    /// keyed under the provider id.
    #[test]
    fn a_truthy_string_is_lifted_renamed_and_keyed() {
        let charge = body(json!("opaque-token"));
        assert_eq!(
            extractor().extract_metadata(&charge),
            attendu("opaque-token")
        );
        assert_eq!(
            classify_extraction(&charge),
            ExtractionDiagnosis::FieldPresent(InnerHop::Message)
        );
    }

    /// TRAP 1, direction one: the two spellings are different on the way in and
    /// on the way out, and only the exact ones are accepted.
    #[test]
    fn the_read_field_is_snake_case_and_the_written_field_is_camel_case() {
        let metadata = extractor().extract_metadata(&body(json!("x"))).unwrap();
        let sortie = serde_json::to_string(&metadata).unwrap();

        assert_eq!(sortie, r#"{"copilot":{"reasoningOpaque":"x"}}"#);
        assert!(sortie.contains(r#""reasoningOpaque":"x""#));
        for forme_interdite in [
            "\"reasoning_opaque\":",
            "\"reasoningopaque\":",
            "\"reasoningOpaqueId\":",
            "\"copilot_id\":",
        ] {
            assert!(
                !sortie.contains(forme_interdite),
                "{} must not appear in {}",
                forme_interdite,
                sortie
            );
        }

        // The read side is the other half of the same lock: the camelCase form
        // is not an accepted spelling on the way in either.
        let mauvais = json!({ "choices": [ { "message": { "reasoningOpaque": "x" } } ] });
        assert_eq!(extractor().extract_metadata(&mauvais), None);
        assert_eq!(
            classify_extraction(&mauvais),
            ExtractionDiagnosis::FieldAbsent(InnerHop::Message)
        );
    }

    /// TRAP 1, direction two: the typed record refuses the snake_case form on
    /// read instead of silently yielding `None`.
    ///
    /// This is the case that is invisible without `deny_unknown_fields`: plain
    /// `serde` strips the unknown key and returns an empty record, and a
    /// consumer would read that as "Copilot reported no opaque token".
    #[test]
    fn the_snake_case_form_is_rejected_when_the_metadata_is_read_back() {
        let faux: Value = serde_json::from_str(r#"{"copilot":{"reasoning_opaque":"x"}}"#).unwrap();
        assert!(
            CopilotMetadataRecord::deserialize(&faux).is_err(),
            "the snake_case spelling must be an error, not a silent None"
        );

        let faux_imbrique: Value =
            serde_json::from_str(r#"{"copilot":{"ReasoningOpaque":"x"}}"#).unwrap();
        assert!(CopilotMetadataRecord::deserialize(&faux_imbrique).is_err());

        let faux_entier: Value =
            serde_json::from_str(r#"{"copilot":{"reasoning_opaque":7}}"#).unwrap();
        assert!(CopilotMetadataRecord::deserialize(&faux_entier).is_err());

        // The exact spelling, and only that one, round-trips.
        let exact: Value = serde_json::from_str(r#"{"copilot":{"reasoningOpaque":"x"}}"#).unwrap();
        let relu: CopilotMetadataRecord = CopilotMetadataRecord::deserialize(&exact).unwrap();
        assert_eq!(relu.reasoning_opaque.as_deref(), Some("x"));
        assert_eq!(
            serde_json::to_string(&relu).unwrap(),
            r#"{"reasoningOpaque":"x"}"#
        );
    }

    /// The three serde failure modes look nothing alike, which is the whole
    /// reason the port has an explicit classification: in Rust an absent field
    /// on a non-`Option` cannot even be written, a `null` on an `Option` is a
    /// silent `None`, and a wrong type is a loud error.
    #[test]
    fn the_three_serde_failure_modes_stay_distinct() {
        // Absent: accepted, `None`, no error.
        let absent: CopilotMetadataRecord = serde_json::from_str("{}").unwrap();
        assert_eq!(absent.reasoning_opaque, None);

        // Explicit null: accepted, `None`, and indistinguishable from absent.
        let nul: CopilotMetadataRecord =
            serde_json::from_str(r#"{"reasoningOpaque":null}"#).unwrap();
        assert_eq!(nul.reasoning_opaque, None);
        assert_eq!(absent, nul);

        // Wrong type: a loud error, not a `None`.
        let mauvais: Result<CopilotMetadataRecord, _> =
            serde_json::from_str(r#"{"reasoningOpaque":42}"#);
        assert!(mauvais.is_err());

        // Which is exactly the split the extractor is on the other side of: it
        // can only ever answer `None` or a value, never an error.
        assert!(!ExtractionDiagnosis::FieldAbsent(InnerHop::Message).yields_metadata());
        assert!(!ExtractionDiagnosis::FieldNull(InnerHop::Message).yields_metadata());
    }

    /// TRAP 2: the two operators, on the four falsy JavaScript values, with the
    /// production filter being the truthiness one.
    #[test]
    fn the_truthiness_filter_drops_the_empty_string_and_the_nullity_filter_keeps_it() {
        for falsy in [json!(""), json!(0), json!(false), Value::Null] {
            assert!(
                truthiness_filter(Some(&falsy)).is_none(),
                "truthiness must drop {}",
                falsy
            );
            let par_nullite = nullity_filter(Some(&falsy));
            let attendu = if falsy == Value::Null {
                None
            } else {
                Some(&falsy)
            };
            assert_eq!(par_nullite, attendu, "nullity on {}", falsy);
        }
        assert!(nullity_filter(None).is_none());
        assert!(truthiness_filter(None).is_none());

        // The source uses the first, and the extractor must inherit that: an
        // empty string coming from Copilot produces no metadata at all.
        let vide = body(json!(""));
        assert_eq!(extractor().extract_metadata(&vide), None);
        assert_eq!(
            classify_extraction(&vide),
            ExtractionDiagnosis::FieldFalsy(InnerHop::Message)
        );
    }

    /// The inverse mistake, and the one a Rust port makes by reaching for
    /// `is_empty()`: `{}` and `[]` are **truthy** in JavaScript.
    #[test]
    fn an_empty_object_and_an_empty_array_are_truthy_in_javascript() {
        assert!(js_truthy(&json!({})));
        assert!(js_truthy(&json!([])));
        assert!(truthiness_filter(Some(&json!({}))).is_some());
        assert!(truthiness_filter(Some(&json!([]))).is_some());
        assert_eq!(
            extractor().extract_metadata(&body(json!({}))),
            copilot_metadata(&json!({}))
        );
    }

    /// The accumulator: nothing to report before any chunk, one report after
    /// one chunk, and a stable report however many idempotent chunks follow.
    #[test]
    fn a_stream_extractor_accumulates_across_chunks() {
        let mut flux = extractor().create_stream_extractor();
        assert_eq!(flux.build_metadata(), None);

        // A chunk with no field is a no-op, and it must not clear anything.
        flux.process_chunk(&json!({ "choices": [ { "delta": {} } ] }));
        assert_eq!(flux.build_metadata(), None);
        flux.process_chunk(&chunk(json!("opaque-token")));
        assert_eq!(flux.build_metadata(), attendu("opaque-token"));
        flux.process_chunk(&json!({ "choices": [] }));
        assert_eq!(flux.build_metadata(), attendu("opaque-token"));
        // Building twice changes nothing: the source closes over the same
        // variable for reading and writing.
        assert_eq!(flux.build_metadata(), attendu("opaque-token"));
    }

    /// A second truthy value is recorded, the first one is kept, and the caller
    /// learns about it through the message it would throw.
    #[test]
    fn a_stream_extractor_records_a_second_opaque_value() {
        // The concrete type, not the trait object: `duplicate_message` is an
        // inherent method the source does not have on the interface.
        let mut flux = CopilotStreamMetadataExtractor::new(&chat_provider_name("copilot"));
        assert_eq!(flux.duplicate_message(), None);

        flux.process_chunk(&chunk(json!("premier")));
        assert_eq!(flux.duplicate_message(), None);

        flux.process_chunk(&chunk(json!("second")));
        assert_eq!(
            flux.duplicate_message(),
            Some(MULTIPLE_OPAQUE_VALUES_MESSAGE)
        );
        assert!(MULTIPLE_OPAQUE_VALUES_MESSAGE.contains("Multiple reasoning_opaque values"));
        // The first value wins, because the source throws instead of
        // overwriting.
        assert_eq!(flux.build_metadata(), attendu("premier"));
        assert!(flux.duplicate_opaque);
    }

    /// The two paths must not disagree: the same payload fed to the
    /// non-streaming extractor and to the streaming accumulator, each through
    /// its own inner hop, gives the same metadata. A divergence here would mean
    /// `processChunk` and `extractMetadata` do not share a filter.
    #[test]
    fn the_stream_and_single_shot_extractors_agree() {
        for valeur in [
            json!("opaque-token"),
            json!("token"),
            json!(""),
            json!(0),
            json!(false),
            Value::Null,
            json!({}),
            json!([1]),
        ] {
            let unique = extractor().extract_metadata(&body(valeur.clone()));
            let mut flux = extractor().create_stream_extractor();
            flux.process_chunk(&chunk(valeur.clone()));
            assert_eq!(unique, flux.build_metadata(), "valeur: {}", valeur);

            // And the two classifications agree on everything except the path,
            // which names the hop each payload actually used.
            let un = classify_extraction(&body(valeur.clone()));
            let un_chunck = classify_stream_chunk(&chunk(valeur.clone()));
            assert_eq!(un.yields_metadata(), un_chunck.yields_metadata());
            assert_eq!(
                un.path().replace("message", "delta"),
                un_chunck.path()
            );
        }
    }

    /// `message` and `delta` are not interchangeable: a chunk-shaped payload
    /// read as a complete response finds no `message`, and vice versa.
    #[test]
    fn the_message_path_and_the_delta_path_are_not_one() {
        let reponse = body(json!("x"));
        let flux = chunk(json!("x"));

        assert_eq!(
            classify_extraction(&reponse),
            ExtractionDiagnosis::FieldPresent(InnerHop::Message)
        );
        assert_eq!(
            classify_extraction(&flux),
            ExtractionDiagnosis::InnerAbsent(InnerHop::Message)
        );
        assert_eq!(
            classify_stream_chunk(&flux),
            ExtractionDiagnosis::FieldPresent(InnerHop::Delta)
        );
        assert_eq!(
            classify_stream_chunk(&reponse),
            ExtractionDiagnosis::InnerAbsent(InnerHop::Delta)
        );
        assert_eq!(InnerHop::Message.path(), "choices[0].message");
        assert_eq!(InnerHop::Delta.path(), "choices[0].delta");
        assert_eq!(
            InnerHop::Delta.field_path(),
            "choices[0].delta.reasoning_opaque"
        );
        // `delta` is `.nullish()` in the source schema, so a null delta is a
        // real input and not a shape the port invented.
        let delta_nul = json!({ "choices": [ { "delta": null } ] });
        assert_eq!(
            classify_stream_chunk(&delta_nul),
            ExtractionDiagnosis::InnerAbsent(InnerHop::Delta)
        );
    }

    /// `undefined` and `{}` are the same object to the only caller, so the
    /// merge must not tell them apart.
    #[test]
    fn undefined_metadata_and_an_empty_metadata_merge_identically() {
        let sans = merge_extracted_metadata("copilot", None);
        let vide = merge_extracted_metadata("copilot", Some(empty_provider_metadata()));
        assert_eq!(sans, vide);
        assert_eq!(serde_json::to_string(&sans).unwrap(), r#"{"copilot":{}}"#);

        // With something to report, the entry lands under its own key.
        let extrait = extractor().extract_metadata(&body(json!("x")));
        let fusionne = merge_extracted_metadata("copilot", extrait);
        assert_eq!(
            serde_json::to_string(&fusionne).unwrap(),
            r#"{"copilot":{"reasoningOpaque":"x"}}"#
        );
    }

    /// The derived key, including the cases that produce an empty string. It
    /// never panics, and the literal `copilot` key stays independent of it.
    #[test]
    fn the_provider_options_name_is_the_segment_before_the_first_dot() {
        for (provider, attendu) in [
            ("copilot.chat", "copilot"),
            ("copilot.responses", "copilot"),
            ("openai-compatible.chat", "openai-compatible"),
            ("copilot", "copilot"),
            ("  spaced . chat", "spaced"),
            (".chat", ""),
            ("", ""),
            (".", ""),
        ] {
            assert_eq!(
                provider_options_name(provider),
                attendu,
                "provider: {}",
                provider
            );
        }
        // And the configuration's own builder, quoted from the model.
        assert_eq!(chat_provider_name("copilot"), "copilot.chat");
        assert_eq!(
            chat_provider_name("openai-compatible"),
            "openai-compatible.chat"
        );
        // The empty bucket key is legal and is not the same thing as `copilot`.
        let vide = merge_extracted_metadata(&provider_options_name(".chat"), None);
        assert_eq!(serde_json::to_string(&vide).unwrap(), r#"{"":{}}"#);
    }

    /// The literal member names, asserted against their own snake_case forms.
    /// A trait method cannot carry these, so the constants are the only place
    /// the spelling exists.
    #[test]
    fn the_member_names_are_camel_case_and_not_snake_case() {
        assert_eq!(MEMBER_EXTRACT_METADATA, "extractMetadata");
        assert_eq!(MEMBER_CREATE_STREAM_EXTRACTOR, "createStreamExtractor");
        assert_eq!(MEMBER_PROCESS_CHUNK, "processChunk");
        assert_eq!(MEMBER_BUILD_METADATA, "buildMetadata");
        assert_eq!(PARAM_PARSED_BODY, "parsedBody");
        assert_eq!(PARAM_PARSED_CHUNK, "parsedChunk");

        for membre in MEMBERS_METADATA_EXTRACTOR {
            assert!(!membre.contains('_'), "{} must not be snake_case", membre);
            assert!(!membre.contains(' '), "{} must not contain a space", membre);
        }
        for (camel, snake) in [
            (MEMBER_EXTRACT_METADATA, "extract_metadata"),
            (MEMBER_CREATE_STREAM_EXTRACTOR, "create_stream_extractor"),
            (MEMBER_PROCESS_CHUNK, "process_chunk"),
            (MEMBER_BUILD_METADATA, "build_metadata"),
            (PARAM_PARSED_BODY, "parsed_body"),
            (PARAM_PARSED_CHUNK, "parsed_chunk"),
        ] {
            assert_ne!(camel, snake);
        }
    }

    /// The whole declaration as one JSON string, which is the strongest
    /// available check that nothing was renamed and nothing was dropped.
    #[test]
    fn the_descriptor_json_is_exactly_the_declaration_of_the_source() {
        let descriptor = metadata_extractor_descriptor();
        assert_eq!(
            serde_json::to_string(&descriptor).unwrap(),
            concat!(
                r#"{"extractMetadata":{"parameterName":"parsedBody","parameterType":"unknown","#,
                r#""returnType":"SharedV3ProviderMetadata | undefined"},"#,
                r#""createStreamExtractor":{"members":["#,
                r#"{"name":"processChunk","parameters":["parsedChunk"],"returnType":"void"},"#,
                r#"{"name":"buildMetadata","parameters":[],"#,
                r#""returnType":"SharedV3ProviderMetadata | undefined"}]}}"#,
            )
        );
        // And it survives a round trip unchanged.
        let relu: MetadataExtractorDescriptor =
            serde_json::from_str(&serde_json::to_string(&descriptor).unwrap()).unwrap();
        assert_eq!(relu, descriptor);
    }

    /// TRAP 1, direction two, at the declaration level: every snake_case
    /// spelling of the members and of the parameters is an error on read. Each
    /// payload is otherwise complete, so the only thing that can reject it is
    /// the rename.
    #[test]
    fn the_descriptor_rejects_the_snake_case_spellings_on_read() {
        let membre_ok = r#"{"parameterName":"parsedBody","parameterType":"unknown","returnType":"x"}"#;
        let faux = vec![
            // The two members, snake_case, with a complete payload underneath.
            format!(
                r#"{{"extract_metadata":{},"createStreamExtractor":{{"members":[]}}}}"#,
                membre_ok
            ),
            format!(
                r#"{{"extractMetadata":{},"create_stream_extractor":{{"members":[]}}}}"#,
                membre_ok
            ),
            // The property name of the parameter of `extractMetadata`.
            format!(
                r#"{{"extractMetadata":{{"parameterName":"parsedBody","parameter_type":"unknown","returnType":"x"}},"createStreamExtractor":{{"members":[]}}}}"#
            ),
            // The members and the parameter name of the stream extractor.
            r#"{"extractMetadata":{"parameterName":"parsedBody","parameterType":"unknown","returnType":"x"},"createStreamExtractor":{"members":[{"name":"process_chunk","parameters":["parsedChunk"],"returnType":"void"}]}}"#.to_string(),
            r#"{"extractMetadata":{"parameterName":"parsedBody","parameterType":"unknown","returnType":"x"},"createStreamExtractor":{"members":[{"name":"processChunk","parameters":["parsed_chunk"],"returnType":"void"}]}}"#.to_string(),
            r#"{"extractMetadata":{"parameterName":"parsedBody","parameterType":"unknown","returnType":"x"},"createStreamExtractor":{"members":[{"name":"build_metadata","parameters":[],"returnType":"x"}]}}"#.to_string(),
        ];
        for faux in &faux {
            let relu: Result<MetadataExtractorDescriptor, _> = serde_json::from_str(faux);
            assert!(relu.is_err(), "{} must be rejected", faux);
        }

        // The one spelling a schema cannot catch, stated so nobody assumes it
        // can: a mistyped *value* is a perfectly valid `String` to `serde`, so
        // `"parameterName": "parsed_body"` is accepted. The constants and
        // `the_member_names_are_camel_case_and_not_snake_case` are the only
        // locks on that half of the trap.
        let valeur_fausse: MetadataExtractorDescriptor = serde_json::from_str(
            r#"{"extractMetadata":{"parameterName":"parsed_body","parameterType":"unknown","returnType":"x"},"createStreamExtractor":{"members":[]}}"#,
        )
        .expect("a mistyped value is still a valid String, so it is accepted");
        assert_eq!(valeur_fausse.extract_metadata.parameter_name, "parsed_body");
        assert_ne!(
            valeur_fausse.extract_metadata.parameter_name,
            metadata_extractor_descriptor().extract_metadata.parameter_name
        );

        // And the exact spellings are accepted, for the record.
        let exact: MetadataExtractorDescriptor = serde_json::from_str(
            r#"{"extractMetadata":{"parameterName":"parsedBody","parameterType":"unknown","returnType":"SharedV3ProviderMetadata | undefined"},"createStreamExtractor":{"members":[{"name":"processChunk","parameters":["parsedChunk"],"returnType":"void"},{"name":"buildMetadata","parameters":[],"returnType":"SharedV3ProviderMetadata | undefined"}]}}"#,
        )
        .unwrap();
        assert_eq!(exact, metadata_extractor_descriptor());
    }

    /// The untyped view and the typed view of the same record must agree. The
    /// extractor builds the map directly, so nothing forces them to match.
    #[test]
    fn the_typed_record_and_the_built_map_are_the_same_thing() {
        let metadata = extractor().extract_metadata(&body(json!("x"))).unwrap();
        assert!(ProviderMetadataRecord::new().is_empty());

        let built = metadata.get(COPILOT_METADATA_KEY).unwrap();
        let typed = CopilotMetadataRecord {
            reasoning_opaque: Some("x".to_string()),
        };
        assert_eq!(
            serde_json::to_value(&built).unwrap(),
            serde_json::to_value(&typed).unwrap()
        );
        assert_eq!(
            serde_json::to_string(&built).unwrap(),
            r#"{"reasoningOpaque":"x"}"#
        );
    }

    /// Every variant of the classification, and the invariant that the
    /// extraction never fails: it answers `None` or a value, never an error.
    #[test]
    fn the_classification_covers_every_step_of_the_walk() {
        for hop in [InnerHop::Message, InnerHop::Delta] {
            let variantes = [
                ExtractionDiagnosis::RootNotObject,
                ExtractionDiagnosis::ChoicesAbsent,
                ExtractionDiagnosis::ChoicesNotArray,
                ExtractionDiagnosis::ChoicesEmpty,
                ExtractionDiagnosis::ChoiceNotObject,
                ExtractionDiagnosis::InnerAbsent(hop),
                ExtractionDiagnosis::InnerNotObject(hop),
                ExtractionDiagnosis::FieldAbsent(hop),
                ExtractionDiagnosis::FieldNull(hop),
                ExtractionDiagnosis::FieldFalsy(hop),
                ExtractionDiagnosis::FieldWrongType(hop),
                ExtractionDiagnosis::FieldPresent(hop),
            ];
            // Each has a non-empty path except the root, and only the two
            // field-level outcomes can yield metadata.
            for variante in variantes {
                if variante == ExtractionDiagnosis::RootNotObject {
                    assert_eq!(variante.path(), "");
                } else {
                    assert!(!variante.path().is_empty(), "{:?}", variante);
                }
                if variante.yields_metadata() {
                    assert!(
                        matches!(
                            variante,
                            ExtractionDiagnosis::FieldPresent(_)
                                | ExtractionDiagnosis::FieldWrongType(_)
                        ),
                        "{:?} must not be classified as producing metadata",
                        variante
                    );
                }
            }
        }

        // The walk is total, and the extraction agrees with the classification
        // on every payload.
        for charge in [
            json!({}),
            json!({ "choices": [ { "message": { "reasoning_opaque": "x" } } ] }),
            json!({ "choices": [ { "delta": { "reasoning_opaque": "x" } } ] }),
            json!(null),
            json!([1, 2, 3]),
            json!({ "choices": [ { "message": { "reasoning_opaque": null } } ] }),
        ] {
            let diagnostic = classify_extraction(&charge);
            assert_eq!(
                extractor().extract_metadata(&charge).is_some(),
                diagnostic.yields_metadata(),
                "charge: {}",
                charge
            );
        }
    }
}
