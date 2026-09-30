//! Portage de `packages/core/src/observability/shared.ts`.
//!
//! La source tient en une seule ligne :
//!
//! ```ts
//! export const runID = crypto.randomUUID().slice(0, 8)
//! ```
//!
//! Elle ne definit aucun type, aucune classe et aucune fonction : seulement un
//! identifiant d execution, tire au hasard au chargement du module et partage
//! par les deux consommateurs du dossier.
//!
//! - `logging.ts` s en sert de valeur par defaut pour l etiquette `run` du
//!   journal, et comme second parametre de `fileLogger`.
//! - `otlp.ts` le publie dans les attributs `opencode.run` et
//!   `service.instance.id`.
//!
//! ## Les deux difficultes du portage
//!
//! 1. `crypto.randomUUID()` rend un UUID version 4, donc une chaine canonique
//!    de la forme `1b9d6bcd-bbfd-4b2d-9b5d-ab8dfbbd4bed`. Les huit premiers
//!    caracteres forment le premier groupe entier de la chaine : ils ne
//!    contiennent que de l hexadecimal minuscule, et le separateur `-` n arrive
//!    qu en neuvieme position. Le resultat fait donc toujours exactement huit
//!    caracteres, jamais moins, et ne contient jamais de separateur.
//! 2. Rust n autorise pas de `const` calcule a l execution. Rendre une fonction
//!    `run_id()` qui tirerait une nouvelle valeur a chaque appel serait faux :
//!    la source en calcule une seule par processus et la reutilise partout. La
//!    semantique est donc conservee avec un `OnceLock`, initialise au premier
//!    appel, et `run_id()` renvoie ensuite toujours la meme chaine, allouee une
//!    seule fois.
//!
//! Le nom passe de `runID` a `run_id` pour respecter la convention Rust, comme
//! le veut le projet. Il n y a aucun nom de champ dans ce fichier, donc aucun
//! `serde(rename = ...)` a poser.
//!
//! ## Note sur l aleatoire
//!
//! L identifiant n a pas besoin d etre reproductible ni previsible : il sert
//! seulement a distinguer deux executions. La crate `uuid` du projet n active
//! pas la feature `v4` (seulement `v7` et `serde`, voir `Cargo.toml`), et
//! `Cargo.toml` n est pas de mon ressort, donc les bits sont tires de
//! `RandomState`, dont la cle est fournie par le systeme a chaque creation. La
//! granularite est celle de la source : huit caracteres hexadecimaux, soit
//! 32 bits.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Emplacement unique de l identifiant, initialise au premier appel de `run_id`.
static RUN_ID: OnceLock<String> = OnceLock::new();

/// Renvoie l identifiant aleatoire de l execution courante.
///
/// La valeur est calculee une seule fois par processus et reste identique pour
/// tous les appels suivants.
pub fn run_id() -> &'static str {
    RUN_ID.get_or_init(generer_run_id)
}

/// Tire un identifiant neuf, independamment de celui deja memorise.
///
/// Ne s appelle pas directement : c est l initialisation paresseuse de
/// `run_id`, et les tests.
fn generer_run_id() -> String {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(nanosecondes_depuis_epoch());
    // 32 bits, comme les huit caracteres hexadecimaux de la source.
    format!("{:08x}", hasher.finish() as u32)
}

/// Nanosecondes ecoulees depuis l epoch, ou zero si l horloge est en amont.
fn nanosecondes_depuis_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|ecoule| ecoule.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{generer_run_id, run_id};
    use std::collections::BTreeSet;

    #[test]
    fn un_run_id_fait_huit_caracteres() {
        assert_eq!(run_id().len(), 8);
    }

    #[test]
    fn un_run_id_ne_contient_que_de_l_hexadecimal_minuscule() {
        for caractere in run_id().chars() {
            assert!(
                caractere.is_ascii_digit() || ('a'..='f').contains(&caractere),
                "caractere inattendu dans le run id : {:?}",
                caractere
            );
        }
    }

    #[test]
    fn un_run_id_ne_contient_pas_le_separateur_d_un_uuid() {
        // Le tiret d un uuid canonique n arrive qu en neuvieme position, donc
        // le run id, qui est la tranche des huit premiers caracteres, n en a pas.
        assert!(!run_id().contains('-'));
    }

    #[test]
    fn le_run_id_du_processus_ne_change_pas_d_un_appel_a_l_autre() {
        let premier = run_id();
        let second = run_id();
        assert_eq!(premier, second);
    }

    #[test]
    fn le_run_id_du_processus_est_calcule_une_seule_fois() {
        // Meme pointeur, donc une seule allocation : une fonction qui regnerait
        // une valeur a chaque appel donnerait des adresses differentes.
        assert_eq!(run_id().as_ptr(), run_id().as_ptr());
    }

    #[test]
    fn deux_generations_successives_donnent_des_run_id_differents() {
        let vus: BTreeSet<String> = (0..16).map(|_| generer_run_id()).collect();
        assert!(
            vus.len() > 1,
            "seize generations ne devraient pas toutes rendre la meme valeur"
        );
    }

    #[test]
    fn chaque_generation_respecte_le_format_attendu() {
        for _ in 0..16 {
            let valeur = generer_run_id();
            assert_eq!(valeur.len(), 8);
            assert!(valeur.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
        }
    }
}
