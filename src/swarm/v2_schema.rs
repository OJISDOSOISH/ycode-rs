//! Portage Rust de `opencode/packages/core/src/v2-schema.ts`.
//!
//! # Etat : rien n'est porte, et c'est voulu
//!
//! La source tient en deux lignes, et ce sont deux reexports. Elle ne contient
//! aucune definition de type, donc il n'y a rien a retranscrire :
//!
//! ```text
//! export * as V2Schema from "./v2-schema"
//! export { DateTimeUtcFromMillis } from "@opencode-ai/schema/schema"
//! ```
//!
//! Ligne 1 : c'est un reexport d'espace de noms qui pointe sur le fichier
//! LUI-MEME (le module se sous-aliasse). C'est du code mort en TypeScript, et un
//! alias de module n'a pas d'equivalent en Rust : rien a ecrire. Le motif est
//! recurrent dans opencode (on le voit aussi dans `wildcard.ts` et
//! `config/command.ts`), ce n'est donc pas un fichier voisin a aller chercher.
//!
//! Ligne 2 : c'est un renommage pur, `DateTimeUtcFromMillis` vient du paquet
//! `@opencode-ai/schema`. La definition reelle est a deux niveaux plus bas :
//!
//! - `packages/schema/src/schema.ts`, ligne 25 :
//!   `Schema.Finite` decode vers `Schema.DateTimeUtc`, donc un nombre fini de
//!   millisecondes depuis l'epoque, converti en date UTC a la lecture et
//!   reconverti en millisecondes a l'ecriture.
//!
//! Ce type **n'existe pas encore en Rust**. Le plus proche est
//! `crate::schema::Millis` (`pub type Millis = i64`, dans
//! `src/schema/session_message.rs`), mais c'est un alias utilitaire ajoute par
//! ce portage, pas le schema `DateTimeUtcFromMillis` : il ne valide rien, n'a
//! ni encodeur ni decodeur, et ne vit pas dans le bon module.
//!
//! Conformement a la regle de ce lot, on ne fabrique pas le type manquant. Ce
//! fichier se limite donc a documenter la provenance et a donner au portage
//! futur les coordonnees exactes de la definition a lire.
//!
//! # Piege des noms de champs : sans objet ici
//!
//! La source ne declare aucun struct et aucun objet, donc aucun nom de champ
//! JSON, et en particulier aucun `projectID` a orthographier en majuscules. Le
//! piege n'apparaitra que le jour ou le type sera porte ; `CHAMPS_JSON` le
//! consigne des lors.
//!
//! # Integration
//!
//! `pub mod v2_schema;` manque dans `src/swarm/mod.rs`. Ce module n'est donc pas
//! compile tant que l'integrateur ne l'a pas ajoute (regle du lot : on ne touche
//! pas a `mod.rs`).

/// Chemin, relatif a la racine du depot `opencode`, du fichier TypeScript dont
/// ce module est le portage.
pub const TS_SOURCE: &str = "packages/core/src/v2-schema.ts";

/// Nom de l'espace de noms cree par la ligne 1. Il correspond au nom du fichier
/// sans extension, et ne designe aucun type en Rust.
pub const NAMESPACE_REEXPORT: &str = "V2Schema";

/// Symbole reexporte par la ligne 2, tel qu'il apparait dans le TS.
pub const REEXPORTED_SYMBOL: &str = "DateTimeUtcFromMillis";

/// Fichier qui porte reellement la definition de `REEXPORTED_SYMBOL`, dans le
/// paquet `@opencode-ai/schema`.
pub const UPSTREAM_MODULE: &str = "packages/schema/src/schema.ts";

/// Ligne de `UPSTREAM_MODULE` ou `DateTimeUtcFromMillis` est ecrit. Le fichier
/// fait 30 lignes, donc la valeur est forcement comprise entre 1 et 30.
pub const UPSTREAM_DEFINITION_LINE: usize = 25;

/// Nombre de lignes de `UPSTREAM_MODULE`, pour que la ligne ci-dessus reste
/// verifiable si le fichier d'origine evolue.
pub const UPSTREAM_MODULE_LINES: usize = 30;

/// Items de schema effectivement portes par ce module.
///
/// Vide, et c'est l'etat attendu : la source n'en declare aucun. Ce n'est pas
/// une liste de noms auxe, c'est le constat de non-portabilite. Le remplir est
/// le travail de l'agent qui portera `DateTimeUtcFromMillis`.
pub const PORTED_ITEMS: &[&str] = &[];

/// Noms de champs JSON exposes par ce module.
///
/// Vide pour la meme raison que `PORTED_ITEMS`. Le piege `projectID` (et non
/// `projectId`) n'a pas d'application tant que la liste est vide ; il n'a pas
/// ete oublie, il n'a pas encore de cible.
pub const CHAMPS_JSON: &[&str] = &[];

#[cfg(test)]
mod tests {
    use super::*;

    /// La ligne 1 du TS ne peut rien apporter a Rust : ni type, ni fonction.
    #[test]
    fn ligne_1_est_un_reexport_d_espace_de_noms_sans_type() {
        assert!(TS_SOURCE.ends_with("v2-schema.ts"), "le nom de fichier a change : {TS_SOURCE}");

        let nom = TS_SOURCE.rsplit('/').next().expect("le chemin contient un dossier");
        let stem = nom.strip_suffix(".ts").expect("l'extension est .ts");
        let mut attendu = String::new();
        for segment in stem.split('-') {
            let mut chars = segment.chars();
            if let Some(premier) = chars.next() {
                attendu.extend(premier.to_uppercase());
                attendu.push_str(chars.as_str());
            }
        }
        // "v2-schema" devient "V2Schema", ce qui est bien le nom du TS.
        assert_eq!(NAMESPACE_REEXPORT, attendu, "le reexport doit suivre le nom du fichier");

        assert!(
            PORTED_ITEMS.is_empty(),
            "la source ne declare aucun item ; si cette assertion echoue, le type a ete porte:\
             completer PORTED_ITEMS et la documentation du module"
        );
    }

    /// La ligne 2 du TS ne fait que renommer un symbole du paquet schema.
    #[test]
    fn ligne_2_reexporte_le_horodatage_du_paquet_schema() {
        assert_eq!(REEXPORTED_SYMBOL, "DateTimeUtcFromMillis");
        assert!(UPSTREAM_MODULE.ends_with("schema.ts"), "mauvais module amont : {UPSTREAM_MODULE}");
        assert!(
            UPSTREAM_DEFINITION_LINE >= 1 && UPSTREAM_DEFINITION_LINE <= UPSTREAM_MODULE_LINES,
            "la ligne {UPSTREAM_DEFINITION_LINE} sort du fichier amont de {UPSTREAM_MODULE_LINES} lignes"
        );
    }

    /// Garde-fou du piege des majuscules : aucun nom de champ tant que rien n'est porte.
    #[test]
    fn aucun_nom_de_champ_json_a_orthographier() {
        assert!(
            CHAMPS_JSON.is_empty(),
            "des champs JSON sont desormais portes : chacun doit s'ecrire exactement comme dans le TS,\
             majuscules comprises (projectID, pas projectId)"
        );
    }
}
