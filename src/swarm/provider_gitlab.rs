//! Portage de `packages/core/src/plugin/provider/gitlab.ts`.
//!
//! ## Ce que fait la source
//!
//! `GitLabPlugin` est un plugin declare par `define({ id: "gitlab", effect })`.
//! Son effet enregistre **deux** crochets, tous deux sur `ctx.aisdk` :
//!
//! 1. Un crochet `sdk` : si `evt.package` est exactement `"gitlab-ai-provider"`,
//!    il importe le paquet et pose `evt.sdk = mod.createGitLab({...})` avec des
//!    valeurs par defaut calculees :
//!    - `instanceUrl` : `evt.options.instanceUrl` s'il est une chaine, sinon la
//!      variable d'environnement `GITLAB_INSTANCE_URL`, sinon
//!      `"https://gitlab.com"` (piege classique : `??` seulement sur la branche
//!      non-chaine, une chaine vide reste une chaine valide) ;
//!    - `apiKey` : `evt.options.apiKey` s'il est une chaine, sinon
//!      `process.env.GITLAB_TOKEN` (qui peut donc etre `undefined`, contrairement
//!      a `instanceUrl` qui a une valeur finale par defaut) ;
//!    - `aiGatewayHeaders` : un objet par defaut (un `User-Agent` compose a
//!      partir de la version d'installation, de `mod.VERSION` et de la plateforme
//!      `os`, plus `"anthropic-beta": "context-1m-2025-08-07"`) surcharge par
//!      `evt.options.aiGatewayHeaders` (spread de fin : les cles de l'option
//!      gagnent) ;
//!    - `featureFlags` : `duo_agent_platform_agentic_chat: true` et
//!      `duo_agent_platform: true`, surcharges par `evt.options.featureFlags`.
//!
//! 2. Un crochet `language` : si `evt.model.providerID` est `"gitlab"` :
//!    - `featureFlags` vaut `evt.options.featureFlags` s'il est un objet non
//!      nul, sinon `{}` (test de veracite `typeof === "object" && truthy`) ;
//!    - si `evt.model.api.id` commence par `"duo-workflow-"`, on appelle
//!      `sdk.workflowChat(gitlab.isWorkflowModel(api.id) ? api.id : "duo-workflow",
//!      { featureFlags, workflowDefinition })`, puis on pose
//!      `language.selectedModelRef = workflowRef` si `workflowRef` est une chaine
//!      du corps de la requete. `workflowRef` et `workflowDefinition` sont lus
//!      dans `evt.model.request.body` avec le meme test de chaine ;
//!    - sinon `evt.language = sdk.agenticChat(api.id, { aiGatewayHeaders,
//!      featureFlags })`.
//!
//! ## Choix de portage
//!
//! - Comme dans `provider_mistral.rs`, le champ `effect` est une fonction : le
//!   plugin est decoupe en une donnee (`GitLabPlugin`, l'`id`) et deux
//!   comportements (`on_sdk_event`, `on_language_event`).
//! - `createGitLab`, `isWorkflowModel`, `workflowChat` et `agenticChat` viennent
//!   du paquet npm `gitlab-ai-provider`, sans equivalent Rust. Elles sont
//!   injectees ou reduites a des decisions de donnees.
//! - Le `User-Agent` depend de `InstallationVersion`, de `mod.VERSION` et de
//!   `os.platform()/release()/arch()` : rien de tout cela n'est connaissable ici.
//!   `on_sdk_event` recoit donc le `user_agent` deja compose, en parametre.
//! - Les variables d'environnement sont lues via une closure injectee
//!   (`env`), pour rester testable et pour que l'appelant decide de la strategie
//!   (`std::env::var` chez l'appelant reel).
//! - `evt.options`, `evt.model` et les resultats SDK restent des
//!   `serde_json::Value`, traduction honnete de `any` / de schemas portes
//!   ailleurs. Les sous-objets que ce plugin lit vraiment (`instanceUrl`,
//!   `apiKey`, `aiGatewayHeaders`, `featureFlags`, `workflowRef`,
//!   `workflowDefinition`) sont extraits explicitement.
//! - Le spread JavaScript `{defaut, ...options}` signifie : les cles presentes
//!   dans `options` gagnent, y compris avec la valeur `null`. Les fusions
//!   ci-dessous reproduisent exactement cela (present gagne, absent herite).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// L'identifiant que le plugin enregistre dans le registre des plugins.
///
/// C'est la valeur de `id: "gitlab"` dans `define`.
pub const PLUGIN_ID: &str = "gitlab";

/// Le seul nom de paquet npm auquel le crochet `sdk` repond.
pub const PACKAGE: &str = "gitlab-ai-provider";

/// Le nom de la fabrique exportee par le paquet npm `gitlab-ai-provider`.
///
/// Sert uniquement a tracer l'appel JavaScript `mod.createGitLab(...)`.
pub const FACTORY: &str = "createGitLab";

/// Le `providerID` de modele qui declenche le crochet `language`
/// (`ProviderV2.ID.gitlab`).
pub const PROVIDER_ID: &str = "gitlab";

/// Le prefixe d'identifiant de modele qui bascule sur `workflowChat`.
pub const PREFIXE_WORKFLOW: &str = "duo-workflow-";

/// Le modele de remplacement quand `isWorkflowModel` refuse l'identifiant.
pub const MODELE_WORKFLOW_PAR_DEFAUT: &str = "duo-workflow";

/// L'instance GitLab par defaut, apres echec de l'option puis de l'environnement.
pub const INSTANCE_URL_PAR_DEFAUT: &str = "https://gitlab.com";

/// Variable d'environnement de repli pour `instanceUrl`.
pub const VAR_ENV_INSTANCE_URL: &str = "GITLAB_INSTANCE_URL";

/// Variable d'environnement de repli pour `apiKey`. Noter la difference : ce
/// repli n'a pas de troisieme valeur par defaut, la cle peut rester absente.
pub const VAR_ENV_TOKEN: &str = "GITLAB_TOKEN";

/// La valeur d'en-tete `anthropic-beta` posee par defaut.
pub const ANTHROPIC_BETA: &str = "context-1m-2025-08-07";

/// Le nom de l'en-tete porte par le `User-Agent` par defaut.
pub const EN_TETE_USER_AGENT: &str = "User-Agent";

/// Le nom de l'en-tete anthropique pose par defaut.
pub const EN_TETE_ANTHROPIC_BETA: &str = "anthropic-beta";

/// Le plugin GitLab, partie donnee.
///
/// En TypeScript, `define({ id: "gitlab", effect })`. Le champ `effect` est une
/// fonction et n'a pas de place dans une structure serialisable : il est porte
/// par `on_sdk_event` et `on_language_event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitLabPlugin {
    /// La valeur de `id`, telle qu'elle circule dans l'enregistrement.
    #[serde(rename = "id")]
    pub id: String,
}

impl GitLabPlugin {
    /// Construit le plugin avec son identifiant officiel.
    pub fn new() -> Self {
        GitLabPlugin {
            id: PLUGIN_ID.to_string(),
        }
    }
}

impl Default for GitLabPlugin {
    fn default() -> Self {
        GitLabPlugin::new()
    }
}

/// Fusionne deux objets JSON comme le spread JavaScript `{ ...defaut, ...surcharge }`.
///
/// Les cles presentes dans `surcharge` gagnent, y compris si leur valeur est
/// `null` : c'est le comportement du spread, et le code source compte dessus.
/// Si `surcharge` n'est pas un objet, les defauts sont conserves intacts.
fn fusionner_objets(defaut: &Map<String, Value>, surcharge: Option<&Value>) -> Map<String, Value> {
    let mut resultat = defaut.clone();
    if let Some(Value::Object(ajouts)) = surcharge {
        for (cle, valeur) in ajouts {
            resultat.insert(cle.clone(), valeur.clone());
        }
    }
    resultat
}

/// Dit si une valeur JSON est une chaine non vide au sens du source.
///
/// La source ecrit `typeof x === "string" ? x : ...` : la chaine vide **est**
/// acceptee (un ternaire, pas un `??`). Cette fonction encode donc seulement le
/// test de type ; ne pas y ajouter de test de vacuite.
fn est_chaine(valeur: Option<&Value>) -> bool {
    matches!(valeur, Some(Value::String(_)))
}

/// Extrait la chaine d'une valeur si elle en est une.
fn chaine_de(valeur: Option<&Value>) -> Option<String> {
    match valeur {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Construit les options transmises a `createGitLab`.
///
/// `options` est `evt.options` tel quel. `env` lit une variable d'environnement
/// et renvoie `None` si elle est absente (le `??` du source traite `undefined`
/// et absent de la meme facon). `user_agent` est le `User-Agent` deja compose
/// par l'appelant, car la source le construit avec `InstallationVersion`,
/// `mod.VERSION` et `os.*`, inconnus ici.
///
/// Ordre de resolution reproduit a l'identique :
/// - `instanceUrl` : option si chaine, sinon `GITLAB_INSTANCE_URL`, sinon
///   `"https://gitlab.com"` ;
/// - `apiKey` : option si chaine, sinon `GITLAB_TOKEN`, sinon absente
///   (contrairement a `instanceUrl`, il n'y a pas de troisieme valeur) ;
/// - `aiGatewayHeaders` : defauts puis spread de l'option (l'option gagne) ;
/// - `featureFlags` : les deux drapeaux duo puis spread de l'option.
pub fn construire_options_sdk(
    options: &Value,
    env: impl Fn(&str) -> Option<String>,
    user_agent: &str,
) -> Value {
    let objet = options.as_object();

    let instance_url = if est_chaine(objet.and_then(|o| o.get("instanceUrl"))) {
        chaine_de(objet.and_then(|o| o.get("instanceUrl"))).unwrap_or_default()
    } else {
        env(VAR_ENV_INSTANCE_URL).unwrap_or_else(|| INSTANCE_URL_PAR_DEFAUT.to_string())
    };

    let api_key = if est_chaine(objet.and_then(|o| o.get("apiKey"))) {
        chaine_de(objet.and_then(|o| o.get("apiKey")))
    } else {
        env(VAR_ENV_TOKEN)
    };

    let mut en_tetes_defaut = Map::new();
    en_tetes_defaut.insert(
        EN_TETE_USER_AGENT.to_string(),
        Value::String(user_agent.to_string()),
    );
    en_tetes_defaut.insert(
        EN_TETE_ANTHROPIC_BETA.to_string(),
        Value::String(ANTHROPIC_BETA.to_string()),
    );
    let en_tetes = fusionner_objets(
        &en_tetes_defaut,
        objet.and_then(|o| o.get("aiGatewayHeaders")),
    );

    let mut drapeaux_defaut = Map::new();
    drapeaux_defaut.insert(
        "duo_agent_platform_agentic_chat".to_string(),
        Value::Bool(true),
    );
    drapeaux_defaut.insert("duo_agent_platform".to_string(), Value::Bool(true));
    let drapeaux = fusionner_objets(&drapeaux_defaut, objet.and_then(|o| o.get("featureFlags")));

    let mut resultat = Map::new();
    resultat.insert("instanceUrl".to_string(), Value::String(instance_url));
    if let Some(cle) = api_key {
        resultat.insert("apiKey".to_string(), Value::String(cle));
    }
    resultat.insert("aiGatewayHeaders".to_string(), Value::Object(en_tetes));
    resultat.insert("featureFlags".to_string(), Value::Object(drapeaux));
    Value::Object(resultat)
}

/// Dit si ce plugin repond a ce nom de paquet, pour le crochet `sdk`.
///
/// La source ecrit `evt.package !== "gitlab-ai-provider"` puis `return`.
/// Egalite stricte, sensible a la casse.
pub fn applies_to(package: &str) -> bool {
    package == PACKAGE
}

/// Dit si le crochet `language` s'applique a ce `providerID`.
///
/// La source ecrit `evt.model.providerID !== ProviderV2.ID.gitlab` puis `return`.
pub fn est_fournisseur_gitlab(provider_id: &str) -> bool {
    provider_id == PROVIDER_ID
}

/// Dit si l'identifiant de modele bascule sur `workflowChat`.
///
/// La source ecrit `evt.model.api.id.startsWith("duo-workflow-")`. Le prefixe
/// seul compte : `"duo-workflow"` sans tiret final ne correspond pas.
pub fn est_duo_workflow(api_id: &str) -> bool {
    api_id.starts_with(PREFIXE_WORKFLOW)
}

/// Le premier argument de `workflowChat` :
/// `isWorkflowModel(api.id) ? api.id : "duo-workflow"`.
///
/// `is_workflow_model` est le predicat du paquet npm, injecte car sans
/// equivalent Rust dans ce depot.
pub fn modele_workflow_effectif<'a>(
    api_id: &'a str,
    est_modele_workflow: impl Fn(&str) -> bool,
) -> &'a str {
    if est_modele_workflow(api_id) {
        api_id
    } else {
        MODELE_WORKFLOW_PAR_DEFAUT
    }
}

/// Le corps de requete lu par le chemin du workflow, `evt.model.request.body`.
///
/// Deux champs seulement sont lus par le source, tous deux testes avec
/// `typeof === "string"` : une valeur non-chaine vaut `undefined`, et un champ
/// `undefined` n'est pas serialise, comme en JavaScript.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CorpsRequete {
    /// La reference de workflow a poser sur `language.selectedModelRef`.
    #[serde(rename = "workflowRef", skip_serializing_if = "Option::is_none")]
    pub workflow_ref: Option<String>,
    /// La definition de workflow passee a `workflowChat`.
    #[serde(rename = "workflowDefinition", skip_serializing_if = "Option::is_none")]
    pub workflow_definition: Option<String>,
}

/// Extrait `workflowRef` et `workflowDefinition` du corps de requete.
///
/// Si le corps n'est pas un objet, les deux champs sont absents : c'est le
/// comportement des tests `typeof` du source sur un corps quelconque.
pub fn extraire_corps(body: &Value) -> CorpsRequete {
    match body.as_object() {
        Some(_) => serde_json::from_value(body.clone()).unwrap_or_default(),
        None => CorpsRequete::default(),
    }
}

/// Extrait `featureFlags` des options pour le crochet `language`.
///
/// La source ecrit `typeof evt.options.featureFlags === "object" &&
/// evt.options.featureFlags ? evt.options.featureFlags : {}`. Attention au piege
/// de veracite : un objet est truthy meme vide, mais `null` est un `typeof
/// "object"` falsy. Donc objet non nul -> garde tel quel, tout le reste -> `{}`.
/// C'est plus permissif qu'un test de chaine : un tableau ou une chaine passeraient
/// le `typeof` et seraient gardes ; on encode fidelement ce comportement.
pub fn extraire_feature_flags(options: &Value) -> Value {
    match options.get("featureFlags") {
        Some(drapeaux) if drapeaux.is_object() && !drapeaux.is_null() => drapeaux.clone(),
        _ => Value::Object(Map::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le `User-Agent` compose par l'appelant, pour les tests.
    const UA: &str = "opencode/1.0.0 gitlab-ai-provider/1.2.3 (linux 6.0; x64)";

    fn env_vide(_cle: &str) -> Option<String> {
        None
    }

    #[test]
    fn le_plugin_s_enregistre_sous_le_nom_gitlab() {
        assert_eq!(GitLabPlugin::new().id, "gitlab");
        assert_eq!(GitLabPlugin::default().id, "gitlab");
        assert_eq!(serde_json::to_string(&GitLabPlugin::new()).unwrap(), r#"{"id":"gitlab"}"#);
    }

    #[test]
    fn le_seul_paquet_reconnu_est_celui_du_gitlab() {
        assert!(applies_to("gitlab-ai-provider"));
        assert!(applies_to(PACKAGE));
        assert!(!applies_to("@ai-sdk/gitlab"));
        assert!(!applies_to("gitlab-ai-provider-x"));
    }

    #[test]
    fn la_casse_du_nom_de_paquet_compte() {
        assert!(!applies_to("GitLab-AI-Provider"));
        assert!(!applies_to(""));
    }

    #[test]
    fn les_options_par_defaut_utilisent_l_instance_et_les_drapeaux_officiels() {
        let options = construire_options_sdk(&serde_json::json!({}), env_vide, UA);
        assert_eq!(
            options,
            serde_json::json!({
                "instanceUrl": "https://gitlab.com",
                "aiGatewayHeaders": {
                    "User-Agent": UA,
                    "anthropic-beta": "context-1m-2025-08-07",
                },
                "featureFlags": {
                    "duo_agent_platform_agentic_chat": true,
                    "duo_agent_platform": true,
                },
            })
        );
    }

    #[test]
    fn les_options_explicites_gagnent_sur_les_defauts() {
        let options = construire_options_sdk(
            &serde_json::json!({
                "instanceUrl": "https://gitlab.example.com",
                "apiKey": "secret",
                "aiGatewayHeaders": { "anthropic-beta": "autre" },
                "featureFlags": { "duo_agent_platform": false },
            }),
            env_vide,
            UA,
        );
        assert_eq!(
            options,
            serde_json::json!({
                "instanceUrl": "https://gitlab.example.com",
                "apiKey": "secret",
                "aiGatewayHeaders": {
                    "User-Agent": UA,
                    "anthropic-beta": "autre",
                },
                "featureFlags": {
                    "duo_agent_platform_agentic_chat": true,
                    "duo_agent_platform": false,
                },
            })
        );
    }

    #[test]
    fn les_variables_d_environnement_completent_les_options() {
        let env = |cle: &str| {
            match cle {
                "GITLAB_INSTANCE_URL" => Some("https://gitlab.interne".to_string()),
                "GITLAB_TOKEN" => Some("jeton".to_string()),
                _ => None,
            }
        };
        let options = construire_options_sdk(&serde_json::json!({}), env, UA);
        assert_eq!(options["instanceUrl"], "https://gitlab.interne");
        assert_eq!(options["apiKey"], "jeton");
    }

    #[test]
    fn l_instance_explicite_batte_l_environnement() {
        let env = |_cle: &str| Some("https://gitlab.interne".to_string());
        let options = construire_options_sdk(
            &serde_json::json!({ "instanceUrl": "https://gitlab.explicite" }),
            env,
            UA,
        );
        assert_eq!(options["instanceUrl"], "https://gitlab.explicite");
    }

    #[test]
    fn une_instance_vide_est_conservee_telle_quelle() {
        // La source ecrit un ternaire `typeof === "string"`, pas un `??` :
        // la chaine vide est une chaine, elle gagne donc sur l'environnement.
        let env = |_cle: &str| Some("https://gitlab.interne".to_string());
        let options = construire_options_sdk(&serde_json::json!({ "instanceUrl": "" }), env, UA);
        assert_eq!(options["instanceUrl"], "");
    }

    #[test]
    fn une_cle_non_chaine_est_remplacee_par_l_environnement() {
        let env = |_cle: &str| Some("jeton-env".to_string());
        let options = construire_options_sdk(&serde_json::json!({ "apiKey": 42 }), env, UA);
        assert_eq!(options["apiKey"], "jeton-env");
    }

    #[test]
    fn sans_cle_nulle_part_api_key_est_absente_du_json() {
        let options = construire_options_sdk(&serde_json::json!({}), env_vide, UA);
        assert!(options.get("apiKey").is_none());
        let json = serde_json::to_string(&options).unwrap();
        assert!(!json.contains("apiKey"));
    }

    #[test]
    fn un_tokento_null_de_l_option_est_traite_comme_absent() {
        // typeof null === "object" : ce n'est pas une chaine, donc l'option
        // ne compte pas et l'environnement est consulte.
        let env = |_cle: &str| Some("jeton".to_string());
        let options = construire_options_sdk(&serde_json::json!({ "apiKey": null }), env, UA);
        assert_eq!(options["apiKey"], "jeton");
    }

    #[test]
    fn un_spread_null_ecrase_le_defaut_d_en_tete() {
        // Le spread JavaScript garde un null explicite : la cle est presente,
        // sa valeur est null. La fusion reproduit ce comportement.
        let options = construire_options_sdk(
            &serde_json::json!({ "aiGatewayHeaders": { "anthropic-beta": null } }),
            env_vide,
            UA,
        );
        assert_eq!(options["aiGatewayHeaders"]["anthropic-beta"], Value::Null);
        assert_eq!(options["aiGatewayHeaders"]["User-Agent"], UA);
    }

    #[test]
    fn des_en_tetes_non_objet_laisse_les_defauts_intacts() {
        let options =
            construire_options_sdk(&serde_json::json!({ "aiGatewayHeaders": "bof" }), env_vide, UA);
        assert_eq!(options["aiGatewayHeaders"]["anthropic-beta"], ANTHROPIC_BETA);
    }

    #[test]
    fn les_noms_de_champs_des_options_sont_exacts() {
        // Les noms TS sont instanceUrl, apiKey, aiGatewayHeaders, featureFlags.
        let options = construire_options_sdk(
            &serde_json::json!({ "apiKey": "secret", "instanceUrl": "https://x" }),
            env_vide,
            UA,
        );
        let json = serde_json::to_string(&options).unwrap();
        assert!(json.contains("\"instanceUrl\""));
        assert!(json.contains("\"apiKey\""));
        assert!(json.contains("\"aiGatewayHeaders\""));
        assert!(json.contains("\"featureFlags\""));
        assert!(json.contains("\"duo_agent_platform_agentic_chat\":true"));
        assert!(json.contains("\"duo_agent_platform\":true"));
        // Les drapeaux duo ne prennent pas de camelCase.
        assert!(!json.contains("duoAgentPlatform"));
    }

    #[test]
    fn les_options_se_relisent_depuis_le_json_du_typescript() {
        let deserialise: Value = serde_json::from_str(
            r#"{"instanceUrl":"https://gitlab.com","apiKey":"k","aiGatewayHeaders":{"User-Agent":"ua"},"featureFlags":{"duo_agent_platform":true}}"#,
        )
        .unwrap();
        assert_eq!(deserialise["instanceUrl"], "https://gitlab.com");
        assert_eq!(deserialise["aiGatewayHeaders"]["User-Agent"], "ua");
        assert_eq!(deserialise["featureFlags"]["duo_agent_platform"], true);
    }

    #[test]
    fn le_crochet_language_ne_vise_que_gitlab() {
        assert!(est_fournisseur_gitlab("gitlab"));
        assert!(!est_fournisseur_gitlab("gitlab.chat"));
        assert!(!est_fournisseur_gitlab(""));
    }

    #[test]
    fn le_prefixe_duo_workflow_est_resolu_strictement() {
        assert!(est_duo_workflow("duo-workflow-"));
        assert!(est_duo_workflow("duo-workflow-agent"));
        assert!(!est_duo_workflow("duo-workflow"));
        assert!(!est_duo_workflow("Duo-Workflow-x"));
        assert!(!est_duo_workflow(""));
    }

    #[test]
    fn le_modele_effectif_reste_l_identifiant_quand_is_workflow_model_accepte() {
        assert_eq!(modele_workflow_effectif("duo-workflow-agent", |_| true), "duo-workflow-agent");
    }

    #[test]
    fn le_modele_effectif_retombe_sur_duo_workflow_sans_tiret() {
        assert_eq!(modele_workflow_effectif("duo-workflow-agent", |_| false), "duo-workflow");
    }

    #[test]
    fn le_corps_de_requete_extrait_les_deux_champs_camelcase() {
        let corps = extraire_corps(&serde_json::json!({
            "workflowRef": "ref-1",
            "workflowDefinition": "def-1",
        }));
        assert_eq!(corps.workflow_ref.as_deref(), Some("ref-1"));
        assert_eq!(corps.workflow_definition.as_deref(), Some("def-1"));
    }

    #[test]
    fn le_corps_de_requete_serialise_garde_les_noms_du_typescript() {
        let corps = CorpsRequete {
            workflow_ref: Some("ref-1".to_string()),
            workflow_definition: Some("def-1".to_string()),
        };
        let json = serde_json::to_string(&corps).unwrap();
        assert_eq!(
            json,
            r#"{"workflowRef":"ref-1","workflowDefinition":"def-1"}"#
        );
        // Aller-retour complet.
        let relu: CorpsRequete = serde_json::from_str(&json).unwrap();
        assert_eq!(relu, corps);
    }

    #[test]
    fn un_workflow_ref_non_chaine_est_absent_du_corps() {
        let corps = extraire_corps(&serde_json::json!({ "workflowRef": 7 }));
        assert_eq!(corps.workflow_ref, None);
    }

    #[test]
    fn un_corps_absent_donne_un_corps_vide() {
        let corps = extraire_corps(&Value::Null);
        assert_eq!(corps, CorpsRequete::default());
        let corps_tableau = extraire_corps(&serde_json::json!([1]));
        assert_eq!(corps_tableau, CorpsRequete::default());
    }

    #[test]
    fn les_feature_flags_options_sont_gardes_tels_quels() {
        let drapeaux = extraire_feature_flags(&serde_json::json!({
            "featureFlags": { "perso": true },
        }));
        assert_eq!(drapeaux, serde_json::json!({ "perso": true }));
    }

    #[test]
    fn des_feature_flags_null_ou_absents_donnent_un_objet_vide() {
        assert_eq!(extraire_feature_flags(&serde_json::json!({})), serde_json::json!({}));
        assert_eq!(
            extraire_feature_flags(&serde_json::json!({ "featureFlags": null })),
            serde_json::json!({})
        );
        // Un objet vide est truthy en JavaScript : il est garde.
        assert_eq!(
            extraire_feature_flags(&serde_json::json!({ "featureFlags": {} })),
            serde_json::json!({})
        );
    }
}
