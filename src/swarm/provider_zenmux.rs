//! Portage Rust de `opencode/packages/core/src/plugin/provider/zenmux.ts`.
//!
//! ## Ce que la source est vraiment
//!
//! Vingt et une lignes, un seul export, et **ce n'est pas** un objet de
//! configuration litteral : c'est un comportement. Le fichier declare un
//! plugin que le moteur interne enregistre au demarrage (c'est la derniere
//! entree de `ProviderPlugins`, `plugin/provider.ts:69`).
//!
//! ```ts
//! export const ZenmuxPlugin = define({
//!   id: "zenmux",
//!   effect: Effect.fn(function* (ctx) {
//!     yield* ctx.catalog.transform(
//!       Effect.fn(function* (evt) {
//!         for (const item of evt.provider.list()) {
//!           if (item.provider.api.type !== "aisdk") continue
//!           if (item.provider.api.package !== "@ai-sdk/openai-compatible") continue
//!           if (item.provider.api.url !== "https://zenmux.ai/api/v1") continue
//!           evt.provider.update(item.provider.id, (provider) => {
//!             provider.request.headers["HTTP-Referer"] ??= "https://opencode.ai/"
//!             provider.request.headers["X-Title"] ??= "opencode"
//!           })
//!         }
//!       }),
//!     )
//!   }),
//! })
//! ```
//!
//! Autrement dit : au moment ou le catalogue est transforme, le plugin
//! recognise le fournisseur Zenmux a trois signes (API de type `aisdk`, paquet
//! npm `@ai-sdk/openai-compatible`, URL exacte `https://zenmux.ai/api/v1`) et
//! lui ajoute **deux entetes de demande par defaut**, `HTTP-Referer` et
//! `X-Title`. Zenmux les exige pour identifier l'appelant, d'ou la valeur
//! `https://opencode.ai/`, qui est l'adresse du site OpenCode et non une URL
//! d'API.
//!
//! `define` (dans `plugin/internal.ts:59`) ne fait rien de plus que
//! retourner son argument : c'est un simple garde-fou de type. Il n'y a
//! donc rien a porter de ce cote, et `Effect.fn` se traduit par une fonction
//! Rust pure, sans `async`.
//!
//! ## Le choix de portage, et ce qu'il laisse de cote
//!
//! La source ne declare **aucun type** : elle manipule des types importes
//! d'ailleurs, `ProviderV2.MutableInfo` et `Catalog.Draft`, qui vivent dans
//! `packages/schema/src/provider.ts` et `packages/core/src/catalog.ts`. Ces
//! deux fichiers appartiennent a d'autres agents et ne sont pas mon
//! revendication. Je ne les porte donc pas.
//!
//! En revanche, un fichier de vingt lignes sans aucun type ne peut pas
//! exprimer son comportement, ni le faire verifier par un test de noms de
//! champs. Je declare donc ici **les formes minimales dont le plugin a
//! besoin** : `id`, `api` (avec `type`, `package`, `url`), `request.headers`.
//! Ce sont des copies locales, pas un portage du schema fournisseur, et il
//! faut le dire : les dix-sept autres plugins `provider/*.ts` du meme lot
//! auront besoin des memes formes. Une deduplication sera necessaire a
//! l'integration, et c'est le fichier de l'integrateur qui devrait ceder,
//! pas celui-ci.
//!
//! Deux choses de `catalog.ts` sont **deliberement absentes** de ce portage,
//! et la revue doit le savoir :
//!
//! 1. `ProviderRecord` vaut `{ provider, models }`. Le plugin ne lit que
//!    `record.provider` et n'ecrit jamais dans `models`. Modeliser la carte
//!    des modeles reviendrait a porter le schema `ModelV2`, qui n'est pas mon
//!    fichier. D'ou la signature `&mut [Info]` plutot qu'un brouillon complet.
//!
//! 2. `evt.provider.update(id, fn)` (catalog.ts:112) fait deux choses de plus
//!    que d'appeler `fn` : il **cree** le fournisseur s'il est absent, puis
//!    appelle `normalizeApi`, qui recopie `request.body.baseURL` dans
//!    `api.url` puis supprime la cle `baseURL` du corps (catalog.ts:99).
//!    Comme le plugin n'appelle `update` que sur des ids qu'il vient de lire
//!    dans `list()`, la creation n'a jamais lieu. Mais `normalizeApi` **si**,
//!    et son effet est visible : apres le passage de ce plugin, un
//!    fournisseur Zenmux qui portait `body.baseURL` se retrouve avec cette
//!    valeur recopiee dans `api.url` et le corps vide de cette cle. Ce
//!    comportement appartient a `catalog.ts`, pas a `zenmux.ts` : il n'est pas
//!    imite ici. C'est le point a verifier en premier si un test d'integration
//!    echoue sur l'URL d'un fournisseur.
//!
//! ## Le piege `??=` : une chaine vide doit survivre
//!
//! ```ts
//! provider.request.headers["HTTP-Referer"] ??= "https://opencode.ai/"
//! ```
//!
//! `??=` est une affectation **coalescente** : elle n'ecrit que si la valeur
//! courante est `null` ou `undefined`. Sur un objet dont la cle est absente,
//! la lecture donne `undefined`, donc l'ecriture a lieu. Mais si la cle est
//! presente avec la chaine vide, `"" ?? x` vaut `""` : **la chaine vide
//! survit et n'est pas remplacee**. C'est l'inverse d'un test de veracite.
//!
//! La traduction Rust est donc `BTreeMap::entry(cle).or_insert(valeur)`, et
//! surtout pas un `if headers.get(cle).is_empty() { ... }` qui, lui,
//! ecraserait la chaine vide. Un test dedie verrouille ce cas.
//!
//! ## Le piege des noms de champs
//!
//! Le champ `integrationID` de `ProviderV2.Info` porte un **D majuscule**, la
//! forme que la revue croisee a deja attrape deux fois ailleurs (`sessionID`,
//! `callID`). Il devient `integration_id` en Rust avec un
//! `#[serde(rename = "integrationID")]` explicite. C'est le seul renommage du
//! fichier.
//!
//! Le champ `type` de l'API n'a pas le meme sort : la source ecrit
//! `Schema.Union([AISDK, Native]).pipe(Schema.toTaggedUnion("type"))`, donc
//! `type` est le **tag de l'union**, pas un champ de struct. Il se traduit
//! par `#[serde(tag = "type")]` sur l'`enum`, et les deux litteraux
//! `"aisdk"` et `"native"` par `#[serde(rename = ...)]` sur les variantes. Le
//! JSON produit porte `type` une seule fois, comme l'original. Contrairement
//! a `swarm/integration_connection.rs`, il n'y a donc **pas** de champ `kind`
//! a renommer ici : la struct interne ne contient pas `type`.
//!
//! Tous les autres noms (`id`, `name`, `disabled`, `package`, `url`,
//! `settings`, `api`, `request`, `headers`, `body`) sont des mots uniques en
//! minuscules, donc aucun renommage n'est requis. Un test serialise chaque
//! struct et compare les noms un par un.
//!
//! ## Une difference de representation, a connaissance
//!
//! En TypeScript, `api` est un objet dont le champ discriminant `type` est
//! une chaine **mutable** : un autre transformateur pourrait reecrire
//! `provider.api.type` sur un objet deja construit. En Rust, `Api` est un
//! `enum`, donc le discriminant n'est pas modifiable apres construction. Cela
//! ne change rien a la logique de ce plugin, qui se contente de lire le
//! discriminant, mais cela rend l'ensemble des plugins du lot plus strict que
//! l'original.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Identifiant du plugin tel qu'il est enregistre par le moteur interne.
///
/// En TS : la propriete `id` de l'objet passe a `define`.
pub const ID: &str = "zenmux";

/// La seule valeur possible du champ `type` d'une API de type `aisdk`.
///
/// En TS : `Schema.Literal("aisdk")` dans `Provider.AISDK`.
pub const TYPE_AISDK: &str = "aisdk";

/// La seule valeur possible du champ `type` d'une API de type `native`.
///
/// En TS : `Schema.Literal("native")` dans `Provider.Native`.
pub const TYPE_NATIVE: &str = "native";

/// Le paquet npm que le plugin exige pour reconnaitre Zenmux.
///
/// En TS : la chaine comparee a `item.provider.api.package`.
pub const PAQUET_AISDK: &str = "@ai-sdk/openai-compatible";

/// L'URL exacte que le plugin exige pour reconnaitre Zenmux.
///
/// En TS : la chaine comparee a `item.provider.api.url`. La comparaison est
/// une egalite stricte, donc un champ `url` absent ne correspond pas.
pub const URL_ZENMUX: &str = "https://zenmux.ai/api/v1";

/// Nom de la premiere entete ajoutee par defaut.
pub const ENTETE_REFERER: &str = "HTTP-Referer";

/// Valeur par defaut de [`ENTETE_REFERER`].
///
/// Ce n'est pas une URL d'API : c'est l'adresse du site OpenCode, que Zenmux
/// attend comme identite de l'appelant.
pub const VALEUR_REFERER: &str = "https://opencode.ai/";

/// Nom de la seconde entete ajoutee par defaut.
pub const ENTETE_TITRE: &str = "X-Title";

/// Valeur par defaut de [`ENTETE_TITRE`].
pub const VALEUR_TITRE: &str = "opencode";

/// Identifiant d'un fournisseur.
///
/// En TS : `Provider.ID`, c'est-a-dire `Schema.String.pipe(Schema.brand(...))`.
/// Le "brand" est une verification de compilation qui n'existe pas dans le
/// JSON, donc un newtype `transparent` qui se serialise comme une chaine.
/// Attention : la comparaison avec l'URL et le paquet se fait sur la **chaine**,
/// pas sur le newtype.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IdFournisseur(pub String);

impl IdFournisseur {
    /// Construit un identifiant a partir de n'importe quelle chaine.
    pub fn nouveau(valeur: impl Into<String>) -> Self {
        Self(valeur.into())
    }

    /// La chaine portee par l'identifiant, sans copie.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Identifiant d'une integration liee a un fournisseur.
///
/// En TS : `Integration.ID`, egalement une chaine brandee. Le champ
/// correspondant dans `Info` s'appelle `integrationID`, avec un **D
/// majuscule** : c'est le renommage le plus important de ce fichier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IdIntegration(pub String);

/// Une cle de `settings` ou de `body` : `Schema.Unknown` en TypeScript, donc
/// n'importe quelle valeur JSON. Le type est `serde_json::Value`.
pub type Valeurs = BTreeMap<String, Value>;

/// Forme de l'API d'un fournisseur, pour le cas `aisdk`.
///
/// En TS : `Provider.AISDK`. Seuls les champs lus ou reecrits par le plugin
/// sont significatifs ici ; `settings` est conserve parce qu'il fait partie
/// de la forme serialisee.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiAisdk {
    /// Nom du paquet npm du SDK AI, par exemple
    /// `@ai-sdk/openai-compatible`. Champ obligatoire.
    pub package: String,

    /// URL de base de l'API. Facultatif : son absence fait echouer la
    /// reconnaissance du fournisseur par le plugin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Reglages libres du SDK. Facultatif.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<Valeurs>,
}

/// Forme de l'API d'un fournisseur, pour le cas `native`.
///
/// En TS : `Provider.Native`. Ici `settings` est **obligatoire**, la ou il
/// est facultatif dans la forme `aisdk` : un objet `native` sans `settings`
/// doit etre refuse, exactement comme en TypeScript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiNative {
    /// URL de base de l'API. Facultatif.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Reglages libres. Obligatoire.
    pub settings: Valeurs,
}

/// Union des deux formes d'API, taggee par le champ `type`.
///
/// En TS :
/// `Schema.Union([AISDK, Native]).pipe(Schema.toTaggedUnion("type"))`.
///
/// C'est le cas d'ADT a tag du tableau de conversion, donc `#[serde(tag =
/// "type")]` et non `untagged`. Le JSON produit porte `type` une seule fois,
/// exactement comme l'original.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Api {
    /// Cas `aisdk`, le seul que ce plugin reconnaisse.
    #[serde(rename = "aisdk")]
    Aisdk(ApiAisdk),

    /// Cas `native`, toujours ecarte par ce plugin.
    #[serde(rename = "native")]
    Native(ApiNative),
}

impl Api {
    /// Construit la forme `aisdk` avec seulement les champs obligatoires.
    pub fn aisdk(package: impl Into<String>) -> Self {
        Self::Aisdk(ApiAisdk {
            package: package.into(),
            url: None,
            settings: None,
        })
    }

    /// Construit la forme `native` avec des reglages vides.
    pub fn native() -> Self {
        Self::Native(ApiNative {
            url: None,
            settings: Valeurs::new(),
        })
    }

    /// La valeur du champ `type`, c'est-a-dire le tag de la variante.
    pub fn tag(&self) -> &'static str {
        match self {
            Self::Aisdk(_) => TYPE_AISDK,
            Self::Native(_) => TYPE_NATIVE,
        }
    }
}

/// Corps de la requete envoyee au fournisseur.
///
/// En TS : `Provider.Request`. Le plugin n'ecrit que dans `headers`, mais
/// `body` fait partie de la forme serialisee et reste donc present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// Entetes HTTP, un `Record<string, string>`. C'est la seule partie que
    /// ce plugin touche.
    pub headers: BTreeMap<String, String>,

    /// Corps de la requete, un `Record<string, Schema.Json>`.
    pub body: Valeurs,
}

impl Request {
    /// Corps vide : ni entete, ni cle de corps.
    pub fn vide() -> Self {
        Self {
            headers: BTreeMap::new(),
            body: Valeurs::new(),
        }
    }
}

/// Description d'un fournisseur du catalogue.
///
/// En TS : `ProviderV2.Info`, en version mutable. Le plugin ne lit que `id`
/// et `api`, et n'ecrit que dans `request.headers` ; la forme complete est
/// neanmoins gardee pour que la serialisation reste conforme a l'original.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Info {
    /// Identifiant du fournisseur.
    pub id: IdFournisseur,

    /// Integration liee au fournisseur, si elle existe.
    ///
    /// **Attention au nom** : le TypeScript ecrit `integrationID`, avec un `D`
    /// majuscule. C'est le renommage invisible de l'interieur du code Rust et
    /// le plus facile a rater de tout ce fichier.
    #[serde(rename = "integrationID", skip_serializing_if = "Option::is_none")]
    pub integration_id: Option<IdIntegration>,

    /// Nom affiche du fournisseur.
    pub name: String,

    /// Vrai si le fournisseur est desactive. Facultatif.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,

    /// Forme de l'API du fournisseur.
    pub api: Api,

    /// Corps de la requete, entetes comprises.
    pub request: Request,
}

impl Info {
    /// Fournisseur minimal : sans integration, actif, avec une API et un
    /// corps vides.
    pub fn nouveau(id: impl Into<String>, name: impl Into<String>, api: Api) -> Self {
        Self {
            id: IdFournisseur::nouveau(id),
            integration_id: None,
            name: name.into(),
            disabled: None,
            api,
            request: Request::vide(),
        }
    }
}

/// Le plugin Zenmux.
///
/// En TS : `export const ZenmuxPlugin = define({ id: "zenmux", effect })`.
///
/// `define` se contente de retourner son argument, donc tout ce que porte le
/// type `Plugin` de `plugin/internal.ts` se resume a un identifiant et a une
/// fonction. La fonction est portee ci-dessous, sous le nom [`appliquer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZenmuxPlugin;

impl ZenmuxPlugin {
    /// L'identifiant du plugin, tel qu'il apparait dans la liste
    /// `ProviderPlugins`. C'est la meme valeur que le [`ID`] du module.
    pub const ID: &'static str = "zenmux";

    /// Construit le plugin. Sans effet, comme `define` en TypeScript.
    pub fn nouveau() -> Self {
        Self
    }

    /// Enregistre la transformation du catalogue.
    ///
    /// Equivalent de l'appel unique
    /// `yield* ctx.catalog.transform(Effect.fn(function* (evt) { ... }))`.
    /// Le `ctx` et l'effet `transform` appartiennent au moteur de plugins, qui
    /// n'est pas porte ici : la fonction recoit donc directement la liste des
    /// fournisseurs a transformer.
    pub fn transformer(self, fournisseurs: &mut [Info]) {
        appliquer(fournisseurs);
    }
}

impl Default for ZenmuxPlugin {
    fn default() -> Self {
        Self
    }
}

/// Indique si l'API d'un fournisseur est celle de Zenmux.
///
/// Les trois conditions de la source, dans le meme ordre et avec la meme
/// portee :
///
/// ```ts
/// if (item.provider.api.type !== "aisdk") continue
/// if (item.provider.api.package !== "@ai-sdk/openai-compatible") continue
/// if (item.provider.api.url !== "https://zenmux.ai/api/v1") continue
/// ```
///
/// La premiere condition ecarte d'office la variante `native`, qui n'a pas de
/// champ `package`. Les deux autres sont des egalites strictes : une URL
/// absente ne correspond pas, et une URL vide ne correspond pas non plus.
pub fn correspond(api: &Api) -> bool {
    match api {
        Api::Aisdk(api) => api.package == PAQUET_AISDK && api.url.as_deref() == Some(URL_ZENMUX),
        Api::Native(_) => false,
    }
}

/// Ajoute les deux entetes par defaut de Zenmux aux fournisseurs concernes.
///
/// Les entetes ne sont ajoutees que si elles sont absentes : c'est la
/// semantique de `??=`. Une entete deja presente survit, y compris quand sa
/// valeur est la chaine vide.
fn ajouter_entetes_par_defaut(fournisseur: &mut Info) {
    fournisseur
        .request
        .headers
        .entry(ENTETE_REFERER.to_string())
        .or_insert_with(|| VALEUR_REFERER.to_string());
    fournisseur
        .request
        .headers
        .entry(ENTETE_TITRE.to_string())
        .or_insert_with(|| VALEUR_TITRE.to_string());
}

/// Applique la transformation Zenmux a un catalogue.
///
/// Equivalent du corps de la fonction passee a `ctx.catalog.transform`. Les
/// fournisseurs sont parcourus dans l'ordre de la liste, et chacun est modifie
/// sur place s'il correspond. La fonction ne renvoie rien, comme l'original.
///
/// La liste venue de `evt.provider.list()` est un instantane ; la muter en
/// place pendant le parcours revient exactement a appeler `update` sur chaque
/// id lu, puisque `update` retrouve le meme enregistrement.
pub fn appliquer(fournisseurs: &mut [Info]) {
    for fournisseur in fournisseurs.iter_mut() {
        if !correspond(&fournisseur.api) {
            continue;
        }
        ajouter_entetes_par_defaut(fournisseur);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit un fournisseur dont l'API est celle de Zenmux, avec des
    /// entetes deja presentes pour pouvoir tester la non-ecrasement.
    fn zenmux(entetes: &[(&str, &str)]) -> Info {
        let mut fournisseur = Info::nouveau("zenmux", "Zenmux", Api::aisdk(PAQUET_AISDK));
        if let Api::Aisdk(api) = &mut fournisseur.api {
            api.url = Some(URL_ZENMUX.to_string());
        }
        for (cle, valeur) in entetes {
            fournisseur
                .request
                .headers
                .insert(cle.to_string(), valeur.to_string());
        }
        fournisseur
    }

    /// Un fournisseur Zenmux recoit les deux entetes par defaut, et son nom,
    /// son identifiant et le reste de sa forme ne bougent pas.
    #[test]
    fn un_fournisseur_zenmux_recoit_les_deux_entetes_par_defaut() {
        let mut catalogue = vec![zenmux(&[])];
        appliquer(&mut catalogue);

        let entetes = &catalogue[0].request.headers;
        assert_eq!(entetes.len(), 2);
        assert_eq!(entetes.get(ENTETE_REFERER).map(String::as_str), Some(VALEUR_REFERER));
        assert_eq!(entetes.get(ENTETE_TITRE).map(String::as_str), Some(VALEUR_TITRE));
        assert_eq!(catalogue[0].id.as_str(), "zenmux");
        assert_eq!(catalogue[0].name, "Zenmux");
        assert_eq!(catalogue[0].integration_id, None);
    }

    /// Une entete deja presente n'est pas remplacee, meme quand sa valeur est
    /// la chaine vide. C'est le cas limite de `??=` : un test de veracite
    /// ecraserait la chaine vide, l'affectation coalescente la conserve.
    #[test]
    fn une_entete_deja_presente_survit_meme_vide() {
        let mut catalogue = vec![zenmux(&[(ENTETE_REFERER, "")])];
        appliquer(&mut catalogue);

        let entetes = &catalogue[0].request.headers;
        assert_eq!(entetes.get(ENTETE_REFERER).map(String::as_str), Some(""));
        assert_eq!(entetes.get(ENTETE_TITRE).map(String::as_str), Some(VALEUR_TITRE));

        // Les deux entetes vides sont preservees de la meme facon.
        let mut catalogue = vec![zenmux(&[(ENTETE_REFERER, ""), (ENTETE_TITRE, "")])];
        appliquer(&mut catalogue);
        let entetes = &catalogue[0].request.headers;
        assert_eq!(entetes.get(ENTETE_REFERER).map(String::as_str), Some(""));
        assert_eq!(entetes.get(ENTETE_TITRE).map(String::as_str), Some(""));
        assert_eq!(entetes.len(), 2);
    }

    /// Une entete deja presente avec une vraie valeur la conserve aussi :
    /// seule l'absence declenche l'ecriture.
    #[test]
    fn une_entete_renseignee_par_l_utilisateur_n_est_pas_ecrasee() {
        let mut catalogue = vec![zenmux(&[(ENTETE_REFERER, "https://moi.example/")])];
        appliquer(&mut catalogue);

        let entetes = &catalogue[0].request.headers;
        assert_eq!(entetes.get(ENTETE_REFERER).map(String::as_str), Some("https://moi.example/"));
        assert_eq!(entetes.get(ENTETE_TITRE).map(String::as_str), Some(VALEUR_TITRE));
        assert_eq!(entetes.len(), 2);
    }

    /// Aucun des trois signes n'est suffisant a lui seul : un paquet
    /// different, une URL differente, une URL absente, une URL vide ou une
    /// API `native` ecarte tous le fournisseur.
    #[test]
    fn un_fournisseur_qui_n_est_pas_zenmux_n_est_pas_touche() {
        let mut autre_paquet = Info::nouveau("autre", "Autre", Api::aisdk("@ai-sdk/anthropic"));
        if let Api::Aisdk(api) = &mut autre_paquet.api {
            api.url = Some(URL_ZENMUX.to_string());
        }

        let mut autre_url = zenmux(&[]);
        if let Api::Aisdk(api) = &mut autre_url.api {
            api.url = Some("https://zenmux.ai/api/v2".to_string());
        }

        let mut url_vide = zenmux(&[]);
        if let Api::Aisdk(api) = &mut url_vide.api {
            api.url = Some(String::new());
        }

        let mut sans_url = zenmux(&[]);
        if let Api::Aisdk(api) = &mut sans_url.api {
            api.url = None;
        }

        let natif = Info::nouveau("natif", "Natif", Api::native());

        let mut catalogue = vec![autre_paquet, autre_url, url_vide, sans_url, natif];
        appliquer(&mut catalogue);

        for (index, fournisseur) in catalogue.iter().enumerate() {
            assert!(
                fournisseur.request.headers.is_empty(),
                "le fournisseur {index} n aurait pas du ete touche"
            );
        }
    }

    /// Seuls les fournisseurs concernes sont modifies, les autres sont laisses
    /// intacts, et l'ordre de la liste n'a aucune importance.
    #[test]
    fn seule_une_partie_du_catalogue_est_modifiee() {
        let avant = Info::nouveau("avant", "Avant", Api::aisdk("@ai-sdk/anthropic"));
        let milieu = zenmux(&[]);
        let apres = Info::nouveau("apres", "Apres", Api::native());
        let dernier = zenmux(&[]);

        let mut catalogue = vec![avant, milieu, apres, dernier];
        appliquer(&mut catalogue);

        assert!(catalogue[0].request.headers.is_empty());
        assert!(catalogue[2].request.headers.is_empty());
        assert_eq!(
            catalogue[1].request.headers.get(ENTETE_REFERER).map(String::as_str),
            Some(VALEUR_REFERER)
        );
        assert_eq!(
            catalogue[3].request.headers.get(ENTETE_TITRE).map(String::as_str),
            Some(VALEUR_TITRE)
        );

        // L'ordre inverse donne exactement le meme resultat.
        let avant2 = Info::nouveau("avant", "Avant", Api::native());
        let milieu2 = zenmux(&[]);
        let mut inverse = vec![avant2, milieu2];
        appliquer(&mut inverse);

        assert!(inverse[0].request.headers.is_empty());
        assert_eq!(inverse[1].request.headers.len(), 2);
        assert_eq!(inverse[1].request.headers, catalogue[1].request.headers);
    }

    /// Une transformation appliquee deux fois ne duplique rien : la seconde
    /// passe ne trouve plus d'entete absente et ne modifie plus rien.
    #[test]
    fn deux_pass_successives_donnent_le_meme_resultat() {
        let mut catalogue = vec![zenmux(&[])];
        appliquer(&mut catalogue);
        let premier = catalogue[0].request.headers.clone();
        appliquer(&mut catalogue);

        assert_eq!(catalogue[0].request.headers, premier);
        assert_eq!(catalogue[0].request.headers.len(), 2);
    }

    /// Une liste vide ne produit rien, et une liste a une seule entree se
    /// comporte comme l'entree isolee.
    #[test]
    fn une_liste_vide_ne_produit_rien() {
        let mut vide: Vec<Info> = Vec::new();
        appliquer(&mut vide);
        assert!(vide.is_empty());

        let mut unique = vec![zenmux(&[])];
        appliquer(&mut unique);
        assert_eq!(unique[0].request.headers.len(), 2);
    }

    /// Les noms de champs serialises sont exactement ceux du TypeScript, y
    /// compris `integrationID` au **D** majuscule, et les deux entetes avec
    /// leur casse. Ce test verrouille l'echange avec le TypeScript.
    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        let mut fournisseur = zenmux(&[]);
        fournisseur.integration_id = Some(IdIntegration("integ_1".to_string()));
        fournisseur.disabled = Some(false);

        let mut catalogue = vec![fournisseur];
        appliquer(&mut catalogue);

        let objet = serde_json::to_value(&catalogue[0]).unwrap();
        let objet = objet.as_object().unwrap();
        for nom in ["id", "integrationID", "name", "disabled", "api", "request"] {
            assert!(objet.contains_key(nom), "champ absent du JSON : {nom}");
        }
        assert_eq!(objet.len(), 6, "aucun champ en trop : {objet:?}");
        assert_eq!(objet.get("id").and_then(|v| v.as_str()), Some("zenmux"));
        assert_eq!(objet.get("integrationID").and_then(|v| v.as_str()), Some("integ_1"));
        assert_eq!(objet.get("name").and_then(|v| v.as_str()), Some("Zenmux"));
        assert_eq!(objet.get("disabled").and_then(|v| v.as_bool()), Some(false));

        // Les variantes Rust des deux noms piegeux ne doivent jamais sortir.
        assert!(!objet.contains_key("integration_id"));
        assert!(!objet.contains_key("integrationId"));
        assert!(!objet.contains_key("IntegrationID"));

        // `request` porte exactement `headers` et `body`.
        let request = objet.get("request").and_then(|v| v.as_object()).unwrap();
        assert_eq!(request.len(), 2);
        assert!(request.contains_key("headers"));
        assert!(request.contains_key("body"));

        // `headers` porte exactement les deux noms d'entetes, avec la casse.
        let headers = request.get("headers").and_then(|v| v.as_object()).unwrap();
        assert_eq!(headers.len(), 2);
        assert_eq!(headers.get(ENTETE_REFERER).and_then(|v| v.as_str()), Some(VALEUR_REFERER));
        assert_eq!(headers.get(ENTETE_TITRE).and_then(|v| v.as_str()), Some(VALEUR_TITRE));
    }

    /// L'API se serialise avec un seul champ `type`, et chaque variante porte
    /// les champs de sa forme d'origine : `package` pour `aisdk`, `settings`
    /// obligatoire pour `native`.
    #[test]
    fn les_noms_de_champs_de_l_api_sont_ceux_du_typescript() {
        let aisdk = Api::aisdk(PAQUET_AISDK);
        assert_eq!(aisdk.tag(), "aisdk");
        let objet = serde_json::to_value(&aisdk).unwrap();
        assert_eq!(objet.get("type").and_then(|v| v.as_str()), Some("aisdk"));
        assert_eq!(objet.get("package").and_then(|v| v.as_str()), Some(PAQUET_AISDK));
        // `url` et `settings` sont absents quand ils sont `None`.
        assert!(!objet.as_object().unwrap().contains_key("url"));
        assert!(!objet.as_object().unwrap().contains_key("settings"));

        let natif = Api::native();
        assert_eq!(natif.tag(), "native");
        let objet = serde_json::to_value(&natif).unwrap();
        assert_eq!(objet.get("type").and_then(|v| v.as_str()), Some("native"));
        assert!(objet.get("settings").is_some());
        assert!(!objet.as_object().unwrap().contains_key("package"));
        assert!(!objet.as_object().unwrap().contains_key("url"));

        // La cle `type` n'apparait qu'une fois par serialisation.
        let json = serde_json::to_string(&Api::aisdk(PAQUET_AISDK)).unwrap();
        assert_eq!(json.matches("\"type\"").count(), 1, "{json}");
    }

    /// Une API venue du TypeScript se relit, et un aller-retour ne change rien.
    #[test]
    fn une_api_venue_du_typescript_se_relit_identique() {
        let brut = r#"{"type":"aisdk","package":"@ai-sdk/openai-compatible","url":"https://zenmux.ai/api/v1"}"#;
        let relu: Api = serde_json::from_str(brut).unwrap();
        assert!(correspond(&relu));
        assert_eq!(serde_json::to_string(&relu).unwrap(), brut);

        let natif: Api = serde_json::from_str(r#"{"type":"native","settings":{}}"#).unwrap();
        assert!(!correspond(&natif));
    }

    /// Une entree invalide est refusee : tag inconnu, champ obligatoire
    /// manquant, `settings` absent d'une API `native`, type faux.
    #[test]
    fn une_api_invalide_est_refusee() {
        assert!(serde_json::from_str::<Api>(r#"{"type":"inconnu","package":"x"}"#).is_err());
        assert!(serde_json::from_str::<Api>(r#"{"type":"aisdk"}"#).is_err());
        assert!(serde_json::from_str::<Api>(r#"{"type":"native"}"#).is_err());
        assert!(serde_json::from_str::<Api>(r#"{"type":"aisdk","package":42}"#).is_err());
        assert!(serde_json::from_str::<Api>(r#"{"type":"aisdk","package":"x","url":7}"#).is_err());
        assert!(serde_json::from_str::<Info>(r#"{"id":"x","name":"y","request":{"headers":{}}}"#).is_err());
    }

    /// L'identifiant du plugin est bien celui qu'attend la liste
    /// `ProviderPlugins`, et le plugin ne fait rien tant qu'on ne l'appelle
    /// pas.
    #[test]
    fn le_plugin_s_annonce_sous_le_bon_identifiant() {
        assert_eq!(ZenmuxPlugin::ID, "zenmux");
        assert_eq!(ID, "zenmux");
        assert_eq!(ZenmuxPlugin::nouveau(), ZenmuxPlugin::default());

        let mut catalogue = vec![zenmux(&[])];
        assert!(
            catalogue[0].request.headers.is_empty(),
            "construire le plugin ne doit rien modifier"
        );
        ZenmuxPlugin::nouveau().transformer(&mut catalogue);
        assert_eq!(catalogue[0].request.headers.len(), 2);
    }
}
