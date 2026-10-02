//! Portage de `packages/core/src/session/error.ts`.
//!
//! La source tient vingt-quatre lignes et ne declare que deux erreurs
//! taggees Effect, sans aucune logique d'acces ni effet :
//!
//! ```ts
//! export class MessageDecodeError ... ("Session.MessageDecodeError", {
//!   sessionID: SessionSchema.ID,
//!   messageID: SessionMessage.ID,
//! }) { override get message() {
//!   return `Failed to decode message ${this.messageID} in session ${this.sessionID}`
//! } }
//! export class ContextSnapshotDecodeError ... ("Session.ContextSnapshotDecodeError", {
//!   sessionID: SessionSchema.ID,
//!   details: Schema.String,
//! }) { override get message() {
//!   return `Failed to decode context snapshot for session ${this.sessionID}: ${this.details}`
//! } }
//! ```
//!
//! `SessionSchema.ID` et `SessionMessage.ID` sont des chaines marquees qui
//! n'existent qu'a la compilation : a l'execution ce sont des chaines
//! ordinaires, d'ou le `String` ici. `Schema.TaggedErrorClass` pose un
//! discriminant `_tag` serialise sur le fil, repris tel quel avec
//! `#[serde(tag = "_tag")]`. Les deux `get message()` sont portes comme des
//! methodes pures, sans `Display` invente.

use serde::{Deserialize, Serialize};

/// `MessageDecodeError` : echec de decodage d'un message dans une session.
///
/// Tag sur le fil : `"Session.MessageDecodeError"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag", rename = "Session.MessageDecodeError")]
pub struct MessageDecodeError {
    /// `SessionSchema.ID`, chaine marquee a la compilation.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// `SessionMessage.ID`, chaine marquee a la compilation.
    #[serde(rename = "messageID")]
    pub message_id: String,
}

impl MessageDecodeError {
    /// Construit l'erreur pour ce couple session et message.
    pub fn new(session_id: &str, message_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            message_id: message_id.to_string(),
        }
    }

    /// Le `get message()` de la source, a l'identique.
    pub fn message(&self) -> String {
        format!(
            "Failed to decode message {} in session {}",
            self.message_id, self.session_id
        )
    }
}

/// `ContextSnapshotDecodeError` : echec de decodage de l'instantane de
/// contexte d'une session.
///
/// Tag sur le fil : `"Session.ContextSnapshotDecodeError"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "_tag", rename = "Session.ContextSnapshotDecodeError")]
pub struct ContextSnapshotDecodeError {
    /// `SessionSchema.ID`, chaine marquee a la compilation.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// Detail libre, `Schema.String` dans la source.
    pub details: String,
}

impl ContextSnapshotDecodeError {
    /// Construit l'erreur pour cette session et ce detail.
    pub fn new(session_id: &str, details: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            details: details.to_string(),
        }
    }

    /// Le `get message()` de la source, a l'identique.
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
    fn le_message_de_decodage_reprend_session_et_message_dans_l_ordre() {
        let erreur = MessageDecodeError::new("ses_1", "msg_2");
        assert_eq!(
            erreur.message(),
            "Failed to decode message msg_2 in session ses_1"
        );
    }

    #[test]
    fn le_message_d_instantane_reprend_session_puis_detail() {
        let erreur = ContextSnapshotDecodeError::new("ses_1", "json invalide");
        assert_eq!(
            erreur.message(),
            "Failed to decode context snapshot for session ses_1: json invalide"
        );
    }

    #[test]
    fn les_identifiants_restent_en_majuscules_sur_le_fil() {
        let erreur = MessageDecodeError::new("ses_1", "msg_2");
        let json = serde_json::to_value(&erreur).expect("serialisation");
        assert_eq!(json["_tag"], "Session.MessageDecodeError");
        assert_eq!(json["sessionID"], "ses_1");
        assert_eq!(json["messageID"], "msg_2");
        assert!(json.get("session_id").is_none());
        assert!(json.get("message_id").is_none());
        let relue: MessageDecodeError =
            serde_json::from_value(json).expect("deserialisation");
        assert_eq!(relue, erreur);
    }

    #[test]
    fn l_erreur_d_instantane_publie_son_tag_et_son_detail() {
        let erreur = ContextSnapshotDecodeError::new("ses_9", "tronque");
        let json = serde_json::to_value(&erreur).expect("serialisation");
        assert_eq!(json["_tag"], "Session.ContextSnapshotDecodeError");
        assert_eq!(json["sessionID"], "ses_9");
        assert_eq!(json["details"], "tronque");
        let relue: ContextSnapshotDecodeError =
            serde_json::from_value(json).expect("deserialisation");
        assert_eq!(relue, erreur);
    }

    #[test]
    fn un_tag_inconnu_est_refuse_a_la_deserialisation() {
        let json = serde_json::json!({
            "_tag": "Session.AutreErreur",
            "sessionID": "ses_1",
            "messageID": "msg_1"
        });
        assert!(serde_json::from_value::<MessageDecodeError>(json).is_err());
    }

    #[test]
    fn une_chaine_vide_reste_une_valeur_presente_dans_le_message() {
        let erreur = MessageDecodeError::new("", "");
        assert_eq!(erreur.message(), "Failed to decode message  in session ");
        let json = serde_json::to_value(&erreur).expect("serialisation");
        assert_eq!(json["sessionID"], "");
    }
}
