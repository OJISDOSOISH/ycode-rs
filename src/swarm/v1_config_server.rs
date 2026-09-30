//! Portage Rust de `opencode/packages/core/src/v1/config/server.ts`.
//!
//! La source fait dix-neuf lignes et ne contient **aucune logique** : uniquement
//! une declaration de schema.
//!
//! ```ts
//! export * as ConfigServerV1 from "./server"
//!
//! import { Schema } from "effect"
//! import { PositiveInt } from "../../schema"
//!
//! export const Server = Schema.Struct({
//!   port: Schema.optional(PositiveInt).annotate({ description: "Port to listen on" }),
//!   hostname: Schema.optional(Schema.String).annotate({ description: "Hostname to listen on" }),
//!   mdns: Schema.optional(Schema.Boolean).annotate({ description: "Enable mDNS service discovery" }),
//!   mdnsDomain: Schema.optional(Schema.String).annotate({
//!     description: "Custom domain name for mDNS service (default: opencode.local)",
//!   }),
//!   cors: Schema.optional(Schema.mutable(Schema.Array(Schema.String))).annotate({
//!     description: "Additional domains to allow for CORS",
//!   }),
//! }).annotate({ identifier: "ServerConfig" })
//! export type Server = Schema.Schema.Type<typeof Server>
//! ```
//!
//! ## Le reexport de la ligne 1
//!
//! `export * as ConfigServerV1 from "./server"` est un reexport d'espace de noms
//! qui pointe sur le fichier lui-meme. C'est du code mort : en Rust le module
//! `v1_config_server` est deja l'espace de noms. Rien a retranscrire.
//!
//! ## Une `Schema.Struct` n'est pas une `Schema.Class`
//!
//! Ici c'est un `Schema.Struct` (et non une `Schema.Class`), donc il n'y a
//! qu'une forme de donnees, sans methode associee. Un `struct` derive
//! `Serialize, Deserialize` suffit. L'identifiant `"ServerConfig"` est un nom
//! de schema pour l'introspection cote serveur ; il n'a pas de contrepartie
//! dans la structure mais on le garde pour que la correspondance reste lisible.
//!
//! ## Piege des noms de champs : `mdnsDomain`
//!
//! C'est le **seul** nom camelCase du fichier, et il porte une majuscule en
//! plein milieu : `mdnsDomain`, pas `mdns_domain`, pas `mdnsdomain`, pas
//! `MdnsDomain`. Le champ Rust s'appelle `mdns_domain` (convention `snake_case`
//! du projet) et porte un `#[serde(rename = "mdnsDomain")]` **explicite et
//! obligatoire**. Sans lui, l'echange avec le TypeScript casserait de facon
//! invisible depuis l'interieur du code Rust. Les quatre autres champs
//! (`port`, `hostname`, `mdns`, `cors`) sont des mots uniques en minuscules :
//! leur nom Rust est deja le nom TypeScript, et le `#[serde(rename = "port")]`
//! mis sur chacun est volontairement redondant, il sert de garde-fou si le nom
//! du champ bouge un jour.
//!
//! Les cinq cles ont ete relues une par une contre la source, ligne a ligne :
//! `port`, `hostname`, `mdns`, `mdnsDomain`, `cors`. Aucune autre.
//!
//! Le `#[serde(rename)]` **remplace** le nom du champ, il ne s'y ajoute pas :
//! ecrire `{"mdns_domain": ...}` dans un fichier de configuration est donc sans
//! effet, la valeur est traitee comme une propriete inconnue et ignoree. C'est
//! ce que verrouille
//! `le_nom_snake_case_mdns_domain_est_refuse_a_la_lecture`, qui essaie les
//! quatre variantes fautives et exige qu'aucune ne remplisse le champ.
//!
//! ## Aucun ternaire `?`, aucun coalescent `??`, et deux fonctions distinctes
//!
//! La source ne contient **ni l'un ni l'autre** : c'est une declaration de
//! schema, pas une expression. Il n'y a donc rien a filtrer, et il faut le
//! dire sans le contourner, parce que ces deux operateurs sont les deux
//! jugement de valeurs les plus facile a confondre du language d'origine :
//!
//! - le **ternaire** `x ? a : b` teste la **veracite**. En JavaScript `""` est
//!   falsy et `0` est falsy, donc `cfg.port ? cfg.port : 4096` avale le `0`.
//! - le **coalescent** `x ?? y` teste la **nullite**. Seuls `null` et
//!   `undefined` declenchent `y` : `cfg.hostname ?? "127.0.0.1"` **conserve**
//!   la chaine vide.
//!
//! Ce module ne contient donc **qu'une seule** de ces deux familles de
//! comportement, et c'est la famille **nullite** : le decodage ne teste rien,
//! il ne fait que transporter ce qui est present. Concretement :
//!
//! - `Some("")` doit **SURVIVRE** (`hostname: ""`, `mdnsDomain: ""`) ;
//! - `Some(false)` doit **SURVIVRE** aussi (`mdns: false` est un booleen faux,
//!   donc falsy en JavaScript, mais c'est bien une valeur de configuration
//!   presente) ;
//! - `Some(vec![])` doit **SURVIVRE** : une liste CORS vide est une liste
//!   donnee, pas une liste absente ;
//! - `None` disparait, et c'est le seul filtre pose :
//!   `skip_serializing_if = "Option::is_none"`.
//!
//! Si un jour quelqu'un ajoute ici un filtre du genre
//! `if v.is_empty() { None }`, il introduit un ternaire `?` qui **n'existe pas**
//! dans la source : ce serait une faute, et elle serait invisible de
//! l'interieur du code Rust. Pour que la distinction reste verifiable et pas
//! seulement affirmee en commentaire, les deux comportements opposes sont
//! ecrits **sous deux formes distinctes** dans le module de tests, avec leurs
//! noms et leurs tests : voir `disparait_si_falsy` (veracite, famille `?`) et
//! `survit_si_null` (nullite, famille `??`), et le test qui prouve qu'ils ne
//! donnent PAS le meme resultat sur `Some("")`. Ce sont des fonctions de test,
//! pas du code porte : ce module n'expose ni l'une ni l'autre.
//!
//! ## `Schema.optional`
//!
//! `Schema.optional(X)` = cle facultative. Cela donne `Option<X>` avec
//! `skip_serializing_if`, donc un champ absent n'apparait pas dans le JSON au
//! lieu de s'y ecrire `null`. Consequence a assumer, identique a celle du
//! reste du lot : Serde lit `null` comme `None` la ou le schema TypeScript
//! distingue `null` de l'absence. Invisible pour du JSON produit par le
//! TypeScript, visible sur une saisie manuelle.
//!
//! ## `cors` est un tableau *mutable* : l'ordre compte
//!
//! La source ecrit `Schema.mutable(Schema.Array(Schema.String))`. Le mot
//! `mutable` est explicite : ce n'est pas un `readonly`, donc pas de
//! `BTreeSet` ni de `BTreeMap` comme le veut la table de conversion pour les
//! collections en lecture seule. C'est un `Vec<String>` et **l'ordre de lecture
//! est preserve**, ce qui compte pour une liste de domaines CORS. Deux tests
//! verrouillent ce point : un seul element, et l'ordre inverse.
//!
//! ## Aucune mecanique reseau, et c'est volontaire
//!
//! Le fichier parle d'un port, d'un nom d'hote et de mDNS, mais **il n'ouvre
//! rien**. Ce sont des valeurs de configuration, pas des actions. Ce qui n'a
//! donc aucune traduction dans ce module, et qui est appele ailleurs dans le
//! code TypeScript :
//!
//! - l'ecoute sur une socket, et l'attribution effective du port d'ecoute ;
//! - la resolution et l'application du nom d'hote ;
//! - l'enregistrement du service mDNS et l'usage du domaine `opencode.local` ;
//! - le traitement des entetes CORS, qui se fait cote middleware HTTP.
//!
//! Rien de tout cela n'est simule ici : ni `TcpListener`, ni `UdpSocket`, ni
//! thread d'annonce. Fabriquer de la mecanique reseau pour "completer" cette
//! structure serait exactement l'erreur a eviter.
//!
//! ## Aucune valeur par defaut, et surtout pas `opencode.local`
//!
//! La chaine `opencode.local` n'apparait que dans le **texte d'une description**,
//! entre parentheses : `(default: opencode.local)`. Ce n'est pas une valeur du
//! schema, pas une constante, et la source ne l'applique nulle part. On ne la
//! declare donc pas, et on n'en fait pas un defaut : le default reel est decide
//! par le code qui lit cette configuration, qui n'est pas dans ce fichier. Pour
//! la meme raison, aucun des cinq champs n'a de defaut ici : les poser en dur
//! creerait une deuxieme source de verite qui divergerait des que l'original
//! bouge. Le test `une_configuration_vide_ne_serialise_que_des_accolades_vides`
//! verrouille ce point.
//!
//! ## `PositiveInt`
//!
//! `PositiveInt` vient de `packages/schema/src/schema.ts` ligne 3 et vaut
//! `Schema.Int.check(Schema.isGreaterThan(0))` : un entier **strictement
//! positif**. On le porte par `u64`, et on **reutilise** le type et le controle
//! deja ports par `config_tool_output.rs` plutot que de les redeclarer ici, pour
//! ne pas creer deux definitions qui divergeraient.
//!
//! Deux queues connues, documentees ici parce qu'elles sont invisibles depuis
//! Rust :
//!
//! 1. `u64` refuse deja les negatifs, mais il accepte `0`, que
//!    `Schema.isGreaterThan(0)` refuse en TypeScript. Le controle `est_positif`
//!    existe pour fermer l'ecart, mais il n'est **pas** appele au decodage :
//!    c'est un choix de coherence avec `config_tool_output.rs`, qui a fait le
//!    meme choix pour `max_lines` et `max_bytes`. Le corriger ici seul
//!    rendrait les deux fichiers divergents pour le meme type ; le corriger
//!    proprement demande de faire de `PositiveInt` un newtype valide au
//!    decodage, dans le module qui le definit, pas ici.
//! 2. `Schema.Int` est un entier **JavaScript**, donc plafonne par
//!    `Number.MAX_SAFE_INTEGER`. `u64` accepte des valeurs bien plus grandes.
//!    Le test `un_entier_au_dela_du_plafond_javascript_est_accepte` fixe cette
//!    divergence ; le comportement exact du cote TypeScript n'a pas pu etre
//!    verifie, la bibliotheque `effect` n'etant pas installee sur cette machine.

use serde::{Deserialize, Serialize};

// `PositiveInt` et son controle sont deja definis par un fichier voisin du meme
// lot. On les reexporte au lieu de les recopier : si la definition change un
// jour (par exemple passage a un newtype valide a la deserialization), ce
// fichier suivra sans etre touche.
pub use crate::swarm::config_tool_output::{est_positif, PositiveInt};

/// Identifiant du schema dans le registre `effect/Schema`.
///
/// En TS : `.annotate({ identifier: "ServerConfig" })` pose en fin de chaine sur
/// le `Schema.Struct`.
pub const SCHEMA_IDENTIFIER: &str = "ServerConfig";

/// Configuration du serveur HTTP, section `server` du fichier de configuration.
///
/// En TS : `export type Server = Schema.Schema.Type<typeof Server>`.
///
/// Utilisee par `v1/config/config.ts` sous la forme
/// `server: Schema.optional(ConfigServerV1.Server)`.
///
/// Les cinq champs sont optionnels : une section `server` absente du fichier de
/// configuration, ou presente mais vide, est valide. Aucun champ n'est
/// obligatoire.
///
/// Les proprietes inconnues sont tolerees, comme en TypeScript : un `Schema.Struct`
/// ignore les champs en trop au lieu de refuser l'objet. On ne pose donc pas
/// `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    /// Port d'ecoute demande, strictement positif.
    ///
    /// La source ne fixe aucun port par defaut et n'ouvre aucune socket : ce
    /// champ ne fait que transmettre une valeur de configuration a l'appelant.
    #[serde(rename = "port", skip_serializing_if = "std::option::Option::is_none")]
    pub port: Option<PositiveInt>,

    /// Nom d'hote auquel le serveur doit se lier.
    #[serde(rename = "hostname", skip_serializing_if = "std::option::Option::is_none")]
    pub hostname: Option<String>,

    /// Active ou non la decouverte de service mDNS.
    #[serde(rename = "mdns", skip_serializing_if = "std::option::Option::is_none")]
    pub mdns: Option<bool>,

    /// Nom de domaine personnalise pour le service mDNS.
    ///
    /// ATTENTION au nom : en TypeScript c'est `mdnsDomain`, avec une majuscule
    /// au milieu. Le `#[serde(rename)]` ci-dessous n'est pas redondant, il est
    /// ce qui fait que l'echange avec le TypeScript fonctionne.
    #[serde(rename = "mdnsDomain", skip_serializing_if = "std::option::Option::is_none")]
    pub mdns_domain: Option<String>,

    /// Domaines supplementaires a autoriser pour le CORS.
    ///
    /// Tableau **mutable** en TypeScript, donc l'ordre est preserve et le type
    /// est un `Vec`, pas un ensemble triee.
    #[serde(rename = "cors", skip_serializing_if = "std::option::Option::is_none")]
    pub cors: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------
    // Les deux jugements de valeurs, ecrits DEUX FOIS et jamais fusionnes.
    //
    // Ce ne sont pas du code porte depuis `server.ts`, qui ne contient ni
    // ternaire ni coalescent. Elles sont ici pour rendre la distinction
    // verifiable : si quelqu'un introduit un jour dans ce module un filtre
    // `is_empty()`, ces deux fonctions montrent en une ligne ce que ce filtre
    // change, et le test qui suit montre qu'elles ne concordent pas.
    // ---------------------------------------------------------------------

    /// Famille du **ternaire** `x ? a : b` : teste la **veracite**.
    ///
    /// Comme en JavaScript, une chaine vide et `0` sont falsy, donc la valeur
    /// **disparait** et l'on obtient `None`.
    fn disparait_si_falsy(valeur: Option<u64>) -> Option<u64> {
        valeur.filter(|v| *v != 0)
    }

    /// Famille du **coalescent** `x ?? y` : teste la **nullite**.
    ///
    /// Seuls `None` declenchent le remplacement. Une chaine vide, donc la
    /// valeur `0` de cet entier, qui serait falsy, **survit** intacte.
    fn survit_si_null<T>(valeur: Option<T>) -> Option<T> {
        // `x ?? y` vaut `x`, sauf si `x` est `null`, auquel cas il vaut `y`.
        // Ici `y` redonne `x` : le seul cas qui change est donc l'absence,
        // qui reste une absence, et une valeur presente reste presente.
        valeur.or(None)
    }

    #[test]
    fn les_deux_familles_de_jugement_ne_donnent_pas_le_meme_resultat() {
        // La ligne qui rend le piege visible : sur `Some(0)`, l'entier
        // equivalent de `Some("")`, le ternaire efface, le coalescent garde.
        assert_eq!(disparait_si_falsy(Some(0)), None);
        assert_eq!(survit_si_null(Some(0)), Some(0));

        // Sur une valeur pleine, les deux concordent : c'est bien la seule
        // difference qui les separe, ce qui rend la comparaison honnete.
        assert_eq!(disparait_si_falsy(Some(4096)), Some(4096));
        assert_eq!(survit_si_null(Some(4096)), Some(4096));

        // Et sur l'absence, les deux sont d'accord : `None` n'est ni falsy ni
        // null, mais il disparait dans les deux cas. Le type du parametre est
        // nomme ici, sinon le `None` seul ne donne rien a compiler.
        assert_eq!(disparait_si_falsy(None), None);
        assert_eq!(survit_si_null::<u64>(None), None);
    }

    #[test]
    fn le_decodage_de_la_configuration_suit_la_famille_nullite() {
        // Le module ne contient aucun ternaire : decodage et re-serialisation
        // suivent donc la famille `??`, la seule presente dans la source.
        // Le test voisin verifie deja que la chaine vide survit ; celui-ci
        // rattache explicitement ce resultat a l'une des deux familles.
        let server: Server =
            serde_json::from_str(r#"{"hostname":"","mdnsDomain":"","cors":[]}"#).unwrap();
        assert_eq!(server.hostname, survit_si_null(Some(String::new())));
        assert_eq!(server.mdns_domain, survit_si_null(Some(String::new())));
        assert_eq!(server.cors, survit_si_null(Some(Vec::new())));

        // Le booleen faux est le second cas falsy du module, et il doit
        // survivre lui aussi : `false` est falsy en JavaScript, mais c'est
        // bien une valeur de configuration presente.
        let faux: Server = serde_json::from_str(r#"{"mdns":false}"#).unwrap();
        assert_eq!(faux.mdns, survit_si_null(Some(false)));
        assert_eq!(serde_json::to_string(&faux).unwrap(), r#"{"mdns":false}"#);

        // Aucun des cinq champs ne s'est fait nettoyer par un filtre de
        // veracite, et le champ absent reste absent.
        let vide: Server = serde_json::from_str("{}").unwrap();
        assert_eq!(serde_json::to_string(&vide).unwrap(), "{}");
    }

    #[test]
    fn le_nom_snake_case_mdns_domain_est_refuse_a_la_lecture() {
        // Test le plus important du fichier apres celui des noms serialises.
        // `#[serde(rename)]` REMPLACE le nom du champ, il ne s'y ajoute pas :
        // du coup `mdns_domain` n'est pas une facon alternative d'ecrire
        // `mdnsDomain`, c'est une propriete inconnue, donc ignoree.
        //
        // Concretement, si le `#[serde(rename = "mdnsDomain")]` disparait du
        // fichier, `serde_json::from_str(r#"{"mdns_domain":"piege.local"}"#)`
        // remplirait le champ. Ce test echouerait. Sans le rename, c'est
        // l'echange avec le TypeScript qui casse, en silence.
        let server: Server = serde_json::from_str(r#"{"mdns_domain":"piege.local"}"#).unwrap();
        assert_eq!(
            server.mdns_domain, None,
            "la forme snake_case a ete acceptee : le rename a disparu"
        );
        assert_eq!(server.hostname, None);
        assert_eq!(server.mdns, None);

        // Les trois autres fautives classiques : tout en minuscules, en
        // PascalCase, et avec la majuscule posee au mauvais endroit. Aucune des
        // trois ne doit remplir le champ.
        for faux in [
            r#"{"mdnsdomain":"piege.local"}"#,
            r#"{"MdnsDomain":"piege.local"}"#,
            r#"{"mdns_DOMAIN":"piege.local"}"#,
        ] {
            let server: Server = serde_json::from_str(faux).unwrap();
            assert_eq!(
                server.mdns_domain, None,
                "forme fautive acceptee : {faux}"
            );
        }

        // Temoin : la seule forme acceptee reste celle de la source, et elle
        // doit toujours fonctionner. Sans ce contre-test, un `#[serde(rename)]`
        // errone vers un nom qui n'existe pas passerait pour un refus correct.
        let bon: Server = serde_json::from_str(r#"{"mdnsDomain":"box.local"}"#).unwrap();
        assert_eq!(bon.mdns_domain.as_deref(), Some("box.local"));
    }

    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        // Test prioritaire : c'est le piege du fichier. `mdnsDomain` doit sortir
        // avec la majuscule, et rien ne doit apparaitre en snake_case.
        let server = Server {
            port: Some(4096),
            hostname: Some("127.0.0.1".to_string()),
            mdns: Some(true),
            mdns_domain: Some("example.local".to_string()),
            cors: Some(vec!["https://a.test".to_string()]),
        };

        let json = serde_json::to_value(&server).unwrap();
        let objet = json.as_object().unwrap();

        for nom in ["port", "hostname", "mdns", "cors"] {
            assert!(objet.contains_key(nom), "champ absent du JSON : {nom}");
        }
        assert!(objet.contains_key("mdnsDomain"), "mdnsDomain absent du JSON");
        assert_eq!(objet.len(), 5, "champ en trop dans le JSON : {json}");

        // La forme snake_case ne doit jamais apparaitre.
        assert!(
            !objet.contains_key("mdns_domain"),
            "le nom Rust a fuite dans le JSON : {json}"
        );

        assert_eq!(objet.get("port").and_then(|v| v.as_u64()), Some(4096));
        assert_eq!(objet.get("mdns").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            objet.get("mdnsDomain").and_then(|v| v.as_str()),
            Some("example.local")
        );
    }

    #[test]
    fn une_configuration_vide_ne_serialise_que_des_accolades_vides() {
        // Aucune valeur par defaut n'est imposee par la source : une section
        // `server: {}` doit rester `{}` et non se remplir de `null`.
        let server = Server {
            port: None,
            hostname: None,
            mdns: None,
            mdns_domain: None,
            cors: None,
        };
        assert_eq!(serde_json::to_string(&server).unwrap(), "{}");

        let relu: Server = serde_json::from_str("{}").unwrap();
        assert_eq!(relu, server);
    }

    #[test]
    fn un_seul_champ_donne_laisse_les_quatre_autres_absents() {
        // Les cinq champs sont independants : en donner un n'oblige pas a
        // donner les autres.
        let server: Server = serde_json::from_str(r#"{"mdnsDomain":"box.local"}"#).unwrap();
        assert_eq!(server.mdns_domain.as_deref(), Some("box.local"));
        assert_eq!(server.port, None);
        assert_eq!(server.hostname, None);
        assert_eq!(server.mdns, None);
        assert_eq!(server.cors, None);

        assert_eq!(
            serde_json::to_string(&server).unwrap(),
            r#"{"mdnsDomain":"box.local"}"#
        );
    }

    #[test]
    fn une_chaine_vide_survit_au_lieu_de_disparaitre() {
        // Piege `?` contre `??` : la source n'a ni ternaire ni coalescent, donc
        // elle ne teste pas la veracite. Une chaine vide est une chaine valide,
        // elle doit rester presente. Un `hostname: ""` ne doit PAS devenir None.
        let server: Server =
            serde_json::from_str(r#"{"hostname":"","mdnsDomain":"","cors":[]}"#).unwrap();

        assert_eq!(server.hostname.as_deref(), Some(""));
        assert_eq!(server.mdns_domain.as_deref(), Some(""));
        let cors: &[String] = server.cors.as_deref().unwrap();
        assert!(cors.is_empty());

        let json = serde_json::to_string(&server).unwrap();
        assert!(json.contains(r#""hostname":"""#), "json inattendu : {json}");
        assert!(json.contains(r#""mdnsDomain":"""#), "json inattendu : {json}");
        // Le tableau vide, lui, reste present aussi : c'est une liste donnee,
        // pas une liste absente.
        assert!(json.contains(r#""cors":[]"#), "json inattendu : {json}");
    }

    #[test]
    fn la_liste_cors_conserve_un_seul_element() {
        let server: Server = serde_json::from_str(r#"{"cors":["https://seul.test"]}"#).unwrap();
        assert_eq!(server.cors, Some(vec!["https://seul.test".to_string()]));

        let vide: Server = serde_json::from_str(r#"{"cors":[]}"#).unwrap();
        assert_eq!(vide.cors, Some(Vec::new()));
    }

    #[test]
    fn la_liste_cors_conserve_lordre_inverse() {
        // Le tableau est `mutable` en TypeScript : l'ordre n'est pas normalise,
        // il est restitue tel quel.
        let source = r#"{"cors":["https://trois.test","https://deux.test","https://un.test"]}"#;
        let server: Server = serde_json::from_str(source).unwrap();

        let attendu = vec![
            "https://trois.test".to_string(),
            "https://deux.test".to_string(),
            "https://un.test".to_string(),
        ];
        assert_eq!(server.cors.as_deref(), Some(attendu.as_slice()));
        assert_eq!(serde_json::to_string(&server).unwrap(), source);
    }

    #[test]
    fn un_port_negatif_ou_non_entier_est_refuse_a_la_lecture() {
        // Le type non signe fait le travail du `check(Schema.isGreaterThan(0))`
        // pour les valeurs negatives.
        assert!(serde_json::from_str::<Server>(r#"{"port":-1}"#).is_err());
        // Un flottant ou une chaine n'est pas un `Schema.Int`.
        assert!(serde_json::from_str::<Server>(r#"{"port":8080.5}"#).is_err());
        assert!(serde_json::from_str::<Server>(r#"{"port":"8080"}"#).is_err());
        // `mdns` reste un booleen, pas une chaine.
        assert!(serde_json::from_str::<Server>(r#"{"mdns":"oui"}"#).is_err());
        // Un `cors` qui n'est pas un tableau de chaines est refuse aussi.
        assert!(serde_json::from_str::<Server>(r#"{"cors":"https://un.test"}"#).is_err());
        assert!(serde_json::from_str::<Server>(r#"{"cors":[1]}"#).is_err());
    }

    #[test]
    fn zero_est_signale_comme_non_positif_par_le_controle() {
        // Divergence connue avec le TypeScript : `u64` accepte `0`, alors que
        // `Schema.isGreaterThan(0)` le refuse au decodage. D'ou `est_positif`,
        // repris du fichier jumeau plutot que redefini ici. Ce controle n'est
        // volontairement pas branche sur la deserialisation, pour rester
        // coherent avec `config_tool_output.rs` sur le meme `PositiveInt`.
        assert!(serde_json::from_str::<Server>(r#"{"port":0}"#).is_ok());
        assert!(!est_positif(0));
        assert!(est_positif(1));
        assert!(est_positif(4096));
    }

    #[test]
    fn un_entier_au_dela_du_plafond_javascript_est_accepte() {
        // Deuxieme divergence connue : `Schema.Int` est un entier JavaScript,
        // donc plafonne par `Number.MAX_SAFE_INTEGER` (2^53 - 1), alors que
        // `u64` monte jusqu'a 2^64 - 1. Ce test fixe le comportement Rust pour
        // qu'il soit au moins explicite ; le comportement exact du schema
        // `effect` n'a pas pu etre verifie ici, la bibliotheque n'etant pas
        // installee sur cette machine.
        //
        // Concretement : un `port` aberrant passe, et il ressort intact, sans
        // etre arrondi ni tronque en chemin.
        let server: Server = serde_json::from_str(r#"{"port":18446744073709551615}"#).unwrap();
        assert_eq!(server.port, Some(u64::MAX));
        assert_eq!(
            serde_json::to_string(&server).unwrap(),
            r#"{"port":18446744073709551615}"#
        );

        // Le plafond JavaScript passe aussi, sans perte.
        let limite: Server = serde_json::from_str(r#"{"port":9007199254740991}"#).unwrap();
        assert_eq!(limite.port, Some(9_007_199_254_740_991));
    }

    #[test]
    fn une_propriete_inconnue_est_ignoree_comme_en_typescript() {
        // Un `Schema.Struct` ne refuse pas les champs en trop : on ne pose pas
        // `deny_unknown_fields`.
        let server: Server = serde_json::from_str(r#"{"port":8080,"inconnu":true}"#).unwrap();
        assert_eq!(server.port, Some(8080));
    }
}
