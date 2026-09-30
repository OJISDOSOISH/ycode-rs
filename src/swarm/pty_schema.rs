//! Portage de `packages/core/src/pty/schema.ts`.
//!
//! La source tient en une seule ligne :
//!
//! ```ts
//! export { ID as PtyID } from "@opencode-ai/schema/pty"
//! ```
//!
//! C'est un renommage pur : le fichier ne definit rien, il ne fait qu'exposer
//! sous le nom `PtyID` un schema qui vit dans le paquet
//! `@opencode-ai/schema/pty`, fichier `packages/schema/src/pty.ts`.
//!
//! ## Ce qu'est reellement `ID` en amont
//!
//! ```ts
//! const IDSchema = Schema.String.check(Schema.isStartsWith("pty")).pipe(Schema.brand("PtyID"))
//!
//! export const ID = IDSchema.pipe(
//!   statics((schema: typeof IDSchema) => {
//!     const create = () => schema.make("pty_" + ascending())
//!     return {
//!       create,
//!       ascending: (id?: string) => (id === undefined ? create() : schema.make(id)),
//!     }
//!   }),
//! )
//! ```
//!
//! Trois choses a retenir, et rien d'autre :
//!
//! 1. C'est une chaine **brandee** `PtyID` qui doit commencer par `"pty"`.
//!    Pas `"pty_"` : le controle est `isStartsWith("pty")`, donc la chaine
//!    nue `"pty"` est acceptee. Le prefixe `"pty_"` n'apparait que dans la
//!    fabrication des identifiants, pas dans la validation.
//! 2. `create()` n'a pas d'argument et produit `"pty_" + ascending()`, ou
//!    `ascending` est le generateur d'identifiants du projet
//!    (`packages/schema/src/identifier.ts`), pas la statique du dessous.
//! 3. `ascending(id?)` compare avec `id === undefined`, donc **seul** `undefined`
//!    declenche `create()`. Une chaine vide ne declenche rien : elle part dans
//!    `schema.make("")` et fait echouer la validation. C'est le piege `?` contre
//!    `??` du projet, ici tranche du bon cote : en Rust `Option<&str>` avec
//!    `None` qui cree et `Some("")` qui echoue reproduit exactement le
//!    comportement, et `Some("")` ne doit surtout pas etre traite comme `None`.
//!
//! ## Deux ecarts assumes par rapport a la source
//!
//! - **Le schema amont n'est pas porte par ce lot.** `packages/schema/src/pty.ts`
//!   ne fait pas partie des fichiers traduits en parallele, donc `PtyID` est
//!   defini ici plutot que reexporte. Si un jour le paquet schema est porte,
//!   ce fichier doit devenir un simple `pub use`.
//!
//! - **Le generateur d'identifiants est recopie ici.** `create()` a besoin de
//!   `ascending()` de `packages/schema/src/identifier.ts`, qui n'est pas encore
//!   traduit dans le crate. Plutot que d'appeler un module voisin dont la
//!   signature n'existe pas encore, la fonction est reprise telle quelle en
//!   prive, avec le meme format de sortie (26 caracteres : 12 chiffres
//!   hexadecimaux d'horodatage puis 14 caracteres d'un alphabet base 62). Le
//!   format est donc identique a celui du TypeScript. Le seul ecart est la
//!   source du hasard : `crypto.getRandomValues` est remplace par un xorshift
//!   sans dependance externe, puisque le crate ne declare pas de crate de
//!   hasard. Cela ne change rien au format, seulement la qualite aleatoire.
//!
//! Le controle de prefixe n'est pas rejoue a la deserialisation : comme les
//! autres identifiants du crate, `PtyID` est un newtype `#[serde(transparent)]`,
//! donc une chaine entree par le JSON est acceptee telle quelle. Le type impose
//! l'invariant a la construction, via `PtyID::new` et `PtyID::ascending`.

use std::fmt;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Prefixe impose par le controle `Schema.isStartsWith("pty")` en amont.
pub const PTY_ID_PREFIX: &str = "pty";

/// Prefixe ajoute par `create()` devant l'identifiant genere.
const PREFIXE_GENERE: &str = "pty_";

/// Repond a la question `Schema.isStartsWith("pty")` du TypeScript.
pub fn is_valid_pty_id(id: &str) -> bool {
    id.starts_with(PTY_ID_PREFIX)
}

/// Erreur de construction d'un `PtyID` : la chaine ne commence pas par `pty`.
///
/// En TypeScript, l'echec de `schema.make(...)` est une exception ; ici c'est
/// une valeur `Err`, ce qui evite un `panic` dans le code appelant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidPtyID {
    /// La chaine refusee, telle qu'elle a ete fournie.
    pub valeur: String,
}

impl fmt::Display for InvalidPtyID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "identifiant pty invalide : {:?} (prefixe attendu {:?})",
            self.valeur, PTY_ID_PREFIX
        )
    }
}

impl std::error::Error for InvalidPtyID {}

/// Identifiant de pseudo terminal, chaine brandee commencant par `pty`.
///
/// Equivaut a la fois au type et a la valeur `ID` de `@opencode-ai/schema/pty`,
/// les deux usages existant en TypeScript : `PtyID.ascending()` pour fabriquer
/// un identifiant, et `id: PtyID` pour typer un champ.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PtyID(pub String);

impl PtyID {
    /// Valide une chaine existante, comme le fait `schema.make(id)`.
    pub fn new(valeur: impl Into<String>) -> Result<Self, InvalidPtyID> {
        let valeur = valeur.into();
        if is_valid_pty_id(&valeur) {
            Ok(Self(valeur))
        } else {
            Err(InvalidPtyID { valeur })
        }
    }

    /// Equivalent de la statique `create()` : un identifiant neuf, jamais
    /// reutilise. Ne peut pas echouer, la valeur produite commence par `pty_`.
    pub fn create() -> Self {
        Self(format!("{}{}", PREFIXE_GENERE, identifiant_ascendant()))
    }

    /// Equivalent de la statique `ascending(id?)` du schema.
    ///
    /// `None` reproduit `undefined` et fabrique un identifiant neuf. `Some("")`
    /// n'est **pas** traite comme `None` : la chaine vide part en validation et
    /// est refusee, exactement comme en TypeScript.
    pub fn ascending(id: Option<&str>) -> Result<Self, InvalidPtyID> {
        match id {
            None => Ok(Self::create()),
            Some(valeur) => Self::new(valeur),
        }
    }

    /// La chaine brute, sans le newtype.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PtyID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<PtyID> for String {
    fn from(id: PtyID) -> String {
        id.0
    }
}

// ---------------------------------------------------------------- generateurs

/// Reprise privee de `packages/schema/src/identifier.ts`.
///
/// Le fichier d'origine utilise `BigInt` et `crypto.getRandomValues`. Ici le
/// BigInt tient dans un `u64` (un horodatage en millisecondes multiplie par
/// 4096 tient tres largement dedans) et le hasard vient d'un xorshift, faute de
/// crate de hasard dans les dependances. Le format de sortie est inchange :
/// 12 chiffres hexadecimaux d'horodatage, puis 14 caracteres base 62.
fn identifiant_ascendant() -> String {
    let horodatage = horodatage_ms();

    // Compteur remis a zero des que l'horodatage change, comme en TypeScript.
    let mut etat = ETAT_IDENTIFIANT.lock().unwrap_or_else(|e| e.into_inner());
    if horodatage != etat.0 {
        etat.0 = horodatage;
        etat.1 = 0;
    }
    etat.1 += 1;
    let compteur = etat.1;

    let valeur = (horodatage.max(0) as u64) * 0x1000 + compteur;

    let mut sortie = String::with_capacity(LONGUEUR_IDENTIFIANT);
    for index in 0..OCTETS_DE_TEMPS {
        let decalage = 8 * (OCTETS_DE_TEMPS - 1 - index);
        let octet = ((valeur >> decalage) & 0xff) as u8;
        sortie.push_str(&format!("{:02x}", octet));
    }
    for _ in 0..OCTETS_ALEATOIRES {
        let octet = (aleatoire() & 0xff) as u8;
        sortie.push(ALPHABET[usize::from(octet) % ALPHABET.len()] as char);
    }
    sortie
}

const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const LONGUEUR_IDENTIFIANT: usize = 26;
const OCTETS_DE_TEMPS: usize = 6;
const OCTETS_ALEATOIRES: usize = LONGUEUR_IDENTIFIANT - 2 * OCTETS_DE_TEMPS;

/// Horodatage du module identifiant : dernier milliseconde vue, et compteur
/// d'identifiants produits dans cette milliseconde.
static ETAT_IDENTIFIANT: Mutex<(i64, u64)> = Mutex::new((0, 0));

/// Etat du xorshift qui remplace `crypto.getRandomValues`.
static ETAT_ALEATOIRE: AtomicU64 = AtomicU64::new(0);

fn horodatage_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Tire un entier 64 bits pseudo aleatoire. Le xorshift ne doit jamais rester a
/// zero, sinon il se bloque sur zero : la graine et la sortie sont donc
/// reparees dans les deux cas.
fn aleatoire() -> u64 {
    let mut valeur = ETAT_ALEATOIRE.load(Ordering::SeqCst);
    if valeur == 0 {
        valeur = graine_non_nulle();
    }
    valeur ^= valeur << 13;
    valeur ^= valeur >> 7;
    valeur ^= valeur << 17;
    if valeur == 0 {
        valeur = 0x9E37_79B9_7F4A_7C15;
    }
    ETAT_ALEATOIRE.store(valeur, Ordering::SeqCst);
    valeur
}

fn graine_non_nulle() -> u64 {
    let deja_lue = ETAT_ALEATOIRE.load(Ordering::SeqCst);
    if deja_lue != 0 {
        return deja_lue;
    }
    let temps = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    let candidat = temps ^ 0x9E37_79B9_7F4A_7C15;
    let _ = ETAT_ALEATOIRE.compare_exchange(0, candidat, Ordering::SeqCst, Ordering::SeqCst);
    ETAT_ALEATOIRE.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sert uniquement a verifier le nom de champ camelCase cote JSON, comme
    /// le fait `packages/core/src/pty/ticket.ts` avec son champ `ptyID`.
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Ticket {
        #[serde(rename = "ptyID")]
        pty_id: PtyID,
    }

    #[test]
    fn une_chaine_commencant_par_pty_est_acceptee() {
        let id = PtyID::new("pty_01H").expect("une chaine valide doit passer");
        assert_eq!(id.as_str(), "pty_01H");
        assert!(is_valid_pty_id("pty_01H"));
    }

    #[test]
    fn le_prefixe_seul_sans_sous_titre_est_accepte_car_le_controle_vaut_pty() {
        let id = PtyID::new("pty").expect("le controle amont porte sur pty, pas sur pty_");
        assert_eq!(id.as_str(), "pty");
    }

    #[test]
    fn une_chaine_vide_est_refusee() {
        assert!(PtyID::new("").is_err());
        assert!(!is_valid_pty_id(""));
    }

    #[test]
    fn une_chaine_sans_le_prefixe_pty_est_refusee() {
        let erreur = PtyID::new("session_01H").expect_err("session_ ne commence pas par pty");
        assert_eq!(erreur.valeur, "session_01H");
        assert!(!is_valid_pty_id("session_01H"));
    }

    #[test]
    fn le_prefixe_est_sensible_a_la_casse() {
        assert!(PtyID::new("PTY_01H").is_err());
        assert!(PtyID::new("Pty_01H").is_err());
    }

    #[test]
    fn un_identifiant_fabrique_commence_par_pty_suivi_d_un_soulignement() {
        let brut = String::from(PtyID::create());
        assert!(brut.starts_with("pty_"), "obtenu : {:?}", brut);
    }

    #[test]
    fn un_identifiant_fabrique_tient_26_caracteres_apres_le_prefixe() {
        let brut = String::from(PtyID::create());
        let corps = &brut[PREFIXE_GENERE.len()..];
        assert_eq!(corps.len(), LONGUEUR_IDENTIFIANT);
        assert_eq!(brut.len(), 4 + LONGUEUR_IDENTIFIANT);
        assert!(
            corps.chars().all(|c| c.is_ascii_alphanumeric()),
            "le corps ne doit contenir que de l'alphanumerique ascii : {:?}",
            corps
        );
    }

    #[test]
    fn un_identifiant_fabrique_commence_par_12_chiffres_hexadecimaux_en_minuscule() {
        let brut = String::from(PtyID::create());
        let temps = &brut[PREFIXE_GENERE.len()..PREFIXE_GENERE.len() + 12];
        assert!(
            temps.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "l'horodatage attend 12 chiffres hexadecimaux, obtenus : {:?}",
            temps
        );
    }

    #[test]
    fn deux_identifiants_fabriques_sont_differents() {
        let premier = PtyID::create();
        let second = PtyID::create();
        assert_ne!(premier, second, "deux appels successifs doivent differer");
    }

    #[test]
    fn ascending_sans_argument_fabrique_un_identifiant_neuf() {
        let id = PtyID::ascending(None).expect("aucune validation ne peut echouer ici");
        assert!(id.as_str().starts_with("pty_"));
    }

    #[test]
    fn ascending_avec_un_identifiant_valide_le_reutilise_tel_quel() {
        let id = PtyID::ascending(Some("pty_deja_connu")).expect("un identifiant valide passe");
        assert_eq!(id.as_str(), "pty_deja_connu");
    }

    #[test]
    fn ascending_avec_une_chaine_vide_refuse_la_chaine_vide_au_lieu_de_fabriquer() {
        // Piege `?` contre `??` : en TypeScript le test est `id === undefined`,
        // donc une chaine vide part en validation et leve une erreur. Elle ne
        // doit pas etre traitee comme un absent.
        let resultat = PtyID::ascending(Some(""));
        assert!(resultat.is_err(), "la chaine vide ne doit pas fabriquer d'identifiant");
    }

    #[test]
    fn un_identifiant_est_serialise_en_chaine_simple_sans_objet_autour() {
        let json = serde_json::to_string(&PtyID("pty_abc".to_string())).expect("serialisation");
        assert_eq!(json, "\"pty_abc\"");
    }

    #[test]
    fn un_identifiant_relu_depuis_le_json_redonne_la_meme_chaine() {
        let id: PtyID = serde_json::from_str("\"pty_abc\"").expect("deserialisation");
        assert_eq!(id.as_str(), "pty_abc");
    }

    #[test]
    fn un_champ_qui_contient_un_identifiant_sort_sous_le_nom_pty_id_en_camel() {
        let ticket = Ticket {
            pty_id: PtyID("pty_abc".to_string()),
        };
        let json = serde_json::to_string(&ticket).expect("serialisation");
        assert_eq!(json, "{\"ptyID\":\"pty_abc\"}");
        let relu: Ticket = serde_json::from_str(&json).expect("deserialisation");
        assert_eq!(relu, ticket);
    }

    #[test]
    fn un_identifiant_affiche_sans_guillemets_comme_dans_une_url_ou_un_entete() {
        let id = PtyID::new("pty_abc").expect("identifiant valide");
        assert_eq!(id.to_string(), "pty_abc");
    }
}
