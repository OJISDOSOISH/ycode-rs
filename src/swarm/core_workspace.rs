//! Portage Rust de `opencode/packages/core/src/workspace.ts`.
//!
//! La source tient en six lignes, et une seule chose y est reellement definie :
//!
//! ```ts
//! export * as WorkspaceV2 from "./workspace"   // reexport de lui-meme
//! import { Workspace } from "@opencode-ai/schema/workspace"
//! export const ID = Workspace.ID                // <- le seul contenu de fond
//! export type ID = typeof ID.Type
//! ```
//!
//! La premiere ligne est du code mort pour nous : en Rust le module
//! `core_workspace` est deja l'espace de noms `WorkspaceV2`, il n'y a donc rien a
//! retranscrire.
//!
//! # Ce que vaut `Workspace.ID`
//!
//! La chaine designee par `Workspace.ID` est
//! `packages/schema/src/workspace.ts`, qui renvoie lui-meme
//! `packages/schema/src/workspace-id.ts` :
//!
//! ```ts
//! export const WorkspaceID = Schema.String.check(Schema.isStartsWith("wrk")).pipe(
//!   Schema.brand("WorkspaceV2.ID"),
//!   statics((schema) => ({
//!     create: () => schema.make("wrk_" + ascending()),
//!     ascending: (id?: string) => {
//!       if (!id) return create()
//!       if (!id.startsWith("wrk")) throw new Error(`ID ${id} does not start with wrk`)
//!       return schema.make(id)
//!     },
//!   })),
//! )
//! ```
//!
//! Donc un identifiant d'espace de travail est une **chaine de caracteres qui
//! commence par `wrk`**, de marque `WorkspaceV2.ID`, qui se fabrique soit neuf
//! soit a partir d'une chaine deja presente, et qui se serialise en JSON comme
//! une simple chaine. C'est tout ce que contient ce fichier.
//!
//! # Le piege : `!id` est un test de veracite, pas de nullite
//!
//! `ascending` commence par `if (!id) return create()`. En JavaScript `!id`
//! est vrai pour `undefined`, pour `null` **et pour la chaine vide**.
//! L'identifiant `""` est donc traite comme un identifiant absent, et un
//! nouvel identifiant est fabrique a sa place.
//!
//! Traduire `id?: string` par un simple `Option<&str>` sans se soucier de ce
//! detail donnerait `Some("")` -> erreur("ID  does not start with wrk"), alors
//! que la source renvoie un identifiant neuf. C est le piege numero 1 du
//! portage, et il se presente ici exactement sous cette forme : le `""` doit
//! **disparaitre** ici. Le test dedie verrouille ce point.
//!
//! Attention a ne pas confondre les deux `ascending` de la source :
//!
//! - `identifier.ts` exporte `ascending()`, qui fabrique une chaine de 26
//!   caracteres et ne prend aucun argument ;
//! - `workspace-id.ts` ajoute une methode statique `ascending(id?)`, qui valide
//!   ou fabrique un identifiant.
//!
//! Les deux portent le meme nom en TypeScript, ce qui rend la lecture trompeuse.
//! Ici ils sont distincts : le generateur est la fonction privee
//! `identifiant_ascendant`, la methode publique est `WorkspaceId::ascendant`.
//!
//! # Les choix de portage
//!
//! 1. **Un seul type, pas deux.** La source exporte une valeur (`ID`, le
//!    schema) et un type (`ID`). Le schema `effect/Schema` n a pas
//!    d'equivalent Rust : il ne produit aucune donnee, seulement une
//!    validation. On garde donc le type seul, `WorkspaceId`.
//! 2. **Marque `WorkspaceV2.ID` conservee** sous forme de constante, comme
//!    `SCHEMA_IDENTIFIER` dans `config_command.rs` : elle ne sert qu'a la
//!    correspondance avec la source.
//! 3. **`Schema.isStartsWith("wrk")` ne demande pas de tiret bas.** La
//!    validation accepte donc `wrk` seul et `wrkXYZ`, meme si `create()`
//!    produit toujours `wrk_...`. On ne resserre pas la regle.
//! 4. **L 'exception devient un `Result`.** La source leve une `Error`, Rust
//!    n a pas d'exception : `ascendant` et `new` renvoient `Result`.
//! 5. **La validation est appliquee a la deserialisation.** Un schema
//!    `effect/Schema` refuse de decoder une chaine qui ne commence pas par
//!    `wrk` ; un `#[derive(Deserialize)]` tout simple l'accepterait. C est
//!    exactement le genre d'ecart invisible de l'interieur du code Rust qui ne
//!    se voit qu'a l'echange avec le TypeScript, donc on le rattrape ici.
//!
//! # Ce qui n'est pas porte
//!
//! - `identifier.ts` expose aussi `descending()` (ordre decroissant) et
//!   `create(descending, timestamp)`. Seule la branche croissante est utilisee
//!   par ce fichier, donc elle seule est portee.
//! - `crypto.getRandomValues` n existe pas en std et aucune dependance
//!   generative n est declaree dans `Cargo.toml`. Les 14 octets aleatoires de
//!   la fin de l'identifiant sont donc tires d'un melangeur interne
//!   (`SplitMix64`) amorce sur l'horloge. C'est le seul ecart de comportement
//!   reel de ce fichier, et il ne change **rien** au format produit : 14
//!   caracteres tires d'un alphabet de 62, comme avant.
//!
//! # Note sur l'ordre
//!
//! La partie temporelle de l'identifiant est le hexadecimal des **48 bits de
//! poids faible** de `horodatage * 4096 + compteur`, exactement comme dans la
//! source. L'horodatage en millisecondes tient sur 41 bits, le multiplieur en
//! 12 : le total occupe 53 bits et les 5 bits de tete sont donc tronques par la
//! source elle-meme. Consequence heredee, et voulue : l'identifiant reste
//! ordonne lexicographiquement, mais sur une fenetre d'environ 2,2 ans, apres
//! laquelle il repasse a zero.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Prefixe impose a tout identifiant d'espace de travail.
///
/// En TS : `Schema.String.check(Schema.isStartsWith("wrk"))`.
pub const ID_PREFIX: &str = "wrk";

/// Nom du schema dans le registre `effect/Schema`.
///
/// En TS : `Schema.brand("WorkspaceV2.ID")`.
pub const SCHEMA_IDENTIFIER: &str = "WorkspaceV2.ID";

/// Longueur de la partie generee, tiree de l'identifiant avant le prefixe.
///
/// En TS : `const length = 26` dans `packages/schema/src/identifier.ts`.
const LONGUEUR: usize = 26;

/// Alphabet de 62 caracteres employe pour la partie aleatoire.
const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Erreur : la chaine fournie n'est pas un identifiant d'espace de travail.
///
/// Message identique a celui de la source : `ID <valeur> does not start with wrk`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ID {id} does not start with wrk")]
pub struct WorkspaceIdError {
    /// La chaine refusee, telle qu'elle a ete recue.
    pub id: String,
}

/// Un identifiant d'espace de travail : une chaine commencant par `wrk`.
///
/// Le type est **transparent** en JSON, comme la chaine de marque qu'il
/// enveloppe : `serde_json::to_string` donne `"wrk_..."`, et non
/// `{"id":"wrk_..."}`. Il n'y a donc aucun nom de champ a renommer ici, et
/// c'est le seul endroit du portage ou il n'y en a pas.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
// `deserialize_with` n'est accepte que sur un CHAMP, jamais sur un conteneur :
// le mettre ici ne compilait pas. Pour un newtype transparent dont la
// construction peut echouer, l'idiome correct est `try_from`, qui fait
// deserialiser une `String` puis passer par `TryFrom`. La validation reste donc
// dans le type, et pas a chaque appelant.
#[serde(try_from = "String")]
pub struct WorkspaceId(String);

impl WorkspaceId {
    /// Valide une chaine et en fait un identifiant.
    ///
    /// Equivalent de l'appel `schema.make(...)` de la source : c'est le seul
    /// point ou la regle `commence par wrk` est appliquee.
    pub fn new(id: impl Into<String>) -> Result<Self, WorkspaceIdError> {
        let id = id.into();
        if !id.starts_with(ID_PREFIX) {
            return Err(WorkspaceIdError { id });
        }
        Ok(Self(id))
    }

    /// Fabrique un identifiant neuf.
    ///
    /// Equivalent de `WorkspaceID.create()` : le prefixe `wrk_` suivi de 26
    /// caracteres generes. Le resultat est toujours valide, donc il n'y a pas
    /// d'erreur a rendre ici.
    pub fn create() -> Self {
        // Le literal "wrk_" est le prefixe de la source, concatene a
        // l'identifiant genere par `identifier.ts`.
        Self(format!("wrk_{}", identifiant_ascendant()))
    }

    /// Reprend un identifiant existant, ou en fabrique un neuf s'il n'y en a pas.
    ///
    /// Equivalent de la methode statique `WorkspaceID.ascending(id?)`.
    ///
    /// Regle fidele a la source : une chaine vide est traitee comme un
    /// identifiant absent, parce que `if (!id)` teste la veracite en
    /// JavaScript. Un `Some("")` **ne doit donc pas** produire une erreur.
    pub fn ascendant(id: Option<&str>) -> Result<Self, WorkspaceIdError> {
        match id {
            None => Ok(Self::create()),
            Some(id) if id.is_empty() => Ok(Self::create()),
            Some(id) => Self::new(id),
        }
    }

    /// La valeur brute de l'identifiant.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<WorkspaceId> for String {
    fn from(id: WorkspaceId) -> String {
        id.0
    }
}

impl AsRef<str> for WorkspaceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Point d'entree de `serde` : lit une chaine, puis la fait passer par la
/// validation.
///
/// La source utilise un schema `effect/Schema`, qui refuse de decoder une
/// valeur invalide. Un `#[derive(Deserialize)]` se contente de verifier le type
/// JSON : il laisserait passer n'importe quelle chaine. C'est cet
/// `TryFrom` qui comble l'ecart.
impl TryFrom<String> for WorkspaceId {
    type Error = WorkspaceIdError;

    fn try_from(brut: String) -> Result<Self, Self::Error> {
        Self::new(brut)
    }
}

/// Etat global du generateur, equivalent des deux variables de module
/// `lastTimestamp` et `counter` de `packages/schema/src/identifier.ts`.
///
/// Le TypeScript n'a pas de concurrence explicite mais son module est partage :
/// en Rust l equivalents le plus simple est un unique `Mutex` qui regroupe
/// l'horodatage du dernier appel, le compteur de la milliseconde courante et
/// le nombre total d'appels (utilise pour la partie aleatoire).
struct EtatGenerateur {
    /// Dernier horodatage vu, en millisecondes.
    dernier_millis: i64,
    /// Compteur remis a zero a chaque changement de milliseconde.
    compteur: u64,
    /// Nombre d'appels depuis le demarrage, pour ne pas repeter la meme graine.
    appels: u64,
}

static ETAT_GENERATEUR: Mutex<EtatGenerateur> = Mutex::new(EtatGenerateur {
    dernier_millis: 0,
    compteur: 0,
    appels: 0,
});

/// L'horodatage courant en millisecondes depuis l'epoch.
///
/// En TS : `Date.now()`. Retourne 0 si l'horloge systeme est anterieure a
/// l'epoch, ce qui n'a aucune consequence ici.
fn millis_actuels() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Melange un entier 64 bits (SplitMix64) et renvoie un entier 64 bits.
///
/// Remplace `crypto.getRandomValues(new Uint8Array(14))` de la source, aucune
/// dependance generative n'etant disponible dans `Cargo.toml`.
fn melange(graine: u64) -> u64 {
    let mut z = graine.wrapping_add(0x9E37_97B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Fabrique une chaine de 26 caracteres : 12 chiffres hexadecimaux de l'heure
/// puis 14 caracteres aleatoires.
///
/// Equivalent de `ascending()` dans `packages/schema/src/identifier.ts`.
/// Seule la branche croissante est portee, car c'est la seule utilisee ici.
fn identifiant_ascendant() -> String {
    let millis = millis_actuels();
    // Un mutex empoisonne ne doit pas empecher la creation d'identifiants.
    let mut etat = ETAT_GENERATEUR
        .lock()
        .unwrap_or_else(|erreur| erreur.into_inner());

    // Compteur remis a zero des que la milliseconde change, puis incremente
    // avant usage : le premier identifiant d'une milliseconde porte 1.
    if millis != etat.dernier_millis {
        etat.dernier_millis = millis;
        etat.compteur = 0;
    }
    etat.compteur += 1;

    // `timestamp * 0x1000n + counter`, puis les 6 octets de poids faible.
    let valeur = u128::from(millis.max(0) as u64) * 0x1000 + u128::from(etat.compteur);

    let mut resultat = String::with_capacity(LONGUEUR);

    for index in 0..6usize {
        let octet = ((valeur >> (8 * (5 - index))) & 0xff) as u8;
        resultat.push_str(&format!("{octet:02x}"));
    }

    // Les 14 derniers octets, comme `crypto.getRandomValues`, traduits en
    // caracteres par le modulo 62 de la source.
    for _ in 0..(LONGUEUR - 12) {
        etat.appels += 1;
        let graine = millis.max(0) as u64 ^ etat.appels.wrapping_mul(0x9E37_97B9_7F4A_7C15);
        let octet = melange(graine) as usize;
        resultat.push(ALPHABET[octet % ALPHABET.len()] as char);
    }

    resultat
}

#[cfg(test)]
mod tests {
    use super::{
        identifiant_ascendant, millis_actuels, melange, WorkspaceId, WorkspaceIdError, ID_PREFIX,
        LONGUEUR, SCHEMA_IDENTIFIER,
    };

    /// Partie aleatoire autorisee : 26 caracteres de l'alphabet de 62.
    fn partie_aleatoire(id: &str) -> &str {
        &id[12..]
    }

    #[test]
    fn un_identifiant_neuf_commence_par_le_prefixe_de_l_espace_de_travail() {
        let id = WorkspaceId::create();
        assert!(id.as_str().starts_with(ID_PREFIX), "{id} ne commence pas par wrk");
        assert!(id.as_str().starts_with("wrk_"), "{id} ne porte pas le tiret bas");
    }

    #[test]
    fn un_identifiant_neuf_fait_trente_caracteres_au_total() {
        // "wrk_" puis les 26 caracteres generes par identifier.ts.
        let id = WorkspaceId::create();
        assert_eq!(id.as_str().len(), 4 + LONGUEUR);
    }

    #[test]
    fn la_partie_aleatoire_n_utilise_que_l_alphabet_autorise() {
        for _ in 0..16 {
            let genere = identifiant_ascendant();
            assert_eq!(genere.len(), LONGUEUR, "longueur inattendue : {genere}");
            for caractere in partie_aleatoire(&genere).chars() {
                assert!(
                    caractere.is_ascii_alphanumeric(),
                    "caractere hors alphabet dans {genere} : {caractere}"
                );
            }
        }
    }

    #[test]
    fn la_partie_temporelle_est_de_l_hexadecimal_en_minuscules() {
        // Les 12 premiers caracteres sont l'heure, en hexadecimal, et c'est ce
        // qui garantit l'ordre lexicographique des identifiants.
        let genere = identifiant_ascendant();
        for caractere in genere[..12].chars() {
            assert!(
                caractere.is_ascii_digit() || ('a'..='f').contains(&caractere),
                "caractere non hexadecimal dans {genere} : {caractere}"
            );
        }
    }

    #[test]
    fn deux_identifiants_neuf_sont_toujours_differents() {
        let premier = WorkspaceId::create();
        let second = WorkspaceId::create();
        assert_ne!(premier, second, "deux identifiantsont ete produits egaux");
    }

    #[test]
    fn des_identifiants_neuf_sont_classes_par_ordre_croissant() {
        let premier = WorkspaceId::create();
        let second = WorkspaceId::create();
        let troisieme = WorkspaceId::create();
        assert!(premier < second, "{premier} devrait preceder {second}");
        assert!(second < troisieme, "{second} devrait preceder {troisieme}");
    }

    #[test]
    fn une_chaine_qui_commence_par_wrk_est_acceptee() {
        // La regle est `startsWith("wrk")` : le tiret bas n'est pas exige, et
        // `wrk` seul est valide. Resserrer la regle serait une invention.
        assert!(WorkspaceId::new("wrk_abc123").is_ok());
        assert!(WorkspaceId::new("wrk").is_ok());
        assert!(WorkspaceId::new("wrkXYZ").is_ok());
    }

    #[test]
    fn une_chaine_qui_ne_commence_pas_par_wrk_est_refusee() {
        assert_eq!(
            WorkspaceId::new("autre"),
            Err(WorkspaceIdError { id: "autre".to_string() })
        );
        assert!(WorkspaceId::new("").is_err());
        assert!(WorkspaceId::new("Wwrk").is_err(), "la casse compte");
    }

    #[test]
    fn une_chaine_vide_est_traitee_comme_un_identifiant_absent() {
        // Point le plus subtil du fichier : dans la source, `if (!id)` teste la
        // veracite, donc `""` est traite comme absent et un identifiant neuf
        // est fabrique. Un `Some("")` qui proverait une erreur serait faux.
        let id = WorkspaceId::ascendant(Some("")).expect("une chaine vide ne doit pas etre refusee");
        assert!(id.as_str().starts_with("wrk_"), "un identifiant neuf etait attendu : {id}");
    }

    #[test]
    fn un_identifiant_valide_fourni_est_reutilise_tel_quel() {
        let fourni = "wrk_deja_connu";
        let id = WorkspaceId::ascendant(Some(fourni)).expect("l'identifiant fourni est valide");
        assert_eq!(id.as_str(), fourni, "un identifiant existant ne doit pas etre regenere");
    }

    #[test]
    fn un_identifiant_fourni_qui_est_invalide_est_refuse() {
        assert_eq!(
            WorkspaceId::ascendant(Some("autre")),
            Err(WorkspaceIdError { id: "autre".to_string() })
        );
    }

    #[test]
    fn sans_identifiant_un_identifiant_neuf_est_cree() {
        let id = WorkspaceId::ascendant(None).expect("l'absence d'identifiant n'est pas une erreur");
        assert!(id.as_str().starts_with("wrk_"), "{id} n'est pas un identifiant neuf");
    }

    #[test]
    fn un_identifiant_se_serialise_en_chaine_simple() {
        let id = WorkspaceId::create();
        let json = serde_json::to_string(&id).expect("serialisation");
        assert_eq!(json, format!("\"{id}\""));
        assert!(json.starts_with("\"wrk_"), "la valeur JSON doit rester une chaine : {json}");

        let relu: WorkspaceId = serde_json::from_str(&json).expect("deserialisation");
        assert_eq!(relu, id);
    }

    #[test]
    fn un_identifiant_invalide_est_refuse_a_la_deserialisation() {
        // Une chaine JSON valide syntaxiquement mais trop breve doit etre
        // rejetee, comme le ferait le schema `effect/Schema` d'origine.
        assert!(serde_json::from_str::<WorkspaceId>(r#""autre""#).is_err());
        assert!(serde_json::from_str::<WorkspaceId>(r#""""#).is_err());
        assert!(serde_json::from_str::<WorkspaceId>(r#"42"#).is_err(), "un nombre n'est pas une chaine");
    }

    #[test]
    fn une_liste_d_identifiants_se_deserialise_entierement() {
        let attendu = WorkspaceId::new("wrk_a").expect("valide");
        let liste: Vec<WorkspaceId> = serde_json::from_str(r#"["wrk_a","wrk_b"]"#).expect("liste");
        assert_eq!(liste, vec![attendu, WorkspaceId::new("wrk_b").expect("valide")]);
    }

    #[test]
    fn le_melange_produit_des_valeurs_differentes_pour_des_graines_differentes() {
        // Le melangeur remplace `crypto.getRandomValues` : deux graines
        // differentes ne doivent pas produire le meme resultat, sans quoi les
        // identifiants de deux processus pourraient coincider.
        let premier = melange(1);
        let second = melange(2);
        assert_ne!(premier, second);
    }

    #[test]
    fn l_horodatage_critique_est_positif_et_en_millisecondes() {
        let millis = millis_actuels();
        assert!(millis > 0, "l'horloge systeme doit etre posterieure a l'epoch");
        // Une valeur en secondes serait autour de 1,7 milliard, en
        // millisecondes autour de 1,7 trillion.
        assert!(millis > 1_000_000_000_000, "l'horodatage doit etre en millisecondes");
    }

    #[test]
    fn le_nom_du_schema_est_conserve() {
        assert_eq!(SCHEMA_IDENTIFIER, "WorkspaceV2.ID");
    }
}
