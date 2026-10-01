//! Port of `opencode/packages/core/src/tool/tools.ts`.
//!
//! The TS module is nothing more than a capability declaration: a narrow
//! `Interface` exposing `register`, wrapped in an Effect `Context.Service`
//! tagged `@opencode/v2/Tools`. There is no logic, no data, nothing to
//! serialize - the whole module exists so that consumers can depend on the
//! *ability to register tools* without depending on the registry itself.
//!
//! The Rust equivalent of an Effect service is a plain trait. The `Effect`
//! return type collapses to `Result<(), ToolRegistrationError>`; the `Scope`
//! requirement is dropped, because lifetime-mediated cleanup has no analogue
//! here and nothing in this crate needs it. The concrete registry lives in
//! `super::registry` - the trait is what keeps the two decoupled, exactly as
//! the TS `Service` decouples consumers from `Registry`.

use thiserror::Error;

/// `Tool.RegistrationError`, as the interface's error channel. Only the
/// variant the TS type system actually carries through `register` survives.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ToolRegistrationError {
    /// A tool could not be registered under its name.
    #[error("failed to register tool: {0}")]
    Failed(String),
}

/// `Tools.Interface`: the narrow, registration-only capability.
///
/// The TS takes a `Readonly<Record<string, Tool.AnyTool>>`; here a name /
/// registration pair slice stands in for the record. The names are opaque to
/// this trait - the TS signature is equally silent about what a tool is,
/// which is the point of the indirection.
pub trait Interface {
    /// `register`: files each named tool so later calls can resolve it.
    fn register(&mut self, tools: &[(&str, super::registry::Registration)]) -> Result<(), ToolRegistrationError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::registry::Registration;

    /// A minimal holder, only to prove the trait is implementable and that
    /// the error path flows through the signature.
    #[derive(Debug, Default)]
    struct Holder {
        filed: Vec<(String, Registration)>,
    }

    impl Interface for Holder {
        fn register(&mut self, tools: &[(&str, Registration)]) -> Result<(), ToolRegistrationError> {
            // Mirrors the TS contract's one observable rule: registering an
            // empty set changes nothing, and an empty name cannot be filed.
            for (name, registration) in tools {
                if name.is_empty() {
                    return Err(ToolRegistrationError::Failed("empty name".to_string()));
                }
                self.filed.push((name.to_string(), registration.clone()));
            }
            Ok(())
        }
    }

    fn reg(identity: u64, action: &str) -> Registration {
        Registration { identity, action: action.to_string() }
    }

    #[test]
    fn registering_files_every_named_tool() {
        let mut holder = Holder::default();
        holder
            .register(&[("read", reg(1, "read")), ("write", reg(2, "write"))])
            .unwrap();
        assert_eq!(holder.filed.len(), 2);
        assert_eq!(holder.filed[0].0, "read");
    }

    #[test]
    fn registering_nothing_changes_nothing() {
        let mut holder = Holder::default();
        assert!(holder.register(&[]).is_ok());
        assert!(holder.filed.is_empty());
    }

    #[test]
    fn an_empty_name_is_refused() {
        let mut holder = Holder::default();
        let error = holder.register(&[("", reg(1, "read"))]).unwrap_err();
        assert_eq!(error, ToolRegistrationError::Failed("empty name".to_string()));
    }

    #[test]
    fn the_error_is_model_visible_verbatim() {
        assert_eq!(
            ToolRegistrationError::Failed("read".to_string()).to_string(),
            "failed to register tool: read"
        );
    }
}
