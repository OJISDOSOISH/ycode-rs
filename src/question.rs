//! Portage Rust de `opencode/packages/schema/src/question.ts` et de
//! `opencode/packages/core/src/question.ts`.
//!
//! Le mecanisme des questions est ce qui permet a un agent de s arreter et de
//! demander une precision plutot que de deviner. C est une barriere anti-
//! hallucination : sans elle, l agent invente une specification.
//!
//! Point de conception a ne pas confondre : une **reponse est une liste de
//! choix**, pas un chaine. L'original declare
//! `Answer = Schema.Array(Schema.String)`, et `Reply.answers` est un tableau de
//! ces listes. Donc pour deux questions, la reponse est
//! `[["oui"], ["non", "peut-etre"]]` : une liste par question, dans l ordre.
//!
//! Confondre les deux est facile et produit un agent qui decale ses reponses
//! d une question a l autre. Les tests verrouillent la structure.

use serde::{Deserialize, Serialize};

/// Prefixe des identifiants de question.
pub const QUESTION_ID_PREFIX: &str = "que_";

pub fn is_valid_question_id(id: &str) -> bool {
    id.starts_with(QUESTION_ID_PREFIX)
}

/// Un choix propose a l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Option_ {
    /// Texte affiche, 1 a 5 mots.
    pub label: String,
    /// Explication du choix.
    pub description: String,
}

impl Option_ {
    pub fn new(label: impl Into<String>, description: impl Into<String>) -> Self {
        Self { label: label.into(), description: description.into() }
    }
}

/// Question posee a l'utilisateur.
///
/// `custom` autorise a taper une reponse libre. Le defaut du TS est `true` :
/// si le champ est absent, l'utilisateur peut repondre librement. On encode ce
/// defaut explicitement plutot que de laisser `None` ambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// Question complete.
    pub question: String,
    /// Etiquette tres courte, 30 caracteres maximum.
    pub header: String,
    /// Choix disponibles.
    pub options: Vec<Option_>,
    /// Autorise a selectionner plusieurs choix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiple: Option<bool>,
    /// Autorise a taper une reponse libre. Defaut `true` dans le TS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<bool>,
}

impl Info {
    pub fn new(question: impl Into<String>, header: impl Into<String>, options: Vec<Option_>) -> Self {
        Self { question: question.into(), header: header.into(), options, multiple: None, custom: None }
    }

    /// L'utilisateur peut-il repondre librement ?
    ///
    /// Renvoie `true` quand `custom` est absent, comme le fait le TS.
    pub fn allows_custom(&self) -> bool {
        self.custom.unwrap_or(true)
    }

    /// Une reponse libre est-elle possible pour cette question ?
    pub fn allows_custom_answer(&self) -> bool {
        self.allows_custom()
    }
}

/// Question sans la mention de reponse libre.
///
/// Le TS distingue `Prompt` (base, sans `custom`) de `Info` (avec `custom`).
/// On conserve la distinction plutot que de fusionner, pour rester fidele.
pub type Prompt = Info;

/// Origine de la question : elle vient d'un appel d'outil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "messageID")]
    pub message_id: String,
    #[serde(rename = "callID")]
    pub call_id: String,
}

/// Demande de question en attente de reponse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub id: String,
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub questions: Vec<Info>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<Tool>,
}

/// Reponse a **une** question : la liste des libelles choisis.
///
/// C'est un tableau, pas une chaine. Voir la note de module.
pub type Answer = Vec<String>;

/// Reponse a une demande : une `Answer` par question, dans l'ordre des
/// questions posees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub answers: Vec<Answer>,
}

impl Reply {
    /// Verifie que la reponse couvre bien toutes les questions.
    ///
    /// Le TS ne verifie pas : il fait confiance a l'appelant. Une reponse trop
    /// courte produirait un decalage silencieux, chaque question recevant la
    /// reponse de la precedente. On ajoute donc la verification, parce
    /// qu'un agent qui recoit une reponse decalee ne peut pas s'en apercevoir.
    pub fn is_complete(&self, questions: &[Info]) -> bool {
        self.answers.len() == questions.len()
    }

    /// Reponse a la question d'index donne.
    pub fn answer_for(&self, index: usize) -> Option<&Answer> {
        self.answers.get(index)
    }
}

/// Erreurs du mecanisme de question.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum QuestionError {
    /// L'utilisateur a ferme la boite de dialogue sans repondre.
    #[error("l'utilisateur a rejete cette question")]
    Rejected,
    /// Demande inconnue.
    #[error("demande de question introuvable : {request_id}")]
    NotFound { request_id: String },
}

/// Regroupe les demandes en attente.
#[derive(Debug, Default)]
pub struct Pending {
    items: Vec<Request>,
}

impl Pending {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, request: Request) -> bool {
        // Un identifiant en double est refuse, comme le `EffectRuntime.die`
        // du TS : c'est un bug de programmation, pas une condition d'usage.
        if self.items.iter().any(|r| r.id == request.id) {
            return false;
        }
        self.items.push(request);
        true
    }

    pub fn get(&self, id: &str) -> Option<&Request> {
        self.items.iter().find(|r| r.id == id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Request> {
        let index = self.items.iter().position(|r| r.id == id)?;
        Some(self.items.remove(index))
    }

    pub fn for_session(&self, session_id: &str) -> Vec<Request> {
        self.items.iter().filter(|r| r.session_id == session_id).cloned().collect()
    }

    pub fn list(&self) -> &[Request] {
        &self.items
    }

    /// Rejette toutes les demandes d'une session.
    ///
    /// Appele quand l'utilisateur refuse une question : les autres questions de
    /// la meme session n'ont plus de sens, elles portaient sur le meme contexte
    /// bloque. Les questions d'autres sessions sont laissees intactes.
    pub fn reject_session(&mut self, session_id: &str) -> usize {
        let before = self.items.len();
        self.items.retain(|r| r.session_id != session_id);
        before - self.items.len()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(q: &str) -> Info {
        Info::new(q, "H", vec![Option_::new("oui", "yes"), Option_::new("non", "no")])
    }

    fn request(id: &str, n: usize) -> Request {
        Request {
            id: id.to_string(),
            session_id: "ses_1".to_string(),
            questions: (0..n).map(|i| question(&format!("q{i}"))).collect(),
            tool: None,
        }
    }

    #[test]
    fn la_reponse_est_une_liste_par_question() {
        // La structure exacte du TS : un tableau de tableaux, dans l'ordre.
        let r = Reply { answers: vec![vec!["oui".into()], vec!["non".into(), "peut-etre".into()]] };
        let v = serde_json::to_value(&r).unwrap();
        assert!(v["answers"][0].is_array());
        assert_eq!(v["answers"][0][0], "oui");
        assert_eq!(v["answers"][1].as_array().unwrap().len(), 2);
    }

    #[test]
    fn une_reponse_trop_courte_est_detectee() {
        // Sans cette verification, la reponse de q0 partirait sur q1.
        let questions = vec![question("q0"), question("q1")];
        let complete = Reply { answers: vec![vec!["oui".into()], vec!["non".into()]] };
        assert!(complete.is_complete(&questions));

        let courte = Reply { answers: vec![vec!["oui".into()]] };
        assert!(!courte.is_complete(&questions));
    }

    #[test]
    fn l_acces_par_index_ne_decale_pas() {
        let r = Reply { answers: vec![vec!["oui".into()], vec!["non".into()]] };
        assert_eq!(r.answer_for(0).unwrap()[0], "oui");
        assert_eq!(r.answer_for(1).unwrap()[0], "non");
        assert!(r.answer_for(2).is_none());
    }

    #[test]
    fn la_reponse_libre_est_autorisee_par_defaut() {
        // Le TS dit `default: true` quand le champ est absent.
        let q = question("q");
        assert!(q.allows_custom_answer());
        assert!(q.allows_custom(), "l absence du champ vaut true, comme dans le TS");
    }

    #[test]
    fn la_reponse_libre_peut_etre_desactivee() {
        let mut q = question("q");
        q.custom = Some(false);
        assert!(!q.allows_custom_answer());
    }

    #[test]
    fn les_noms_de_champs_json_sont_en_camelcase() {
        // Piege deja vu deux fois dans ce portage.
        let r = Request {
            id: "que_1".into(),
            session_id: "ses_1".into(),
            questions: vec![],
            tool: Some(Tool { message_id: "msg_1".into(), call_id: "call_1".into() }),
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["sessionID"], "ses_1");
        assert_eq!(v["tool"]["messageID"], "msg_1");
        assert_eq!(v["tool"]["callID"], "call_1");
        assert!(v.get("session_id").is_none());
    }

    #[test]
    fn un_identifiant_en_double_est_refuse() {
        let mut p = Pending::new();
        assert!(p.push(request("que_1", 1)));
        assert!(!p.push(request("que_1", 1)), "identifiant en double refuse");
        assert_eq!(p.list().len(), 1);
    }

    #[test]
    fn rejeter_une_question_rejette_celle_de_la_meme_session() {
        let mut p = Pending::new();
        p.push(request("que_1", 1));
        p.push(Request { session_id: "ses_2".into(), ..request("que_2", 1) });

        assert_eq!(p.reject_session("ses_1"), 1);
        assert!(p.get("que_1").is_none());
        assert!(p.get("que_2").is_some(), "une autre session n est pas affectee");
    }

    #[test]
    fn les_demandes_sont_filtrees_par_session() {
        let mut p = Pending::new();
        p.push(request("que_1", 1));
        p.push(Request { session_id: "ses_2".into(), ..request("que_2", 1) });
        assert_eq!(p.for_session("ses_1").len(), 1);
        assert_eq!(p.for_session("ses_2").len(), 1);
    }

    #[test]
    fn le_prefixe_d_identifiant_est_verifie() {
        assert!(is_valid_question_id("que_abc"));
        assert!(!is_valid_question_id("msg_abc"));
    }
}
