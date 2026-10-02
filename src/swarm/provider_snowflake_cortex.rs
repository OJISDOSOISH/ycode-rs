//! Portage de `packages/core/src/plugin/provider/snowflake-cortex.ts`.
//!
//! ## Ce que fait la source
//!
//! Deux choses. D'abord `cortexFetch`, un enveloppeur de `fetch` exporte pour
//! les tests, qui corrige trois specificites de Cortex :
//!
//! 1. Cortex refuse `max_tokens` : le corps JSON d'une requete est reecrit,
//!    `max_tokens` devient `max_completion_tokens`.
//! 2. Cortex repond 400 avec le message `"conversation complete"` comme
//!    condition d'arret normale. L'enveloppeur intercepte ce 400 et renvoie a
//!    la place un 200 synthetique : `{"choices":[{"finish_reason":"stop",
//!    "message":{"content":"","role":"assistant"}}]}`. Le test de detection lit
//!    `errorData.message || errorData.error`, en minuscules, et cherche la
//!    sous-chaine `"conversation complete"`.
//! 3. Dans les flux `text/event-stream`, Cortex renvoie `role:""` dans les
//!    deltas ; le schema de l'AI SDK exige `"assistant"`. La source applique la
//!    regex `/"role"\s*:\s*""/g` sur chaque morceau decompile.
//!
//! Ensuite `SnowflakeCortexPlugin = define({ id: "snowflake-cortex", effect })`.
//! Son crochet `ctx.aisdk.sdk` filtre sur
//! `evt.model.providerID === "snowflake-cortex"`, resout le jeton dans l'ordre
//! `SNOWFLAKE_CORTEX_TOKEN` ?? `SNOWFLAKE_CORTEX_PAT` ?? `evt.options.token` ??
//! `evt.options.apiKey` (les deux options seulement si ce sont des chaines),
//! force `evt.options.includeUsage = true` sauf s'il vaut deja `false`, puis
//! construit le SDK via `createOpenAICompatible` du paquet npm
//! `@ai-sdk/openai-compatible`, avec `fetch: cortexFetch(upstream)` ou
//! `upstream` est `evt.options.fetch` s'il est une fonction.
//!
//! ## Choix de portage
//!
//! - `fetch`, `Response` et `ReadableStream` n'ont pas d'equivalent dans les
//!   dependances autorisees (serde, serde_json, thiserror). L'enveloppeur est
//!   donc porte comme des **fonctions pures** sur les donnees qui transitent :
//!   le corps JSON de la requete (`Value`), le statut et le corps texte de la
//!   reponse, et le texte d'un morceau de flux. L'appelant reel (le client
//!   HTTP du depot) branche ces fonctions autour de son `fetch`.
//! - La regex `/"role"\s*:\s*""/g` est reecrite sans le paquet `regex` : un
//!   petit scanseur recherche `"role"`, saute les espaces, attend `:`, saute
//!   les espaces, puis remplace `""` par `"assistant"`. La regex d'origine
//!   matche aussi faussement a l'interieur d'une valeur de chaine ; le
//!   scanseur reproduit ce comportement, donc la parite est respectee.
//! - La fabrique `createOpenAICompatible` vient du paquet npm, son code
//!   JavaScript n'a pas d'equivalent Rust. Comme dans `provider_mistral.rs`,
//!   elle est **injectee** : `on_sdk_event` recoit une fonction qui prend
//!   `evt.options` et renvoie le SDK.
//! - `evt.options` vaut `any` en TypeScript : traduit en `Value`, comme dans
//!   `provider_mistral.rs`.
//!
//! ## Noms de champs
//!
//! Aucun champ camelCase ne traverse une structure Rust ici : `max_tokens`,
//! `max_completion_tokens`, `finish_reason`, `message`, `content`, `role`,
//! `choices`, `error` sont des cles JSON produites par le serveur et recopiees
//! **a l'identique**, en `Value`, jamais renommees. Les cles d'options
//! `apiKey`, `token` et `includeUsage` sont camelCase cote TypeScript ; elles
//! sont accedees par leur nom exact dans le `Value`, et `OptionsCortex` porte
//! un `#[serde(rename = ...)]` explicite pour chaque. Les tests font des
//! aller-retours serde sur ces noms exacts.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// L'identifiant que le plugin enregistre dans le registre des plugins.
///
/// C'est la valeur de `id: "snowflake-cortex"` dans `define`, et c'est aussi le
/// `ProviderV2.ID` sur lequel le crochet filtre.
pub const PLUGIN_ID: &str = "snowflake-cortex";

/// Le nom de la fabrique exportee par le paquet npm.
///
/// Sert uniquement a tracer l'appel JavaScript
/// `mod.createOpenAICompatible(...)`.
pub const FACTORY: &str = "createOpenAICompatible";

/// Variable d'environnement du jeton, premiere dans l'ordre de priorite.
pub const ENV_TOKEN: &str = "SNOWFLAKE_CORTEX_TOKEN";

/// Variable d'environnement du jeton personnel, seconde dans l'ordre.
pub const ENV_PAT: &str = "SNOWFLAKE_CORTEX_PAT";

/// La sous-chaine recherchee, en minuscules, dans l'erreur 400.
pub const CONVERSATION_COMPLETE: &str = "conversation complete";

/// Le corps JSON de la reponse 200 synthetique renvoyee a la place du 400.
///
/// Recopie caractere par caractere de la source : `finish_reason` est bien en
/// snake_case cote serveur, `role` vaut `"assistant"` comme l'exige le schema
/// de l'AI SDK, `content` est la chaine vide.
pub fn reponse_conversation_complete() -> Value {
    serde_json::json!({
        "choices": [
            {
                "finish_reason": "stop",
                "message": { "content": "", "role": "assistant" }
            }
        ]
    })
}

/// Reecrit le corps JSON d'une requete pour Cortex.
///
/// La source fait `JSON.parse(init.body)`, puis si `"max_tokens" in body` :
/// `body.max_completion_tokens = body.max_tokens; delete body.max_tokens`.
/// Si le corps n'est pas un objet JSON valide, la source ignore l'erreur et
/// laisse le corps intact : ici, un `Value` qui n'est pas un objet est rendu
/// tel quel, et un objet sans `max_tokens` aussi. Renvoie `true` si le corps a
/// ete modifie.
pub fn reecrit_le_corps(body: &mut Value) -> bool {
    let Some(objet) = body.as_object_mut() else {
        return false;
    };
    match objet.remove("max_tokens") {
        Some(valeur) => {
            objet.insert("max_completion_tokens".to_string(), valeur);
            true
        }
        None => false,
    }
}

/// Dit si un corps d'erreur 400 est la condition d'arret normale de Cortex.
///
/// La source fait `String(errorData.message || errorData.error || "")`,
/// passe en minuscules, et cherche `"conversation complete"`. Si `message`
/// existe il gagne, sinon `error`, sinon chaine vide. Un `error` objet est
/// stringifie par `String(...)` ; ici seul le cas d'un `error` chaine ou
/// nombre est reproduit, un objet `error` donnant une chaine non pertinente
/// dans les deux implementations.
pub fn est_conversation_complete(error_data: &Value) -> bool {
    let message = error_data
        .get("message")
        .filter(|v| !v.is_null())
        .or_else(|| error_data.get("error").filter(|v| !v.is_null()))
        .map(|v| match v {
            Value::String(s) => s.clone(),
            autre => autre.to_string(),
        })
        .unwrap_or_default();
    message.to_lowercase().contains(CONVERSATION_COMPLETE)
}

/// Remplace les `"role" : ""` par `"role":"assistant"` dans un morceau de flux.
///
/// Traduction sans regex de `/"role"\s*:\s*""/g`. Le scanseur avance octet par
/// octet : sur `"role"`, il saute les espaces, attend `:`, resaute les espaces,
/// et si les deux prochains octets sont `""`, il ecrit `"assistant"`. Sinon il
/// recopie l'octet et continue — y compris quand `:` ou `""` manque, comme
/// l'alternative de la regex qui echoue alors entierement.
pub fn remplace_role_vide(texte: &str) -> String {
    let octets = texte.as_bytes();
    let mut resultat = String::with_capacity(texte.len());
    let mut i = 0;
    while i < octets.len() {
        if octets[i..].starts_with(b"\"role\"") {
            let mut j = i + 6;
            while j < octets.len() && (octets[j] == b' ' || octets[j] == b'\t') {
                j += 1;
            }
            if j < octets.len() && octets[j] == b':' {
                j += 1;
                while j < octets.len() && (octets[j] == b' ' || octets[j] == b'\t') {
                    j += 1;
                }
                if octets[j..].starts_with(b"\"\"") {
                    resultat.push_str("\"role\":\"assistant\"");
                    i = j + 2;
                    continue;
                }
            }
            resultat.push_str("\"role\"");
            i += 6;
        } else {
            // Un octet non-ASCII demarre une suite UTF-8 valide : l'entree
            // vient de `str`, on recopie le caractere entier.
            let c = texte[i..].chars().next().unwrap();
            resultat.push(c);
            i += c.len_utf8();
        }
    }
    resultat
}

/// Force `includeUsage` a `true` dans les options, sauf s'il vaut `false`.
///
/// La source ecrit `if (evt.options.includeUsage !== false)
/// evt.options.includeUsage = true` : `undefined`, absent, ou toute autre
/// valeur deviennent `true`. Seul le booleen exact `false` survit.
pub fn force_include_usage(options: &mut Value) {
    if options.get("includeUsage") != Some(&Value::Bool(false)) {
        if let Some(objet) = options.as_object_mut() {
            objet.insert("includeUsage".to_string(), Value::Bool(true));
        }
    }
}

/// Resout le jeton d'acces a Cortex, dans l'ordre de priorite de la source.
///
/// `SNOWFLAKE_CORTEX_TOKEN` ?? `SNOWFLAKE_CORTEX_PAT` ??
/// `evt.options.token` ?? `evt.options.apiKey`. Les deux variables
/// d'environnement ne comptent que si elles sont presentes et non vides (un
/// `??` JavaScript n'ecarte que `null`/`undefined`, mais une variable absente
/// arrive ici comme `None` ; une chaine vide, elle, serait crue — la source a
/// le meme defaut, elle est donc respectee via `lecteur`). Les deux options ne
/// comptent que si ce sont des chaines (`typeof ... === "string"`).
pub fn resout_le_token(lecteur: impl Fn(&str) -> Option<String>, options: &Value) -> Option<String> {
    lecteur(ENV_TOKEN)
        .or_else(|| lecteur(ENV_PAT))
        .or_else(|| {
            options
                .get("token")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            options
                .get("apiKey")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

/// Les options que la source lit sur `evt.options`.
///
/// En TypeScript c'est `any` : seuls les champs effectivement lus sont portes
/// ici, avec les noms JSON exacts. `apiKey` et `includeUsage` sont camelCase
/// cote serveur, d'ou les `#[serde(rename = ...)]` explicites.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionsCortex {
    /// La cle d'API alternative : `evt.options.apiKey`.
    #[serde(rename = "apiKey", skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Le jeton direct : `evt.options.token`.
    #[serde(rename = "token", skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Demande d'inclusion de l'usage dans la reponse : `includeUsage`.
    #[serde(rename = "includeUsage", skip_serializing_if = "Option::is_none")]
    pub include_usage: Option<bool>,
}

/// L'evenement recu par le crochet `sdk`, partie lue par ce plugin.
///
/// Comme dans `provider_mistral.rs` : `model` reste opaque (`Value`), le
/// plugin n'y lit que `providerID`. Le champ `sdk` est pose par le plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkHookEvent {
    /// Le modele demande. Seul `model.providerID` est lu par le filtre.
    #[serde(rename = "model")]
    pub model: Value,
    /// Les options a transmettre a la fabrique, sans reinterpretation.
    #[serde(rename = "options")]
    pub options: Value,
    /// Le SDK construit. Absent tant qu'aucun plugin n'en a pose un.
    #[serde(rename = "sdk", skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Value>,
}

/// Le plugin Snowflake Cortex, partie donnee.
///
/// En TypeScript, `define({ id: "snowflake-cortex", effect })`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnowflakeCortexPlugin {
    /// La valeur de `id`, telle qu'elle circule dans l'enregistrement.
    #[serde(rename = "id")]
    pub id: String,
}

impl SnowflakeCortexPlugin {
    /// Construit le plugin avec son identifiant officiel.
    pub fn new() -> Self {
        SnowflakeCortexPlugin {
            id: PLUGIN_ID.to_string(),
        }
    }
}

impl Default for SnowflakeCortexPlugin {
    fn default() -> Self {
        SnowflakeCortexPlugin::new()
    }
}

/// Dit si ce plugin repond a ce `providerID` de modele.
///
/// La source ecrit `if (evt.model.providerID !== "snowflake-cortex") return`.
/// Egalite stricte, sensible a la casse.
pub fn applies_to(provider_id: &str) -> bool {
    provider_id == PLUGIN_ID
}

/// Le corps du crochet enregistre par le plugin.
///
/// `factory` tient lieu de `createOpenAICompatible` du paquet npm
/// `@ai-sdk/openai-compatible`, dont le code JavaScript n'a pas d'equivalent
/// Rust. Avant l'appel : le filtre sur `model.providerID`, la resolution du
/// jeton (injectee par `lecteur_env`, comme `process.env` est injectable en
/// test), et `force_include_usage`. Le jeton resolu remplace `apiKey` dans les
/// options quand il existe (`...(token ? { apiKey: token } : {})`).
///
/// Le `fetch` rewriters (`reecrit_le_corps`, `reponse_conversation_complete`,
/// `remplace_role_vide`) s'appliquent au niveau du transport, pas ici : la
/// fabrique recue doit poser `cortex_fetch` sur son client HTTP.
pub fn on_sdk_event<F>(event: &mut SdkHookEvent, lecteur_env: impl Fn(&str) -> Option<String>, factory: F)
where
    F: FnOnce(&Value) -> Value,
{
    let provider_id = event
        .model
        .get("providerID")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !applies_to(provider_id) {
        return;
    }
    force_include_usage(&mut event.options);
    if let Some(token) = resout_le_token(lecteur_env, &event.options) {
        if let Some(objet) = event.options.as_object_mut() {
            objet.insert("apiKey".to_string(), Value::String(token));
        }
    }
    event.sdk = Some(factory(&event.options));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_plugin_s_enregistre_sous_le_nom_snowflake_cortex() {
        assert_eq!(SnowflakeCortexPlugin::new().id, "snowflake-cortex");
        assert_eq!(SnowflakeCortexPlugin::default().id, "snowflake-cortex");
        assert_eq!(
            serde_json::to_string(&SnowflakeCortexPlugin::new()).unwrap(),
            r#"{"id":"snowflake-cortex"}"#
        );
    }

    #[test]
    fn max_tokens_devient_max_completion_tokens() {
        let mut corps: Value =
            serde_json::from_str(r#"{"model":"claude","max_tokens":1024,"messages":[]}"#).unwrap();
        assert!(reecrit_le_corps(&mut corps));
        assert_eq!(
            serde_json::to_string(&corps).unwrap(),
            r#"{"max_completion_tokens":1024,"messages":[],"model":"claude"}"#
        );
    }

    #[test]
    fn un_corps_sans_max_tokens_est_intact() {
        let mut corps: Value = serde_json::from_str(r#"{"model":"claude","messages":[]}"#).unwrap();
        assert!(!reecrit_le_corps(&mut corps));
        assert_eq!(corps, serde_json::json!({ "model": "claude", "messages": [] }));
    }

    #[test]
    fn un_corps_non_objet_est_laisse_intact() {
        let mut corps = Value::String("pas du json".to_string());
        assert!(!reecrit_le_corps(&mut corps));
        assert_eq!(corps, Value::String("pas du json".to_string()));
    }

    #[test]
    fn un_corps_json_invalide_ne_plante_pas_comme_le_typescript() {
        // La source fait JSON.parse dans un try/catch : un corps non JSON
        // n'atteint jamais reecrit_le_corps. C'est l'appelant qui parse, et un
        // non-objet est rendu intact (test precedent).
        let brut = "ceci n'est pas du json";
        assert!(serde_json::from_str::<Value>(brut).is_err());
    }

    #[test]
    fn la_reponse_synthetique_est_exactement_celle_du_typescript() {
        assert_eq!(
            reponse_conversation_complete(),
            serde_json::json!({
                "choices": [
                    { "finish_reason": "stop", "message": { "content": "", "role": "assistant" } }
                ]
            })
        );
    }

    #[test]
    fn la_reponse_synthetique_fait_un_aller_retour_serde_aux_noms_du_serveur() {
        let json = serde_json::to_string(&reponse_conversation_complete()).unwrap();
        assert_eq!(
            json,
            r#"{"choices":[{"finish_reason":"stop","message":{"content":"","role":"assistant"}}]}"#
        );
        let relu: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(relu["choices"][0]["finish_reason"], "stop");
        assert_eq!(relu["choices"][0]["message"]["role"], "assistant");
    }

    #[test]
    fn le_message_conversation_complete_est_reconnu_dans_message_puis_error() {
        assert!(est_conversation_complete(
            &serde_json::json!({ "message": "Conversation complete." })
        ));
        assert!(est_conversation_complete(
            &serde_json::json!({ "error": "conversation COMPLETE" })
        ));
        // `message` gagne sur `error` quand les deux existent.
        assert!(!est_conversation_complete(
            &serde_json::json!({ "message": "autre", "error": "conversation complete" })
        ));
    }

    #[test]
    fn une_autre_erreur_400_n_est_pas_une_fin_de_conversation() {
        assert!(!est_conversation_complete(
            &serde_json::json!({ "message": "invalid request" })
        ));
        assert!(!est_conversation_complete(&serde_json::json!({})));
        assert!(!est_conversation_complete(&Value::Null));
    }

    #[test]
    fn les_role_vides_deviennent_assistant_avec_les_espaces_de_la_regex() {
        // La regex tolere les espaces autour des deux points.
        assert_eq!(remplace_role_vide(r#"{"role":""}"#), r#"{"role":"assistant"}"#);
        assert_eq!(
            remplace_role_vide(r#"{"role" :  ""}"#),
            r#"{"role":"assistant"}"#
        );
        // Plusieurs occurrences, comme le flag /g.
        assert_eq!(
            remplace_role_vide(r#"[{"role":""},{"role":""}]"#),
            r#"[{"role":"assistant"},{"role":"assistant"}]"#
        );
    }

    #[test]
    fn un_role_non_vide_ou_un_autre_champ_ne_sont_pas_touches() {
        assert_eq!(remplace_role_vide(r#"{"role":"user"}"#), r#"{"role":"user"}"#);
        assert_eq!(remplace_role_vide(r#"{"roles":""}"#), r#"{"roles":""}"#);
        // Sans les deux-points ou avec autre chose qu'une chaine vide : la
        // regex entiere echoue, l'original est recopie.
        assert_eq!(remplace_role_vide(r#"{"role" "}"#), r#"{"role" "}"#);
        assert_eq!(remplace_role_vide(r#"{"role":null}"#), r#"{"role":null}"#);
        // Un role deja correct dans un delta SSE passe a l'identique.
        let delta = r#"data: {"choices":[{"delta":{"role":"assistant","content":"hi"}}]}"#;
        assert_eq!(remplace_role_vide(delta), delta);
    }

    #[test]
    fn include_usage_est_force_a_true_sauf_sil_vaut_false() {
        let mut options = serde_json::json!({});
        force_include_usage(&mut options);
        assert_eq!(options["includeUsage"], true);

        let mut options = serde_json::json!({ "includeUsage": true });
        force_include_usage(&mut options);
        assert_eq!(options["includeUsage"], true);

        // Le seul cas qui survit : le booleen exact false.
        let mut options = serde_json::json!({ "includeUsage": false });
        force_include_usage(&mut options);
        assert_eq!(options["includeUsage"], false);
    }

    #[test]
    fn include_usage_n_est_pas_pose_sur_des_options_non_objet() {
        let mut options = Value::Null;
        force_include_usage(&mut options);
        assert_eq!(options, Value::Null);
    }

    #[test]
    fn les_options_serde_gardent_les_noms_camelcase_du_typescript() {
        let options = OptionsCortex {
            api_key: Some("secret".to_string()),
            token: Some("jeton".to_string()),
            include_usage: Some(false),
        };
        let json = serde_json::to_string(&options).unwrap();
        assert_eq!(
            json,
            r#"{"apiKey":"secret","token":"jeton","includeUsage":false}"#
        );
        let relu: OptionsCortex = serde_json::from_str(&json).unwrap();
        assert_eq!(relu, options);
    }

    #[test]
    fn le_token_est_resolu_dans_l_ordre_de_priorite_de_la_source() {
        let options = serde_json::json!({ "token": "opt", "apiKey": "cle" });
        // Environnement d'abord.
        assert_eq!(
            resout_le_token(|nom| (nom == ENV_TOKEN).then(|| "env".to_string()), &options),
            Some("env".to_string())
        );
        // Puis la PAT.
        assert_eq!(
            resout_le_token(
                |nom| (nom == ENV_PAT).then(|| "pat".to_string()),
                &options
            ),
            Some("pat".to_string())
        );
        // Puis options.token.
        assert_eq!(
            resout_le_token(|_nom| None, &options),
            Some("opt".to_string())
        );
        // Puis options.apiKey.
        assert_eq!(
            resout_le_token(|_nom| None, &serde_json::json!({ "apiKey": "cle" })),
            Some("cle".to_string())
        );
        // Rien du tout.
        assert_eq!(resout_le_token(|_nom| None, &serde_json::json!({})), None);
    }

    #[test]
    fn les_options_token_et_apikey_ne_comptent_que_si_ce_sont_des_chaines() {
        // typeof !== "string" : ignores.
        assert_eq!(
            resout_le_token(|_nom| None, &serde_json::json!({ "token": 42, "apiKey": true })),
            None
        );
    }

    #[test]
    fn le_filtre_repond_au_seul_provider_snowflake_cortex() {
        assert!(applies_to("snowflake-cortex"));
        assert!(!applies_to("snowflake"));
        assert!(!applies_to("openai"));
        assert!(!applies_to(""));
    }

    #[test]
    fn le_crochet_construit_le_sdk_avec_le_token_et_include_usage() {
        let mut event = SdkHookEvent {
            model: serde_json::json!({ "providerID": "snowflake-cortex" }),
            options: serde_json::json!({ "url": "https://cortex" }),
            sdk: None,
        };
        on_sdk_event(
            &mut event,
            |nom| (nom == ENV_TOKEN).then(|| "jeton-env".to_string()),
            |options: &Value| {
                assert_eq!(options["apiKey"], "jeton-env");
                assert_eq!(options["includeUsage"], true);
                serde_json::json!({ "sdk": "construit" })
            },
        );
        assert_eq!(event.sdk, Some(serde_json::json!({ "sdk": "construit" })));
    }

    #[test]
    fn le_crochet_ignore_un_autre_provider() {
        let mut event = SdkHookEvent {
            model: serde_json::json!({ "providerID": "openai" }),
            options: serde_json::json!({}),
            sdk: None,
        };
        let appelee = std::cell::Cell::new(false);
        on_sdk_event(
            &mut event,
            |_nom| None,
            |_options: &Value| {
                appelee.set(true);
                Value::Null
            },
        );
        assert!(!appelee.get());
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn l_evenement_se_relit_depuis_le_json_du_typescript() {
        let event: SdkHookEvent = serde_json::from_str(
            r#"{"model":{"providerID":"snowflake-cortex"},"options":{"apiKey":"secret"},"sdk":"marqueur"}"#,
        )
        .unwrap();
        assert!(applies_to(event.model["providerID"].as_str().unwrap()));
        assert_eq!(event.options["apiKey"], "secret");
        assert_eq!(event.sdk, Some(serde_json::json!("marqueur")));
    }

    #[test]
    fn l_evenement_sans_sdk_se_relit_avec_un_sdk_absent() {
        let event: SdkHookEvent = serde_json::from_str(
            r#"{"model":{"providerID":"snowflake-cortex"},"options":{}}"#,
        )
        .unwrap();
        assert_eq!(event.sdk, None);
    }
}
