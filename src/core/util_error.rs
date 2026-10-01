//! Port of `opencode/packages/core/src/util/error.ts`.
//!
//! This file is where the `{ name, data }` shape of every named error in the
//! codebase comes from: `toObject()` returns both levels explicitly, so the
//! payload is never flattened next to the tag. Kilo's `v1_config_error.rs` and
//! my `session_v1.rs` both follow this file's convention, which is why it was
//! worth porting rather than treating as infrastructure.
//!
//! The TS uses an abstract class with static factories, which Rust has no
//! direct equivalent for. The portable parts are the trait, the name test, the
//! two-level object, and the `UnknownError` the file declares.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A named error: a tag plus a payload, serialised on two levels.
pub trait NamedError: std::fmt::Debug {
    /// The tag written in `name`, set at construction time by the factory.
    fn error_name(&self) -> &str;

    /// The payload written in `data`.
    fn error_data(&self) -> Value;
}

/// `NamedError.hasName`: a tag comparison, the type test of the TS factory.
pub fn has_name<E: NamedError + ?Sized>(error: &E, name: &str) -> bool {
    error.error_name() == name
}

/// `toObject()`: `{ name, data }`, never flattened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorObject {
    pub name: String,
    pub data: Value,
}

/// Build the two-level object for any named error.
pub fn to_object<E: NamedError + ?Sized>(error: &E) -> ErrorObject {
    ErrorObject { name: error.error_name().to_string(), data: error.error_data() }
}

/// `NamedError.Unknown = create("UnknownError", { message, ref? })`.
///
/// `ref` is a Rust keyword, so the field is `ref_` with an explicit rename -
/// and the name on the wire stays `ref`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownError {
    pub message: String,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
}

impl NamedError for UnknownError {
    fn error_name(&self) -> &str {
        "UnknownError"
    }

    fn error_data(&self) -> Value {
        // Serialising the payload only: the tag belongs to the envelope, and
        // putting it inside `data` would nest it twice.
        let mut map = serde_json::Map::new();
        map.insert("message".to_string(), Value::String(self.message.clone()));
        if let Some(r) = &self.ref_ {
            map.insert("ref".to_string(), Value::String(r.clone()));
        }
        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_object_keeps_both_levels() {
        let e = UnknownError { message: "boom".to_string(), ref_: Some("r1".to_string()) };
        let v = serde_json::to_value(to_object(&e)).unwrap();
        assert_eq!(
            v,
            json!({ "name": "UnknownError", "data": { "message": "boom", "ref": "r1" } })
        );
        // The tag must not be repeated inside data.
        assert!(v["data"].get("name").is_none());
    }

    #[test]
    fn has_name_compares_the_tag() {
        let e = UnknownError { message: "m".to_string(), ref_: None };
        assert!(has_name(&e, "UnknownError"));
        assert!(!has_name(&e, "APIError"));
    }

    #[test]
    fn an_absent_ref_disappears_instead_of_becoming_null() {
        let e = UnknownError { message: "m".to_string(), ref_: None };
        let v = serde_json::to_value(to_object(&e)).unwrap();
        assert_eq!(v["data"], json!({ "message": "m" }));
        assert!(v["data"].get("ref").is_none());
    }

    #[test]
    fn the_payload_key_is_ref_not_ref_() {
        let e = UnknownError { message: "m".to_string(), ref_: Some("r".to_string()) };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["ref"], json!("r"));
        assert!(v.get("ref_").is_none());
        let back: UnknownError = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn the_data_helper_returns_only_the_payload() {
        let e = UnknownError { message: "m".to_string(), ref_: None };
        assert_eq!(e.error_data(), json!({ "message": "m" }));
    }
}