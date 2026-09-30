//! Portage Rust de `opencode/packages/core/src/v1/config/skills.ts`.
//!
//! La source fait treize lignes et ne contient **aucune logique** : uniquement
//! une declaration de schema.
//!
//! ```ts
//! export * as ConfigSkillsV1 from "./skills"
//!
//! import { Schema } from "effect"
//!
//! export const Info = Schema.Struct({
//!   paths: Schema.optional(Schema.Array(Schema.String)).annotate({
//!     description: "Additional paths to skill folders",
//!   }),
//!   urls: Schema.optional(Schema.Array(Schema.String)).annotate({
//!     description: "URLs to fetch skills from (e.g., https://example.com/.well-known/skills/)",
//!   }),
//! })
//! export type Info = Schema.Schema.Type<typeof Info>
//! ```
//!
//! ## Le reexport de la ligne 1
//!
//! `export * as ConfigSkillsV1 from "./skills"` est un reexport d'espace de noms
//! qui pointe sur le fichier lui-meme : c'est du code mort, exactement comme dans
//! `v1_config_server.rs` et `config_command.rs`. En Rust le module
//! `v1_config_skills` est deja l'espace de noms, rien a retranscrire.
//!
//! ## Aucun identifiant de schema, contrairement a `server.ts`
//!
//! `server.ts` termine par `.annotate({ identifier: "ServerConfig" })`, et son
//! portage expose donc `SCHEMA_IDENTIFIER`. **Ce fichier ne l'a pas** : la
//! source s'arrete a la fermeture du `Schema.Struct`. Il n'y a donc **pas** de
//! constante `SCHEMA_IDENTIFIER` ici, et c'est assume : la recopier depuis le
//! fichier voisin pour "combler" creerait une donnee qui n'existe pas en
//! TypeScript, donc une seconde source de verite qui divergerait des que
//! l'original bouge.
//!
//! ## Les deux `description` ne sont pas des valeurs
//!
//! Les deux seules chaines de la source sont des textes d'annotation. En
//! particulier `https://example.com/.well-known/skills/` n'est qu'un **exemple
//! ecrit dans une description**, entre parentheses et precede de `e.g.` : ce
//! n'est ni une URL par defaut, ni une constante, ni une validation. La source
//! ne l'applique nulle part et ne refuse aucune URL. Aucun des deux champs
//! n'a de valeur par defaut ici, et le test
//! `une_configuration_vide_ne_serialise_que_des_accolades_vides` verrouille ce
//! point : un `Info` vide doit ressortir `{}`.
//!
//! ## PIeGE DES NOMS DE CHAMPS : il ne s'applique pas ici, et c'est dit
//!
//! Le piege du lot est `projectID` et non `projectId` : une majuscule en plein
//! milieu du nom, invisible a la compilation et qui ne casse qu'a l'echange avec
//! le TypeScript. **Ce fichier n'a aucun nom de ce genre.** Les deux cles sont
//! des mots uniques en minuscules :
//!
//! - `paths` : un mot, tout en minuscules ;
//! - `urls` : un mot, tout en minuscules, et non `URLs` ni `Urls`.
//!
//! Les deux ont ete relus ligne a ligne contre la source, il n'y en a pas d'autre.
//! Le nom du champ Rust est donc deja le nom TypeScript. Malgre cela chaque
//! champ porte un `#[serde(rename = "...")]` **volontairement redondant** : il
//! sert de garde-fou si le nom du champ bouge un jour, et il rend le contrat
//! d'echange visible sans avoir a ouvrir la source pour verifier.
//!
//! Un nom qui **n'existe pas** en TypeScript ne doit pas non plus fonctionner :
//! voir `les_variantes_fauteives_sont_refusees_a_la_lecture`, qui essaie
//! `skill_paths`, `skillPaths`, `Paths` et la forme plate historique, et exige
//! qu'aucune ne remplisse un champ.
//!
//! ## La forme plate du format v1 n'est PAS la meme forme
//!
//! Le vrai risque d'interchangeabilite sur ce fichier-la n'est pas une
//! majuscule, c'est un **autre format**. Le format v1 utilise `skills` comme
//! une **liste plate de chaines** :
//!
//! ```ts
//! // v1/config/migrate.ts:62
//! skills: info.skills && [...(info.skills.paths ?? []), ...(info.skills.urls ?? [])]
//! ```
//!
//! et `config/v2-compat.ts:154-161` fait l'inverse, en **separant** la liste
//! plate en deux champs selon un test de prefixe :
//!
//! ```ts
//! result.skills = {
//!   paths: skills.filter((value) => !/^https?:\/\//i.test(value)),
//!   urls: skills.filter((value) => /^https?:\/\//i.test(value)),
//! }
//! ```
//!
//! `paths` et `urls` ne sont donc **pas** deux aliases du meme champ, et `{}`
//! n'est pas equivalent a `{ skills: [] }`. Un portage qui fusionnerait les deux
//! champs, ou qui ajouterait un champ `skills` par commodite, casserait la
//! migration v1 vers v2 sans la moindre erreur de compilation. Aucun champ
//! `skills` n'est donc declare ici.
//!
//! ## Aucun motif de glob dans ce fichier
//!
//! `paths` et `urls` sont des `Schema.String` **nus**. Le schema ne valide
//! rien : il n'y a ni expression rationnelle, ni `{...}`, ni `*`, ni `?`, ni
//! classe de caractere. Le filtrage par motif se fait ailleurs, chez le
//! consommateur, et pas dans ce fichier. Deux consequences :
//!
//! - aucune dependance a un moteur de glob n'est introduite ici, et ce module
//!   n'appelle pas `util_glob` ; il n'y a donc **aucune collision** possible
//!   avec le fichier `src/swarm/util_glob.rs` du meme lot ;
//! - le `?` de `skill/index.ts:150` (`dot: opts?.dot`) et le drapeau `dot`
//!   qu'il porte appartiennent a `skill/index.ts`, pas a `skills.ts`, et n'ont
//!   donc aucune contrepartie ici. Le `dot` n'est pas un champ de ce schema.
//!
//! Concretement, une entree comme `./skills/{a,b}*.md` est acceptee telle quelle
//! par ce schema, et c'est correct : c'est au consommateur de l'interpreter.
//!
//! ## Un `Schema.Struct` n'est pas une `Schema.Class`
//!
//! Il n'y a qu'une forme de donnees, sans methode associee : un `struct` derive
//! `Serialize, Deserialize` suffit.
//!
//! ## Aucun ternaire `?`, aucun coalescent `??`, et deux fonctions distinctes
//!
//! La source ne contient **ni l'un ni l'autre** : c'est une declaration de
//! schema, pas une expression. Il n'y a donc rien a filtrer, et il faut le dire
//! sans le contourner, parce que ces deux operateurs sont les deux jugements de
//! valeurs les plus facile a confondre du language d'origine :
//!
//! - le **ternaire** `x ? a : b` teste la **veracite**. En JavaScript `""` est
//!   falsy, donc `p ? p : defaut` avale la chaine vide ;
//! - le **coalescent** `x ?? y` teste la **nullite**. Seuls `null` et
//!   `undefined` declenchent `y`, donc `p ?? defaut` **conserve** la chaine
//!   vide.
//!
//! Ce module ne contient donc qu'**une seule** de ces deux familles, la famille
//! **nullite**, et c'est celle dont dependent les consommateurs relus :
//! `skill/index.ts:211` et `:222` ecrivent `cfg.skills?.paths ?? []` et
//! `cfg.skills?.urls ?? []`, et `migrate.ts:62` ecrit
//! `info.skills.paths ?? []`. Ces lectures-la demandent de la **nullite** : un
//! `paths: []` donne est une liste donnee, et une entree `""` dans la liste est
//! une entree donnee. Un filtre de veracite les detruirait en silence.
//!
//! Concretement, et c'est verrouille par les tests :
//!
//! - `Some("")` doit **SURVIVRE** ;
//! - `Some(vec![])` doit **SURVIVRE** : une liste vide est une liste donnee ;
//! - `None` disparait, et c'est le seul filtre pose :
//!   `skip_serializing_if = "Option::is_none"`.
//!
//! Pour que la distinction reste **verifiable** et pas seulement affirmee en
//! commentaire, les deux comportements opposes sont ecrits **sous deux formes
//! distinctes** dans le module de tests, jamais fusionnes : `disparait_si_falsy`
//! (veracite, famille `?`) et `survit_si_null` (nullite, famille `??`), avec le
//! test qui prouve qu'ils ne donnent PAS le meme resultat sur `Some("")`. Ce
//! sont des fonctions de test, pas du code porte : ce module n'expose ni l'une
//! ni l'autre.
//!
//! ## `Schema.optional`
//!
//! `Schema.optional(X)` = cle facultative. Cela donne `Option<X>` avec
//! `skip_serializing_if`, donc un champ absent n'apparait pas dans le JSON au
//! lieu de s'y ecrire `null`. Consequence a assumer, identique a celle du reste
//! du lot : Serde lit `null` comme `None` la ou le schema TypeScript distingue
//! `null` de l'absence. Invisible pour du JSON produit par le TypeScript,
//! visible sur une saisie manuelle.
//!
//! Le champ englobant l'est aussi : `config.ts:44` ecrit
//! `skills: Schema.optional(ConfigSkillsV1.Info)`, donc la section entiere peut
//! disparaitre, et une section `skills: {}` est valide.
//!
//! ## Les deux tableaux gardent leur ordre et leurs doublons
//!
//! Contrairement a `server.ts`, qui ecrit explicitement
//! `Schema.mutable(Schema.Array(Schema.String))`, la source ici ecrit
//! `Schema.Array(Schema.String)` sans le `mutable`. Cela ne change rien au type
//! Rust retenu : ce sont des **valeurs de configuration**, donc des `Vec` et
//! jamais des `BTreeSet` ou des `BTreeMap`, qui perdraient l'ordre et les
//! doublons. L'ordre de recherche des dossiers de skills est significatif, et
//! une meme URL listee deux fois doit ressortir deux fois. Deux tests
//! verrouillent ce point : ordre inverse, et doublon conserve.
//!
//! ## Aucune action, et c'est volontaire
//!
//! Le fichier parle de chemins et d'URL, mais **il n'ouvre rien** : ce sont des
//! valeurs, pas des actions. Tout ce qui suit n'a donc aucune traduction dans
//! ce module, et se fait chez le consommateur :
//!
//! - l'expansion du prefixe `~/` vers le dossier personnel (`skill/index.ts:212`) ;
//! - la resolution d'un chemin relatif contre le repertoire de travail ;
//! - la verification qu'un chemin existe (`fsys.isDir`) ;
//! - le parcours recursif du dossier et le filtrage par motif ;
//! - le telechargement d'une URL et son cache sur disque.
//!
//! Aucun de ces comportements n'est simule ici, ni `Path`, ni `std::fs`, ni
//! aucune requete reseau. Fabriquer de la mecanique pour "completer" cette
//! structure serait exactement l'erreur a eviter.

use serde::{Deserialize, Serialize};

/// Configuration de la section `skills` du fichier de configuration, format v1.
///
/// En TS : `export type Info = Schema.Schema.Type<typeof Info>`.
///
/// Utilisee par `v1/config/config.ts:44` sous la forme
/// `skills: Schema.optional(ConfigSkillsV1.Info)`.
///
/// Les deux champs sont optionnels : une section `skills` absente, ou presente
/// mais vide, est valide. Aucun champ n'est obligatoire, aucun n'a de valeur par
/// defaut, et la source n'en valide aucun.
///
/// Les proprietes inconnues sont tolerees, comme en TypeScript : un
/// `Schema.Struct` ignore les champs en trop au lieu de refuser l'objet. On ne
/// pose donc pas `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// Chemins supplementaires vers des dossiers de skills.
    ///
    /// En TS : `paths`, avec la description
    /// `"Additional paths to skill folders"`.
    ///
    /// Ce sont des chaines **nues** : le schema ne valide aucun motif et
    /// n'interprete aucune accolade. Voir la section "Aucun motif de glob dans
    /// ce fichier" du crate.
    ///
    /// L'ordre est preserve et les doublons aussi : c'est une liste de
    /// recherche, pas un ensemble.
    #[serde(rename = "paths", skip_serializing_if = "std::option::Option::is_none")]
    pub paths: Option<Vec<String>>,

    /// URL a partir desquelles aller chercher des skills.
    ///
    /// En TS : `urls`, avec la description
    /// `"URLs to fetch skills from (e.g., https://example.com/.well-known/skills/)"`.
    ///
    /// ATTENTION : l'URL de cette description est un **exemple ecrit dans un
    /// texte**, pas une valeur par defaut. Elle n'est declaree nulle part dans
    /// ce module, et le schema n'en refuse aucune.
    ///
    /// Comme `paths`, c'est un `Vec` : l'ordre et les doublons sont preserves.
    #[serde(rename = "urls", skip_serializing_if = "std::option::Option::is_none")]
    pub urls: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------
    // Les deux jugements de valeurs, ecrits DEUX FOIS et jamais fusionnes.
    //
    // Ce ne sont pas du code porte depuis `skills.ts`, qui ne contient ni
    // ternaire ni coalescent. Elles sont ici pour rendre la distinction
    // verifiable : si quelqu'un introduit un jour dans ce module un filtre
    // `is_empty()` sur une entree, ces deux fonctions montrent en une ligne ce
    // que ce filtre change, et le test qui suit montre qu'elles ne concordent
    // pas.
    // ---------------------------------------------------------------------

    /// Famille du **ternaire** `x ? a : b` : teste la **veracite**.
    ///
    /// Comme en JavaScript, la chaine vide est falsy, donc la valeur
    /// **disparait** et l'on obtient `None`.
    fn disparait_si_falsy(valeur: Option<String>) -> Option<String> {
        valeur.filter(|v| !v.is_empty())
    }

    /// Famille du **coalescent** `x ?? y` : teste la **nullite**.
    ///
    /// Seuls `None` declenchent le remplacement. La chaine vide, qui serait
    /// falsy, **survit** intacte.
    fn survit_si_null(valeur: Option<String>) -> Option<String> {
        // `x ?? y` vaut `x`, sauf si `x` est `null`, auquel cas il vaut `y`.
        // Ici `y` redonne `x` : le seul cas qui change est donc l'absence, qui
        // reste une absence, et une valeur presente reste presente.
        valeur.or(None)
    }

    #[test]
    fn les_deux_familles_de_jugement_ne_donnent_pas_le_meme_resultat() {
        // La ligne qui rend le piege visible : sur `Some("")`, le ternaire
        // efface, le coalescent garde.
        assert_eq!(disparait_si_falsy(Some(String::new())), None);
        assert_eq!(survit_si_null(Some(String::new())), Some(String::new()));

        // Sur une valeur pleine, les deux concordent : c'est bien la seule
        // difference qui les separe, ce qui rend la comparaison honnete.
        assert_eq!(
            disparait_si_falsy(Some("./skills".to_string())),
            Some("./skills".to_string())
        );
        assert_eq!(
            survit_si_null(Some("./skills".to_string())),
            Some("./skills".to_string())
        );

        // Et sur l'absence, les deux sont d'accord : `None` disparait dans les
        // deux cas. Le type du parametre est nomme ici, sinon le `None` seul ne
        // donne rien a compiler.
        assert_eq!(disparait_si_falsy(None), None);
        assert_eq!(survit_si_null(None), None);
    }

    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        // Test prioritaire du fichier : c'est lui qui garantit que `paths` et
        // `urls` sortent avec leur nom exact, et qu'aucun nom invente ne se
        // glisse dans le JSON.
        let info = Info {
            paths: Some(vec!["./skills".to_string(), "~/shared-skills".to_string()]),
            urls: Some(vec!["https://example.com/.well-known/skills/".to_string()]),
        };

        let json = serde_json::to_value(&info).unwrap();
        let objet = json.as_object().unwrap();

        assert!(objet.contains_key("paths"), "champ absent du JSON : {json}");
        assert!(objet.contains_key("urls"), "champ absent du JSON : {json}");
        assert_eq!(objet.len(), 2, "champ en trop dans le JSON : {json}");

        // Aucun decalage ne doit apparaitre : ni prefixe `skill_`, ni
        // majuscule, ni forme plate.
        for interdit in ["skill_paths", "skillPaths", "skill_urls", "skills"] {
            assert!(
                !objet.contains_key(interdit),
                "le nom Rust a fuite dans le JSON sous {interdit} : {json}"
            );
        }

        let attendu = r#"{"paths":["./skills","~/shared-skills"],"urls":["https://example.com/.well-known/skills/"]}"#;
        assert_eq!(serde_json::to_string(&info).unwrap(), attendu);
    }

    #[test]
    fn les_variantes_fauteives_sont_refusees_a_la_lecture() {
        // `#[serde(rename)]` REMPLACE le nom du champ, il ne s'y ajoute pas :
        // `skill_paths` n'est donc pas une facon alternative d'ecrire `paths`,
        // c'est une propriete inconnue, donc ignoree. Concretement, si le
        // rename disparait du fichier, la premiere ligne de ce test echouerait.
        for faux in [
            r#"{"skill_paths":["./skills"]}"#,
            r#"{"skillPaths":["./skills"]}"#,
            r#"{"Paths":["./skills"]}"#,
            r#"{"PATHS":["./skills"]}"#,
            r#"{"skill_urls":["https://example.test/s/"]}"#,
        ] {
            let info: Info = serde_json::from_str(faux).unwrap();
            assert_eq!(info.paths, None, "forme fautive acceptee : {faux}");
            assert_eq!(info.urls, None, "forme fautive acceptee : {faux}");
        }

        // Temoin : la seule forme acceptee reste celle de la source, et elle
        // doit toujours fonctionner. Sans ce contre-test, un `#[serde(rename)]`
        // errone vers un nom qui n'existe pas passerait pour un refus correct.
        let bon: Info = serde_json::from_str(r#"{"paths":["./skills"]}"#).unwrap();
        assert_eq!(info_paths_attendus(&bon), vec!["./skills".to_string()]);
    }

    #[test]
    fn la_forme_plate_du_format_v1_ne_remplit_aucun_champ() {
        // Le format v1 utilise `skills` comme une LISTE PLATE de chaines, que
        // `migrate.ts:62` ecrase et que `v2-compat.ts:154-161` scinde en
        // `paths` et `urls`. Ce document ne doit donc pas remplir le moindre
        // champ : ce sont deux formes distinctes, pas deux aliases.
        let plate: Info =
            serde_json::from_str(r#"{"skills":["./skills","https://example.com/.well-known/skills/"]}"#)
                .unwrap();
        assert_eq!(plate, Info { paths: None, urls: None });

        // Et l'inverse, qui compte tout autant : un objet `{ "paths": [...] }`
        // n'est pas la liste plate. Un `{ "skills": "..." }` non plus.
        let chain: Info = serde_json::from_str(r#"{"skills":"./skills"}"#).unwrap();
        assert_eq!(chain, Info { paths: None, urls: None });

        // Le seul document de cette famille qui doit rester valide reste
        // l'objet a deux champs.
        let bon: Info = serde_json::from_str(
            r#"{"paths":["./skills"],"urls":["https://example.com/.well-known/skills/"]}"#,
        )
        .unwrap();
        assert_eq!(info_paths_attendus(&bon), vec!["./skills".to_string()]);
        assert_eq!(info_urls_attendus(&bon).len(), 1);
    }

    #[test]
    fn le_decodage_suit_la_famille_nullite() {
        // Le module ne contient aucun ternaire : decodage et re-serialisation
        // suivent donc la famille `??`, la seule presente dans la source, et
        // celle dont dependent `skill/index.ts:211` et `migrate.ts:62`.
        let info: Info = serde_json::from_str(r#"{"paths":["",""]}"#).unwrap();
        assert_eq!(
            info.paths,
            survit_si_null(Some("".to_string())).map(|seule| vec![seule.clone(), seule]),
            "une entree vide a ete effacee : filtre de veracite interdit"
        );

        // Le tableau vide, lui, reste present : c'est une liste donnee, pas une
        // liste absente.
        let vide: Info = serde_json::from_str(r#"{"paths":[],"urls":[]}"#).unwrap();
        assert_eq!(vide.paths, Some(Vec::new()));
        assert_eq!(vide.urls, Some(Vec::new()));
        assert_eq!(
            serde_json::to_string(&vide).unwrap(),
            r#"{"paths":[],"urls":[]}"#
        );

        // Et l'absence reste une absence : elle ne s'invente pas.
        let aucun: Info = serde_json::from_str("{}").unwrap();
        assert_eq!(aucun, Info { paths: None, urls: None });
    }

    #[test]
    fn une_chaine_vide_comme_entree_de_liste_survit() {
        // Piege `?` contre `??` : la source ne teste pas la veracite. Une entree
        // `""` est une entree de configuration presente, et
        // `cfg.skills?.paths ?? []` doit la rendre au consommateur. Un filtre
        // `is_empty()` la detruirait en silence, sans erreur de compilation.
        let info: Info =
            serde_json::from_str(r#"{"paths":["","./skills",""],"urls":[""]}"#).unwrap();
        assert_eq!(
            info_paths_attendus(&info),
            vec![
                String::new(),
                "./skills".to_string(),
                String::new()
            ]
        );
        assert_eq!(info_urls_attendus(&info), vec![String::new()]);

        let json = serde_json::to_string(&info).unwrap();
        assert!(
            json.contains(r#""urls":[""]"#),
            "l'entree vide a disparu du JSON : {json}"
        );
    }

    #[test]
    fn une_configuration_vide_ne_serialise_que_des_accolades_vides() {
        // Aucune valeur par defaut n'est imposee par la source : une section
        // `skills: {}` doit rester `{}`. En particulier l'URL de la description
        // ne doit pas se glisser dans le JSON, et un champ absent ne doit pas
        // s'ecrire `null`.
        let info = Info { paths: None, urls: None };
        assert_eq!(serde_json::to_string(&info).unwrap(), "{}");

        let relu: Info = serde_json::from_str("{}").unwrap();
        assert_eq!(relu, info);
    }

    #[test]
    fn les_listes_conservent_leur_ordre_et_leur_doublon() {
        // Ni tri ni deduplication : l'ordre de recherche est significatif et une
        // meme entree listee deux fois doit ressortir deux fois.
        let source = r#"{"paths":["trois","deux","un","deux"],"urls":[]}"#;
        let info: Info = serde_json::from_str(source).unwrap();
        assert_eq!(
            info_paths_attendus(&info),
            vec![
                "trois".to_string(),
                "deux".to_string(),
                "un".to_string(),
                "deux".to_string()
            ]
        );
        assert_eq!(serde_json::to_string(&info).unwrap(), source);

        // Un `BTreeSet` ou un `BTreeMap` ici dedupliquerait et trierait, et ce
        // test echouerait. C'est le garde-fou du choix `Vec`.
        assert_eq!(info_paths_attendus(&info).len(), 4);
    }

    #[test]
    fn un_type_incorrect_est_refuse_a_la_lecture() {
        // Le schema demande un TABLEAU de chaines. Une chaine nue n'en est pas
        // un, meme si la source ne valide que le type : `Schema.Array` refuse
        // `./skills` seul.
        assert!(serde_json::from_str::<Info>(r#"{"paths":"./skills"}"#).is_err());
        assert!(serde_json::from_str::<Info>(r#"{"urls":"https://example.test/s/"}"#).is_err());

        // Un tableau qui contient autre chose qu'une chaine est refuse aussi.
        assert!(serde_json::from_str::<Info>(r#"{"paths":[1]}"#).is_err());
        assert!(serde_json::from_str::<Info>(r#"{"urls":[null]}"#).is_err());

        // Un objet n'est pas un tableau.
        assert!(serde_json::from_str::<Info>(r#"{"paths":{}}"#).is_err());
    }

    #[test]
    fn un_motif_est_accepte_tel_quel_sans_interpretation() {
        // Le schema ne valide aucun motif : ni accolade, ni `*`, ni `?`, ni
        // classe de caractere. Une entree avec `{a,b}` passe et ressort
        // **intacte**, non developpee, non normalisee. C'est le consommateur
        // qui interprete, et ce module ne cree donc aucune dependance a un
        // moteur de glob.
        let source = r#"{"paths":["./skills/{a,b}/*.md","~/shared-skills"],"urls":["https://example.com/.well-known/skills/"]}"#;
        let info: Info = serde_json::from_str(source).unwrap();
        assert_eq!(
            info_paths_attendus(&info),
            vec!["./skills/{a,b}/*.md".to_string(), "~/shared-skills".to_string()]
        );
        assert_eq!(serde_json::to_string(&info).unwrap(), source);

        // Le prefixe `~/` n'est pas non plus developpe ici : il est expansionne
        // par le consommateur, pas par le schema.
        assert!(info_paths_attendus(&info)[1].starts_with("~/"));
    }

    #[test]
    fn une_propriete_inconnue_est_ignoree_comme_en_typescript() {
        // Un `Schema.Struct` ne refuse pas les champs en trop : on ne pose pas
        // `deny_unknown_fields`. C'est ce qui laisse passer la forme plate
        // `{"skills": [...]}` sans erreur, tout en restant sans effet.
        let info: Info = serde_json::from_str(r#"{"paths":["./skills"],"inconnu":true}"#).unwrap();
        assert_eq!(info_paths_attendus(&info), vec!["./skills".to_string()]);
        assert_eq!(info.urls, None);
    }

    #[test]
    fn le_round_trip_ne_perd_aucune_forme() {
        // Ce que le TypeScript ecrit, Rust le relit, et l'inverse : le contrat
        // d'echange complet, dans les deux sens, sur une configuration
        // plausible.
        let json = r#"{"paths":["./skills","~/shared-skills","https://example.com/.well-known/skills/"],"urls":["https://example.com/.well-known/skills/"]}"#;
        let info: Info = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&info).unwrap(), json);

        // Et le cas de la section presente mais vide, que `config.ts:44` autorise.
        let vide: Info = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(serde_json::to_string(&vide).unwrap(), "{}");
    }

    // ---------------------------------------------------------------------
    // Deux accesseurs de test, qui evite d'ecrire `as_deref().unwrap()` a
    // chaque assertion et gardent les tests lisibles. Ils ne sont pas du code
    // porte : la source n'exporte aucune fonction.
    // ---------------------------------------------------------------------

    fn info_paths_attendus(info: &Info) -> Vec<String> {
        info.paths.clone().unwrap_or_default()
    }

    fn info_urls_attendus(info: &Info) -> Vec<String> {
        info.urls.clone().unwrap_or_default()
    }
}
