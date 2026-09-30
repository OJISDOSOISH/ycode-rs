//! Portage Rust de `opencode/packages/core/src/config/watcher.ts`.
//!
//! ## Ce que porte vraiment la source
//!
//! Le fichier d'origine fait **sept lignes** :
//!
//! ```ts
//! export * as ConfigWatcher from "./watcher"
//!
//! import { Schema } from "effect"
//!
//! export class Info extends Schema.Class<Info>("ConfigV2.Watcher")({
//!   ignore: Schema.String.pipe(Schema.Array, Schema.optional),
//! }) {}
//! ```
//!
//! Il y a trois choses dedans, dont **deux qui ne se traduisent pas**.
//!
//! 1. `export * as ConfigWatcher from "./watcher"` est l'auto-reexport du
//!    module sous son propre nom. En TypeScript il cree l'alias d'espace de
//!    noms `ConfigWatcher`, ce qui permet d'ecrire `ConfigWatcher.Info` dans
//!    `packages/core/src/config.ts`. Rust n'a pas d'alias de ce genre : la
//!    hierarchie de modules joue le meme role, et l'import se fera
//!    `use crate::swarm::config_watcher::Info`. Il n'y a donc rien a ecrire,
//!    et c'est volontaire, pas oublie.
//! 2. `Schema.Class<Info>("ConfigV2.Watcher")` est un `struct` derives, avec
//!    l'identifiant de schema `"ConfigV2.Watcher"`. Cet identifiant ne sert
//!    qu'au message d'erreur du decodeur TypeScript ; Serde ne produit rien de
//!    tel. Il est perdu, comme dans tous les autres `Schema.Class` du portage.
//! 3. Le champ `ignore` est le **seul contenu reel** de ce fichier.
//!
//! ## Le point important : il n'y a AUCUNE surveillance ici
//!
//! Malgre le nom du module, ce fichier **n'observe rien**. Il ne cree aucun
//! thread, n'inscrit aucun chemin, n'abonne rien et n'emet aucun evenement.
//! Il ne contient qu'une **forme de donnees** : la liste des motifs que
//! l'utilisateur veut ignorer, telle qu'ecrite dans le fichier de
//! configuration.
//!
//! Cote TypeScript, tout ce qui reagit reellement au systeme de fichiers vit
//! dans un **autre** fichier, `packages/core/src/filesystem/watcher.ts`. C'est
//! lui qui charge `@parcel/watcher` (liaison native), qui appelle
//! `subscribe(...)`, et qui recoit les evenements. La seule ligne de ce
//! fichier-la qui touche a ce mecanisme est d'ailleurs un `?? []` :
//!
//! ```ts
//! .flatMap((item) => item.info.watcher?.ignore ?? [])
//! ```
//!
//! Aucun crate de surveillance n'est declare dans `Cargo.toml` : ni `notify`,
//! ni `watchexec`, ni equivalent. **On ne fabrique donc aucune mecanique de
//! surveillance ici.** Ni thread, ni canal, ni evenement, ni comparaison d'un
//! motif a un chemin. Le seul point de contact possible est
//! `ignore_or_empty`, qui reproduit le `?? []` de la source et rien de plus.
//! Ce qui se brancherait dessus est l'affaire du portage de
//! `filesystem/watcher.ts`, pas de ce fichier. Aucune dependance n'a ete
//! ajoutee.
//!
//! ## La forme exacte du champ `ignore`
//!
//! `Schema.String.pipe(Schema.Array, Schema.optional)` se lit de gauche a
//! droite : `String`, puis `Array`, puis `optional`. Le champ est donc
//! `Array<string> | undefined`, place dans la configuration sous la cle
//! `watcher` de `packages/core/src/config.ts`, ligne 69, lui-meme optionnel.
//!
//! - `Vec<String>` et non `BTreeSet<String>` : la source dit `Array`, pas
//!   `Set`. L'ordre des motifs est significatif cote TypeScript, puisque le
//!   consumer les concatene dans l'ordre a `[...Ignore.PATTERNS, ...config,
//!   ...protecteds(...)]` avant de les passer a `subscribe`. Un `BTreeSet`
//!   trierait les motifs et perdrait cet ordre.
//! - `Option<Vec<String>>` avec `default` et `skip_serializing_if`, les deux
//!   etant necessaires : le premier pour lire un objet sans la cle, le second
//!   pour ne pas ecrire un `ignore: null` que le TypeScript n'ecrit jamais.
//! - Pas de `#[serde(rename = ...)]` : le nom `ignore` est deja identique en
//!   TypeScript et en Rust, il ne contient aucune majuscule interne. Un test
//!   verifie quand meme le nom au JSON, parce que c'est l'erreur la plus
//!   frequente de ce portage.
//! - Une chaine vide reste une chaine vide. Le `?? []` du consumer teste la
//!   nullite, pas la veracite, donc un motif `""` le traverse intact. Un
//!   ternaire l'aurait supprime. Voir le test dedie.
//!
//! ## Le point NON verifie : `null` dans `ignore`
//!
//! Le test `un_null_se_decode_en_absence_cote_serde` verrouille
//! `{"ignore": null}` comme une absence. Ce qui est **certain**, c'est le
//! comportement de Serde : `null` se decode en `None` pour un `Option`, sans
//! qu'aucune option ne le demande. Ce qui ne l'est **pas**, c'est la parite
//! avec Effect.
//!
//! L'hypothese a trancher est la suivante : `Schema.optional` n'etant pas
//! appele avec `{ exact: true }`, il traiterait `null` comme `undefined`.
//! **Elle n'a pas pu etre verifiee dans ce depot**, et voici ce qui a ete
//! cherche :
//!
//! - `Schema.optional` sert des centaines de champs du depot
//!   (`packages/schema/src/v1/*.ts`, `packages/protocol/src/groups/session.ts`,
//!   `packages/core/src/config.ts`...), mais **jamais** avec un cas `null`
//!   pose a cote, et aucune source de schema n'emploie `exact: true`. Aucune
//!   autre utilisation ne permet donc de trancher.
//! - Le decodeur d Effect n'est pas interrogeable ici : `node_modules` est
//!   absent du depot opencode, donc aucun `Schema.decodeUnknown` ne peut etre
//!   execute pour verifier le comportement reel.
//! - `patches/effect@4.0.0-beta.83.patch` ne touche que `HttpApiSchema`, pas
//!   `PropertySignature` : il ne modifie donc pas ce comportement de notre
//!   cote.
//!
//! Consequence a connaitre : si Effect refuse `null` sur une propriete
//! optionnelle non exacte, alors `{"ignore": null}` **est** une erreur de
//! decodage cote TypeScript, la ou le derive Serde l'accepte ici. Le test
//! reste en place parce qu'il verrouille une verite qui, elle, ne depend pas
//! d Effect : ce que fait Serde. Le jour ou la parite est etablie, le
//! correctif tient en un `deserialize_with` qui refuse `null` sur le champ,
//! et rien d'autre ne change.
//!
//! L'hypothese se propage au niveau superieur : `config.ts` ligne 69 declare
//! `watcher: ConfigWatcher.Info.pipe(Schema.optional)`, donc `{"watcher":
//! null}` est soumis a la meme question. Ce fichier ne porte que le niveau
//! interieur.
//!
//! ## Rappel pour l'agent principal
//!
//! Le module **est** declare : `src/swarm/mod.rs` porte `pub mod
//! config_watcher;` a la ligne 29.
//!
//! Les tests sont purs et instantanes : aucun thread, aucune attente, aucun
//! `sleep`, et **aucun acces au systeme de fichiers**. La source elle-meme
//! n'en fait aucun non plus, ce qui permet de la porter entierement hors ligne.

use serde::{Deserialize, Serialize};

/// Liste des motifs a ignorer par le watcher de systeme de fichiers.
///
/// Cote TypeScript : `Schema.String.pipe(Schema.Array)`, c'est-a-dire
/// `readonly string[]`. Un `Vec` et non un `BTreeSet` parce que la source
///preserve l'ordre, et que cet ordre est celui dans lequel le consumer
/// assemble la liste finale des motifs.
///
/// Ce type ne decrit aucun mecanisme : il ne sait pas comparer un motif a un
/// chemin, il ne sait pas lire un repertoire. C'est une liste de chaines, rien
/// de plus.
pub type Ignore = Vec<String>;

/// Configuration du watcher de systeme de fichiers.
///
/// En TS : `export class Info extends Schema.Class<Info>("ConfigV2.Watcher")`.
///
/// Le struct ne contient qu'un champ, et c'est tout ce que le module expose
/// comme contenu. Il n'a aucune methode de surveillance, ce qui est le
/// comportement de la source et non une omission du portage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// Motifs a ignorer, ou `None` si la cle est absente de la configuration.
    ///
    /// `Schema.optional` autorise l'absence de la cle et donne le type
    /// `Array<string> | undefined`. Une liste vide reste une liste vide
    /// presente : elle n'est pas la meme chose qu'une absence, et le JSON les
    /// distingue (`{"ignore": []}` contre `{}`).
    ///
    /// Cote decodage, `null` se lit comme une absence : c'est le comportement
    /// de `Option` sous Serde, et il n'a rien a voir avec une hypothese sur
    /// Effect. La parite avec `Schema.optional` sans `exact: true`, qui
    /// accepterait `null` comme `undefined`, **n'est pas verifiee** ; voir la
    /// section « Le point NON verifie » du module.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Ignore>,
}

impl Info {
    /// Configuration sans motif d'ignore : la forme la plus courte.
    ///
    /// Produit `{}` au JSON, donc exactement ce que produit un objet de
    /// configuration sans bloc `watcher`.
    pub fn new() -> Self {
        Self { ignore: None }
    }

    /// Configuration avec une liste de motifs.
    ///
    /// Prend le `Vec` tel quel, ordre compris. Une liste vide donne une cle
    /// `ignore` presente et vide, ce qui n'est pas la meme chose que `new()`.
    pub fn with_ignore(ignore: Ignore) -> Self {
        Self { ignore: Some(ignore) }
    }

    /// Les motifs d'ignore, ou une liste vide si la cle est absente.
    ///
    /// Equivalent direct du `?? []` de
    /// `packages/core/src/filesystem/watcher.ts` ligne 108 :
    ///
    /// ```ts
    /// .flatMap((item) => item.info.watcher?.ignore ?? [])
    /// ```
    ///
    /// Comme lui, ce test porte sur la **nullite** et non sur la veracite :
    /// `None` devient une liste vide, mais une liste vide reste une liste vide.
    /// C'est le seul endroit du portage ou cette distinction est utile, et il
    /// n'y a rien d'autre a faire de ces motifs ici.
    pub fn ignore_or_empty(&self) -> Ignore {
        self.ignore.clone().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ------------------------------------------------------------- liste vide

    #[test]
    fn une_liste_ignore_vide_donne_une_liste_vide() {
        let info: Info = serde_json::from_value(json!({ "ignore": [] })).unwrap();

        assert_eq!(info, Info::with_ignore(Vec::new()));
        assert_eq!(info.ignore_or_empty(), Vec::<String>::new());
        // La cle reste presente : une liste vide n'est pas une absence.
        assert_eq!(serde_json::to_value(&info).unwrap(), json!({ "ignore": [] }));
    }

    #[test]
    fn une_configuration_sans_motif_donne_un_objet_vide() {
        let info = Info::new();

        assert!(info.ignore.is_none());
        // `skip_serializing_if` doit supprimer la cle, pas ecrire `null`.
        assert_eq!(serde_json::to_value(&info).unwrap(), json!({}));
        assert!(serde_json::to_value(&info).unwrap().get("ignore").is_none());
    }

    // ------------------------------------------------------------- un element

    #[test]
    fn un_seul_motif_d_ignore_est_conserve() {
        let info: Info = serde_json::from_value(json!({ "ignore": [".git"] })).unwrap();

        assert_eq!(info.ignore, Some(vec![".git".to_string()]));
        assert_eq!(info.ignore_or_empty(), vec![".git".to_string()]);
        assert_eq!(serde_json::to_value(&info).unwrap(), json!({ "ignore": [".git"] }));
    }

    // ------------------------------------------------------------------ ordre

    #[test]
    fn l_ordre_des_motifs_est_preserve_dans_les_deux_sens() {
        // Le consumer concatene ces motifs dans l'ordre a la liste finale passee
        // a `subscribe`. Trier les motifs changerait donc le comportement, et
        // un `BTreeSet` serait une faute de portage.
        let json = json!({ "ignore": ["z", "a", "m", "b"] });

        let info: Info = serde_json::from_value(json.clone()).unwrap();

        assert_eq!(info.ignore_or_empty(), vec!["z", "a", "m", "b"]);
        assert_eq!(serde_json::to_value(&info).unwrap(), json);
    }

    #[test]
    fn une_liste_inversee_ne_se_retourne_pas() {
        let info = Info::with_ignore(vec!["b".to_string(), "a".to_string()]);

        assert_eq!(info.ignore_or_empty(), vec!["b", "a"]);
    }

    // ------------------------------------------------------------- valeurs par defaut

    #[test]
    fn la_valeur_par_defaut_ne_declenche_aucune_surveillance() {
        // Valeur par defaut : aucun motif, donc rien de plus a surveiller. Le
        // struct ne peut rien faire d'autre, et c'est le but.
        let info = Info::default();

        assert_eq!(info, Info::new());
        assert!(info.ignore_or_empty().is_empty());
    }

    #[test]
    fn une_absence_de_la_cle_donne_une_liste_vide_a_la_lecture() {
        // `default` sur le champ : un objet sans la cle doit se lire, et le
        // `?? []` du consumer doit alors produire une liste vide.
        let info: Info = serde_json::from_value(json!({})).unwrap();

        assert!(info.ignore.is_none());
        assert!(info.ignore_or_empty().is_empty());
    }

    #[test]
    fn un_null_se_decode_en_absence_cote_serde_et_la_parite_effect_reste_non_verifiee() {
        // Ce que ce test prouve, c'est le comportement de Serde : `null` se
        // decode en `None` pour un `Option`, sans aucune option sur le champ.
        // Ce qu'il ne prouve PAS, c'est la parite avec `Schema.optional` sans
        // `exact: true`, qui accepterait `null` comme `undefined` : cette
        // hypothese n'a pas pu etre verifiee dans le depot (node_modules
        // absent, aucune autre occurrence de `Schema.optional` avec un cas
        // `null` a cote). Si Effect refuse `null` ici, `{"ignore": null}` est
        // une erreur cote TS et une absence ici : c'est le seul ecart possible
        // de ce fichier, et il est assume.
        let info: Info = serde_json::from_value(json!({ "ignore": null })).unwrap();

        assert_eq!(info, Info::new());
        assert!(info.ignore.is_none());
        assert!(info.ignore_or_empty().is_empty());
        // Une absence se reecrit sans la cle, jamais avec `null`.
        assert_eq!(serde_json::to_value(&info).unwrap(), json!({}));
    }

    #[test]
    fn null_comme_champ_entier_est_une_absence_mais_null_dans_la_liste_est_refuse() {
        // L'asymetrie que le derive introduit, et le lieu exact ou une
        // correction devra etre appliquee : `null` a la place du champ
        // donne `None`, `null` dans le tableau ne donne pas `Vec<String>`.
        let absence: Info = serde_json::from_value(json!({ "ignore": null })).unwrap();
        assert_eq!(absence, Info::new());

        let liste: Result<Info, _> = serde_json::from_value(json!({ "ignore": [null] }));
        assert!(liste.is_err(), "un null dans la liste ne peut pas devenir \"\"");
    }

    #[test]
    fn une_liste_vide_et_une_absence_ne_se_confondent_pas() {
        // Deux etats distincts, et le JSON les distingue. Les confondre
        // reviendrait a ecrire un `Vec` simple et a perdre l'information que
        // l'utilisateur a ecrit `ignore: []` explicitement.
        let vide = Info::with_ignore(Vec::new());
        let absent = Info::new();

        assert_ne!(vide, absent);
        assert_eq!(serde_json::to_value(&vide).unwrap(), json!({ "ignore": [] }));
        assert_eq!(serde_json::to_value(&absent).unwrap(), json!({}));
    }

    // -------------------------------------------------- noms de champs et entrees invalides

    #[test]
    fn le_nom_du_champ_json_est_ignore() {
        let info = Info::with_ignore(vec!["node_modules".to_string()]);

        let json = serde_json::to_value(&info).unwrap();

        assert_eq!(json["ignore"], json!(["node_modules"]));
        // La casse ne doit pas changer, et aucun pluriel ne doit apparaitre.
        assert!(json.get("Ignore").is_none());
        assert!(json.get("ignoreList").is_none());
    }

    #[test]
    fn un_motif_vide_reste_un_motif_valide() {
        // Piege `?` contre `??` : en JavaScript `""` est falsy. Le consumer
        // utilise `config.includes(".git")` et le `?? []`, jamais la veracite
        // d'un element, donc un motif vide doit traverser le JSON intact.
        let info: Info = serde_json::from_value(json!({ "ignore": [""] })).unwrap();

        assert_eq!(info.ignore_or_empty(), vec![""]);
        assert_eq!(serde_json::to_value(&info).unwrap(), json!({ "ignore": [""] }));
    }

    #[test]
    fn une_valeur_qui_n_est_pas_une_liste_de_chaines_est_refusee() {
        // `Schema.String.pipe(Schema.Array, Schema.optional)` refuse tout ce qui
        // n'est pas un tableau de chaines, et ne convertit rien. `null` est
        // traite a part, dans le test dedie ci-dessus.
        for invalide in [
            json!({ "ignore": ".git" }),
            json!({ "ignore": 1 }),
            json!({ "ignore": { "a": 1 } }),
            json!({ "ignore": [1] }),
            json!({ "ignore": [null] }),
            json!({ "ignore": [["a"]] }),
            json!({ "ignore": true }),
        ] {
            assert!(
                serde_json::from_value::<Info>(invalide.clone()).is_err(),
                "{invalide} ne devrait pas etre accepte"
            );
        }
    }

    #[test]
    fn un_champ_inconnu_est_ignore_comme_en_typescript() {
        // Le decodeur de Schema ne rejette pas les proprietes en trop. Serde
        // les ignore aussi, et c'est le comportement qu'on veut garder.
        let info: Info = serde_json::from_value(json!({ "ignore": [".git"], "inconnu": 42 })).unwrap();

        assert_eq!(info.ignore_or_empty(), vec![".git"]);
    }

    // ------------------------------------------------------------------ purete

    #[test]
    fn la_methode_rend_une_copie_et_ne_modifie_jamais_la_configuration() {
        // Le `?? []` de la source ne fait que lire. La copie est donc la seule
        // garantie qu'un appelant qui complete la liste finale ne touche pas
        // la configuration d'origine.
        let info = Info::with_ignore(vec![".git".to_string()]);

        let mut copie = info.ignore_or_empty();
        copie.push("ajout-de-l-appelant".to_string());

        assert_eq!(copie.len(), 2);
        assert_eq!(info.ignore_or_empty(), vec![".git"]);
        assert_eq!(serde_json::to_value(&info).unwrap(), json!({ "ignore": [".git"] }));
    }

    #[test]
    fn deux_appels_identiques_rendent_le_meme_resultat() {
        // Aucun thread, aucune attente, aucun acces disque : la fonction est
        // pure, donc la repetition ne peut rien changer.
        let info = Info::with_ignore(vec!["a".to_string(), "b".to_string()]);

        assert_eq!(info.ignore_or_empty(), info.ignore_or_empty());
        assert_eq!(Info::new().ignore_or_empty(), Info::new().ignore_or_empty());
    }
}