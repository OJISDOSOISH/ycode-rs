//! Portage Rust de `opencode/packages/core/src/project/schema.ts`.
//!
//! ## Ce que porte vraiment la source
//!
//! Le fichier d'origine fait **16 lignes** :
//!
//! ```ts
//! export * as ProjectSchema from "./schema"
//!
//! import { Schema } from "effect"
//! import { Project } from "@opencode-ai/schema/project"
//! import { AbsolutePath } from "../schema"
//!
//! export const ID = Project.ID
//! export type ID = typeof ID.Type
//!
//! export const Vcs = Schema.Union([
//!   Schema.Struct({
//!     type: Schema.Literal("git"),
//!     store: AbsolutePath,
//!   }),
//! ])
//! export type Vcs = typeof Vcs.Type
//! ```
//!
//! Sur ces 16 lignes, il n'y a que **deux** choses a porter, et il faut les
//! traiter tres differemment.
//!
//! La premiere est un **reexport**. `Project.ID` vient de
//! `packages/schema/src/project-id.ts`, et c'est une chaine de caracteres
//! marquee ("branded") : `Schema.String.pipe(Schema.brand("Project.ID"))`. Le
//! nom de marque n'existe qu'a la compilation TypeScript, a l'execution c'est un
//! `string` ordinaire. Le contrat de l'identifiant de projet est donc
//! **reexporte**, pas recopie : voir plus bas pourquoi.
//!
//! La deuxieme est le **seul vrai contenu** de ce fichier : le contrat `Vcs`.
//!
//! ## Pourquoi on reexporte au lieu de redeclarer
//!
//! `src/core/session/schema.rs` porte deja `pub type ProjectId = String;` et
//! `pub type AbsolutePath = String;`. Les redeclarer ici donnerait deux
//! definitions independantes du meme contrat, qui peuvent diverger en silence
//! sans qu'aucun compilateur ne s'en apercoive. C'est exactement le defaut que
//! la revue croisee a deja attrape une fois, sur `sessionID` / `callID`. Un
//! simple `pub use` rend les deux modules solidaires litteralement.
//!
//! Ce couplage a un cout, et il faut l'assumer : `core::session::schema` est
//! documente comme **intermediaire**, avec l'intention affichee de se reduire
//! un jour a des `pub use crate::schema::...`. Le jour ou ces deux alias
//! remonteront dans `crate::schema`, la seule ligne a changer ici sera le
//! `use`. C'est un renommage de chemin, pas une reecriture de contrat.
//!
//! ## Le point piege : une union d'un seul membre
//!
//! `Schema.Union([...])` ne contient ici **qu'un seul** schema. L'ADT n'a donc
//! aucune variante a choisir, et se comporte exactement comme le struct qu'il
//! contient. Le porter par un `struct` n'est donc pas un raccourci : c'est la
//! traduction exacte.
//!
//! Ce n'est pas un choix fige pour autant. Le jour ou opencode ajoute un
//! second gestionnaire de versions, l'union gagne une variante et le struct
//! doit alors devenir un `enum` tagge par `type`. Tant qu'il n'y a que git, un
//! `enum` d'une seule variante n'apporterait rien et couterait un niveau
//! d'indirection a chaque lecture.
//!
//! ## Deux `Vcs` distincts dans le depot, et ne pas les confondre
//!
//! `packages/schema/src/project.ts` definit **aussi** un `Vcs`, ligne 11 :
//!
//! ```ts
//! export const Vcs = Schema.Literal("git").annotate({ identifier: "Project.Vcs" })
//! ```
//!
//! Celui-la est un `Literal` **nu** : une chaine `"git"`, pas un objet. Celui
//! qu'on porte ici, dans la couche core, est l'objet `{ type, store }`, qui dit
//! en plus **ou** se trouve le depot git. Les deux coexistent dans le
//! TypeScript d'origine et sont tous deux utilises. Les confondre casserait
//! `Project.resolve` et le file watcher de
//! `packages/core/src/filesystem/watcher.ts`, qui teste
//! `location.vcs?.type === "git"` avant de s'abonner a `.git`. C'est le point de
//! verification le plus important de ce fichier.
//!
//! ## Le champ `type`
//!
//! Deux difficultes se superposent sur ce seul champ.
//!
//! D'abord, `type` est un mot cle reserve en Rust. Le champ s'appelle donc
//! `r#type` (identifiant brut), et porte le `#[serde(rename = "type")]` impose
//! par la regle des noms de champs.
//!
//! Ensuite, `Schema.Literal("git")` n'est pas un `String` : c'est une ADT a une
//! seule valeur, qui **refuse** de decoder autre chose que `"git"`. C'est
//! exactement la forme d'un `enum` a variante unique, d'ou `VcsType`. Un simple
//! `String` aurait serialize la meme chose mais aurait laisse passer n'importe
//! quoi a la deserialisation, ce qui aurait ete une perte de fidelite.

use serde::{Deserialize, Serialize};

use crate::core::session::schema::AbsolutePath;

/// Identifiant de projet.
///
/// Reexport de `crate::core::session::schema::ProjectId`, qui correspond a
/// `Project.ID` de `packages/schema/src/project-id.ts`. Comme tous les identifiants
/// du projet, c'est une chaine de caracteres dont le nom de marque est efface a
/// l'execution.
///
/// On le reexporte plutot que de le redeclarer : voir l'en-tete du module.
pub use crate::core::session::schema::ProjectId;

/// Identifiant du projet qui ne correspond a aucun depot git.
///
/// Cote TypeScript, `ProjectID` s'enrichit d'une methode statique :
///
/// ```ts
/// export const ProjectID = Schema.String.pipe(
///   Schema.brand("Project.ID"),
///   statics((schema) => ({ global: schema.make("global") })),
/// )
/// ```
///
/// `packages/core/src/project.ts` s'en sert comme valeur de repli, aux lignes
/// 112 et 118 : quand le repertoire demande n'est dans aucun depot, et quand
/// aucune remote, aucune valeur en cache et aucun commit racine n'ont pu etre
/// resolus. C'est bien une valeur du **contrat** publiee dans le JSON, et pas un
/// detail d'implementation, d'ou sa presence ici.
///
/// Le type est `&str` et non `ProjectId` : une allocation de `String` n'est pas
/// evaluable a la compilation, donc `const X: String = "..."` ne compile pas.
/// C'est aussi la forme retenue par `DEFAULT_VARIANT` dans
/// `src/core/session/schema.rs`. Un `.to_string()` au moment de l'appel donne le
/// `ProjectId` attendu.
pub const GLOBAL_PROJECT_ID: &str = "global";

/// Gestionnaire de versions reconnu pour un projet.
///
/// Unique variante de l'ADT `Schema.Literal("git")` de la source. Le nom
/// `VcsType` evite la collision avec le type `Vcs`, qui est l'objet complet.
///
/// Le nom de la variante est explicitement renomme : la convention du fichier
/// neighbour `src/core/session/schema.rs` serait `rename_all = "lowercase"`,
/// qui donnerait le meme resultat ici mais sur un nom de variante qui n'existe
/// pas dans le TypeScript d'origine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VcsType {
    /// Depot git. La seule valeur que la source accepte.
    #[serde(rename = "git")]
    Git,
}

/// Localisation du depot git d'un projet.
///
/// Correspond a `Vcs` de `packages/core/src/project/schema.ts`.
///
/// L'ordre des champs reprend celui de la source. Il n'a aucune signification
/// en JSON, mais il rend la comparaison avec l'original lisible, et ca ne coute
/// rien.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vcs {
    /// Gestionnaire de versions. Toujours `git`.
    ///
    /// `type` est un mot cle reserve en Rust, d'ou l'identifiant brut `r#type`.
    /// Le nom JSON est pose explicitement, comme l'impose la regle des noms de
    /// champs.
    #[serde(rename = "type")]
    pub r#type: VcsType,
    /// Repertoire physique du depot git.
    ///
    /// Chemin **absolu**, et plus precisement le repertoire git *commun* : dans
    /// `packages/core/src/project.ts` ligne 120, il vient de
    /// `repo.commonDirectory` de `simple-git`. Pour un worktree, c'est donc le
    /// `.git` du depot d'origine, partage avec les autres worktrees, et non le
    /// fichier `.git` du worktree lui-meme. C'est ce chemin que le file watcher
    /// parcourt pour s'abonner aux evenements.
    pub store: AbsolutePath,
}

impl Vcs {
    /// Construit une localisation git pour le depot donne.
    ///
    /// Raccourci qui garantit la valeur de `type` sans avoir a l'ecrire. Les
    /// deux appels suivants produisent le meme objet :
    ///
    /// ```ignore
    /// Vcs { r#type: VcsType::Git, store }
    /// Vcs::git(store)
    /// ```
    pub fn git(store: AbsolutePath) -> Self {
        Vcs { r#type: VcsType::Git, store }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_vcs_serialise_le_type_et_le_depot() {
        // Les deux noms de champs du JSON, verifiees dans le sens de
        // l'encodage. Une faute sur `store` passerait la compilation.
        let vcs = Vcs::git("/srv/depot/.git".to_string());
        let json = serde_json::to_value(&vcs).unwrap();
        assert_eq!(json["type"], "git");
        assert_eq!(json["store"], "/srv/depot/.git");
    }

    #[test]
    fn le_type_vcs_vaut_toujours_git() {
        // Le constructeur ne laisse pas le choix, et c'est la seule valeur
        // que l'enum sait produire.
        assert_eq!(Vcs::git("/x".to_string()).r#type, VcsType::Git);
        assert_eq!(serde_json::to_string(&VcsType::Git).unwrap(), "\"git\"");
    }

    #[test]
    fn un_type_vcs_inconnu_est_refuse_a_la_lecture() {
        // `Schema.Literal("git")` refuse tout le reste. C'est tout l'interet
        // d'avoir un enum a variante unique plutot qu'un `String` : un champ
        // `String` aurait accepte les deux sans broncher.
        for inconnu in [r#"{"type":"mercurial","store":"/x"}"#, r#"{"type":"Git","store":"/x"}"#] {
            assert!(
                serde_json::from_str::<Vcs>(inconnu).is_err(),
                "{inconnu} ne devrait pas etre accepte"
            );
        }
    }

    #[test]
    fn un_vcs_lu_depuis_le_json_reprend_le_depot() {
        // Sens inverse : ce que le TypeScript produit doit se relire tel quel.
        let json = r#"{"type":"git","store":"/srv/depot/.git"}"#;
        let vcs: Vcs = serde_json::from_str(json).unwrap();
        assert_eq!(vcs.r#type, VcsType::Git);
        assert_eq!(vcs.store, "/srv/depot/.git");
    }

    #[test]
    fn un_vcs_sans_type_est_refuse() {
        // Champ obligatoire des deux cotes : `Schema.Literal` n'a pas de
        // valeur par defaut, donc l'absence de la cle doit echouer.
        assert!(serde_json::from_str::<Vcs>(r#"{"store":"/x"}"#).is_err());
    }

    #[test]
    fn l_identifiant_global_est_la_chaine_global() {
        // La valeur de repli de `Project.resolve`. Elle est publiee dans le
        // JSON comme n'importe quel autre identifiant, donc `const` et non
        // `fn`.
        assert_eq!(GLOBAL_PROJECT_ID, "global");
    }

    #[test]
    fn un_chemin_vide_est_accepte_comme_chemin() {
        // `AbsolutePath` est une marque sur un `String`, sans contrainte de
        // contenu de cote Rust. Une chaine vide traverse donc le contrat
        // telle quelle. Attention : c'est le comportement de **ce** schema, et
        // il ne dit rien du nettoyage fait plus haut, dans `project.ts`, ou la
        // ligne 67 utilise un ternaire (`value ? ... : undefined`) et non un
        // coalescent. Une chaine vide y disparait, ici elle survit.
        let vcs = Vcs::git(String::new());
        let json = serde_json::to_value(&vcs).unwrap();
        assert_eq!(json["store"], "");
        assert_eq!(serde_json::from_str::<Vcs>(r#"{"type":"git","store":""}"#).unwrap().store, "");
    }
}
