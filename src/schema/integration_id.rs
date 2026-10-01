//! Port of `opencode/packages/schema/src/integration-id.ts`.
//!
//! Two branded strings and nothing else. The branding is a compile-time check
//! in TypeScript with no runtime representation, so both are plain string
//! aliases here - and that is the faithful port, not a shortcut: a newtype
//! would add a type the source does not have.
//!
//! The prefix convention that the brands imply lives with the identifiers that
//! use them: `IntegrationId` is built in `core::integration`.

/// `Integration.ID`.
pub type IntegrationId = String;

/// `Integration.MethodID`.
pub type IntegrationMethodId = String;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_ids_are_plain_strings() {
        let i: IntegrationId = "int_1".to_string();
        let m: IntegrationMethodId = "met_1".to_string();
        assert_eq!(i, "int_1");
        assert_eq!(m, "met_1");
    }
}