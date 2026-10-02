//! Portage de `packages/core/src/session/error.ts`.
//!
//! La source declare deux erreurs etiquetees Effect :
//!
//! ```ts
//! export class MessageDecodeError ... ("Session.MessageDecodeError", {
//!   sessionID: SessionSchema.ID, messageID: SessionMessage.ID,
//! }) // message: `Failed to decode message ${messageID} in session ${sessionID}`
//! export class ContextSnapshotDecodeError ... ("Session.ContextSnapshotDecodeError", {
//!   sessionID: SessionSchema.ID, details: Schema.String,
//! }) // message: `Failed to decode context snapshot for session ${sessionID}: ${details}`
//! ```
//!
//! Ce sont des donnees, pas des comportements : deux champs chacune, plus un
//! texte derive. En Rust c'est deux structs qui portent leurs champs en
//! `String`, deux constantes d'etiquette, et une methode `message()` qui rend
//! exactement la chaine de la source. Aucun pilote, aucune dependance.

/// Etiquette de `MessageDecodeError`, telle qu'ecrite dans la source.
pub const MESSAGE_DECODE_TAG: &str = "Session.MessageDecodeError";

/// Etiquette de `ContextSnapshotDecodeError`.
pub const CONTEXT_SNAPSHOT_DECODE_TAG: &str = "Session.ContextSnapshotDecodeError";

/// Echec de decodage d'un message d'une session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageDecodeError {
    /// Identifiant de la session (`SessionSchema.ID`, `ses_...`).
    pub session_id: String,
    /// Identifiant du message (`SessionMessage.ID`, `msg_...`).
    pub message_id: String,
}

impl MessageDecodeError {
    /// Construit l'erreur a partir de ses deux identifiants.
    pub fn new(session_id: &str, message_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
        }
    }

    /// Texte de l'erreur, a l'identique de la source.
    pub fn message(&self) -> String {
        format!(
            "Failed to decode message {} in session {}",
            self.message_id, self.session_id
        )
    }

    /// Etiquette de l'erreur.
    pub fn tag(&self) -> &'static str {
        MESSAGE_DECODE_TAG
    }
}

impl std::fmt::Display for MessageDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for MessageDecodeError {}

/// Echec de decodage d'un instantane de contexte d'une session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSnapshotDecodeError {
    /// Identifiant de la session.
    pub session_id: String,
    /// Detail libre, `Schema.String` dans la source.
    pub details: String,
}

impl ContextSnapshotDecodeError {
    /// Construit l'erreur a partir de la session et du detail.
    pub fn new(session_id: &str, details: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            details: details.to_string(),
        }
    }

    /// Texte de l'erreur, a l'identique de la source.
    pub fn message(&self) -> String {
        format!(
            "Failed to decode context snapshot for session {}: {}",
            self.session_id, self.details
        )
    }

    /// Etiquette de l'erreur.
    pub fn tag(&self) -> &'static str {
        CONTEXT_SNAPSHOT_DECODE_TAG
    }
}

impl std::fmt::Display for ContextSnapshotDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for ContextSnapshotDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_etiquettes_sont_celles_de_la_source() {
        assert_eq!(MESSAGE_DECODE_TAG, "Session.MessageDecodeError");
        assert_eq!(CONTEXT_SNAPSHOT_DECODE_TAG, "Session.ContextSnapshotDecodeError");
        assert_ne!(MESSAGE_DECODE_TAG, CONTEXT_SNAPSHOT_DECODE_TAG);
    }

    #[test]
    fn le_message_de_decodage_reprend_les_deux_identifiants() {
        let erreur = MessageDecodeError::new("ses_1", "msg_2");
        assert_eq!(erreur.message(), "Failed to decode message msg_2 in session ses_1");
        assert_eq!(erreur.tag(), MESSAGE_DECODE_TAG);
    }

    #[test]
    fn le_message_d_instantane_reprend_session_et_detail() {
        let erreur = ContextSnapshotDecodeError::new("ses_1", "json invalide");
        assert_eq!(
            erreur.message(),
            "Failed to decode context snapshot for session ses_1: json invalide"
        );
        assert_eq!(erreur.tag(), CONTEXT_SNAPSHOT_DECODE_TAG);
    }

    #[test]
    fn l_affichage_reprend_le_message() {
        let erreur = MessageDecodeError::new("ses_1", "msg_2");
        assert_eq!(format!("{}", erreur), erreur.message());
        let erreur = ContextSnapshotDecodeError::new("ses_1", "d");
        assert_eq!(format!("{}", erreur), erreur.message());
    }

    #[test]
    fn un_detail_vide_reste_un_detail() {
        // Chaine vide contre absence : le detail est un `Schema.String`,
        // donc `""` est une valeur et se retrouve tel quel dans le message.
        let erreur = ContextSnapshotDecodeError::new("ses_1", "");
        assert!(erreur.message().ends_with(": "));
    }

    #[test]
    fn deux_erreurs_aux_champs_egaux_sont_egales() {
        assert_eq!(
            MessageDecodeError::new("ses_1", "msg_2"),
            MessageDecodeError::new("ses_1", "msg_2")
        );
        assert_ne!(
            MessageDecodeError::new("ses_1", "msg_2"),
            MessageDecodeError::new("ses_1", "msg_3")
        );
    }
}
