//! Portage Rust de `opencode/packages/core/src/plugin/provider/nvidia.ts`.
//!
//! Vingt-deux lignes de TypeScript, dont l'essentiel est une boucle. Le plugin
//! NVIDIA ne declare aucun fournisseur : il ne fait qu'ajouter trois en-tetes
//! HTTP a une seule famille de fournisseurs deja presents dans le catalogue.
//!
//! ## Ce que dit la source
//!
//! Le plugin s'identifie par `id: "nvidia"`. Son unique effet enregistre une
//! transformation du catalogue. A chaque passage de cette transformation, le
//! code parcourt la liste des fournisseurs et retient uniquement ceux qui
//! satisfont les trois conditions suivantes, dans cet ordre :
//!
//! - `item.provider.api.type === "aisdk"`
//! - `item.provider.api.package === "@ai-sdk/openai-compatible"`
//! - `item.provider.api.url === "https://integrate.api.nvidia.com/v1"`
//!
//! Chaque fournisseur retenu recoit alors trois en-tetes dans
//! `provider.request.headers` :
//!
//! - `HTTP-Referer` vaut `"https://opencode.ai/"`, ecriture directe ;
//! - `X-Title` vaut `"opencode"`, ecriture directe ;
//! - `X-BILLING-INVOKE-ORIGIN` vaut `"OpenCode"`, mais **seulement si la cle
//!   est absente**, car le TypeScript ecrit `??=` et non `=`. C'est le point
//!   subtil du fichier, il est traite plus bas.
//!
//! Ces trois en-tetes sont exiges par le service NVIDIA : les deux premiers
//! identifient l'appelant, le troisieme indique l'origine de la facturation.
//!
//! ## Choix de portage
//!
//! - `define({ id, effect })` devient un `struct NvidiaPlugin` qui porte le
//!   `id`, plus une fonction `transform`. `Effect.fn` devient une fonction Rust
//!   pure, sans `async` : rien ici n'attend d'EIO, la seule dependance du
//!   plugin (`ctx.catalog`) disparait une fois le "draft" resolu en liste.
//! - Les trois conditions du TypeScript deviennent trois methodes sur `Api`
//!   (`is_aisdk`, `is_package`, `is_url`) appelees par `correspond`, dans le
//!   meme ordre et avec la meme semantique. Une comparaison de chaine Rust est
//!   une comparaison de chaine JavaScript : pas de test de veracite, donc une
//!   chaine vide ne se fait pas passer pour une absence.
//! - `evt.provider.update(id, fn)` est traduit par une mutation sur place.
//!   Comme l'identifiant vient de la liste parcourue, l'enregistrement existe
//!   deja et le `update` ne peut pas le creer : la mutation directe est
//!   equivalente, sans avoir a porter le catalogue.
//! - `headers["X"] ??= "OpenCode"` est un `entry().or_insert_with()`. C'est la
//!   seule ecriture conditionnelle du fichier et elle se comporte a
//!   l'inverse d'un ternaire : une valeur deja presente est conservee, y
//!   compris si elle est vide. Une chaine vide n'est pas falsy en JavaScript,
//!   donc `??=` ne l'ecrase pas. C'est le point que la relecture doit verifier
//!   en premier.
//! - Les en-tetes sont un `BTreeMap<String, String>` : `Schema.Record(Schema.
//!   String, Schema.String)` dans le TS. Un objet JavaScript garde l'ordre
//!   d'insertion, la `BTreeMap` donne un ordre deterministe, ce qui est le
//!   choix retenu partout dans ce portage.
//! - `api` est une union **taggee** par `type` dans le TS
//!   (`Schema.toTaggedUnion("type")`), donc un `enum` avec `#[serde(tag =
//!   "type")]` et un `rename` par variante.
//!
//! ## Portee volontairement restreinte des structures
//!
//! `Provider`, `Api` et `Request` ci-dessous ne sont **pas** les schemas
//! complets du TypeScript. Ils ne modelent que ce que ce plugin lit ou ecrit :
//! `provider.id`, `provider.api.{type, package, url}` et
//! `provider.request.headers`. Les champs `name`, `integrationID`, `disabled`,
//! `env`, `models`, `api.settings` et `request.body` existent bien dans
//! `ProviderV2.Info`, mais ce plugin ne les touche jamais ; les porter serait
//! inventer du comportement. Consequence a assumer : un aller-retour JSON sur
//! ces structures perd les champs non modeles, et le fichier ne pretend pas
//! etre la source de verite du catalogue.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// En-tetes HTTP d'un fournisseur : cle en chaine, valeur en chaine.
///
/// Le TS ecrit `Schema.Record(Schema.String, Schema.String)`, donc les deux
/// moities sont des chaines. Pas de valeur nulle possible cote TS non plus,
/// ce qui rend l'equivalent de `??=` exact (voir `apply_headers`).
pub type Headers = BTreeMap<String, String>;

/// Identifiant du plugin, tel qu'il apparait dans `define({ id })`.
pub const PLUGIN_ID: &str = "nvidia";

/// Paquet AI SDK attendu par ce plugin, compare a `api.package`.
pub const API_PACKAGE: &str = "@ai-sdk/openai-compatible";

/// URL attendue par ce plugin, comparee a `api.url`.
pub const API_URL: &str = "https://integrate.api.nvidia.com/v1";

/// Nom du premier en-tete ecrit. La casse est celle du service NVIDIA et ne
/// doit pas etre "normalisee" en `Http-Referer`.
pub const HEADER_REFERER: &str = "HTTP-Referer";

/// Nom du deuxieme en-tete ecrit.
pub const HEADER_TITLE: &str = "X-Title";

/// Nom du troisieme en-tete ecrit, celui de la seule ecriture conditionnelle.
pub const HEADER_BILLING_ORIGIN: &str = "X-BILLING-INVOKE-ORIGIN";

/// Valeur de `HEADER_REFERER`.
pub const REFERER_VALUE: &str = "https://opencode.ai/";

/// Valeur de `HEADER_TITLE`.
pub const TITLE_VALUE: &str = "opencode";

/// Valeur par defaut de `HEADER_BILLING_ORIGIN`, appliquee seulement si la
/// cle est absente.
pub const BILLING_ORIGIN_VALUE: &str = "OpenCode";

/// Partie `request` du fournisseur, restreinte a ce que le plugin ecrit.
///
/// En TS : `Provider.Request`, dont seuls les `headers` sont utilises ici.
/// `body` existe dans le schema et n'est jamais touche par ce plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// En-tetes de la requete vers le fournisseur.
    pub headers: Headers,
}

impl Request {
    /// Requete sans aucun en-tete, l'etat de depart d'un fournisseur neuf.
    pub fn new() -> Self {
        Self { headers: Headers::new() }
    }
}

/// Partie `api` du fournisseur.
///
/// En TS : `Provider.Api = Schema.Union([AISDK, Native]).pipe(Schema.
/// toTaggedUnion("type"))`. L'union est donc **taggee** par la chaine `type`,
/// ce que `#[serde(tag = "type")]` reproduit exactement. Le tag est aussi le
/// premier des trois filtres du plugin.
///
/// Chaque variante ne garde que `type`, `package` et `url`, parce que ce sont
/// les seuls champs lus ici. `settings`, present dans les deux variantes du
/// schema, n'est pas modele : il ne sert a rien pour ce plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Api {
    /// Variante `Provider.AISDK`, la seule que ce plugin retienne.
    #[serde(rename = "aisdk")]
    Aisdk {
        /// Paquet AI SDK, compare a `API_PACKAGE`. Obligatoire dans le schema.
        package: String,
        /// URL de base, comparee a `API_URL`. Absente possible.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
    /// Variante `Provider.Native`, toujours ecartee par le premier filtre.
    #[serde(rename = "native")]
    Native {
        /// URL de base, presente dans le schema mais jamais comparee ici, la
        /// variante etant deja eliminee par `is_aisdk`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
}

impl Api {
    /// Premier filtre du TypeScript : `api.type !== "aisdk"`.
    pub fn is_aisdk(&self) -> bool {
        matches!(self, Api::Aisdk { .. })
    }

    /// Deuxieme filtre : `api.package !== "@ai-sdk/openai-compatible"`.
    /// La variante native n'a pas de `package`, elle ne peut donc pas
    /// correspondre, ce qui est le comportement du TypeScript.
    pub fn is_package(&self, package: &str) -> bool {
        match self {
            Api::Aisdk { package: value, .. } => value.as_str() == package,
            Api::Native { .. } => false,
        }
    }

    /// Troisieme filtre : `api.url !== "https://integrate.api.nvidia.com/v1"`.
    /// Une URL absente ne correspond pas, une URL vide ne correspond pas
    /// non plus : la comparaison est exacte des deux cotes, sans test de
    /// veracite.
    pub fn is_url(&self, url: &str) -> bool {
        match self {
            Api::Aisdk { url: Some(value), .. } => value.as_str() == url,
            Api::Native { url: Some(value), .. } => value.as_str() == url,
            _ => false,
        }
    }
}

/// Fournisseur, restreint aux champs utilises par ce plugin.
///
/// En TS : `ProviderV2.MutableInfo`, dont ce plugin lit `id`, `api` et
/// `request`, et n'ecrit que `request.headers`. Voir la note de portee en tete
/// de fichier pour les champs volontairement absents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provider {
    /// Identifiant du fournisseur, lu pour savoir quel enregistrement
    /// mettre a jour.
    pub id: String,
    /// Description de l'API, source des trois filtres.
    pub api: Api,
    /// Partie de la requete que le plugin complete.
    pub request: Request,
}

/// Le plugin NVIDIA.
///
/// En TS : `export const NvidiaPlugin = define({ id: "nvidia", effect })`.
/// `define` ne fait rien de plus que renvoyer l'objet qu'il recoit, donc un
/// `struct` suffit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvidiaPlugin {
    /// Identifiant du plugin, `"nvidia"`.
    pub id: String,
}

impl NvidiaPlugin {
    /// Plugin declare, avec son identifiant.
    pub fn new() -> Self {
        Self { id: PLUGIN_ID.to_string() }
    }

    /// L'effet du plugin, une fois `ctx.catalog` resolu.
    ///
    /// Le TypeScript enregistre cette fonction aupres du catalogue ; ici elle
    /// est appelee directement sur une liste de fournisseurs. Chaque
    /// fournisseur correspondant recoit les trois en-tetes NVIDIA, les autres
    /// restent intacts.
    pub fn transform(providers: &mut [Provider]) {
        for provider in providers.iter_mut() {
            if !correspond(provider) {
                continue;
            }
            apply_headers(&mut provider.request.headers);
        }
    }
}

impl Default for NvidiaPlugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Les trois filtres du TypeScript, dans leur ordre d'ecriture.
///
/// Un `continue` par condition non satisfaite, donc un `&&` de trois
/// comparaisons. Aucune des trois ne teste la veracite : une chaine vide est
/// comparee comme une chaine vide.
pub fn correspond(provider: &Provider) -> bool {
    provider.api.is_aisdk() && provider.api.is_package(API_PACKAGE) && provider.api.is_url(API_URL)
}

/// Ecrit les trois en-tetes NVIDIA dans une table d'en-tetes.
///
/// Les deux premiers sont ecrits directement : ils ecrasent toute valeur
/// precedente. Le troisieme est un `??=` en TypeScript, c'est-a-dire une
/// ecriture qui n'a lieu que si la cle est absente. Une valeur deja presente
/// est conservee, y compris une chaine vide : en JavaScript `""` n'est pas
/// `null` ni `undefined`, donc `??=` ne la remplace pas. Traduire ce `??=` en
/// test de veracite, ou en ecriture inconditionnelle, serait une divergence
/// silencieuse.
pub fn apply_headers(headers: &mut Headers) {
    headers.insert(HEADER_REFERER.to_string(), REFERER_VALUE.to_string());
    headers.insert(HEADER_TITLE.to_string(), TITLE_VALUE.to_string());
    headers
        .entry(HEADER_BILLING_ORIGIN.to_string())
        .or_insert_with(|| BILLING_ORIGIN_VALUE.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Fournisseur qui satisfait les trois filtres, sans aucun en-tete.
    fn fournisseur_nvidia() -> Provider {
        Provider {
            id: "nvidia".to_string(),
            api: Api::Aisdk {
                package: API_PACKAGE.to_string(),
                url: Some(API_URL.to_string()),
            },
            request: Request::new(),
        }
    }

    /// Fournisseur deja dote de la seule cle ecriture conditionnellement.
    fn fournisseur_avec_entete_de_facturation(valeur: &str) -> Provider {
        let mut provider = fournisseur_nvidia();
        provider.request.headers.insert(HEADER_BILLING_ORIGIN.to_string(), valeur.to_string());
        provider
    }

    // --------------------------------------------------- entetes ecrits

    #[test]
    fn un_fournisseur_nvidia_recoit_les_trois_entetes_avec_leur_casse_exacte() {
        let mut providers = [fournisseur_nvidia()];

        NvidiaPlugin::transform(&mut providers);

        let headers = &providers[0].request.headers;
        assert_eq!(headers.len(), 3, "seuls les trois en-tetes du plugin sont ecrits");
        assert_eq!(headers.get(HEADER_REFERER).map(String::as_str), Some(REFERER_VALUE));
        assert_eq!(headers.get(HEADER_TITLE).map(String::as_str), Some(TITLE_VALUE));
        assert_eq!(
            headers.get(HEADER_BILLING_ORIGIN).map(String::as_str),
            Some(BILLING_ORIGIN_VALUE)
        );
        // La casse est imposee par le service NVIDIA, elle ne se normalise pas.
        assert!(headers.get("Http-Referer").is_none());
        assert!(headers.get("x-title").is_none());
        assert!(headers.get("X-Billing-Invoke-Origin").is_none());
    }

    #[test]
    fn un_fournisseur_non_nvidia_ne_recoit_aucun_entete() {
        let mut providers = [Provider {
            id: "groq".to_string(),
            api: Api::Aisdk {
                package: "@ai-sdk/groq".to_string(),
                url: Some("https://api.groq.com/openai/v1".to_string()),
            },
            request: Request::new(),
        }];

        NvidiaPlugin::transform(&mut providers);

        assert!(providers[0].request.headers.is_empty(), "un autre paquet ne doit rien recevoir");
    }

    #[test]
    fn un_fournisseur_native_ne_recoit_aucun_entete_meme_avec_la_bonne_url() {
        // Le premier filtre elimine la variante native avant meme que l'URL
        // soit comparee, donc une URL identique ne rattrape rien.
        let mut providers = [Provider {
            id: "nvidia".to_string(),
            api: Api::Native { url: Some(API_URL.to_string()) },
            request: Request::new(),
        }];

        NvidiaPlugin::transform(&mut providers);

        assert!(providers[0].request.headers.is_empty());
    }

    #[test]
    fn une_url_absente_ou_vide_ne_ressemble_pas_a_l_url_nvidia() {
        // Piege `?` contre `??` : ni `undefined` ni `""` ne valent l'URL
        // attendue, et aucune des deux formes ne doit MATCHER.
        let sans_url = Provider {
            id: "nvidia".to_string(),
            api: Api::Aisdk { package: API_PACKAGE.to_string(), url: None },
            request: Request::new(),
        };
        let url_vide = Provider {
            id: "nvidia".to_string(),
            api: Api::Aisdk { package: API_PACKAGE.to_string(), url: Some(String::new()) },
            request: Request::new(),
        };

        assert!(!correspond(&sans_url));
        assert!(!correspond(&url_vide));

        let mut providers = [sans_url, url_vide];
        NvidiaPlugin::transform(&mut providers);
        assert!(providers[0].request.headers.is_empty());
        assert!(providers[1].request.headers.is_empty());
    }

    // -------------------------------------- ecriture conditionnelle et collisions

    #[test]
    fn un_entete_de_facturation_deja_present_n_est_pas_ecrase() {
        // `??=` ne remplace que l'absence. Une valeur deja la, meme banale,
        // reste en place.
        let mut providers = [fournisseur_avec_entete_de_facturation("UnAutreClient")];

        NvidiaPlugin::transform(&mut providers);

        assert_eq!(providers[0].request.headers.get(HEADER_BILLING_ORIGIN).map(String::as_str), Some("UnAutreClient"));
        // Les deux autres en-tetes, eux, sont bien ecrits.
        assert_eq!(providers[0].request.headers.get(HEADER_REFERER).map(String::as_str), Some(REFERER_VALUE));
    }

    #[test]
    fn un_entete_de_facturation_vide_survit_a_la_transformation() {
        // Point le plus subtil du fichier : en JavaScript une chaine vide est
        // falsy mais n'est ni `null` ni `undefined`, donc `??=` ne la touche
        // pas. Un test de veracite, ou une ecriture inconditionnelle, la
        // detruirait.
        let mut providers = [fournisseur_avec_entete_de_facturation("")];

        NvidiaPlugin::transform(&mut providers);

        assert_eq!(providers[0].request.headers.get(HEADER_BILLING_ORIGIN).map(String::as_str), Some(""));
    }

    #[test]
    fn deux_fournisseurs_correspondant_ecrasent_chacun_leur_propre_copie() {
        // Chaque fournisseur a sa propre table d'en-tetes : l'ecriture de l'un
        // ne doit pas se voir sur l'autre.
        let mut providers = [fournisseur_nvidia(), fournisseur_nvidia()];
        providers[1].id = "nvidia-bis".to_string();

        NvidiaPlugin::transform(&mut providers);

        assert_eq!(providers[0].request.headers.len(), 3);
        assert_eq!(providers[1].request.headers.len(), 3);
    }

    // ------------------------------------------------------------- listes

    #[test]
    fn une_liste_vide_ne_produit_aucun_changement() {
        let mut providers: [Provider; 0] = [];

        NvidiaPlugin::transform(&mut providers);

        assert!(providers.is_empty());
    }

    #[test]
    fn une_liste_inversee_est_parcourue_dans_l_ordre_du_fichier_et_tous_les_correspondants_sont_traites() {
        let mut providers = [fournisseur_nvidia(), fournisseur_nvidia(), fournisseur_nvidia()];
        providers[0].id = "premier".to_string();
        providers[1].id = "deuxieme".to_string();
        providers[2].id = "troisieme".to_string();

        NvidiaPlugin::transform(&mut providers);

        // L'ordre de la liste est celui de la source, et il n'est pas trie.
        for provider in &providers {
            assert_eq!(provider.request.headers.get(HEADER_TITLE).map(String::as_str), Some(TITLE_VALUE));
        }
    }

    // ------------------------------------------- noms de champs et cas limites

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        let provider = fournisseur_nvidia();

        let json = serde_json::to_value(&provider).unwrap();

        assert_eq!(json["id"], "nvidia", "le champ doit s'appeler id");
        assert_eq!(json["api"]["type"], "aisdk", "la variante doit porter le tag type");
        assert_eq!(json["api"]["package"], API_PACKAGE, "le champ doit s'appeler package");
        assert_eq!(json["api"]["url"], API_URL, "le champ doit s'appeler url");
        assert_eq!(json["request"]["headers"], json!({}), "le chemin doit etre request.headers");
        assert!(json.get("providerId").is_none(), "pas de suffixe Id invente");
        assert!(json["api"].get("Package").is_none(), "la casse ne doit pas changer");
        assert!(json.get("Id").is_none(), "la casse ne doit pas changer");
        // `url` absent ne doit pas laisser de cle `null` derriere lui.
        let sans_url = Provider {
            id: "nvidia".to_string(),
            api: Api::Aisdk { package: API_PACKAGE.to_string(), url: None },
            request: Request::new(),
        };
        let json_sans_url = serde_json::to_value(&sans_url).unwrap();
        assert!(json_sans_url["api"].get("url").is_none(), "une URL absente ne serialise pas en null");
    }

    #[test]
    fn un_fournisseur_seriale_puis_relu_retrouve_le_meme_contenu() {
        let provider = fournisseur_nvidia();

        let json = serde_json::to_string(&provider).unwrap();
        let relu: Provider = serde_json::from_str(&json).unwrap();

        assert_eq!(relu, provider);
        // La variante native se relit aussi, avec son propre tag.
        let native = Provider {
            id: "maison".to_string(),
            api: Api::Native { url: None },
            request: Request::new(),
        };
        let json_native = serde_json::to_value(&native).unwrap();
        assert_eq!(json_native["api"]["type"], "native");
        let relu = serde_json::from_value::<Provider>(json_native).unwrap();
        assert_eq!(relu, native);
        assert!(!relu.api.is_aisdk(), "la variante native ne doit jamais passer le premier filtre");
    }

    #[test]
    fn une_valeur_absente_du_json_provoque_une_erreur_et_non_un_valeur_par_defaut_inventee() {
        // `id` et `package` sont obligatoires cote TS, ils le restent ici.
        assert!(serde_json::from_value::<Provider>(json!({ "api": { "type": "native" } })).is_err());
        assert!(serde_json::from_value::<Provider>(json!({ "id": "nvidia", "api": { "type": "aisdk" } })).is_err());
        assert!(serde_json::from_value::<Api>(json!({ "type": "inconnu" })).is_err());
    }
}
