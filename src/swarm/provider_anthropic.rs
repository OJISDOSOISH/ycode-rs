//! Portage Rust de `opencode/packages/core/src/plugin/provider/anthropic.ts`.
//!
//! La source fait vingt-sept lignes et n'exporte qu'une chose,
//! `AnthropicPlugin`, construit par `define({ id, effect })`. Le `define` est
//! une fonction qui rend son argument sans le modifier
//! (`packages/core/src/plugin/internal.ts`), donc tout ce qui compte est
//! l'objet `{ id: "anthropic", effect }`.
//!
//! L'effet enregistre deux callbacks, dans cet ordre :
//!
//! 1. `ctx.catalog.transform(...)` : a chaque construction de catalogue,
//!    ajoute l'en-tete `anthropic-beta` a tout fournisseur dont l'API est de
//!    type `aisdk` **et** dont le paquet est `@ai-sdk/anthropic`.
//! 2. `ctx.aisdk.sdk(...)` : quand un SDK est demande pour le paquet
//!    `@ai-sdk/anthropic`, construit la fabrique avec `createAnthropic`.
//!
//! Ce que la source **ne** declare pas, et que ce fichier declare quand meme :
//! les formes `ProviderV2Info`, `ProviderApi`, `ProviderRequest`,
//! `CatalogProviderRecord` et `SdkHookEvent` viennent d'autres paquets
//! (`@opencode-ai/sdk/v2/types` et `@opencode-ai/plugin/v2/effect`), absents
//! de ce lot. Ce sont donc des formes locales, minimales : elles ne gardent que
//! ce que les deux callbacks lisent ou ecrivent, et chaque champ laisse de
//! cote est signale dans sa documentation. Si ces paquets sont ports plus tard,
//! ces formes doivent ceder la place a leurs.
//!
//! Ce qui n'a pas de traduction directe :
//!
//! - `Effect.fn` et `yield*` n'existent pas en Rust. L'enregistrement d'un
//!   callback devient un appel de fonction que l'appelant fait au moment ou
//!   l'effet aurait ete execute. Voir `catalog_transform` et `apply_aisdk_sdk`.
//! - `await import("@ai-sdk/anthropic")` n'a aucun equivalent : le crate ne
//!   charge pas de JavaScript a l'execution. Voir `AnthropicSdk`.
//!
//! Pieges a ecarter, releves dans la source :
//!
//! - **Aucun ternaire `?` ni coalescent `??`.** Les deux comparaisons de la
//!   source sont des `!==` sur des chaines, donc des egalites strictes : une
//!   chaine vide est simplement une chaine vide, elle ne se transforme ni en
//!   "absente" ni en "presente". Rien a traduire de ce cote-la.
//! - **Un nom de champ en majuscules : `integrationID`**, dans `ProviderV2Info`.
//!   Le nom Rust est `integration_id`, avec un `#[serde(rename)]` explicite.
//!   C'est le piege du lot sur ce fichier.
//! - **`type` et `package` sont des mots cles Rust.** Les champs Rust sont
//!   `r#type` (dans `ProviderApi`, via le tag) et `r#package`
//!   (`SdkHookEvent`). Le nom de la cle JSON, lui, reste `type` et `package`.
//! - **L'API est une union AVEC tag.** `ProviderApi = ProviderAisdk |
//!   ProviderNative` se distingue par un champ `type` a la valeur `"aisdk"` ou
//!   `"native"`. D'ou des variantes de type structure, et surtout **pas** des
//!   variantes `newtype` autour d'une structure portant elle-meme un champ
//!   `type` : serde ecrirait alors deux fois la cle `type` dans le JSON.
//!
//! Aucun comportement n'est invente. Ce que la source ne fait pas, ce module ne
//! le fait pas : en particulier il ne teste jamais le champ `disabled`, donc
//! un fournisseur desactive recoit l'en-tete comme les autres.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `id: "anthropic"`, l'identifiant enregistre par le registre de plugins.
///
/// `PluginInternal.add` ne lit que `id` et `effect` de l'objet passe a
/// `define`, donc cet identifiant est la seule donnee de l'export qui ne vit
/// pas dans les deux callbacks.
pub const PLUGIN_ID: &str = "anthropic";

/// Le paquet npm que ce plugin reconnait, aussi bien dans `evt.package` que
/// dans `provider.api.package`.
pub const ANTHROPIC_SDK_PACKAGE: &str = "@ai-sdk/anthropic";

/// La cle d'en-tete ajoutee par le callback de catalogue.
///
/// Volontairement un nom **avec** un tiret, comme dans la source. Ce n'est pas
/// un identifiant Rust, c'est une cle d'objet `headers`.
pub const ANTHROPIC_BETA_HEADER: &str = "anthropic-beta";

/// La valeur exacte ecrite sous `anthropic-beta`.
///
/// Deux drapeaux separes par une virgule, sans espace. Ne pas reordonner, ne
/// pas mettre d'espace autour de la virgule : la valeur est comparee par le
/// service Anthropic.
pub const ANTHROPIC_BETA_VALUE: &str =
    "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14";

/// La valeur de `type` de la variante `Aisdk` de `ProviderApi`.
pub const API_TYPE_AISDK: &str = "aisdk";

/// La valeur de `type` de la variante `Native` de `ProviderApi`.
pub const API_TYPE_NATIVE: &str = "native";

/// Forme libre equivalente du `{ [key: string]: unknown }` du TypeScript.
///
/// `BTreeMap` plutot que `HashMap` parce que la table de conversion du projet
/// impose des types ordonnes et deterministes, et qu'un catalogue compare
/// parfois par ordre de cle.
pub type JsonMap = BTreeMap<String, Value>;

/// La partie `api` d'un `ProviderV2Info`.
///
/// En TS : `export type ProviderApi = ProviderAisdk | ProviderNative`.
///
/// C'est une union **avec** tag : chaque variante porte un champ `type` valant
/// soit `"aisdk"`, soit `"native"`. D'ou `#[serde(tag = "type")]` pose sur des
/// variantes de type structure. Le nom de chaque variante est ramene
/// explicitement en minuscules, parce que serde mettrait sinon `Aisdk` et
/// `Native` dans le JSON, avec une majuscule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProviderApi {
    /// `ProviderAisdk` : un paquet `@ai-sdk/...` pilote par le SDK AI.
    #[serde(rename = "aisdk")]
    Aisdk {
        /// Paquet npm, par exemple `@ai-sdk/anthropic`.
        package: String,
        /// Point de terminaison, absent quand le SDK a sa valeur par defaut.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        /// Options libres du paquet. Facultatif dans la variante `Aisdk`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        settings: Option<JsonMap>,
    },
    /// `ProviderNative` : appel HTTP direct, sans SDK.
    #[serde(rename = "native")]
    Native {
        /// Point de terminaison, facultatif.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        /// Options libres. Ici le champ est **obligatoire** dans le
        /// TypeScript, a la difference de la variante `Aisdk`. C'est une
        /// inegalite de la source, pas une approximation : elle est conservee.
        settings: JsonMap,
    },
}

impl ProviderApi {
    /// Le paquet npm, ou `None` pour la variante `Native` qui n'en a pas.
    ///
    /// Cette fonction n'existe pas en tant telle dans la source. Elle sert a
    /// rendre le second `continue` du callback de catalogue lisible, et a
    /// eviter d'ecrire un `match` dans un predicat. Elle ne fait qu'acceder au
    /// champ, sans ajouter de condition.
    pub fn package(&self) -> Option<&str> {
        match self {
            ProviderApi::Aisdk { package, .. } => Some(package.as_str()),
            ProviderApi::Native { .. } => None,
        }
    }
}

/// `ProviderRequest` : ce qui est ajoute a chaque appel d'un fournisseur.
///
/// En TS : `export type ProviderRequest = { headers: ..., body: ... }`. Les
/// deux champs sont obligatoires, donc aucun `#[serde(default)]` ici.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequest {
    /// En-tetes HTTP, cles et valeurs en texte. C'est le seul champ que le
    /// plugin modifie.
    pub headers: BTreeMap<String, String>,
    /// Corps HTTP, forme libre. Le plugin n'y touche jamais.
    pub body: JsonMap,
}

impl Default for ProviderRequest {
    /// Un `ProviderRequest` vide : aucun en-tete, corps vide.
    ///
    /// La source n'a pas de valeur par defaut pour ce type-la, c'est un ajout
    /// de confort pour les tests et pour le code qui construit un fournisseur.
    /// Le resultat est exactement la forme qu'envoie le serveur quand tout est
    /// vide.
    fn default() -> Self {
        Self {
            headers: BTreeMap::new(),
            body: JsonMap::new(),
        }
    }
}

/// La fiche d'un fournisseur dans le catalogue.
///
/// En TS : `ProviderV2Info`, depuis `@opencode-ai/sdk/v2/types`.
///
/// Cette forme n'est pas serialisee par le plugin lui-meme : elle vient du
/// serveur, elle est modifiee en memoire, et c'est pourquoi les noms de champs
/// comptent autant. Le test `les_noms_de_champs_serialises_sont_ceux_du_typescript`
/// verrouille le cas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderV2Info {
    /// Identifiant du fournisseur. C'est la valeur que `catalog.provider.update`
    /// recoit dans la source.
    pub id: String,
    /// Integration d'origine du fournisseur, absente quand il n'est rattache a
    /// aucune integration.
    ///
    /// **Piege de nom.** Le TypeScript ecrit `integrationID`, deux majuscules.
    /// Le nom Rust est `integration_id`, et le renommage explicite est
    /// obligatoire. Sans lui, le champ disparaitrait a l'echange.
    #[serde(rename = "integrationID", default, skip_serializing_if = "Option::is_none")]
    pub integration_id: Option<String>,
    /// Nom affiche. Jamais lu par le plugin.
    pub name: String,
    /// Vrai si le fournisseur est desactive.
    ///
    /// Le plugin ne teste **jamais** ce champ : un fournisseur desactive
    /// recoit l'en-tete exactement comme un fournisseur actif.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    /// Nature de l'API. C'est le seul champ que le callback de catalogue
    /// regarde, pour decider si le fournisseur est concerne.
    pub api: ProviderApi,
    /// En-tetes et corps de requete. Le callback y ecrit l'en-tete
    /// `anthropic-beta`.
    pub request: ProviderRequest,
}

/// Un element renvoye par `catalog.provider.list()`.
///
/// Le TypeScript ajoute `models: ReadonlyMap<string, ModelV2Info>`. Ce champ
/// n'est **pas** represente ici : le plugin ne le lit jamais, et
/// `ModelV2Info` n'est porte par aucun fichier de ce lot. L'omettre evite
/// d'inventer un type pour un champ mort.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogProviderRecord {
    /// La fiche du fournisseur, seule partie lue par le plugin.
    pub provider: ProviderV2Info,
}

/// La partie `provider` d'un `CatalogDraft`, restreinte a ce que le plugin
/// utilise.
///
/// Le TypeScript declare aussi `get`, `remove`, et tout un objet `model` avec
/// `get`, `update`, `remove` et `default`. Aucun n'est appele par ce plugin,
/// donc aucun n'est porte. `get` est pourtant fourni ici parce qu'il rend le
/// type lisible depuis l'exterieur ; `remove` et l'objet `model` se rajouteront
/// entiers s'ils servent un jour.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CatalogProviderStore {
    /// Les fournisseurs, dans l'ordre d'insertion.
    ///
    /// Le TypeScript renvoie une liste et cherche par identifiant. Un `Vec`
    /// conserve l'ordre d'origine, ce qui garde le parcours lisible. L'ordre
    /// n'a aucune influence sur le resultat du plugin, qui touche tous les
    /// fournisseurs concernes quel que soit leur rang.
    entries: Vec<CatalogProviderRecord>,
}

impl CatalogProviderStore {
    /// Un catalogue vide.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Ajoute un fournisseur a la fin de la liste, comme le ferait une
    /// construction de catalogue cote serveur.
    pub fn push(&mut self, provider: ProviderV2Info) {
        self.entries.push(CatalogProviderRecord { provider });
    }

    /// La liste rendue par `catalog.provider.list()`, dans l'ordre d'insertion.
    pub fn list(&self) -> &[CatalogProviderRecord] {
        &self.entries
    }

    /// La fiche du fournisseur `provider_id`, ou `None` s'il est inconnu.
    ///
    /// Present pour la lisibilite, pas utilise par le plugin.
    pub fn get(&self, provider_id: &str) -> Option<&CatalogProviderRecord> {
        self.entries
            .iter()
            .find(|item| item.provider.id == provider_id)
    }

    /// Applique `update` au fournisseur `provider_id`.
    ///
    /// Comme dans la source, un identifiant inconnu ne fait rien et ne signale
    /// rien : la fonction rend `()`.
    pub fn update<F>(&mut self, provider_id: &str, update: F)
    where
        F: FnOnce(&mut ProviderV2Info),
    {
        if let Some(item) = self
            .entries
            .iter_mut()
            .find(|item| item.provider.id == provider_id)
        {
            update(&mut item.provider);
        }
    }
}

/// L'evenement recu par le callback enregistre sur `ctx.aisdk.sdk`.
///
/// En TS : `{ readonly model: ModelV2Info, readonly package: string, readonly
/// options: Record<string, any>, sdk?: any }`.
///
/// Le champ `model` n'est **pas** represente : le plugin ne le lit pas, et
/// `ModelV2Info` n'est porte par aucun fichier de ce lot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkHookEvent {
    /// Paquet demande, par exemple `@ai-sdk/anthropic`.
    ///
    /// `package` est un mot cle de Rust, d'ou le nom brut. Le renommage est
    /// pose malgre tout, pour que le nom de la cle soit visible dans le
    /// fichier plutot que deduit du nom brut.
    #[serde(rename = "package")]
    pub r#package: String,
    /// Options destinees au constructeur du SDK, transmises telles quelles.
    pub options: JsonMap,
    /// Fabrique construite. Absent tant qu'aucun plugin ne l'a remplie, et le
    /// plugin ne touche pas a une valeur deja presente.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk: Option<AnthropicSdk>,
}

/// Ce que le TypeScript met dans `evt.sdk`.
///
/// En TS, `sdk?: any` recoit `mod.createAnthropic(evt.options)`, c'est a dire un
/// objet JavaScript construit a partir du paquet `@ai-sdk/anthropic`. Ce module
/// est charge par un `import()` dynamique, ce qui n'a pas d'equivalent Rust :
/// un crate ne charge pas de JavaScript a l'execution. On garde donc le paquet
/// et les options qui auraient ete passees au constructeur, ce qui est
/// exactement toute l'information que la source y met. Le client reel, lui, ne
/// peut pas exister tant que le portage n'a pas de pont vers le SDK AI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnthropicSdk {
    /// Paquet qui aurait produit la fabrique. Toujours
    /// `ANTHROPIC_SDK_PACKAGE` tant que ce module est le seul a en construire
    /// une.
    pub package: String,
    /// Options qui auraient ete passees a `createAnthropic`.
    pub options: JsonMap,
}

/// L'export `AnthropicPlugin`.
///
/// En TS, c'est `{ id: "anthropic", effect }`. `effect` enregistre deux
/// callbacks, et il n'y a rien d'autre a savoir de l'objet. Ici l'enregistrement
/// n'existe pas : `catalog_transform` s'appelle quand le catalogue se
/// construit, et `apply_aisdk_sdk` quand un SDK est demande.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnthropicPlugin;

impl AnthropicPlugin {
    /// L'identifiant enregistre par le registre de plugins.
    pub const ID: &'static str = PLUGIN_ID;

    /// Le paquet reconnu par ce plugin.
    pub const PACKAGE: &'static str = ANTHROPIC_SDK_PACKAGE;
}

/// Le callback enregistre par `ctx.catalog.transform`, en Rust pur.
///
/// La source, ligne pour ligne :
///
/// ```text
/// for (const item of evt.provider.list()) {
///   if (item.provider.api.type !== "aisdk") continue
///   if (item.provider.api.package !== "@ai-sdk/anthropic") continue
///   evt.provider.update(item.provider.id, (provider) => {
///     provider.request.headers["anthropic-beta"] =
///       "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14"
///   })
/// }
/// ```
///
/// Les deux `continue` sont compiles ici en un seul predicat : la variante
/// `Native` est rejetee par le premier test, et une variante `Aisdk` dont le
/// paquet ne correspond pas l'est par le second. Aucune autre condition
/// n'intervient, en particulier `disabled` n'est pas teste.
///
/// Les identifiants retenus sont collectes avant toute ecriture, parce qu'on
/// modifie le catalogue pendant qu'on le parcourt.
pub fn catalog_transform(draft: &mut CatalogProviderStore) {
    let concerne: Vec<String> = draft
        .list()
        .iter()
        .filter(|item| {
            item.provider.api.package() == Some(ANTHROPIC_SDK_PACKAGE)
        })
        .map(|item| item.provider.id.clone())
        .collect();

    for provider_id in concerne {
        draft.update(&provider_id, |provider| {
            provider
                .request
                .headers
                .insert(ANTHROPIC_BETA_HEADER.to_string(), ANTHROPIC_BETA_VALUE.to_string());
        });
    }
}

/// `true` si ce plugin reconnait le paquet donne.
///
/// Isole le test de la source, `evt.package !== ANTHROPIC_SDK_PACKAGE`, pour
/// qu'il soit verifiable seul. C'est une egalite de chaine stricte : une
/// chaine vide ne correspond pas et n'est pas traitee comme une valeur
/// absente.
pub fn handles_package(package: &str) -> bool {
    package == ANTHROPIC_SDK_PACKAGE
}

/// Le callback enregistre par `ctx.aisdk.sdk`, en Rust pur.
///
/// La source, ligne pour ligne :
///
/// ```text
/// if (evt.package !== "@ai-sdk/anthropic") return
/// const mod = yield* Effect.promise(() => import("@ai-sdk/anthropic"))
/// evt.sdk = mod.createAnthropic(evt.options)
/// ```
///
/// Le `return` de la source sort simplement de la fonction, sans erreur : un
/// paquet qui ne correspond pas laisse `evt.sdk` intact, y compris si une
/// fabrique y etait deja. `evt.options` est transmis tel quel, sans copie
/// selective ni renommage de cle.
///
/// Seule la ligne `createAnthropic(evt.options)` n'est pas litterale : le
/// `import()` dynamique n'a pas d'equivalent Rust. Elle est remplacee par la
/// construction d'un `AnthropicSdk` qui retient le paquet et les options.
pub fn apply_aisdk_sdk(evt: &mut SdkHookEvent) {
    if !handles_package(&evt.r#package) {
        return;
    }

    evt.sdk = Some(AnthropicSdk {
        package: ANTHROPIC_SDK_PACKAGE.to_string(),
        options: evt.options.clone(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un fournisseur `aisdk` portant le paquet attendu, avec un en-tete
    /// prealable pour verifier qu'il survit a l'ajout.
    fn fournisseur_anthropic(id: &str) -> ProviderV2Info {
        ProviderV2Info {
            id: id.to_string(),
            integration_id: None,
            name: id.to_string(),
            disabled: None,
            api: ProviderApi::Aisdk {
                package: ANTHROPIC_SDK_PACKAGE.to_string(),
                url: None,
                settings: None,
            },
            request: ProviderRequest {
                headers: BTreeMap::from([("x-essai".to_string(), "1".to_string())]),
                body: JsonMap::new(),
            },
        }
    }

    /// Un fournisseur `aisdk` d'un autre paquet, qui ne doit jamais etre touche.
    fn fournisseur_etranger(id: &str, package: &str) -> ProviderV2Info {
        ProviderV2Info {
            api: ProviderApi::Aisdk {
                package: package.to_string(),
                url: None,
                settings: None,
            },
            ..fournisseur_anthropic(id)
        }
    }

    /// Un fournisseur `native`, qui n'a pas de champ `package` du tout.
    fn fournisseur_natif(id: &str) -> ProviderV2Info {
        ProviderV2Info {
            api: ProviderApi::Native {
                url: Some("https://exemple.test".to_string()),
                settings: JsonMap::new(),
            },
            ..fournisseur_anthropic(id)
        }
    }

    /// L'en-tete du fournisseur `id`, ou `None` s'il est absent.
    fn entete(draft: &CatalogProviderStore, id: &str) -> Option<String> {
        // Le chemin est `record.provider.request.headers` : la fiche du
        // catalogue expose le fournisseur sous le nom `provider`, et c'est
        // `ProviderV2Info` qui porte le champ `request`.
        draft
            .get(id)?
            .provider
            .request
            .headers
            .get(ANTHROPIC_BETA_HEADER)
            .cloned()
    }

    /// Un catalogue vide ne leve rien et n'ajoute aucun en-tete.
    #[test]
    fn un_catalogue_vide_najoute_aucun_en_tete() {
        let mut draft = CatalogProviderStore::new();

        catalog_transform(&mut draft);

        assert!(draft.list().is_empty());
    }

    /// Un seul fournisseur, du bon type et du bon paquet, recoit exactement la
    /// bonne cle et la bonne valeur, et ses autres en-tetes survivent.
    #[test]
    fn un_seul_fournisseur_du_bon_paquet_recoit_len_tete() {
        let mut draft = CatalogProviderStore::new();
        draft.push(fournisseur_anthropic("anthropic"));

        catalog_transform(&mut draft);

        assert_eq!(
            entete(&draft, "anthropic").as_deref(),
            Some(ANTHROPIC_BETA_VALUE)
        );
        assert_eq!(
            draft
                .get("anthropic")
                .unwrap()
                .provider
                .request
                .headers
                .get("x-essai")
                .map(String::as_str),
            Some("1")
        );
        assert_eq!(draft.list().len(), 1);
    }

    /// Un fournisseur `native` et un `aisdk` d'un autre paquet sont ignores,
    /// alors qu'ils portent tous les deux le bon `id` apparent.
    #[test]
    fn les_fournisseurs_d_un_autre_type_ou_d_un_autre_paquet_sont_ignores() {
        let mut draft = CatalogProviderStore::new();
        draft.push(fournisseur_etranger("google", "@ai-sdk/google"));
        draft.push(fournisseur_natif("natif"));

        catalog_transform(&mut draft);

        assert_eq!(entete(&draft, "google"), None);
        assert_eq!(entete(&draft, "natif"), None);
        assert!(draft.list().iter().all(|item| item
            .provider
            .request
            .headers
            .get(ANTHROPIC_BETA_HEADER)
            .is_none()));
    }

    /// Dans une liste ou les fournisseurs sont presents dans les deux sens,
    /// seuls les deux concernes sont modifies, et l'ordre de la liste est
    /// laisse intact.
    #[test]
    fn seuls_les_fournisseurs_concernes_sont_modifies_dans_une_liste_melangee() {
        let mut draft = CatalogProviderStore::new();
        draft.push(fournisseur_etranger("z-google", "@ai-sdk/google"));
        draft.push(fournisseur_anthropic("m-bedrock-partenaire"));
        draft.push(fournisseur_natif("a-natif"));
        draft.push(fournisseur_anthropic("b-anthropic"));

        catalog_transform(&mut draft);

        assert_eq!(entete(&draft, "z-google"), None);
        assert_eq!(entete(&draft, "a-natif"), None);
        assert_eq!(
            entete(&draft, "m-bedrock-partenaire").as_deref(),
            Some(ANTHROPIC_BETA_VALUE)
        );
        assert_eq!(
            entete(&draft, "b-anthropic").as_deref(),
            Some(ANTHROPIC_BETA_VALUE)
        );

        let ordre: Vec<&str> = draft
            .list()
            .iter()
            .map(|item| item.provider.id.as_str())
            .collect();
        assert_eq!(ordre, ["z-google", "m-bedrock-partenaire", "a-natif", "b-anthropic"]);
    }

    /// Un en-tete `anthropic-beta` deja present est ecrase, pas complete, et le
    /// nombre d'en-tetes du fournisseur ne change pas.
    #[test]
    fn un_en_tete_deja_present_est_ecrase() {
        let mut fournisseur = fournisseur_anthropic("anthropic");
        fournisseur.request.headers.insert(
            ANTHROPIC_BETA_HEADER.to_string(),
            "ancien-drapeau".to_string(),
        );
        let mut draft = CatalogProviderStore::new();
        draft.push(fournisseur);

        catalog_transform(&mut draft);

        assert_eq!(
            entete(&draft, "anthropic").as_deref(),
            Some(ANTHROPIC_BETA_VALUE)
        );
        assert_eq!(draft.get("anthropic").unwrap().provider.request.headers.len(), 2);
    }

    /// Le plugin ne teste jamais `disabled` : un fournisseur desactive recoit
    /// l'en-tete comme un fournisseur actif. C'est une caracteristique de la
    /// source, pas un oubli.
    #[test]
    fn un_fournisseur_desactive_recoit_len_tete_comme_les_autres() {
        let mut desactive = fournisseur_anthropic("anthropic-off");
        desactive.disabled = Some(true);
        let mut draft = CatalogProviderStore::new();
        draft.push(desactive);

        catalog_transform(&mut draft);

        assert_eq!(
            entete(&draft, "anthropic-off").as_deref(),
            Some(ANTHROPIC_BETA_VALUE)
        );
    }

    /// Actualiser un identifiant qui n'existe pas ne fait rien et ne provoque
    /// pas de panique, comme dans la source.
    #[test]
    fn mettre_a_jour_un_identifiant_inconnu_ne_fait_rien() {
        let mut draft = CatalogProviderStore::new();
        draft.push(fournisseur_anthropic("anthropic"));

        draft.update("pas-la", |provider| {
            provider.request.headers.insert("peu-importe".to_string(), "x".to_string());
        });

        assert_eq!(entete(&draft, "anthropic"), None);
        assert_eq!(draft.list().len(), 1);
        assert!(draft.get("pas-la").is_none());
    }

    /// Un paquet qui ne correspond pas laisse la fabrique absente, et les
    /// options intactes.
    #[test]
    fn un_paquet_qui_ne_correspond_pas_laisse_la_fabrique_absente() {
        let mut evt = SdkHookEvent {
            r#package: "@ai-sdk/google".to_string(),
            options: JsonMap::from([("temperature".to_string(), Value::from(0.7))]),
            sdk: None,
        };

        apply_aisdk_sdk(&mut evt);

        assert_eq!(evt.sdk, None);
        assert_eq!(evt.options.len(), 1);
    }

    /// Le bon paquet construit la fabrique, avec les options transmises telles
    /// quelles, y compris leurs cles et leurs chaines vides.
    #[test]
    fn le_paquet_anthropic_construit_la_fabrique_avec_les_options_telles_quelles() {
        let mut options = JsonMap::new();
        options.insert("baseURL".to_string(), Value::from("https://exemple.test"));
        options.insert("apiKey".to_string(), Value::from(""));

        let mut evt = SdkHookEvent {
            r#package: ANTHROPIC_SDK_PACKAGE.to_string(),
            options,
            sdk: None,
        };

        apply_aisdk_sdk(&mut evt);

        let sdk = evt.sdk.expect("la fabrique doit etre construite");
        assert_eq!(sdk.package, ANTHROPIC_SDK_PACKAGE);
        assert_eq!(sdk.options.get("baseURL").and_then(Value::as_str), Some("https://exemple.test"));
        // Une chaine vide est une chaine vide : elle n'est pas traitee comme
        // une valeur absente.
        assert_eq!(sdk.options.get("apiKey").and_then(Value::as_str), Some(""));
    }

    /// Une chaine vide n'est pas un paquet valide : le test est une egalite
    /// stricte, donc elle est simplement rejetee, sans etre confondue avec une
    /// valeur manquante.
    #[test]
    fn un_paquet_vide_ne_construit_pas_la_fabrique() {
        assert!(!handles_package(""));
        assert!(!handles_package(" @ai-sdk/anthropic"));
        assert!(!handles_package("@ai-sdk/anthropic "));
        assert!(handles_package(ANTHROPIC_SDK_PACKAGE));

        let mut evt = SdkHookEvent {
            r#package: String::new(),
            options: JsonMap::new(),
            sdk: None,
        };
        apply_aisdk_sdk(&mut evt);
        assert_eq!(evt.sdk, None);
    }

    /// Les noms de champs de `ProviderV2Info` sont ceux du TypeScript. Le test
    /// verrouille `integrationID`, qui s'ecrit avec deux majuscules cote
    /// TypeScript et qui disparaitrait si le renommage sautait.
    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        let mut provider = fournisseur_anthropic("anthropic");
        provider.integration_id = Some("int-anthropic".to_string());
        provider.disabled = Some(false);

        // L'en-tete `anthropic-beta` n'est ecrit que par le callback de
        // catalogue : sans passer par lui, `headers` ne contient que
        // l'en-tete prealable du fournisseur de test.
        let mut draft = CatalogProviderStore::new();
        draft.push(provider);
        catalog_transform(&mut draft);
        let provider = draft.get("anthropic").unwrap().provider.clone();

        let json = serde_json::to_value(&provider).unwrap();
        let objet = json.as_object().unwrap();

        assert_eq!(objet.len(), 6);
        for nom in ["id", "integrationID", "name", "disabled", "api", "request"] {
            assert!(objet.contains_key(nom), "champ absent du JSON : {nom}");
        }
        assert!(!objet.contains_key("integrationId"));
        assert!(!objet.contains_key("integration_id"));

        // Le tag de l'union sort sous le nom `type`, en minuscules, et une
        // seule fois : deux cles seulement pour la variante Aisdk, puisque
        // `url` et `settings` sont absents.
        let api = objet.get("api").and_then(Value::as_object).unwrap();
        assert_eq!(api.len(), 2);
        assert_eq!(api.get("type").and_then(Value::as_str), Some(API_TYPE_AISDK));
        assert_eq!(api.get("package").and_then(Value::as_str), Some(ANTHROPIC_SDK_PACKAGE));

        // L'en-tete du plugin porte bien la cle avec son tiret, sous `headers`,
        // a cote de l'en-tete prealable qui survit.
        let headers = objet
            .get("request")
            .and_then(Value::as_object)
            .and_then(|request| request.get("headers"))
            .and_then(Value::as_object)
            .unwrap();
        assert_eq!(headers.len(), 2);
        assert_eq!(
            headers.get(ANTHROPIC_BETA_HEADER).and_then(Value::as_str),
            Some(ANTHROPIC_BETA_VALUE)
        );
    }

    /// Un champ optionnel a `None` disparait du JSON, et l'en-tete du plugin
    /// apparait des que le catalogue est transforme.
    #[test]
    fn les_champs_optionnels_absents_van_rarement() {
        let mut draft = CatalogProviderStore::new();
        draft.push(fournisseur_anthropic("anthropic"));
        catalog_transform(&mut draft);

        let provider = &draft.get("anthropic").unwrap().provider;
        let json = serde_json::to_value(provider).unwrap();
        let objet = json.as_object().unwrap();

        assert!(!objet.contains_key("integrationID"));
        assert!(!objet.contains_key("disabled"));
        assert_eq!(objet.len(), 4);

        let api = objet.get("api").and_then(Value::as_object).unwrap();
        assert!(!api.contains_key("url"));
        assert!(!api.contains_key("settings"));
    }

    /// L'union `api` se relit sur son tag, dans les deux sens, et refuse tout
    /// autre tag. La variante `Native` exige `settings`, comme dans la source.
    #[test]
    fn l_api_se_deserialize_sur_son_type() {
        let aisdk: ProviderV2Info = serde_json::from_str(
            r#"{"id":"anthropic","name":"Anthropic","api":{"type":"aisdk","package":"@ai-sdk/anthropic"},"request":{"headers":{},"body":{}}}"#,
        )
        .unwrap();
        assert_eq!(aisdk.api.package(), Some("@ai-sdk/anthropic"));
        assert_eq!(aisdk.integration_id, None);

        let natif: ProviderV2Info = serde_json::from_str(
            r#"{"id":"local","name":"Local","api":{"type":"native","settings":{}},"request":{"headers":{},"body":{}}}"#,
        )
        .unwrap();
        assert_eq!(natif.api.package(), None);

        assert!(serde_json::from_str::<ProviderV2Info>(
            r#"{"id":"x","name":"X","api":{"type":"inconnu"},"request":{"headers":{},"body":{}}}"#
        )
        .is_err());
        assert!(serde_json::from_str::<ProviderV2Info>(
            r#"{"id":"x","name":"X","api":{"type":"native"},"request":{"headers":{},"body":{}}}"#
        )
        .is_err());
        assert!(serde_json::from_str::<ProviderV2Info>(
            r#"{"id":"x","name":"X","api":{"type":"aisdk","package":"p"},"request":{"body":{}}}"#
        )
        .is_err());
    }

    /// L'evenement du hook `aisdk` se serialise sous les noms `package`,
    /// `options` et `sdk`, et `sdk` disparait tant qu'il est absent.
    #[test]
    fn l_evenement_du_hook_aisdk_serialize_les_bons_noms() {
        let mut options = JsonMap::new();
        options.insert("apiKey".to_string(), Value::from("secret"));

        let evt = SdkHookEvent {
            r#package: ANTHROPIC_SDK_PACKAGE.to_string(),
            options,
            sdk: None,
        };
        let json = serde_json::to_value(&evt).unwrap();
        let objet = json.as_object().unwrap();

        assert_eq!(objet.len(), 2);
        assert!(objet.contains_key("package"));
        assert!(!objet.contains_key("r#package"));
        assert!(objet.contains_key("options"));
        assert!(!objet.contains_key("sdk"));

        let relu: SdkHookEvent =
            serde_json::from_value(serde_json::to_value(&evt).unwrap()).unwrap();
        assert_eq!(relu.r#package, ANTHROPIC_SDK_PACKAGE);
        assert_eq!(relu.sdk, None);
    }

    /// L'identifiant du plugin est le nom de fichier sans prefixe, et le
    /// paquet reconnu est exactement celui que la source compare.
    #[test]
    fn l_identifiant_et_le_paquet_sont_ceux_de_la_source() {
        assert_eq!(AnthropicPlugin::ID, "anthropic");
        assert_eq!(PLUGIN_ID, "anthropic");
        assert_eq!(AnthropicPlugin::PACKAGE, "@ai-sdk/anthropic");
        assert_eq!(API_TYPE_AISDK, "aisdk");
        assert_eq!(API_TYPE_NATIVE, "native");
        assert_eq!(
            ANTHROPIC_BETA_VALUE,
            "interleaved-thinking-2025-05-14,fine-grained-tool-streaming-2025-05-14"
        );
    }
}
