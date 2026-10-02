//! Portage Rust de `packages/core/src/database/schema.sql.ts`.
//!
//! ## Ce que contient la source
//!
//! Dix lignes, un seul export, aucune table :
//!
//! ```ts
//! import { integer } from "drizzle-orm/sqlite-core"
//! export const Timestamps = {
//!   time_created: integer().notNull().$default(() => Date.now()),
//!   time_updated: integer().notNull().$onUpdate(() => Date.now()),
//! }
//! ```
//!
//! `Timestamps` est un melange (mixin) etale dans chaque `sqliteTable` du
//! projet (`...Timestamps`). Il porte deux colonnes `integer NOT NULL`, dont
//! la valeur est en millisecondes depuis l'epoch (`Date.now()`).
//!
//! ## La difference entre les deux colonnes, qui est reelle
//!
//! - `time_created` porte `$default` : l'ORM pose la valeur a l'insertion quand
//!   elle est absente. Il n'y a pas de `DEFAULT` dans le DDL genere (voir
//!   `schema.gen.ts` : `` `time_created` integer NOT NULL ``, sans defaut) :
//!   la base, elle, exige la colonne.
//! - `time_updated` porte `$onUpdate` : l'ORM repose `Date.now()` a chaque
//!   mise a jour. Meme consequence : pas de defaut cote base.
//!
//! [`Horodatages::new`] pose les deux champs sur le meme instant (creation),
//! [`Horodatages::toucher`] ne bouge que `time_updated` (mise a jour). L'instant
//! est fourni par l'appelant, jamais lu dans l'horloge : le module reste pur.
//!
//! ## Ce qui n'est pas determine par la source
//!
//! Ni la declaration `integer()` ni l'instantane de `drizzle-kit` ne disent
//! l'unite. C'est l'usage (`Date.now()`) qui la donne : millisecondes. Un
//! appelant qui comparerait ces colonnes a des secondes Unix serait faux sans
//! aucun signal, comme sur `account.id` voisin.

use serde::{Deserialize, Serialize};

/// Nom de la colonne de creation, tel que la source l'ecrit.
pub const COLONNE_CREATION: &str = "time_created";

/// Nom de la colonne de mise a jour, tel que la source l'ecrit.
pub const COLONNE_MAJ: &str = "time_updated";

/// Les deux colonnes du melange, dans l'ordre de la source.
pub const COLONNES: [&str; 2] = [COLONNE_CREATION, COLONNE_MAJ];

/// Les deux horodatages poses par le melange `Timestamps`.
///
/// Millisecondes depuis l'epoch (`Date.now()`). Les champs sont dans l'ordre
/// des colonnes du DDL genere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Horodatages {
    /// Pose par `$default(() => Date.now())` a l'insertion, jamais retouche.
    #[serde(rename = "time_created")]
    pub time_created: i64,
    /// Repose par `$onUpdate(() => Date.now())` a chaque mise a jour.
    #[serde(rename = "time_updated")]
    pub time_updated: i64,
}

impl Horodatages {
    /// Construit les deux horodatages a la creation, sur le meme instant.
    pub fn new(maintenant: i64) -> Self {
        Self {
            time_created: maintenant,
            time_updated: maintenant,
        }
    }

    /// Applique une mise a jour : seul `time_updated` bouge.
    ///
    /// C'est la traduction de `$onUpdate(() => Date.now())`. La date de
    /// creation ne recule jamais.
    pub fn toucher(&mut self, maintenant: i64) {
        self.time_updated = maintenant;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_melange_porte_exactement_deux_colonnes_snake_case() {
        assert_eq!(COLONNES, ["time_created", "time_updated"]);
        assert_eq!(COLONNE_CREATION, "time_created");
        assert_eq!(COLONNE_MAJ, "time_updated");
    }

    #[test]
    fn la_creation_pose_les_deux_horodatages_sur_le_meme_instant() {
        let h = Horodatages::new(1_700_000_000_000);
        assert_eq!(h.time_created, 1_700_000_000_000);
        assert_eq!(h.time_updated, 1_700_000_000_000);
    }

    #[test]
    fn une_mise_a_jour_ne_bouge_que_l_horodatage_de_maj() {
        let mut h = Horodatages::new(100);
        h.toucher(500);
        assert_eq!(h.time_updated, 500);
        assert_eq!(h.time_created, 100, "la date de creation ne recule jamais");
    }

    #[test]
    fn la_ligne_serialise_les_noms_de_colonnes_du_ddl() {
        let json = serde_json::to_value(Horodatages::new(42)).unwrap();
        assert_eq!(json["time_created"], 42);
        assert_eq!(json["time_updated"], 42);
        assert_eq!(json.as_object().unwrap().len(), 2);
        assert!(json.get("timeCreated").is_none());
        assert!(json.get("timeUpdated").is_none());
    }

    #[test]
    fn un_aller_retour_json_conserve_les_deux_instants() {
        let h = Horodatages::new(7);
        let relu: Horodatages =
            serde_json::from_str(&serde_json::to_string(&h).unwrap()).unwrap();
        assert_eq!(relu, h);
    }
}
