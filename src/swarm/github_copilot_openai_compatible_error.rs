//! Portage de `github-copilot/openai-compatible-error.ts`.
//!
//! La source fait 27 lignes et ne contient aucune fonction de transformation.
//! Elle declare un schema de reponse d'erreur, le type de la structure qui
//! regroupe ce schema, et une seule instance de cette structure. Le
//! comportement utile du fichier est donc la **validation**, pas le calcul.
//!
//! ## Forme portee, champ par champ
//!
//! ```text
//! { error: { message: string, type?: string|null, param?: any|null,
//!            code?: string|number|null } }
//! ```
//!
//! - `error` et `error.message` sont **obligatoires**. Ce sont les deux seules
//!   entrees du fichier qui ne sont pas `.nullish()`.
//! - `type`, `param` et `code` sont `.nullish()` : la cle peut etre absente ou
//!   valoir `null`. Ils deviennent `Option<T>`.
//! - `param` est `z.any()` : aucune contrainte du tout, donc `serde_json::Value`
//!   est la traduction exacte, objet, tableau ou booleen compris.
//! - `code` est `z.union([z.string(), z.number()])` : une chaine **ou** un
//!   nombre, jamais l'un converti en l'autre. C'est une union sans tag, donc
//!   `#[serde(untagged)]`, et non `#[serde(tag = "...")]`. L'ordre des variantes
//!   est celui de la source ; il n'a aucune incidence, une chaine JSON ne peut
//!   pas se lire comme un nombre.
//!
//! ## Le piege de la mission : ce que rend une valeur non reconnue
//!
//! Il y a deux traductions possibles et une seule est la bonne.
//!
//! 1. `errorToMessage` est `(data) => data.error.message` : une identite. Il
//!    rend la chaine telle quelle. Une chaine vide rend la chaine vide, pas un
//!    texte de repli, et il ne taille pas, ne nettoie pas, ne complete pas.
//! 2. Une charge utile qui ne correspond pas au schema ne rend **rien du tout**.
//!    `z.union` n'a pas de branche de repli : un `code` booleen, objet ou tableau
//!    fait echouer la validation, et l'appelant ne dispose alors d'aucun objet
//!    sur lequel appeler `errorToMessage`. Il n'y a pas de valeur par defaut, il
//!    y a une erreur.
//!
//! C'est le point que la migration doit verrouiller : un portage qui modelise
//! `code` en `serde_json::Value`, ou qui rend `String::new()` quand le message
//! manque, accepte silencieusement ce que la source refuse. Ici la construction
//! de `OpenAiCompatibleErrorData` passe par une fonction qui renvoie
//! `Result` : un `Err` ne produit aucune donnee, donc `error_to_message` ne peut
//! pas etre appele sur une charge rejetee. Ce que zod verifie a l'execution, le
//! systeme de types Rust le verifie a la compilation.
//!
//! Les tests `un_code_de_type_inconnu_est_rejete_sans_valeur_par_defaut`,
//! `un_message_absent_ou_non_chaine_est_rejete` et
//! `le_message_est_rendut_tel_quel_sans_repli` ferment ce point.
//!
//! ## Noms de champs
//!
//! Aucun nom de ce fichier n'est en camelCase et aucun ne porte de majuscule
//! inattendue : `error`, `message`, `type`, `param`, `code` sont des mots uniques
//! en minuscules. Le piege `projectID` contre `projectId` ne se pose donc pas.
//! Le piege **reel** est `type`, qui est un mot cle reserve en Rust : le champ
//! s'appelle `error_type` et porte un `#[serde(rename = "type")]` explicite,
//! sans quoi il sortirait du cote JSON sous le nom `error_type` et casserait
//! l'echange avec le TypeScript. Le test
//! `les_noms_json_sont_exactement_ceux_de_la_source` compare la chaine JSON
//! complete, donc il couvre ce renommage la.
//!
//! Deux autres comportements par defaut de `z.object` sont figes par un test :
//! une cle inconnue est **retiree** et non refusee (`z.object` ne refuse que
//! `z.strictObject`), et un champ optionnel absent disparait de la sortie au
//! lieu d'y apparaitre avec la valeur `undefined`.
//!
//! ## Divergence assumee
//!
//! `Option` ne distingue pas la cle absente de la valeur `null` : les deux
//! donnent `None`, et le champ disparait du JSON. En TypeScript `null` est
//! conserve et ressort en `"param": null`. La divergence est invisible pour un
//! JSON produit par le TypeScript, visible sur une saisie manuelle, et le test
//! `un_null_lu_disparait_du_json_et_un_null_construit_y_reste` la documente.
//!
//! ## Ce qui n'est volontairement pas porte
//!
//! `isRetryable` est absent de la structure par defaut, donc ce fichier ne
//! definit aucune politique de reprise et aucune n'est inventee ici. Le type
//! `Response` du DOM n'est pas porte non plus : `ProviderResponse` n'en garde
//! que `status`, la seule propriete que la signature nomme de facon fiable, et
//! le reste du corps HTTP n'appartient pas a ce fichier.

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

/// Le code d'erreur, accepte comme une chaine ou comme un nombre.
///
/// En TS : `z.union([z.string(), z.number()]).nullish()`. Pas de tag, donc
/// `#[serde(untagged)]` : serde essaie les variantes dans l'ordre declare,
/// comme fait `z.union`.
///
/// Le nombre est un `serde_json::Number` et non un `f64` : c'est ce qui fait
/// que `429` se re-serialise en `429` et non en `429.0`, alors que
/// `JSON.stringify(429)` vaut `"429"` en JavaScript. Un `f64` introduirait ici
/// une divergence de JSON que rien ne signalerait.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProviderErrorCode {
    /// `z.string()` : un code litteral, comme `"model_not_found"`.
    CodeTexte(String),

    /// `z.number()` : un code chiffre, comme `429`. La chaine `"429"` reste une
    /// chaine, aucune conversion n'est faite dans un sens ni dans l'autre.
    CodeNombre(Number),
}

/// Le corps de l'erreur, l'objet `error` de la reponse.
///
/// En TS, cet objet est anonyme : il est ecrit en place dans le schema. Seule
/// `message` est obligatoire ; les trois autres champs sont `.nullish()`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAiCompatibleErrorBody {
    /// `z.string()`, **non** `.nullish()` : champ obligatoire. Une chaine vide
    /// est valide et reste une chaine vide, c'est `errorToMessage` qui la
    /// rendra telle quelle.
    pub message: String,

    /// Champ `type` du JSON. `type` est un mot cle reserve en Rust, d'ou le nom
    /// interne `error_type` et le renommage explicite qui retablit le nom JSON
    /// d'origine. `z.string().nullish()` : un nombre ici est refuse, `null` et
    /// l'absence donnent `None`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,

    /// Champ `param` du JSON. `z.any().nullish()` : aucune contrainte, donc
    /// `serde_json::Value`. La chaine vide est une valeur comme une autre et ne
    /// devient pas `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<Value>,

    /// Champ `code` du JSON. L'union chaine ou nombre, jamais une valeur par
    /// defaut en cas d'echec : voir `ProviderErrorCode`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<ProviderErrorCode>,
}

/// Une reponse d'erreur d'un fournisseur compatible OpenAI.
///
/// En TS : `z.infer<typeof openaiCompatibleErrorDataSchema>`, c'est-a-dire le
/// resultat **valide** du schema. Cette structure ne peut donc pas representer
/// une charge rejetee, ce qui est exactement la garantie que zod offre a
/// l'execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAiCompatibleErrorData {
    /// Objet `error`, obligatoire : son absence est un echec de validation.
    pub error: OpenAiCompatibleErrorBody,
}

/// Ce que renvoie `openaiCompatibleErrorDataSchema` quand la charge utile ne
/// correspond pas au schema.
///
/// Le TypeScript laisse faire zod et recupere un `ZodError`. Aucun type
/// d'erreur n'est donc nomme dans la source : celui-ci n'expose que le chemin
/// du champ fautif, qui est l'information utile pour diagnostiquer une reponse
/// de fournisseur, et qui evite d'inventer un texte d'erreur qui n'existe pas
/// dans l'original.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderErrorSchemaError {
    /// La racine de la charge utile n'est pas un objet JSON.
    RacineNonObjet,

    /// La cle `error` est absente.
    ErrorAbsent,

    /// La cle `error` est presente mais n'est pas un objet.
    ErrorNonObjet,

    /// `error.message` est absente.
    MessageAbsent,

    /// `error.message` est presente mais n'est pas une chaine, `null` compris.
    MessageNonChaine,

    /// `error.type` est presente, non nulle, et n'est pas une chaine.
    TypeNonChaine,

    /// `error.code` est present, non nul, et n'est ni une chaine ni un nombre.
    CodeNiChaineNiNombre,
}

impl ProviderErrorSchemaError {
    /// Le chemin du champ fautif, comme zod le rapporte. La racine n'a pas de
    /// champ, donc une chaine vide.
    pub fn path(&self) -> &'static str {
        match self {
            ProviderErrorSchemaError::RacineNonObjet => "",
            ProviderErrorSchemaError::ErrorAbsent | ProviderErrorSchemaError::ErrorNonObjet => {
                "error"
            }
            ProviderErrorSchemaError::MessageAbsent
            | ProviderErrorSchemaError::MessageNonChaine => "error.message",
            ProviderErrorSchemaError::TypeNonChaine => "error.type",
            ProviderErrorSchemaError::CodeNiChaineNiNombre => "error.code",
        }
    }
}

/// La reponse HTTP vue par `isRetryable`.
///
/// Le TypeScript nomme `Response`, l'objet du DOM, dont le portage complet ne
/// fait pas partie de ce fichier. Seule la propriete `status` est conservee :
/// c'est la seule donnee que la signature de `isRetryable` designe de facon
/// verifiable sans le paquet `@ai-sdk/provider-utils`, qui n'est pas installe
/// sur ce poste. Aucun comportement de reprise n'est invente ici, la structure
/// par defaut de ce fichier n'en fournit aucun.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderResponse {
    /// Le code de statut HTTP.
    pub status: u16,
}

/// Un schema de validation, c'est-a-dire une fonction de lecture.
///
/// En TS `errorSchema` est une **valeur** de type `ZodType<T>`. En Rust un
/// schema n'est pas une donnee, c'est la fonction qui lit une valeur JSON et
/// renvoie `Ok(T)` ou une erreur. C'est la seule facon d'avoir le meme
/// comportement, puisque zod n'a pas d'equivalent de type en Rust.
pub type ErrorSchemaFn<T> = fn(&Value) -> Result<T, ProviderErrorSchemaError>;

/// `(error: T) => string`, donc `fn(&T) -> String`. La reference evite de
/// consommer la donnee, la chaine est rendue par copie comme en JavaScript ou
/// les chaines sont immuables.
pub type ErrorToMessageFn<T> = fn(&T) -> String;

/// `(response: Response, error?: T) => boolean`, donc le second parametre est
/// un `Option`. Le type est un pointeur de fonction et non une boite a closure :
/// les structures de ce fichier sont des valeurs statiques, et le `isRetryable`
/// de la source est une fonction de bibliotheque sans etat.
pub type IsRetryableFn<T> = fn(&ProviderResponse, Option<&T>) -> bool;

/// Les trois gestes qu'un fournisseur-compatible doit fournir sur ses erreurs.
///
/// En TS : `ProviderErrorStructure<T>`. Les champs optionnels sont des
/// `Option`, et `error_to_message` est obligatoire comme en JavaScript ou le
/// type ne dit pas `?`.
#[derive(Debug, Clone)]
pub struct ProviderErrorStructure<T> {
    /// Le schema qui lit une charge utile de reponse d'erreur.
    pub error_schema: ErrorSchemaFn<T>,

    /// La fonction qui extrait le message affichable.
    pub error_to_message: ErrorToMessageFn<T>,

    /// La decision de reprise. Absente ici, donc `None`, jamais `false` : la
    /// source ne dit pas `isRetryable: false`, elle ne dit rien du tout.
    pub is_retryable: Option<IsRetryableFn<T>>,
}

/// Valide une charge utile JSON contre `openaiCompatibleErrorDataSchema`.
///
/// Renvoie `Err` plutot qu'une valeur partielle ou qu'une valeur par defaut :
/// voir le piege de la mission en tete de module. Les cles inconnues sont
/// retirees et non refusees, comme le fait `z.object`.
pub fn parse_openai_compatible_error_data(
    valeur: &Value,
) -> Result<OpenAiCompatibleErrorData, ProviderErrorSchemaError> {
    let racine = match valeur.as_object() {
        Some(objet) => objet,
        None => return Err(ProviderErrorSchemaError::RacineNonObjet),
    };
    let corps = match racine.get("error") {
        Some(Value::Object(corps)) => corps,
        None => return Err(ProviderErrorSchemaError::ErrorAbsent),
        Some(_) => return Err(ProviderErrorSchemaError::ErrorNonObjet),
    };
    let message = match corps.get("message") {
        Some(Value::String(message)) => message.clone(),
        None => return Err(ProviderErrorSchemaError::MessageAbsent),
        Some(_) => return Err(ProviderErrorSchemaError::MessageNonChaine),
    };
    // `z.string().nullish()` : `null` et l'absence sont equivalents, un nombre
    // est refuse. Une chaine vide reste une chaine vide.
    let error_type = match corps.get("type") {
        None | Some(Value::Null) => None,
        Some(Value::String(valeur_type)) => Some(valeur_type.clone()),
        Some(_) => return Err(ProviderErrorSchemaError::TypeNonChaine),
    };
    // `z.any().nullish()` : tout passe, sauf `null` qui devient l'absence.
    let param = match corps.get("param") {
        None | Some(Value::Null) => None,
        Some(valeur_param) => Some(valeur_param.clone()),
    };
    // `z.union([z.string(), z.number()]).nullish()` : pas de branche de repli.
    let code = match corps.get("code") {
        None | Some(Value::Null) => None,
        Some(Value::String(texte)) => Some(ProviderErrorCode::CodeTexte(texte.clone())),
        Some(Value::Number(nombre)) => Some(ProviderErrorCode::CodeNombre(nombre.clone())),
        Some(_) => return Err(ProviderErrorSchemaError::CodeNiChaineNiNombre),
    };
    Ok(OpenAiCompatibleErrorData {
        error: OpenAiCompatibleErrorBody {
            message,
            error_type,
            param,
            code,
        },
    })
}

/// `(data) => data.error.message`, c'est-a-dire l'identite.
///
/// La chaine est rendue telle quelle : pas de `trim`, pas de valeur de repli
/// pour une chaine vide, pas de normalisation. Cette fonction ne peut pas etre
/// appelee sur une charge rejetee, puisque `OpenAiCompatibleErrorData` n'existe
/// que si la validation a reussi.
pub fn openai_compatible_error_to_message(data: &OpenAiCompatibleErrorData) -> String {
    data.error.message.clone()
}

/// `defaultOpenAICompatibleErrorStructure`.
///
/// Le schema et la fonction de message sont ceux du fichier, et `isRetryable`
/// est **absent** de l'objet d'origine : il vaut donc `None`. Une fonction
/// Rust ne se remplace pas en changeant la valeur du JSON, cette absence est le
/// seul comportement a restituer.
pub fn default_openai_compatible_error_structure() -> ProviderErrorStructure<OpenAiCompatibleErrorData> {
    ProviderErrorStructure {
        error_schema: parse_openai_compatible_error_data,
        error_to_message: openai_compatible_error_to_message,
        is_retryable: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_openai_compatible_error_structure, openai_compatible_error_to_message,
        parse_openai_compatible_error_data, OpenAiCompatibleErrorBody, OpenAiCompatibleErrorData,
        ProviderErrorCode, ProviderErrorSchemaError, ProviderErrorStructure, ProviderResponse,
    };
    use serde::Deserialize;
    use serde_json::{json, Value};

    fn analyser(texte: &str) -> Result<OpenAiCompatibleErrorData, ProviderErrorSchemaError> {
        let valeur: Value = serde_json::from_str(texte).unwrap();
        parse_openai_compatible_error_data(&valeur)
    }

    #[test]
    fn un_message_seul_est_valide_et_les_autres_champs_restent_absents() {
        let data = analyser(r#"{"error":{"message":"Rate limit reached"}}"#).unwrap();
        assert_eq!(data.error.message, "Rate limit reached");
        assert_eq!(data.error.error_type, None);
        assert_eq!(data.error.param, None);
        assert_eq!(data.error.code, None);
    }

    #[test]
    fn les_noms_json_sont_exactement_ceux_de_la_source() {
        // Comparaison de la chaine JSON complete : c'est ce test qui attrape un
        // nom de faux, y compris le `type` qui est un mot cle Rust.
        let data = OpenAiCompatibleErrorData {
            error: OpenAiCompatibleErrorBody {
                message: "m".to_string(),
                error_type: Some("invalid_request_error".to_string()),
                param: Some(json!("model")),
                code: Some(ProviderErrorCode::CodeTexte("model_not_found".to_string())),
            },
        };
        assert_eq!(
            serde_json::to_string(&data).unwrap(),
            r#"{"error":{"message":"m","type":"invalid_request_error","param":"model","code":"model_not_found"}}"#
        );
    }

    #[test]
    fn le_champ_type_ressort_du_cote_json_sous_le_nom_type() {
        let data = analyser(r#"{"error":{"message":"m","type":"rate_limit_exceeded"}}"#).unwrap();
        assert_eq!(data.error.error_type.as_deref(), Some("rate_limit_exceeded"));
        let sortie = serde_json::to_string(&data).unwrap();
        assert!(sortie.contains(r#""type":"rate_limit_exceeded""#));
        assert!(!sortie.contains("error_type"));
    }

    #[test]
    fn un_code_chaine_n_est_pas_converti_en_nombre() {
        let data = analyser(r#"{"error":{"message":"m","code":"429"}}"#).unwrap();
        assert_eq!(
            data.error.code,
            Some(ProviderErrorCode::CodeTexte("429".to_string()))
        );
        assert_eq!(
            serde_json::to_string(&data).unwrap(),
            r#"{"error":{"message":"m","code":"429"}}"#
        );
    }

    #[test]
    fn un_code_nombre_reste_un_nombre_sans_partie_decimale() {
        let data = analyser(r#"{"error":{"message":"m","code":429}}"#).unwrap();
        match &data.error.code {
            Some(ProviderErrorCode::CodeNombre(nombre)) => assert_eq!(nombre.as_i64(), Some(429)),
            autre => panic!("un nombre etait attendu, obtenu {:?}", autre),
        }
        // En JavaScript, JSON.stringify(429) vaut "429" : un f64 donnerait
        // "429.0" et divergerait du TypeScript sans qu'on le voie.
        assert_eq!(
            serde_json::to_string(&data).unwrap(),
            r#"{"error":{"message":"m","code":429}}"#
        );
        let flottant = analyser(r#"{"error":{"message":"m","code":1.5}}"#).unwrap();
        assert_eq!(
            serde_json::to_string(&flottant).unwrap(),
            r#"{"error":{"message":"m","code":1.5}}"#
        );
    }

    #[test]
    fn un_code_de_type_inconnu_est_rejete_sans_valeur_par_defaut() {
        // Le piege de la mission. `z.union` n'a pas de branche de repli : ces
        // quatre charges echouent en TypeScript, donc aucune donnee n'est
        // produite et `errorToMessage` n'est jamais appele. Un `serde_json::Value`
        // a la place de l'union les accepterait tous en silence.
        for texte in [
            r#"{"error":{"message":"m","code":true}}"#,
            r#"{"error":{"message":"m","code":{"a":1}}}"#,
            r#"{"error":{"message":"m","code":["429"]}}"#,
            r#"{"error":{"message":"m","code":"429","type":3}}"#,
        ] {
            let erreur = analyser(texte).unwrap_err();
            if texte.contains("\"type\"") {
                assert_eq!(erreur, ProviderErrorSchemaError::TypeNonChaine);
                assert_eq!(erreur.path(), "error.type");
            } else {
                assert_eq!(erreur, ProviderErrorSchemaError::CodeNiChaineNiNombre);
                assert_eq!(erreur.path(), "error.code");
            }
        }
    }

    #[test]
    fn un_message_absent_ou_non_chaine_est_rejete() {
        // `message` est le seul champ obligatoire de l'objet `error`, et il ne
        // tolere ni l'absence ni `null`, contrairement a ses trois voisins.
        assert_eq!(
            analyser(r#"{"error":{}}"#).unwrap_err(),
            ProviderErrorSchemaError::MessageAbsent
        );
        assert_eq!(
            analyser(r#"{"error":{"message":null}}"#).unwrap_err(),
            ProviderErrorSchemaError::MessageNonChaine
        );
        assert_eq!(
            analyser(r#"{"error":{"message":42}}"#).unwrap_err(),
            ProviderErrorSchemaError::MessageNonChaine
        );
    }

    #[test]
    fn une_racine_invalide_ou_une_erreur_absente_est_rejetee() {
        assert_eq!(
            analyser(r#"{}"#).unwrap_err(),
            ProviderErrorSchemaError::ErrorAbsent
        );
        assert_eq!(
            analyser(r#"{"error":"boom"}"#).unwrap_err(),
            ProviderErrorSchemaError::ErrorNonObjet
        );
        assert_eq!(
            analyser(r#"{"error":null}"#).unwrap_err(),
            ProviderErrorSchemaError::ErrorNonObjet
        );
        assert_eq!(
            analyser(r#"[1,2]"#).unwrap_err(),
            ProviderErrorSchemaError::RacineNonObjet
        );
        assert_eq!(ProviderErrorSchemaError::RacineNonObjet.path(), "");
    }

    #[test]
    fn les_champs_loisibles_valent_null_et_disparaissent_du_json() {
        // `.nullish()` accepte `null`. Le champ disparait ensuite de la sortie,
        // comme une cle absente : le TypeScript fait de meme pour une cle
        // absente, et c'est `undefined` qui ne se serialise pas.
        let data =
            analyser(r#"{"error":{"message":"m","type":null,"param":null,"code":null}}"#).unwrap();
        assert_eq!(data.error.error_type, None);
        assert_eq!(data.error.param, None);
        assert_eq!(data.error.code, None);
        assert_eq!(
            serde_json::to_string(&data).unwrap(),
            r#"{"error":{"message":"m"}}"#
        );
    }

    #[test]
    fn param_accepte_nimporte_quelle_valeur() {
        // `z.any()` n'applique aucune contrainte, pas plus un `Value` en Rust.
        for valeur in [
            json!("texte"),
            json!(42),
            json!(true),
            json!({"clef": "valeur"}),
            json!([1, 2]),
        ] {
            let charge = json!({ "error": { "message": "m", "param": valeur.clone() } });
            let data = parse_openai_compatible_error_data(&charge).unwrap();
            assert_eq!(data.error.param, Some(valeur));
        }
    }

    #[test]
    fn un_null_lu_disparait_du_json_et_un_null_construit_y_reste() {
        // Divergence documentee et volontaire : `Option` ne separe pas la cle
        // absente de `null`.
        let relu = analyser(r#"{"error":{"message":"m","param":null}}"#).unwrap();
        assert_eq!(relu.error.param, None);
        assert_eq!(
            serde_json::to_string(&relu).unwrap(),
            r#"{"error":{"message":"m"}}"#
        );
        // Construit a la main, le `null` se serialise bien.
        let construit = OpenAiCompatibleErrorData {
            error: OpenAiCompatibleErrorBody {
                message: "m".to_string(),
                error_type: None,
                param: Some(Value::Null),
                code: None,
            },
        };
        assert_eq!(
            serde_json::to_string(&construit).unwrap(),
            r#"{"error":{"message":"m","param":null}}"#
        );
    }

    #[test]
    fn les_cles_inconnues_sont_retirees_et_non_refusees() {
        // `z.object` retire les cles inconnues, il ne les refuse pas : seul
        // `z.strictObject` refuserait.
        let data = analyser(r#"{"error":{"message":"m","provider":"autre"},"inconnu":1}"#).unwrap();
        assert_eq!(
            serde_json::to_string(&data).unwrap(),
            r#"{"error":{"message":"m"}}"#
        );
    }

    #[test]
    fn une_chaine_vide_ne_devient_pas_absente() {
        // Piege de veracite : "" est falsy en JavaScript, mais `z.string()`
        // l'accepte et `z.union([z.string(), z.number()])` l'accepte aussi.
        // Un portage qui filtrerait les chaines vides les perdrait.
        let data = analyser(r#"{"error":{"message":"","type":"","code":""}}"#).unwrap();
        assert_eq!(data.error.message, "");
        assert_eq!(data.error.error_type.as_deref(), Some(""));
        assert_eq!(
            data.error.code,
            Some(ProviderErrorCode::CodeTexte(String::new()))
        );
        assert_eq!(
            serde_json::to_string(&data).unwrap(),
            r#"{"error":{"message":"","type":"","code":""}}"#
        );
    }

    #[test]
    fn le_message_est_rendut_tel_quel_sans_repli() {
        let structure = default_openai_compatible_error_structure();
        // Une chaine vide rend une chaine vide, pas un texte de remplacement.
        let vide = analyser(r#"{"error":{"message":""}}"#).unwrap();
        assert_eq!((structure.error_to_message)(&vide), "");
        // Ni taille, ni nettoyage, ni normalisation.
        let brute = analyser(r#"{"error":{"message":"  contexte  "}}"#).unwrap();
        assert_eq!((structure.error_to_message)(&brute), "  contexte  ");
        let long = analyser(r#"{"error":{"message":"a\nb"}}"#).unwrap();
        assert_eq!((structure.error_to_message)(&long), "a\nb");
    }

    #[test]
    fn la_structure_par_defaut_ne_declenche_pas_de_reprise() {
        let structure = default_openai_compatible_error_structure();
        // L'objet d'origine n'a pas de cle `isRetryable` : c'est une absence,
        // pas `false`.
        assert!(structure.is_retryable.is_none());
        let data = analyser(r#"{"error":{"message":"m"}}"#).unwrap();
        assert_eq!((structure.error_to_message)(&data), "m");
    }

    #[test]
    fn une_structure_personnalisee_peut_fournir_un_test_de_reprise() {
        fn toujours_reprendre(
            _response: &ProviderResponse,
            error: Option<&OpenAiCompatibleErrorData>,
        ) -> bool {
            error.is_some()
        }
        fn message_fixe(_data: &OpenAiCompatibleErrorData) -> String {
            "toujours la meme".to_string()
        }
        let structure = ProviderErrorStructure {
            error_schema: parse_openai_compatible_error_data,
            error_to_message: message_fixe,
            is_retryable: Some(toujours_reprendre),
        };
        let data = analyser(r#"{"error":{"message":"m"}}"#).unwrap();
        assert_eq!((structure.error_to_message)(&data), "toujours la meme");
        let reprise = structure.is_retryable.unwrap();
        assert!(reprise(&ProviderResponse { status: 429 }, Some(&data)));
        assert!(reprise(&ProviderResponse { status: 500 }, Some(&data)));
        assert!(!reprise(&ProviderResponse { status: 429 }, None));
    }

    #[test]
    fn la_fonction_de_message_est_bien_celle_de_la_structure_par_defaut() {
        let data = analyser(r#"{"error":{"message":"boom"}}"#).unwrap();
        assert_eq!(openai_compatible_error_to_message(&data), "boom");
    }

    #[test]
    fn la_lecture_directe_et_le_schema_donnent_le_meme_verdict() {
        // Le schema est ecrit a la main, les derives de Serde font le meme
        // travail. Ce test garantit que les deux chemins ne divergent pas.
        for texte in [
            r#"{"error":{"message":"m"}}"#,
            r#"{"error":{"message":"m","type":"t","param":{"a":1},"code":"c"}}"#,
            r#"{"error":{"message":"m","code":7}}"#,
            r#"{"error":{"message":"m","type":null,"code":null}}"#,
            r#"{"error":{"message":""}}"#,
            r#"{"error":{}}"#,
            r#"{"error":{"message":null}}"#,
            r#"{"error":{"message":"m","code":true}}"#,
            r#"{"error":{"message":"m","type":3}}"#,
            r#"{}"#,
            r#"{"error":[]}"#,
            r#"[1,2]"#,
        ] {
            let valeur: Value = serde_json::from_str(texte).unwrap();
            let direct = OpenAiCompatibleErrorData::deserialize(&valeur);
            let par_schema = parse_openai_compatible_error_data(&valeur);
            assert_eq!(
                direct.is_ok(),
                par_schema.is_ok(),
                "verdict divergent sur {}",
                texte
            );
            if let (Ok(a), Ok(b)) = (direct, par_schema) {
                assert_eq!(a, b, "valeurs divergentes sur {}", texte);
            }
        }
    }
}