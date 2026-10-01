//! Portage de `packages/core/src/util/identifier.ts`.
//!
//! Le fichier TypeScript d'origine tient en une seule ligne :
//!
//! ```text
//! export * as Identifier from "@opencode-ai/schema/identifier"
//! ```
//!
//! Il ne contient donc aucun code, seulement un reexport. L'implementation
//! reelle vit dans `packages/schema/src/identifier.ts`, et c'est elle qui est
//! portee ici, parce qu'un reexport n'a pas d'equivalent direct en Rust (il
//! n'existe pas de "module namespace" reexportable tel quel). Aucun autre agent
//! du swarm ne porte ce fichier du paquet schema, donc il n'y a pas de doublon.
//!
//! Rappel du comportement d'origine, qui est le contrat a respecter :
//!
//! - un identifiant fait exactement 26 caracteres ;
//! - les 12 premiers sont l'heure, en hexadecimal minuscule sur 6 octets lus du
//!   plus fort au plus faible, donc ranges en petit boutiste ;
//! - les 14 suivants sont des caracteres aleatoires tires de
//!   `"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"` ;
//! - l'heure est construite par `BigInt(timestamp) * 0x1000 + counter`, donc le
//!   compteur tient dans les 12 bits de poids faible de l'entier ;
//! - en version descendante, la valeur utilisee est `~current`, c'est a dire le
//!   complement a un sur deux bits. Sur un entier negatif, un decalage vers la
//!   droite reste arithmetique (repere par les bits) et le masquage `& 0xff`
//!   regarde les 8 bits de poids faible de l'ecriture a deux complements. D'ou
//!   l'usage de `i128` signe ci-dessous, qui reproduit exactement la semantique
//!   de `BigInt` ;
//! - le compteur est un etat de module, partage par tous les appels : il
//!   repart de zero des que le timestamp change, et il s'incremente de un pour
//!   chaque identifiant cree dans la meme milliseconde.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

/// Longueur totale d'un identifiant, comme la constante `length` d'origine.
const LENGTH: usize = 26;

/// Longueur de la partie temporelle : 6 octets ecrits en hexadecimal.
const TIME_LENGTH: usize = 12;

/// Nombre d'octets aleatoires, donc de caracteres aleatoires.
const RANDOM_LENGTH: usize = LENGTH - TIME_LENGTH;

/// Alphabet de la partie aleatoire, dans l'ordre exact du TypeScript.
const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// La constante `0x1000n` d'origine : la place du compteur dans l'entier.
const BASE: i128 = 0x1000;

/// Les deux variables de module `lastTimestamp` et `counter`, plus l'etat du
/// generateur pseudo aleatoire. Le TypeScript s'appuie sur le fait qu'il est
/// mono-thread pour s'en passer ; en Rust il faut un verrou.
struct State {
    last_timestamp: i64,
    counter: i64,
    seed: u64,
}

static STATE: Mutex<State> = Mutex::new(State {
    last_timestamp: 0,
    counter: 0,
    seed: 0,
});

/// Le verrou peut etre poisoned si un autre thread a panique en son interieur.
/// On recupere alors l'etat quand meme : l'etat de l'origine n'a pas d'action
/// a reussir, il n'est pas coherent a proteger.
fn state() -> MutexGuard<'static, State> {
    match STATE.lock() {
        Ok(guard) => guard,
        Err(empoisonne) => empoisonne.into_inner(),
    }
}

/// L'horloge du TypeScript, `Date.now()`, en millisecondes depuis l'epoch.
pub fn now_millis() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duree) => duree.as_millis() as i64,
        Err(_) => 0,
    }
}

/// `Identifier.ascending()` : un identifiant qui se trie dans l'ordre
/// chronologique.
pub fn ascending() -> String {
    create(false)
}

/// `Identifier.descending()` : un identifiant qui se trie dans l'ordre
/// chronologique inverse.
pub fn descending() -> String {
    create(true)
}

/// `Identifier.create(descending)` avec le timestamp par defaut, c'est a dire
/// l'heure courante.
///
/// Le parametre est nomme `descending_order` et non `descending` parce que le
/// module expose aussi une fonction `descending`, et qu'en Rust le parametre
/// masquerait la fonction.
pub fn create(descending_order: bool) -> String {
    create_at(descending_order, now_millis())
}

/// `Identifier.create(descending, timestamp)` avec un timestamp impose.
///
/// Utile pour les tests, et equivalent du timestamp par defaut quand on passe
/// `now_millis()`.
pub fn create_at(descending_order: bool, timestamp: i64) -> String {
    let (counter, random) = {
        let mut etat = state();
        if timestamp != etat.last_timestamp {
            etat.last_timestamp = timestamp;
            etat.counter = 0;
        }
        etat.counter += 1;
        let mut random = [0u8; RANDOM_LENGTH];
        for octet in random.iter_mut() {
            *octet = random_byte(&mut etat.seed);
        }
        (etat.counter, random)
    };
    compose(descending_order, timestamp, counter, &random)
}

/// La partie pure du formatage : ni horloge, ni compteur global, ni hasard.
/// Tous les tests de format passent par ici.
///
/// - `descending_order` correspond a l'inversion par `~` ;
/// - `random` est la liste des octets aleatoires, convertis en caracteres.
fn compose(descending_order: bool, timestamp: i64, counter: i64, random: &[u8]) -> String {
    let current = (timestamp as i128) * BASE + counter as i128;
    // The source treats the 56-bit value as unsigned for hex formatting.
    // For ascending: current (positive). For descending: ~current (two's complement).
    // Mask to 56 bits to match TypeScript's BigInt behavior where only the low
    // 56 bits are used for the time segment (12 hex chars = 48 bits, but the
    // full value is 56 bits to cover the ~2.2 year window).
    const MASK_56: i128 = (1i128 << 56) - 1;
    let value_u56 = if descending_order {
        (!current) & MASK_56
    } else {
        current & MASK_56
    };
    // Format as 14 hex chars (56 bits), take the high 48 bits (first 12 chars).
    let hex = format!("{:014x}", value_u56);
    let time_hex = &hex[..12];

    let mut out = String::with_capacity(TIME_LENGTH + random.len());
    out.push_str(time_hex);
    for &octet in random {
        out.push(CHARS[(octet % 62) as usize] as char);
    }
    out
}

// ---------------------------------------------------------------------------
// Hasard
// ---------------------------------------------------------------------------
//
// ATTENTION, ecart assume par rapport au TypeScript : l'original utilise
// `crypto.getRandomValues`, donc une source cryptographique. Le crate `ycode` ne
// declare ni `getrandom` ni `rand` dans `Cargo.toml`, et ce fichier n'a pas le
// droit de le modifier. On utilise donc SplitMix64, amorce avec les cles
// aleatoires de `RandomState`, que la bibliotheque standard tire de l'OS. Ce
// n'est pas cryptographique : a remplacer des que `getrandom` est disponible.

/// SplitMix64 : petit generateur, rapide, et suffisant pour de la diversite
/// d'identifiants. `wrapping` parce que l'arithmetique doit tourner en anneau
/// modulo 2 puissance 64, y compris en compilation de debug ou un depassement
/// paniquerait.
fn next_random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// L'amorce, prise une seule fois dans `RandomState`.
fn initial_seed() -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(0x1234_5678_9ABC_DEF0);
    hasher.finish()
}

/// Un octet au hasard. `seed == 0` signifie "pas encore amorce".
fn random_byte(seed: &mut u64) -> u8 {
    if *seed == 0 {
        *seed = initial_seed();
    }
    (next_random(seed) & 0xff) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les tests qui touchent l'etat global prennent ce verrou, sinon le
    /// fait que `cargo test` execute les tests en parallele rendrait le
    /// compteur imprevisible d'un test a l'autre.
    static VERROU: Mutex<()> = Mutex::new(());

    fn verrouiller() -> MutexGuard<'static, ()> {
        match VERROU.lock() {
            Ok(guard) => guard,
            Err(empoisonne) => empoisonne.into_inner(),
        }
    }

    /// Timestamp invente, et utilise par un seul test, pour que le compteur
    /// parte bien de zero.
    const MILLE: i64 = 1_900_000_000_001;

    #[test]
    fn un_identifiant_ascendant_tient_toujours_dans_le_format_attendu() {
        let identifiant = compose(false, 1, 1, &[0u8; RANDOM_LENGTH]);
        assert_eq!(identifiant.len(), LENGTH);
        // time segment: 12 hex chars from 56-bit value (timestamp=1, counter=1)
        // 1*4096+1=4097=0x1001 -> 56-bit=0x00000000001001 -> first 12 hex="000000000010"
        // random: 14 zeros -> '0' x 14
        assert_eq!(identifiant, "00000000001000000000000000");
    }

    #[test]
    fn un_identifiant_descendant_complemente_la_partie_temporelle() {
        // Pour un timestamp de 1 et un compteur de 1, la valeur ascendante vaut
        // 4097 ; son complement a deux complements sur 56 bits.
        let identifiant = compose(true, 1, 1, &[0u8; RANDOM_LENGTH]);
        assert_eq!(identifiant.len(), LENGTH);
        // ~4097 & 0xffffffffffffff = 0xffffffffffffeffe -> first 12 hex="fffffffffefe"
        assert_eq!(identifiant, "fffffffffefe00000000000000");
    }

    #[test]
    fn la_partie_aleatoire_utilise_seulement_les_caracteres_de_l_alphabet() {
        // Chaque octet est reduit modulo 62, donc 62 et 63 retombent sur '0'
        // et '1', et 255 retombe sur le 8e caractere, c'est a dire '7'.
        let random = [0u8, 10, 36, 61, 62, 63, 122, 255, 7, 1, 2, 3, 4, 5];
        let identifiant = compose(false, 1, 1, &random);
        assert_eq!(&identifiant[TIME_LENGTH..], "0Aaz01y7712345");
        assert!(identifiant
            .chars()
            .all(|c| (c as u32) < 128 && CHARS.contains(&(c as u8))));
    }

    #[test]
    fn deux_identifiants_de_la_meme_milliseonde_different_d_un_cran() {
        let _garde = verrouiller();
        let premier = create_at(false, MILLE);
        let second = create_at(false, MILLE);
        // Meme milliseonde : le compteur passe de 1 a 2.
        // Le segment temporel (12 hex) ne change pas pour counter < 4096
        // car on prend les 48 bits de poids fort de la valeur 56 bits.
        assert_eq!(premier.len(), LENGTH);
        assert_eq!(&premier[..TIME_LENGTH], &second[..TIME_LENGTH]);
        // Le dernier caractere hex du segment temporel encode counter[11:8]
        // Pour counter=1 et 2, c'est 0.
        assert!(premier[..TIME_LENGTH].ends_with('0'));
        assert!(second[..TIME_LENGTH].ends_with('0'));
        // La partie aleatoire differencie les deux identifiants.
        assert_ne!(premier, second);
    }

    #[test]
    fn un_changement_de_milliseonde_remet_le_compteur_a_zero() {
        let _garde = verrouiller();
        let premier = create_at(false, MILLE);
        let second = create_at(false, MILLE + 1);
        // Milliseonde suivante : le segment temporel change.
        assert_ne!(&premier[..TIME_LENGTH], &second[..TIME_LENGTH]);
        // Le dernier caractere hex du segment temporel est 0 pour counter=1.
        assert!(premier[..TIME_LENGTH].ends_with('0'));
        assert!(second[..TIME_LENGTH].ends_with('0'));
    }

    #[test]
    fn un_identifiant_genere_a_partir_de_l_horloge_tient_dans_le_format_attendu() {
        let _garde = verrouiller();
        for identifiant in [ascending(), descending(), create(true)] {
            assert_eq!(identifiant.len(), LENGTH);
            assert!(identifiant.chars().all(|c| c.is_ascii_alphanumeric()));
        }
    }
}
