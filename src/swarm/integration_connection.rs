//! Portage de `packages/core/src/integration/connection.ts`.
//!
//! ## Ce que la source est vraiment
//!
//! Le fichier TypeScript d'origine fait douze lignes et ne contient aucune
//! logique. Il se resume a un reexport de trois types venus du paquet
//! `@opencode-ai/schema` :
//!
//! ```ts
//! export * as IntegrationConnection from "./connection"
//!
//! import { Connection } from "@opencode-ai/schema/connection"
//!
//! export const CredentialInfo = Connection.CredentialInfo
//! export type CredentialInfo = Connection.CredentialInfo
//!
//! export const EnvInfo = Connection.EnvInfo
//! export type EnvInfo = Connection.EnvInfo
//!
//! export const Info = Connection.Info
//! export type Info = Connection.Info
//! ```
//!
//! Deux consequences a retenir avant de lire la suite.
//!
//! - `export * as IntegrationConnection from "./connection"` est le reexport
//!   d'espace de noms que le projet opencode pose en tete de presque chaque
//!   fichier. Il pointe sur le fichier lui-meme, donc il n'apporte rien : en
//!   Rust le module `integration_connection` est deja cet espace de noms. Il n'y
//!   a donc rien a retranscrire pour cette ligne.
//! - Les trois paires `export const X` / `export type X` sont la meme
//!   declaration vue sous deux angles : la valeur est le schema, le type est la
//!   forme des donnees. Le schema TypeScript n'existe pas a l'execution sous
//!   cette forme, il se resout en un simple type de donnees. Il devient donc un
//!   `struct` derive `Serialize, Deserialize`.
//!
//! Aucun autre agent du lot ne porte `packages/schema/src/connection.ts` (seul
//! le fichier `integration_connection` est reclame). Si on n'ecrivait rien,
//! les trois types disparaitraient completement du projet. On porte donc les
//! formes des donnees, en restant dans la stricte limite de ce que la source
//! declare.
//!
//! ## Les formes portees
//!
//! Le schema d'origine, dans `packages/schema/src/connection.ts`, est :
//!
//! ```ts
//! CredentialInfo = Schema.Struct({ type: Literal("credential"), id: Credential.ID, label: String })
//! EnvInfo       = Schema.Struct({ type: Literal("env"), name: String })
//! Info          = Union([CredentialInfo, EnvInfo]).pipe(Schema.toTaggedUnion("type"))
//! ```
//!
//! ## Secrets
//!
//! Cette source ne contient aucun secret et n'en materialise aucun. Elle
//! decrit la *forme* d'un renvoi de connexion, jamais une valeur reelle : les
//! champs sont `id`, `label` et `name`, c'est a dire un identifiant, une
//! etiquette et un nom de variable. Les valeurs sensibles elles-memes (jeton
//! d'acces, cle, donnees OAuth) vivent dans `Credential.Value`, un autre
//! schema, qui n'est pas porte ici. Aucun exemple de jeton n'est donc ecrit
//! dans ce fichier, ni dans ses tests.
//!
//! ## Pieges
//!
//! - **Aucun nom de champ en camelCase.** Les quatre noms de la source
//!   (`type`, `id`, `label`, `name`) sont des mots uniques en minuscules, donc
//!   le nom Rust est deja identique au nom TypeScript. Aucun `serde(rename)`
//!   n'est requis, et c'est verifie par un test. Le piege `projectID` contre
//!   `projectId` ne se pose pas ici.
//! - **Le champ `type` est un mot cle reserve en Rust.** Il porte donc un
//!   `#[serde(rename = "type")]` explicite et se nomme `kind` en Rust, comme
//!   le fait deja `src/schema/session_message.rs`.
//! - **Aucun ternaire `?` et aucun coalescent `??` dans la source.** Il n'y a
//!   donc aucune divergence possible entre chaine vide et `undefined`. Une
//!   chaine vide envoyee par le TypeScript est une chaine valide et survit des
//!   deux cotes : c'est le comportement correct, verifie par un test.
//! - **Aucun champ optionnel.** Tous les champs sont obligatoires dans le
//!   `Schema.Struct` d'origine, donc tous sont des `String` ou des newtypes
//!   ici, et aucun `skip_serializing_if` n'est pose. Un objet auquel il
//!   manque `label` ou `name` doit etre refuse, exactement comme en
//!   TypeScript.
//! - **`Credential.ID` ne valide rien.** En TypeScript c'est un `Schema.String`
//!   suivi d'un `Schema.brand`, c'est a dire une simple chaine distinguee au
//!   moment de la compilation. Aucun filtre de prefixe n'est applique a la
//!   lecture, donc on n'en met pas non plus en Rust : le newtype est
//!   transparent et accepte n'importe quelle chaine.

use serde::{Deserialize, Serialize};

/// Identifiants de schema, tels qu'ils apparaissent dans l'introspection
/// `effect/Schema` cote serveur OpenCode.
///
/// La source pose ces noms via `.annotate({ identifier: ... })`. Ils n'ont
/// aucune contrepartie dans la structure des donnees, mais on les conserve pour
/// que la correspondance avec le TypeScript reste lisible a la relecture.
pub const SCHEMA_IDENTIFIER_CREDENTIAL_INFO: &str = "Connection.CredentialInfo";
pub const SCHEMA_IDENTIFIER_ENV_INFO: &str = "Connection.EnvInfo";
pub const SCHEMA_IDENTIFIER_INFO: &str = "Connection.Info";

/// Prefixe construit par l'usine d'identifiants du paquet `schema`.
///
/// En TypeScript, `Credential.ID` expose un statique `create` qui produit
/// `"cred_" + ascending()`. Le prefixe est une convention de construction, pas
/// une regle de lecture : le schema ne refuse pas une chaine qui ne le porte
/// pas. Il est donc documente ici, et applique par
/// [`CredentialId::nouveau_avec_prefixe`], mais jamais verifie a la
/// deserialization.
pub const PREFIXE_IDENTIFIANT_CREDENTIAL: &str = "cred_";

/// Identifiant de credential.
///
/// En TypeScript : `Schema.String.pipe(Schema.brand("Credential.ID"))`.
///
/// Le "brand" du TypeScript est une verification de type a la compilation, qui
/// n'existe pas dans le JSON. On en fait un newtype `transparent` : il empeche
/// Rust de confondre un identifiant de credential avec un nom de variable
/// d'environnement, tout en se serialisant exactement comme une chaine, ce que
/// le TypeScript produit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CredentialId(pub String);

impl CredentialId {
    /// Construit un identifiant a partir de n'importe quelle chaine.
    ///
    /// A la difference du statique `create` du TypeScript, le prefixe n'est
    /// pas ajoute ici : le nom de la fonction dit ce qu'elle fait.
    pub fn nouveau(valeur: impl Into<String>) -> Self {
        Self(valeur.into())
    }

    /// Construit un identifiant avec le prefixe `cred_`, comme le statique
    /// `create` du TypeScript.
    pub fn nouveau_avec_prefixe(valeur: impl Into<String>) -> Self {
        // Le type est fixe explicitement : sans cette liaison, le compilateur
        // ne peut pas deduire la cible de `into()`.
        let valeur: String = valeur.into();
        Self(format!("{}{}", PREFIXE_IDENTIFIANT_CREDENTIAL, valeur))
    }

    /// La chaine portee par l'identifiant, sans copie.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Rend la chaine portee par l'identifiant.
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// La seule valeur possible du champ `type` d'un [`CredentialInfo`].
///
/// En TypeScript : `Schema.Literal("credential")`. Un litteral est une valeur
/// unique imposee par le type, ce qui devient un `enum` a une seule variante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeCredential {
    /// Le seul litteral accepte : `"credential"`.
    #[serde(rename = "credential")]
    Credential,
}

impl Default for TypeCredential {
    fn default() -> Self {
        Self::Credential
    }
}

/// La seule valeur possible du champ `type` d'un [`EnvInfo`].
///
/// En TypeScript : `Schema.Literal("env")`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TypeEnv {
    /// Le seul litteral accepte : `"env"`.
    #[serde(rename = "env")]
    Env,
}

impl Default for TypeEnv {
    fn default() -> Self {
        Self::Env
    }
}

/// Description d'une connexion a un credential enregistre.
///
/// En TS : `Connection.CredentialInfo`.
///
/// Les trois champs sont obligatoires. Aucun ne contient de secret : `id`
/// designe l'element dans le magasin, `label` est le texte affiche a
/// l'utilisateur. Le contenu sensible du credential n'est pas expose ici.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialInfo {
    /// Champ `type` du TypeScript, toujours `"credential"`.
    #[serde(rename = "type")]
    pub kind: TypeCredential,

    /// Identifiant du credential dans le magasin.
    pub id: CredentialId,

    /// Etiquette affichee a l'utilisateur.
    pub label: String,
}

impl CredentialInfo {
    /// Construit une description de credential avec le tag par defaut.
    pub fn nouveau(id: CredentialId, label: impl Into<String>) -> Self {
        Self {
            kind: TypeCredential::Credential,
            id,
            label: label.into(),
        }
    }
}

/// Description d'une connexion a une variable d'environnement.
///
/// En TS : `Connection.EnvInfo`.
///
/// Les deux champs sont obligatoires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvInfo {
    /// Champ `type` du TypeScript, toujours `"env"`.
    #[serde(rename = "type")]
    pub kind: TypeEnv,

    /// Nom de la variable d'environnement.
    pub name: String,
}

impl EnvInfo {
    /// Construit une description de variable d'environnement avec le tag par
    /// defaut.
    pub fn nouveau(name: impl Into<String>) -> Self {
        Self {
            kind: TypeEnv::Env,
            name: name.into(),
        }
    }
}

/// Union taggee des deux formes de connexion.
///
/// En TS :
/// `Schema.Union([CredentialInfo, EnvInfo]).pipe(Schema.toTaggedUnion("type"))`.
///
/// Le tag est le champ `type`, et il se trouve deja dans le payload de chacune
/// des deux formes. C'est pour cela que l'union est `untagged` et non
/// `#[serde(tag = "type")]` : avec un tag serde ajoute, la serialisation
/// ecrirait deux fois la cle `type`, une fois ajoutee par l'enum et une fois
/// produite par le struct. Les deux formes se trient donc sur la valeur de
/// `kind`, ce que le litteral de chaque struct verifie deja. Le JSON produit
/// est identique, sans cle en double.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Info {
    /// Connexion a un credential enregistre.
    Credential(CredentialInfo),

    /// Connexion a une variable d'environnement.
    Env(EnvInfo),
}

impl Info {
    /// Le tag de la variante, c'est a dire la valeur du champ `type`.
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Credential(_) => "credential",
            Self::Env(_) => "env",
        }
    }
}

impl From<CredentialInfo> for Info {
    fn from(valeur: CredentialInfo) -> Self {
        Self::Credential(valeur)
    }
}

impl From<EnvInfo> for Info {
    fn from(valeur: EnvInfo) -> Self {
        Self::Env(valeur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Aucun test de ce fichier n'ecrit de jeton, de cle ni de donnee OAuth :
    // seule la forme des donnees est portee, pas leur contenu.

    /// Une description de credential envoyee par le TypeScript se relit
    /// integralement, avec ses trois champs.
    #[test]
    fn une_info_de_credential_se_deserialize_avec_son_identifiant_et_son_libelle() {
        let info: CredentialInfo =
            serde_json::from_str(r#"{"type":"credential","id":"cred_1","label":"Compte principal"}"#)
                .unwrap();

        assert_eq!(info.kind, TypeCredential::Credential);
        assert_eq!(info.id.as_str(), "cred_1");
        assert_eq!(info.label, "Compte principal");
    }

    /// Une description de variable d'environnement se relit avec son nom.
    #[test]
    fn une_info_de_variable_d_environnement_se_deserialize_avec_son_nom() {
        let info: EnvInfo = serde_json::from_str(r#"{"type":"env","name":"OPENCODE_KEY"}"#).unwrap();

        assert_eq!(info.kind, TypeEnv::Env);
        assert_eq!(info.name, "OPENCODE_KEY");
    }

    /// Les noms de champs produits sont exactement ceux du TypeScript, y
    /// compris le champ `type` qui s'appelle `kind` en Rust. Ce test verrouille
    /// l'echange.
    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        let credential = CredentialInfo::nouveau(CredentialId::nouveau("cred_1"), "Compte");
        let objet = serde_json::to_value(&credential).unwrap();
        let champs = objet.as_object().unwrap();

        assert_eq!(champs.len(), 3);
        assert_eq!(champs.get("type").and_then(|v| v.as_str()), Some("credential"));
        assert_eq!(champs.get("id").and_then(|v| v.as_str()), Some("cred_1"));
        assert_eq!(champs.get("label").and_then(|v| v.as_str()), Some("Compte"));

        let env = EnvInfo::nouveau("OPENCODE_KEY");
        let objet = serde_json::to_value(&env).unwrap();
        let champs = objet.as_object().unwrap();

        assert_eq!(champs.len(), 2);
        assert_eq!(champs.get("type").and_then(|v| v.as_str()), Some("env"));
        assert_eq!(champs.get("name").and_then(|v| v.as_str()), Some("OPENCODE_KEY"));

        // Aucun champ en camelCase dans cette source : `type`, `id`, `label`
        // et `name` sont les seuls noms, et ils sont ecrits ainsi.
        assert!(!champs.contains_key("kind"));
        assert!(!champs.contains_key("Kind"));
    }

    /// L'identifiant de credential est une chaine nue dans le JSON, pas un
    /// objet : le newtype est transparent, comme la chaine brandee du
    /// TypeScript.
    #[test]
    fn un_identifiant_de_credential_s_serialise_comme_une_chaine_nue() {
        let json = serde_json::to_value(CredentialId::nouveau("cred_7")).unwrap();
        assert_eq!(json, serde_json::Value::String("cred_7".to_string()));

        let relu: CredentialId = serde_json::from_str(r#""cred_7""#).unwrap();
        assert_eq!(relu.as_str(), "cred_7");
    }

    /// L'identifiant de credential n'est pas valide : une chaine vide ou une
    /// chaine sans le prefixe passe quand meme, parce que le schema
    /// TypeScript n'applique aucun filtre de prefixe a la lecture.
    #[test]
    fn un_identifiant_de_credential_sans_prefixe_est_tout_de_meme_accepte() {
        let sans_prefixe: CredentialId = serde_json::from_str(r#""abcdef""#).unwrap();
        assert_eq!(sans_prefixe.as_str(), "abcdef");

        let vide: CredentialId = serde_json::from_str(r#""""#).unwrap();
        assert_eq!(vide.as_str(), "");

        assert_eq!(
            CredentialId::nouveau_avec_prefixe("1").as_str(),
            "cred_1",
            "seul le constructeur ajoute le prefixe"
        );
    }

    /// Une chaine vide est une chaine valide : elle ne disparait pas et ne
    /// devient pas `None`. Aucun `skip_serializing_if` n'est pose, donc elle
    /// reste presente dans le JSON. C'est le cas limite a ne pas confondre avec
    /// une valeur `undefined`.
    #[test]
    fn une_chaine_vide_est_conservee_comme_une_valeur_presente() {
        let credential: CredentialInfo =
            serde_json::from_str(r#"{"type":"credential","id":"","label":""}"#).unwrap();
        assert_eq!(credential.id.as_str(), "");
        assert_eq!(credential.label, "");

        let env: EnvInfo = serde_json::from_str(r#"{"type":"env","name":""}"#).unwrap();
        assert_eq!(env.name, "");

        let objet = serde_json::to_value(&env).unwrap();
        let champs = objet.as_object().unwrap();
        assert_eq!(champs.len(), 2, "une chaine vide reste une cle presente");
        assert_eq!(champs.get("name").and_then(|v| v.as_str()), Some(""));
    }

    /// L'union lit le tag et reconnait la bonne variante : une connexion par
    /// variable d'environnement n'est jamais lue comme une connexion par
    /// credential, meme si les autres champs correspondent.
    #[test]
    fn le_tag_de_l_union_selectionne_la_variante_correcte() {
        let credential: Info =
            serde_json::from_str(r#"{"type":"credential","id":"cred_1","label":"Compte"}"#).unwrap();
        assert_eq!(credential.tag(), "credential");
        match credential {
            Info::Credential(info) => assert_eq!(info.label, "Compte"),
            Info::Env(_) => panic!("une connexion par credential ne doit pas devenir une connexion par variable"),
        }

        let env: Info = serde_json::from_str(r#"{"type":"env","name":"OPENCODE_KEY"}"#).unwrap();
        assert_eq!(env.tag(), "env");
        match env {
            Info::Env(info) => assert_eq!(info.name, "OPENCODE_KEY"),
            Info::Credential(_) => panic!("une connexion par variable ne doit pas devenir une connexion par credential"),
        }
    }

    /// L'union se re-serialise sans ecrire deux fois la cle `type`, et le
    /// resultat se relit comme le point de depart.
    #[test]
    fn une_info_serialisee_puis_relu_donne_la_meme_info() {
        let depart = Info::Env(EnvInfo::nouveau("OPENCODE_KEY"));
        let json = serde_json::to_string(&depart).unwrap();

        assert_eq!(
            json.matches("\"type\"").count(),
            1,
            "la cle type ne doit apparaitre qu'une fois : {json}"
        );

        let arrivee: Info = serde_json::from_str(&json).unwrap();
        assert_eq!(depart, arrivee);
    }

    /// Une connexion par credential ne porte pas le nom `name`, une connexion
    /// par variable ne porte ni `id` ni `label`, et le tag inconnu est refuse.
    #[test]
    fn une_info_invalide_ou_ambigue_est_refusee() {
        // Tag inconnu des deux cotes.
        assert!(serde_json::from_str::<Info>(r#"{"type":"oauth","name":"X"}"#).is_err());
        // Champ obligatoire absent.
        assert!(serde_json::from_str::<CredentialInfo>(r#"{"type":"credential","id":"cred_1"}"#).is_err());
        assert!(serde_json::from_str::<EnvInfo>(r#"{"type":"env"}"#).is_err());
        // Champ obligatoire present mais du mauvais type.
        assert!(serde_json::from_str::<EnvInfo>(r#"{"type":"env","name":42}"#).is_err());
        // Identifiant qui n'est pas une chaine.
        assert!(
            serde_json::from_str::<CredentialInfo>(r#"{"type":"credential","id":{"v":1},"label":"x"}"#)
                .is_err()
        );
    }

    /// Une liste vide d'informations ne contient rien, et une liste a une seule
    /// entree se comporte comme l'entree isolee.
    #[test]
    fn une_liste_vide_d_infos_ne_produit_aucune_entree() {
        let vide: Vec<Info> = Vec::new();
        assert!(vide.is_empty());
        assert_eq!(serde_json::to_string(&vide).unwrap(), "[]");

        let une_seule = vec![Info::Env(EnvInfo::nouveau("OPENCODE_KEY"))];
        let relue: Vec<Info> =
            serde_json::from_str(&serde_json::to_string(&une_seule).unwrap()).unwrap();
        assert_eq!(relue.len(), 1);
        assert_eq!(relue[0].tag(), "env");
    }
}
