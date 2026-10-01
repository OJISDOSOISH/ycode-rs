//! Portage de `packages/core/src/plugin/provider/openai.ts`.
//!
//! ## Ce que fait la source
//!
//! `OpenAIPlugin` est un plugin declare par `define({ id: "openai", effect })`.
//! Son effet fait quatre choses : enregistrer deux methodes OAuth
//! d'integration (`browser` et `headless`, toutes deux pour
//! "ChatGPT Pro/Plus"), desactiver le modele `gpt-5-chat-latest` dans le
//! catalogue des fournisseurs OpenAI, fabriquer le SDK via
//! `@ai-sdk/openai`, et acheminer les modeles OpenAI par l'API Responses.
//!
//! ## Choix de portage
//!
//! - Une grosse moitie du fichier vit sur des effects `Effect`, `fetch`,
//!   `node:http` et `crypto.subtle` : serveur de callback local, boucle de
//!   polling device-auth, generation PKCE (aleatoire + SHA-256). Aucun de ces
//!   mecanismes n'a d'equivalent dans les dependances autorisees
//!   (`serde`, `serde_json`, `thiserror`). On ne les invente pas : ce fichier
//!   porte **les donnees et les fonctions pures**, qui sont precisement les
//!   parties ou un contrat JSON existe et peut se casser :
//!   `TokenResponse`, `Claims`, `Credential.OAuth`, `authorizeURL`, les corps
//!   de requete `application/x-www-form-urlencoded`, et l'extraction du
//!   `accountID` depuis les JWT.
//! - `credential()` calcule `Date.now() + (expires_in ?? 3600) * 1000`. L'heure
//!   courante n'est pas une fonction pure : `construire_credential` la recoit
//!   en parametre (`maintenant_ms`), ce qui reproduit exactement la formule.
//! - La fallback `?? 3600` de `expires_in` est appliquee dans
//!   `construire_credential`, pas dans le champ : le JSON de `TokenResponse`
//!   doit rester identique a la source, donc `expires_in` reste absent du JSON
//!   quand le serveur ne l'envoie pas.
//! - La generation PKCE (`generatePKCE`) exige un generateur aleatoire et
//!   SHA-256. La structure `Pkce` est portee, la production ne l'est pas : un
//!   appelant branche sa propre crypto. Le decodeur base64url utilise par
//!   `claim` est reecrit a la main (sans dependance), avec le padding `=`
//!   tolere mais non exige, comme `Buffer.from(..., "base64url")`.
//! - `Integration.MethodID` est une chaine marque en TypeScript : porte ici
//!   comme `String` nouveau type `MethodID`.
//!
//! ## Noms de champs
//!
//! - `TokenResponse` : les quatre noms (`id_token`, `access_token`,
//!   `refresh_token`, `expires_in`) sont deja en snake_case cote TypeScript —
//!   ce sont des noms renvoyes par le serveur OAuth d'OpenAI. Ils sont
//!   ecrits explicitement dans les `#[serde(rename = ...)]` malgre tout.
//! - `Credential.OAuth` : la la casse TypeScript est camelCase —
//!   `methodID` (et non `method_id`), `accountID` (et non `account_id`) dans
//!   `metadata`. Ce sont les deux renames a ne pas rater.
//! - `Claims` : la cle `"https://api.openai.com/auth"` est un URL litteral,
//!   impossible comme identifiant Rust, d'ou le rename explicite.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Le client OAuth OpenAI utilise par le flux Codex.
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// Le fournisseur d'identite OAuth.
pub const ISSUER: &str = "https://auth.openai.com";

/// Le port du serveur de callback local du flux navigateur.
pub const CALLBACK_PORT: u16 = 1455;

/// La marge de securite ajoutee a l'intervalle de polling device-auth (ms).
pub const POLLING_SAFETY_MARGIN_MS: u64 = 3000;

/// L'identifiant de la methode OAuth navigateur.
pub const BROWSER_METHOD_ID: &str = "chatgpt-browser";

/// L'identifiant de la methode OAuth headless (device auth).
pub const HEADLESS_METHOD_ID: &str = "chatgpt-headless";

/// Le mode du modele desactive dans le catalogue : voir la doc du module.
pub const MODELE_DESACTIVE: &str = "gpt-5-chat-latest";

/// Le paquet npm auquel ce plugin repond pour la fabrique de SDK.
pub const PACKAGE: &str = "@ai-sdk/openai";

/// L'identifiant d'une methode d'integration.
///
/// En TypeScript, `Integration.MethodID.make(...)`, une chaine marquee.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MethodID(pub String);

impl MethodID {
    /// La methode OAuth du flux navigateur.
    pub fn navigateur() -> Self {
        MethodID(BROWSER_METHOD_ID.to_string())
    }

    /// La methode OAuth headless.
    pub fn headless() -> Self {
        MethodID(HEADLESS_METHOD_ID.to_string())
    }
}

/// La paire PKCE du flux d'autorisation.
///
/// En TypeScript, `type Pkce = { verifier, challenge }`. La production de cette
/// paire (43 octets aleatoires + SHA-256 base64url) demande `crypto.subtle` et
/// n'est pas portee ici : l'appelant fournit sa propre crypto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pkce {
    #[serde(rename = "verifier")]
    pub verifier: String,
    #[serde(rename = "challenge")]
    pub challenge: String,
}

/// La reponse du point de terminaison `${ISSUER}/oauth/token`.
///
/// En TypeScript, `type TokenResponse`. Les quatre noms de champs sont deja
/// en snake_case cote TypeScript (ce sont des noms OAuth du serveur) ; les
/// renames sont ecrits malgre tout pour bloquer toute conversion accidentelle.
/// `expires_in` est absent du JSON quand le serveur ne l'envoie pas — la
/// fallback `?? 3600` est appliquee dans `construire_credential`, pas ici.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenResponse {
    #[serde(rename = "id_token")]
    pub id_token: String,
    #[serde(rename = "access_token")]
    pub access_token: String,
    #[serde(rename = "refresh_token")]
    pub refresh_token: String,
    #[serde(
        rename = "expires_in",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub expires_in: Option<u64>,
}

/// Les revendications lues dans un JWT OpenAI.
///
/// En TypeScript, `type Claims`. La cle `"https://api.openai.com/auth"` est un
/// URL litteral : rename explicite obligatoire. Les trois chemins de
/// `extraire_compte` sont, dans l'ordre de la source :
/// `chatgpt_account_id`, puis `claims["https://api.openai.com/auth"]`
/// `.chatgpt_account_id`, puis `organizations[0].id`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Claims {
    #[serde(
        rename = "chatgpt_account_id",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub chatgpt_account_id: Option<String>,
    #[serde(rename = "organizations", skip_serializing_if = "Option::is_none", default)]
    pub organizations: Option<Vec<Organization>>,
    #[serde(
        rename = "https://api.openai.com/auth",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub auth_openai: Option<AuthOpenAi>,
}

/// Un element de `organizations` : `{ id: string }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    #[serde(rename = "id")]
    pub id: String,
}

/// Le sous-objet de la cle `"https://api.openai.com/auth"`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AuthOpenAi {
    #[serde(
        rename = "chatgpt_account_id",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub chatgpt_account_id: Option<String>,
}

/// Les metadonnees attachees a la credential : `{ accountID }` seulement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialMetadata {
    /// Attention : camelCase en TypeScript, d'ou le rename explicite.
    #[serde(rename = "accountID")]
    pub account_id: String,
}

/// La credential OAuth produite par `credential()` dans la source.
///
/// En TypeScript, `Credential.OAuth.make({ type, methodID, refresh, access,
/// expires, metadata? })`. `methodID` est camelCase : rename obligatoire.
/// `metadata` est absent du JSON quand il vaut `undefined`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialOAuth {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "methodID")]
    pub method_id: MethodID,
    #[serde(rename = "refresh")]
    pub refresh: String,
    #[serde(rename = "access")]
    pub access: String,
    /// Date d'expiration en millisecondes depuis l'epoque Unix, comme
    /// `Date.now() + expires_in * 1000` dans la source.
    #[serde(rename = "expires")]
    pub expires: u64,
    #[serde(rename = "metadata", skip_serializing_if = "Option::is_none")]
    pub metadata: Option<CredentialMetadata>,
}

/// Une erreur du portage, pour le decodage JWT.
#[derive(Debug, Error)]
pub enum OpenAiError {
    /// Le token n'a pas de seconde partie (pas un JWT a trois segments).
    #[error("token JWT sans charge utile : {0}")]
    JwtSansCharge(String),
    /// La charge utile n'est pas du base64url valide.
    #[error("charge utile JWT illisible en base64url")]
    Base64UrlInvalide,
    /// La charge decodee n'est pas du JSON conforme a `Claims`.
    #[error("charge utile JWT illisible comme JSON : {0}")]
    JsonInvalide(#[from] serde_json::Error),
}

/// Dit si ce plugin repond a ce nom de paquet.
///
/// La source ecrit `evt.package !== "@ai-sdk/openai"` puis `return`, et filtre
/// aussi `evt.providerID !== ProviderV2.ID.openai`. C'est une egalite stricte,
/// sensible a la casse.
pub fn applies_to(package: &str) -> bool {
    package == PACKAGE
}

/// Le corps form-urlencoded de la requete d'echange de code.
///
/// Portage exact du `new URLSearchParams({ grant_type: "authorization_code",
/// ... })` de `exchange`. L'ordre des parametres est celui de la source.
pub fn corps_echange(code: &str, redirect: &str, pkce: &Pkce) -> String {
    corps_urlencoded(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect),
        ("client_id", CLIENT_ID),
        ("code_verifier", &pkce.verifier),
    ])
}

/// Le corps form-urlencoded de la requete de rafraichissement.
///
/// Portage exact du `new URLSearchParams({ grant_type: "refresh_token", ... })`
/// de `refresh`.
pub fn corps_rafraichissement(token_rafraichissement: &str) -> String {
    corps_urlencoded(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", token_rafraichissement),
        ("client_id", CLIENT_ID),
    ])
}

/// L'URL d'autorisation complete du flux navigateur.
///
/// Portage exact de `authorizeURL` : meme ordre de parametres, memes valeurs
/// litterales (`scope`, `code_challenge_method: "S256"`, `originator`...).
pub fn construire_url_autorisation(redirect: &str, pkce: &Pkce, state: &str) -> String {
    let requete = corps_urlencoded(&[
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect),
        ("scope", "openid profile email offline_access"),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("state", state),
        ("originator", "opencode"),
    ]);
    format!("{ISSUER}/oauth/authorize?{requete}")
}

/// L'URL de la requete d'echange (utilisee par `exchange` et `refresh`).
pub fn url_jeton() -> String {
    format!("{ISSUER}/oauth/token")
}

/// L'URL de callback du flux navigateur.
pub fn url_callback_navigateur() -> String {
    format!("http://localhost:{CALLBACK_PORT}/auth/callback")
}

/// Fabrique la credential OAuth depuis une reponse de jeton.
///
/// Portage de `credential()` : `expires = maintenant_ms
/// + (expires_in ?? 3600) * 1000`, et `metadata` ne vaut `{ accountID }` que
/// si un compte a ete extrait des jetons. L'heure courante est un parametre
/// (`maintenant_ms`) car la source lit `Date.now()`.
pub fn construire_credential(
    method_id: &MethodID,
    jetons: &TokenResponse,
    maintenant_ms: u64,
) -> CredentialOAuth {
    let secondes = jetons.expires_in.unwrap_or(3600);
    CredentialOAuth {
        kind: "oauth".to_string(),
        method_id: method_id.clone(),
        refresh: jetons.refresh_token.clone(),
        access: jetons.access_token.clone(),
        expires: maintenant_ms + secondes * 1000,
        metadata: extraire_compte(jetons)
            .map(|account_id| CredentialMetadata { account_id }),
    }
}

/// Fusionne une credential rafraichie avec les metadonnees existantes.
///
/// Portage de la fin de `refresh()` : `metadata: next.metadata ??
/// value.metadata`. La credential rafraichie gagne, l'ancienne sert de
/// repli. `anciennes_metadata` correspond au `value.metadata` de la source.
pub fn fusionner_credential(
    rafraichie: CredentialOAuth,
    anciennes_metadata: Option<CredentialMetadata>,
) -> CredentialOAuth {
    let metadata = rafraichie.metadata.clone().or(anciennes_metadata);
    CredentialOAuth { metadata, ..rafraichie }
}

/// Extrait l'identifiant de compte, d'abord de l'id_token puis de l'access.
///
/// Portage de `extractAccountID` : `claim(id_token) ?? claim(access_token)`.
pub fn extraire_compte(jetons: &TokenResponse) -> Option<String> {
    Some(revendication(&jetons.id_token).unwrap_or_else(|| revendication(&jetons.access_token)))
}

/// Lit `chatgpt_account_id` dans la charge utile d'un JWT.
///
/// Portage de `claim` : prendre la seconde partie du token (apres le premier
/// `.`), la decoder en base64url, la parser comme `Claims`, puis essayer dans
/// l'ordre `chatgpt_account_id`, `auth_openai.chatgpt_account_id`,
/// `organizations[0].id`. Un token mal forme renvoie `None`, comme le
/// `try/catch` de la source — sauf l'absence de seconde partie, que la source
/// traite aussi par un `return` silencieux mais qui merite ici une erreur
/// explicite pour distinguer les deux cas.
pub fn revendication(token: &str) -> Option<String> {
    let partie = token.split('.').nth(1)?;
    // `decoder_base64url` yields bytes, and `parse` is a `str` method: the UTF-8
    // conversion has to be explicit. Naming the type is also required here,
    // because `.and_then` hands the next closure an un-inferred generic.
    decoder_base64url(partie)
        .ok()
        .and_then(|octets| std::str::from_utf8(&octets).ok())
        .and_then(|texte| serde_json::from_str::<Claims>(texte).ok())
        .and_then(|claims| {
            claims
                .chatgpt_account_id
                .or_else(|| claims.auth_openai.and_then(|auth| auth.chatgpt_account_id))
                .or_else(|| {
                    claims
                        .organizations
                        .and_then(|orgs| orgs.into_iter().next().map(|o| o.id))
                })
        })
}

/// Encode une chaine en `application/x-www-form-urlencoded`, comme
/// `new URLSearchParams(...).toString()`.
///
/// Les caracteres reservees (`&`, `=`, `%`, `+`, espace, etc.) sont
/// echappes en `%XX` avec les majuscules hexadecimales de `URLSearchParams`.
/// L'ordre des paires est preserve.
fn corps_urlencoded(paires: &[(&str, &str)]) -> String {
    paires
        .iter()
        .map(|(cle, valeur)| {
            format!(
                "{}={}",
                encoder_urlencoded(cle),
                encoder_urlencoded(valeur)
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// Encode un composant selon `application/x-www-form-urlencoded`.
///
/// Meme table que `URLSearchParams` : tout sauf `A-Z a-z 0-9 * - . _` est
/// echappe, et l'espace devient `+`.
fn encoder_urlencoded(entree: &str) -> String {
    let mut sortie = String::with_capacity(entree.len());
    for octet in entree.as_bytes() {
        match octet {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                sortie.push(*octet as char)
            }
            b' ' => sortie.push('+'),
            autre => sortie.push_str(&format!("%{autre:02X}")),
        }
    }
    sortie
}

/// Decoders du base64url, sans dependance : reecriture de
/// `Buffer.from(part, "base64url")`.
///
/// Le padding `=` est tolere mais non exige. Un caractere hors alphabet
/// base64url est une erreur.
fn decoder_base64url(entree: &str) -> Result<Vec<u8>, OpenAiError> {
    let mut sortie = Vec::with_capacity(entree.len() * 3 / 4 + 3);
    let mut accumulateur: u32 = 0;
    let mut bits: u32 = 0;
    for caractere in entree.chars() {
        if caractere == '=' {
            break;
        }
        let valeur = match caractere {
            'A'..='Z' => caractere as u32 - 'A' as u32,
            'a'..='z' => caractere as u32 - 'a' as u32 + 26,
            '0'..='9' => caractere as u32 - '0' as u32 + 52,
            '-' => 62,
            '_' => 63,
            _ => return Err(OpenAiError::Base64UrlInvalide),
        };
        accumulateur = (accumulateur << 6) | valeur;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            sortie.push((accumulateur >> bits) as u8);
        }
    }
    Ok(sortie)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un TokenResponse complet, tel que le serveur OAuth le renvoie.
    fn jetons() -> TokenResponse {
        TokenResponse {
            id_token: "a.b.c".to_string(),
            access_token: "at".to_string(),
            refresh_token: "rt".to_string(),
            expires_in: Some(7200),
        }
    }

    #[test]
    fn token_response_serialise_avec_les_noms_du_serveur_oauth() {
        let json = serde_json::to_string(&jetons()).unwrap();
        assert_eq!(
            json,
            r#"{"id_token":"a.b.c","access_token":"at","refresh_token":"rt","expires_in":7200}"#
        );
    }

    #[test]
    fn expires_in_absent_reste_absent_du_json() {
        let mut sans = jetons();
        sans.expires_in = None;
        let json = serde_json::to_string(&sans).unwrap();
        assert_eq!(
            json,
            r#"{"id_token":"a.b.c","access_token":"at","refresh_token":"rt"}"#
        );
        // Aller-retour : l'absence se relit en None, base de la fallback ?? 3600.
        let relu: TokenResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(relu.expires_in, None);
    }

    #[test]
    fn token_response_se_relit_depuis_le_json_du_typescript() {
        let relu: TokenResponse = serde_json::from_str(
            r#"{"id_token":"i","access_token":"a","refresh_token":"r"}"#,
        )
        .unwrap();
        assert_eq!(relu.expires_in, None);
        assert_eq!(relu.refresh_token, "r");
    }

    #[test]
    fn le_corps_d_echange_reproduit_urlsearchparams() {
        let pkce = Pkce {
            verifier: "ver ifier+".to_string(),
            challenge: String::new(),
        };
        assert_eq!(
            corps_echange("le code", "http://localhost:1455/auth/callback", &pkce),
            "grant_type=authorization_code&code=le+code&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback&client_id=app_EMoamEEZ73f0CkXaXp7hrann&code_verifier=ver+ifier%2B"
        );
    }

    #[test]
    fn le_corps_de_rafraichissement_reproduit_urlsearchparams() {
        assert_eq!(
            corps_rafraichissement("rt=val"),
            "grant_type=refresh_token&refresh_token=rt%3Dval&client_id=app_EMoamEEZ73f0CkXaXp7hrann"
        );
    }

    #[test]
    fn l_url_d_autorisation_reproduit_authorizeurl() {
        let pkce = Pkce {
            verifier: "v".to_string(),
            challenge: "challenge&value".to_string(),
        };
        assert_eq!(
            construire_url_autorisation("http://localhost:1455/auth/callback", &pkce, "etat=x"),
            "https://auth.openai.com/oauth/authorize?response_type=code&client_id=app_EMoamEEZ73f0CkXaXp7hrann&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback&scope=openid+profile+email+offline_access&code_challenge=challenge%26value&code_challenge_method=S256&id_token_add_organizations=true&codex_cli_simplified_flow=true&state=etat%3Dx&originator=opencode"
        );
    }

    #[test]
    fn la_credential_calcule_expires_avec_la_fallback_3600() {
        let mut sans = jetons();
        sans.expires_in = None;
        let credential = construire_credential(&MethodID::navigateur(), &sans, 1_000);
        assert_eq!(credential.expires, 1_000 + 3600 * 1000);
        let avec = construire_credential(&MethodID::navigateur(), &jetons(), 1_000);
        assert_eq!(avec.expires, 1_000 + 7200 * 1000);
    }

    #[test]
    fn la_credential_serialise_methodid_en_camelcase() {
        let credential = construire_credential(&MethodID::navigateur(), &jetons(), 0);
        let json = serde_json::to_string(&credential).unwrap();
        assert!(json.contains(r#""methodID":"chatgpt-browser""#));
        assert!(json.contains(r#""type":"oauth""#));
    }

    #[test]
    fn la_credential_n_a_pas_de_metadata_sans_compte() {
        let credential = construire_credential(&MethodID::headless(), &jetons(), 0);
        let json = serde_json::to_string(&credential).unwrap();
        assert!(!json.contains("metadata"));
        assert!(!json.contains("accountID"));
    }

    #[test]
    fn la_metadata_est_camelcase_accountid() {
        let metadata = CredentialMetadata {
            account_id: "acc".to_string(),
        };
        assert_eq!(
            serde_json::to_string(&metadata).unwrap(),
            r#"{"accountID":"acc"}"#
        );
        let relu: CredentialMetadata = serde_json::from_str(r#"{"accountID":"acc"}"#).unwrap();
        assert_eq!(relu.account_id, "acc");
    }

    #[test]
    fn la_fusion_prefere_les_metadata_rafraichies() {
        let rafraichie = CredentialOAuth {
            metadata: Some(CredentialMetadata {
                account_id: "nouveau".to_string(),
            }),
            ..construire_credential(&MethodID::navigateur(), &jetons(), 0)
        };
        let fusion = fusionner_credential(
            rafraichie,
            Some(CredentialMetadata {
                account_id: "ancien".to_string(),
            }),
        );
        assert_eq!(fusion.metadata.unwrap().account_id, "nouveau");
    }

    #[test]
    fn la_fusion_garde_les_anciennes_metadata_en_repli() {
        let rafraichie = construire_credential(&MethodID::navigateur(), &jetons(), 0);
        assert!(rafraichie.metadata.is_none());
        let fusion = fusionner_credential(
            rafraichie,
            Some(CredentialMetadata {
                account_id: "ancien".to_string(),
            }),
        );
        assert_eq!(fusion.metadata.unwrap().account_id, "ancien");
    }

    /// Un JWT factice : entete, charge base64url de `Claims`, signature.
    fn jwt(charge: &str) -> String {
        format!("entete.{charge}.signature")
    }

    #[test]
    fn la_revendication_lit_chatgpt_account_id_dabord() {
        let charge = serde_json::json!({ "chatgpt_account_id": "acc-1" }).to_string();
        let token = jwt(&encoder_urlencoded(&decoder_vers_base64url(charge.as_bytes())));
        assert_eq!(revendication(&token).as_deref(), Some("acc-1"));
    }

    #[test]
    fn la_revendication_lit_la_cle_url_puis_les_organizations() {
        let charge = serde_json::json!({
            "https://api.openai.com/auth": { "chatgpt_account_id": "acc-2" }
        })
        .to_string();
        let token = jwt(&decoder_vers_base64url(charge.as_bytes()));
        assert_eq!(revendication(&token).as_deref(), Some("acc-2"));

        let charge = serde_json::json!({ "organizations": [{ "id": "org-1" }] }).to_string();
        let token = jwt(&decoder_vers_base64url(charge.as_bytes()));
        assert_eq!(revendication(&token).as_deref(), Some("org-1"));
    }

    #[test]
    fn l_ordre_des_chemins_est_celui_de_la_source() {
        // chatgpt_account_id gagne sur la cle URL et sur organizations.
        let charge = serde_json::json!({
            "chatgpt_account_id": "direct",
            "https://api.openai.com/auth": { "chatgpt_account_id": "url" },
            "organizations": [{ "id": "org" }]
        })
        .to_string();
        let token = jwt(&decoder_vers_base64url(charge.as_bytes()));
        assert_eq!(revendication(&token).as_deref(), Some("direct"));
    }

    #[test]
    fn un_token_mal_forme_renvoie_aucun_comme_le_try_catch() {
        assert_eq!(revendication("pas-un-jwt"), None);
        assert_eq!(revendication("entete.cGFzLVdTT04=.fin"), None);
        assert_eq!(revendication("entete.!!!.fin"), None);
    }

    #[test]
    fn extraire_compte_prefere_l_id_token() {
        let mut valeurs = jetons();
        valeurs.id_token = format!(
            "e.{}.s",
            decoder_vers_base64url(
                serde_json::json!({ "chatgpt_account_id": "depuis-id" })
                    .to_string()
                    .as_bytes()
            )
        );
        valeurs.access_token = format!(
            "e.{}.s",
            decoder_vers_base64url(
                serde_json::json!({ "chatgpt_account_id": "depuis-access" })
                    .to_string()
                    .as_bytes()
            )
        );
        assert_eq!(
            extraire_compte(&valeurs).as_deref(),
            Some("depuis-id")
        );
    }

    #[test]
    fn extraire_compte_replie_sur_l_access_token() {
        let mut valeurs = jetons();
        valeurs.id_token = "sans.charge".to_string();
        valeurs.access_token = format!(
            "e.{}.s",
            decoder_vers_base64url(
                serde_json::json!({ "chatgpt_account_id": "depuis-access" })
                    .to_string()
                    .as_bytes()
            )
        );
        assert_eq!(extraire_compte(&valeurs).as_deref(), Some("depuis-access"));
    }

    #[test]
    fn le_decodeur_base64url_tolere_le_padding_et_rejette_le_standard() {
        // 'e' = 30 -> 011100 ; 'A' = 0 -> 000000 ; 01110000 0000xxxx -> 112.
        assert_eq!(decoder_base64url("eA==").unwrap(), vec![112u8]);
        assert_eq!(decoder_base64url("eA").unwrap(), vec![112u8]);
        assert!(decoder_base64url("e+/=").is_err()); // '+' et '/' hors alphabet
    }

    #[test]
    fn le_filtre_de_paquet_est_strict() {
        assert!(applies_to("@ai-sdk/openai"));
        assert!(!applies_to("@ai-sdk/openai-compatible"));
        assert!(!applies_to("@AI-SDK/OpenAI"));
        assert!(!applies_to(""));
    }

    #[test]
    fn les_identifiants_de_methode_sont_ceux_de_la_source() {
        assert_eq!(MethodID::navigateur().0, "chatgpt-browser");
        assert_eq!(MethodID::headless().0, "chatgpt-headless");
        let json = serde_json::to_string(&MethodID::navigateur()).unwrap();
        assert_eq!(json, r#""chatgpt-browser""#);
    }

    /// Encodage base64url de test (avec padding), l'inverse du decodeur.
    fn decoder_vers_base64url(donnees: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut sortie = String::new();
        for bloc in donnees.chunks(3) {
            let b = [
                bloc[0],
                *bloc.get(1).unwrap_or(&0),
                *bloc.get(2).unwrap_or(&0),
            ];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            sortie.push(ALPHABET[(n >> 18) as usize & 63] as char);
            sortie.push(ALPHABET[(n >> 12) as usize & 63] as char);
            if bloc.len() > 1 {
                sortie.push(ALPHABET[(n >> 6) as usize & 63] as char);
            }
            if bloc.len() > 2 {
                sortie.push(ALPHABET[n as usize & 63] as char);
            }
        }
        sortie
    }
}
