//! Portage Rust de `opencode/packages/core/src/plugin/provider/kilo.ts`.
//!
//! ## Ce que dit la source
//!
//! Le fichier TypeScript fait vingt et une lignes et definit un seul objet :
//! `KiloPlugin`, construit par `define({ id: "kilo", effect })`. `define` est
//! l'identite (`export function define<R>(plugin: Plugin<R>) { return plugin }`),
//! donc il n'y a rien a porter la-dessus.
//!
//! L'effet enregistre un unique transformateur de catalogue. Pour chaque
//! fournisseur du catalogue, il applique trois filtres :
//!
//! - `item.provider.api.type !== "aisdk"` : seuls les fournisseurs **`aisdk`**
//!   sont concernes, les fournisseurs `native` sont ignores ;
//! - `item.provider.api.package !== "@ai-sdk/openai-compatible"` ;
//! - `item.provider.api.url !== "https://api.kilo.ai/api/gateway"`.
//!
//! Un fournisseur qui passe les trois filtres recoit deux entetes de requete :
//! `HTTP-Referer: https://opencode.ai/` et `X-Title: opencode`. Rien d'autre.
//! Il n'y a ni cle, ni jeton, ni chargement de module dans ce fichier, alors
//! que les plugins voisins (`vercel.ts`, `openai.ts`) en font : kilo ne fait
//! qu'identifier le trafic devant la passerelle kilo.ai.
//!
//! ## Choix de portage
//!
//! - Les trois filtres sont traduits par `Api::correspond`, qui reproduit
//!   exactement la conjonction des trois `continue`. Le troisieme filtre est une
//!   **egalite stricte** (`!==`) et non un test de veracite : en JavaScript
//!   `undefined !== "https://..."` est vrai comme `"" !== "https://..."`, donc
//!   une url absente **et** une url vide sont ecartees de la meme facon. Aucun
//!   piege `?` contre `??` ici, et le test le verifie.
//! - `evt.provider.update(id, fn)` cherche l'enregistrement par identifiant et
//!   appelle `fn` dessus. Comme l'identifiant vient de la liste parcourue juste
//!   avant, l'enregistrement existe toujours : le `update` se reduit donc a une
//!   mutation sur place. La fonction `transform` mute un slice de fournisseurs
//!   et renvoie les identifiants effectivement modifies.
//! - L'enregistrement du catalogue (`{ provider, models }`, avec sa `Map` de
//!   modeles) **n'est pas modelise** ici : le plugin n'utilise que son membre
//!   `provider`. Le modele de l'API fournisseur est repris tel quel depuis
//!   `packages/schema/src/provider.ts`, ou le champ `type` est un
//!   `toTaggedUnion("type")` : il devient le tag de l'enum Rust, et non un champ
//!   a part. Les types `Info` complets du catalogue sont portes par le fichier
//!   qui porte le schema fournisseur ; ceux-ci n'en sont que la sous-partie
//!   necessaire au plugin, ce qui evite d'inventer un second catalogue.
//! - Les entetes sont une `BTreeMap<String, String>`, comme le
//!   `Schema.Record(Schema.String, Schema.String)` de la source. On perd
//!   l'ordre d'insertion des cles, on gagne un ordre deterministe.
//! - La casse des **cles d'entete** est un nom de champ au meme titre que les
//!   autres, et c'est le point le plus fragile du fichier : kilo ecrit
//!   `HTTP-Referer` et `X-Title`, alors que le plugin voisin `vercel.ts` ecrit
//!   `http-referer` et `x-title` pour la meme operation. Ce ne sont pas les
//!   memes cles. Un test y tient.
//!
//! ## Ce qui n'est pas porte
//!
//! L'enregistrement du transformateur dans le catalogue
//! (`ctx.catalog.transform(...)`) n'a pas d'equivalent cote Rust ici : le
//! catalogue est un service a part, et le corps de l'effet est une fonction
//! pure, dont le portage tient entierement dans `transform`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Identifiant du plugin, tel qu'enregistre dans la liste `ProviderPlugins`.
pub const KILO_PLUGIN_ID: &str = "kilo";

/// Valeur du champ `api.type` que le plugin reconnait.
pub const API_TYPE_AISDK: &str = "aisdk";

/// Nom du paquet AI SDK attendu par le deuxieme filtre.
pub const KILO_API_PACKAGE: &str = "@ai-sdk/openai-compatible";

/// Url exacte attendue par le troisieme filtre.
pub const KILO_API_URL: &str = "https://api.kilo.ai/api/gateway";

/// Premiere entete ajoutee. La casse compte : c'est `HTTP-Referer`, avec la
/// ligature en majuscules, et non la forme minuscule du plugin vercel.
pub const HEADER_HTTP_REFERER: &str = "HTTP-Referer";

/// Valeur de [`HEADER_HTTP_REFERER`], barre finale comprise.
pub const VALUE_HTTP_REFERER: &str = "https://opencode.ai/";

/// Deuxieme entete ajoutee, avec son trait d'union et sa majuscule initiale.
pub const HEADER_X_TITLE: &str = "X-Title";

/// Valeur de [`HEADER_X_TITLE`].
pub const VALUE_X_TITLE: &str = "opencode";

/// Reglages libres d'une API, `Schema.Record(Schema.String, Schema.Unknown)`.
pub type Settings = BTreeMap<String, Value>;

/// Partie `aisdk` de l'API d'un fournisseur.
///
/// Le champ `type` est porte par le tag de [`Api`] et ne se repete donc pas
/// ici. Le TS declare `url` et `settings` avec `optional`, c'est-a-dire
/// `optionalKey` : la cle peut manquer entierement, ce qui donne un `Option`
/// avec `default` et `skip_serializing_if` plutot qu'un `null` a la
/// serialisation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AisdkApi {
    /// Paquet npm du SDK, `"@ai-sdk/openai-compatible"` chez kilo.
    pub package: String,
    /// Url de la passerelle. Absente pour un `aisdk` qui ne passe pas par une
    /// passerelle, et c'est alors un filtre rate du plugin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Reglages libres passes au SDK.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<Settings>,
}

/// Partie `native` de l'API d'un fournisseur.
///
/// Ici `settings` n'est pas optionnel dans le TS : il est toujours present,
/// meme vide. La difference de cardinalite avec [`AisdkApi`] est voulue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeApi {
    /// Url du fournisseur natif, absente possiblement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Reglages libres, toujours presents.
    pub settings: Settings,
}

/// API d'un fournisseur, discriminated par `type`.
///
/// Le TS ecrit `Schema.Union([AISDK, Native]).pipe(Schema.toTaggedUnion("type"))`.
/// Le tag est donc la valeur du champ `type` lui-meme, et non un champ
/// discriminant ajoute a cote : il disparait des variantes et ne se retrouve
/// qu'un fois, dans le nom de la variante et dans la sortie JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Api {
    /// Fournisseur passant par un paquet AI SDK.
    #[serde(rename = "aisdk")]
    Aisdk(AisdkApi),
    /// Fournisseur appele en direct par opencode.
    #[serde(rename = "native")]
    Native(NativeApi),
}

impl Api {
    /// La partie `aisdk`, ou `None` pour un fournisseur `native`.
    ///
    /// Le plugin n'a besoin que de celle-ci : sa premiere condition est
    /// `api.type === "aisdk"`.
    pub fn aisdk(&self) -> Option<&AisdkApi> {
        match self {
            Api::Aisdk(api) => Some(api),
            Api::Native(_) => None,
        }
    }

    /// Le nom de la variante, c'est-a-dire la valeur du champ `type`.
    pub fn type_name(&self) -> &'static str {
        match self {
            Api::Aisdk(_) => API_TYPE_AISDK,
            Api::Native(_) => "native",
        }
    }

    /// Les trois conditions du plugin reunies en une seule.
    ///
    /// Le troisieme test est une egalite stricte. Une url absente (`None`) et
    /// une url vide (`Some("")`) sont donc ecartees l'une comme l'autre, sans
    /// qu'aucune ne soit remplacee par une valeur par defaut.
    pub fn correspond(&self) -> bool {
        match self {
            Api::Native(_) => false,
            Api::Aisdk(api) => {
                api.package == KILO_API_PACKAGE && api.url.as_deref() == Some(KILO_API_URL)
            }
        }
    }
}

/// Corps d'une requete envoyee au fournisseur.
///
/// `headers` est un `Record(String, String)` et `body` un `Record(String, Json)`
/// dans la source. Le plugin n'ecrit que dans `headers`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// Entetes HTTP, cles et valeurs en chaine. La casse des cles est
    /// significative pour la comparaison avec le TypeScript.
    pub headers: BTreeMap<String, String>,
    /// Corps de la requete, valeurs JSON libres.
    pub body: BTreeMap<String, Value>,
}

impl Request {
    /// Corps vide : aucune entete, aucun champ.
    pub fn empty() -> Self {
        Self { headers: BTreeMap::new(), body: BTreeMap::new() }
    }
}

/// Informations d'un fournisseur, sous la forme que le catalogue manipule.
///
/// Il s'agit d'un `ProviderV2.MutableInfo` : c'est la seule forme que le
/// transformateur voit, puisque `update` passe un `MutableInfo` a la fonction de
/// mutation. L'identifiant y est une chaine, sans marque : la marque
/// `ProviderV2.ID` du TS n'a pas d'equivalent en Rust, comme toutes les autres
/// marques du portage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderInfo {
    /// Identifiant du fournisseur, sert de cle dans le catalogue.
    pub id: String,
    /// Integration rattachee. `integrationID` porte deux majuscules finales en TS,
    /// c'est la raison d'etre du `rename` ci-dessous.
    #[serde(rename = "integrationID", default, skip_serializing_if = "Option::is_none")]
    pub integration_id: Option<String>,
    /// Nom affiche.
    pub name: String,
    /// Fournisseur desactive. Absent vaut `false` pour le catalogue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    /// API du fournisseur.
    pub api: Api,
    /// Corps de requete.
    pub request: Request,
}

impl ProviderInfo {
    /// Fournisseur minimal : identifiant et nom egaux, API native vide, aucune
    /// entete. C'est la forme que le catalogue cree pour un identifiant inconnu,
    /// reprise de `ProviderV2.Info.empty`.
    pub fn empty(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            integration_id: None,
            disabled: None,
            api: Api::Native(NativeApi { url: None, settings: Settings::new() }),
            request: Request::empty(),
        }
    }

    /// Fournisseur `aisdk` portant exactement l'url de la passerelle kilo.
    /// C'est le seul cas que le plugin modifie.
    pub fn kilo_gateway(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            integration_id: None,
            disabled: None,
            api: Api::Aisdk(AisdkApi {
                package: KILO_API_PACKAGE.to_string(),
                url: Some(KILO_API_URL.to_string()),
                settings: None,
            }),
            request: Request::empty(),
        }
    }
}

/// Le plugin kilo, sous la forme du `define` de la source.
///
/// `define` se reduit a l'identite, donc l'objet `{ id, effect }` devient cette
/// structure. L'effet, lui, tient entierement dans [`KiloPlugin::transform`] :
/// c'est une fonction de mutation, pas un effet asynchrone, et il n'y a rien a
/// rendre asynchrone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KiloPlugin {
    /// `"kilo"`, nom sous lequel le plugin est enregistre.
    pub id: String,
}

impl KiloPlugin {
    /// L'identifiant attendu, en constante pour eviter de le retaper.
    pub const ID: &'static str = KILO_PLUGIN_ID;

    /// Le plugin, avec son identifiant.
    pub fn new() -> Self {
        Self { id: KILO_PLUGIN_ID.to_string() }
    }

    /// Enregistre le transformateur de catalogue, puis ne fait rien d'autre.
    ///
    /// L'enregistrement lui-meme (`ctx.catalog.transform`) appartient au service
    /// catalogue, qui n'est pas porte ici. Ce que le plugin apporte, c'est la
    /// fonction qu'on lui passe ; elle est exposee telle quelle.
    pub fn transform(&self, providers: &mut [ProviderInfo]) -> Vec<String> {
        transform(providers)
    }
}

impl Default for KiloPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Ajoute les deux entetes kilo a un fournisseur.
fn ajouter_entetes(provider: &mut ProviderInfo) {
    provider
        .request
        .headers
        .insert(HEADER_HTTP_REFERER.to_string(), VALUE_HTTP_REFERER.to_string());
    provider
        .request
        .headers
        .insert(HEADER_X_TITLE.to_string(), VALUE_X_TITLE.to_string());
}

/// Le corps du transformateur de `kilo.ts`.
///
/// Parcourt les fournisseurs dans l'ordre, ecarte ceux dont l'API ne
/// correspond pas aux trois constantes, ajoute les deux entetes aux autres, et
/// renvoie les identifiants modifies dans l'ordre de parcours.
///
/// Une entete deja presente est **ecrasee**, pas.completee : c'est ce que fait
/// une affectation JavaScript sur un objet, et c'est le seul comportement
/// observable quand la liste contient deux fois le meme fournisseur.
pub fn transform(providers: &mut [ProviderInfo]) -> Vec<String> {
    let mut modifies = Vec::new();
    for provider in providers.iter_mut() {
        if !provider.api.correspond() {
            continue;
        }
        ajouter_entetes(provider);
        modifies.push(provider.id.clone());
    }
    modifies
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ----------------------------------------------------------------- l'identite

    #[test]
    fn le_plugin_s_annonce_comme_kilo() {
        let plugin = KiloPlugin::new();

        assert_eq!(plugin.id, "kilo");
        assert_eq!(KiloPlugin::ID, "kilo");
        assert_eq!(KiloPlugin::default(), plugin);
    }

    // ------------------------------------------------------------- les entetes mises

    #[test]
    fn un_fournisseur_aisdk_de_la_passerelle_kilo_recoit_les_deux_entetes() {
        let mut providers = vec![ProviderInfo::kilo_gateway("kilo")];

        let modifies = transform(&mut providers);

        assert_eq!(modifies, vec!["kilo".to_string()]);
        assert_eq!(providers[0].request.headers.get(HEADER_HTTP_REFERER), Some(&VALUE_HTTP_REFERER.to_string()));
        assert_eq!(providers[0].request.headers.get(HEADER_X_TITLE), Some(&VALUE_X_TITLE.to_string()));
        assert_eq!(providers[0].request.headers.len(), 2, "aucune autre entete ne doit apparaitre");
    }

    #[test]
    fn la_valeur_du_referer_se_termine_par_une_barre() {
        let mut providers = vec![ProviderInfo::kilo_gateway("kilo")];

        transform(&mut providers);

        // `https://opencode.ai/` et non `https://opencode.ai` : la barre finale
        // fait partie de la valeur, elle ne se perd pas.
        assert_eq!(providers[0].request.headers[HEADER_HTTP_REFERER], "https://opencode.ai/");
        assert!(VALUE_HTTP_REFERER.ends_with('/'));
    }

    #[test]
    fn les_cles_d_entete_sont_en_majuscules_et_ce_ne_sont_pas_celles_de_vercel() {
        let mut providers = vec![ProviderInfo::kilo_gateway("kilo")];

        transform(&mut providers);

        let headers = &providers[0].request.headers;
        // Le meme travail est fait par `vercel.ts`, mais avec la casse minuscule.
        // Confondre les deux enverrait des entetes non reconnues par la passerelle.
        assert!(headers.contains_key("HTTP-Referer"), "la cle doit garder sa majuscule initiale");
        assert!(headers.contains_key("X-Title"), "la cle doit garder sa majuscule initiale");
        assert!(!headers.contains_key("http-referer"), "la forme minuscule n'appartient pas a kilo");
        assert!(!headers.contains_key("x-title"), "la forme minuscule n'appartient pas a kilo");
    }

    #[test]
    fn une_entete_deja_presente_est_ecrasee_et_non_completee() {
        let mut providers = vec![ProviderInfo::kilo_gateway("kilo")];
        providers[0].request.headers.insert(HEADER_X_TITLE.to_string(), "autre".to_string());

        transform(&mut providers);

        // Une affectation JavaScript remplace, elle ne concatene pas.
        assert_eq!(providers[0].request.headers[HEADER_X_TITLE], "opencode");
    }

    // ------------------------------------------------------------- les trois filtres

    #[test]
    fn un_fournisseur_natif_est_laisse_intact() {
        let mut providers = vec![ProviderInfo::empty("kilo")];

        let modifies = transform(&mut providers);

        // Meme avec la bonne url, un fournisseur `native` passe le premier filtre
        // uniquement si son type vaut `aisdk` : ici c'est `native`, donc non.
        assert!(modifies.is_empty());
        assert!(providers[0].request.headers.is_empty());
    }

    #[test]
    fn un_autre_paquet_que_openai_compatible_est_laisse_intact() {
        let mut provider = ProviderInfo::kilo_gateway("kilo");
        provider.api = Api::Aisdk(AisdkApi {
            package: "@ai-sdk/vercel".to_string(),
            url: Some(KILO_API_URL.to_string()),
            settings: None,
        });
        let mut providers = vec![provider];

        let modifies = transform(&mut providers);

        assert!(modifies.is_empty(), "le deuxieme filtre doit ecarter les autres paquets");
        assert!(providers[0].request.headers.is_empty());
    }

    #[test]
    fn une_url_absente_ecnote_le_fournisseur_sans_le_remplacer() {
        let mut provider = ProviderInfo::kilo_gateway("kilo");
        provider.api = Api::Aisdk(AisdkApi {
            package: KILO_API_PACKAGE.to_string(),
            url: None,
            settings: None,
        });
        let mut providers = vec![provider];

        let modifies = transform(&mut providers);

        // `undefined !== "https://..."` est vrai en JavaScript : le fournisseur
        // est ecarte, et son url n'est pas remplie par une valeur par defaut.
        assert!(modifies.is_empty());
        assert_eq!(providers[0].api.aisdk().unwrap().url, None);
        assert!(providers[0].request.headers.is_empty());
    }

    #[test]
    fn une_url_vide_ecnote_autant_que_une_url_absente() {
        let mut avec_vide = ProviderInfo::kilo_gateway("kilo");
        avec_vide.api = Api::Aisdk(AisdkApi {
            package: KILO_API_PACKAGE.to_string(),
            url: Some(String::new()),
            settings: None,
        });
        let mut providers = vec![avec_vide];

        let modifies = transform(&mut providers);

        // Piege `?` contre `??` : si le portage avait teste la veracite de
        // l'url, la chaine vide serait tombee dans la branche "pas d'url" et
        // aurait pu etre remplacee. Ici l'egalite stricte ecarte les deux.
        assert!(modifies.is_empty());
        assert_eq!(providers[0].api.aisdk().unwrap().url, Some(String::new()), "la chaine vide survit");
        assert!(providers[0].request.headers.is_empty());
    }

    #[test]
    fn la_bonne_url_avec_une_barre_finale_supplementaire_est_refusee() {
        let mut provider = ProviderInfo::kilo_gateway("kilo");
        provider.api = Api::Aisdk(AisdkApi {
            package: KILO_API_PACKAGE.to_string(),
            url: Some(format!("{}/", KILO_API_URL)),
            settings: None,
        });
        let mut providers = vec![provider];

        assert!(transform(&mut providers).is_empty());
        assert!(providers[0].request.headers.is_empty());
    }

    // ------------------------------------------------------------------- les listes

    #[test]
    fn une_liste_vide_de_fournisseurs_ne_modifie_rien() {
        let mut providers: Vec<ProviderInfo> = Vec::new();

        let modifies = transform(&mut providers);

        assert!(modifies.is_empty());
        assert!(providers.is_empty());
    }

    #[test]
    fn un_seul_fournisseur_donne_une_ligne_de_modifies() {
        let mut providers = vec![ProviderInfo::kilo_gateway("seul")];

        let modifies = transform(&mut providers);

        assert_eq!(modifies.len(), 1);
        assert_eq!(modifies[0], "seul");
    }

    #[test]
    fn les_identifiants_modifies_suivent_l_ordre_de_parcours_sans_tri() {
        let mut providers = vec![
            ProviderInfo::kilo_gateway("gamma"),
            ProviderInfo::empty("entre-deux"),
            ProviderInfo::kilo_gateway("alpha"),
            ProviderInfo::kilo_gateway("beta"),
        ];

        let modifies = transform(&mut providers);

        // Le catalogue est une `Map` dont on parcourt les valeurs dans leur
        // ordre d'insertion. Ni trie, ni deduplication.
        assert_eq!(modifies, vec!["gamma".to_string(), "alpha".to_string(), "beta".to_string()]);
        assert!(providers[1].request.headers.is_empty(), "le fournisseur natif reste intact");
    }

    #[test]
    fn deux_fournisseurs_identiques_sont_comptes_deux_fois() {
        let mut providers = vec![ProviderInfo::kilo_gateway("dups"), ProviderInfo::kilo_gateway("dups")];

        let modifies = transform(&mut providers);

        // On parcourt une liste d'enregistrements, pas une table : le meme
        // identifiant peut apparaitre deux fois et est alors traite deux fois.
        assert_eq!(modifies, vec!["dups".to_string(), "dups".to_string()]);
    }

    // ------------------------------------------------- noms de champs et allers-retours

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        let provider = ProviderInfo::kilo_gateway("kilo");

        let json = serde_json::to_value(&provider).unwrap();

        assert_eq!(json["id"], "kilo");
        assert_eq!(json["name"], "kilo");
        assert_eq!(json["api"]["type"], "aisdk", "le tag doit valoir aisdk, pas Aisdk");
        assert_eq!(json["api"]["package"], "@ai-sdk/openai-compatible");
        assert_eq!(json["api"]["url"], "https://api.kilo.ai/api/gateway");
        assert!(json["request"].get("headers").is_some());
        assert!(json["request"].get("body").is_some());
        // `type` est un tag d'enum, il ne doit pas apparaitre deux fois.
        assert_eq!(json["api"].as_object().unwrap().len(), 3);
    }

    #[test]
    fn integration_id_sort_entete_avec_ses_deux_majuscules_finales() {
        let mut provider = ProviderInfo::kilo_gateway("kilo");
        provider.integration_id = Some("kilo-account".to_string());

        let json = serde_json::to_value(&provider).unwrap();

        // C'est la faute de nom de champ la plus probable du fichier : `ID` en
        // majuscules dans le TS, pas `Id`.
        assert_eq!(json["integrationID"], "kilo-account");
        assert!(json.get("integration_id").is_none());
        assert!(json.get("integrationId").is_none());
    }

    #[test]
    fn integration_id_absent_ne_produit_pas_de_cle_ni_null() {
        let provider = ProviderInfo::kilo_gateway("kilo");

        let json = serde_json::to_value(&provider).unwrap();

        assert!(json.get("integrationID").is_none());
        assert!(json.get("disabled").is_none());
    }

    #[test]
    fn un_fournisseur_lit_depuis_le_json_reconserve_les_memes_noms() {
        let json = json!({
            "id": "kilo",
            "integrationID": "kilo-account",
            "name": "Kilo Code",
            "disabled": true,
            "api": {
                "type": "aisdk",
                "package": "@ai-sdk/openai-compatible",
                "url": "https://api.kilo.ai/api/gateway",
                "settings": { "baseURL": "https://api.kilo.ai/api/gateway" }
            },
            "request": { "headers": { "X-Title": "avant" }, "body": { "apiKey": "secret" } }
        });

        let provider: ProviderInfo = serde_json::from_value(json.clone()).unwrap();

        assert_eq!(provider.id, "kilo");
        assert_eq!(provider.integration_id, Some("kilo-account".to_string()));
        assert_eq!(provider.disabled, Some(true));
        assert_eq!(provider.api.type_name(), "aisdk");
        assert_eq!(provider.request.headers["X-Title"], "avant");
        assert_eq!(serde_json::to_value(&provider).unwrap(), json, "aller-retour sans perte");
    }

    #[test]
    fn un_fournisseur_sans_reglages_et_sans_url_ne_serialise_pas_de_cle_vide() {
        let mut provider = ProviderInfo::kilo_gateway("kilo");
        provider.api = Api::Aisdk(AisdkApi {
            package: KILO_API_PACKAGE.to_string(),
            url: None,
            settings: None,
        });

        let json = serde_json::to_value(&provider).unwrap();

        // `optionalKey` en TS : la cle disparait, elle ne devient pas `null`.
        assert!(json["api"].get("url").is_none());
        assert!(json["api"].get("settings").is_none());
    }

    #[test]
    fn un_api_native_sans_url_garde_bien_ses_reglages_vides() {
        let provider = ProviderInfo::empty("kilo");

        let json = serde_json::to_value(&provider).unwrap();

        // `settings` n'est pas optionnel chez `native` : il doit etre present,
        // meme vide, la difference avec `aisdk` est volontaire dans la source.
        assert_eq!(json["api"]["type"], "native");
        assert_eq!(json["api"]["settings"], json!({}));
        assert!(json["api"].get("url").is_none());
    }

    #[test]
    fn un_champ_inconnu_dans_un_fournisseur_est_ignore_comme_en_typescript() {
        let json = json!({
            "id": "kilo",
            "name": "Kilo Code",
            "inconnu": 42,
            "api": { "type": "aisdk", "package": "@ai-sdk/openai-compatible" },
            "request": { "headers": {}, "body": {} }
        });

        let provider: ProviderInfo = serde_json::from_value(json).unwrap();

        assert_eq!(provider.id, "kilo");
        // Pas d'url : le transformateur l'ecartera.
        assert!(!provider.api.correspond(), "sans url, l'egalite stricte ecarte le fournisseur");
    }

    #[test]
    fn le_plugin_expose_le_meme_transformateur_que_la_fonction_libre() {
        let plugin = KiloPlugin::new();
        let mut par_la_fonction = vec![ProviderInfo::kilo_gateway("kilo")];
        let mut par_le_plugin = vec![ProviderInfo::kilo_gateway("kilo")];

        let a = transform(&mut par_la_fonction);
        let b = plugin.transform(&mut par_le_plugin);

        assert_eq!(a, b);
        assert_eq!(par_la_fonction, par_le_plugin);
    }
}
