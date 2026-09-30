//! Portage Rust de `opencode/packages/core/src/plugin/provider/vercel.ts`.
//!
//! ## Ce que fait la source
//!
//! Vingt-sept lignes, deux crochets, aucun schema :
//!
//! 1. **Transformation du catalogue.** Pour chaque fournisseur dont l'api est
//!    de type `aisdk` **et** dont le paquet est `@ai-sdk/vercel`, le plugin
//!    ecrit deux en-tetes dans `provider.request.headers` : `http-referer`
//!    valant `https://opencode.ai/`, puis `x-title` valant `opencode`.
//! 2. **Crochet `aisdk.sdk`.** Si le paquet de l'evenement vaut
//!    `@ai-sdk/vercel`, le plugin importe le paquet npm et range
//!    `createVercel(evt.options)` dans `evt.sdk`. Sinon il ne touche a rien.
//!
//! ## Ce qui est representable en Rust, et ce qui ne l'est pas
//!
//! - L'enregistrement des crochets (`ctx.catalog.transform(...)`,
//!   `ctx.aisdk.sdk(...)`) et les `Effect.fn` qui les enveloppe appartiennent a
//!   l'hote de plugins. Ce fichier expose donc les deux regles sous forme de
//!   fonctions pures, comme la regle le demande. Comme en TS, rien ne
//!   s'execute a la construction du module.
//! - `import("@ai-sdk/vercel")` est un import dynamique de module npm : il
//!   n'a pas d'equivalent Rust. On ne peut pas charger du JavaScript a
//!   l'execution, et fabriquer un objet `sdk` ici serait inventer un
//!   comportement. Le crochet est donc porte jusqu'a son point de decision et
//!   s'arrete la : la fonction renvoie le nom de la fabrique a appeler
//!   (`createVercel`) et les options a lui passer. C'est la seule chose que la
//!   source perde, et elle ne se voyait qu'a l'execution JavaScript.
//! - `evt.sdk` est de type `any` cote TS. Il n'a pas de representation Rust, il
//!   n'est donc pas declare dans l'evenement.
//!
//! ## Les types de donnees
//!
//! Le plugin lit trois formes declarees ailleurs : `ProviderApi` et
//! `ProviderRequest` du catalogue, et l'evenement `AISDK.SDKEvent`. Elles sont
//! recopiees ici **au strict minimum de ce que ce fichier touche**, avec les
//! noms de champs exacts du TypeScript, parce qu'un module voisin du portage
//! porte deja les formes completes. Ces recopies n'ont pas vocation a
//! remplacer celles-ci : si elles apparaissent un jour, il faut supprimer celles
//! d'ici et ne garder que des `use`.
//!
//! - `VercelApi` reprend l'union `{ type: "aisdk", package, url?, settings? } |
//!   { type: "native", url?, settings }`. Elle est **interne** : le champ `type`
//!   est le discriminant, ce qui est exactement la forme du JSON.
//! - `VercelRequest` reprend `{ headers, body }`, deux dictionnaires libres.
//! - `VercelProvider` reprend le strict necessaire de `ProviderV2Info` pour ce
//!   plugin : `id`, `api`, `request`. Les champs `name`, `disabled` et
//!   `integrationID` ne sont ni lus ni ecrits ici, ils sont donc absents.
//! - `VercelCatalogDraft` est leEquivalent du `Draft` du catalogue, reduit a
//!   sa partie `provider` et aux deux methodes utilisees (`list` et `update`).
//!   La vraie Map de fournisseurs est ordonnee par insertion en JavaScript,
//!   d'ou le `Vec` ici : l'ordre du catalogue est observable, il est donc
//!   preserve et jamais trie.
//!
//! ## Noms de champs
//!
//! Aucun nom de ce fichier n'est en camelCase a l'exception de `type`, qui
//! confle avec le mot cle Rust et porte donc un `#[serde(rename = "type")]`
//! explicite. `package`, `url`, `settings`, `headers`, `body`, `id`, `api` et
//! `request` sont identiques dans les deux langages, donc sans renommage. Un
//! test serialise chaque structure et compare les cles une par une, parce que
//! c'est l'erreur la plus invisible de ce portage.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Litteraux du plugin
// ---------------------------------------------------------------------------

/// Identifiant enregistre par `define`, dans `plugin.internal`.
pub const PLUGIN_ID: &str = "vercel";

/// Type d'api que le plugin reconnait, premiere des deux conditions du filtre.
pub const AISDK_TYPE: &str = "aisdk";

/// Type d'api concurrent. Le plugin l'ignore sans jamais lire son paquet.
pub const NATIVE_TYPE: &str = "native";

/// Paquet npm reconnu, deuxieme condition du filtre, et paquet du crochet sdk.
pub const AISDK_PACKAGE: &str = "@ai-sdk/vercel";

/// Fabrique exportee par le paquet, appelee avec les options de l'evenement.
pub const AISDK_FACTORY: &str = "createVercel";

/// Premiere entete ajoutee. En minuscules et en tirets, comme en TypeScript :
/// le test TS verifie explicitement que `HTTP-Referer` n'est **pas** ecrit.
pub const HTTP_REFERER_KEY: &str = "http-referer";

/// Valeur de la premiere entete.
pub const HTTP_REFERER_VALUE: &str = "https://opencode.ai/";

/// Deuxieme entete ajoutee, meme convention de casse.
pub const X_TITLE_KEY: &str = "x-title";

/// Valeur de la deuxieme entete.
pub const X_TITLE_VALUE: &str = "opencode";

// ---------------------------------------------------------------------------
// Donnees lues et ecrites par le plugin
// ---------------------------------------------------------------------------

/// L'api d'un fournisseur : soit un paquet aiskd, soit une api native.
///
/// En TS : `ProviderApi = ProviderAisdk | ProviderNative`, deux objets
/// discriminant par `type`. `ProviderAisdk` porte `package` ; `ProviderNative`
/// n'a **pas** de champ `package`, c'est pourquoi la variante native n'en
/// declare pas. `settings` est optionnel pour `aisdk` et obligatoire pour
/// `native`, comme dans la source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum VercelApi {
    /// `{ type: "aisdk", package, url?, settings? }`.
    Aisdk {
        /// Paquet npm du fournisseur.
        package: String,
        /// URL de base, absente tant que le catalogue ne l'a pas remplie.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        /// Reglages libres passes au paquet.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        settings: Option<BTreeMap<String, Value>>,
    },
    /// `{ type: "native", url?, settings }`.
    Native {
        /// URL de base, absente tant que le catalogue ne l'a pas remplie.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        /// Reglages libres. Obligatoires en TS, donc obligatoires ici.
        settings: BTreeMap<String, Value>,
    },
}

impl VercelApi {
    /// Api aiskd reduite a son paquet, la forme que tous les plugins ecrivent.
    pub fn aisdk(package: impl Into<String>) -> Self {
        VercelApi::Aisdk { package: package.into(), url: None, settings: None }
    }

    /// Api native sans URL et sans reglage.
    pub fn native() -> Self {
        VercelApi::Native { url: None, settings: BTreeMap::new() }
    }

    /// Le discriminant, c'est-a-dire la valeur du champ JSON `type`.
    pub fn type_name(&self) -> &'static str {
        match self {
            VercelApi::Aisdk { .. } => AISDK_TYPE,
            VercelApi::Native { .. } => NATIVE_TYPE,
        }
    }

    /// Le paquet npm, ou `None` pour une api native qui n'en a pas.
    pub fn package(&self) -> Option<&str> {
        match self {
            VercelApi::Aisdk { package, .. } => Some(package.as_str()),
            VercelApi::Native { .. } => None,
        }
    }

    /// Les deux conditions du filtre du TS, dans le meme ordre : le type
    /// d'abord, le paquet ensuite. Une api native echoue sur la premiere.
    pub fn is_vercel_aisdk(&self) -> bool {
        match self {
            VercelApi::Aisdk { package, .. } => package.as_str() == AISDK_PACKAGE,
            VercelApi::Native { .. } => false,
        }
    }
}

/// La partie `request` d'un fournisseur : deux dictionnaires libres.
///
/// En TS : `ProviderRequest = { headers: Record<string, string>, body:
/// Record<string, unknown> }`. Les deux champs sont obligatoires. Les cles de
/// `headers` sont des noms d'en-tetes HTTP, donc des chaines libres en
/// minuscules ou en majuscules : le filtre du plugin ne s'y applique pas et il
/// ne faut surtout pas les normaliser.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct VercelRequest {
    /// En-tetes additionnels, cle et valeur en chaine.
    pub headers: BTreeMap<String, String>,
    /// Corps de requete additionnel, valeurs libres.
    pub body: BTreeMap<String, Value>,
}

impl VercelRequest {
    /// Requete vide : aucun en-tete, aucun corps.
    pub fn new() -> Self {
        VercelRequest::default()
    }

    /// Lit un en-tete par son nom exact, ou `None` s'il n'est pas present.
    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers.get(key).map(|value| value.as_str())
    }

    /// Ecrit un en-tete. Comme l'affectation JavaScript
    /// `headers[key] = value`, un nom deja present est **ecrase**, pas refuse.
    pub fn set_header(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.headers.insert(key.into(), value.into());
    }
}

/// Un fournisseur tel que la transformation du catalogue le voit.
///
/// Version reduite de `ProviderV2Info` : seuls `id`, `api` et `request` sont
/// lus ou ecrits par ce plugin. Les champs `name`, `disabled` et
/// `integrationID` de la source sont absents ici ; s'ils sont reecrits par un
/// portage du catalogue, ce sont eux qui doivent les porter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VercelProvider {
    /// Identifiant du fournisseur. Sert de cle a `update` dans le catalogue.
    pub id: String,
    /// Son api, qui porte le type et le paquet lus par le filtre.
    pub api: VercelApi,
    /// Sa requete, seule partie que le plugin modifie.
    pub request: VercelRequest,
}

impl VercelProvider {
    /// Fournisseur sans en-tete ni corps.
    pub fn new(id: impl Into<String>, api: VercelApi) -> Self {
        VercelProvider { id: id.into(), api, request: VercelRequest::new() }
    }

    /// Ecrit les deux en-tetes heritage, dans l'ordre de la source.
    ///
    /// Le TS ecrit les deux en-tetes a la main, sans condition. Elles sont
    /// regroupees ici pour qu'il n'existe qu'un seul endroit ou tenir la liste,
    /// et pour que les valeurs ne puissent pas diverger de la liste.
    pub fn apply_legacy_referer_headers(&mut self) {
        for (key, value) in VercelPlugin::legacy_headers() {
            self.request.set_header(key, value);
        }
    }
}

/// Brouillon de catalogue, version reduite de `Catalog.Draft`.
///
/// Seule la partie `provider` est portee, avec les deux methodes utilisees par
/// la transformation. Le vrai brouillon expose aussi `model`, et son `update`
/// cree le fournisseur s'il est absent : ici `update` renvoie `false` et ne
/// fait rien. La difference est inobservable depuis ce fichier, qui n'appelle
/// `update` que sur des identifiants venus de `list()`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct VercelCatalogDraft {
    /// Fournisseurs du brouillon, dans leur ordre d'insertion, comme la Map
    /// JavaScript. L'ordre est preserve et jamais trie.
    pub providers: Vec<VercelProvider>,
}

impl VercelCatalogDraft {
    /// Brouillon a partir d'une liste de fournisseurs.
    pub fn new(providers: Vec<VercelProvider>) -> Self {
        VercelCatalogDraft { providers }
    }

    /// `evt.provider.list()` : tous les fournisseurs, dans l'ordre.
    pub fn list(&self) -> &[VercelProvider] {
        &self.providers
    }

    /// `evt.provider.get(providerID)` : un fournisseur, ou `None`.
    pub fn get(&self, providerID: &str) -> Option<&VercelProvider> {
        self.providers.iter().find(|provider| provider.id == providerID)
    }

    /// `evt.provider.update(providerID, fn)` : applique `update` au
    /// fournisseur nomme, et renvoie `true` s'il existait.
    pub fn update<F>(&mut self, providerID: &str, update: F) -> bool
    where
        F: FnOnce(&mut VercelProvider),
    {
        match self.providers.iter_mut().find(|provider| provider.id == providerID) {
            Some(provider) => {
                update(provider);
                true
            }
            None => false,
        }
    }
}

/// Evenement recu par le crochet `aisdk.sdk`.
///
/// En TS : `{ readonly model, readonly package, readonly options, sdk? }`. Le
/// champ `model` n'est pas lu par le plugin vercel, et `sdk` est un `any`
/// ecrit par le crochet lui-meme : ni l'un ni l'autre n'est declare ici.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct VercelSdkEvent {
    /// Paquet npm demande. Compare avec `!==` en TS : c'est une inegalite
    /// stricte, pas un test de veracite, donc une chaine vide est simplement
    /// un paquet qui ne correspond pas.
    pub package: String,
    /// Options a passer a la fabrique, dictionnaire libre.
    pub options: BTreeMap<String, Value>,
}

impl VercelSdkEvent {
    /// Evenement sans option.
    pub fn new(package: impl Into<String>) -> Self {
        VercelSdkEvent { package: package.into(), options: BTreeMap::new() }
    }
}

/// Ce que le plugin decide de faire d'un evenement sdk.
///
/// Le TS ecrit dans `evt.sdk` sans condition des que le paquet correspond. On
/// rend donc la decision plutot que l'effet : l'appel a la fabrique, lui, est
/// l'equivalent de l'import dynamique du paquet npm, qui n'a pas de
/// representation Rust. Ce type n'a pas de version JSON, il n'en derive donc
/// pas.
#[derive(Debug, Clone, PartialEq)]
pub enum VercelSdkAction {
    /// Paquet different : le crochet rend la main sans rien changer, comme le
    /// `return` du TypeScript.
    Ignore,
    /// Paquet reconnu : la fabrique designee doit etre appelee avec ces
    /// options, et son resultat.range dans `evt.sdk`.
    Create { factory: &'static str, options: BTreeMap<String, Value> },
}

// ---------------------------------------------------------------------------
// Le plugin
// ---------------------------------------------------------------------------

/// Portage du `VercelPlugin` exporte par la source.
///
/// En TS, `define({ id, effect })` renvoie l'objet tel quel : `define` ne fait
/// rien d'autre que fixer le type. Le plugin n'a donc aucun etat et aucune
/// donnee propre, seulement un identifiant et deux regles, qui sont portees ici
/// comme fonctions pures. L'enregistrement effectif des crochets revient a
/// l'hote de plugins, comme en TS.
pub struct VercelPlugin;

impl VercelPlugin {
    /// L'identifiant enregistre par l'hote, `"vercel"`.
    pub const ID: &'static str = PLUGIN_ID;

    /// Les deux en-tetes ecrits par la transformation, dans l'ordre du TS.
    pub fn legacy_headers() -> Vec<(&'static str, &'static str)> {
        vec![(HTTP_REFERER_KEY, HTTP_REFERER_VALUE), (X_TITLE_KEY, X_TITLE_VALUE)]
    }

    /// Regle de la transformation du catalogue.
    ///
    /// Le TS fait `for (const item of evt.provider.list())`, saute tout
    /// fournisseur dont l'api n'est pas `aisdk` ou dont le paquet n'est pas
    /// `@ai-sdk/vercel`, puis appelle `update` sur les autres pour y ecrire
    /// les deux en-tetes. Cette fonction fait exactement cela, et renvoie les
    /// identifiants touches, dans l'ordre du catalogue.
    ///
    /// La liste est parcourue avant toute ecriture, comme en TS : le `for` y
    /// iterate sur le resultat de `list()`, pas sur le catalogue modifie.
    pub fn transform(draft: &mut VercelCatalogDraft) -> Vec<String> {
        let touches: Vec<String> = draft
            .list()
            .iter()
            .filter(|provider| provider.api.is_vercel_aisdk())
            .map(|provider| provider.id.clone())
            .collect();
        for providerID in &touches {
            draft.update(providerID, |provider| provider.apply_legacy_referer_headers());
        }
        touches
    }

    /// Regle du crochet `aisdk.sdk`.
    ///
    /// Le TS ecrit `if (evt.package !== "@ai-sdk/vercel") return`, puis charge
    /// le paquet et appelle `createVercel(evt.options)`. On s'arrete avant
    /// l'appel, qui est l'equivalent de l'import dynamique : la decision et
    /// les options sont rendues intactes.
    pub fn sdk_action(event: &VercelSdkEvent) -> VercelSdkAction {
        if event.package.as_str() != AISDK_PACKAGE {
            return VercelSdkAction::Ignore;
        }
        VercelSdkAction::Create { factory: AISDK_FACTORY, options: event.options.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn fournisseur_vercel() -> VercelProvider {
        VercelProvider::new("vercel", VercelApi::aisdk(AISDK_PACKAGE))
    }

    // ------------------------------------------------- transformation catalogue

    #[test]
    fn un_fournisseur_aisdk_vercel_recoit_les_deux_entetes_heritage() {
        let mut draft = VercelCatalogDraft::new(vec![fournisseur_vercel()]);

        let touches = VercelPlugin::transform(&mut draft);

        assert_eq!(touches, vec!["vercel".to_string()]);
        let request = &draft.get("vercel").unwrap().request;
        assert_eq!(request.header("http-referer"), Some("https://opencode.ai/"));
        assert_eq!(request.header("x-title"), Some("opencode"));
        assert_eq!(request.headers.len(), 2);
    }

    #[test]
    fn les_entetes_ne_prennent_pas_la_forme_majuscule() {
        // Le test TypeScript verifie exactement cela : le plugin ecrit des
        // noms en minuscules, et surtout pas `HTTP-Referer` ni `X-Title`.
        let mut draft = VercelCatalogDraft::new(vec![fournisseur_vercel()]);

        VercelPlugin::transform(&mut draft);

        let request = &draft.get("vercel").unwrap().request;
        assert!(request.header("HTTP-Referer").is_none(), "HTTP-Referer ne doit pas etre ecrit");
        assert!(request.header("X-Title").is_none(), "X-Title ne doit pas etre ecrit");
    }

    #[test]
    fn les_entetes_deja_presentes_sont_conservees() {
        let mut provider = fournisseur_vercel();
        provider.request.set_header("Existing", "1");
        let mut draft = VercelCatalogDraft::new(vec![provider]);

        VercelPlugin::transform(&mut draft);

        let request = &draft.get("vercel").unwrap().request;
        assert_eq!(request.header("Existing"), Some("1"));
        assert_eq!(request.headers.len(), 3);
    }

    #[test]
    fn une_entete_du_plugin_deja_presente_est_ecrasee() {
        // Le TS ecrit `headers[key] = value` sans tester la presence, donc il
        // ecrase. Une valeur choisie par l'utilisateur disparait.
        let mut provider = fournisseur_vercel();
        provider.request.set_header("http-referer", "https://autre.example/");
        provider.request.set_header("x-title", "autre");
        let mut draft = VercelCatalogDraft::new(vec![provider]);

        VercelPlugin::transform(&mut draft);

        let request = &draft.get("vercel").unwrap().request;
        assert_eq!(request.header("http-referer"), Some("https://opencode.ai/"));
        assert_eq!(request.header("x-title"), Some("opencode"));
    }

    #[test]
    fn un_fournisseur_qui_n_est_pas_vercel_garde_ses_entetes_vides() {
        let mut draft = VercelCatalogDraft::new(vec![
            VercelProvider::new("gateway", VercelApi::native()),
            VercelProvider::new("cohere", VercelApi::aisdk("@ai-sdk/cohere")),
            VercelProvider::new("anthropic", VercelApi::aisdk("@ai-sdk/anthropic")),
        ]);

        let touches = VercelPlugin::transform(&mut draft);

        assert!(touches.is_empty());
        for providerID in ["gateway", "cohere", "anthropic"] {
            assert!(
                draft.get(providerID).unwrap().request.headers.is_empty(),
                "{} ne doit pas etre touche",
                providerID
            );
        }
    }

    #[test]
    fn un_catalogue_vide_ne_produit_aucun_changement() {
        let mut draft = VercelCatalogDraft::new(Vec::new());

        let touches = VercelPlugin::transform(&mut draft);

        assert!(touches.is_empty());
        assert!(draft.list().is_empty());
    }

    #[test]
    fn un_catalogue_avec_un_seul_fournisseur_non_vercel_ne_produit_rien() {
        let mut draft = VercelCatalogDraft::new(vec![VercelProvider::new("gateway", VercelApi::native())]);

        let touches = VercelPlugin::transform(&mut draft);

        assert!(touches.is_empty());
        assert_eq!(draft.list().len(), 1);
    }

    #[test]
    fn les_fournisseurs_sont_traites_dans_l_order_du_catalogue() {
        let mut draft = VercelCatalogDraft::new(vec![
            VercelProvider::new("zeta", VercelApi::aisdk(AISDK_PACKAGE)),
            VercelProvider::new("alpha", VercelApi::aisdk(AISDK_PACKAGE)),
            VercelProvider::new("milieu", VercelApi::native()),
        ]);

        let touches = VercelPlugin::transform(&mut draft);

        assert_eq!(touches, vec!["zeta".to_string(), "alpha".to_string()]);
        let ids: Vec<&str> = draft.list().iter().map(|provider| provider.id.as_str()).collect();
        assert_eq!(ids, vec!["zeta", "alpha", "milieu"], "le catalogue ne doit pas etre trie");
    }

    #[test]
    fn seul_le_paquet_vercel_exact_est_reconnu() {
        assert!(VercelApi::aisdk("@ai-sdk/vercel").is_vercel_aisdk());
        assert!(!VercelApi::aisdk("@ai-sdk/vercel-next").is_vercel_aisdk());
        assert!(!VercelApi::aisdk("@ai-sdk/Vercel").is_vercel_aisdk(), "la casse compte");
        assert!(!VercelApi::aisdk("vercel").is_vercel_aisdk());
        assert!(!VercelApi::aisdk("").is_vercel_aisdk());
        assert!(!VercelApi::native().is_vercel_aisdk(), "une api native n'a pas de paquet");
    }

    // ----------------------------------------------------------------- sdk

    #[test]
    fn le_crochet_sdk_choisit_create_vercel_pour_le_paquet_vercel() {
        let mut event = VercelSdkEvent::new(AISDK_PACKAGE);
        event.options.insert("name".to_string(), Value::String("custom-vercel".to_string()));

        match VercelPlugin::sdk_action(&event) {
            VercelSdkAction::Create { factory, options } => {
                assert_eq!(factory, "createVercel");
                assert_eq!(options.get("name"), Some(&Value::String("custom-vercel".to_string())));
                assert_eq!(options.len(), 1, "les options sont passees telles quelles");
            }
            autre => panic!("le paquet vercel doit demander une creation, pas {:?}", autre),
        }
    }

    #[test]
    fn le_crochet_sdk_ignore_un_paquet_qui_n_est_pas_vercel() {
        let event = VercelSdkEvent::new("@ai-sdk/openai");

        assert_eq!(VercelPlugin::sdk_action(&event), VercelSdkAction::Ignore);
    }

    #[test]
    fn un_paquet_vide_est_ignore_par_le_crochet_sdk() {
        // Le TS compare avec `!==`, qui teste l'egalite stricte et non la
        // veracite. Une chaine vide n'est donc pas un paquet valide, elle est
        // simplement un paquet qui ne correspond pas.
        let event = VercelSdkEvent::new("");

        assert_eq!(VercelPlugin::sdk_action(&event), VercelSdkAction::Ignore);
    }

    #[test]
    fn des_options_vides_donnent_un_dictionnaire_vide() {
        let event = VercelSdkEvent::new(AISDK_PACKAGE);

        match VercelPlugin::sdk_action(&event) {
            VercelSdkAction::Create { options, .. } => assert!(options.is_empty()),
            autre => panic!("le paquet vercel doit demander une creation, pas {:?}", autre),
        }
    }

    // ------------------------------------------------------------ noms de champs

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        let json = serde_json::to_value(fournisseur_vercel()).unwrap();

        assert_eq!(json["id"], "vercel");
        assert_eq!(json["api"]["type"], "aisdk");
        assert_eq!(json["api"]["package"], "@ai-sdk/vercel");
        assert_eq!(json["request"]["headers"], json!({}));
        assert_eq!(json["request"]["body"], json!({}));
        assert!(json.get("Id").is_none(), "la casse ne doit pas changer");
        assert!(json.get("Api").is_none());
        assert!(json.get("Request").is_none());
        assert!(json.get("Name").is_none());
        assert!(json["api"].get("Type").is_none(), "le champ discriminant s'appelle type");
        assert!(json["api"].get("Package").is_none());
        assert!(json["api"].get("Url").is_none());
        assert!(json["api"].get("Settings").is_none());
        assert!(json["request"].get("Headers").is_none());
        assert!(json["request"].get("Body").is_none());
    }

    #[test]
    fn un_fournisseur_natif_ne_serialize_ni_package_ni_url() {
        let provider = VercelProvider::new("gateway", VercelApi::native());

        let json = serde_json::to_value(provider).unwrap();

        assert_eq!(json["api"], json!({ "type": "native", "settings": {} }));
        assert!(json["api"].get("package").is_none(), "une api native n'a pas de paquet");
        assert!(json["api"].get("url").is_none());
    }

    #[test]
    fn un_fournisseur_se_lit_depuis_le_json_du_catalogue() {
        let json = json!({
            "id": "vercel",
            "name": "Vercel",
            "disabled": false,
            "integrationID": "int-1",
            "api": { "type": "aisdk", "package": "@ai-sdk/vercel", "url": "https://ai.vercel.sh" },
            "request": { "headers": { "Existing": "1" }, "body": { "apiKey": "k" } }
        });

        let provider: VercelProvider = serde_json::from_value(json).unwrap();

        assert_eq!(provider.id, "vercel");
        assert_eq!(provider.api.type_name(), "aisdk");
        assert_eq!(provider.api.package(), Some("@ai-sdk/vercel"));
        assert_eq!(provider.request.header("Existing"), Some("1"));
        assert_eq!(provider.request.body.get("apiKey"), Some(&Value::String("k".to_string())));
    }

    #[test]
    fn un_api_avec_url_et_reglages_survit_a_l_aller_retour() {
        let json = json!({
            "type": "aisdk",
            "package": "@ai-sdk/vercel",
            "url": "https://ai.vercel.sh",
            "settings": { "region": "iad" }
        });

        let api: VercelApi = serde_json::from_value(json.clone()).unwrap();

        assert_eq!(serde_json::to_value(api).unwrap(), json);
    }

    #[test]
    fn un_type_d_api_inconnu_est_refuse() {
        // Le TS declare une union de deux formes, pas une chaine libre : une
        // troisieme valeur de `type` n'existe pas et ne doit pas etre inventee.
        assert!(serde_json::from_value::<VercelApi>(json!({ "type": "graphql" })).is_err());
        assert!(serde_json::from_value::<VercelApi>(json!({ "package": "@ai-sdk/vercel" })).is_err());
    }

    #[test]
    fn le_brouillon_du_catalogue_serialize_sous_la_cle_providers() {
        let draft = VercelCatalogDraft::new(vec![fournisseur_vercel()]);

        let json = serde_json::to_value(draft).unwrap();

        assert_eq!(json["providers"][0]["id"], "vercel");
        assert!(json.get("provider").is_none(), "la cle se lit au pluriel");
    }

    #[test]
    fn les_noms_de_champs_de_l_evenement_sdk_sont_ceux_du_typescript() {
        let json = json!({ "package": "@ai-sdk/vercel", "options": { "name": "custom-vercel" } });

        let event: VercelSdkEvent = serde_json::from_value(json.clone()).unwrap();

        assert_eq!(serde_json::to_value(&event).unwrap(), json);
        assert!(json.get("pkg").is_none(), "le champ s'appelle package");
        assert!(json.get("Options").is_none());
    }

    #[test]
    fn un_evenement_sdk_sans_paquet_est_refuse() {
        // `package` est obligatoire en TS, donc obligatoire ici aussi.
        assert!(serde_json::from_value::<VercelSdkEvent>(json!({ "options": {} })).is_err());
    }

    #[test]
    fn un_identifiant_inconnu_ne_cree_pas_de_fournisseur() {
        let mut draft = VercelCatalogDraft::new(vec![VercelProvider::new("inconnu", VercelApi::native())]);

        let touches = VercelPlugin::transform(&mut draft);

        assert!(touches.is_empty());
        assert!(draft.get("inconnu").unwrap().request.headers.is_empty());
        // `update` sur un identifiant absent ne cree rien, contrairement au
        // vrai catalogue. Le plugin n'appelle jamais `update` ainsi.
        assert!(!draft.update("absent", |provider| provider.apply_legacy_referer_headers()));
        assert_eq!(draft.list().len(), 1);
    }

    #[test]
    fn les_deux_entetes_du_plugin_sont_bien_les_deux_du_typescript() {
        assert_eq!(
            VercelPlugin::legacy_headers(),
            vec![("http-referer", "https://opencode.ai/"), ("x-title", "opencode")]
        );
        assert_eq!(VercelPlugin::ID, "vercel");
    }
}