//! Portage Rust de `opencode/packages/core/src/session/runner/index.ts`.
//!
//! La source fait 28 lignes et ne contient **aucune implementation** : pas de
//! `Layer`, pas de `resolve`, pas de corps de fonction. Elle declare trois
//! choses, et rien d'autre.
//!
//! 1. `export type RunError` : une union de six types d'erreur, tous definis
//!    ailleurs. C'est le canal d'echec du `run`.
//! 2. `export interface Interface` : une seule methode, `run`, qui prend un
//!    objet `{ sessionID, force }` et renvoie `Effect.Effect<void, RunError>`.
//! 3. `export class Service extends Context.Service<...>` : la classe porteuse
//!    du tag `"@opencode/v2/SessionRunner"`. Elle n'a aucun membre propre, donc
//!    en Rust elle se reduit a une constante, [`SERVICE_TAG`].
//!
//! ## Ce qui n'est pas traduit, et pourquoi
//!
//! - La ligne 1, `export * as SessionRunner from "./index"`, est un reexport de
//!   l'espace de noms du module sur LUI-MEME. C'est du code mort, comme le
//!   `export * as ConfigCommand from "./command"` rencontre ailleurs dans ce
//!   depot. Sans traduction.
//!
//! - Le graphe de couches de `Context.Service` n'a pas d'equivalent en Rust.
//!   Comme dans `session/execution.ts`, le tag est conserve, le mecanisme ne
//!   l'est pas.
//!
//! - Les **charges utiles** des six erreurs ne sont pas portees, et c'est un
//!   choix, pas une omission. Voir la section suivante.
//!
//! ## Pourquoi [`RunError`] n'a pas de donnee portee
//!
//! L'union de la source n'est pas une union de six classes : c'est une union
//! de six *types*, dont deux sont eux-memes des unions. Aplatie, elle compte
//! **dix** classes au total :
//!
//! - `LLMError` (`packages/llm/src/schema/errors.ts`, tag `"LLM.Error"`) est une
//!   seule classe, dont le champ `reason` est lui-meme une union de **dix**
//!   classes (`InvalidRequest`, `NoRoute`, `Authentication`, `RateLimit`,
//!   `QuotaExceeded`, `ContentPolicy`, `ProviderInternal`, `Transport`,
//!   `InvalidProviderOutput`, `UnknownProvider`). Ces dix raisons sont une
//!   donnee de `LLMError`, pas des membres de `RunError`.
//! - `SessionRunnerModel.Error` (`session/runner/model.ts:67`) est une union de
//!   **cinq** classes : `ModelNotSelected`, `ModelUnavailable`,
//!   `VariantUnavailable`, `UnsupportedApi` et `Integration.AuthorizationError`,
//!   la derniere appartenant a un autre paquet encore.
//! - `ToolOutputStore.Error` (`tool-output-store.ts:40`) ne contient qu'une
//!   classe, `StorageError`.
//!
//! Total : 1 (`LLMError`) + 5 (modele) + 1 (`MessageDecodeError`) + 1
//! (`ContextSnapshotDecodeError`) + 1 (`SystemContext.InitializationBlocked`) +
//! 1 (`StorageError`) = 10.
//!
//! Ecrire ces dix classes dans ce fichier reviendrait a recopier cinq modules
//! qui ne sont pas portes, et a creer deux definitions divergentes du meme
//! contrat qui ne casseraient qu'a l'echange avec le TypeScript. On porte donc
//! l'union au niveau ou elle est ecrite, c'est-a-dire au niveau de ses six
//! membres, chacun reconnaissable par le nom que la source lui donne.
//!
//! `RunError` doit devenir un enum **enveloppant** des que les modules
//! proprietaires existent : `RunError::Llm(LlmError)`, et non plus une
//! variante unitaire. C'est le point de vigilance principal de ce fichier.
//!
//! ## Ce qui est importe, et non reecrit
//!
//! `RunInput::session_id` est du type [`SessionId`], c'est-a-dire l'alias
//! `crate::core::session::schema::SessionId` (**importe**, pas redefini). La
//! source ecrit `SessionSchema.ID`, qui est `Session.ID` de
//! `packages/schema/src/session.ts`, une chaine brandee sans contrainte de
//! lecture cote Rust. Reduire cet alias a `String` dans ce fichier aurait cree
//! un deuxieme contrat du meme nom, susceptible de diverger en silence.
//! C'est exactement la faute que la relecture a deja attrapee sur les noms de
//! champs, et c'est la raison pour laquelle ce module n'a que trois types
//! propres : [`RunInput`], [`RunError`] et [`Interface`].
//!
//! ## Le nom du service
//!
//! Le trait s'appelle [`Interface`], comme dans le TypeScript. Il ne s'appelle
//! pas `Service` : `session/execution.ts` definit deja un `trait Service`, pour
//! un *autre* service (`"@opencode/v2/SessionExecution"`). Les confondre serait
//! le genre de divergence silencieuse que la relecture a deja attrape une fois
//! sur les noms de champs.
//!
//! ## Les noms de champs
//!
//! `sessionID` porte une majuscule. C'est le piege le plus courant de tout ce
//! portage, il est invisible de l'interieur du code Rust, et il ne se voit
//! qu'a la serialisation. D'ou le `#[serde(rename = "sessionID")]` pose
//! explicitement sur [`RunInput::session_id`], et le test qui compare la
//! sortie JSON champ par champ.
//!
//! Precison ce qui est un choix de portage : la source ne donne aucun schema
//! d'execution de [`RunInput`]. C'est un type TypeScript pur, verifie a la
//! compilation, et il ne traverse donc jamais de `JSON.parse` cote TypeScript.
//! Les derives `Serialize` / `Deserialize` sont un **pont** ajoute ici pour que
//! le nom de champ reste verifiable en test ; elles n'inventent pas de
//! contrainte. C'est pourquoi la lecture reste permissive sur un champ inconnu,
//! comme JavaScript, et stricte sur un champ manquant, puisque les deux sont
//! obligatoires.

use serde::{Deserialize, Serialize};

use crate::core::session::schema::SessionId;

/// Nom du service dans le graphe `Context.Service` d'origine.
///
/// La classe `Service` du TypeScript porte ce tag et rien d'autre : elle n'a
/// aucun membre, et le contrat qu'elle expose est [`Interface`].
pub const SERVICE_TAG: &str = "@opencode/v2/SessionRunner";

/// Objet d'entree de [`Interface::run`].
///
/// Les deux champs sont **obligatoires** : `force` est un `readonly boolean`,
/// pas un `readonly boolean | undefined`, et `sessionID` est marque
/// `readonly` sans `?`. Aucun des deux n'a donc de valeur par defaut, et le
/// decodage d'un objet dont il manque un doit echouer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunInput {
    /// Session a relancer. Ecran : `sessionID`, avec la majuscule.
    #[serde(rename = "sessionID")]
    pub session_id: SessionId,
    /// Execution explicite.
    ///
    /// Une execution explicite tente une fois le fournisseur meme quand aucun
    /// travail durable n'est eligible. Sans ce drapeau, `run` se contente de
    /// vider le travail eligible et peut ne rien faire du tout.
    pub force: bool,
}

impl RunInput {
    /// Construit une entree de `run`.
    pub fn new(session_id: impl Into<SessionId>, force: bool) -> Self {
        Self {
            session_id: session_id.into(),
            force,
        }
    }
}

/// Les six membres de l'union `RunError` de la source, dans l'ordre ou le
/// TypeScript les ecrit.
///
/// Les variantes sont unitaires : voir la documentation de module pour la
/// raison, qui est une contrainte de portage et non un raccourci.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunError {
    /// `LLMError`, tag `"LLM.Error"`.
    Llm,
    /// `SessionRunnerModel.Error` : union de cinq classes.
    Model,
    /// `MessageDecodeError`, tag `"Session.MessageDecodeError"`.
    MessageDecode,
    /// `ContextSnapshotDecodeError`, tag `"Session.ContextSnapshotDecodeError"`.
    ContextSnapshotDecode,
    /// `SystemContext.InitializationBlocked`, tag identique.
    SystemContextInitializationBlocked,
    /// `ToolOutputStore.Error` : l'union ne contient que `StorageError`.
    ToolOutputStore,
}

impl RunError {
    /// Les six noms de membres, dans l'ordre de la declaration TypeScript.
    pub const MEMBERS: [&'static str; 6] = [
        "LLMError",
        "SessionRunnerModel.Error",
        "MessageDecodeError",
        "ContextSnapshotDecodeError",
        "SystemContext.InitializationBlocked",
        "ToolOutputStore.Error",
    ];

    /// Nombre de membres de l'union. Le TypeScript en declare six.
    pub const LEN: usize = 6;

    /// Le nom du membre, ecrit exactement comme la source l'ecrit.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Llm => "LLMError",
            Self::Model => "SessionRunnerModel.Error",
            Self::MessageDecode => "MessageDecodeError",
            Self::ContextSnapshotDecode => "ContextSnapshotDecodeError",
            Self::SystemContextInitializationBlocked => "SystemContext.InitializationBlocked",
            Self::ToolOutputStore => "ToolOutputStore.Error",
        }
    }

    /// Retrouve le membre a partir du nom de la source.
    ///
    /// La comparaison est exacte : `"llmerror"` ou `"LLM Error"` ne sont pas
    /// reconnus, parce que la source distingue la casse et le separateur.
    ///
    /// Cette fonction ne s'appelle pas `from_str` volontairement : ce nom est
    /// reserve au trait `std::str::FromStr`, dont la signature n'a rien a voir
    /// (elle rend un `Result`, pas un `Option`).
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "LLMError" => Some(Self::Llm),
            "SessionRunnerModel.Error" => Some(Self::Model),
            "MessageDecodeError" => Some(Self::MessageDecode),
            "ContextSnapshotDecodeError" => Some(Self::ContextSnapshotDecode),
            "SystemContext.InitializationBlocked" => Some(Self::SystemContextInitializationBlocked),
            "ToolOutputStore.Error" => Some(Self::ToolOutputStore),
            _ => None,
        }
    }
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for RunError {}

/// Contrat du service `@opencode/v2/SessionRunner`.
///
/// La source ne fournit aucune implementation de cette interface : elle vit
/// dans `runner/llm.ts`. Ce fichier ne fabrique donc pas de `Layer`, et les
/// tests utilisent un double local.
pub trait Interface {
    /// Execute une continuation locale depuis l'historique deja enregistre.
    ///
    /// La source : "Vide le travail durable eligible. Une execution explicite
    /// tente une fois le fournisseur meme quand aucun travail n'est
    /// eligible."
    fn run(&self, input: RunInput) -> Result<(), RunError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Double local : enregistre les entrees et reussit toujours.
    #[derive(Default)]
    struct Journal {
        entrees: std::cell::RefCell<Vec<RunInput>>,
    }

    impl Interface for Journal {
        fn run(&self, input: RunInput) -> Result<(), RunError> {
            self.entrees.borrow_mut().push(input);
            Ok(())
        }
    }

    /// Double local : echoue toujours, sur le membre demande.
    struct Echec(RunError);

    impl Interface for Echec {
        fn run(&self, _input: RunInput) -> Result<(), RunError> {
            Err(self.0)
        }
    }

    #[test]
    fn la_serialisation_ecrit_session_id_avec_la_majuscule() {
        let entree = RunInput::new("ses_01", true);
        let json = serde_json::to_string(&entree).unwrap();
        assert_eq!(json, r#"{"sessionID":"ses_01","force":true}"#);
    }

    #[test]
    fn la_serialisation_n_ecrit_jamais_la_forme_minuscule_du_champ() {
        let json = serde_json::to_string(&RunInput::new("ses_01", false)).unwrap();
        // `sessionId` est la faute classique : invisible a la compilation,
        // visible uniquement a l'echange avec le TypeScript.
        assert!(!json.contains("sessionId"));
        assert!(json.contains("sessionID"));
        // `force` est deja en minuscules des deux cotes, donc sans renommage.
        assert!(json.contains("\"force\""));
    }

    #[test]
    fn la_relecture_json_restaure_le_meme_objet_avec_la_casse_du_type_script() {
        // Ordre inverse des cles : l'ordre de lecture ne doit rien changer.
        let depuis_json: RunInput =
            serde_json::from_str(r#"{"force":false,"sessionID":"ses_42"}"#).unwrap();
        assert_eq!(depuis_json, RunInput::new("ses_42", false));
    }

    #[test]
    fn une_entree_sans_identifiant_de_session_est_refusee() {
        // `sessionID` n'est pas optionnel dans la source.
        let erreur = serde_json::from_str::<RunInput>(r#"{"force":true}"#);
        assert!(erreur.is_err());
    }

    #[test]
    fn une_entree_sans_force_est_refusee() {
        // `force` n'est pas optionnel non plus : pas de valeur par defaut.
        let erreur = serde_json::from_str::<RunInput>(r#"{"sessionID":"ses_01"}"#);
        assert!(erreur.is_err());
    }

    #[test]
    fn un_identifiant_vide_reste_une_chaine_vide() {
        // Le type `sessionID` ne filtre pas la chaine vide. La distinguer d'un
        // identifiant absent (`None`) serait inventer une validation.
        let entree: RunInput = serde_json::from_str(r#"{"sessionID":"","force":true}"#).unwrap();
        assert_eq!(entree.session_id, "");
        assert!(entree.force);
        let json = serde_json::to_string(&entree).unwrap();
        assert_eq!(json, r#"{"sessionID":"","force":true}"#);
    }

    #[test]
    fn un_champ_inconnu_est_ignore_comme_en_javascript() {
        // Aucun decodage au runtime n'existe cote TypeScript pour cet objet :
        // une propriete de trop y passe inapercue, et ne doit donc pas
        // faire echouer la lecture.
        let entree: RunInput =
            serde_json::from_str(r#"{"sessionID":"ses_01","force":true,"step":3}"#).unwrap();
        assert_eq!(entree, RunInput::new("ses_01", true));
    }

    #[test]
    fn un_run_reussit_et_transmet_son_entree_sans_la_modifier() {
        let journal = Journal::default();
        let entree = RunInput::new("ses_07", true);
        let resultat = journal.run(entree.clone());
        assert!(resultat.is_ok());
        assert_eq!(journal.entrees.borrow().as_slice(), &[entree]);
    }

    #[test]
    fn un_run_echoue_avec_le_membre_d_erreur_transmis() {
        let service = Echec(RunError::MessageDecode);
        let resultat = service.run(RunInput::new("ses_07", false));
        assert_eq!(resultat, Err(RunError::MessageDecode));
        // Le message est le nom du membre, tel qu'ecrit dans la source.
        assert_eq!(resultat.unwrap_err().to_string(), "MessageDecodeError");
    }

    #[test]
    fn l_erreur_de_run_declenche_ses_six_membres_dans_l_ordre_de_la_source() {
        assert_eq!(RunError::LEN, 6);
        let vus: Vec<&str> = [
            RunError::Llm,
            RunError::Model,
            RunError::MessageDecode,
            RunError::ContextSnapshotDecode,
            RunError::SystemContextInitializationBlocked,
            RunError::ToolOutputStore,
        ]
        .iter()
        .map(|membre| membre.as_str())
        .collect();
        assert_eq!(vus, RunError::MEMBERS);
    }

    #[test]
    fn un_membre_d_erreur_se_retrouve_par_son_nom_avec_la_casse_exacte() {
        for nom in RunError::MEMBERS {
            let membre = RunError::from_name(nom).expect("nom issu de MEMBERS");
            assert_eq!(membre.as_str(), nom);
        }
        // La casse et le separateur ne sont pas normalises.
        assert_eq!(RunError::from_name("llmerror"), None);
        assert_eq!(RunError::from_name("LLM Error"), None);
        assert_eq!(RunError::from_name(""), None);
        assert_eq!(RunError::from_name("SessionRunner.Error"), None);
    }

    #[test]
    fn le_nom_du_service_est_ceux_du_type_script() {
        assert_eq!(SERVICE_TAG, "@opencode/v2/SessionRunner");
        // Distinct de celui du service d'execution, qui est un autre service.
        assert_ne!(SERVICE_TAG, "@opencode/v2/SessionExecution");
    }
}
