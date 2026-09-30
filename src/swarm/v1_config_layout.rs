//! Portage de `packages/core/src/v1/config/layout.ts`.
//!
//! La source fait six lignes et ne contient **aucune logique** : deux litteraux
//! et une annotation.
//!
//! ```ts
//! export * as ConfigLayoutV1 from "./layout"
//!
//! import { Schema } from "effect"
//!
//! export const Layout = Schema.Literals(["auto", "stretch"]).annotate({ identifier: "LayoutConfig" })
//! export type Layout = Schema.Schema.Type<typeof Layout>
//! ```
//!
//! ## Le reexport de la ligne 1
//!
//! `export * as ConfigLayoutV1 from "./layout"` est un reexport d'espace de noms
//! qui pointe sur le fichier lui-meme, donc du code mort : en Rust le module
//! `v1_config_layout` est deja cet espace de noms. Rien a retranscrire, comme
//! dans `v1_config_server.rs` et `v1_config_error.rs`.
//!
//! ## Le type : deux chaines, pas une struct
//!
//! `Schema.Literals(["auto", "stretch"])` est l'union de deux litteraux de
//! chaine, donc `"auto" | "stretch"`. La traduction directe est un `enum` sans
//! donnee, et **pas** une `struct` : il n'y a aucun champ, donc aucun nom de
//! champ a transcrire.
//!
//! C'est ce qui distingue ce fichier de `v1_config_server.rs`, ou chaque champ
//! portait un `#[serde(rename)]` defense. Ici, ce qui porte une casse sensible
//! n'est pas un nom de champ mais **la valeur litterale elle-meme**.
//!
//! ## Piege des majuscules : il ne se pose pas, et ce n'est pas un oubli
//!
//! Les noms de champs notorious de ce depot, `projectID` et `sessionID`, sont
//! des majuscules en plein milieu. Le present fichier **n en contient aucun** :
//! pas de `Schema.Struct`, pas de champ, pas de propriete. Il ne faut donc pas
//! inventer un `#[serde(rename)]` de champ qui n existe pas, ni ajouter un champ
//! `layoutName` pour "completer" la structure : la source n en a pas.
//!
//! Le meme piege, transpose au niveau du litteral, mord quand meme, et c est
//! la que il faut surveiller :
//!
//! - la source ecrit `"auto"` et `"stretch"`, en **minuscules** ;
//! - les variantes Rust s appellent `Auto` et `Stretch`, en PascalCase, comme
//!   l impose la convention du projet ;
//! - sans `#[serde(rename = "auto")]`, serde emettrait `"Auto"`, qui n existe
//!   pas dans l union TypeScript.
//!
//! C'est exactement la meme famille de faute que `projectID` versus
//! `project_id` : invisible depuis l interieur du code Rust, decisive a
//! l echange avec le TypeScript. Les deux `rename` sont donc ecrits **un par
//! un**, jamais via un `rename_all`, pour qu une relecture les verifie d un
//! coup d oeil et qu un changement de convention de casse ne les derive pas.
//!
//! Deux tests verrouillent le point dans les deux sens :
//! `les_noms_serialises_sont_ceux_du_typescript` verifie l ecriture, et
//! `les_formes_fautives_sont_refusees_a_la_lecture` verifie que `"Auto"`,
//! `"STRETCH"`, `"Auto "`, `" stretch"`, `"stretch "`, `"auto\n"` et les
//! autres formes fautives sont **refusees**, cote decodeur comme cote
//! `from_name`. Le refus a la lecture est le test le plus fort des deux : un
//! `rename` errone vers `"Auto"` se verrait a l ecriture, mais un `rename`
//! absent se verrait uniquement la, en lecture.
//!
//! ## La forme sur le fil est une chaine nue, pas un objet
//!
//! `config.ts:127` ecrit
//! `layout: Schema.optional(ConfigLayoutV1.Layout)`, donc sur le fil la valeur
//! de la cle `layout` est `"auto"`. Elle n est **jamais** `{"type":"auto"}` :
//! aucune representation adjacente ou interne ne vient s interposer, donc
//! ni `#[serde(tag = ...)]` ni newtype enveloppante ici. Une valeur-objet
//! serait rejetee a la lecture, et
//! `la_forme_sur_le_fil_est_une_chaine_nue_pas_un_objet` le fixe.
//!
//! Le `Option` n appartient pas a ce module : `Schema.optional` est pose dans
//! `config.ts`, donc le champ `layout` - **tout en minuscules**, sans
//! majuscule cachee - est declare dans le portage de `config.ts`, pas ici.
//! Ce fichier ne porte que le type de la valeur, jamais le conteneur.
//!
//! ## `?` contre `??` : les deux familles, puis une troisieme famille
//!
//! La source ne contient **ni ternaire ni coalescent** : c est une declaration
//! de schema, pas une expression. Les deux jugements de valeurs les plus facile
//! a confondre du langage d origine n ont donc rien a filtrer ici :
//!
//! - le **ternaire** `x ? a : b` teste la **veracite**. En JavaScript `""` est
//!   falsy, donc `cfg.layout ? cfg.layout : "stretch"` avale la chaine vide.
//! - le **coalescent** `x ?? y` teste la **nullite**. Seuls `null` et
//!   `undefined` declenchent `y`, donc `cfg.layout ?? "stretch"` **conserve**
//!   la chaine vide.
//!
//! Les deux sont ecrits sous deux formes distinctes dans le module de tests,
//! jamais fusionnes : `disparait_si_falsy` (veracite, famille `?`) et
//! `survit_si_null` (nullite, famille `??`), avec un test qui prouve qu elles
//! ne concordent pas sur `Some("")`.
//!
//! Ce qui rend la suite non triviale, c est que la chaine vide n est pas
//! hypothetique ici. Un fichier de configuration est saisi a la main, donc
//! `layout: ""` est une saisie reelle, et il y a **trois** conduites possibles
//! dont deux sont fausses :
//!
//! 1. le ternaire : `""` est falsy, la valeur **disparait** et le defaut
//!    s applique en silence. Faux : la saisie de l utilisateur disparait.
//! 2. le coalescent : `""` **survit** en tant que chaine vide. Faux aussi :
//!    une chaine vide n est ni `"auto"` ni `"stretch"`, la faire passer pour
//!    une valeur de configuration serait un mensonge.
//! 3. **ce que fait la source** : ni l une ni l autre. `""` n appartient pas a
//!    l union, donc le decodage **echoue**. Le filtre est porte par le **type**,
//!    pas par un test de valeur, et il est plus strict que les deux operateurs.
//!
//! Le module implemente donc la famille **nullite** comme reference de
//! comparaison, et aucune des deux operations. Concretement :
//!
//! - `""` est refuse a la lecture, et n est **ni remplace par `stretch`, ni
//!   ecrit `null`, ni supprime en silence** ;
//! - aucune valeur par defaut n est introduite, ni fonction ni `Default` ;
//! - l absence reste une absence, et c est le lecteur - le portage de
//!   `config.ts`, un autre fichier - qui decidera du sort d une cle absente.
//!
//! ## Le `@deprecated` est une description, pas une reecriture
//!
//! `config.ts:127` annote le champ par
//! `.annotate({ description: "@deprecated Always uses stretch layout." })`.
//! Une `annotation` de `Schema` est une **chaine de documentation** : elle ne
//!contraint rien et ne transforme rien. `Layout::Auto` reste donc constructible
//! et se serialise toujours en `"auto"`.
//!
//! Il serait tentant de faire diverger ici `Auto` par `Stretch` pour respecter
//! l intention de l auteur. Ce serait ajouter une substitution que la source ne
//! fait pas, avec le risque de perdre une configuration valide. La deprecation
//! se traite cote lecteur, quand il ignore reellement la valeur. Le test
//! `auto_ne_devient_jamais_stretch` verrouille ce refus.
//!
//! ## Deux noms a ne pas confondre
//!
//! - le **type** s appelle `Layout` ;
//! - le **schema** s appelle `"LayoutConfig"`, et c est la chaine de
//!   `annotate({ identifier: ... })`, un nom d introspection.
//!
//! `config.ts:20` fait `export type Layout = ConfigLayoutV1.Layout`, un simple
//! alias de type : c est pourquoi le type Rust doit s appeler exactement
//! `Layout`, et non `LayoutConfig`. Les deux noms ne sont pas
//! interchangeables, et `SCHEMA_IDENTIFIER` est le seul qui porte `Config`.
//!
//! ## Limite de verification
//!
//! La bibliotheque `effect` n est pas installee sur cette machine, donc le
//! comportement exact de `Schema.Literals` - notamment le fait qu il prenne un
//! tableau et produise une union a deux membres, plutot que de renvoyer un
//! schema vide ou un seul litteral - n a pas pu etre observe a l execution.
//! Le portage se fonde sur la lecture de la source et sur l usage de la meme
//! construction dans `config.ts:27` pour `LogLevelRef`, ou la forme
//! `Schema.Literals([...])` est de meme nature. Une legere difference de syntaxe
//! est notable : la ce ligne la, l `identifier` est passe dans le meme objet
//! `annotate` que la `description`, alors que `layout.ts:5` ne porte que
//! l `identifier`. Cela ne change rien au type, qui reste une union de deux
//! litteraux de chaine.
//!
//! Ce module ne depend que de `serde` et `serde_json`, comme ses voisins du
//! meme lot.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Identifiant du schema dans le registre `effect/Schema`.
///
/// En TS : `.annotate({ identifier: "LayoutConfig" })` pose en fin de chaine.
///
/// A ne pas confondre avec le nom du type Rust, qui est `Layout` : le
/// suffixe `Config` n appartient qu a cette chaine d introspection.
pub const SCHEMA_IDENTIFIER: &str = "LayoutConfig";

/// Les deux seules valeurs de l union, dans l ordre de la source.
///
/// Sert de temoin : `VALEURS` et `Layout` doivent decrire le meme ensemble,
/// et le test `valeurs_et_type_decrivent_le_meme_ensemble` le verifie dans les
/// deux sens. Ajouter un troisieme litteral dans un seul des deux laisserait le
/// desaccord passer inapercu.
pub const VALEURS: [&str; 2] = ["auto", "stretch"];

/// Mode de mise en page du terminal.
///
/// En TS : `export type Layout = Schema.Schema.Type<typeof Layout>`, soit
/// l union `"auto" | "stretch"`.
///
/// Utilise par `v1/config/config.ts:127` sous la forme
/// `layout: Schema.optional(ConfigLayoutV1.Layout)`. Le champ `layout` et son
/// caractere optionnel sont du ressort de ce fichier la, pas du present.
///
/// Les `rename` ne sont pas redondants : sans eux, serde emettrait `"Auto"`
/// et `"Stretch"`, qui n existent pas dans l union TypeScript. Voir la note
/// sur les majuscules en tete de module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layout {
    /// Le litteral `"auto"` de `Schema.Literals`.
    ///
    /// `@deprecated` cote `config.ts:127`, mais la valeur reste valide et
    /// reste telle quelle sur le fil.
    #[serde(rename = "auto")]
    Auto,

    /// Le litteral `"stretch"` de `Schema.Literals`.
    #[serde(rename = "stretch")]
    Stretch,
}

impl Layout {
    /// La chaine attendue par l union TypeScript.
    ///
    /// Renvoie `"auto"` ou `"stretch"`, jamais `"Auto"`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Layout::Auto => "auto",
            Layout::Stretch => "stretch",
        }
    }

    /// La variante dont la valeur est la chaine donnee, si elle existe.
    ///
    /// La comparaison est **exacte** : `"Auto"`, `"STRETCH"` et `"auto "` ne
    /// donnent rien, comme le refus de ces formes a la lecture. Aucun
    /// `trim`, aucun `eq_ignore_ascii_case`, aucune normalisation : une
    /// difference d un octet fait echouer la conversion.
    pub fn from_name(nom: &str) -> Option<Self> {
        match nom {
            "auto" => Some(Layout::Auto),
            "stretch" => Some(Layout::Stretch),
            _ => None,
        }
    }
}

impl fmt::Display for Layout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------
    // Les deux jugements de valeurs, ecrits DEUX FOIS et jamais fusionnes.
    //
    // Ce ne sont pas du code porte depuis `layout.ts`, qui ne contient ni
    // ternaire ni coalescent. Elles sont ici pour rendre la distinction
    // verifiable : sur une saisie `layout: ""`, elles donnent deux reponses
    // opposees, et aucune des deux n est celle du schema.
    // ---------------------------------------------------------------------

    /// Famille du **ternaire** `x ? a : b` : teste la **veracite**.
    ///
    /// Comme en JavaScript, une chaine vide est falsy, donc la valeur
    /// **disparait** et l on obtient `None`.
    fn disparait_si_falsy(valeur: Option<String>) -> Option<String> {
        valeur.filter(|v| !v.is_empty())
    }

    /// Famille du **coalescent** `x ?? y` : teste la **nullite**.
    ///
    /// Seul `None` declenche le remplacement. Une chaine vide, qui serait
    /// falsy, **survit** intacte.
    fn survit_si_null(valeur: Option<String>) -> Option<String> {
        // `x ?? y` vaut `x`, sauf si `x` est `null`, auquel cas il vaut `y`.
        // Ici `y` redonne `x` : le seul cas qui change est donc l absence, qui
        // reste une absence, et une valeur presente reste presente.
        valeur.or(None)
    }

    /// Conteneur de test, jamais exporte.
    ///
    /// Il ne fait que reproduire la ligne `config.ts:127`,
    /// `layout: Schema.optional(ConfigLayoutV1.Layout)`, pour verifier que
    /// l echange se fait bien sur la cle `layout` en minuscules. Le vrai
    /// conteneur est declare dans le portage de `config.ts`, qui n est pas ce
    /// fichier : ce type vit donc dans `mod tests` et n engage aucun nom.
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Porteur {
        #[serde(rename = "layout", skip_serializing_if = "Option::is_none")]
        layout: Option<Layout>,
    }

    #[test]
    fn les_deux_familles_de_jugement_ne_donnent_pas_le_meme_resultat() {
        // La ligne qui rend le piege visible : sur `Some("")`, le ternaire
        // efface, le coalescent garde.
        assert_eq!(disparait_si_falsy(Some(String::new())), None);
        assert_eq!(survit_si_null(Some(String::new())), Some(String::new()));

        // Sur une valeur pleine, les deux concordent : c est bien la seule
        // difference qui les separe, ce qui rend la comparaison honnete.
        assert_eq!(
            disparait_si_falsy(Some("stretch".to_string())),
            Some("stretch".to_string())
        );
        assert_eq!(
            survit_si_null(Some("stretch".to_string())),
            Some("stretch".to_string())
        );

        // Et sur l absence, les deux sont d accord : `None` n est ni falsy ni
        // null, mais il disparait dans les deux cas. Le type du parametre est
        // nomme ici, sinon le `None` seul ne donne rien a compiler.
        assert_eq!(disparait_si_falsy(None), None);
        assert_eq!(survit_si_null(None), None);
    }

    #[test]
    fn les_noms_serialises_sont_ceux_du_typescript() {
        // Test prioritaire : le piege du fichier est la casse des litteraux.
        // `"auto"` et `"stretch"` doivent sortir en minuscules, et la forme
        // PascalCase de serde ne doit jamais apparaitre.
        assert_eq!(serde_json::to_string(&Layout::Auto).unwrap(), r#""auto""#);
        assert_eq!(
            serde_json::to_string(&Layout::Stretch).unwrap(),
            r#""stretch""#
        );

        // Aucune des formes parasites que serde produirait sans les `rename`
        // ne doit apparaitre. Le test verifie l absence de fuite de casse, et
        // non l egalite avec chaque variante : l egalite est deja piegee par les
        // deux `assert_eq` du dessus, et la refuser ici rendrait ce test
        // impossible a satisfaire.
        let auto = serde_json::to_string(&Layout::Auto).unwrap();
        let stretch = serde_json::to_string(&Layout::Stretch).unwrap();
        for (json, parasites) in [
            (&auto, vec![r#""Auto""#, r#""AUTO""#, r#""aUtO""#, r#""auto\n""#]),
            (&stretch, vec![r#""Stretch""#, r#""STRETCH""#, r#""stretch ""#]),
        ] {
            for parasite in parasites {
                assert_ne!(
                    json.as_str(),
                    parasite,
                    "forme parasite a la place du litteral : {parasite}"
                );
                assert!(
                    !json.contains(parasite),
                    "forme parasite presente dans le json : {json}"
                );
            }
        }

        // Temoin : `as_str` et `Display` rendent la meme chose que serde, donc
        // les deux chemins de relecture ne peuvent pas diverger.
        assert_eq!(Layout::Auto.to_string(), "auto");
        assert_eq!(Layout::Stretch.to_string(), "stretch");
    }

    #[test]
    fn les_formes_fautives_sont_refusees_a_la_lecture() {
        // Contre-test du precedent. Le `#[serde(rename = "auto")]` REMPLACE le
        // nom de la variante, il ne s y ajoute pas : sans lui,
        // `from_str::<Layout>(r#""Auto""#)` reussirait et l echange avec le
        // TypeScript casserait, en silence, sans jamais aucune erreur Rust.
        //
        // C est la transposed exacte du piege `projectID` : le defaut ne se voit
        // qu au moment de parler a l autre language.
        for faux in [
            r#""Auto""#,
            r#""AUTO""#,
            r#""aUtO""#,
            r#""Auto ""#,
            r#""Auto\t""#,
            r#""Stretch""#,
            r#""STRETCH""#,
            r#"" stretch""#,
            r#""stretch ""#,
            r#""auto\n""#,
            r#""auto\u0000""#,
        ] {
            assert!(
                serde_json::from_str::<Layout>(faux).is_err(),
                "forme fautive acceptee : {faux}"
            );
        }

        // Le meme refus, cote convertisseur : les deux chemins de lecture
        // repondent la meme chose. `from_name` est exact, il ne tolere ni
        // casse, ni espace, ni caractere de controle - et le test le prouve sur
        // les memes formes, sinon l un des deux pourrait diverger en silence.
        for faux in [
            "Auto", "AUTO", "aUtO", "auto ", "Auto ", "Auto\t", "Stretch", "STRETCH",
            " stretch", "stretch ", "auto\n", "auto\u{0}", "",
        ] {
            assert_eq!(Layout::from_name(faux), None, "forme fautive acceptee : {faux:?}");
        }

        // Temoin obligatoire : sans lui, un `rename` errone vers un nom qui
        // n existe pas passerait pour un refus correct.
        assert_eq!(
            serde_json::from_str::<Layout>(r#""auto""#).unwrap(),
            Layout::Auto
        );
        assert_eq!(
            serde_json::from_str::<Layout>(r#""stretch""#).unwrap(),
            Layout::Stretch
        );
    }

    #[test]
    fn la_forme_sur_le_fil_est_une_chaine_nue_pas_un_objet() {
        // `config.ts:127` range `ConfigLayoutV1.Layout` tel quel sous la cle
        // `layout` : la valeur est la chaine, pas un objet. Donc pas de
        // representation adjacente, et un objet doit etre refuse.
        for faux in [r#"{"type":"auto"}"#, r#"{"value":"stretch"}"#, r#"["auto"]"#] {
            assert!(
                serde_json::from_str::<Layout>(faux).is_err(),
                "objet accepte a la place d une chaine : {faux}"
            );
        }

        // Echange complet, par le conteneur de `config.ts`.
        let porteur: Porteur = serde_json::from_str(r#"{"layout":"stretch"}"#).unwrap();
        assert_eq!(porteur.layout, Some(Layout::Stretch));
        assert_eq!(
            serde_json::to_string(&porteur).unwrap(),
            r#"{"layout":"stretch"}"#
        );

        // Le nom de la cle est `layout`, tout en minuscules. Si le
        // `#[serde(rename)]` du conteneur disparait, `{"Layout":"auto"}`
        // remplirait le champ : ce test echouerait.
        let majuscule: Porteur = serde_json::from_str(r#"{"Layout":"auto"}"#).unwrap();
        assert_eq!(
            majuscule.layout, None,
            "la forme PascalCase de la cle a ete acceptee : le rename a disparu"
        );
        assert_eq!(serde_json::to_string(&majuscule).unwrap(), "{}");

        // Et la seule forme acceptee reste celle de la source.
        let bon: Porteur = serde_json::from_str(r#"{"layout":"auto"}"#).unwrap();
        assert_eq!(bon.layout, Some(Layout::Auto));
    }

    #[test]
    fn la_chaine_vide_est_refusee_par_le_type_et_non_par_un_filtre() {
        // Piege `?` contre `??`, applique a une saisie reelle : un fichier de
        // configuration saisi a la main peut contenir `layout: ""`.
        //
        // Le ternaire l avalerait (falsy), le coalescent la laisserait passer,
        // et les deux seraient faux. Le schema fait un troisieme chose : `""`
        // n est ni `"auto"` ni `"stretch"`, donc le decodage echoue.
        let vide = Some(String::new());
        assert_eq!(disparait_si_falsy(vide.clone()), None);
        assert_eq!(survit_si_null(vide.clone()), Some(String::new()));

        // Ce que fait reellement le module : refuser, sans rien inventer.
        assert!(serde_json::from_str::<Layout>(r#""""#).is_err());
        assert_eq!(Layout::from_name(""), None);

        // Aucun des deux operateurs n est applique : rien n est remplace par un
        // defaut, rien n est ecrit `null`, rien n est supprime en silence.
        // Seule l absence reste une absence, et c est le lecteur qui tranche.
        let porteur = serde_json::from_str::<Porteur>(r#"{"layout":""}"#);
        assert!(porteur.is_err(), "la chaine vide a ete acceptee");
    }

    #[test]
    fn auto_ne_devient_jamais_stretch() {
        // `config.ts:127` porte `.annotate({ description: "@deprecated Always
        // uses stretch layout." })`. Une annotation est une chaine de
        // documentation : elle n impose rien. Reecrire `Auto` en `Stretch`
        // ajouterait une substitution que la source ne fait pas, et
        // perdrait une configuration valide.
        assert_eq!(Layout::from_name("auto"), Some(Layout::Auto));
        assert_ne!(Layout::Auto, Layout::Stretch);
        assert_eq!(serde_json::to_string(&Layout::Auto).unwrap(), r#""auto""#);

        // Aller-retour : la valeur se restitue inchangee.
        let json = serde_json::to_string(&Layout::Auto).unwrap();
        let relu: Layout = serde_json::from_str(&json).unwrap();
        assert_eq!(relu, Layout::Auto);
    }

    #[test]
    fn valeurs_et_type_decrivent_le_meme_ensemble() {
        // `VALEURS` et `Layout` dupliquent la liste de la source : ils
        // pourraient diverger si un troisieme litteral etait ajoute d un seul
        // cote. On verifie dans les deux sens.
        for nom in VALEURS {
            let variante = Layout::from_name(nom)
                .unwrap_or_else(|| panic!("valeur listee non convertible : {nom}"));
            assert_eq!(variante.as_str(), nom, "aller simple casse : {nom}");

            // Et l'aller-retour par le decodeur lui-meme.
            let json = serde_json::to_string(&variante).unwrap();
            assert_eq!(json, format!(r#""{nom}""#));
            assert_eq!(serde_json::from_str::<Layout>(&json).unwrap(), variante);
        }

        // Sens inverse : toute variante du type est listee, et le compte
        // correspond a celui de `Schema.Literals`.
        assert_eq!(VALEURS.len(), 2);
        for variante in [Layout::Auto, Layout::Stretch] {
            assert!(
                VALEURS.contains(&variante.as_str()),
                "variante absente de VALEURS : {}",
                variante.as_str()
            );
        }

        // Une valeur hors liste n entre dans aucune des deux.
        assert_eq!(Layout::from_name("center"), None);
        assert_eq!(Layout::from_name("fill"), None);
    }

    #[test]
    fn le_nom_du_type_et_lidentifiant_du_schema_sont_deux_choses() {
        // `export type Layout = ConfigLayoutV1.Layout` (`config.ts:20`) oblige
        // le type a s appeler exactement `Layout`, tandis que l introspection
        // porte `"LayoutConfig"`. Inverser les deux ferait echouer l alias de
        // `config.ts`.
        assert_eq!(SCHEMA_IDENTIFIER, "LayoutConfig");
        assert_eq!(VALEURS, ["auto", "stretch"]);
        // Le suffixe `Config` n appartient qu a la chaine d introspection.
        assert!(!SCHEMA_IDENTIFIER.contains("V1"));
    }
}
