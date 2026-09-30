//! Portage Rust de `opencode/packages/core/src/v1/config/attachment.ts`.
//!
//! La source fait vingt-cinq lignes et ne contient **aucune logique** : deux
//! declarations de schema, quatre `description`, et deux annotations
//! `identifier`.
//!
//! ```ts
//! export * as ConfigAttachmentV1 from "./attachment"
//!
//! import { Schema } from "effect"
//! import { PositiveInt } from "../../schema"
//!
//! export const Image = Schema.Struct({
//!   auto_resize: Schema.optional(Schema.Boolean).annotate({ description: "..." }),
//!   max_width: Schema.optional(PositiveInt).annotate({ description: "..." }),
//!   max_height: Schema.optional(PositiveInt).annotate({ description: "..." }),
//!   max_base64_bytes: Schema.optional(PositiveInt).annotate({ description: "..." }),
//! }).annotate({ identifier: "ImageAttachmentConfig" })
//! export type Image = Schema.Schema.Type<typeof Image>
//!
//! export const Info = Schema.Struct({
//!   image: Schema.optional(Image).annotate({ description: "..." }),
//! }).annotate({ identifier: "AttachmentConfig" })
//! export type Info = Schema.Schema.Type<typeof Info>
//! ```
//!
//! ## Le reexport de la ligne 1
//!
//! `export * as ConfigAttachmentV1 from "./attachment"` est un reexport d'espace
//! de noms qui pointe sur le fichier lui-meme : c'est du code mort, exactement
//! comme dans `v1_config_server.rs` et `v1_config_skills.rs`. En Rust le module
//! `v1_config_attachment` est deja cet espace de noms, rien a retranscrire.
//!
//! ## ATTENTION : deux fichiers voisins ne se confondent pas
//!
//! Il existe **deux** sources distinctes, et ce module porte la **singuliere** :
//!
//! - `packages/core/src/v1/config/attachment.ts` - **le present fichier**. Il
//!   exporte `ConfigAttachmentV1`, utilise `Schema.Struct` (donc pas de
//!   classe, pas de methode, pas de constructeur) et porte les identifiants
//!   `"ImageAttachmentConfig"` et `"AttachmentConfig"`.
//! - `packages/core/src/config/attachments.ts` - le **pluriel**, fichier d'une
//!   autre generation du schema. Il exporte `ConfigAttachments`, utilise
//!   `Schema.Class` et porte les identifiants `"ConfigV2.Attachments.Image"` et
//!   `"ConfigV2.Attachments"`. C'est un autre fichier Rust, et ce module-ci n'a
//!   **rien** a voir avec lui : ni type partage, ni constante, ni identifiant.
//!
//! Les deux sources ont les memes noms de champs, parce qu'elles decrivent la
//! meme idee, mais ce n'est pas une raison de fusionner les deux ports : le
//! passage v1 vers v2 existe et il est ecrit. `v1/config/migrate.ts:51` fait
//! exactement `attachments: info.attachment`, c'est-a-dire que la **cle** passe
//! du **singulier** au **pluriel**. Un module unique qui accepte les deux
//! formes supprimerait ce renommage et ferait disparaitre une distinction
//! reelle du format.
//!
//! C'est aussi pourquoi la cle de section est testee ici : la section v1 est
//! `attachment` (`v1/config/config.ts:130`) et la section v2 est `attachments`
//! (`config.ts:78`). Voir `la_cle_de_section_v1_est_au_singulier`.
//!
//! ## Une `Schema.Struct` n'est pas une `Schema.Class`
//!
//! Le pluriel utilise `export class Image extends Schema.Class<Image>(...)`,
//! ce qui lui donne un constructeur et des methodes. Ici c'est
//! `export const Image = Schema.Struct({...})` : il n'y a qu'une forme de
//! donnees. Aucune methode, aucun `Default`, aucun constructeur ne sont
//! portes, et c'est assume : en ajouter un reviendrait a inventer une API que
//! la source n'a pas.
//!
//! ## Piege des majuscules : ici, la tentation est l'inverse
//!
//! Les noms de champs de la source sont **tous en minuscules, en snake_case** :
//! `auto_resize`, `max_width`, `max_height`, `max_base64_bytes`, puis `image`.
//! Il n'y a **aucune majuscule** a transcrire, et c'est relu ligne a ligne.
//! Contrairement a `v1_config_server.rs` et son `mdnsDomain`, le `#[serde(rename
//! = "...")]` de chaque champ est ici **volontairement redondant** : il sert de
//! garde-fou si le nom du champ bouge un jour, et il rend le contrat d'echange
//! visible sans avoir a ouvrir la source.
//!
//! Le piege reel est donc inverse, et il est **documentaire** : le consommateur
//! emploie du camelCase, et il est tentant d'en propager un dans le schema.
//!
//! - `packages/core/src/image.ts:69-72` ecrit `image.auto_resize ?? true`,
//!   puis `image.max_width ?? 2_000`, `image.max_height ?? 2_000`,
//!   `image.max_base64_bytes ?? 5 * 1024 * 1024` ;
//! - et il transmet a l'adaptateur un objet **interne** en camelCase, dont
//!   `image/photon.ts:24-27` declare `readonly autoResize`, `readonly maxWidth`,
//!   `readonly maxHeight`, `readonly maxBase64Bytes`.
//!
//! Ce dernier objet n'est **pas** la configuration : c'est un `limits`
//! deja normalise, fabrique dans `image.ts`, jamais ecrit sur le disque et
//! jamais relu par le schema. Ecrire `maxBase64Bytes` dans la struct au lieu de
//! `max_base64_bytes` produirait un JSON que le TypeScript lit comme un objet
//! vide silencieusement : tous les reglages d'image seraient ignores, sans la
//! moindre erreur. C'est invisible a la compilation, et c'est exactement ce que
//! verrouillent les deux tests dedies :
//! `les_noms_serialises_sont_ceux_du_typescript` verifie l'ecriture, et
//! `les_formes_camel_et_pascal_case_sont_refusees_a_la_lecture` verifie que
//! `maxBase64Bytes`, `max_width` en PascalCase et les autres formes parasites
//! ne remplissent **aucun** champ.
//!
//! Rappel mecanique : `#[serde(rename)]` **remplace** le nom du champ, il ne
//! s'y ajoute pas. Une propriete inconnue est ignoree au lieu de faire echouer
//! le decodage, comme en TypeScript. Un test qui n'exigerait que le refus de
//! la forme erronee passerait donc meme sans aucun `rename` : il faut donc,
//! dans le meme test, un temoin qui prouve que la forme correcte fonctionne
//! toujours.
//!
//! ## `?` contre `??` : les deux familles, deux fonctions distinctes
//!
//! La source ne contient **ni ternaire ni coalescent** : c'est une declaration
//! de schema, pas une expression. Il n'y a donc rien a filtrer, et il faut le
//! dire sans le contourner, parce que ce sont les deux jugements de valeurs les
//! plus facile a confondre du langage d'origine :
//!
//! - le **ternaire** `x ? a : b` teste la **veracite**. En JavaScript `false`,
//!   `0` et `""` sont falsy, donc `cfg.image?.auto_resize ? ... : ...` avale
//!   `false` ;
//! - le **coalescent** `x ?? y` teste la **nullite**. Seuls `null` et
//!   `undefined` declenchent `y`, donc `image.auto_resize ?? true` **conserve**
//!   `false`.
//!
//! Ce fichier-la est precisement celui ou la confusion coute cher, parce que
//! quatre des cinq champs sont optionnels et parce que l'un d'eux est un
//! **booleen dont la valeur `false` est le seul choix utile**.
//!
//! `auto_resize: false` veut dire "ne redimensionne pas". Un filtre de
//! veracite, c'est-a-dire un ternaire traduit, transforme `false` en absence,
//! puis l'absence en defaut `true` via le `??` de `image.ts:69` : le
//! redimensionnement **revient** alors que l'utilisateur l'a explicitement
//! desactive, en silence, sans erreur de compilation ni erreur de decodage.
//!
//! Concretement, et c'est verrouille par les tests :
//!
//! - `Some(false)` doit **SURVIVRE** pour `auto_resize` ;
//! - `Some(0)` doit **SURVIVRE** au decodage pour les trois `PositiveInt`, la
//!   famille `??` ne filtrant que l'absence ;
//! - `None` disparait, et c'est le seul filtre pose :
//!   `skip_serializing_if = "Option::is_none"`.
//!
//! Pour que la distinction reste **verifiable** et pas seulement affirmee en
//! commentaire, les deux comportements opposes sont ecrits **sous deux formes
//! distinctes** dans le module de tests, jamais fusionnes :
//! `disparait_si_falsy` (veracite, famille `?`) et `survit_si_null` (nullite,
//! famille `??`), avec un test qui prouve qu'elles ne concordent pas sur
//! `Some(false)`. Ce sont des fonctions de test, pas du code porte : ce module
//! n'expose ni l'une ni l'autre.
//!
//! Un second ternaire existe chez le consommateur, `image.ts:64` :
//! `entry.info.attachments?.image ? [entry.info.attachments.image] : []`. Il
//! teste la veracite d'un **objet**, qui est toujours truthy en JavaScript,
//! donc il se comporte comme un test de nullite. Ce n'est pas une
//! autorisation a traiter les champs de ce schema par veracite : c'est
//! exactement l'inverse, un objet n'a pas de valeur "vide".
//!
//! ## Aucun defaut dans ce module : ils sont dans `image.ts`
//!
//! Les quatre `description` de la source contiennent des defauts en toutes
//! lettres : `(default: true)`, `(default: 2000)`, `(default: 2000)`,
//! `(default: 5242880)`. Ce sont des **textes d'annotation**, pas des valeurs.
//! Les vrais defaut sont appliques plus loin, et uniquement par le coalescent
//! de `image.ts:69-72` : `true`, `2_000`, `2_000`, `5 * 1024 * 1024`.
//!
//! Donc :
//!
//! - aucune constante de defaut n'est declaree ici, ni `DEFAULT_MAX_WIDTH` ni
//!   rien d'autre : la source n'en a pas ;
//! - `Image` et `Info` ne remplissent aucun champ a l'ecriture : une section
//!   `attachment: {}` doit ressortir `attachment: {}` ;
//! - un `5_242_880` saisi par l'utilisateur doit ressortir tel quel, et non
//!   normalise vers une constante.
//!
//! ## `PositiveInt`
//!
//! `PositiveInt` vient de `packages/schema/src/schema.ts:3` et vaut
//! `Schema.Int.check(Schema.isGreaterThan(0))` : un entier **strictement
//! positif**. On le porte par `u64`, et on **reutilise** le type et le controle
//! deja ports par `config_tool_output.rs` plutot que de les redeclarer, comme
//! `v1_config_server.rs` l'a fait pour son champ `port`. Une recopie ici
//! divergerait des que l'original bouge.
//!
//! Deux divergences assumees, identiques a celles de `v1_config_server.rs` :
//!
//! 1. `u64` accepte `0`, alors que `Schema.isGreaterThan(0)` le refuse au
//!    decodage. D'ou `est_positif`, repris du fichier jumeau, volontairement
//!    non branche sur la deserialisation pour rester coherent avec lui sur le
//!    meme `PositiveInt`.
//! 2. `Schema.Int` est un entier JavaScript, donc plafonne par
//!    `Number.MAX_SAFE_INTEGER`, alors que `u64` monte bien plus haut.
//!
//! Le comportement exact du cote TypeScript n'a pas pu etre observe a
//! l'execution : la bibliotheque `effect` n'est pas installee sur cette
//! machine.
//!
//! ## Aucun acces disque, et c'est volontaire
//!
//! Le fichier parle d'images, de largeur, de hauteur et d'octets base64, et il
//! ne **lit rien** : ce sont des valeurs, pas une action. Aucun `std::fs`, aucun
//! `Path`, aucun decodeur base64, aucune mesure de dimension n'est porte, et
//! les tests n'ouvrent aucun fichier non plus. Tout ce qui suit se fait chez
//! le consommateur :
//!
//! - le decodage base64 et la lecture de l'image (`image/photon.ts`) ;
//! - la mesure de la largeur, de la hauteur et de la taille encodee, qui sont
//!   des octets et non des caracteres ;
//! - le calcul du facteur d'echelle et le choix du format de re-encode ;
//! - la liberation de la memoire allouee par la bibliotheque de decodage.
//!
//! Aucun index d'octet n'est calcule ici non plus : il n'y a aucune chaine a
//! trancher, donc le risque de couper un caractere non-ASCII n'a pas lieu
//! d'etre ici : il n'y a aucune chaine a trancher, donc le risque de couper un
//! caractere non-ASCII n'a pas lieu d'etre. Les tests sont purs et instantanes :
//! `serde_json` ne fait que de l'arithmetique sur des valeurs en memoire.
//!
//! Ce module ne depend que de `serde` et `serde_json`, comme ses voisins du
//! meme lot.

use serde::{Deserialize, Serialize};

// `PositiveInt` et son controle sont deja definis par un fichier voisin du meme
// lot. On les reexporte au lieu de les recopier : si la definition change un
// jour (par exemple passage a un newtype valide a la deserialization), ce
// fichier suivra sans etre touche.
pub use crate::swarm::config_tool_output::{est_positif, PositiveInt};

/// Identifiant du schema `Image` dans le registre `effect/Schema`.
///
/// En TS : `.annotate({ identifier: "ImageAttachmentConfig" })` pose en fin de
/// chaine sur le `Schema.Struct`.
///
/// A ne pas confondre avec `"AttachmentConfig"`, qui est l'identifiant du
/// `Struct` englobant, ni avec les identifiants du fichier **pluriel**
/// `config/attachments.ts` (`"ConfigV2.Attachments.Image"`), qui sont une autre
/// generation du schema. Voir la section sur les deux fichiers voisins.
pub const IMAGE_SCHEMA_IDENTIFIER: &str = "ImageAttachmentConfig";

/// Identifiant du schema `Info` dans le registre `effect/Schema`.
///
/// En TS : `.annotate({ identifier: "AttachmentConfig" })`.
///
/// Le nom de la **valeur** reste `Info` : c'est le type que
/// `config.ts:130` consomme sous le nom `ConfigAttachmentV1.Info`, et le
/// suffixe `Config` n'appartient qu'a la chaine d'introspection. Le fichier
/// expose deux identifiants, d'ou deux constantes : les fusionner en un seul
/// `SCHEMA_IDENTIFIER` ferait perdre l'un des deux.
pub const INFO_SCHEMA_IDENTIFIER: &str = "AttachmentConfig";

/// Reglages de traitement d'une image en piece jointe, format v1.
///
/// En TS :
/// `export type Image = Schema.Schema.Type<typeof Image>`.
///
/// Utilise par `Info`, qui est le seul conteneur de ce schema.
/// `v1/config/config.ts:130` expose le tout sous la cle **`attachment`**, au
/// singulier, via `attachment: Schema.optional(ConfigAttachmentV1.Info)`.
///
/// Les quatre champs sont optionnels : un objet `image` absent, ou present
/// mais vide, est valide. Aucun champ n'est obligatoire, aucun n'a de valeur par
/// defaut dans ce module, et la source n'en valide aucun.
///
/// Les proprietes inconnues sont tolerees, comme en TypeScript : un
/// `Schema.Struct` ignore les champs en trop au lieu de refuser l'objet. On ne
/// pose donc pas `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// Redimensionner ou non les images avant de les envoyer au modele.
    ///
    /// En TS : `auto_resize`, avec la description
    /// `"Resize images before sending them to the model when they exceed
    /// configured limits (default: true)"`.
    ///
    /// ATTENTION : ce champ est un `Option<bool>` et **non** un `bool`. La
    /// valeur `false` est un choix explicite de l'utilisateur, elle doit donc
    /// survivre au decodage et a la re-serialisation. La distinguer d'une
    /// absence est le seul travail de ce module, et c'est exactement la ou le
    /// ternaire `?` et le coalescent `??` ne se ressemblent pas. Voir
    /// `auto_resize_faux_survit_au_lieu_de_disparaitre`.
    ///
    /// Le `(default: true)` de la description est un **texte** : le defaut
    /// est applique par `image.ts:69`, chez le consommateur.
    #[serde(rename = "auto_resize", skip_serializing_if = "std::option::Option::is_none")]
    pub auto_resize: Option<bool>,

    /// Largeur maximale d'image, strictement positive.
    ///
    /// En TS : `max_width`, avec la description
    /// `"Maximum image width before resizing or rejecting the attachment
    /// (default: 2000)"`.
    ///
    /// Le `(default: 2000)` est un texte d'annotation : la valeur reelle du
    /// defaut est `2_000` et elle est appliquee par `image.ts:70`, pas ici.
    #[serde(rename = "max_width", skip_serializing_if = "std::option::Option::is_none")]
    pub max_width: Option<PositiveInt>,

    /// Hauteur maximale d'image, strictement positive.
    ///
    /// En TS : `max_height`, avec la description
    /// `"Maximum image height before resizing or rejecting the attachment
    /// (default: 2000)"`.
    #[serde(rename = "max_height", skip_serializing_if = "std::option::Option::is_none")]
    pub max_height: Option<PositiveInt>,

    /// Taille maximale de la charge utile base64, strictement positive.
    ///
    /// En TS : `max_base64_bytes`, avec la description
    /// `"Maximum base64 payload bytes for an image attachment (default:
    /// 5242880)"`.
    ///
    /// ATTENTION au nom : c'est bien `max_base64_bytes` en snake_case, et non
    /// `maxBase64Bytes`. Le camelCase existe dans ce depot, mais dans un autre
    /// objet : `image/photon.ts:27` declare `readonly maxBase64Bytes` dans les
    /// `limits` **internes**, deja normalises par `image.ts:72`. Ecrire le
    /// camelCase ici produirait une configuration que le TypeScript lit comme
    /// un objet vide, donc des reglages ignores en silence.
    #[serde(rename = "max_base64_bytes", skip_serializing_if = "std::option::Option::is_none")]
    pub max_base64_bytes: Option<PositiveInt>,
}

/// Configuration du traitement des pieces jointes, section `attachment`.
///
/// En TS : `export type Info = Schema.Schema.Type<typeof Info>`.
///
/// Utilisee par `v1/config/config.ts:130` sous la forme
/// `attachment: Schema.optional(ConfigAttachmentV1.Info)`, puis reprise par
/// `v1/config/migrate.ts:51` qui la renomme `attachments` pour le format v2.
///
/// Le **niveau d'imbrication fait partie du contrat** : `image` est un objet,
/// donc la forme sur le fil est `{"attachment":{"image":{"max_width":2000}}}` et
/// non `{"attachment":{"max_width":2000}}`. Il n'y a pas d'aplatissement, et
/// aucun `flatten` n'est pose ici.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// Reglages specifiques aux images.
    ///
    /// En TS : `image`, avec la description
    /// `"Image attachment configuration"`.
    ///
    /// Le champ englobant est lui aussi optionnel :
    /// `image: Schema.optional(Image)`. Une section `attachment: {}` est donc
    /// valide, et c'est le cas le plus courant.
    #[serde(rename = "image", skip_serializing_if = "std::option::Option::is_none")]
    pub image: Option<Image>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------
    // Les deux jugements de valeurs, ecrits DEUX FOIS et jamais fusionnes.
    //
    // Ce ne sont pas du code porte depuis `attachment.ts`, qui ne contient ni
    // ternaire ni coalescent. Elles sont ici parce que ce fichier est celui ou
    // la distinction mord le plus : `auto_resize` est un booleen dont la seule
    // valeur utile est `false`, qui est falsy en JavaScript. Si quelqu'un
    // introduit un jour dans ce module un filtre de veracite, ces deux
    // fonctions montrent en une ligne ce que ce filtre detruit.
    // ---------------------------------------------------------------------

    /// Famille du **ternaire** `x ? a : b` : teste la **veracite**.
    ///
    /// Comme en JavaScript, `false` est falsy, donc la valeur **disparait** et
    /// l'on obtient `None`. C'est exactement le piege de `auto_resize`.
    fn disparait_si_falsy(valeur: Option<bool>) -> Option<bool> {
        valeur.filter(|&v| v)
    }

    /// Famille du **coalescent** `x ?? y` : teste la **nullite**.
    ///
    /// Seuls `None` declenchent le remplacement. `false`, qui serait falsy,
    /// **survit** intact.
    fn survit_si_null<T>(valeur: Option<T>) -> Option<T> {
        // `x ?? y` vaut `x`, sauf si `x` est `null`, auquel cas il vaut `y`.
        // Ici `y` redonne `x` : le seul cas qui change est donc l'absence, qui
        // reste une absence, et une valeur presente reste presente.
        valeur.or(None)
    }

    /// Conteneur de test, jamais exporte.
    ///
    /// Il ne fait que reproduire `v1/config/config.ts:130`,
    /// `attachment: Schema.optional(ConfigAttachmentV1.Info)`, pour verifier
    /// que l'echange se fait sur la cle **`attachment`**, au singulier. Le vrai
    /// conteneur est declare dans le portage de `config.ts`, qui n'est pas ce
    /// fichier : ce type vit donc dans `mod tests` et n'engage aucun nom.
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Porteur {
        #[serde(rename = "attachment", skip_serializing_if = "std::option::Option::is_none")]
        attachment: Option<Info>,
    }

    #[test]
    fn les_deux_familles_de_jugement_ne_donnent_pas_le_meme_resultat() {
        // La ligne qui rend le piege visible : sur `Some(false)`, le ternaire
        // efface, le coalescent garde.
        assert_eq!(disparait_si_falsy(Some(false)), None);
        assert_eq!(survit_si_null(Some(false)), Some(false));

        // Sur une valeur pleine, les deux concordent : c'est bien la seule
        // difference qui les separe, ce qui rend la comparaison honnete.
        assert_eq!(disparait_si_falsy(Some(true)), Some(true));
        assert_eq!(survit_si_null(Some(true)), Some(true));

        // Et sur l'absence, les deux sont d'accord : `None` n'est ni falsy ni
        // null, mais il disparait dans les deux cas. Le type du parametre est
        // nomme ici, sinon le `None` seul ne donne rien a compiler.
        assert_eq!(disparait_si_falsy(None), None);
        assert_eq!(survit_si_null::<bool>(None), None);

        // Second falsy du fichier, numerique : `0` disparaitrait aussi sous un
        // ternaire, alors que la famille `??` le conserve. Les trois
        // `PositiveInt` sont concernes exactement comme `auto_resize`.
        let zero = Some(0u64);
        assert_eq!(zero.filter(|v| *v == 0), None);
        assert_eq!(survit_si_null(zero), Some(0));
    }

    #[test]
    fn les_noms_serialises_sont_ceux_du_typescript() {
        // Test prioritaire. Les cinq cles doivent sortir exactement comme la
        // source les ecrit, et les formes parasites ne doivent jamais apparaitre.
        let info = Info {
            image: Some(Image {
                auto_resize: Some(true),
                max_width: Some(2000),
                max_height: Some(1200),
                max_base64_bytes: Some(5_242_880),
            }),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert_eq!(
            json,
            r#"{"image":{"auto_resize":true,"max_width":2000,"max_height":1200,"max_base64_bytes":5242880}}"#
        );

        let objet = serde_json::to_value(&info).unwrap();
        let racine = objet.as_object().unwrap();
        assert_eq!(racine.len(), 1, "cle en trop a la racine : {objet}");
        assert!(racine.contains_key("image"), "cle absente : {objet}");

        let image = racine.get("image").unwrap().as_object().unwrap();
        assert_eq!(image.len(), 4, "champ en trop dans image : {objet}");
        for nom in ["auto_resize", "max_width", "max_height", "max_base64_bytes"] {
            assert!(image.contains_key(nom), "champ absent : {nom} dans {objet}");
        }

        // Le camelCase de `image/photon.ts:24-27` appartient aux `limits`
        // internes, pas a la configuration. Aucune de ces formes ne doit sortir.
        for interdit in [
            "autoResize",
            "maxWidth",
            "maxHeight",
            "maxBase64Bytes",
            "AutoResize",
            "MAX_WIDTH",
            "attachments",
        ] {
            assert!(
                !json.contains(interdit),
                "le nom interne de photon.ts a fuite dans le JSON sous {interdit} : {json}"
            );
        }

        // Temoin : `Display` n'existe pas pour une struct, mais l'aller-retour
        // par le decodeur lui-meme doit restituer la valeur inchangee.
        let relu: Info = serde_json::from_str(&json).unwrap();
        assert_eq!(relu, info);
    }

    #[test]
    fn les_formes_camel_et_pascal_case_sont_refusees_a_la_lecture() {
        // Contre-test du precedent, et le plus important du fichier apres lui.
        // `#[serde(rename)]` REMPLACE le nom du champ, il ne s'y ajoute pas :
        // `maxBase64Bytes` n'est donc pas une facon alternative d'ecrire
        // `max_base64_bytes`, c'est une propriete inconnue, donc ignoree.
        //
        // Concretement, si un `#[serde(rename)]` disparait du fichier,
        // `serde_json::from_str(r#"{"maxBase64Bytes":5242880}"#)` remplirait le
        // champ et le TypeScript verrait un objet vide. Ce test echouerait.
        for faux in [
            r#"{"image":{"autoResize":true}}"#,
            r#"{"image":{"AutoResize":true}}"#,
            r#"{"image":{"AUTO_RESIZE":true}}"#,
            r#"{"image":{"autoresize":true}}"#,
            r#"{"image":{"maxWidth":2000}}"#,
            r#"{"image":{"Max_Width":2000}}"#,
            r#"{"image":{"MAX_WIDTH":2000}}"#,
            r#"{"image":{"maxBase64Bytes":5242880}}"#,
            r#"{"image":{"MaxBase64Bytes":5242880}}"#,
        ] {
            let info: Info = serde_json::from_str(faux).unwrap();
            let image = info.image.expect("l'objet image n'a pas ete lu");
            assert_eq!(
                image,
                Image {
                    auto_resize: None,
                    max_width: None,
                    max_height: None,
                    max_base64_bytes: None,
                },
                "forme fautive acceptee : {faux}"
            );
        }

        // Temoin obligatoire : sans lui, un `#[serde(rename)]` errone vers un
        // nom qui n'existe pas passerait pour un refus correct.
        let bon: Info =
            serde_json::from_str(r#"{"image":{"auto_resize":true,"max_base64_bytes":5242880}}"#)
                .unwrap();
        let image = bon.image.expect("la forme correcte a ete refusee");
        assert_eq!(image.auto_resize, Some(true));
        assert_eq!(image.max_base64_bytes, Some(5_242_880));
    }

    #[test]
    fn la_forme_plate_ne_remplit_aucun_champ() {
        // Le niveau d'imbrication fait partie du contrat : `image` est un
        // objet. Un portage qui aplatirait les quatre champs dans `Info`
        // accepterait `{"attachment":{"max_width":2000}}`, forme que le
        // TypeScript lit comme une section sans image, donc ignoree.
        let plate: Info =
            serde_json::from_str(r#"{"auto_resize":true,"max_width":2000,"max_base64_bytes":5242880}"#)
                .unwrap();
        assert_eq!(plate, Info { image: None });
        assert_eq!(serde_json::to_string(&plate).unwrap(), "{}");

        // Et l'inverse : l'objet imbrique est la seule forme acceptee.
        let imbriquee: Info = serde_json::from_str(r#"{"image":{"max_width":2000}}"#).unwrap();
        assert_eq!(
            imbriquee.image.as_ref().and_then(|i| i.max_width),
            Some(2000)
        );
    }

    #[test]
    fn la_cle_de_section_v1_est_au_singulier() {
        // `v1/config/config.ts:130` ecrit `attachment:` au SINGULIER, et
        // `config.ts:78` ecrit `attachments:` au PLURIEL pour la generation
        // suivante. `v1/config/migrate.ts:51` fait le renommage
        // (`attachments: info.attachment`).
        //
        // Accepter les deux formes ici supprimerait ce renommage et
        // confondrait `v1/config/attachment.ts` avec
        // `config/attachments.ts`, qui est un **autre fichier** a porter
        // ailleurs. C'est le genre de faute qui ne se voit qu'a l'echange.
        let porteur: Porteur =
            serde_json::from_str(r#"{"attachment":{"image":{"max_width":2000}}}"#).unwrap();
        assert!(porteur.attachment.is_some(), "la cle v1 n'a pas ete lue");
        assert_eq!(
            serde_json::to_string(&porteur).unwrap(),
            r#"{"attachment":{"image":{"max_width":2000}}}"#
        );

        // La forme pluriee ne remplit rien : c'est la cle de la generation
        // suivante, portee par un autre fichier.
        let pluriel: Porteur =
            serde_json::from_str(r#"{"attachments":{"image":{"max_width":2000}}}"#).unwrap();
        assert_eq!(pluriel, Porteur { attachment: None });
        assert_eq!(serde_json::to_string(&pluriel).unwrap(), "{}");

        // Et la seule forme acceptee reste bien celle de la source.
        let bon: Porteur = serde_json::from_str(r#"{"attachment":{}}"#).unwrap();
        assert_eq!(bon, Porteur { attachment: Some(Info { image: None }) });
    }

    #[test]
    fn auto_resize_faux_survit_au_lieu_de_disparaitre() {
        // Piege `?` contre `??`, sur la seule valeur utile du champ.
        //
        // `auto_resize: false` veut dire "ne redimensionne pas". Un filtre de
        // veracite le transformerait en absence, puis `image.ts:69`
        // (`image.auto_resize ?? true`) appliquerait `true` : le
        // redimensionnement reviendrait, en silence. Le module applique la
        // famille **nullite** : la valeur presente reste presente.
        let info: Info = serde_json::from_str(r#"{"image":{"auto_resize":false}}"#).unwrap();
        let image = info.image.as_ref().expect("image absent");
        assert_eq!(image.auto_resize, survit_si_null(Some(false)));
        assert_ne!(
            image.auto_resize,
            Some(true),
            "le faux a ete remplace par le defaut : filtre de veracite interdit"
        );

        // L'aller-retour doit restituer `false`, et non l'omettre.
        let json = serde_json::to_string(&info).unwrap();
        assert_eq!(json, r#"{"image":{"auto_resize":false}}"#);

        // Le ternaire, lui, aurait disparu. Les deux comportements sont
        // compares ici pour que la difference reste visible.
        assert_eq!(disparait_si_falsy(Some(false)), None);
    }

    #[test]
    fn un_zero_survit_au_decodage_mais_est_refuse_par_le_controle() {
        // Deuxieme falsy du fichier. La famille `??` ne filtre que l'absence,
        // donc `0` se decode et se restitue tel quel.
        let info: Info = serde_json::from_str(r#"{"image":{"max_width":0}}"#).unwrap();
        let image = info.image.as_ref().expect("image absent");
        assert_eq!(image.max_width, Some(0));
        assert_eq!(
            serde_json::to_string(&info).unwrap(),
            r#"{"image":{"max_width":0}}"#
        );

        // En revanche `Schema.isGreaterThan(0)` refuse `0` en TypeScript, et
        // `u64` ne le peut pas. D'ou `est_positif`, repris de
        // `config_tool_output.rs` et volontairement non branche sur la
        // deserialisation, pour rester coherent avec lui sur le meme
        // `PositiveInt`. Voir la section `PositiveInt` du crate.
        assert!(!est_positif(0));
        assert!(est_positif(1));
        assert!(est_positif(5_242_880));
    }

    #[test]
    fn une_configuration_vide_ne_serialise_que_des_accolades_vides() {
        // Aucune valeur par defaut n'est imposee par la source : une section
        // `attachment: {}` doit rester `{}`, et non se remplir de `true`, de
        // `2000` et de `5242880`. Les quatre `description` qui citent ces
        // defauts sont des **textes**, et les vrais defauts sont appliques par
        // `image.ts:69-72`.
        let info = Info { image: None };
        assert_eq!(serde_json::to_string(&info).unwrap(), "{}");

        let image = Info {
            image: Some(Image {
                auto_resize: None,
                max_width: None,
                max_height: None,
                max_base64_bytes: None,
            }),
        };
        assert_eq!(serde_json::to_string(&image).unwrap(), r#"{"image":{}}"#);

        // Aller-retour des deux etats vides.
        let relu: Info = serde_json::from_str("{}").unwrap();
        assert_eq!(relu, info);
        let relu_image: Info = serde_json::from_str(r#"{"image":{}}"#).unwrap();
        assert_eq!(relu_image, image);
    }

    #[test]
    fn une_valeur_saisie_n_est_jamais_remplacee_par_un_defaut() {
        // `max_base64_bytes: 5242880` est aussi la valeur du defaut, donc le
        // test serait trompeur s'il ne distinguait pas "l'utilisateur l'a
        // ecrite" de "le module l'a ajoute". Une valeur **differente** du
        // defaut ne doit surtout pas disparaitre au profit d'une constante.
        let saisi: Info = serde_json::from_str(r#"{"image":{"max_base64_bytes":7}}"#).unwrap();
        let image = saisi.image.as_ref().expect("image absent");
        assert_eq!(image.max_base64_bytes, Some(7));
        assert_eq!(
            serde_json::to_string(&saisi).unwrap(),
            r#"{"image":{"max_base64_bytes":7}}"#
        );

        // Le defaut du texte d'annotation ne doit apparaitre nulle part tout
        // seul, et la source ne fixe aucun `max_width` par defaut dans ce
        // module.
        let vide: Info = serde_json::from_str(r#"{"image":{}}"#).unwrap();
        let json = serde_json::to_string(&vide).unwrap();
        for interdit in ["2000", "5242880", "true", "null"] {
            assert!(
                !json.contains(interdit),
                "une valeur a ete injectee dans une section vide : {json}"
            );
        }
    }

    #[test]
    fn un_seul_champ_laisse_les_autres_absents() {
        // Les cinq champs sont independants : en donner un n'oblige pas a
        // donner les autres, et n'en supprime aucun.
        let info: Info = serde_json::from_str(r#"{"image":{"max_base64_bytes":1048576}}"#).unwrap();
        let image = info.image.expect("image absent");
        assert_eq!(image.max_base64_bytes, Some(1_048_576));
        assert_eq!(image.auto_resize, None);
        assert_eq!(image.max_width, None);
        assert_eq!(image.max_height, None);

        // Et l'objet `Info` lui-meme : donner `image` n'oblige a rien d'autre,
        // c'est le seul champ qu'il porte.
        let porteur: Porteur =
            serde_json::from_str(r#"{"attachment":{"image":{"max_width":800}}}"#).unwrap();
        assert_eq!(
            serde_json::to_string(&porteur).unwrap(),
            r#"{"attachment":{"image":{"max_width":800}}}"#
        );
    }

    #[test]
    fn un_type_incorrect_est_refuse_a_la_lecture() {
        // `auto_resize` reste un booleen, pas une chaine et pas un nombre.
        assert!(serde_json::from_str::<Image>(r#"{"auto_resize":"true"}"#).is_err());
        assert!(serde_json::from_str::<Image>(r#"{"auto_resize":1}"#).is_err());
        assert!(serde_json::from_str::<Image>(r#"{"auto_resize":null}"#).is_ok());

        // Les trois autres restent des `Schema.Int` : pas un flottant, pas une
        // chaine, pas un negatif. Le type non signe fait le travail du
        // `check(Schema.isGreaterThan(0))` pour les valeurs negatives.
        assert!(serde_json::from_str::<Image>(r#"{"max_width":2000.5}"#).is_err());
        assert!(serde_json::from_str::<Image>(r#"{"max_width":"2000"}"#).is_err());
        assert!(serde_json::from_str::<Image>(r#"{"max_width":-1}"#).is_err());
        assert!(serde_json::from_str::<Image>(r#"{"max_height":[]}"#).is_err());
        assert!(serde_json::from_str::<Image>(r#"{"max_base64_bytes":{}}"#).is_err());

        // `image` est un objet, pas un tableau ni une chaine.
        assert!(serde_json::from_str::<Info>(r#"{"image":[]}"#).is_err());
        assert!(serde_json::from_str::<Info>(r#"{"image":"oui"}"#).is_err());
        assert!(serde_json::from_str::<Info>(r#"{"image":{"max_width":false}}"#).is_err());

        // Et la section entiere doit etre un objet.
        assert!(serde_json::from_str::<Porteur>(r#"{"attachment":"oui"}"#).is_err());
        assert!(serde_json::from_str::<Porteur>(r#"{"attachment":[1]}"#).is_err());
    }

    #[test]
    fn une_propriete_inconnue_est_ignoree_comme_en_typescript() {
        // Un `Schema.Struct` ne refuse pas les champs en trop : on ne pose pas
        // `deny_unknown_fields`. C'est ce qui laisse passer une saisie avec une
        // faute de frappe **sans erreur**, et sans effet non plus.
        let info: Info = serde_json::from_str(
            r#"{"image":{"max_width":2000,"inconnu":true},"autre_section":1}"#,
        )
        .unwrap();
        let image = info.image.expect("image absent");
        assert_eq!(image.max_width, Some(2000));
        assert_eq!(image.max_height, None);
        assert_eq!(image.max_base64_bytes, None);
        assert_eq!(image.auto_resize, None);
    }

    #[test]
    fn un_entier_au_dela_du_plafond_javascript_est_accepte() {
        // Deuxieme divergence assumee avec le TypeScript : `Schema.Int` est un
        // entier JavaScript, donc plafonne par `Number.MAX_SAFE_INTEGER`
        // (2^53 - 1), alors que `u64` monte jusqu'a 2^64 - 1. Ce test fixe le
        // comportement Rust pour qu'il soit explicite ; le comportement exact du
        // schema `effect` n'a pas pu etre verifie ici, la bibliotheque n'etant
        // pas installee sur cette machine.
        let gros: Info =
            serde_json::from_str(r#"{"image":{"max_base64_bytes":18446744073709551615}}"#)
                .unwrap();
        let image = gros.image.expect("image absent");
        assert_eq!(image.max_base64_bytes, Some(u64::MAX));

        // Et la valeur juste au-dessus du plafond JavaScript passe aussi.
        let limite: Info =
            serde_json::from_str(r#"{"image":{"max_width":9007199254740992}}"#).unwrap();
        assert_eq!(
            limite.image.as_ref().and_then(|i| i.max_width),
            Some(9_007_199_254_740_992)
        );
    }

    #[test]
    fn le_round_trip_ne_perd_aucune_forme() {
        // Ce que le TypeScript ecrit, Rust le relit, et l'inverse : le contrat
        // d'echange complet, dans les deux sens, sur une configuration
        // plausible et sur la forme la plus ennuyeuse, c'est-a-dire
        // `auto_resize: false`.
        let json = r#"{"attachment":{"image":{"auto_resize":false,"max_width":2000,"max_height":2000,"max_base64_bytes":5242880}}}"#;
        let porteur: Porteur = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&porteur).unwrap(), json);

        // Et la section presente mais vide, que `config.ts:130` autorise.
        let vide: Porteur = serde_json::from_str(r#"{"attachment":{}}"#).unwrap();
        assert_eq!(serde_json::to_string(&vide).unwrap(), r#"{"attachment":{}}"#);

        // Le decodage ne touche a aucun octet de donnees : le JSON ressort
        // octet pour octet identique.
        let image: Image = serde_json::from_str(
            r#"{"auto_resize":false,"max_width":1,"max_height":2,"max_base64_bytes":3}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_string(&image).unwrap(),
            r#"{"auto_resize":false,"max_width":1,"max_height":2,"max_base64_bytes":3}"#
        );
    }

    #[test]
    fn les_deux_identifiants_de_schema_sont_ceux_du_v1() {
        // Ce fichier porte **deux** `identifier`, d'ou deux constantes. Les
        // confondre ferait echouer l'introspection cote serveur.
        assert_eq!(IMAGE_SCHEMA_IDENTIFIER, "ImageAttachmentConfig");
        assert_eq!(INFO_SCHEMA_IDENTIFIER, "AttachmentConfig");

        // Ce ne sont pas les identifiants du fichier **pluriel**
        // `config/attachments.ts`, qui est une autre generation du schema et
        // doit etre porte ailleurs. Aucun de ces quatre noms ne doit apparaitre
        // ici.
        for interdit in [
            "ConfigV2.Attachments.Image",
            "ConfigV2.Attachments",
            "ConfigAttachmentsV1",
        ] {
            assert_ne!(IMAGE_SCHEMA_IDENTIFIER, interdit);
            assert_ne!(INFO_SCHEMA_IDENTIFIER, interdit);
        }

        // Et les deux noms de ce fichier ne se ressemblent pas : les fusionner
        // en un seul `SCHEMA_IDENTIFIER` ferait perdre l'un des deux.
        assert_ne!(IMAGE_SCHEMA_IDENTIFIER, INFO_SCHEMA_IDENTIFIER);
        assert!(IMAGE_SCHEMA_IDENTIFIER.contains("Image"));
        assert!(!INFO_SCHEMA_IDENTIFIER.contains("Image"));
    }
}
