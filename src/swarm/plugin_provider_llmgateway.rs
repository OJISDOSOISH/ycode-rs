//! Portage Rust de `opencode/packages/core/src/plugin/provider/llmgateway.ts`.
//!
//! ## Ce que la source est vraiment
//!
//! Vingt-six lignes, un seul export, et aucun type declare. Le fichier
//! enregistre un plugin dont l'effet unique est de transformer le catalogue :
//!
//! ```ts
//! export const LLMGatewayPlugin = define({
//!   id: "llmgateway",
//!   effect: Effect.fn(function* (ctx) {
//!     const integrations = yield* Integration.Service
//!     yield* ctx.catalog.transform(
//!       Effect.fn(function* (evt) {
//!         for (const item of evt.provider.list()) {
//!           if (item.provider.disabled) continue
//!           if (item.provider.api.type !== "aisdk") continue
//!           if (item.provider.api.package !== "@ai-sdk/openai-compatible") continue
//!           if (item.provider.api.url !== "https://api.llmgateway.io/v1") continue
//!           if (!(yield* integrations.get(Integration.ID.make(item.provider.id)))) continue
//!           evt.provider.update(item.provider.id, (provider) => {
//!             provider.request.headers["HTTP-Referer"] = "https://opencode.ai/"
//!             provider.request.headers["X-Title"] = "opencode"
//!             provider.request.headers["X-Source"] = "opencode"
//!           })
//!         }
//!       }),
//!     )
//!   }),
//! })
//! ```
//!
//! LLM Gateway exige que chaque appel s'identifie. Le plugin ajoute donc trois
//! en-tetes de demande a tout fournisseur qui remplit cinq conditions : actif,
//! API de type `aisdk`, paquet npm `@ai-sdk/openai-compatible`, URL exactement
//! `https://api.llmgateway.io/v1`, et **integration enregistree** du meme nom.
//!
//! `define` (`plugin/internal.ts:59`) ne fait que retourner son argument.
//! `Effect.fn` et `yield*` se traduisent par des fonctions Rust pures, sans
//! `async` : la seule chose asynchrone ici est `integrations.get`, dont on ne
//! garde que le resultat booleen.
//!
//! ## Les types sont IMPORTES, pas redeclares
//!
//! La source ne declare aucun type : elle manipule `ProviderV2.MutableInfo`,
//! `ProviderV2.Api`, `ProviderV2.Request` et `Integration.ID`, qui vivent dans
//! `packages/schema/src/provider.ts` et `packages/schema/src/integration.ts`.
//! Aucun de ces deux fichiers n'est dans le lot.
//!
//! Les dix-huit autres plugins `provider/*.ts` du meme lot ont tous recopie
//! ces formes localement, ce qui est exactement le defaut que ce portage essaie
//! d'eliminer. Plutot que d'ajouter un dixieme exemplaire, ce fichier
//! **importe les types depuis un module qui les porte deja** :
//!
//! ```rust
//! use super::provider_zenmux::{Api, IdFournisseur, IdIntegration, Info};
//! ```
//!
//! `provider_zenmux.rs` expose `Api` (union taggee par `type`), `Info` (avec
//! `integrationID`), `Request` et `IdFournisseur`, tous copies du meme schema
//! `ProviderV2`. Les formes etant identiques, les reutiliser est correct, et
//! c'est une seule definition de `Info` que la suite du lot devra dedupliquer :
//! voir la section "A faire" plus bas.
//!
//! Ce qui reste declare ici est ce que **ce** plugin ajoute et qu'aucun voisin
//! ne porte : le registre d'integrations (le service `Integration.Service`),
//! les constantes de reconnaissance, et la transformation elle-meme.
//!
//! ## Le piege 1 : `if (item.provider.disabled)` est un test de VERACITE
//!
//! `disabled` est un `Schema.Boolean.pipe(optional)`, donc `boolean | undefined`
//! en TypeScript, `Option<bool>` en Rust. Le test de veracite donne :
//!
//! - `undefined` -> falsy -> le fournisseur **est** traite,
//! - `false` -> falsy -> le fournisseur **est** traite,
//! - `true` -> falsy... non : verite -> le fournisseur est ecarte.
//!
//! Autrement dit, seuls les fournisseurs explicitement desactives sont ecartes.
//! La traduction naive `if fournisseur.disabled.is_some() { continue }` est
//! **FAUSSE** : elle ecarterait aussi les fournisseurs `disabled: false`, que
//! le TypeScript traite. La forme correcte est
//! `matches!(fournisseur.disabled, Some(true))`, portee par [`est_desactive`].
//! Un test dedie verrouille les trois cas.
//!
//! ## Le piege 2 : `=` et non `??=` sur les en-tetes
//!
//! Le plugin voisin `provider_zenmux.rs` ecrit `??=` sur `HTTP-Referer` et
//! `X-Title`, donc n'ecrase rien. Ici les trois ecritures sont des
//! affectations **simples**, `=` :
//!
//! ```ts
//! provider.request.headers["HTTP-Referer"] = "https://opencode.ai/"
//! ```
//!
//! Donc une entete deja presente **est remplacee**, y compris quand sa valeur
//! est la chaine vide. C'est l'inverse exact du comportement de Zenmux, et
//! c'est le piege le plus facile a introduire ici : copier la version Zenmux
//! donnerait trois `or_insert` qui ne remplaceraient rien. La traduction Rust
//! est donc `BTreeMap::insert`, pas `entry().or_insert()`. Un test verrouille le
//! cas de la chaine vide ecrasee.
//!
//! ## Le piege 3 : la recherche d'integration ignore le champ `integrationID`
//!
//! ```ts
//! integrations.get(Integration.ID.make(item.provider.id))
//! ```
//!
//! L'identifiant recherche est celui du **fournisseur**, jamais la valeur du
//! champ `integrationID` du meme objet. `Integration.ID.make` est un simple
//! "brand" de compilation : a l'execution la chaine est inchangee, donc aucun
//! identifiant n'est valide ni refuse sur la forme. Un fournisseur dont
//! `integrationID` vaut autre chose est donc reconnu d'apres son propre `id`,
//! et un fournisseur dont `id` est enregistre mais dont `integrationID` ne l'est
//! pas reste traite. Un test verrouille les deux sens.
//!
//! `if (!(yield* integrations.get(...)))` est un test de veracite sur un
//! **objet** : un objet est toujours vrai, donc la seule valeur falsy possible
//! est `undefined`, c'est-a-dire une integration absente. D'ou le `Option` du
//! portage, et l'absence de toute autre condition.
//!
//! ## Le piege des noms de champs
//!
//! Le champ `integrationID` de `ProviderV2.Info` porte un **D majuscule**, la
//! forme que la relecture croisee a deja attrape deux fois ailleurs
//! (`sessionID`, `callID`). Il devient `integration_id` en Rust, avec le
//! `#[serde(rename = "integrationID")]` pose par le module dont le type est
//! importe. Ce fichier ne pose aucun renommage de son cote, et c'est
//! volontaire : redeclarer le struct pour y remettre le `rename` serait
//! exactement le doublon a eviter. Le test de serialisation verifie donc la
//! casse au travers du type partage.
//!
//! Les trois noms d'en-tetes sont eux aussi verifie un par un, car leur casse
//! fait partie du contrat avec le service distant : `HTTP-Referer` (avec le R
//! majuscule), `X-Title`, `X-Source`. `X-Source` n'existe que dans ce fichier,
//! aucun autre plugin du lot ne l'ecrit.
//!
//! ## Deux choses de `catalog.ts` laissees de cote, comme pour Zenmux
//!
//! 1. `evt.provider.list()` renvoie des `ProviderRecord`, c'est-a-dire des
//!    paires `{ provider, models }`. Le plugin ne lit que `record.provider` et
//!    n'ecrit jamais dans `models`. La signature `&mut [Info]` evite donc de
//!    porter le schema `ModelV2`, qui n'est pas mon fichier.
//!
//! 2. `evt.provider.update(id, fn)` (`catalog.ts:112`) fait deux choses de plus
//!    que d'appeler `fn` : il **cree** le fournisseur s'il est absent, puis
//!    appelle `normalizeApi`, qui recopie `request.body.baseURL` dans
//!    `api.url` puis supprime la cle `baseURL` du corps (`catalog.ts:99`).
//!    Comme le plugin n'appelle `update` que sur des ids qu'il vient de lire
//!    dans `list()`, la creation n'a jamais lieu. Mais `normalizeApi` **si**,
//!    et son effet est visible : apres le passage de ce plugin, un fournisseur
//!    qui portait `body.baseURL` se retrouve avec cette valeur recopiee dans
//!    `api.url` et le corps prive de cette cle. Ce comportement appartient a
//!    `catalog.ts`, pas a `llmgateway.ts` : il n'est pas imite ici. C'est le
//!    premier point a verifier si un test d'integration echoue sur l'URL d'un
//!    fournisseur.
//!
//! ## Ordre des conditions
//!
//! Les cinq `continue` sont evalues dans l'ordre de la source. Comme ils
//! ecarpent tous, l'ordre n'est pas observable sur le resultat ; il l'est en
//! revanche sur le **nombre d'appels** faits au registre d'integrations, qui
//! n'est consulte que par les fournisseurs deja passes au filtre d'API. Le
//! portage conserve cet ordre, donc cette propriete, meme si elle n'a aucune
//! importance tant que `get` reste une lecture pure.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::provider_zenmux::{Api, IdIntegration, Info};

/// Identifiant du plugin tel qu'il est enregistre par le moteur interne.
///
/// En TS : la propriete `id` de l'objet passe a `define`.
/// C'est la valeur sous laquelle `LLMGatewayPlugin` apparait dans la liste
/// `ProviderPlugins` de `plugin/provider.ts:55`.
pub const ID: &str = "llmgateway";

/// La seule valeur possible du champ `type` d'une API de type `aisdk`.
///
/// En TS : `Schema.Literal("aisdk")` dans `Provider.AISDK`.
pub const TYPE_AISDK: &str = "aisdk";

/// Le paquet npm que le plugin exige pour reconnaitre LLM Gateway.
///
/// En TS : la chaine comparee a `item.provider.api.package`.
/// Valeur identique a celle qu'attend Zenmux : c'est le meme SDK
/// openai-compatible. Seule la chaine est partagee, pas un type.
pub const PAQUET: &str = "@ai-sdk/openai-compatible";

/// L'URL exacte que le plugin exige pour reconnaitre LLM Gateway.
///
/// En TS : la chaine comparee a `item.provider.api.url`. La comparaison est
/// une egalite stricte, donc un champ `url` absent ne correspond pas, et une
/// URL vide non plus.
pub const URL_LLMGATEWAY: &str = "https://api.llmgateway.io/v1";

/// Nom de la premiere entete imposee aux fournisseurs LLM Gateway.
pub const ENTETE_REFERER: &str = "HTTP-Referer";

/// Valeur imposee a [`ENTETE_REFERER`].
///
/// Ce n'est pas une URL d'API : c'est l'adresse du site OpenCode, que le
/// service attend comme identite de l'appelant.
pub const VALEUR_REFERER: &str = "https://opencode.ai/";

/// Nom de la deuxieme entete imposee aux fournisseurs LLM Gateway.
pub const ENTETE_TITRE: &str = "X-Title";

/// Valeur imposee a [`ENTETE_TITRE`].
pub const VALEUR_TITRE: &str = "opencode";

/// Nom de la troisieme entete imposee aux fournisseurs LLM Gateway.
///
/// Cette entete n'existe que dans ce fichier : aucun autre plugin du lot
/// n'ecrit `X-Source`.
pub const ENTETE_SOURCE: &str = "X-Source";

/// Valeur imposee a [`ENTETE_SOURCE`].
pub const VALEUR_SOURCE: &str = "opencode";

/// Reference a une integration enregistree.
///
/// En TS : `Integration.Ref`, le couple `{ id, name }` que renvoie
/// `Integration.Draft.get` (`integration.ts:130`). `Interface.get`
/// (`integration.ts:143`) renvoie un `Info` plus riche, dont `id` et `name`
/// sont les deux premiers champs ; le reste (`methods`, `connections`) n'est
/// jamais lu par ce plugin, donc n'est pas porte ici.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InfoIntegration {
    /// Identifiant de l'integration, chaine brandee cote TypeScript.
    pub id: IdIntegration,

    /// Nom affiche de l'integration.
    pub name: String,
}

impl InfoIntegration {
    /// Construit une reference a partir de son identifiant ; le nom vaut
    /// l'identifiant, comme dans `integration.ts:236`.
    pub fn nouveau(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            id: IdIntegration(id.clone()),
            name: id,
        }
    }
}

/// Le registre d'integrations, seul service dont ce plugin a besoin.
///
/// En TS : `Integration.Service`, dont la seule utilisation ici est
/// `integrations.get(id)`. Le contrat est volontairement minimal : une
/// recherche par identifiant, qui renvoie l'integration si elle est
/// enregistree et `None` sinon, exactement comme le `undefined` du TypeScript.
pub trait RegistreIntegrations {
    /// Renvoie l'integration enregistree sous `id`, ou `None`.
    fn get(&self, id: &IdIntegration) -> Option<InfoIntegration>;
}

/// Une implementation concrete du registre, indexee par identifiant.
///
/// En TS : le `Map<ID, Entry>` de `integration.ts:125`. Un `BTreeMap` est
/// choisi conformement au tableau de conversion, donc l'iteration est
/// deterministe ; le plugin n'itere d'ailleurs jamais ce registre.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registre {
    entrees: BTreeMap<IdIntegration, InfoIntegration>,
}

impl Registre {
    /// Registre vide : aucune integration enregistree, donc plus aucune
    /// condition ne peut etre satisfaite.
    pub fn nouveau() -> Self {
        Self {
            entrees: BTreeMap::new(),
        }
    }

    /// Enregistre une integration, en remplacant une entree de meme
    /// identifiant. Renvoie l'entree remplacee, s'il y en avait une.
    pub fn inserer(&mut self, id: impl Into<String>, name: impl Into<String>) -> Option<InfoIntegration> {
        let info = InfoIntegration {
            id: IdIntegration(id.into()),
            name: name.into(),
        };
        self.entrees.insert(info.id.clone(), info)
    }

    /// Nombre d'integrations enregistrees.
    pub fn len(&self) -> usize {
        self.entrees.len()
    }

    /// Vrai si le registre ne contient aucune integration.
    pub fn est_vide(&self) -> bool {
        self.entrees.is_empty()
    }
}

impl RegistreIntegrations for Registre {
    fn get(&self, id: &IdIntegration) -> Option<InfoIntegration> {
        self.entrees.get(id).cloned()
    }
}

/// Le plugin LLM Gateway.
///
/// En TS : `export const LLMGatewayPlugin = define({ id: "llmgateway", effect })`.
///
/// `define` se contente de retourner son argument, donc le type `Plugin` de
/// `plugin/internal.ts:54` se resume ici a un identifiant et a une fonction.
/// La fonction est portee ci-dessous sous le nom [`appliquer`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LlmGatewayPlugin;

impl LlmGatewayPlugin {
    /// L'identifiant du plugin, tel qu'il apparait dans la liste
    /// `ProviderPlugins`. C'est la meme valeur que le [`ID`] du module.
    pub const ID: &'static str = "llmgateway";

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
    /// fournisseurs a transformer et le registre des integrations.
    pub fn transformer<R: RegistreIntegrations + ?Sized>(
        self,
        fournisseurs: &mut [Info],
        integrations: &R,
    ) {
        appliquer(fournisseurs, integrations);
    }
}

impl Default for LlmGatewayPlugin {
    fn default() -> Self {
        Self
    }
}

/// Indique si un fournisseur est explicitement desactive.
///
/// Equivalent de `if (item.provider.disabled) continue`, qui est un test de
/// **veracite** et non de nullite. Le champ est `Option<bool>` :
///
/// - `None` vaut `undefined`, qui est falsy en JavaScript, donc le
///   fournisseur n'est pas ecarte ;
/// - `Some(false)` est falsy, donc le fournisseur n'est pas ecarte ;
/// - `Some(true)` est verite, donc le fournisseur est ecarte.
///
/// Un `is_some()` ici serait faux : il ecarterait les fournisseurs
/// `disabled: false`, que le TypeScript traite normalement.
pub fn est_desactive(fournisseur: &Info) -> bool {
    matches!(fournisseur.disabled, Some(true))
}

/// Indique si l'API d'un fournisseur est celle de LLM Gateway.
///
/// Les trois conditions de la source, dans le meme ordre et avec la meme
/// portee :
///
/// ```ts
/// if (item.provider.api.type !== "aisdk") continue
/// if (item.provider.api.package !== "@ai-sdk/openai-compatible") continue
/// if (item.provider.api.url !== "https://api.llmgateway.io/v1") continue
/// ```
///
/// La premiere condition ecarte d'office la variante `native`, qui n'a pas de
/// champ `package`. Les deux autres sont des egalites strictes : une URL
/// absente ne correspond pas, et une URL vide non plus.
pub fn correspond(api: &Api) -> bool {
    match api {
        Api::Aisdk(api) => api.package == PAQUET && api.url.as_deref() == Some(URL_LLMGATEWAY),
        Api::Native(_) => false,
    }
}

/// Impose les trois en-tetes d'identification de LLM Gateway.
///
/// Les affectations de la source sont des `=` simples, pas des `??=` : une
/// entete deja presente est donc **remplacee**, y compris quand elle vaut la
/// chaine vide. C'est l'inverse du comportement du plugin Zenmux voisin, et
/// l'inverse de ce que donnerait un `entry().or_insert()`.
pub fn ajouter_entetes(fournisseur: &mut Info) {
    fournisseur
        .request
        .headers
        .insert(ENTETE_REFERER.to_string(), VALEUR_REFERER.to_string());
    fournisseur
        .request
        .headers
        .insert(ENTETE_TITRE.to_string(), VALEUR_TITRE.to_string());
    fournisseur
        .request
        .headers
        .insert(ENTETE_SOURCE.to_string(), VALEUR_SOURCE.to_string());
}

/// Applique la transformation LLM Gateway a un catalogue.
///
/// Equivalent du corps de la fonction passee a `ctx.catalog.transform`. Les
/// fournisseurs sont parcourus dans l'ordre de la liste, et chacun est modifie
/// sur place s'il remplit les cinq conditions. La fonction ne renvoie rien,
/// comme l'original.
///
/// La liste venue de `evt.provider.list()` est un instantane ; la muter en
/// place pendant le parcours revient exactement a appeler `update` sur chaque
/// id lu, puisque `update` retrouve le meme enregistrement.
pub fn appliquer<R: RegistreIntegrations + ?Sized>(fournisseurs: &mut [Info], integrations: &R) {
    for fournisseur in fournisseurs.iter_mut() {
        if est_desactive(fournisseur) {
            continue;
        }
        if !correspond(&fournisseur.api) {
            continue;
        }
        // `Integration.ID.make(item.provider.id)` : c'est l'identifiant du
        // FOURNISSEUR qui est recherche, jamais la valeur du champ
        // `integrationID` du meme objet. Le brand est purement statique, donc
        // la chaine reste inchangee.
        let identifiant = IdIntegration(fournisseur.id.0.clone());
        if integrations.get(&identifiant).is_none() {
            continue;
        }
        ajouter_entetes(fournisseur);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::provider_zenmux::{ApiAisdk, IdFournisseur};

    /// Construit un fournisseur dont l'API est celle de LLM Gateway, avec des
    /// en-tetes deja presents pour pouvoir tester l'ecrasement.
    fn llmgateway(entetes: &[(&str, &str)]) -> Info {
        let mut fournisseur = Info::nouveau("llmgateway", "LLM Gateway", Api::aisdk(PAQUET));
        if let Api::Aisdk(api) = &mut fournisseur.api {
            api.url = Some(URL_LLMGATEWAY.to_string());
        }
        for (cle, valeur) in entetes {
            fournisseur
                .request
                .headers
                .insert(cle.to_string(), valeur.to_string());
        }
        fournisseur
    }

    /// Registre contenant exactement les identifiants donnes.
    fn registre_avec(identifiants: &[&str]) -> Registre {
        let mut registre = Registre::nouveau();
        for identifiant in identifiants {
            registre.inserer(*identifiant, *identifiant);
        }
        registre
    }

    /// Un fournisseur LLM Gateway dont l'integration est enregistree recoit
    /// les trois en-tetes, et le reste de sa forme ne bouge pas.
    #[test]
    fn un_fournisseur_llmgateway_avec_son_integration_recoit_les_trois_entetes() {
        let integrations = registre_avec(&["llmgateway"]);
        let mut catalogue = vec![llmgateway(&[])];

        appliquer(&mut catalogue, &integrations);

        let entetes = &catalogue[0].request.headers;
        assert_eq!(entetes.len(), 3);
        assert_eq!(entetes.get(ENTETE_REFERER).map(String::as_str), Some(VALEUR_REFERER));
        assert_eq!(entetes.get(ENTETE_TITRE).map(String::as_str), Some(VALEUR_TITRE));
        assert_eq!(entetes.get(ENTETE_SOURCE).map(String::as_str), Some(VALEUR_SOURCE));
        assert_eq!(catalogue[0].id.0, "llmgateway");
        assert_eq!(catalogue[0].name, "LLM Gateway");
    }

    /// Un fournisseur dont l'integration n'est pas enregistree n'est pas
    /// touche, meme si tout le reste correspond. C'est la cinquieme condition.
    #[test]
    fn un_fournisseur_sans_integration_enregistree_n_est_pas_touche() {
        let integrations = registre_avec(&[]);
        assert!(integrations.est_vide());

        let mut catalogue = vec![llmgateway(&[])];
        appliquer(&mut catalogue, &integrations);
        assert!(catalogue[0].request.headers.is_empty());

        // Une integration portant un autre nom ne sauve pas le fournisseur.
        let integrations = registre_avec(&["autre-fournisseur"]);
        let mut catalogue = vec![llmgateway(&[])];
        appliquer(&mut catalogue, &integrations);
        assert!(catalogue[0].request.headers.is_empty());

        // Un registre enregistre bien la bonne cle, et le fournisseur est
        // alors traite : le test n'est pas trivial.
        let integrations = registre_avec(&["llmgateway"]);
        let mut catalogue = vec![llmgateway(&[])];
        appliquer(&mut catalogue, &integrations);
        assert_eq!(catalogue[0].request.headers.len(), 3);
    }

    /// La recherche d'integration se fait sur l'identifiant du fournisseur,
    /// jamais sur le champ `integrationID` du meme objet. C'est le troisieme
    /// piege : le code fuente est `integrations.get(Integration.ID.make(
    /// item.provider.id))`, pas `provider.integrationID`.
    #[test]
    fn la_recherche_d_integration_utilise_l_identifiant_du_fournisseur() {
        // Champ `integrationID` qui ne correspond a rien, identifiant du
        // fournisseur enregistre : le fournisseur est traite.
        let mut fournisseur = llmgateway(&[]);
        fournisseur.integration_id = Some(IdIntegration("inexistant".to_string()));
        let integrations = registre_avec(&["llmgateway"]);
        let mut catalogue = vec![fournisseur];

        appliquer(&mut catalogue, &integrations);
        assert_eq!(catalogue[0].request.headers.len(), 3);
        assert_eq!(
            catalogue[0].integration_id.as_ref().map(|id| id.0.as_str()),
            Some("inexistant"),
            "le champ integrationID ne doit pas etre modifie"
        );

        // Cas inverse : c'est `integrationID` qui est enregistre, pas l'id du
        // fournisseur. Le fournisseur n'est alors pas traite.
        let mut autre = llmgateway(&[]);
        autre.id = IdFournisseur::nouveau("autre-fournisseur");
        autre.integration_id = Some(IdIntegration("mon-fournisseur".to_string()));
        let integrations = registre_avec(&["mon-fournisseur"]);
        let mut catalogue = vec![autre];

        appliquer(&mut catalogue, &integrations);
        assert!(catalogue[0].request.headers.is_empty());
    }

    /// Seul `disabled: true` ecarte un fournisseur. `disabled: false` et
    /// l'absence de `disabled` le laissent passer, parce que le test
    /// TypeScript est un test de veracite, pas de nullite.
    #[test]
    fn seul_un_fournisseur_explicitement_desactive_est_ecarte() {
        let integrations = registre_avec(&["llmgateway"]);

        let mut desactive = llmgateway(&[]);
        desactive.disabled = Some(true);
        let mut explicitement_actif = llmgateway(&[]);
        explicitement_actif.disabled = Some(false);
        let mut sans_mention = llmgateway(&[]);
        sans_mention.disabled = None;

        let mut catalogue = vec![desactive, explicitement_actif, sans_mention];
        appliquer(&mut catalogue, &integrations);

        assert!(
            catalogue[0].request.headers.is_empty(),
            "disabled: true doit ecarter le fournisseur"
        );
        assert_eq!(
            catalogue[1].request.headers.len(),
            3,
            "disabled: false ne doit pas ecarter le fournisseur"
        );
        assert_eq!(
            catalogue[2].request.headers.len(),
            3,
            "disabled absent ne doit pas ecarter le fournisseur"
        );
    }

    /// Les en-tetes deja presentes sont remplacees, y compris quand leur
    /// valeur est la chaine vide. C'est la difference avec le plugin Zenmux
    /// voisin, qui utilise `??=` et ne remplace donc rien.
    #[test]
    fn les_entetes_deja_presentes_sont_ecrasees_y_compris_les_vides() {
        let integrations = registre_avec(&["llmgateway"]);
        let mut catalogue = vec![llmgateway(&[
            (ENTETE_REFERER, ""),
            (ENTETE_TITRE, "autre chose"),
            (ENTETE_SOURCE, ""),
        ])];

        appliquer(&mut catalogue, &integrations);

        let entetes = &catalogue[0].request.headers;
        assert_eq!(entetes.len(), 3);
        assert_eq!(entetes.get(ENTETE_REFERER).map(String::as_str), Some(VALEUR_REFERER));
        assert_eq!(entetes.get(ENTETE_TITRE).map(String::as_str), Some(VALEUR_TITRE));
        assert_eq!(entetes.get(ENTETE_SOURCE).map(String::as_str), Some(VALEUR_SOURCE));
    }

    /// Aucun des trois signes de l'API n'est suffisant a lui seul : un autre
    /// paquet, une autre URL, une URL absente, une URL vide ou une API
    /// `native` ecarte tous le fournisseur, integration comprise.
    #[test]
    fn un_fournisseur_qui_n_est_pas_llmgateway_n_est_pas_touche() {
        let integrations = registre_avec(&["llmgateway", "autre", "natif", "url_vide", "sans_url"]);

        let mut autre_paquet = llmgateway(&[]);
        if let Api::Aisdk(api) = &mut autre_paquet.api {
            api.package = "@ai-sdk/anthropic".to_string();
        }

        let mut autre_url = llmgateway(&[]);
        if let Api::Aisdk(api) = &mut autre_url.api {
            api.url = Some("https://api.llmgateway.io/v2".to_string());
        }

        let mut url_vide = llmgateway(&[]);
        if let Api::Aisdk(api) = &mut url_vide.api {
            api.url = Some(String::new());
        }

        let mut sans_url = llmgateway(&[]);
        if let Api::Aisdk(api) = &mut sans_url.api {
            api.url = None;
        }

        let natif = Info::nouveau("natif", "Natif", Api::native());

        let mut catalogue = vec![autre_paquet, autre_url, url_vide, sans_url, natif];
        appliquer(&mut catalogue, &integrations);

        for (index, fournisseur) in catalogue.iter().enumerate() {
            assert!(
                fournisseur.request.headers.is_empty(),
                "le fournisseur {index} n aurait pas du etre touche"
            );
        }
    }

    /// Une liste vide ne produit rien, une liste a une seule entree se comporte
    /// comme l'entree isolee, et l'ordre inverse donne le meme resultat.
    #[test]
    fn une_liste_vide_ne_produit_rien_et_l_inverse_est_equivalent() {
        let integrations = registre_avec(&["llmgateway"]);

        let mut vide: Vec<Info> = Vec::new();
        appliquer(&mut vide, &integrations);
        assert!(vide.is_empty());

        let mut unique = vec![llmgateway(&[])];
        appliquer(&mut unique, &integrations);
        assert_eq!(unique[0].request.headers.len(), 3);

        // Deux listes de meme contenu mais d'ordre oppose : le fournisseur
        // concerne est modifie dans les deux cas, l'autre ne l'est dans
        // aucun des deux, et le contenu modifie est identique.
        let mut direct = vec![
            Info::nouveau("avant", "Avant", Api::native()),
            llmgateway(&[]),
            Info::nouveau("apres", "Apres", Api::aisdk("@ai-sdk/anthropic")),
        ];
        appliquer(&mut direct, &integrations);

        let mut inverse = vec![
            Info::nouveau("apres", "Apres", Api::aisdk("@ai-sdk/anthropic")),
            llmgateway(&[]),
            Info::nouveau("avant", "Avant", Api::native()),
        ];
        appliquer(&mut inverse, &integrations);

        assert!(direct[0].request.headers.is_empty());
        assert!(direct[2].request.headers.is_empty());
        assert!(inverse[0].request.headers.is_empty());
        assert!(inverse[2].request.headers.is_empty());

        assert_eq!(direct[1].request.headers.len(), 3);
        assert_eq!(inverse[1].request.headers.len(), 3);
        assert_eq!(direct[1].request.headers, inverse[1].request.headers);
        assert_eq!(direct[1].request.headers, unique[0].request.headers);
    }

    /// Une transformation appliquee deux fois donne exactement le meme
    /// resultat : les affectations sont des `=` simples, donc idempotentes.
    #[test]
    fn deux_pass_successives_donnent_le_meme_resultat() {
        let integrations = registre_avec(&["llmgateway"]);
        let mut catalogue = vec![llmgateway(&[])];

        appliquer(&mut catalogue, &integrations);
        let premier = catalogue[0].request.headers.clone();
        appliquer(&mut catalogue, &integrations);

        assert_eq!(catalogue[0].request.headers, premier);
        assert_eq!(catalogue[0].request.headers.len(), 3);
    }

    /// Les noms de champs serialises sont exactement ceux du TypeScript : le
    /// `integrationID` au **D** majuscule, `disabled` absent quand il est
    /// `undefined`, et les trois noms d'en-tetes avec leur casse exacte. Ce
    /// test verrouille l'echange avec le TypeScript. Il porte sur le type
    /// partage importe de `provider_zenmux`, ce qui est voulu : une seule
    /// definition du renommage, verifiee une seule fois.
    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        let integrations = registre_avec(&["llmgateway"]);

        let mut avec = llmgateway(&[]);
        avec.integration_id = Some(IdIntegration("llmgateway".to_string()));
        avec.disabled = Some(false);

        let mut catalogue = vec![avec];
        appliquer(&mut catalogue, &integrations);

        let objet = serde_json::to_value(&catalogue[0]).unwrap();
        let objet = objet.as_object().unwrap();
        for nom in ["id", "integrationID", "name", "disabled", "api", "request"] {
            assert!(objet.contains_key(nom), "champ absent du JSON : {nom}");
        }
        assert_eq!(objet.len(), 6, "aucun champ en trop : {objet:?}");
        assert_eq!(objet.get("id").and_then(|v| v.as_str()), Some("llmgateway"));
        assert_eq!(
            objet.get("integrationID").and_then(|v| v.as_str()),
            Some("llmgateway")
        );
        assert_eq!(objet.get("disabled").and_then(|v| v.as_bool()), Some(false));

        // Les variantes Rust des deux noms piegeux ne doivent jamais sortir.
        assert!(!objet.contains_key("integration_id"));
        assert!(!objet.contains_key("integrationId"));

        // `request` porte exactement `headers` et `body`.
        let request = objet.get("request").and_then(|v| v.as_object()).unwrap();
        assert_eq!(request.len(), 2);
        assert!(request.contains_key("headers"));
        assert!(request.contains_key("body"));

        // Les en-tetes portent exactement les trois noms, avec leur casse.
        let headers = request.get("headers").and_then(|v| v.as_object()).unwrap();
        assert_eq!(headers.len(), 3);
        for (nom, valeur) in [
            (ENTETE_REFERER, VALEUR_REFERER),
            (ENTETE_TITRE, VALEUR_TITRE),
            (ENTETE_SOURCE, VALEUR_SOURCE),
        ] {
            assert_eq!(headers.get(nom).and_then(|v| v.as_str()), Some(valeur));
        }
        // Une casse erronee ne serait pas acceptee par le service distant.
        assert!(!headers.contains_key("HTTP-referer"));
        assert!(!headers.contains_key("X-Title-Source"));

        // Un fournisseur sans integration ni mention de desactivation
        // n'emporte pas les cles optionnelles.
        let mut sans = llmgateway(&[]);
        sans.integration_id = None;
        sans.disabled = None;
        let objet = serde_json::to_value(&sans).unwrap();
        let objet = objet.as_object().unwrap();
        assert_eq!(objet.len(), 4, "aucun champ en trop : {objet:?}");
        assert!(!objet.contains_key("integrationID"));
        assert!(!objet.contains_key("disabled"));
    }

    /// Un fournisseur venue du TypeScript se relit, la transformation s'y
    /// applique, et un aller-retour ne change rien.
    #[test]
    fn un_fournisseur_lu_depuis_le_json_du_typescript_est_reconnu() {
        let integrations = registre_avec(&["llmgateway"]);
        let brut = r#"{
            "id": "llmgateway",
            "integrationID": "llmgateway",
            "name": "LLM Gateway",
            "api": {
                "type": "aisdk",
                "package": "@ai-sdk/openai-compatible",
                "url": "https://api.llmgateway.io/v1"
            },
            "request": { "headers": {}, "body": {} }
        }"#;

        let relu: Info = serde_json::from_str(brut).unwrap();
        assert!(correspond(&relu.api));
        assert!(!est_desactive(&relu));

        let mut catalogue = vec![relu];
        appliquer(&mut catalogue, &integrations);

        let objet = serde_json::to_value(&catalogue[0]).unwrap();
        assert_eq!(objet.get("request").and_then(|v| v.get("headers")).and_then(|v| v.as_object()).unwrap().len(), 3);
        assert_eq!(catalogue[0].api.tag(), TYPE_AISDK);
        assert_eq!(catalogue[0].id.0, "llmgateway");
    }

    /// Une entree invalide est refusee : champ obligatoire manquant, type
    /// faux, URL de type faux, `disabled` de type faux.
    #[test]
    fn une_entree_invalide_est_refusee() {
        assert!(serde_json::from_str::<Info>(r#"{"id":"x","name":"y","request":{"headers":{}}}"#).is_err());
        assert!(serde_json::from_str::<Info>(
            r#"{"id":"x","name":"y","disabled":"oui","api":{"type":"aisdk","package":"p"},"request":{"headers":{},"body":{}}}"#
        )
        .is_err());
        assert!(serde_json::from_str::<Info>(
            r#"{"id":"x","name":"y","api":{"type":"inconnu","package":"p"},"request":{"headers":{},"body":{}}}"#
        )
        .is_err());
        assert!(serde_json::from_str::<Info>(
            r#"{"id":"x","name":"y","api":{"type":"aisdk","package":"p","url":7},"request":{"headers":{},"body":{}}}"#
        )
        .is_err());

        // La forme `native` est valide mais refusee par la reconnaissance.
        let natif: Info = serde_json::from_str(
            r#"{"id":"x","name":"y","api":{"type":"native","settings":{}},"request":{"headers":{},"body":{}}}"#,
        )
        .unwrap();
        assert!(!correspond(&natif.api));

        // Une entete de type faux est refusee, et `package` est obligatoire.
        assert!(serde_json::from_str::<ApiAisdk>(r#"{"package":42}"#).is_err());
        assert!(serde_json::from_str::<ApiAisdk>(r#"{"url":"https://api.llmgateway.io/v1"}"#).is_err());
        assert!(serde_json::from_str::<ApiAisdk>(r#"{"package":"p","url":"u"}"#).is_ok());
    }

    /// Le plugin s'annonce sous le bon identifiant, et le construire ne
    /// modifie rien tant qu'on ne l'appelle pas.
    #[test]
    fn le_plugin_s_annonce_sous_le_bon_identifiant() {
        assert_eq!(LlmGatewayPlugin::ID, "llmgateway");
        assert_eq!(ID, "llmgateway");
        assert_eq!(LlmGatewayPlugin::nouveau(), LlmGatewayPlugin::default());

        let integrations = registre_avec(&["llmgateway"]);
        let mut catalogue = vec![llmgateway(&[])];
        assert!(
            catalogue[0].request.headers.is_empty(),
            "construire le plugin ne doit rien modifier"
        );

        LlmGatewayPlugin::nouveau().transformer(&mut catalogue, &integrations);
        assert_eq!(catalogue[0].request.headers.len(), 3);

        // Le registre se comporte comme la Map du TypeScript : inserer deux
        // fois le meme identifiant remplace l'entree et renvoie l'ancienne.
        let mut registre = Registre::nouveau();
        assert!(registre.est_vide());
        assert_eq!(registre.len(), 0);
        assert!(registre.inserer("llmgateway", "premier").is_none());
        assert_eq!(registre.len(), 1);
        let remplace = registre.inserer("llmgateway", "second");
        assert_eq!(remplace.map(|info| info.name), Some("premier".to_string()));
        assert_eq!(registre.len(), 1);
        assert_eq!(
            registre.get(&IdIntegration("llmgateway".to_string())).map(|info| info.name),
            Some("second".to_string())
        );
        assert!(registre.get(&IdIntegration("autre".to_string())).is_none());
    }
}
