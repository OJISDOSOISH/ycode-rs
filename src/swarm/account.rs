//! Portage de `packages/core/src/account.ts`.
//!
//! La source fait quatre-vingt-dix lignes : des marques de chaines (`ID`,
//! `OrgID`, jetons, codes), deux classes de donnees (`Info`, `Org`), trois
//! erreurs (`AccountRepoError`, `AccountServiceError`,
//! `AccountTransportError` avec sa fabrique et son message), un `Login`, et
//! six variantes de resultat de scrutation (`PollSuccess`, `PollPending`,
//! `PollSlow`, `PollExpired`, `PollDenied`, `PollError`) reunies en
//! `PollResult`. Aucun test TypeScript.
//!
//! Les marques `Schema.brand` n ont aucun effet a l execution : ce sont des
//! chaines. Elles sont portees en nouveaux types transparents pour garder la
//! distinction sans changer le JSON (qui reste une chaine). Les classes
//! `Schema.Class` deviennent des structures, les `TaggedClass` un enumere.

use serde::{Deserialize, Serialize};

/// Marque `AccountID` : une chaine opaque cote JSON.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(pub String);

/// Marque `OrgID`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrgId(pub String);

/// Marque `AccessToken`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccessToken(pub String);

/// Marque `RefreshToken`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RefreshToken(pub String);

/// Marque `DeviceCode`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceCode(pub String);

/// Marque `UserCode`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserCode(pub String);

/// `Info` : le compte tel que l API le renvoie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountInfo {
    /// `id: ID`.
    pub id: AccountId,
    /// `email: string`.
    pub email: String,
    /// `url: string`.
    pub url: String,
    /// `active_org_id: OrgID | null`.
    pub active_org_id: Option<OrgId>,
}

impl AccountInfo {
    /// Construit une fiche de compte.
    pub fn new(id: AccountId, email: String, url: String, active_org_id: Option<OrgId>) -> Self {
        Self { id, email, url, active_org_id }
    }
}

/// `Org` : une organisation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Org {
    /// `id: OrgID`.
    pub id: OrgId,
    /// `name: string`.
    pub name: String,
}

impl Org {
    /// Construit une organisation.
    pub fn new(id: OrgId, name: String) -> Self {
        Self { id, name }
    }
}

/// `AccountRepoError` : `{ message, cause? }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRepoError {
    /// Message d erreur.
    pub message: String,
    /// Cause textuelle, si presente. La source accepte tout defaut ; ici
    /// seul le texte est conserve, sans inventer de charge.
    pub cause: Option<String>,
}

impl AccountRepoError {
    /// Construit l erreur.
    pub fn new(message: String, cause: Option<String>) -> Self {
        Self { message, cause }
    }
}

/// `AccountServiceError` : `{ message, cause? }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountServiceError {
    /// Message d erreur.
    pub message: String,
    /// Cause textuelle, si presente.
    pub cause: Option<String>,
}

impl AccountServiceError {
    /// Construit l erreur.
    pub fn new(message: String, cause: Option<String>) -> Self {
        Self { message, cause }
    }
}

/// `AccountTransportError` : `{ method, url, description?, cause? }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountTransportError {
    /// Methode HTTP de la requete qui n a pas abouti.
    pub method: String,
    /// URL appelee.
    pub url: String,
    /// Description fournie par le client HTTP, si presente.
    pub description: Option<String>,
    /// Cause textuelle, si presente.
    pub cause: Option<String>,
}

impl AccountTransportError {
    /// Construit l erreur.
    pub fn new(method: String, url: String, description: Option<String>, cause: Option<String>) -> Self {
        Self { method, url, description, cause }
    }

    /// Le `message` calcule de la source : quatre phrases jointes par des
    /// retours a la ligne, la description etant sautee quand elle est absente.
    pub fn message(&self) -> String {
        let mut lignes = vec![
            format!("Could not reach {} {}.", self.method, self.url),
            "This failed before the server returned an HTTP response.".to_string(),
        ];
        if let Some(description) = &self.description {
            if !description.is_empty() {
                lignes.push(description.clone());
            }
        }
        lignes.push("Check your network, proxy, or VPN configuration and try again.".to_string());
        lignes.join("\n")
    }
}

/// Les trois erreurs reunies, comme le type `AccountError` de la source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountError {
    /// Erreur de depot.
    Repo(AccountRepoError),
    /// Erreur de service.
    Service(AccountServiceError),
    /// Erreur de transport.
    Transport(AccountTransportError),
}

/// `Login` : la session d enregistrement d appareil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Login {
    /// `code: DeviceCode`.
    pub code: DeviceCode,
    /// `user: UserCode`.
    pub user: UserCode,
    /// `url: string`.
    pub url: String,
    /// `server: string`.
    pub server: String,
    /// Delai avant expiration, en millisecondes. La source utilise
    /// `Schema.Duration` ; sans crate de duree, l entier est l unite
    /// transportee par le JSON.
    pub expiry_ms: i64,
    /// Delai entre deux scrutations, en millisecondes.
    pub interval_ms: i64,
}

impl Login {
    /// Construit une session de connexion.
    pub fn new(code: DeviceCode, user: UserCode, url: String, server: String, expiry_ms: i64, interval_ms: i64) -> Self {
        Self { code, user, url, server, expiry_ms, interval_ms }
    }
}

/// Le resultat de scrutation `PollResult`, porte en enumere.
///
/// Les six variantes reprennent les six `TaggedClass` de la source. La charge
/// de `PollError` (`Schema.Defect()`, donc n importe quoi) est conservee en
/// texte, comme pour les erreurs ci-dessus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PollResult {
    /// `PollSuccess` : `{ email }`.
    #[serde(rename = "PollSuccess")]
    Success {
        /// Adresse validee.
        email: String,
    },
    /// `PollPending` : en attente.
    #[serde(rename = "PollPending")]
    Pending,
    /// `PollSlow` : ralentir la scrutation.
    #[serde(rename = "PollSlow")]
    Slow,
    /// `PollExpired` : code expire.
    #[serde(rename = "PollExpired")]
    Expired,
    /// `PollDenied` : connexion refusee.
    #[serde(rename = "PollDenied")]
    Denied,
    /// `PollError` : echec technique.
    #[serde(rename = "PollError")]
    Error {
        /// Cause textuelle.
        cause: String,
    },
}

impl PollResult {
    /// Vrai si et seulement si la scrutation a reussi.
    pub fn est_un_succes(&self) -> bool {
        matches!(self, PollResult::Success { .. })
    }

    /// Vrai si la scrutation doit continuer (`Pending` ou `Slow`).
    pub fn doit_continuer(&self) -> bool {
        matches!(self, PollResult::Pending | PollResult::Slow)
    }

    /// Vrai si la scrutation est definitivement terminee en echec
    /// (`Expired`, `Denied` ou `Error`).
    pub fn est_terminee_en_echec(&self) -> bool {
        matches!(self, PollResult::Expired | PollResult::Denied | PollResult::Error { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_marques_restent_des_chaines_en_json() {
        assert_eq!(serde_json::to_value(AccountId("a1".to_string())).unwrap(), "a1");
        assert_eq!(serde_json::to_value(OrgId("o1".to_string())).unwrap(), "o1");
        assert_eq!(serde_json::to_value(AccessToken("x".to_string())).unwrap(), "x");
        assert_eq!(serde_json::to_value(RefreshToken("y".to_string())).unwrap(), "y");
        assert_eq!(serde_json::to_value(DeviceCode("d".to_string())).unwrap(), "d");
        assert_eq!(serde_json::to_value(UserCode("u".to_string())).unwrap(), "u");
    }

    #[test]
    fn la_fiche_de_compte_serialise_ses_quatre_champs_en_snake_case() {
        let fiche = AccountInfo::new(
            AccountId("a1".to_string()),
            "a@b.c".to_string(),
            "https://ex".to_string(),
            Some(OrgId("o1".to_string())),
        );
        let json = serde_json::to_value(&fiche).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 4);
        assert_eq!(json["id"], "a1");
        assert_eq!(json["email"], "a@b.c");
        assert_eq!(json["url"], "https://ex");
        assert_eq!(json["active_org_id"], "o1");
    }

    #[test]
    fn une_fiche_sans_organisation_active_serialise_un_null() {
        let fiche = AccountInfo::new(AccountId("a1".to_string()), "a@b.c".to_string(), "https://ex".to_string(), None);
        let json = serde_json::to_value(&fiche).unwrap();
        assert!(json["active_org_id"].is_null());
        let relue: AccountInfo = serde_json::from_value(json).unwrap();
        assert_eq!(relue.active_org_id, None);
    }

    #[test]
    fn l_organisation_serialise_ses_deux_champs() {
        let org = Org::new(OrgId("o1".to_string()), "Equipe".to_string());
        let json = serde_json::to_value(&org).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 2);
        assert_eq!(json["id"], "o1");
        assert_eq!(json["name"], "Equipe");
    }

    #[test]
    fn les_erreurs_de_depot_et_de_service_portent_message_et_cause() {
        let depot = AccountRepoError::new("panne".to_string(), Some("sqlite".to_string()));
        assert_eq!(depot.message, "panne");
        assert_eq!(depot.cause.as_deref(), Some("sqlite"));
        let service = AccountServiceError::new("panne".to_string(), None);
        assert_eq!(service.cause, None);
        let json = serde_json::to_value(&service).unwrap();
        assert!(json["cause"].is_null());
    }

    #[test]
    fn le_message_de_transport_joint_quatre_phrases_sans_description() {
        let erreur = AccountTransportError::new("GET".to_string(), "https://ex".to_string(), None, None);
        let message = erreur.message();
        let lignes: Vec<&str> = message.split('\n').collect();
        assert_eq!(lignes.len(), 3);
        assert!(lignes[0].contains("Could not reach GET https://ex."));
        assert!(lignes[1].contains("before the server returned"));
        assert!(lignes[2].contains("proxy"));
    }

    #[test]
    fn le_message_de_transport_insere_la_description_quand_presente() {
        let erreur = AccountTransportError::new(
            "POST".to_string(),
            "https://ex/token".to_string(),
            Some("reset by peer".to_string()),
            None,
        );
        let lignes: Vec<&str> = erreur.message().split('\n').collect();
        assert_eq!(lignes.len(), 4);
        assert_eq!(lignes[2], "reset by peer");
    }

    #[test]
    fn une_description_vide_ne_fait_pas_de_ligne_supplementaire() {
        let erreur = AccountTransportError::new("GET".to_string(), "https://ex".to_string(), Some(String::new()), None);
        assert_eq!(erreur.message().split('\n').count(), 3);
    }

    #[test]
    fn la_connexion_serialise_ses_six_champs() {
        let login = Login::new(
            DeviceCode("d".to_string()),
            UserCode("u".to_string()),
            "https://ex".to_string(),
            "https://srv".to_string(),
            900_000,
            5_000,
        );
        let json = serde_json::to_value(&login).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 6);
        assert_eq!(json["code"], "d");
        assert_eq!(json["user"], "u");
        assert_eq!(json["expiry_ms"], 900_000);
        assert_eq!(json["interval_ms"], 5_000);
    }

    #[test]
    fn chaque_variante_de_scrutation_porte_son_propre_type() {
        let cas: Vec<(PollResult, &str)> = vec![
            (PollResult::Success { email: "a@b.c".to_string() }, "PollSuccess"),
            (PollResult::Pending, "PollPending"),
            (PollResult::Slow, "PollSlow"),
            (PollResult::Expired, "PollExpired"),
            (PollResult::Denied, "PollDenied"),
            (PollResult::Error { cause: "boom".to_string() }, "PollError"),
        ];
        for (resultat, etiquette) in &cas {
            let json = serde_json::to_value(resultat).unwrap();
            assert_eq!(json["type"], *etiquette);
        }
        assert_eq!(cas.len(), 6);
    }

    #[test]
    fn le_succes_transporte_l_adresse_validee() {
        let resultat = PollResult::Success { email: "a@b.c".to_string() };
        assert!(resultat.est_un_succes());
        assert!(!resultat.doit_continuer());
        assert!(!resultat.est_terminee_en_echec());
        assert_eq!(serde_json::to_value(&resultat).unwrap()["email"], "a@b.c");
    }

    #[test]
    fn seules_l_attente_et_le_ralentissement_continuent() {
        assert!(PollResult::Pending.doit_continuer());
        assert!(PollResult::Slow.doit_continuer());
        assert!(!PollResult::Expired.doit_continuer());
        assert!(!PollResult::Denied.doit_continuer());
        assert!(!PollResult::Error { cause: "x".to_string() }.doit_continuer());
    }

    #[test]
    fn expiration_refus_et_erreur_sont_des_echecs_definitifs() {
        assert!(PollResult::Expired.est_terminee_en_echec());
        assert!(PollResult::Denied.est_terminee_en_echec());
        assert!(PollResult::Error { cause: "x".to_string() }.est_terminee_en_echec());
        assert!(!PollResult::Pending.est_terminee_en_echec());
        assert!(!PollResult::Slow.est_terminee_en_echec());
        assert!(!PollResult::Success { email: "a@b.c".to_string() }.est_terminee_en_echec());
    }

    #[test]
    fn l_erreur_de_scrutation_transporte_sa_cause_en_texte() {
        let resultat = PollResult::Error { cause: "boom".to_string() };
        let json = serde_json::to_value(&resultat).unwrap();
        assert_eq!(json["cause"], "boom");
        let relue: PollResult = serde_json::from_value(json).unwrap();
        assert_eq!(relue, resultat);
    }

    #[test]
    fn l_enumere_des_erreurs_distribue_les_trois_cas() {
        let cas = AccountError::Transport(AccountTransportError::new("GET".to_string(), "https://ex".to_string(), None, None));
        assert!(matches!(cas, AccountError::Transport(_)));
        let cas = AccountError::Repo(AccountRepoError::new("m".to_string(), None));
        assert!(matches!(cas, AccountError::Repo(_)));
        let cas = AccountError::Service(AccountServiceError::new("m".to_string(), None));
        assert!(matches!(cas, AccountError::Service(_)));
    }
}
