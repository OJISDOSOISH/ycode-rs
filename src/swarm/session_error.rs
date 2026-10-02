//! Portage Rust de `packages/core/src/session/error.ts`.
//!
//! La source declare deux erreurs `Schema.TaggedErrorClass` : l une porte
//! `sessionID` et `messageID`, l autre `sessionID` et `details`. Chacune expose
//! un accesseur `message` qui formate ces champs. Ce module fige les deux
//! etiquettes et les deux formatages comme des donnees et des fonctions pures,
//! sans dependance a `effect`.
//!
//! `Cargo.toml` ne declare pas `effect` : les classes `Schema` n ont donc pas
//! d equivalent ici. Seuls le contenu transporte et le texte produit sont
//! portes, ce qui est la seule chose testable sans le moteur de schemas.

/// Etiquette de l erreur de decodage d un message, telle que declaree.
pub const MESSAGE_DECODE_TAG: &str = "Session.MessageDecodeError";

/// Etiquette de l erreur de decodage d un instantane de contexte.
pub const CONTEXT_SNAPSHOT_DECODE_TAG: &str = "Session.ContextSnapshotDecodeError";

/// Echec du decodage d un message dans une session.
///
/// Champs repris de la source : `sessionID` et `messageID` deviennent
/// `session_id` et `message_id` en snake case, sans changer le sens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDecodeError {
    /// Identifiant de la session qui contient le message illisible.
    pub session_id: String,
    /// Identifiant du message qui n a pas pu etre decode.
    pub message_id: String,
}

impl MessageDecodeError {
    /// Construit l erreur a partir de ses deux champs.
    pub fn new(session_id: String, message_id: String) -> Self {
        MessageDecodeError {
            session_id,
            message_id,
        }
    }

    /// L etiquette telle que la source la declare.
    pub fn tag(&self) -> &'static str {
        MESSAGE_DECODE_TAG
    }

    /// Le texte produit par l accesseur `message` de la source.
    pub fn message(&self) -> String {
        format!(
            "Failed to decode message {} in session {}",
            self.message_id, self.session_id
        )
    }
}

/// Echec du decodage d un instantane de contexte pour une session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSnapshotDecodeError {
    /// Identifiant de la session dont l instantane est illisible.
    pub session_id: String,
    /// Detail fourni par le moteur de schemas, texte libre.
    pub details: String,
}

impl ContextSnapshotDecodeError {
    /// Construit l erreur a partir de ses deux champs.
    pub fn new(session_id: String, details: String) -> Self {
        ContextSnapshotDecodeError {
            session_id,
            details,
        }
    }

    /// L etiquette telle que la source la declare.
    pub fn tag(&self) -> &'static str {
        CONTEXT_SNAPSHOT_DECODE_TAG
    }

    /// Le texte produit par l accesseur `message` de la source.
    pub fn message(&self) -> String {
        format!(
            "Failed to decode context snapshot for session {}: {}",
            self.session_id, self.details
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_etiquettes_sont_celles_de_la_source() {
        assert_eq!(MESSAGE_DECODE_TAG, "Session.MessageDecodeError");
        assert_eq!(
            CONTEXT_SNAPSHOT_DECODE_TAG,
            "Session.ContextSnapshotDecodeError"
        );
    }

    #[test]
    fn le_message_de_decodage_cite_le_message_et_la_session() {
        let erreur = MessageDecodeError::new("ses_1".to_string(), "msg_2".to_string());
        assert_eq!(
            erreur.message(),
            "Failed to decode message msg_2 in session ses_1"
        );
    }

    #[test]
    fn le_message_de_decodage_distribue_les_champs_dans_l_ordre() {
        let a = MessageDecodeError::new("ses_a".to_string(), "msg_b".to_string());
        let b = MessageDecodeError::new("msg_b".to_string(), "ses_a".to_string());
        assert_ne!(a.message(), b.message());
        assert!(a.message().contains("msg_b"));
        assert!(a.message().contains("ses_a"));
    }

    #[test]
    fn le_message_d_instantane_cite_la_session_et_le_detail() {
        let erreur =
            ContextSnapshotDecodeError::new("ses_1".to_string(), "trame coupee".to_string());
        assert_eq!(
            erreur.message(),
            "Failed to decode context snapshot for session ses_1: trame coupee"
        );
    }

    #[test]
    fn les_accesseurs_d_etiquette_suivent_la_source() {
        let decodage = MessageDecodeError::new("s".to_string(), "m".to_string());
        assert_eq!(decodage.tag(), MESSAGE_DECODE_TAG);
        let instantane = ContextSnapshotDecodeError::new("s".to_string(), "d".to_string());
        assert_eq!(instantane.tag(), CONTEXT_SNAPSHOT_DECODE_TAG);
    }

    #[test]
    fn deux_erreurs_identiques_sont_egales() {
        let premiere = MessageDecodeError::new("s".to_string(), "m".to_string());
        let seconde = MessageDecodeError::new("s".to_string(), "m".to_string());
        assert_eq!(premiere, seconde);
        let autre = MessageDecodeError::new("s".to_string(), "autre".to_string());
        assert_ne!(premiere, autre);
    }
}
