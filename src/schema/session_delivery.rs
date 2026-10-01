//! Port of `opencode/packages/schema/src/session-delivery.ts`.
//!
//! `Delivery = Schema.Literals(["steer", "queue"])`: how a prompt joins a
//! session that is already busy. Two literals, so two variants - no third
//! state is representable, exactly like the TS.

use serde::{Deserialize, Serialize};

/// `Schema.Literals(["steer", "queue"])`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Delivery {
    /// Interrupt the running turn with the new prompt.
    #[serde(rename = "steer")]
    Steer,
    /// Queue the prompt for after the current turn.
    #[serde(rename = "queue")]
    Queue,
}

impl Delivery {
    /// Literal spelling of this variant, as it appears on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Delivery::Steer => "steer",
            Delivery::Queue => "queue",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn both_literals_use_their_own_spelling() {
        assert_eq!(serde_json::to_value(Delivery::Steer).unwrap(), json!("steer"));
        assert_eq!(serde_json::to_value(Delivery::Queue).unwrap(), json!("queue"));
    }

    #[test]
    fn reading_a_literal_gives_back_the_variant() {
        assert_eq!(serde_json::from_value::<Delivery>(json!("steer")).unwrap(), Delivery::Steer);
        assert_eq!(serde_json::from_value::<Delivery>(json!("queue")).unwrap(), Delivery::Queue);
    }

    #[test]
    fn an_unknown_delivery_is_refused() {
        assert!(serde_json::from_value::<Delivery>(json!("append")).is_err());
    }

    #[test]
    fn as_str_agrees_with_the_serialized_form() {
        for d in [Delivery::Steer, Delivery::Queue] {
            let v = serde_json::to_value(d).unwrap();
            assert_eq!(v, json!(d.as_str()));
        }
    }
}