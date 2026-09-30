//! Portage Rust de `opencode/packages/core/src/session/schema.ts`.
//!
//! ## Role de ce fichier, et pourquoi il existe malgre tout
//!
//! La source fait **9 lignes** :
//!
//! ```ts
//! export const ID = Session.ID
//! export const Info = Session.Info
//! ```
//!
//! C'est un pur reexport de `packages/schema/src/session.ts`. Un portage qui se
//! contenterait de reproduire ces deux lignes serait vide a l'interieur et
//! trompeur a l'exterieur.
//!
//! On y met donc le **contrat** lui-meme : les types que l'API publie, avec
//! leurs noms de champs JSON exacts. C'est ici, et non dans la conversion depuis
//! la base, que vit la compatibilite avec le TypeScript d'origine.
//!
//! ## Suite donnee par la revue croisee
//!
//! La vraie source de ces types est `packages/schema/src/session.ts`, dont une
//! partie est deja portee dans `crate::schema` (`session_message.rs`). Donc ce
//! fichier est **intermediaire** : a terme il doit se reduire a des
//! `pub use crate::schema::...`.
//!
//! On ne le reduit pas encore, et la raison est concrete : `crate::schema` ne
//! contient aujourd'hui que les messages, pas les infos de session. Definir le
//! contrat ici, puis reexporter quand la source sera complete, evite d'ecrire
//! les types deux fois. L'alternative, les definir des maintenant dans
//! `crate::schema`, laisserait ici un module vide, et surtout risquerait la
//! duplication que la revue a deja attrapee une fois sur `sessionID` /
//! `callID` : deux definitions d'un meme contrat qui divergent en silence.

use serde::{Deserialize, Serialize};

/// Identifiant de session.
pub type SessionId = String;
/// Identifiant de projet.
pub type ProjectId = String;
/// Identifiant d'espace de travail.
pub type WorkspaceId = String;
/// Identifiant d'agent.
pub type AgentId = String;
/// Identifiant de modele.
pub type ModelId = String;
/// Identifiant de fournisseur.
pub type ProviderId = String;
/// Identifiant de message de session.
pub type MessageId = String;
/// Chemin relatif a la racine du depot.
pub type RelativePath = String;
/// Chemin absolu.
pub type AbsolutePath = String;

/// Variante de modele substituee quand la colonne `model` n'en porte pas.
///
/// Cote TypeScript, la valeur par defaut n'est nulle part dans le schema : elle
/// est ecrite en dur dans `fromRow`, avec `?? "default"`. La constante est
/// declaree ici plutot que la-bas, parce que c'est une valeur du **contrat**
/// publiee dans le JSON, pas un detail de conversion.
pub const DEFAULT_VARIANT: &str = "default";

/// Reference vers un modele, telle que publiee par l'API.
///
/// Correspond a `Model.Ref` de `packages/schema/src/model.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRef {
    /// Identifiant du modele.
    pub id: ModelId,
    /// Fournisseur du modele.
    ///
    /// La majuscule est volontaire : `providerID`, pas `providerId`. C'est le
    /// genre de nom que TypeScript ecrit sans y penser et que le
    /// compilateur ne signale jamais.
    #[serde(rename = "providerID")]
    pub provider_id: ProviderId,
    /// Variante du modele.
    ///
    /// Facultatif dans le schema, mais toujours present en sortie : la
    /// conversion depuis la base substitue [`DEFAULT_VARIANT`] a une valeur
    /// absente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Statut d'un fichier dans un diff. ADT a tag de l'original.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffStatus {
    /// Fichier cree.
    Added,
    /// Fichier modifie.
    Modified,
    /// Fichier supprime.
    Deleted,
}

/// Difference d'un fichier, telle qu'empaquetee dans un etat de retour arriere.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileDiff {
    /// Chemin du fichier, relatif au depot.
    pub path: RelativePath,
    /// Statut du fichier.
    pub status: DiffStatus,
    /// Nombre de lignes ajoutees.
    pub additions: u64,
    /// Nombre de lignes supprimees.
    pub deletions: u64,
    /// Patch unifie.
    pub patch: String,
}

/// Etat du point de retour arriere, tel que stocke dans la colonne JSON
/// `revert` et publie tel quel par l'API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevertState {
    /// Message auquel revenir.
    #[serde(rename = "messageID")]
    pub message_id: MessageId,
    /// Piece du message a considers.
    #[serde(rename = "partID", default, skip_serializing_if = "Option::is_none")]
    pub part_id: Option<String>,
    /// Instantane a restaurer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    /// Diff a appliquer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    /// Fichiers concernes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileDiff>>,
}

/// Compteurs de jetons mis en cache.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct CacheTokens {
    /// Jetons lus en cache.
    pub read: f64,
    /// Jetons ecrits en cache.
    pub write: f64,
}

/// Total des jetons d'une session.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Tokens {
    /// Jetons d'entree.
    pub input: f64,
    /// Jetons de sortie.
    pub output: f64,
    /// Jetons de raisonnement.
    pub reasoning: f64,
    /// Jetons de cache.
    pub cache: CacheTokens,
}

/// Horodatages d'une session.
///
/// L'encodage est en **millisecondes depuis l'epoch**, et non une chaine ISO :
/// `DateTimeUtcFromMillis` encode via `DateTime.toEpochMillis`. Une chaine
/// ISO ici serait le genre d'erreur invisible jusqu'a l'echange avec le
/// TypeScript.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Time {
    /// Creation de la session.
    pub created: i64,
    /// Derniere mise a jour.
    pub updated: i64,
    /// Archivage. Absent si la session n'a jamais ete archivee.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<i64>,
}

/// Reperage d'une session sur le disque.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocationRef {
    /// Repertoire de travail.
    pub directory: AbsolutePath,
    /// Espace de travail parent. Absent si la session est a la racine.
    #[serde(rename = "workspaceID", default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<WorkspaceId>,
}

/// Une session, telle que l'API la publie. Correspond a `Session.Info`.
///
/// L'ordre des champs reprend celui de `packages/schema/src/session.ts`. Il n'a
/// aucune signification en JSON, mais il rend la comparaison avec l'original
/// lisible, et ca ne coute rien.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Info {
    /// Identifiant de session.
    pub id: SessionId,
    /// Session parente, si cette session est une fourche.
    #[serde(rename = "parentID", default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<SessionId>,
    /// Projet auquel la session appartient.
    #[serde(rename = "projectID")]
    pub project_id: ProjectId,
    /// Agent utilise, si la session en a choisi un explicitement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentId>,
    /// Modele utilise, si la session en a choisi un explicitement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    /// Cout cumule, en unites de la monnaie du fournisseur.
    ///
    /// Flottant et non entier : cote schema c'est un `Schema.Finite`, et en
    /// base une colonne `real`.
    pub cost: f64,
    /// Compteurs de jetons.
    pub tokens: Tokens,
    /// Horodatages.
    pub time: Time,
    /// Titre affiche.
    pub title: String,
    /// Emplacement sur le disque.
    pub location: LocationRef,
    /// Sous-chemin dans le depot, si la session est cantonnee a un sous-dossier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subpath: Option<RelativePath>,
    /// Point de retour arriere, si l'utilisateur en a pose un.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revert: Option<RevertState>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_contrat_a_bien_les_majuscules_du_typescript() {
        // Le test le plus important du fichier. `projectId` au lieu de
        // `projectID` passerait la compilation ET tous les tests de logique :
        // l'erreur n'apparaitrait qu'a l'echange avec le TypeScript.
        let info = Info {
            id: "ses_1".to_string(),
            parent_id: Some("ses_0".to_string()),
            project_id: "pro_1".to_string(),
            agent: None,
            model: Some(ModelRef {
                id: "gpt".to_string(),
                provider_id: "openai".to_string(),
                variant: Some("high".to_string()),
            }),
            cost: 0.0,
            tokens: Tokens::default(),
            time: Time::default(),
            title: "T".to_string(),
            location: LocationRef { directory: "/tmp".to_string(), workspace_id: Some("wsp_1".to_string()) },
            subpath: None,
            revert: None,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["parentID"], "ses_0");
        assert_eq!(json["projectID"], "pro_1");
        assert_eq!(json["model"]["providerID"], "openai");
        assert_eq!(json["location"]["workspaceID"], "wsp_1");
    }

    #[test]
    fn un_contrat_minimal_ne_serialise_que_l_obligatoire() {
        // `optionalKey` retire la cle a l'encodage : une cle a null divergerait.
        let info = Info {
            id: "ses_1".to_string(),
            parent_id: None,
            project_id: "pro_1".to_string(),
            agent: None,
            model: None,
            cost: 0.0,
            tokens: Tokens::default(),
            time: Time::default(),
            title: "T".to_string(),
            location: LocationRef { directory: "/tmp".to_string(), workspace_id: None },
            subpath: None,
            revert: None,
        };
        let json = serde_json::to_value(&info).unwrap();
        let obj = json.as_object().unwrap();
        for key in ["parentID", "agent", "model", "subpath", "revert"] {
            assert!(!obj.contains_key(key), "{key} ne doit pas apparaitre");
        }
        assert!(!obj["time"].as_object().unwrap().contains_key("archived"));
    }

    #[test]
    fn le_statut_de_diff_est_un_adt_a_tag_valide() {
        // Exactement les trois valeurs de l'original, et rien de plus.
        for (variante, attendue) in
            [(DiffStatus::Added, "added"), (DiffStatus::Modified, "modified"), (DiffStatus::Deleted, "deleted")]
        {
            assert_eq!(serde_json::to_string(&variante).unwrap(), format!("\"{attendue}\""));
        }
    }

    #[test]
    fn les_horodatages_sont_des_millisecondes_et_non_des_chaines() {
        let time = Time { created: 1_700_000_000_000, updated: 0, archived: Some(1) };
        let json = serde_json::to_value(&time).unwrap();
        assert!(json["created"].is_i64(), "created doit rester un nombre");
        // Le type `i64` est explicite : sans lui, l'entier literal serait
        // infere en `i32`, ou un horodatage en millisecondes deborde.
        assert_eq!(json["created"], 1_700_000_000_000i64);
        assert_eq!(json["archived"], 1);
    }

    #[test]
    fn le_revert_conserve_les_noms_de_champs_de_l_original() {
        let revert = RevertState {
            message_id: "msg_1".to_string(),
            part_id: Some("prt_1".to_string()),
            snapshot: None,
            diff: Some("patch".to_string()),
            files: Some(Vec::new()),
        };
        let json = serde_json::to_value(&revert).unwrap();
        assert_eq!(json["messageID"], "msg_1");
        assert_eq!(json["partID"], "prt_1");
        assert!(!json.as_object().unwrap().contains_key("snapshot"));
        // Une liste vide reste presente : elle a ete explicitement fournie.
        assert_eq!(json["files"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn un_contrat_vient_du_json_avec_les_bons_noms() {
        // Sens inverse : ce que le TypeScript produit doit se relire tel quel.
        let json = r#"{
            "id":"ses_1","parentID":"ses_0","projectID":"pro_1","agent":"build",
            "model":{"id":"gpt","providerID":"openai","variant":"high"},
            "cost":1.5,
            "tokens":{"input":1,"output":2,"reasoning":3,"cache":{"read":4,"write":5}},
            "time":{"created":100,"updated":200},
            "title":"T",
            "location":{"directory":"/tmp","workspaceID":"wsp_1"}
        }"#;
        let info: Info = serde_json::from_str(json).unwrap();
        assert_eq!(info.id, "ses_1");
        assert_eq!(info.model.unwrap().provider_id, "openai");
        assert_eq!(info.tokens.cache.write, 5.0);
        assert_eq!(info.time.archived, None);
    }
}
