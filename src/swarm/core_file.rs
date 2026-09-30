//! Portage Rust de `opencode/packages/core/src/file.ts`.
//!
//! Ce fichier TypeScript ne contient que six lignes, et il ne fait presque rien :
//!
//! ```ts
//! export * as File from "./file"        // reexport de lui-meme, sans effet
//! import { Revert } from "@opencode-ai/schema/revert"
//! export const Diff = Revert.FileDiff
//! export type Diff = typeof Diff.Type
//! ```
//!
//! Autrement dit, `file.ts` est un **alias**. Toute la definition de `Diff`
//! se trouve ailleurs, dans `packages/schema/src/revert.ts`, ou le type est
//! declare ainsi :
//!
//! ```ts
//! export const FileDiff = Schema.Struct({
//!   path: RelativePath,
//!   status: Schema.Literals(["added", "modified", "deleted"]),
//!   additions: NonNegativeInt,
//!   deletions: NonNegativeInt,
//!   patch: Schema.String,
//! }).annotate({ identifier: "File.Diff" })
//! ```
//!
//! On le porte quand meme, parce que `File.Diff` est un nom public utilise
//! ailleurs, et parce que l'alias doit exister quelque part en Rust. Le
//! contenu reel vient de `revert.ts` ; c'est ce fichier qui fait foi.
//!
//! Deux points meritent l'attention a la relecture.
//!
//! **1. Le nom `Diff` reste `Diff`.** Le TS exporte la valeur sous le nom
//! `Diff` et le type sous le meme nom. En Rust on ne peut pas avoir les deux
//! sous un seul identifiant, donc on garde `Diff` pour le struct : c'est le
//! nom utilise partout dans le code applicatif, et c'est celui qui apparait
//! dans les messages d'erreur.
//!
//! **2. Le champ `status` n'est PAS un objet avec un tag.** C'est le point
//! que la consigne signalait comme a surveiller, et la reponse est contre
//! intuitive : `Schema.Literals([...])` produit une **chaine nue**, pas un
//! `{ _tag: "added" }`. En JSON, une valeur de `status` vaut `"added"`, pas
//! `{ "type": "added" }`.
//!
//! Concretement, on ne doit **pas** mettre `#[serde(tag = "...")]` sur
//! `DiffStatus`. Serde externally-taguerait chaque variante en objet
//! `{"Added": ...}`, ce qui casse l'echange avec le TypeScript. On laisse
//! l'etalonnage externe par defaut, qui serialise une variante unitaire en
//! chaine. Les trois tags sont donnes explicitement, un par un, plutot que via
//! `rename_all`, pour qu'une relecture puisse les verifier d'un coup d'oeil et
//! qu'un changement de convention de casse ne puisse pas les deriver sans
//! qu'on le voie.
//!
//! Les cinq noms de champs (`path`, `status`, `additions`, `deletions`,
//! `patch`) sont des mots simples sans majuscule interne, donc aucun
//! `#[serde(rename = ...)]` n'est necessaire ici. C'est verifie par un test.

use serde::{Deserialize, Serialize};

/// Ce qu'un diff dit du sort d'un fichier.
///
/// Equivalent de `Schema.Literals(["added", "modified", "deleted"])`.
///
/// Les trois valeurs possibles sont exactement celles du TypeScript, dans le
/// meme ordre, et leur contenu JSON est une chaine nue. Voir la note 2 de
/// l'en-tete de module : surtout pas de `#[serde(tag = "...")]` ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DiffStatus {
    #[serde(rename = "added")]
    Added,
    #[serde(rename = "modified")]
    Modified,
    #[serde(rename = "deleted")]
    Deleted,
}

impl DiffStatus {
    /// Les trois valeurs, dans l'ordre declare par le TypeScript.
    pub const ALL: [DiffStatus; 3] = [DiffStatus::Added, DiffStatus::Modified, DiffStatus::Deleted];

    /// La chaine attendue dans le JSON.
    ///
    /// Utile quand on compare une valeur venue du TypeScript a une valeur
    /// construite en Rust : la comparaison se fait sur le texte, comme le fait
    /// le TypeScript, et non sur l'identifiant de variante Rust.
    pub fn as_str(self) -> &'static str {
        match self {
            DiffStatus::Added => "added",
            DiffStatus::Modified => "modified",
            DiffStatus::Deleted => "deleted",
        }
    }
}

/// Le statut, lu depuis la chaine JSON du TypeScript.
///
/// `serde` fait cet appel tout seul via le derive ; la fonction est exposee
/// parce qu'un message d'erreur parlant vaut mieux qu'un `expected one of`
/// quand la valeur vient d'un fichier de patch ecrit a la main.
pub fn parse_diff_status(raw: &str) -> Option<DiffStatus> {
    match raw {
        "added" => Some(DiffStatus::Added),
        "modified" => Some(DiffStatus::Modified),
        "deleted" => Some(DiffStatus::Deleted),
        _ => None,
    }
}

/// Le resultat d'une edition de fichier, fichier par fichier.
///
/// Equivalent de `Revert.FileDiff`, expose par `file.ts` sous le nom `Diff`
/// avec l'identifiant de schema `File.Diff`.
///
/// `path` porte un `brand("RelativePath")` cote TypeScript. Ce brand est une
/// verification de type a la compilation, qui disparait a l'execution et ne
/// se voit donc pas dans le JSON : un `String` ordinaire rend le meme service
/// ici, et un chemin relatif reste un chemin relatif.
///
/// `additions` et `deletions` sont des `NonNegativeInt`, c'est-a-dire des
/// entiers positifs ou nuls. On les type `u64` plutot que `i64` : la
/// contrainte du TypeScript devient une contrainte du compilateur Rust, donc
/// elle ne peut pas etre violee par accident, et un JSON contenant un nombre
/// negatif est refuse a la lecture, comme il l'est cote TypeScript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diff {
    /// Chemin du fichier touche, relatif a la racine du projet.
    pub path: String,
    /// Sort du fichier : ajoute, modifie ou supprime.
    pub status: DiffStatus,
    /// Nombre de lignes ajoutees. Jamais negatif.
    pub additions: u64,
    /// Nombre de lignes supprimees. Jamais negatif.
    pub deletions: u64,
    /// Le patch unifie, au format git. Vide si le fichier n'a aucun changement textuel.
    pub patch: String,
}

impl Diff {
    /// Construit un diff a partir de ses cinq champs.
    pub fn new(
        path: impl Into<String>,
        status: DiffStatus,
        additions: u64,
        deletions: u64,
        patch: impl Into<String>,
    ) -> Self {
        Self { path: path.into(), status, additions, deletions, patch: patch.into() }
    }

    /// Nombre total de lignes touchees, ajouts et suppressions confondus.
    ///
    /// Cette notion n'existe pas dans le TypeScript. Elle est quand meme
    /// ecrite parce que c'est le seul calcul que les deux compteurs permettent
    /// et qu'un resume de modification s'en sert partout.
    pub fn total_changed(&self) -> u64 {
        self.additions + self.deletions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff() -> Diff {
        Diff::new("src/main.rs", DiffStatus::Modified, 3, 1, "@@ -1 +1 @@\n-a\n+b")
    }

    #[test]
    fn chaque_statut_s_echappe_dans_le_json_comme_une_chaine_nue() {
        // Le point le plus important du portage : `Schema.Literals` donne une
        // chaine, pas un objet balise. Un `{"Added": ...}` serait faux.
        let attendu = [
            (DiffStatus::Added, "\"added\""),
            (DiffStatus::Modified, "\"modified\""),
            (DiffStatus::Deleted, "\"deleted\""),
        ];
        for (status, json) in attendu {
            assert_eq!(serde_json::to_string(&status).unwrap(), json);
        }
    }

    #[test]
    fn les_cinq_noms_de_champs_sont_ceux_du_typescript() {
        // Aucun nom en camelCase ici, donc aucun `rename`. Ce test le verrouille.
        let json = serde_json::to_string(&diff()).unwrap();
        assert_eq!(
            json,
            r#"{"path":"src/main.rs","status":"modified","additions":3,"deletions":1,"patch":"@@ -1 +1 @@\n-a\n+b"}"#
        );
    }

    #[test]
    fn un_diff_ecrit_en_json_par_le_typescript_est_relu_identique() {
        let brut = r#"{"path":"a.ts","status":"added","additions":0,"deletions":0,"patch":""}"#;
        let relu: Diff = serde_json::from_str(brut).unwrap();
        assert_eq!(relu.path, "a.ts");
        assert_eq!(relu.status, DiffStatus::Added);
        assert_eq!(relu.total_changed(), 0);
        // Et l'aller-retour ne perd rien.
        assert_eq!(serde_json::to_string(&relu).unwrap(), brut);
    }

    #[test]
    fn un_statut_inconnu_est_refuse() {
        // Un fichier de patch ecrit a la main peut contenir n'importe quoi.
        let brut = r#"{"path":"a.ts","status":"renamed","additions":1,"deletions":0,"patch":""}"#;
        assert!(serde_json::from_str::<Diff>(brut).is_err());
        assert_eq!(parse_diff_status("renamed"), None);
    }

    #[test]
    fn un_nombre_de_lignes_negatif_est_refuse() {
        // `NonNegativeInt` interdit le negatif des deux cotes ; ici c'est le type
        // lui-meme qui l'interdit, donc le JSON ne peut meme pas etre construit.
        let brut = r#"{"path":"a.ts","status":"modified","additions":-1,"deletions":0,"patch":""}"#;
        assert!(serde_json::from_str::<Diff>(brut).is_err());
    }

    #[test]
    fn les_trois_statuts_vont_et_viennent_du_json_sans_perte() {
        let lus: Vec<DiffStatus> =
            DiffStatus::ALL.iter().map(|s| parse_diff_status(s.as_str()).unwrap()).collect();
        assert_eq!(lus, vec![DiffStatus::Added, DiffStatus::Modified, DiffStatus::Deleted]);
    }

    #[test]
    fn un_fichier_ajoute_ne_compte_que_des_ajouts() {
        let d = Diff::new("nouveau.rs", DiffStatus::Added, 12, 0, "");
        assert_eq!(d.total_changed(), 12);
    }

    #[test]
    fn un_fichier_supprime_ne_compte_que_des_suppressions() {
        let d = Diff::new("obsolete.rs", DiffStatus::Deleted, 0, 40, "");
        assert_eq!(d.total_changed(), 40);
    }

    #[test]
    fn un_fichier_sans_changement_textuel_a_un_total_nul() {
        // Cas limite : un fichier marque modifie mais dont le patch est vide.
        let d = Diff::new("vide.rs", DiffStatus::Modified, 0, 0, "");
        assert_eq!(d.total_changed(), 0);
        assert!(d.patch.is_empty());
    }
}
