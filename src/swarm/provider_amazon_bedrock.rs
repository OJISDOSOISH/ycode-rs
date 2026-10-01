//! Portage Rust de `packages/core/src/plugin/provider/amazon-bedrock.ts`.
//!
//! ## Ce que fait la source
//!
//! Trois crochets, une fonction pure de resolution d'identifiant :
//!
//! 1. **Transformation du catalogue.** Pour chaque fournisseur dont l'api est
//!    de type `aisdk` et dont le paquet est `@ai-sdk/amazon-bedrock` : si
//!    `provider.request.body.endpoint` est une chaine, le plugin la deplace
//!    dans `provider.api.url` puis la supprime du corps. Les utilisateurs
//!    configurent les endpoints prives/VPC Bedrock comme `endpoint`, alors que
//!    l'AI SDK attend une URL de base ; le deplacement n'a lieu qu'une fois.
//! 2. **Crochet `aisdk.sdk`.** Repond a deux paquets : `@ai-sdk/amazon-bedrock`
//!    et `@ai-sdk/amazon-bedrock/mantle`. Il normalise les options (`region`,
//!    `baseURL` depuis `endpoint`, `profile`, `bearerToken`), lit et parfois
//!    ecrit des variables d'environnement AWS, puis choisit la fabrique
//!    `createAmazonBedrock` ou `createBedrockMantle` selon le paquet.
//! 3. **Crochet `aisdk.language`.** Pour un modele du fournisseur
//!    `amazon-bedrock` : si l'api du modele est le paquet mantle, choisit
//!    `sdk.chat` ou `sdk.responses` selon l'identifiant (`selectMantleModel`) ;
//!    sinon appelle `sdk.languageModel(resolveModelID(api.id, region))`.
//!
//! ## Ce qui est representable en Rust, et ce qui ne l'est pas
//!
//! - L'enregistrement des crochets (`ctx.catalog.transform(...)`,
//!   `ctx.aisdk.sdk(...)`, `ctx.aisdk.language(...)`) et les `Effect.fn`
//!   appartiennent a l'hote de plugins. Comme pour `vercel.ts`, chaque regle
//!   est portee sous forme de fonction pure ; rien ne s'execute a la
//!   construction du module.
//! - Les imports dynamiques npm (`@ai-sdk/amazon-bedrock`,
//!   `@ai-sdk/amazon-bedrock/mantle`, `@aws-sdk/credential-providers`) n'ont
//!   pas d'equivalent Rust. Le crochet sdk s'arrete donc a la **decision** :
//!   le nom de la fabrique a appeler, les options preparees, et deux
//!   instructions d'effet (ecrire `AWS_BEARER_TOKEN_BEDROCK`, injecter
//!   `fromNodeProviderChain`) rendues a l'hote. Le portage n'execute jamais
//!   lui-meme la chaine d'identifiants AWS.
//! - `process.env` est un effet global. La decision est rendue pure : elle
//!   recoit un instantane `AwsEnv` des variables lues par la source, et rend
//!   l'ecriture eventuelle de `AWS_BEARER_TOKEN_BEDROCK` sous forme de valeur
//!   `set_bearer_token_env` au lieu de muter l'environnement.
//! - `evt.model` (l'evenement `language`) n'est lu que pour `providerID` et
//!   `api.id`, deux champs deja portes par ailleurs. L'evenement n'est donc
//!   pas recopie : la regle prend l'identifiant de modele et la region.
//!
//! ## Les types de donnees
//!
//! Comme dans `provider_vercel.rs`, les formes du catalogue (`ProviderApi`,
//! `ProviderRequest`, `ProviderV2Info`, `Catalog.Draft`) sont recopiees au
//! strict minimum touche par ce plugin, avec les noms de champs exacts du
//! TypeScript. Un module voisin porte les formes completes ; ces recopies
//! cederont la place a des `use` le jour ou elles existent.
//!
//! - `BedrockApi` reprend l'union `{ type: "aisdk", package, url?, ... } |
//!   { type: "native", ... }`, discriminee par `type`. Seuls `package` et
//!   `url` sont lus ou ecrits ici ; le deplacement `endpoint` → `url` est
//!   toute l'ecriture.
//! - `BedrockRequest` ne porte que `body`, dictionnaire libre : `headers`
//!   n'est jamais touche par ce plugin.
//!
//! ## Noms de champs
//!
//! Les identifiants du corps (`endpoint`, `baseURL`, `profile`, `region`,
//! `bearerToken`, `credentialProvider`) restent des cles de dictionnaires
//! libres, manipulees sous leur forme TypeScript exacte, surtout `baseURL` et
//! `bearerToken` en camelCase. Les champs de structures sont `id`, `api`,
//! `request`, `body`, `package`, `url`, `providers`, tous identiques dans les
//! deux langages ; chaque `rename` est ecrit explicitement malgre tout, et les
//! tests serialisent chaque structure champ par champ.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Litteraux du plugin
// ---------------------------------------------------------------------------

/// Identifiant enregistre par `define`, dans `plugin.internal`.
pub const PLUGIN_ID: &str = "amazon-bedrock";

/// Type d'api que le plugin reconnait, premiere condition du filtre.
pub const AISDK_TYPE: &str = "aisdk";

/// Paquet npm principal, et valeur de `ProviderV2.ID.amazonBedrock`.
pub const AISDK_PACKAGE: &str = "@ai-sdk/amazon-bedrock";

/// Paquet npm de la variante mantle, reconnu par le crochet sdk et lu par le
/// crochet language sur `evt.model.api.package`.
pub const MANTLE_PACKAGE: &str = "@ai-sdk/amazon-bedrock/mantle";

/// Fabrique exportee par le paquet principal.
pub const AISDK_FACTORY: &str = "createAmazonBedrock";

/// Fabrique exportee par le paquet mantle.
pub const MANTLE_FACTORY: &str = "createBedrockMantle";

/// Fabrique d'identifiants importee de `@aws-sdk/credential-providers` quand
/// aucune chaine d'identifiants explicite n'est fournie.
pub const CREDENTIAL_FACTORY: &str = "fromNodeProviderChain";

/// Variable d'environnement lue pour le profil AWS.
pub const AWS_PROFILE_ENV: &str = "AWS_PROFILE";

/// Variable d'environnement lue pour la region AWS.
pub const AWS_REGION_ENV: &str = "AWS_REGION";

/// Variable d'environnement lue, et parfois ecrite, pour le jeton Bearer.
pub const AWS_BEARER_TOKEN_ENV: &str = "AWS_BEARER_TOKEN_BEDROCK";

/// Variable d'environnement qui signale des identifiants de conteneur.
pub const AWS_CONTAINER_RELATIVE_URI_ENV: &str = "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI";

/// Seconde variable d'environnement de conteneur, meme role.
pub const AWS_CONTAINER_FULL_URI_ENV: &str = "AWS_CONTAINER_CREDENTIALS_FULL_URI";

/// Region par defaut : `region ?? "us-east-1"` dans `resolveModelID`, et
/// `process.env.AWS_REGION ?? "us-east-1"` dans le crochet sdk.
pub const DEFAULT_REGION: &str = "us-east-1";

/// Cle du corps de requete que la transformation du catalogue deplace.
pub const ENDPOINT_BODY_KEY: &str = "endpoint";

/// Cle d'option qui recoit la valeur deplacee dans le crochet sdk.
pub const BASE_URL_OPTION_KEY: &str = "baseURL";

// ---------------------------------------------------------------------------
// Resolution d'identifiant de modele (resolveModelID)
// ---------------------------------------------------------------------------

/// Prefixes inter-region : un identifiant qui commence deja par l'un d'eux est
/// rendu tel quel, pour ne jamais le double-prefixer. L'ordre n'importe pas,
/// ce sont des debuts de chaine testes un par un.
pub const CROSS_REGION_PREFIXES: [&str; 6] = ["global.", "us.", "eu.", "jp.", "apac.", "au."];

/// Modeles qui exigent un prefixe regional en region `us`.
pub const US_REQUIRES_PREFIX: [&str; 7] =
    ["nova-micro", "nova-lite", "nova-pro", "nova-premier", "nova-2", "claude", "deepseek.r1"];

/// Regions `eu` qui exigent un prefixe. Attention : la source teste
/// `resolvedRegion.includes(item)`, une **sous-chaine**, pas une egalite.
pub const EU_REGIONS_REQUIRES_PREFIX: [&str; 7] = [
    "eu-west-1",
    "eu-west-2",
    "eu-west-3",
    "eu-north-1",
    "eu-central-1",
    "eu-south-1",
    "eu-south-2",
];

/// Modeles qui exigent un prefixe en region `eu`.
pub const EU_MODELS_REQUIRES_PREFIX: [&str; 5] = ["claude", "nova-lite", "nova-micro", "llama3", "pixtral"];

/// Regions australiennes : la source teste l'appartenance a la liste
/// (`["a", "b"].includes(region)`), donc une **egalite** cette fois.
pub const AUSTRALIA_REGIONS: [&str; 2] = ["ap-southeast-2", "ap-southeast-4"];

/// Modeles qui exigent le prefixe `au.` dans une region australienne.
pub const AU_MODELS_REQUIRES_PREFIX: [&str; 2] = ["anthropic.claude-sonnet-4-5", "anthropic.claude-haiku"];

/// Modeles qui exigent un prefixe dans le reste de l'Asie-Pacifique.
pub const AP_MODELS_REQUIRES_PREFIX: [&str; 4] = ["claude", "nova-lite", "nova-micro", "nova-pro"];

/// Region qui recoit le prefixe `jp.` au lieu de `apac.`.
pub const JAPAN_REGION: &str = "ap-northeast-1";

/// Dit si l'identifiant commence par l'un des prefixes inter-region.
fn a_deja_un_prefixe_inter_region(model_id: &str) -> bool {
    CROSS_REGION_PREFIXES.iter().any(|prefix| model_id.starts_with(prefix))
}

/// Dit si l'identifiant contient l'un des fragments, comme
/// `liste.some((item) => modelID.includes(item))`.
fn contient_un(model_id: &str, fragments: &[&str]) -> bool {
    fragments.iter().any(|item| model_id.contains(item))
}

/// Portage de `resolveModelID(modelID, region)`.
///
/// Les profils d'inference inter-region de Bedrock n'exigent un prefixe
/// regional que pour certains couples modele/region. La fonction garde la
/// correspondance etroite de la source et ne double-prefixe jamais un
/// identifiant deja marque `global.` / `us.` / `eu.` / etc.
///
/// Ordre des tests, tel quel dans le TypeScript :
///
/// 1. `arn:` : rendu tel quel, avant tout le reste.
/// 2. prefixe inter-region deja present : rendu tel quel, **avant** le
///    defaut de region — la region n'est donc pas lue dans ce cas.
/// 3. region par defaut `us-east-1` quand `region` est absente.
/// 4. region `us` : prefixe si le modele est sur la liste, sauf region
///    `us-gov-*` (teste par `startsWith`, pas par egalite).
/// 5. region `eu` : prefixe si la region contient l'une des regions de la
///    liste (sous-chaine) **et** le modele est sur la liste.
/// 6. toute autre region sauf `ap*` : rendu tel quel.
/// 7. region `ap*` : `au.` dans les deux regions australiennes pour les
///    modeles anthropiques lists ; sinon `jp.` en `ap-northeast-1`,
///    `apac.` ailleurs, et seulement pour les modeles lists.
pub fn resolve_model_id(model_id: &str, region: Option<&str>) -> String {
    if model_id.starts_with("arn:") {
        return model_id.to_string();
    }

    if a_deja_un_prefixe_inter_region(model_id) {
        return model_id.to_string();
    }

    let resolved_region = region.unwrap_or(DEFAULT_REGION);
    let region_prefix = resolved_region.split('-').next().unwrap_or("");

    if region_prefix == "us" {
        let requires_prefix = contient_un(model_id, &US_REQUIRES_PREFIX);
        if requires_prefix && !resolved_region.starts_with("us-gov") {
            return format!("{}.{}", region_prefix, model_id);
        }
        return model_id.to_string();
    }

    if region_prefix == "eu" {
        let region_requires_prefix = contient_un(resolved_region, &EU_REGIONS_REQUIRES_PREFIX);
        let model_requires_prefix = contient_un(model_id, &EU_MODELS_REQUIRES_PREFIX);
        return if region_requires_prefix && model_requires_prefix {
            format!("{}.{}", region_prefix, model_id)
        } else {
            model_id.to_string()
        };
    }

    if region_prefix != "ap" {
        return model_id.to_string();
    }

    let australia = AUSTRALIA_REGIONS.iter().any(|item| *item == resolved_region);
    if australia && contient_un(model_id, &AU_MODELS_REQUIRES_PREFIX) {
        return format!("au.{}", model_id);
    }

    let prefix = if resolved_region == JAPAN_REGION { "jp" } else { "apac" };
    if contient_un(model_id, &AP_MODELS_REQUIRES_PREFIX) {
        format!("{}.{}", prefix, model_id)
    } else {
        model_id.to_string()
    }
}

// ---------------------------------------------------------------------------
// Selection du modele mantle (selectMantleModel)
// ---------------------------------------------------------------------------

/// La methode du SDK mantle a appeler pour un identifiant de modele.
///
/// En TypeScript, `selectMantleModel` renvoie `sdk.chat(modelID)` pour les
/// deux modeles `openai.gpt-oss-safeguard`, et `sdk.responses(modelID)` pour
/// tous les autres. La fabrique etant un import npm sans equivalent Rust, on
/// rend la methode designee plutot que son resultat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MantleMethod {
    /// `sdk.chat(modelID)`.
    Chat,
    /// `sdk.responses(modelID)`.
    Responses,
}

impl MantleMethod {
    /// Le nom de la methode JavaScript, tel qu'appelle sur le SDK.
    pub fn method_name(&self) -> &'static str {
        match self {
            MantleMethod::Chat => "chat",
            MantleMethod::Responses => "responses",
        }
    }
}

/// Portage de `selectMantleModel(sdk, modelID)`, reduit a sa decision.
pub fn select_mantle_method(model_id: &str) -> MantleMethod {
    if model_id == "openai.gpt-oss-safeguard-20b" || model_id == "openai.gpt-oss-safeguard-120b" {
        MantleMethod::Chat
    } else {
        MantleMethod::Responses
    }
}

// ---------------------------------------------------------------------------
// Donnees lues et ecrites par le plugin
// ---------------------------------------------------------------------------

/// L'api d'un fournisseur : soit un paquet aisdk, soit une api native.
///
/// En TS : `ProviderApi`, deux objets discriminant par `type`. Ce plugin ne
/// lit que `package` et n'ecrit que `url` sur la variante aisdk ; les autres
/// champs eventuels (`settings`, etc.) sont ignores par serde et restent dans
/// le JSON d'origine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BedrockApi {
    /// `{ type: "aisdk", package, url?, ... }`.
    Aisdk {
        /// Paquet npm du fournisseur.
        package: String,
        /// URL de base. Absente jusqu'a ce que la transformation du catalogue
        /// y deplace la valeur de `request.body.endpoint`.
        #[serde(rename = "url", default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
    },
    /// `{ type: "native", ... }`. Ne porte pas de `package`, comme la source.
    Native {},
}

impl BedrockApi {
    /// Api aisdk reduite a son paquet, sans URL.
    pub fn aisdk(package: impl Into<String>) -> Self {
        BedrockApi::Aisdk { package: package.into(), url: None }
    }

    /// Api native, jamais touchee par ce plugin.
    pub fn native() -> Self {
        BedrockApi::Native {}
    }

    /// Le discriminant, c'est-a-dire la valeur du champ JSON `type`.
    pub fn type_name(&self) -> &'static str {
        match self {
            BedrockApi::Aisdk { .. } => AISDK_TYPE,
            BedrockApi::Native {} => "native",
        }
    }

    /// Le paquet npm, ou `None` pour une api native qui n'en a pas.
    pub fn package(&self) -> Option<&str> {
        match self {
            BedrockApi::Aisdk { package, .. } => Some(package.as_str()),
            BedrockApi::Native {} => None,
        }
    }
}

/// La partie `request` d'un fournisseur : le corps libre uniquement.
///
/// En TS : `ProviderRequest = { headers, body }`. Ce plugin n'ecrit jamais
/// d'en-tete ; seul `body` est lu (cle `endpoint`) et modifie.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BedrockRequest {
    /// Corps de requete additionnel, valeurs libres.
    #[serde(rename = "body")]
    pub body: BTreeMap<String, Value>,
}

impl BedrockRequest {
    /// Requete sans corps.
    pub fn new() -> Self {
        BedrockRequest::default()
    }
}

/// Un fournisseur tel que la transformation du catalogue le voit.
///
/// Version reduite de `ProviderV2Info` : seuls `id`, `api` et `request` sont
/// lus ou ecrits par ce plugin. Les autres champs de la source sont absents
/// d'ici ; serde les ignore a la relecture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BedrockProvider {
    /// Identifiant du fournisseur. Sert de cle a `update` dans le catalogue.
    #[serde(rename = "id")]
    pub id: String,
    /// Son api, qui porte le type et le paquet lus par le filtre.
    #[serde(rename = "api")]
    pub api: BedrockApi,
    /// Sa requete, seule partie que le plugin modifie.
    #[serde(rename = "request")]
    pub request: BedrockRequest,
}

impl BedrockProvider {
    /// Fournisseur sans corps de requete.
    pub fn new(id: impl Into<String>, api: BedrockApi) -> Self {
        BedrockProvider { id: id.into(), api, request: BedrockRequest::new() }
    }
}

/// Brouillon de catalogue, version reduite de `Catalog.Draft`.
///
/// Comme dans `provider_vercel.rs` : seule la partie `provider` est portee,
/// avec `list` et `update`. La vraie Map JavaScript preserve l'ordre
/// d'insertion, d'ou le `Vec` : l'ordre du catalogue est observable, il est
/// preserve et jamais trie.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BedrockCatalogDraft {
    /// Fournisseurs du brouillon, dans leur ordre d'insertion.
    #[serde(rename = "providers")]
    pub providers: Vec<BedrockProvider>,
}

impl BedrockCatalogDraft {
    /// Brouillon a partir d'une liste de fournisseurs.
    pub fn new(providers: Vec<BedrockProvider>) -> Self {
        BedrockCatalogDraft { providers }
    }

    /// `evt.provider.list()` : tous les fournisseurs, dans l'ordre.
    pub fn list(&self) -> &[BedrockProvider] {
        &self.providers
    }

    /// `evt.provider.get(providerID)` : un fournisseur, ou `None`.
    pub fn get(&self, provider_id: &str) -> Option<&BedrockProvider> {
        self.providers.iter().find(|provider| provider.id == provider_id)
    }

    /// `evt.provider.update(providerID, fn)` : applique `update` au
    /// fournisseur nomme, et renvoie `true` s'il existait.
    pub fn update<F>(&mut self, provider_id: &str, update: F) -> bool
    where
        F: FnOnce(&mut BedrockProvider),
    {
        match self.providers.iter_mut().find(|provider| provider.id == provider_id) {
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
/// En TS : `{ readonly model, readonly package, readonly options, sdk? }`.
/// `model` n'est pas lu par ce crochet et `sdk` est un `any` ecrit par l'hote :
/// ni l'un ni l'autre n'est declare ici. Les options sont un dictionnaire
/// libre, parce que la source lit et ecrit des cles isolees dessus.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BedrockSdkEvent {
    /// Paquet npm demande. Le crochet accepte exactement les deux paquets
    /// constants, avec la casse : le test TS est `includes([...])`.
    #[serde(rename = "package")]
    pub package: String,
    /// Options a passer a la fabrique, dictionnaire libre.
    #[serde(rename = "options")]
    pub options: BTreeMap<String, Value>,
}

impl BedrockSdkEvent {
    /// Evenement sans option.
    pub fn new(package: impl Into<String>) -> Self {
        BedrockSdkEvent { package: package.into(), options: BTreeMap::new() }
    }
}

/// Instantane des variables d'environnement AWS lues par le crochet sdk.
///
/// Le TypeScript lit `process.env` directement ; ici l'instantane est fourni
/// par l'hote (voir [`AwsEnv::capture`]), ce qui garde la decision pure et
/// testable.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AwsEnv {
    /// `process.env.AWS_PROFILE`.
    #[serde(rename = "profile")]
    pub profile: Option<String>,
    /// `process.env.AWS_REGION`.
    #[serde(rename = "region")]
    pub region: Option<String>,
    /// `process.env.AWS_BEARER_TOKEN_BEDROCK`.
    #[serde(rename = "bearerToken")]
    pub bearer_token: Option<String>,
    /// `Boolean(AWS_CONTAINER_CREDENTIALS_RELATIVE_URI ||
    /// AWS_CONTAINER_CREDENTIALS_FULL_URI)`.
    #[serde(rename = "containerCredentials")]
    pub container_credentials: bool,
}

impl AwsEnv {
    /// Lit les variables d'environnement reelles, comme le fait le TS.
    pub fn capture() -> Self {
        fn var(name: &str) -> Option<String> {
            std::env::var(name).ok()
        }
        AwsEnv {
            profile: var(AWS_PROFILE_ENV),
            region: var(AWS_REGION_ENV),
            bearer_token: var(AWS_BEARER_TOKEN_ENV),
            container_credentials: var(AWS_CONTAINER_RELATIVE_URI_ENV).is_some()
                || var(AWS_CONTAINER_FULL_URI_ENV).is_some(),
        }
    }
}

/// L'identifiant de chaine d'identifiants AWS a injecter dans les options.
///
/// La source importe `fromNodeProviderChain` et l'appelle avec
/// `profile ? { profile } : {}`. L'appel redevient la responsabilite de
/// l'hote, qui seul peut executer du JavaScript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialProviderInjection {
    /// La fabrique a appeler : `fromNodeProviderChain`.
    #[serde(rename = "factory")]
    pub factory: &'static str,
    /// Le profil passe dans l'argument, absent si l'appel recoit `{}`.
    #[serde(rename = "profile", skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// Ce que le plugin decide de faire d'un evenement sdk.
///
/// Type sans representation JSON : c'est une decision, pas une donnee qui
/// circule. Les options rendues sont **deja preparees** (region inseree,
/// `endpoint` copie vers `baseURL`), exactement comme le TS les a mutees
/// avant l'appel de fabrique.
#[derive(Debug, Clone, PartialEq)]
pub enum BedrockSdkAction {
    /// Paquet different des deux reconnus : le crochet rend la main sans rien
    /// changer, comme le `return` du TypeScript.
    Ignore,
    /// Paquet reconnu : appeler la fabrique avec ces options, puis ranger le
    /// resultat dans `evt.sdk`.
    Create {
        /// Le paquet reconnu, donc le module npm a importer.
        package: &'static str,
        /// La fabrique exportee par ce module.
        factory: &'static str,
        /// Les options preparees, a passer telles quelles.
        options: BTreeMap<String, Value>,
        /// Ecrire `AWS_BEARER_TOKEN_BEDROCK` avec cette valeur avant
        /// l'appel, quand le jeton est venu des options et pas de
        /// l'environnement. La source le fait par effet de bord.
        set_bearer_token_env: Option<String>,
        /// Injecter `fromNodeProviderChain` dans
        /// `options.credentialProvider`, quand ni jeton Bearer ni chaine
        /// d'identifiants explicite n'existent.
        inject_credential_provider: Option<CredentialProviderInjection>,
    },
}

// ---------------------------------------------------------------------------
// Le plugin
// ---------------------------------------------------------------------------

/// Portage du `AmazonBedrockPlugin` exporte par la source.
///
/// En TS, `define({ id: "amazon-bedrock", effect })` : `define` ne fait rien
/// de plus que fixer le type. Le plugin n'a aucun etat ; ses trois regles sont
/// portees ici comme fonctions pures, et l'enregistrement effectif des
/// crochets revient a l'hote de plugins, comme en TS.
pub struct BedrockPlugin;

impl BedrockPlugin {
    /// L'identifiant enregistre par l'hote, `"amazon-bedrock"`.
    pub const ID: &'static str = PLUGIN_ID;

    /// Dit si le paquet est l'un des deux reconnus par le crochet sdk.
    ///
    /// Le TS ecrit
    /// `!["@ai-sdk/amazon-bedrock", "@ai-sdk/amazon-bedrock/mantle"].includes(evt.package)`.
    /// Egalite stricte sur chaque element : la casse compte, la chaine vide ne
    /// correspond a rien.
    pub fn reconnait_le_paquet(package: &str) -> bool {
        package == AISDK_PACKAGE || package == MANTLE_PACKAGE
    }

    /// Regle de la transformation du catalogue.
    ///
    /// Le TS parcourt `evt.provider.list()`, saute tout fournisseur dont l'api
    /// n'est pas `aisdk` ou dont le paquet n'est pas `@ai-sdk/amazon-bedrock`,
    /// puis appelle `update` sur les autres. Dans le `update`, deux gardes de
    /// plus : l'api doit encore etre `aisdk`, et `request.body.endpoint` doit
    /// etre une **chaine**. Alors `api.url = endpoint` et `delete
    /// body.endpoint`. Renvoie les identifiants touches, dans l'ordre du
    /// catalogue.
    ///
    /// La liste est parcourue avant toute ecriture, comme en TS.
    pub fn transform(draft: &mut BedrockCatalogDraft) -> Vec<String> {
        let touches: Vec<String> = draft
            .list()
            .iter()
            .filter(|provider| {
                matches!(provider.api, BedrockApi::Aisdk { .. })
                    && provider.api.package() == Some(AISDK_PACKAGE)
            })
            .map(|provider| provider.id.clone())
            .collect();
        for provider_id in &touches {
            draft.update(provider_id, |provider| {
                // Garde interne du `update` TS : l'api doit encore etre aisdk.
                let BedrockApi::Aisdk { url, .. } = &mut provider.api else {
                    return;
                };
                let Some(endpoint) = provider.request.body.get(ENDPOINT_BODY_KEY) else {
                    return;
                };
                let Some(endpoint) = endpoint.as_str() else {
                    return;
                };
                *url = Some(endpoint.to_string());
                provider.request.body.remove(ENDPOINT_BODY_KEY);
            });
        }
        touches
    }

    /// Regle du crochet `aisdk.sdk`.
    ///
    /// Porte la decision jusqu'au seuil des imports npm, avec l'instantane
    /// d'environnement fourni par l'hote. Dans l'ordre du TypeScript :
    ///
    /// 1. paquet non reconnu : `Ignore`, les options restent intactes ;
    /// 2. `profile` : l'option si c'est une chaine, sinon `AWS_PROFILE` ;
    /// 3. `region` : l'option si c'est une chaine, sinon `AWS_REGION`, sinon
    ///    `"us-east-1"` ;
    /// 4. `bearerToken` : `AWS_BEARER_TOKEN_BEDROCK` d'abord, sinon l'option
    ///    `bearerToken` si c'est une chaine ;
    /// 5. si un jeton existe et que l'environnement n'en avait pas, l'ecriture
    ///    de `AWS_BEARER_TOKEN_BEDROCK` est demandee a l'hote ;
    /// 6. `options.region = region`, toujours, meme si la valeur est la meme ;
    /// 7. si `options.endpoint` est une chaine, `options.baseURL = endpoint`.
    ///    L'option `endpoint` n'est **pas** supprimee ici (contrairement a la
    ///    transformation du catalogue, qui, elle, supprime sa cle) ;
    /// 8. si pas de jeton et `options.credentialProvider === undefined`,
    ///    injection de `fromNodeProviderChain(profile ? { profile } : {})`.
    ///    Le commentaire de la source precise que la decision ne depend pas
    ///    des variables AWS explicites : la chaine par defaut couvre aussi
    ///    `~/.aws/credentials`, SSO, identifiants de processus et roles
    ///    d'instance ;
    /// 9. fabrique : `createBedrockMantle` pour le paquet mantle,
    ///    `createAmazonBedrock` sinon.
    ///
    /// `containerCredentials` de l'instantane est lu par la source mais ne
    /// change aucune branche : elle ne sert qu'au commentaire. Rien a rendre.
    pub fn sdk_action(event: &BedrockSdkEvent, env: &AwsEnv) -> BedrockSdkAction {
        if !BedrockPlugin::reconnait_le_paquet(&event.package) {
            return BedrockSdkAction::Ignore;
        }

        let mut options = event.options.clone();

        let profile = match options.get("profile") {
            Some(Value::String(value)) => Some(value.clone()),
            _ => env.profile.clone(),
        };
        let region = match options.get("region") {
            Some(Value::String(value)) => Some(value.clone()),
            _ => env.region.clone().unwrap_or_else(|| DEFAULT_REGION.to_string()),
        };
        let bearer_token = env.bearer_token.clone().or_else(|| match options.get("bearerToken") {
            Some(Value::String(value)) => Some(value.clone()),
            _ => None,
        });

        let set_bearer_token_env = match (&bearer_token, &env.bearer_token) {
            (Some(token), None) => Some(token.clone()),
            _ => None,
        };

        if let Some(region) = region.clone() {
            options.insert("region".to_string(), Value::String(region));
        }
        if let Some(Value::String(endpoint)) = options.get("endpoint").cloned() {
            options.insert(BASE_URL_OPTION_KEY.to_string(), Value::String(endpoint));
        }

        let inject_credential_provider = if bearer_token.is_none()
            && !options.contains_key("credentialProvider")
        {
            Some(CredentialProviderInjection { factory: CREDENTIAL_FACTORY, profile })
        } else {
            None
        };

        let (package, factory) = if event.package == MANTLE_PACKAGE {
            (MANTLE_PACKAGE, MANTLE_FACTORY)
        } else {
            (AISDK_PACKAGE, AISDK_FACTORY)
        };

        BedrockSdkAction::Create { package, factory, options, set_bearer_token_env, inject_credential_provider }
    }

    /// Regle du crochet `aisdk.language`, partie identifiant.
    ///
    /// Le TS rend `evt.sdk.languageModel(resolveModelID(evt.model.api.id,
    /// region))`, avec `region` = l'option `region` si c'est une chaine, sinon
    /// `AWS_REGION`. L'appel au SDK appartient a l'hote ; la decision porte
    /// donc sur l'identifiant resolu. `resolve_model_id` porte toute la logique
    /// et la region a le meme defaut que le crochet sdk quand elle manque des
    /// deux cotes, c'est-a-dire jamais ici : la source ne passe jamais une
    /// region `undefined` par defaut, seulement `AWS_REGION` non definie.
    pub fn language_model_id(model_id: &str, region: Option<&str>) -> String {
        resolve_model_id(model_id, region)
    }

    /// Regle du crochet `aisdk.language`, variante mantle.
    ///
    /// Quand `evt.model.api.package` vaut `@ai-sdk/amazon-bedrock/mantle`, la
    /// source appelle `selectMantleModel` au lieu de `languageModel`.
    pub fn mantle_method(model_id: &str) -> MantleMethod {
        select_mantle_method(model_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fournisseur_bedrock() -> BedrockProvider {
        BedrockProvider::new("bedrock", BedrockApi::aisdk(AISDK_PACKAGE))
    }

    // ------------------------------------------------- transformation catalogue

    #[test]
    fn un_endpoint_en_chaine_est_deplace_vers_l_url_de_l_api() {
        let mut provider = fournisseur_bedrock();
        provider.request.body.insert(ENDPOINT_BODY_KEY.to_string(), json!("https://vpc.example.com"));
        let mut draft = BedrockCatalogDraft::new(vec![provider]);

        let touches = BedrockPlugin::transform(&mut draft);

        assert_eq!(touches, vec!["bedrock".to_string()]);
        let touche = draft.get("bedrock").unwrap();
        assert_eq!(touche.api.package(), Some(AISDK_PACKAGE));
        match &touche.api {
            BedrockApi::Aisdk { url, .. } => assert_eq!(url.as_deref(), Some("https://vpc.example.com")),
            autre => panic!("l'api doit rester aisdk, pas {:?}", autre),
        }
        assert!(touche.request.body.get(ENDPOINT_BODY_KEY).is_none(), "la cle endpoint est supprimee");
    }

    #[test]
    fn un_endpoint_non_chaine_n_est_pas_deplace_et_n_est_pas_supprime() {
        // Le TS teste `typeof provider.request.body.endpoint !== "string"` :
        // nombre, objet, null ou cle absente laissent le fournisseur intact.
        for valeur in [json!(42), json!(null), json!({ "url": "x" })] {
            let mut provider = fournisseur_bedrock();
            provider.request.body.insert(ENDPOINT_BODY_KEY.to_string(), valeur.clone());
            let mut draft = BedrockCatalogDraft::new(vec![provider]);

            let touches = BedrockPlugin::transform(&mut draft);

            assert!(touches.is_empty(), "la valeur {:?} ne doit pas declencher le deplacement", valeur);
            let intact = draft.get("bedrock").unwrap();
            assert_eq!(intact.request.body.get(ENDPOINT_BODY_KEY), Some(&valeur));
        }
    }

    #[test]
    fn une_cle_endpoint_absente_ne_provoque_rien() {
        let mut draft = BedrockCatalogDraft::new(vec![fournisseur_bedrock()]);

        let touches = BedrockPlugin::transform(&mut draft);

        assert!(touches.is_empty());
    }

    #[test]
    fn un_fournisseur_qui_n_est_pas_bedrock_n_est_pas_touche() {
        let mut draft = BedrockCatalogDraft::new(vec![
            BedrockProvider::new("gateway", BedrockApi::native()),
            BedrockProvider::new("anthropic", BedrockApi::aisdk("@ai-sdk/anthropic")),
        ]);

        let touches = BedrockPlugin::transform(&mut draft);

        assert!(touches.is_empty());
        for id in ["gateway", "anthropic"] {
            assert!(draft.get(id).unwrap().request.body.is_empty());
        }
    }

    #[test]
    fn le_paquet_mantle_n_est_pas_reconnu_par_la_transformation_du_catalogue() {
        // Le filtre du catalogue ne liste que `@ai-sdk/amazon-bedrock`, sans
        // la variante mantle, contrairement au crochet sdk.
        let mut draft =
            BedrockCatalogDraft::new(vec![BedrockProvider::new("mantle", BedrockApi::aisdk(MANTLE_PACKAGE))]);

        let touches = BedrockPlugin::transform(&mut draft);

        assert!(touches.is_empty());
    }

    #[test]
    fn un_catalogue_vide_ne_produit_aucun_changement() {
        let mut draft = BedrockCatalogDraft::new(Vec::new());

        assert!(BedrockPlugin::transform(&mut draft).is_empty());
        assert!(draft.list().is_empty());
    }

    #[test]
    fn les_fournisseurs_sont_traites_dans_l_ordre_du_catalogue() {
        let mut provider_a = fournisseur_bedrock();
        provider_a.id = "zeta".to_string();
        provider_a.request.body.insert(ENDPOINT_BODY_KEY.to_string(), json!("https://a.example.com"));
        let mut provider_b = fournisseur_bedrock();
        provider_b.id = "alpha".to_string();
        provider_b.request.body.insert(ENDPOINT_BODY_KEY.to_string(), json!("https://b.example.com"));
        let mut draft = BedrockCatalogDraft::new(vec![provider_a, provider_b]);

        let touches = BedrockPlugin::transform(&mut draft);

        assert_eq!(touches, vec!["zeta".to_string(), "alpha".to_string()], "l'ordre est preserve");
    }

    // ------------------------------------------------- resolution d'identifiant

    #[test]
    fn un_arn_est_rendu_tel_quel_quelle_que_soit_la_region() {
        let arn = "arn:aws:bedrock:eu-west-3::foundation-model/anthropic.claude-3-5-sonnet";
        assert_eq!(resolve_model_id(arn, Some("eu-west-3")), arn);
        assert_eq!(resolve_model_id(arn, None), arn);
    }

    #[test]
    fn un_identifiant_deja_prefixe_inter_region_n_est_jamais_double_prefixe() {
        for id in ["global.claude-x", "us.claude-x", "eu.claude-x", "jp.claude-x", "apac.claude-x", "au.claude-x"] {
            assert_eq!(resolve_model_id(id, Some("us-east-1")), id, "{}", id);
            assert_eq!(resolve_model_id(id, Some("eu-west-1")), id, "{}", id);
            assert_eq!(resolve_model_id(id, None), id, "{}", id);
        }
    }

    #[test]
    fn la_region_par_defaut_est_us_east_1() {
        // `resolveModelID(id)` sans region vaut `resolveModelID(id, "us-east-1")`.
        assert_eq!(resolve_model_id("anthropic.claude-3-5-sonnet", None), "us.anthropic.claude-3-5-sonnet");
        assert_eq!(
            resolve_model_id("anthropic.claude-3-5-sonnet", Some("us-east-1")),
            resolve_model_id("anthropic.claude-3-5-sonnet", None)
        );
    }

    #[test]
    fn une_region_us_prefixe_les_modeles_de_la_liste() {
        for id in ["nova-micro", "nova-lite", "nova-pro", "nova-premier", "nova-2", "claude-x", "deepseek.r1-v2"] {
            assert_eq!(resolve_model_id(id, Some("us-east-1")), format!("us.{}", id), "{}", id);
        }
    }

    #[test]
    fn une_region_us_gov_ne_prefixe_pas_meme_pour_les_modeles_de_la_liste() {
        // Le test TS est `!resolvedRegion.startsWith("us-gov")`, un prefixe :
        // toutes les regions us-gov sont epargnees, y compris us-gov-west-1.
        for region in ["us-gov-east-1", "us-gov-west-1"] {
            assert_eq!(resolve_model_id("claude-x", Some(region)), "claude-x");
            assert_eq!(resolve_model_id("nova-lite", Some(region)), "nova-lite");
        }
    }

    #[test]
    fn une_region_us_sans_modele_de_la_liste_n_est_pas_prefixee() {
        assert_eq!(resolve_model_id("llama3-x", Some("us-east-1")), "llama3-x");
        assert_eq!(resolve_model_id("pixtral-x", Some("us-west-2")), "pixtral-x");
    }

    #[test]
    fn le_fragment_est_cherche_partout_dans_l_identifiant() {
        // `modelID.includes(item)`, pas un `startsWith` : le fragment peut
        // etre au milieu, comme dans les identifiants anthropiques.
        assert_eq!(resolve_model_id("anthropic.claude-3-5-sonnet", Some("us-east-1")), "us.anthropic.claude-3-5-sonnet");
        assert_eq!(resolve_model_id("us.anthropic.claude", Some("us-east-1")), "us.anthropic.claude");
    }

    #[test]
    fn une_region_eu_listee_prefixe_les_modeles_de_la_liste_eu() {
        for region in ["eu-west-1", "eu-west-2", "eu-west-3", "eu-north-1", "eu-central-1", "eu-south-1", "eu-south-2"] {
            for id in ["claude-x", "nova-lite", "nova-micro", "llama3-x", "pixtral-x"] {
                assert_eq!(resolve_model_id(id, Some(region)), format!("eu.{}", id), "{} en {}", id, region);
            }
        }
    }

    #[test]
    fn la_region_eu_est_cherchee_en_sous_chaine_pas_en_egalite() {
        // `resolvedRegion.includes(item)` : une region comme eu-west-1-fips
        // contient "eu-west-1" et declenche donc le prefixe, contrairement a
        // une appartenance stricte.
        assert_eq!(resolve_model_id("claude-x", Some("eu-west-1-fips")), "eu.claude-x");
    }

    #[test]
    fn une_region_eu_hors_liste_ne_prefixe_pas() {
        assert_eq!(resolve_model_id("claude-x", Some("eu-central-2")), "claude-x");
    }

    #[test]
    fn une_region_eu_listee_avec_un_modele_hors_liste_ne_prefixe_pas() {
        assert_eq!(resolve_model_id("deepseek.r1", Some("eu-west-1")), "deepseek.r1");
    }

    #[test]
    fn une_region_autre_que_us_eu_ou_ap_ne_prefixe_pas() {
        for region in ["ca-central-1", "sa-east-1", "af-south-1", "me-south-1", "il-central-1"] {
            assert_eq!(resolve_model_id("claude-x", Some(region)), "claude-x", "{}", region);
        }
    }

    #[test]
    fn une_region_australienne_prefixe_les_modeles_anthropiques_lists() {
        for region in ["ap-southeast-2", "ap-southeast-4"] {
            assert_eq!(
                resolve_model_id("anthropic.claude-sonnet-4-5", Some(region)),
                "au.anthropic.claude-sonnet-4-5",
                "{}",
                region
            );
            assert_eq!(resolve_model_id("anthropic.claude-haiku", Some(region)), "au.anthropic.claude-haiku");
        }
    }

    #[test]
    fn une_region_australienne_avec_un_autre_modele_recoit_le_prefixe_apac() {
        // Un modele de la liste AP mais pas de la liste au. : la branche
        // australienne echoue, la branche generale apac s'applique.
        assert_eq!(resolve_model_id("nova-lite", Some("ap-southeast-2")), "apac.nova-lite");
    }

    #[test]
    fn le_japon_recoit_le_prefixe_jp_pour_les_modeles_de_la_liste_ap() {
        for id in ["claude-x", "nova-lite", "nova-micro", "nova-pro"] {
            assert_eq!(resolve_model_id(id, Some("ap-northeast-1")), format!("jp.{}", id), "{}", id);
        }
    }

    #[test]
    fn le_reste_de_l_asie_pacifique_recoit_le_prefixe_apac() {
        for region in ["ap-south-1", "ap-northeast-2", "ap-southeast-1", "ap-east-1"] {
            assert_eq!(resolve_model_id("claude-x", Some(region)), "apac.claude-x", "{}", region);
        }
    }

    #[test]
    fn une_region_ap_avec_un_modele_hors_liste_ne_prefixe_pas() {
        assert_eq!(resolve_model_id("llama3-x", Some("ap-northeast-1")), "llama3-x");
        assert_eq!(resolve_model_id("pixtral-x", Some("ap-south-1")), "pixtral-x");
    }

    // -------------------------------------------------------------- mantle

    #[test]
    fn les_deux_modeles_safeguard_utilisent_chat() {
        assert_eq!(select_mantle_method("openai.gpt-oss-safeguard-20b"), MantleMethod::Chat);
        assert_eq!(select_mantle_method("openai.gpt-oss-safeguard-120b"), MantleMethod::Chat);
        assert_eq!(MantleMethod::Chat.method_name(), "chat");
    }

    #[test]
    fn tout_autre_modele_mantle_utilise_responses() {
        for id in ["openai.gpt-oss-120b", "anthropic.claude-x", "", "openai.gpt-oss-safeguard"] {
            assert_eq!(select_mantle_method(id), MantleMethod::Responses, "{}", id);
        }
        assert_eq!(MantleMethod::Responses.method_name(), "responses");
    }

    // ------------------------------------------------------------- sdk hook

    fn environnement_vide() -> AwsEnv {
        AwsEnv::default()
    }

    #[test]
    fn le_crochet_sdk_ignore_un_paquet_non_reconnu() {
        for paquet in ["@ai-sdk/openai", "@ai-sdk/amazon-bedrock/mantle/", "", "@AI-SDK/Amazon-Bedrock"] {
            let event = BedrockSdkEvent::new(paquet);
            assert_eq!(BedrockPlugin::sdk_action(&event, &environnement_vide()), BedrockSdkAction::Ignore, "{}", paquet);
        }
    }

    #[test]
    fn le_crochet_sdk_reconnait_exactement_les_deux_paquets() {
        assert!(BedrockPlugin::reconnait_le_paquet("@ai-sdk/amazon-bedrock"));
        assert!(BedrockPlugin::reconnait_le_paquet("@ai-sdk/amazon-bedrock/mantle"));
        assert!(!BedrockPlugin::reconnait_le_paquet("@ai-sdk/amazon-bedrock/mantle2"));
        assert!(!BedrockPlugin::reconnait_le_paquet(""));
    }

    #[test]
    fn le_paquet_principal_demande_la_fabrique_create_amazon_bedrock() {
        let event = BedrockSdkEvent::new(AISDK_PACKAGE);

        match BedrockPlugin::sdk_action(&event, &environnement_vide()) {
            BedrockSdkAction::Create { package, factory, options, set_bearer_token_env, inject_credential_provider } => {
                assert_eq!(package, "@ai-sdk/amazon-bedrock");
                assert_eq!(factory, "createAmazonBedrock");
                assert_eq!(options.get("region"), Some(&json!(DEFAULT_REGION)));
                assert_eq!(set_bearer_token_env, None);
                assert!(inject_credential_provider.is_some(), "sans jeton ni chaine explicite, la chaine par defaut est injectee");
            }
            autre => panic!("le paquet principal doit demander une creation, pas {:?}", autre),
        }
    }

    #[test]
    fn le_paquet_mantle_demande_la_fabrique_create_bedrock_mantle() {
        let event = BedrockSdkEvent::new(MANTLE_PACKAGE);

        match BedrockPlugin::sdk_action(&event, &environnement_vide()) {
            BedrockSdkAction::Create { package, factory, .. } => {
                assert_eq!(package, "@ai-sdk/amazon-bedrock/mantle");
                assert_eq!(factory, "createBedrockMantle");
            }
            autre => panic!("le paquet mantle doit demander une creation, pas {:?}", autre),
        }
    }

    #[test]
    fn la_region_de_loption_gagne_sur_lenvironnement_et_le_defaut() {
        let mut event = BedrockSdkEvent::new(AISDK_PACKAGE);
        event.options.insert("region".to_string(), json!("eu-west-3"));

        match BedrockPlugin::sdk_action(&event, &environnement_vide()) {
            BedrockSdkAction::Create { options, .. } => {
                assert_eq!(options.get("region"), Some(&json!("eu-west-3")));
            }
            autre => panic!("{:?}", autre),
        }

        let mut env = environnement_vide();
        env.region = Some("ap-southeast-2".to_string());
        let event = BedrockSdkEvent::new(AISDK_PACKAGE);
        match BedrockPlugin::sdk_action(&event, &env) {
            BedrockSdkAction::Create { options, .. } => {
                assert_eq!(options.get("region"), Some(&json!("ap-southeast-2")));
            }
            autre => panic!("{:?}", autre),
        }
    }

    #[test]
    fn une_region_non_chaine_dans_les_options_est_ignoree() {
        // `typeof options.region === "string"` : un nombre ou un objet dans
        // les options ne compte pas, l'environnement prend le relais.
        let mut event = BedrockSdkEvent::new(AISDK_PACKAGE);
        event.options.insert("region".to_string(), json!(42));
        let mut env = environnement_vide();
        env.region = Some("eu-west-1".to_string());

        match BedrockPlugin::sdk_action(&event, &env) {
            BedrockSdkAction::Create { options, .. } => {
                assert_eq!(options.get("region"), Some(&json!("eu-west-1")));
            }
            autre => panic!("{:?}", autre),
        }
    }

    #[test]
    fn le_profil_de_loption_gagne_sur_lenvironnement() {
        let mut event = BedrockSdkEvent::new(AISDK_PACKAGE);
        event.options.insert("profile".to_string(), json!("prod"));
        let mut env = environnement_vide();
        env.profile = Some("dev".to_string());

        match BedrockPlugin::sdk_action(&event, &env) {
            BedrockSdkAction::Create { inject_credential_provider, .. } => {
                assert_eq!(
                    inject_credential_provider,
                    Some(CredentialProviderInjection { factory: "fromNodeProviderChain", profile: Some("prod".to_string()) })
                );
            }
            autre => panic!("{:?}", autre),
        }
    }

    #[test]
    fn sans_profil_ni_partout_la_chaine_d_identifiants_est_appatee_sans_argument() {
        let event = BedrockSdkEvent::new(AISDK_PACKAGE);

        match BedrockPlugin::sdk_action(&event, &environnement_vide()) {
            BedrockSdkAction::Create { inject_credential_provider, .. } => {
                assert_eq!(
                    inject_credential_provider,
                    Some(CredentialProviderInjection { factory: "fromNodeProviderChain", profile: None })
                );
            }
            autre => panic!("{:?}", autre),
        }
    }

    #[test]
    fn un_endpoint_en_option_est_copie_vers_baseurl_sans_etre_supprime() {
        // Le crochet sdk copie `endpoint` vers `baseURL` mais ne supprime pas
        // `endpoint` : seule la transformation du catalogue supprime sa cle.
        let mut event = BedrockSdkEvent::new(AISDK_PACKAGE);
        event.options.insert("endpoint".to_string(), json!("https://vpc.example.com"));

        match BedrockPlugin::sdk_action(&event, &environnement_vide()) {
            BedrockSdkAction::Create { options, .. } => {
                assert_eq!(options.get("baseURL"), Some(&json!("https://vpc.example.com")));
                assert_eq!(options.get("endpoint"), Some(&json!("https://vpc.example.com")));
            }
            autre => panic!("{:?}", autre),
        }
    }

    #[test]
    fn un_endpoint_non_chaine_ne_cree_pas_de_baseurl() {
        let mut event = BedrockSdkEvent::new(AISDK_PACKAGE);
        event.options.insert("endpoint".to_string(), json!(42));

        match BedrockPlugin::sdk_action(&event, &environnement_vide()) {
            BedrockSdkAction::Create { options, .. } => {
                assert!(options.get("baseURL").is_none());
            }
            autre => panic!("{:?}", autre),
        }
    }

    #[test]
    fn un_jeton_venu_des_options_demande_l_ecriture_de_la_variable_d_environnement() {
        // Le TS ecrit `process.env.AWS_BEARER_TOKEN_BEDROCK = bearerToken`
        // seulement si la variable n'existait pas deja.
        let mut event = BedrockSdkEvent::new(AISDK_PACKAGE);
        event.options.insert("bearerToken".to_string(), json!("jeton"));

        match BedrockPlugin::sdk_action(&event, &environnement_vide()) {
            BedrockSdkAction::Create { set_bearer_token_env, inject_credential_provider, .. } => {
                assert_eq!(set_bearer_token_env.as_deref(), Some("jeton"));
                assert!(inject_credential_provider.is_none(), "un jeton Bearer empeche l'injection de la chaine par defaut");
            }
            autre => panic!("{:?}", autre),
        }
    }

    #[test]
    fn un_jeton_deja_dans_lenvironnement_n_est_pas_reecrit() {
        let mut env = environnement_vide();
        env.bearer_token = Some("de-la-variable".to_string());
        let mut event = BedrockSdkEvent::new(AISDK_PACKAGE);
        event.options.insert("bearerToken".to_string(), json!("de-loption"));

        match BedrockPlugin::sdk_action(&event, &env) {
            BedrockSdkAction::Create { set_bearer_token_env, inject_credential_provider, .. } => {
                assert_eq!(set_bearer_token_env, None, "la variable existait deja, pas d'ecriture");
                assert!(inject_credential_provider.is_none());
            }
            autre => panic!("{:?}", autre),
        }
    }

    #[test]
    fn un_jeton_en_environnement_suffit_a_empecher_l_injection() {
        let mut env = environnement_vide();
        env.bearer_token = Some("de-la-variable".to_string());

        match BedrockPlugin::sdk_action(&BedrockSdkEvent::new(AISDK_PACKAGE), &env) {
            BedrockSdkAction::Create { set_bearer_token_env, inject_credential_provider, .. } => {
                assert_eq!(set_bearer_token_env, None);
                assert!(inject_credential_provider.is_none());
            }
            autre => panic!("{:?}", autre),
        }
    }

    #[test]
    fn une_chaine_d_identifiants_explicite_empeche_l_injection() {
        let mut event = BedrockSdkEvent::new(AISDK_PACKAGE);
        event.options.insert("credentialProvider".to_string(), json!({ "deja": "la" }));

        match BedrockPlugin::sdk_action(&event, &environnement_vide()) {
            BedrockSdkAction::Create { inject_credential_provider, .. } => {
                assert_eq!(inject_credential_provider, None);
            }
            autre => panic!("{:?}", autre),
        }
    }

    #[test]
    fn les_options_existantes_ne_sont_pas_perdues_et_l_evenement_n_est_pas_mute() {
        // La source clone les options (`{ ...evt.options }`) avant de les
        // muter : l'evenement d'origine doit rester intact.
        let mut event = BedrockSdkEvent::new(AISDK_PACKAGE);
        event.options.insert("apiKey".to_string(), json!("secret"));
        event.options.insert("endpoint".to_string(), json!("https://vpc.example.com"));
        let avant = event.clone();

        match BedrockPlugin::sdk_action(&event, &environnement_vide()) {
            BedrockSdkAction::Create { options, .. } => {
                assert_eq!(options.get("apiKey"), Some(&json!("secret")));
                assert_eq!(options.get("baseURL"), Some(&json!("https://vpc.example.com")));
            }
            autre => panic!("{:?}", autre),
        }
        assert_eq!(event, avant, "l'evenement d'origine ne doit pas etre modifie");
    }

    // --------------------------------------------------------- language hook

    #[test]
    fn le_crochet_language_resout_l_identifiant_avec_la_region_fournie() {
        assert_eq!(
            BedrockPlugin::language_model_id("anthropic.claude-3-5-sonnet", Some("us-east-1")),
            "us.anthropic.claude-3-5-sonnet"
        );
        // `region` absent : la source passe `process.env`, potentiellement
        // `undefined`, et le defaut interne `us-east-1` s'applique alors.
        assert_eq!(BedrockPlugin::language_model_id("anthropic.claude-3-5-sonnet", None), "us.anthropic.claude-3-5-sonnet");
    }

    #[test]
    fn le_crochet_language_mantle_choisit_chat_ou_responses() {
        assert_eq!(BedrockPlugin::mantle_method("openai.gpt-oss-safeguard-120b"), MantleMethod::Chat);
        assert_eq!(BedrockPlugin::mantle_method("openai.gpt-oss-120b"), MantleMethod::Responses);
    }

    // ------------------------------------------------------- noms de champs

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        let mut provider = fournisseur_bedrock();
        provider.request.body.insert("endpoint".to_string(), json!("https://vpc.example.com"));

        let json = serde_json::to_value(&provider).unwrap();

        assert_eq!(json["id"], "bedrock");
        assert_eq!(json["api"]["type"], "aisdk");
        assert_eq!(json["api"]["package"], AISDK_PACKAGE);
        assert!(json["api"].get("url").is_none(), "l'url absente ne doit pas etre serialisee");
        assert_eq!(json["request"]["body"]["endpoint"], "https://vpc.example.com");
        assert!(json.get("Id").is_none());
        assert!(json.get("Api").is_none());
        assert!(json.get("Request").is_none());
        assert!(json["request"].get("Body").is_none());
    }

    #[test]
    fn un_fournisseur_avec_url_survit_a_l_aller_retour() {
        let mut provider = fournisseur_bedrock();
        provider.request.body.insert("endpoint".to_string(), json!("https://vpc.example.com"));

        let json = serde_json::to_value(&provider).unwrap();
        let relu: BedrockProvider = serde_json::from_value(json).unwrap();
        assert_eq!(relu, provider);

        let provider_avec_url = BedrockProvider::new(
            "bedrock",
            BedrockApi::Aisdk { package: AISDK_PACKAGE.to_string(), url: Some("https://vpc.example.com".to_string()) },
        );
        let json = serde_json::to_value(&provider_avec_url).unwrap();
        assert_eq!(json["api"]["url"], "https://vpc.example.com");
        let relu: BedrockProvider = serde_json::from_value(json).unwrap();
        assert_eq!(relu, provider_avec_url);
    }

    #[test]
    fn un_api_native_ne_serialize_ni_package_ni_url() {
        let provider = BedrockProvider::new("gateway", BedrockApi::native());

        let json = serde_json::to_value(&provider).unwrap();

        assert_eq!(json["api"], json!({ "type": "native" }));
        assert!(json["api"].get("package").is_none(), "une api native n'a pas de paquet");
        assert!(json["api"].get("url").is_none());
    }

    #[test]
    fn un_type_d_api_inconnu_est_refuse() {
        assert!(serde_json::from_value::<BedrockApi>(json!({ "type": "graphql" })).is_err());
        assert!(serde_json::from_value::<BedrockApi>(json!({ "package": AISDK_PACKAGE })).is_err());
    }

    #[test]
    fn un_fournisseur_se_relit_depuis_le_json_du_catalogue_en_ignorant_les_champs_etrangers() {
        let json = json!({
            "id": "bedrock",
            "name": "Amazon Bedrock",
            "disabled": false,
            "integrationID": "int-1",
            "api": { "type": "aisdk", "package": AISDK_PACKAGE, "settings": {} },
            "request": { "headers": {}, "body": { "endpoint": "https://vpc.example.com" } }
        });

        let provider: BedrockProvider = serde_json::from_value(json).unwrap();

        assert_eq!(provider.id, "bedrock");
        assert_eq!(provider.api.package(), Some(AISDK_PACKAGE));
        assert_eq!(provider.request.body.get("endpoint"), Some(&json!("https://vpc.example.com")));
    }

    #[test]
    fn le_brouillon_du_catalogue_serialize_sous_la_cle_providers() {
        let draft = BedrockCatalogDraft::new(vec![fournisseur_bedrock()]);

        let json = serde_json::to_value(&draft).unwrap();

        assert_eq!(json["providers"][0]["id"], "bedrock");
        assert!(json.get("provider").is_none(), "la cle se lit au pluriel");
    }

    #[test]
    fn un_evenement_sdk_se_relit_depuis_le_json_du_typescript() {
        let json = json!({
            "package": AISDK_PACKAGE,
            "options": { "profile": "prod", "bearerToken": "jeton", "endpoint": "https://vpc.example.com" }
        });

        let event: BedrockSdkEvent = serde_json::from_value(json.clone()).unwrap();

        assert_eq!(serde_json::to_value(&event).unwrap(), json);
        assert_eq!(event.package, AISDK_PACKAGE);
        assert_eq!(event.options.get("bearerToken"), Some(&json!("jeton")));
    }

    #[test]
    fn un_evenement_sdk_sans_paquet_est_refuse() {
        assert!(serde_json::from_value::<BedrockSdkEvent>(json!({ "options": {} })).is_err());
    }

    #[test]
    fn l_instantane_d_environnement_serialize_avec_les_cles_camel_case() {
        let env = AwsEnv {
            profile: Some("prod".to_string()),
            region: Some("eu-west-3".to_string()),
            bearer_token: Some("jeton".to_string()),
            container_credentials: true,
        };

        let json = serde_json::to_value(&env).unwrap();

        assert_eq!(json["profile"], "prod");
        assert_eq!(json["region"], "eu-west-3");
        assert_eq!(json["bearerToken"], "jeton", "la cle doit rester en camelCase, comme process.env ne porte pas de nom Rust");
        assert_eq!(json["containerCredentials"], true);
        let relu: AwsEnv = serde_json::from_value(json).unwrap();
        assert_eq!(relu, env);
    }

    #[test]
    fn l_instantane_d_environnement_vide_n_serialize_pas_les_options_absentes() {
        let json = serde_json::to_value(AwsEnv::default()).unwrap();

        assert!(json.get("profile").is_none());
        assert!(json.get("bearerToken").is_none());
    }

    #[test]
    fn les_litteraux_du_plugin_sont_ceux_du_typescript() {
        assert_eq!(BedrockPlugin::ID, "amazon-bedrock");
        assert_eq!(AISDK_TYPE, "aisdk");
        assert_eq!(AISDK_PACKAGE, "@ai-sdk/amazon-bedrock");
        assert_eq!(MANTLE_PACKAGE, "@ai-sdk/amazon-bedrock/mantle");
        assert_eq!(AISDK_FACTORY, "createAmazonBedrock");
        assert_eq!(MANTLE_FACTORY, "createBedrockMantle");
        assert_eq!(CREDENTIAL_FACTORY, "fromNodeProviderChain");
        assert_eq!(AWS_PROFILE_ENV, "AWS_PROFILE");
        assert_eq!(AWS_REGION_ENV, "AWS_REGION");
        assert_eq!(AWS_BEARER_TOKEN_ENV, "AWS_BEARER_TOKEN_BEDROCK");
        assert_eq!(AWS_CONTAINER_RELATIVE_URI_ENV, "AWS_CONTAINER_CREDENTIALS_RELATIVE_URI");
        assert_eq!(AWS_CONTAINER_FULL_URI_ENV, "AWS_CONTAINER_CREDENTIALS_FULL_URI");
        assert_eq!(DEFAULT_REGION, "us-east-1");
        assert_eq!(ENDPOINT_BODY_KEY, "endpoint");
        assert_eq!(BASE_URL_OPTION_KEY, "baseURL");
        assert_eq!(JAPAN_REGION, "ap-northeast-1");
    }
}
