//! Portage Rust de `opencode/packages/core/src/plugin/provider/google-vertex.ts`.
//!
//! La source declare deux plugins jumeaux : `google-vertex` (paquets
//! `@ai-sdk/google-vertex` et Vertex via `@ai-sdk/openai-compatible`) et
//! `google-vertex-anthropic` (paquet `@ai-sdk/google-vertex/anthropic`). Chacun
//! fait trois choses : resoudre `project` et `location` (options puis variables
//! d'environnement), reecrire ces valeurs dans le corps de requete du
//! fournisseur, et construire le SDK correspondant.
//!
//! Decomposition de la source :
//!
//! - `resolveProject` / `resolveLocation` : options d'abord, puis une cascade
//!   de variables d'environnement, puis une valeur par defaut (`us-central1`
//!   pour le premier plugin, `global` pour le second). Le second plugin
//!   n'utilise PAS les variables `GOOGLE_VERTEX_*`, sa cascade est plus courte.
//! - `vertexEndpoint` : `global` donne `aiplatform.googleapis.com`, toute autre
//!   region donne `{region}-aiplatform.googleapis.com`.
//! - `replaceVertexVars` : expansion des gabarits `${GOOGLE_VERTEX_PROJECT}`,
//!   `${GOOGLE_VERTEX_LOCATION}` et `${GOOGLE_VERTEX_ENDPOINT}` dans l'URL du
//!   catalogue. Si le projet n'a pas pu etre resolu, le gabarit projet reste
//!   tel quel (le `?? "${GOOGLE_VERTEX_PROJECT}"` de la source).
//! - `authFetch` : injecte un jeton Google dans les en-tetes. C'est du cote
//!   execution (ADC, `google-auth-library`) : le portage enregistre l'intention
//!   dans `AuthFetchCall`, il n'execute pas l'authentification.
//! - Les deux crochets `aisdk.sdk` construisent `createVertex` /
//!   `createVertexAnthropic`. Le premier plugin a une branche speciale : pour un
//!   modele Vertex passe par `@ai-sdk/openai-compatible`, il branche
//!   `authFetch` sur `evt.options.fetch` et s'arrete la. Le second ajoute une
//!   `baseURL` de region continentale pour `eu` et `us`, mais seulement si un
//!   projet est connu et si l'appelant n'a pas deja fourni sa propre `baseURL`.
//!
//! Contrats TypeScript verrouilles par des tests :
//!
//! - **Les cles des options sont des litteraux, pas des noms de champs
//!   structures.** `project`, `location`, `baseURL`, `fetch` vivent dans un
//!   objet libre (`Record<string, any>`). `baseURL` s'ecrit avec un `U`
//!   majuscule et `providerID` avec un `ID` majuscule : ils sont conserves
//!   verbatim, jamais converts en snake_case, car ce sont des cles JSON
//!   existantes et non des champs de structure declares ici.
//! - **Les gabarits de variables** `${GOOGLE_VERTEX_PROJECT}` etc. sont des
//!   chaines exactes du catalogue ; un `replace` approximatif casserait l'URL.
//! - Les constantes de paquets (`@ai-sdk/google-vertex`,
//!   `@ai-sdk/google-vertex/anthropic`, `@ai-sdk/openai-compatible`) sont
//!   comparees en egalite stricte ou par `contains`, selon la source.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Champ `id` du premier objet passe a `define`.
pub const PLUGIN_ID: &str = "google-vertex";

/// Champ `id` du second objet passe a `define`.
pub const PLUGIN_ID_ANTHROPIC: &str = "google-vertex-anthropic";

/// Domaine sur lequel les plugins enregistrent leurs crochets.
pub const HOOK_DOMAIN: &str = "aisdk.sdk";

/// Identifiant du fournisseur Vertex dans `ProviderV2.ID`.
pub const PROVIDER_ID_GOOGLE_VERTEX: &str = "google-vertex";

/// Paquet AI SDK officiel du premier plugin, compare en egalite stricte.
pub const AI_SDK_PACKAGE: &str = "@ai-sdk/google-vertex";

/// Paquet AI SDK du second plugin, compare en egalite stricte.
pub const AI_SDK_PACKAGE_ANTHROPIC: &str = "@ai-sdk/google-vertex/anthropic";

/// Paquet OpenAI-compatible reconnu par `includes`, jamais en egalite stricte.
pub const AI_SDK_PACKAGE_OPENAI_COMPATIBLE: &str = "@ai-sdk/openai-compatible";

/// Fabrique exportee par `@ai-sdk/google-vertex`.
pub const AI_SDK_FACTORY: &str = "createVertex";

/// Fabrique exportee par `@ai-sdk/google-vertex/anthropic`.
pub const AI_SDK_FACTORY_ANTHROPIC: &str = "createVertexAnthropic";

/// Portee OAuth demandee par `authFetch` via `google-auth-library`.
pub const AUTH_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";

/// Region par defaut du premier plugin (`resolveLocation`).
pub const DEFAULT_LOCATION: &str = "us-central1";

/// Region par defaut du second plugin (`resolveLocation` d'Anthropic).
pub const DEFAULT_LOCATION_ANTHROPIC: &str = "global";

/// Endpoint pour la pseudo-region `global`.
pub const GLOBAL_ENDPOINT: &str = "aiplatform.googleapis.com";

/// Gabarit de projet dans les URL du catalogue.
pub const TEMPLATE_PROJECT: &str = "${GOOGLE_VERTEX_PROJECT}";

/// Gabarit de region dans les URL du catalogue.
pub const TEMPLATE_LOCATION: &str = "${GOOGLE_VERTEX_LOCATION}";

/// Gabarit d'endpoint dans les URL du catalogue.
pub const TEMPLATE_ENDPOINT: &str = "${GOOGLE_VERTEX_ENDPOINT}";

/// Nom du champ `fetch` dans les options OpenAI-compatible.
pub const OPTION_KEY_FETCH: &str = "fetch";

/// Nom du champ `baseURL` dans les options. Casse verbatim du TypeScript.
pub const OPTION_KEY_BASE_URL: &str = "baseURL";

/// Type des options libres passees aux fabriques et aux resolutions.
///
/// En TypeScript c'est un `Record<string, any>` ; les cles (`project`,
/// `location`, `baseURL`, `fetch`) restent les litteraux de la source.
pub type Options = BTreeMap<String, Value>;

/// Traduction de `resolveProject(options)` du premier plugin.
///
/// Source : `options.project`, puis `GOOGLE_VERTEX_PROJECT`,
/// `GOOGLE_CLOUD_PROJECT`, `GCP_PROJECT`, `GCLOUD_PROJECT`. `env` est passe par
/// le code appelant (lisant `std::env`) pour rester testable.
pub fn resoudre_projet(options: &Options, env: &BTreeMap<String, String>) -> Option<String> {
    options
        .get("project")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            ["GOOGLE_VERTEX_PROJECT", "GOOGLE_CLOUD_PROJECT", "GCP_PROJECT", "GCLOUD_PROJECT"]
                .iter()
                .find_map(|cle| env.get(*cle).cloned())
        })
}

/// Traduction de `resolveLocation(options)` du premier plugin.
///
/// Source : `options.location` (converti en chaine par `String(...)`), puis
/// `GOOGLE_VERTEX_LOCATION`, `GOOGLE_CLOUD_LOCATION`, `VERTEX_LOCATION`, puis
/// `"us-central1"`.
pub fn resoudre_region(options: &Options, env: &BTreeMap<String, String>) -> String {
    options
        .get("location")
        .map(valeur_en_chaine)
        .or_else(|| {
            ["GOOGLE_VERTEX_LOCATION", "GOOGLE_CLOUD_LOCATION", "VERTEX_LOCATION"]
                .iter()
                .find_map(|cle| env.get(*cle).cloned())
        })
        .unwrap_or_else(|| DEFAULT_LOCATION.to_string())
}

/// Traduction de la cascade de projet du second plugin.
///
/// Source : `options.project`, puis `GOOGLE_CLOUD_PROJECT`, `GCP_PROJECT`,
/// `GCLOUD_PROJECT`. Pas de `GOOGLE_VERTEX_PROJECT` ici.
pub fn resoudre_projet_anthropic(options: &Options, env: &BTreeMap<String, String>) -> Option<String> {
    options
        .get("project")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            ["GOOGLE_CLOUD_PROJECT", "GCP_PROJECT", "GCLOUD_PROJECT"]
                .iter()
                .find_map(|cle| env.get(*cle).cloned())
        })
}

/// Traduction de la cascade de region du second plugin.
///
/// Source : `options.location`, puis `GOOGLE_CLOUD_LOCATION`,
/// `VERTEX_LOCATION`, puis `"global"`. Pas de `GOOGLE_VERTEX_LOCATION` ici.
pub fn resoudre_region_anthropic(options: &Options, env: &BTreeMap<String, String>) -> String {
    options
        .get("location")
        .map(valeur_en_chaine)
        .or_else(|| {
            ["GOOGLE_CLOUD_LOCATION", "VERTEX_LOCATION"].iter().find_map(|cle| env.get(*cle).cloned())
        })
        .unwrap_or_else(|| DEFAULT_LOCATION_ANTHROPIC.to_string())
}

/// Traduction de `String(value)` applique au champ `location`.
///
/// Le TypeScript convertit la valeur en chaine quelle qu'elle soit ; pour une
/// chaine c'est un no-op, pour un nombre c'est sa representation decimale,
/// pour `null`/`undefined` c'est `"null"`/`"undefined"`. Le portage reproduit
/// ces trois cas et delegue le reste a la serialisation JSON.
pub fn valeur_en_chaine(valeur: &Value) -> String {
    match valeur {
        Value::Null => String::from("null"),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        autre => autre.to_string(),
    }
}

/// Traduction de `vertexEndpoint(location)`.
///
/// `"global"` donne le domaine mondial, toute autre region est prefisee.
pub fn endpoint_region(region: &str) -> String {
    if region == "global" {
        GLOBAL_ENDPOINT.to_string()
    } else {
        format!("{region}-{GLOBAL_ENDPOINT}")
    }
}

/// Traduction de `replaceVertexVars(value, project, location)`.
///
/// Les trois gabarits sont remplaces dans l'ordre de la source. Si le projet
/// est inconnu, le gabarit projet reste tel quel, conformement au
/// `?? "${GOOGLE_VERTEX_PROJECT}"` de la source.
pub fn remplacer_variables_vertex(valeur: &str, projet: Option<&str>, region: &str) -> String {
    valeur
        .replace(
            TEMPLATE_PROJECT,
            projet.unwrap_or(TEMPLATE_PROJECT),
        )
        .replace(TEMPLATE_LOCATION, region)
        .replace(TEMPLATE_ENDPOINT, &endpoint_region(region))
}

/// Trace de l'injection d'authentification de `authFetch`.
///
/// En TypeScript, `authFetch` retourne une fonction qui charge
/// `google-auth-library`, obtient un jeton ADC et pose l'en-tete
/// `Authorization: Bearer <token>`. Le portage n'execute pas ADC : il
/// enregistre la portee demandee pour que l'hote puisse realiser l'appel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthFetchCall {
    /// Portee OAuth demandee, ici la seule valeur de la source.
    pub scopes: Vec<String>,
    /// Nom de l'en-tete pose, verbatim du TypeScript.
    pub header: String,
}

impl AuthFetchCall {
    /// Trace correspondant a la closure retournee par `authFetch`.
    pub fn nouveau() -> Self {
        Self {
            scopes: vec![AUTH_SCOPE.to_string()],
            header: String::from("Authorization"),
        }
    }
}

/// Ce qu'un crochet ecrit dans le champ `sdk` de l'evenement.
///
/// Comme pour `provider_google.rs`, la fabrique n'est pas executee : on
/// enregistre quel appel aurait lieu et avec quelles options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkFactoryCall {
    /// Nom de la fabrique appelee, tel qu'il est exporte par le paquet.
    pub factory: String,

    /// Options que la fabrique a recues, apres resolution.
    pub options: Options,
}

/// Evenement recu par le crochet `aisdk.sdk`.
///
/// En TypeScript : la charge utile de `AISDKHooks["sdk"]`, soit
/// `{ model, package, options, sdk? }`. `model` est un `ModelV2Info` non porte
/// par ce lot : le champ reste une valeur JSON opaque. Ses champs internes en
/// camelCase (`providerID`) ne sont donc jamais renommes ici.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkEvent {
    /// Modele demande. Le premier plugin lit `model.providerID` pour sa branche
    /// OpenAI-compatible ; ce module garde le champ tel quel.
    pub model: Value,

    /// Nom du paquet a charger. C'est ce champ que les gardes comparent.
    pub package: String,

    /// Options destinees a la fabrique, recues telles quelles.
    pub options: Options,

    /// Ce qu'un plugin a ecrit dans l'evenement. Absent a l'arrivee.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdk: Option<SdkFactoryCall>,
}

impl SdkEvent {
    /// Construit un evenement d'arrivee : aucun SDK encore construit.
    pub fn new(model: Value, package: impl Into<String>, options: Options) -> Self {
        Self {
            model,
            package: package.into(),
            options,
            sdk: None,
        }
    }
}

/// Resultat du crochet `aisdk.sdk` du premier plugin.
///
/// La source a deux branches exclusives, traduites ligne a ligne :
///
/// ```text
/// if (evt.model.providerID === "google-vertex" && evt.package.includes("@ai-sdk/openai-compatible")) {
///   evt.options.fetch = authFetch(evt.options.fetch)
///   return
/// }
/// if (evt.package !== "@ai-sdk/google-vertex") return
/// evt.sdk = mod.createVertex({ ...options, project, location })
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum SdkHookOutcome {
    /// Branche OpenAI-compatible : l'authentification est branchee sur `fetch`.
    AuthFetch(AuthFetchCall),

    /// Branche officielle : la fabrique `createVertex` doit etre appelee.
    Factory(SdkFactoryCall),

    /// Aucune garde n'a reconnu l'evenement, le crochet n'a rien fait.
    Inerte,
}

/// Gestionnaire enregistre par le plugin `google-vertex` sur `aisdk.sdk`.
///
/// Les options envoyees a la fabrique sont une copie des options de
/// l'evenement, sans le champ `fetch` (la source fait `delete options.fetch`),
/// completees par `project` et `location` resolus — meme `None`, comme dans la
/// source ou `project` vaut alors `undefined`.
pub fn sdk_hook(evt: &mut SdkEvent, env: &BTreeMap<String, String>) -> SdkHookOutcome {
    let branche_openai_compatible = modele_est_google_vertex(&evt.model)
        && evt.package.contains(AI_SDK_PACKAGE_OPENAI_COMPATIBLE);
    if branche_openai_compatible {
        let appel = AuthFetchCall::nouveau();
        evt.options
            .insert(OPTION_KEY_FETCH.to_string(), serde_json::to_value(&appel).unwrap());
        return SdkHookOutcome::AuthFetch(appel);
    }
    if evt.package != AI_SDK_PACKAGE {
        return SdkHookOutcome::Inerte;
    }
    let projet = resoudre_projet(&evt.options, env);
    let region = resoudre_region(&evt.options, env);
    let mut options = evt.options.clone();
    options.remove(OPTION_KEY_FETCH);
    options.insert(
        String::from("project"),
        projet.clone().map(Value::String).unwrap_or(Value::Null),
    );
    options.insert(String::from("location"), Value::String(region));
    let appel = SdkFactoryCall {
        factory: AI_SDK_FACTORY.to_string(),
        options,
    };
    evt.sdk = Some(appel.clone());
    SdkHookOutcome::Factory(appel)
}

/// Traduction de `evt.model.providerID === ProviderV2.ID.googleVertex`.
///
/// `providerID` est la casse verbatim du modele TypeScript ; le champ est
/// cherche dans la valeur JSON opaque sans etre renomme.
fn modele_est_google_vertex(model: &Value) -> bool {
    model.get("providerID").and_then(|v| v.as_str()) == Some(PROVIDER_ID_GOOGLE_VERTEX)
}

/// Resultat du crochet `aisdk.sdk` du second plugin.
#[derive(Debug, Clone, PartialEq)]
pub enum SdkHookAnthropicOutcome {
    /// La fabrique `createVertexAnthropic` doit etre appelee.
    Factory(SdkFactoryCall),

    /// La garde sur le paquet a arrete le gestionnaire.
    Inerte,
}

/// Gestionnaire enregistre par le plugin `google-vertex-anthropic`.
///
/// Traduction de la source : garde stricte sur
/// `@ai-sdk/google-vertex/anthropic`, resolution de `project` et `location`
/// par les cascades Anthropic, puis ajout conditionnel d'une `baseURL` de
/// region continentale. La condition de la source est
/// `(location === "eu" || location === "us") && project && !evt.options.baseURL` :
/// sans projet resolu ou avec une `baseURL` deja fournie, aucune n'est ajoutee.
pub fn sdk_hook_anthropic(evt: &mut SdkEvent, env: &BTreeMap<String, String>) -> SdkHookAnthropicOutcome {
    if evt.package != AI_SDK_PACKAGE_ANTHROPIC {
        return SdkHookAnthropicOutcome::Inerte;
    }
    let projet = resoudre_projet_anthropic(&evt.options, env);
    let region = resoudre_region_anthropic(&evt.options, env);
    let mut options = evt.options.clone();
    options.insert(
        String::from("project"),
        projet.clone().map(Value::String).unwrap_or(Value::Null),
    );
    options.insert(String::from("location"), Value::String(region.clone()));
    let region_continentale = region == "eu" || region == "us";
    if region_continentale {
        if let Some(projet) = projet.as_deref() {
            let a_deja_base_url = evt.options.contains_key(OPTION_KEY_BASE_URL);
            if !a_deja_base_url {
                options.insert(
                    OPTION_KEY_BASE_URL.to_string(),
                    Value::String(url_regional_anthropic(projet, &region)),
                );
            }
        }
    }
    let appel = SdkFactoryCall {
        factory: AI_SDK_FACTORY_ANTHROPIC.to_string(),
        options,
    };
    evt.sdk = Some(appel.clone());
    SdkHookAnthropicOutcome::Factory(appel)
}

/// URL de plateforme regionale construite par le second plugin pour `eu`/`us`.
///
/// Source : `https://aiplatform.{location}.rep.googleapis.com/v1/projects/{project}/locations/{location}/publishers/anthropic/models`.
pub fn url_regional_anthropic(projet: &str, region: &str) -> String {
    format!(
        "https://aiplatform.{region}.rep.googleapis.com/v1/projects/{projet}/locations/{region}/publishers/anthropic/models"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_de_test() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    fn env_avec(cles: &[(&str, &str)]) -> BTreeMap<String, String> {
        cles.iter()
            .map(|(k, v)| (String::from(*k), String::from(*v)))
            .collect()
    }

    /// Modele de test. Sa forme reelle (`ModelV2Info`) n'est pas portee par ce
    /// lot ; on y met `providerID` en majuscules, casse verbatim du TypeScript.
    fn model_de_test() -> Value {
        let mut model = serde_json::Map::new();
        model.insert("id".to_string(), Value::String(String::from("gemini-2.5-pro")));
        model.insert("providerID".to_string(), Value::String(String::from("google-vertex")));
        Value::Object(model)
    }

    /// La cascade de projet du premier plugin respecte l'ordre exact des
    /// variables d'environnement de la source.
    #[test]
    fn la_cascade_de_projet_prefer_options_puis_env_dans_l_ordre_de_la_source() {
        let mut options = Options::new();
        options.insert(String::from("project"), Value::String(String::from("opt-proj")));

        assert_eq!(resoudre_projet(&options, &env_de_test()).as_deref(), Some("opt-proj"));

        options.remove("project");
        let env = env_avec(&[
            ("GOOGLE_VERTEX_PROJECT", "v"),
            ("GOOGLE_CLOUD_PROJECT", "c"),
            ("GCP_PROJECT", "g"),
            ("GCLOUD_PROJECT", "gc"),
        ]);
        assert_eq!(resoudre_projet(&options, &env).as_deref(), Some("v"));

        let env_sans_vertex = env_avec(&[("GOOGLE_CLOUD_PROJECT", "c"), ("GCP_PROJECT", "g")]);
        assert_eq!(resoudre_projet(&options, &env_sans_vertex).as_deref(), Some("c"));

        let env_gcp_seul = env_avec(&[("GCP_PROJECT", "g"), ("GCLOUD_PROJECT", "gc")]);
        assert_eq!(resoudre_projet(&options, &env_gcp_seul).as_deref(), Some("g"));

        let env_gcloud_seul = env_avec(&[("GCLOUD_PROJECT", "gc")]);
        assert_eq!(resoudre_projet(&options, &env_gcloud_seul).as_deref(), Some("gc"));
    }

    /// La cascade de region du premier plugin finit sur `us-central1`, et
    /// `String(options.location)` convertit une valeur non-chaine.
    #[test]
    fn la_cascade_de_region_finit_sur_us_central1_et_convertit_en_chaine() {
        assert_eq!(resoudre_region(&Options::new(), &env_de_test()), "us-central1");

        let env = env_avec(&[("VERTEX_LOCATION", "europe-west1")]);
        assert_eq!(resoudre_region(&Options::new(), &env), "europe-west1");

        let mut options = Options::new();
        options.insert(String::from("location"), Value::Number(serde_json::Number::from(7)));
        assert_eq!(resoudre_region(&options, &env_de_test()), "7");
    }

    /// Les cascades du second plugin sont plus courtes : pas de
    /// `GOOGLE_VERTEX_*`, defaut `"global"`.
    #[test]
    fn les_cascades_anthropic_ignorent_les_variables_google_vertex_et_finit_sur_global() {
        let env = env_avec(&[
            ("GOOGLE_VERTEX_PROJECT", "v"),
            ("GOOGLE_VERTEX_LOCATION", "vloc"),
            ("GOOGLE_CLOUD_PROJECT", "c"),
            ("GOOGLE_CLOUD_LOCATION", "cloc"),
        ]);

        // Le projet Vertex - specifique est ignore par le second plugin.
        assert_eq!(
            resoudre_projet_anthropic(&Options::new(), &env).as_deref(),
            Some("c")
        );
        // La region Vertex - specifique est ignoree, GOOGLE_CLOUD_LOCATION gagne.
        assert_eq!(resoudre_region_anthropic(&Options::new(), &env), "cloc");

        assert_eq!(
            resoudre_projet_anthropic(&Options::new(), &env_de_test()),
            None
        );
        assert_eq!(resoudre_region_anthropic(&Options::new(), &env_de_test()), "global");
    }

    /// `vertexEndpoint` : `global` donne le domaine mondial, sinon le domaine
    /// est prefise par la region.
    #[test]
    fn l_endpoint_depend_de_la_region() {
        assert_eq!(endpoint_region("global"), "aiplatform.googleapis.com");
        assert_eq!(endpoint_region("us-central1"), "us-central1-aiplatform.googleapis.com");
        assert_eq!(endpoint_region("europe-west1"), "europe-west1-aiplatform.googleapis.com");
    }

    /// `replaceVertexVars` : les trois gabarits sont remplaces, et le gabarit
    /// projet survit si le projet est inconnu (le `??` de la source).
    #[test]
    fn les_gabarits_d_url_sont_expanses_et_le_projet_inconnu_survit() {
        let modele = "https://${GOOGLE_VERTEX_ENDPOINT}/v1/projects/${GOOGLE_VERTEX_PROJECT}/locations/${GOOGLE_VERTEX_LOCATION}";

        let url = remplacer_variables_vertex(modele, Some("p"), "europe-west4");
        assert_eq!(
            url,
            "https://europe-west4-aiplatform.googleapis.com/v1/projects/p/locations/europe-west4"
        );

        let url = remplacer_variables_vertex(modele, None, "global");
        assert_eq!(
            url,
            "https://aiplatform.googleapis.com/v1/projects/${GOOGLE_VERTEX_PROJECT}/locations/global"
        );

        // Un gabarit qui n'existe pas n'est pas touche.
        assert_eq!(remplacer_variables_vertex("x${AUTRE}y", Some("p"), "us"), "x${AUTRE}y");
    }

    /// Branche OpenAI-compatible du premier crochet : elle exige a la fois le
    /// `providerID` exact et un paquet qui CONTIENT le paquet compatible ; elle
    /// branche l'authentification et ne construit pas de fabrique.
    #[test]
    fn la_branche_openai_compatible_branche_authfetch_et_s_arrete_la() {
        let mut evt = SdkEvent::new(
            model_de_test(),
            "@ai-sdk/openai-compatible",
            Options::new(),
        );

        match sdk_hook(&mut evt, &env_de_test()) {
            SdkHookOutcome::AuthFetch(appel) => {
                assert_eq!(appel.scopes, vec![AUTH_SCOPE.to_string()]);
                assert_eq!(appel.header, "Authorization");
            }
            autre => panic!("branche inattendue : {autre:?}"),
        }
        assert!(evt.sdk.is_none(), "aucune fabrique ne doit etre construite");
        assert!(evt.options.contains_key(OPTION_KEY_FETCH));

        // Sans le providerID exact, la branche ne s'ouvre pas.
        let mut model = serde_json::Map::new();
        model.insert("providerID".to_string(), Value::String(String::from("google")));
        let mut evt = SdkEvent::new(
            Value::Object(model),
            "@ai-sdk/openai-compatible",
            Options::new(),
        );
        assert!(matches!(sdk_hook(&mut evt, &env_de_test()), SdkHookOutcome::Inerte));

        // `includes` accepte un paquet compose, contrairement a la garde stricte.
        let mut evt = SdkEvent::new(
            model_de_test(),
            "@ai-sdk/openai-compatible/foo",
            Options::new(),
        );
        assert!(matches!(sdk_hook(&mut evt, &env_de_test()), SdkHookOutcome::AuthFetch(_)));
    }

    /// Branche officielle du premier crochet : garde stricte sur le paquet,
    /// champ `fetch` supprime, `project` et `location` ajoutes.
    #[test]
    fn la_branche_officielle_construit_createvertex_sans_le_champ_fetch() {
        let mut options = Options::new();
        options.insert(String::from("project"), Value::String(String::from("p")));
        options.insert(String::from("location"), Value::String(String::from("eu")));
        options.insert(String::from("fetch"), Value::String(String::from("ancien")));
        let mut evt = SdkEvent::new(model_de_test(), AI_SDK_PACKAGE, options);

        match sdk_hook(&mut evt, &env_de_test()) {
            SdkHookOutcome::Factory(appel) => {
                assert_eq!(appel.factory, "createVertex");
                assert_eq!(appel.options.get("project").and_then(|v| v.as_str()), Some("p"));
                assert_eq!(appel.options.get("location").and_then(|v| v.as_str()), Some("eu"));
                assert!(appel.options.get("fetch").is_none(), "fetch doit etre supprime");
            }
            autre => panic!("branche inattendue : {autre:?}"),
        }

        // Les paquets voisins du meme depot ne declenchent pas la fabrique.
        for paquet in [
            "@ai-sdk/google-vertex/anthropic",
            "@ai-sdk/google",
            "@ai-sdk/Google-vertex",
        ] {
            let mut evt = SdkEvent::new(model_de_test(), paquet, Options::new());
            assert!(matches!(sdk_hook(&mut evt, &env_de_test()), SdkHookOutcome::Inerte));
        }

        // Sans projet resolu, `project` vaut null comme `undefined` en JS.
        let mut evt = SdkEvent::new(model_de_test(), AI_SDK_PACKAGE, Options::new());
        match sdk_hook(&mut evt, &env_de_test()) {
            SdkHookOutcome::Factory(appel) => {
                assert_eq!(appel.options.get("project"), Some(&Value::Null));
                assert_eq!(appel.options.get("location").and_then(|v| v.as_str()), Some("us-central1"));
            }
            autre => panic!("branche inattendue : {autre:?}"),
        }
    }

    /// Second crochet : garde stricte, et la `baseURL` continentale n'apparait
    /// que pour `eu`/`us`, avec un projet resolu, et sans `baseURL` deja la.
    #[test]
    fn la_base_url_continentale_est_conditionnelle() {
        let url_attendue = "https://aiplatform.eu.rep.googleapis.com/v1/projects/p/locations/eu/publishers/anthropic/models";

        // eu + projet -> baseURL ajoutee.
        let mut options = Options::new();
        options.insert(String::from("project"), Value::String(String::from("p")));
        options.insert(String::from("location"), Value::String(String::from("eu")));
        let mut evt = SdkEvent::new(model_de_test(), AI_SDK_PACKAGE_ANTHROPIC, options);
        match sdk_hook_anthropic(&mut evt, &env_de_test()) {
            SdkHookAnthropicOutcome::Factory(appel) => {
                assert_eq!(appel.factory, "createVertexAnthropic");
                assert_eq!(appel.options.get(OPTION_KEY_BASE_URL).and_then(|v| v.as_str()), Some(url_attendue));
            }
            autre => panic!("branche inattendue : {autre:?}"),
        }

        // `us` donne la variante us.
        let mut options = Options::new();
        options.insert(String::from("location"), Value::String(String::from("us")));
        let mut evt = SdkEvent::new(model_de_test(), AI_SDK_PACKAGE_ANTHROPIC, options);
        match sdk_hook_anthropic(&mut evt, &env_de_test()) {
            SdkHookAnthropicOutcome::Factory(appel) => {
                // Pas de projet resolu -> pas de baseURL, malgre la region us.
                assert!(appel.options.get(OPTION_KEY_BASE_URL).is_none());
                assert_eq!(appel.options.get("location").and_then(|v| v.as_str()), Some("us"));
            }
            autre => panic!("branche inattendue : {autre:?}"),
        }

        // Region quelconque + projet -> pas de baseURL.
        let mut options = Options::new();
        options.insert(String::from("project"), Value::String(String::from("p")));
        let mut evt = SdkEvent::new(model_de_test(), AI_SDK_PACKAGE_ANTHROPIC, options);
        match sdk_hook_anthropic(&mut evt, &env_de_test()) {
            SdkHookAnthropicOutcome::Factory(appel) => {
                assert!(appel.options.get(OPTION_KEY_BASE_URL).is_none());
            }
            autre => panic!("branche inattendue : {autre:?}"),
        }

        // `baseURL` deja fournie -> respectee, jamais ecrasee.
        let mut options = Options::new();
        options.insert(String::from("project"), Value::String(String::from("p")));
        options.insert(
            OPTION_KEY_BASE_URL.to_string(),
            Value::String(String::from("https://perso.example")),
        );
        let mut evt = SdkEvent::new(model_de_test(), AI_SDK_PACKAGE_ANTHROPIC, options);
        match sdk_hook_anthropic(&mut evt, &env_de_test()) {
            SdkHookAnthropicOutcome::Factory(appel) => {
                assert_eq!(appel.options.get(OPTION_KEY_BASE_URL).and_then(|v| v.as_str()), Some("https://perso.example"));
            }
            autre => panic!("branche inattendue : {autre:?}"),
        }

        // Garde stricte : le paquet du premier plugin n'ouvre pas ce crochet.
        let mut evt = SdkEvent::new(model_de_test(), AI_SDK_PACKAGE, Options::new());
        assert!(matches!(sdk_hook_anthropic(&mut evt, &env_de_test()), SdkHookAnthropicOutcome::Inerte));
    }

    /// Les cles JSON echangsees sont exactement celles du TypeScript, avec les
    /// litteraux verbatim (`baseURL`, `providerID`, `fetch`) jamais renommes.
    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        let mut options = Options::new();
        options.insert(String::from("project"), Value::String(String::from("p")));
        // Region continentale `eu` : c'est la seule condition sous laquelle la
        // source ajoute une `baseURL`
        // (`(location === "eu" || location === "us") && project && !baseURL`).
        // Avec `global`, aucune `baseURL` n'est produite.
        options.insert(String::from("location"), Value::String(String::from("eu")));
        let mut evt = SdkEvent::new(model_de_test(), AI_SDK_PACKAGE_ANTHROPIC, options);

        let json = serde_json::to_value(&evt).unwrap();
        let objet = json.as_object().unwrap();
        for nom in ["model", "package", "options"] {
            assert!(objet.contains_key(nom), "champ absent du JSON : {nom}");
        }
        assert!(objet.get("sdk").is_none(), "sdk absent doit etre omis");
        assert_eq!(
            objet.get("model").unwrap().get("providerID").and_then(|v| v.as_str()),
            Some("google-vertex"),
            "providerID garde sa casse verbatim"
        );

        let mut reconstruit: SdkEvent =
            serde_json::from_value(serde_json::to_value(&evt).unwrap()).unwrap();
        assert!(matches!(
            sdk_hook_anthropic(&mut reconstruit, &env_de_test()),
            SdkHookAnthropicOutcome::Factory(_)
        ));

        // Aller-retour complet : le JSON apres crochet se relit a l'identique.
        let json_apres = serde_json::to_string(&reconstruit).unwrap();
        let relu: SdkEvent = serde_json::from_str(&json_apres).unwrap();
        assert_eq!(reconstruit, relu);

        let objet = serde_json::to_value(&relu).unwrap().as_object().unwrap().clone();
        let appel = objet.get("sdk").unwrap().as_object().unwrap();
        assert_eq!(appel.get("factory").and_then(|v| v.as_str()), Some("createVertexAnthropic"));
        let options_appel = appel.get("options").unwrap().as_object().unwrap();
        assert!(options_appel.contains_key(OPTION_KEY_BASE_URL), "baseURL garde sa casse verbatim");

        // La trace d'authentification fait aussi son aller-retour.
        let appel = AuthFetchCall::nouveau();
        let relu: AuthFetchCall = serde_json::from_str(&serde_json::to_string(&appel).unwrap()).unwrap();
        assert_eq!(appel, relu);
    }
}
