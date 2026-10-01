//! Port of `opencode/packages/schema/src/pty-ticket.ts`.
//!
//! `ConnectToken` for the pty websocket. The one trap: `expires_in` is already
//! snake_case in the TS and keeps its underscore on the wire - renaming it to
//! `expiresIn` would be the reflexive camelCase "fix", and it would break.

use serde::{Deserialize, Serialize};

/// `PtyTicket.ConnectToken`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectToken {
    pub ticket: String,
    pub expires_in: i64,
}

impl ConnectToken {
    pub fn new(ticket: impl Into<String>, expires_in: i64) -> Self {
        Self { ticket: ticket.into(), expires_in }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn expires_in_keeps_its_underscore_on_the_wire() {
        let v = serde_json::to_value(ConnectToken::new("t_1", 30)).unwrap();
        assert_eq!(v, json!({ "ticket": "t_1", "expires_in": 30 }));
        assert!(v.get("expiresIn").is_none());
    }

    #[test]
    fn the_token_round_trips() {
        let t = ConnectToken::new("t_2", 60);
        let back: ConnectToken = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
    }
}