# swarm-oc2-websearch

source : opencode/packages/core/src/tool/websearch.ts
cible : src/swarm/oc2_websearch.rs (a creer par l agent principal, non cree ici)
taille : 21434 octets
tests : 7

===DEBUT===
```rust
//! Portage Rust de `opencode/packages/core/src/tool/websearch.ts`.
//!
//! Logique metier pure seulement. Tout ce qui touche au reseau ou aux
//! effets n est pas porte et est signale comme saute dans le rapport :
//! client HTTP, timeout de 25 secondes, corps borne a 256 Kio, permission,
//! enregistrement de l outil, couches Effect, service de config lu depuis
//! l environnement, et noeuds de localisation.
//!
//! Ce qui est porte :
//! - constantes et texte descriptif avec annee en parametre
//! - choix du fournisseur avec priorites et parite du checksum
//! - construction de l URL Exa avec cle optionnelle
//! - analyse d une reponse directe ou SSE avec prefixe "data: "
//! - defauts et validateurs pour les champs d entree
//! - construction des arguments Exa et Parallel et de la sortie finale
//!
//! Pieges repris de la source :
//! - `if (!cle)` est un test de veracite : une chaine vide donne l URL de
//!   base, comme une cle absente.
//! - `.find((item) => item.text)` est un test de veracite : un texte vide
//!   est ignore au profit du suivant.
//! - `input.numResults || 8` donne 8 pour 0 ou None.
//! - `text ?? NO_RESULTS` ne remplace que None : une chaine vide survit.

use serde::{Deserialize, Serialize};

/// Nom de l outil, miroir de `name` en TS.
pub const NAME: &str = "websearch";
/// Texte rendu quand aucun resultat n est disponible.
pub const NO_RESULTS: &str = "No search results found. Please try a different query.";
/// URL du backend Exa.
pub const EXA_URL: &str = "https://mcp.exa.ai/mcp";
/// URL du backend Parallel.
pub const PARALLEL_URL: &str = "https://search.parallel.ai/mcp";
/// Nombre maximal de resultats, miroir de `MAX_NUM_RESULTS`.
pub const MAX_NUM_RESULTS: u64 = 20;
/// Nombre maximal de caracteres de contexte, miroir de `MAX_CONTEXT_CHARACTERS`.
pub const MAX_CONTEXT_CHARACTERS: u64 = 50_000;
/// Taille maximale de reponse acceptee en octets, miroir de `MAX_RESPONSE_BYTES`.
pub const MAX_RESPONSE_BYTES: usize = 256 * 1024;
/// Defaut applique par `input.numResults || 8` en TS.
pub const DEFAULT_NUM_RESULTS: u64 = 8;

/// Texte descriptif de l outil avec annee en parametre.
///
/// La source interpole `new Date().getFullYear()` dans le gabarit. Ici
/// l annee est un parametre pour garder une fonction pure.
pub fn description(annee: u32) -> String {
    format!(
        "Search the web using the session's local web search provider. Use this for current information beyond knowledge cutoff.\n\nThis is a provider-independent local tool backed by Exa or Parallel. Provider-hosted web search tools are separate and execute at the model provider.\n\nOptional controls support result count, live crawling ('fallback' or 'preferred'), search type ('auto', 'fast', or 'deep'), and maximum context characters.\n\nThe current year is {}. Use this year when searching for recent information or current events.",
        annee
    )
}

/// Fournisseur de recherche, miroir de `Provider` en TS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Provider {
    #[serde(rename = "exa")]
    Exa,
    #[serde(rename = "parallel")]
    Parallel,
}

/// Mode de live crawl, miroir du literal `["fallback", "preferred"]` en TS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LivecrawlMode {
    #[serde(rename = "fallback")]
    Fallback,
    #[serde(rename = "preferred")]
    Preferred,
}

/// Type de recherche, miroir du literal `["auto", "fast", "deep"]` en TS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchType {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "fast")]
    Fast,
    #[serde(rename = "deep")]
    Deep,
}

/// Entree de l outil, miroir de `Input` en TS.
///
/// `type` est un mot cle en Rust et devient `search_type` avec un `rename`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSearchInput {
    pub query: String,
    #[serde(rename = "numResults", skip_serializing_if = "Option::is_none")]
    pub num_results: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub livecrawl: Option<LivecrawlMode>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub search_type: Option<SearchType>,
    #[serde(rename = "contextMaxCharacters", skip_serializing_if = "Option::is_none")]
    pub context_max_characters: Option<u64>,
}

/// Config de l outil, miroir de `Config` en TS.
///
/// Seule la donnee est portee. La couche qui la lit depuis l environnement
/// (`OPENCODE_WEBSEARCH_PROVIDER`, `OPENCODE_ENABLE_EXA`, etc.) n est pas
/// portee car c est un effet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSearchConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<Provider>,
    #[serde(rename = "enableExa")]
    pub enable_exa: bool,
    #[serde(rename = "enableParallel")]
    pub enable_parallel: bool,
    #[serde(rename = "exaApiKey", skip_serializing_if = "Option::is_none")]
    pub exa_api_key: Option<String>,
    #[serde(rename = "parallelApiKey", skip_serializing_if = "Option::is_none")]
    pub parallel_api_key: Option<String>,
}

/// Arguments envoyes au backend Exa, miroir de `ExaArgs` en TS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExaArgs {
    pub query: String,
    #[serde(rename = "type")]
    pub search_type: String,
    #[serde(rename = "numResults")]
    pub num_results: u64,
    pub livecrawl: String,
    #[serde(rename = "contextMaxCharacters", skip_serializing_if = "Option::is_none")]
    pub context_max_characters: Option<u64>,
}

/// Arguments envoyes au backend Parallel, miroir de `ParallelArgs` en TS.
///
/// Les champs sont deja en snake_case dans la source donc sans `rename`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParallelArgs {
    pub objective: String,
    pub search_queries: Vec<String>,
    pub session_id: String,
}

/// Un element de contenu MCP, miroir du struct inline de `McpResult` en TS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpContentItem {
    #[serde(rename = "type")]
    pub item_type: String,
    pub text: String,
}

/// Enveloppe `result` du resultat MCP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpResultInner {
    pub content: Vec<McpContentItem>,
}

/// Resultat MCP decode depuis le JSON, miroir de `McpResult` en TS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpResult {
    pub result: McpResultInner,
}

/// Parametres d une requete JSON-RPC, miroir de `params` en TS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpParams<T> {
    pub name: String,
    pub arguments: T,
}

/// Requete JSON-RPC `tools/call`, miroir de la fabrique `McpRequest` en TS.
///
/// Les champs `jsonrpc`, `id` et `method` sont des litteraux en TS
/// ("2.0", 1, "tools/call"). Ici ce sont des donnees avec constructeur
/// qui impose les constantes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpRequest<T> {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: McpParams<T>,
}

/// Sortie de l outil, miroir de `Output` en TS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebSearchOutput {
    pub provider: Provider,
    pub text: String,
}

/// Construit une requete JSON-RPC avec les constantes imposees.
pub fn requete_mcp<T>(nom_outil: &str, arguments: T) -> McpRequest<T> {
    McpRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "tools/call".to_string(),
        params: McpParams {
            name: nom_outil.to_string(),
            arguments,
        },
    }
}

/// Vrai si le nombre de resultats respecte `PositiveInt` et la borne.
pub fn est_num_results_valide(n: u64) -> bool {
    n >= 1 && n <= MAX_NUM_RESULTS
}

/// Vrai si le maximum de caracteres respecte `PositiveInt` et la borne.
pub fn est_context_max_valide(n: u64) -> bool {
    n >= 1 && n <= MAX_CONTEXT_CHARACTERS
}

/// Applique `input.numResults || 8` : None ou 0 donne 8.
pub fn resolve_num_results(n: Option<u64>) -> u64 {
    match n {
        Some(v) if v != 0 => v,
        _ => DEFAULT_NUM_RESULTS,
    }
}

/// Applique `input.livecrawl || "fallback"`.
pub fn resolve_livecrawl(m: Option<LivecrawlMode>) -> LivecrawlMode {
    m.unwrap_or(LivecrawlMode::Fallback)
}

/// Applique `input.type || "auto"`.
pub fn resolve_search_type(t: Option<SearchType>) -> SearchType {
    t.unwrap_or(SearchType::Auto)
}

/// Rend la forme texte attendue par Exa pour un mode de crawl.
pub fn livecrawl_as_str(m: LivecrawlMode) -> &'static str {
    match m {
        LivecrawlMode::Fallback => "fallback",
        LivecrawlMode::Preferred => "preferred",
    }
}

/// Rend la forme texte attendue par Exa pour un type de recherche.
pub fn search_type_as_str(t: SearchType) -> &'static str {
    match t {
        SearchType::Auto => "auto",
        SearchType::Fast => "fast",
        SearchType::Deep => "deep",
    }
}

/// Construit l URL Exa, miroir de `exaUrl` en TS.
///
/// `if (!apiKey)` en TS est un test de veracite : None ou chaine vide
/// donne l URL de base. Seule difference assumee : la cle n est pas
/// encodee en pourcent ici, la source utilise `URL.searchParams`.
pub fn exa_url(cle: Option<&str>) -> String {
    match cle {
        None => EXA_URL.to_string(),
        Some(k) if k.is_empty() => EXA_URL.to_string(),
        Some(k) => format!("{}?exaApiKey={}", EXA_URL, k),
    }
}

/// Valeur d un chiffre en base 36, minuscules et majuscules acceptees.
fn valeur_base36(c: char) -> Option<u32> {
    if c.is_ascii_digit() {
        Some((c as u32) - ('0' as u32))
    } else if c.to_ascii_lowercase() >= 'a' && c.to_ascii_lowercase() <= 'z' {
        Some(10 + ((c.to_ascii_lowercase() as u32) - ('a' as u32)))
    } else {
        None
    }
}

/// Parite d un entier ecrit en base 36, comme `parseInt(s, 36) % 2`.
///
/// Rend None quand il n y a aucun prefixe valide, ce qui correspond a NaN
/// en JS. Le calcul reste modulo 2 a chaque pas pour ne jamais deborder.
fn parite_base36(texte: &str) -> Option<bool> {
    let t = texte.trim_start();
    let t = t
        .strip_prefix('+')
        .or_else(|| t.strip_prefix('-'))
        .unwrap_or(t);
    let mut vu = false;
    let mut parite: u32 = 0;
    for c in t.chars() {
        match valeur_base36(c) {
            Some(d) => {
                vu = true;
                parite = (parite * 36 + d) % 2;
            }
            None => break,
        }
    }
    if vu { Some(parite == 0) } else { None }
}

/// Fournisseur par defaut tire du checksum, miroir de la derniere ligne de
/// `selectProvider` en TS.
///
/// `checksum(sessionID) ?? "0"` devient ici un parametre : la fonction de
/// hachage vit dans un autre module et n est pas portee. Pair donne "exa",
/// impair ou invalide donne "parallel" car `NaN % 2 === 0` est faux en JS.
pub fn fournisseur_par_defaut(checksum: Option<&str>) -> Provider {
    let texte = checksum.unwrap_or("0");
    match parite_base36(texte) {
        Some(true) => Provider::Exa,
        Some(false) => Provider::Parallel,
        None => Provider::Parallel,
    }
}

/// Choisit le fournisseur, miroir de `selectProvider` en TS.
///
/// Priorites : surcharge explicite, puis flag parallel, puis flag exa,
/// puis parite du checksum. Les flags parallel et exa gagent dans cet
/// ordre quand les deux sont vrais.
pub fn select_provider(
    checksum: Option<&str>,
    enable_exa: bool,
    enable_parallel: bool,
    surcharge: Option<Provider>,
) -> Provider {
    if let Some(p) = surcharge {
        return p;
    }
    if enable_parallel {
        return Provider::Parallel;
    }
    if enable_exa {
        return Provider::Exa;
    }
    fournisseur_par_defaut(checksum)
}

/// Analyse un bloc JSON MCP et rend le premier texte non vide.
///
/// Rend None si le bloc ne commence pas par "{" apres trim, si le JSON
/// est invalide, ou si tous les textes sont vides. Le test de vide reprend
/// le `.find((item) => item.text)` de la source qui ignore une chaine vide.
pub fn parse_payload(payload: &str) -> Option<String> {
    let t = payload.trim();
    if !t.starts_with('{') {
        return None;
    }
    let v: McpResult = serde_json::from_str(t).ok()?;
    v.result
        .content
        .into_iter()
        .find(|item| !item.text.is_empty())
        .map(|item| item.text)
}

/// Analyse une reponse brute directe ou SSE, miroir de `parseResponse`.
///
/// Ordre de la source : essai direct sur le corps trimme, puis balayage des
/// lignes avec prefixe exact `"data: "`, premier texte non vide gagne.
/// Le prefixe fait 6 octets ASCII donc `&ligne[6..]` est sur une frontiere.
pub fn parse_response(corps: &str) -> Option<String> {
    let t = corps.trim();
    if !t.is_empty() && t.starts_with('{') {
        match serde_json::from_str::<McpResult>(t) {
            Err(_) => return None,
            Ok(v) => {
                let trouve = v
                    .result
                    .content
                    .into_iter()
                    .find(|item| !item.text.is_empty())
                    .map(|item| item.text);
                if trouve.is_some() {
                    return trouve;
                }
                // JSON valide mais sans texte : on continue vers les lignes
                // SSE comme la source qui recoit `undefined` sans echouer.
            }
        }
    }
    for ligne in corps.split('\n') {
        if !ligne.starts_with("data: ") {
            continue;
        }
        if let Some(texte) = parse_payload(&ligne[6..]) {
            return Some(texte);
        }
    }
    None
}

/// Construit les arguments Exa avec defauts, miroir du `yield* callMcp`
/// pour le cas `provider === "exa"` en TS.
pub fn build_exa_args(entree: &WebSearchInput) -> ExaArgs {
    ExaArgs {
        query: entree.query.clone(),
        search_type: search_type_as_str(resolve_search_type(entree.search_type)).to_string(),
        num_results: resolve_num_results(entree.num_results),
        livecrawl: livecrawl_as_str(resolve_livecrawl(entree.livecrawl)).to_string(),
        context_max_characters: entree.context_max_characters,
    }
}

/// Construit les arguments Parallel, miroir du cas `provider !== "exa"`.
///
/// La source envoie `objective = query`, `search_queries = [query]` et
/// `session_id = sessionID`. Le commentaire sur le modele non expose est
/// un choix d appel, pas une donnee, donc non porte.
pub fn build_parallel_args(query: &str, session_id: &str) -> ParallelArgs {
    ParallelArgs {
        objective: query.to_string(),
        search_queries: vec![query.to_string()],
        session_id: session_id.to_string(),
    }
}

/// Construit la sortie finale, miroir de `{ provider, text: text ?? NO_RESULTS }`.
///
/// `??` ne remplace que None : `Some("")` survit tel quel.
pub fn resolve_output(fournisseur: Provider, texte: Option<String>) -> WebSearchOutput {
    WebSearchOutput {
        provider: fournisseur,
        text: texte.unwrap_or_else(|| NO_RESULTS.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_corps_vide_ou_sans_data_donne_rien() {
        assert_eq!(parse_response(""), None);
        assert_eq!(parse_response("   \n  "), None);
        assert_eq!(parse_response("bonjour\npas de prefixe"), None);
        assert_eq!(parse_response("data: pas du json"), None);
        assert_eq!(parse_response("data:bonjour sans espace"), None);
    }

    #[test]
    fn un_json_direct_non_vide_est_retenu() {
        let corps = r#"{"result":{"content":[{"type":"text","text":"bonjour"}]}}"#;
        assert_eq!(parse_response(corps), Some("bonjour".to_string()));
        // Espaces autour acceptes comme le trim de la source.
        let entoure = format!("  \n{}\n  ", corps);
        assert_eq!(parse_response(&entoure), Some("bonjour".to_string()));
    }

    #[test]
    fn un_texte_vide_est_ignore_au_profit_du_suivant() {
        // Premier item vide ignore, comme `.find((item) => item.text)` en TS.
        let corps = r#"{"result":{"content":[{"type":"text","text":""},{"type":"text","text":"utile"}]}}"#;
        assert_eq!(parse_payload(corps), Some("utile".to_string()));
        let que_du_vide = r#"{"result":{"content":[{"type":"text","text":""}]}}"#;
        assert_eq!(parse_payload(que_du_vide), None);
        assert_eq!(parse_response(que_du_vide), None);
    }

    #[test]
    fn la_premiere_ligne_sse_valide_gagne() {
        let corps = "event: message\n\
             data: {\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"premier\"}]}}\n\
             data: {\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"second\"}]}}\n";
        assert_eq!(parse_response(corps), Some("premier".to_string()));
        // Ligne vide ignoree puis ligne valide.
        let corps2 = "data: {\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"\"}]}}\n\
             data: {\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"bon\"}]}}";
        assert_eq!(parse_response(corps2), Some("bon".to_string()));
    }

    #[test]
    fn le_choix_du_fournisseur_respecte_les_priorites() {
        // Surcharge gagne sur tout.
        assert_eq!(
            select_provider(Some("0"), true, true, Some(Provider::Exa)),
            Provider::Exa
        );
        assert_eq!(
            select_provider(Some("0"), false, false, Some(Provider::Parallel)),
            Provider::Parallel
        );
        // Flags : parallel gagne quand les deux sont vrais.
        assert_eq!(select_provider(Some("0"), true, true, None), Provider::Parallel);
        assert_eq!(select_provider(Some("1"), true, false, None), Provider::Exa);
        assert_eq!(select_provider(Some("0"), false, true, None), Provider::Parallel);
        // Parite du checksum : "0" pair donne exa, "1" impair donne parallel.
        assert_eq!(select_provider(Some("0"), false, false, None), Provider::Exa);
        assert_eq!(select_provider(Some("1"), false, false, None), Provider::Parallel);
        assert_eq!(select_provider(None, false, false, None), Provider::Exa);
        // Invalide donne parallel car NaN % 2 n est jamais 0 en JS.
        assert_eq!(select_provider(Some("!!!"), false, false, None), Provider::Parallel);
    }

    #[test]
    fn l_url_exa_et_les_defauts_sont_fideles() {
        // Piege falsy : chaine vide = URL de base.
        assert_eq!(exa_url(None), EXA_URL);
        assert_eq!(exa_url(Some("")), EXA_URL);
        let avec_cle = exa_url(Some("abc123"));
        assert!(avec_cle.starts_with(EXA_URL));
        assert!(avec_cle.contains("exaApiKey=abc123"));
        // Defauts `||` de la source.
        assert_eq!(resolve_num_results(None), 8);
        assert_eq!(resolve_num_results(Some(0)), 8);
        assert_eq!(resolve_num_results(Some(5)), 5);
        assert_eq!(resolve_livecrawl(None), LivecrawlMode::Fallback);
        assert_eq!(resolve_search_type(None), SearchType::Auto);
        // `??` : None donne NO_RESULTS mais "" survit.
        assert_eq!(resolve_output(Provider::Exa, None).text, NO_RESULTS);
        assert_eq!(resolve_output(Provider::Exa, Some("".to_string())).text, "");
        // Bornes PositiveInt.
        assert!(est_num_results_valide(1));
        assert!(est_num_results_valide(20));
        assert!(!est_num_results_valide(0));
        assert!(!est_num_results_valide(21));
        assert!(!est_context_max_valide(0));
        assert!(!est_context_max_valide(50_001));
    }

    #[test]
    fn la_serialisation_garde_le_camel_case() {
        let entree = WebSearchInput {
            query: "rust".to_string(),
            num_results: Some(5),
            livecrawl: Some(LivecrawlMode::Preferred),
            search_type: Some(SearchType::Deep),
            context_max_characters: Some(1000),
        };
        let json = serde_json::to_value(&entree).expect("serialisation");
        assert_eq!(json["query"], serde_json::json!("rust"));
        assert_eq!(json["numResults"], serde_json::json!(5));
        assert!(json.get("num_results").is_none());
        assert_eq!(json["type"], serde_json::json!("deep"));
        assert_eq!(json["livecrawl"], serde_json::json!("preferred"));
        assert_eq!(json["contextMaxCharacters"], serde_json::json!(1000));
        let back: WebSearchInput = serde_json::from_value(json).expect("retour");
        assert_eq!(back, entree);
        // Option absente = champ absent.
        let mini = WebSearchInput {
            query: "q".to_string(),
            num_results: None,
            livecrawl: None,
            search_type: None,
            context_max_characters: None,
        };
        let v = serde_json::to_value(&mini).expect("serialisation mini");
        assert!(v.get("numResults").is_none());
        assert!(v.get("type").is_none());
        assert!(v.get("contextMaxCharacters").is_none());
        // Fournisseur et requete MCP.
        let req = requete_mcp("web_search_exa", build_exa_args(&entree));
        let rv = serde_json::to_value(&req).expect("requete");
        assert_eq!(rv["jsonrpc"], serde_json::json!("2.0"));
        assert_eq!(rv["id"], serde_json::json!(1));
        assert_eq!(rv["method"], serde_json::json!("tools/call"));
        assert_eq!(rv["params"]["arguments"]["numResults"], serde_json::json!(5));
    }
}
```
===FIN===

CONFIANCE : moyenne
POINT FAIBLE : effets et reseau non portes (client HTTP, timeout 25s, limite 256 Kio, permission, couches Effect et noeuds) ; encodage de la cle Exa simplifie en concat simple sans pourcent ; echec de decodage JSON mappe en None total au lieu d un echec Effect, avec retour immediat sans repli SSE quand le corps direct commence par "{" ; parite base36 suppose minuscules et majuscules equivalentes comme parseInt JS.
A VERIFIER : relire que "numResults", "contextMaxCharacters" et "type" gardent leur casse exacte, que Some("") survit dans resolve_output mais pas dans parse_payload, et que la priorite parallel avant exa est bien l ordre voulu.
