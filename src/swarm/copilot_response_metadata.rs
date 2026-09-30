//! Portage de `github-copilot/chat/get-response-metadata.ts`.
//!
//! ## Ce que dit la source
//!
//! Quinze lignes, une seule fonction, aucun effet de bord, aucune validation.
//! C'est un **renommage de trois champs optionnels** :
//!
//! ```text
//! entree : { id?: string|null, created?: number|null, model?: string|null }
//! sortie : { id: string|undefined, modelId: string|undefined,
//!           timestamp: Date|undefined }
//! ```
//!
//! Deux des trois cles sont **renommees** : `model` devient `modelId`, et
//! `created` (en secondes) devient `timestamp` (un `Date`). `id` garde son nom.
//! C'est tout le contenu du fichier, et c'est pourquoi les tests de ce
//! fichier portent sur les **noms de cle** et non sur un calcul.
//!
//! ## Le piege des noms de champs : il est reel, mais il ne porte pas sur `id`
//!
//! Le piege `projectID` contre `projectId` **ne se pose pas** dans ce fichier :
//! aucune des six cles ne porte de sigle. Le piege equivalent, et plus
//! vicieux parce qu'il joue en sens inverse, est `modelId` :
//!
//! | cle TS | style      | champ Rust     | surcharge        |
//! |---|---|---|---|
//! | `id`        | mot seul   | `id`          | `id`            |
//! | `created`   | mot seul   | `created`     | `created`       |
//! | `model`     | mot seul   | `model`       | `model`         |
//! | `modelId`   | camelCase  | `model_id`    | `modelId`       |
//! | `timestamp` | mot seul   | `timestamp`   | `timestamp`     |
//!
//! Le reste du portage de ce depot ecrit les sigles en **majuscules**
//! (`sessionID`, `messageID`, `providerID`, voir
//! `src/core/session_event.rs`). Un agent qui transpose cette reflexe poserait
//! ici `modelID` : la faute ne se voit pas a la compilation, le modele de langue
//! ne lit jamais le champ, et l'echange avec le TypeScript est rompu sans
//! qu'aucun test du code Rust ne rougisse. D'ou les deux verrous de ce fichier :
//!
//! 1. un test qui **serialise** et compare la chaine JSON complete ;
//! 2. un test qui **refuse** la forme `model_id` a la lecture, parce que
//!    `serde` ignore silencieusement une cle inconnue et se contente de rendre
//!    `None` : sans parseur explicite, la faute serait invisible a l'aller
//!    comme au retour.
//!
//! Aucun `rename_all` n'est pose : il donnerait `model_id` a la seule cle
//! camelCase du fichier, ce qui est exactement la faute a eviter.
//!
//! ## Le piege de la veracite : il mord, et sur deux valeurs
//!
//! La source melee les deux operateurs, et les deux ont des valeurs qui
//! disparaissent si on les confond :
//!
//! - `id ?? undefined` et `model ?? undefined` sont des **coalescents**. Ils
//!   testent la **nullite**. `id: ""` est une chaine : elle **survit**.
//! - `created != null ? new Date(created * 1000) : undefined` est un **test de
//!   nullite** ecrit sous forme de ternaire, pas un test de veracite. Il
//!   compare `created` a `null` et rien d'autre. `created: 0` est un nombre :
//!   il **survit** aussi, et donne la date epoch. Un portage par veracite
//!   (un `?:` transcrit en `.filter(|c| *c != 0.0)`) perdrait les deux.
//!
//! C'est la ou ce fichier se fait mordre : une fonction de **metadonnees de
//! reponse** est par nature un paquet de champs optionnels, et la chaine vide
//! comme le zero y sont des valeurs recues de l'API, pas des cas de bord.
//! Chaque operateur a donc sa fonction, nommee d'apres lui :
//! `coalescent_nullish` et `ternaire_nullish`. Les variantes
//! `*_si_veracite` sont la **contrepartie de comparaison** : elles ne sont
//! appelees par aucun code de production, elles existent pour que la
//! difference soit visible et testee plutot que decrite en commentaire.
//!
//! ## Ce qui change a la restitution
//!
//! - `undefined` disparait du JSON. La source renvoie des cles
//!   **presentes** avec la valeur `undefined` (l'objet est ecrit en toutes
//!   lettres), que `JSON.stringify` supprime ensuite. Le portage omet les
//!   cles absentes : `response.id` et `"id" in response` se comportent comme
//!   en JavaScript, `Object.keys(response)` non. Aucun code de la source
//!   n'enumerate les cles de ce resultat, il le decompose (`...`).
//! - `null` se lit comme une absence. L'entree accepte `string | undefined |
//!   null`, donc `null` est une valeur legitime, mais `?? undefined` la
//!   transforme deja en absence dans la source. `Option` fait la meme chose
//!   sans divergence observable.
//! - `timestamp` est un `i64` de millisecondes depuis l'epoch, pas un `Date`.
//!   Le crate n'a pas de type de date et `JSON.stringify` d'un `Date` donnerait
//!   une chaine ISO que la source ne produit jamais ici. La convention
//!   `DateTimeUtcFromMillis = i64` de `src/core/session_event.rs` est
//!   reprise. Une conversion vers le texte ISO sort du perimetre de ce
//!   fichier.
//! - `created` est un `f64` a l'entree : c'est `number` en TypeScript, donc un
//!   flottant, et `1.5` est une entree valide qui donne `1500` ms. La
//!   conversion `as i64` tronque vers zero, comme `ToIntegerOrInfinity` de
//!   `Date`. Seul cas non couvert : un `NaN` construit a la main donnerait `0`
//!   la ou JavaScript produit une `Invalid Date`, mais JSON ne peut pas
//!   contenir `NaN` et le parseur de ce fichier refuse tout de meme une
//!   valeur non numerique.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// L'entree de `getResponseMetadata`, c'est-a-dire le type litteral ecrit en
/// place dans la signature.
///
/// Les trois champs sont optionnels **et** nullables, dans cet ordre-la :
/// `id`, `created`, `model`. Un objet vide est valide et donne trois
/// metadonnees absentes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CopilotResponseInput {
    /// `id?: string | undefined | null`. Coalescent nul : la chaine vide
    /// survit, elle n'est pas une absence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// `created?: number | undefined | null`, en **secondes**. Test de nullite
    /// dans la source : `0` est un nombre, il survait et donne l'epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<f64>,

    /// `model?: string | undefined | null`. Coalescent nul comme `id`, et
    /// renomme en `modelId` a la sortie.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// Le `Date` de la sortie, en millisecondes depuis l'epoch.
///
/// Type interne du portage : la source produit un `Date`, le crate n'a pas de
/// type de date. `i64` millisecondes est la convention deja posee par
/// `src/core/session_event.rs` pour `DateTimeUtcFromMillis`.
pub type TimestampMs = i64;

/// Le resultat de `getResponseMetadata`, celui qui est decompose par `...`
/// dans `response` et dans la partie `response-metadata` du flux.
///
/// Les trois noms de cle sont contractuels. `model_id` porte donc une
/// surcharge explicite, et `timestamp` aussi, pour que les trois se lisent
/// d'un coup d'oeil sans avoir a deduire quoi que ce soit du nom du champ.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CopilotResponseMetadata {
    /// `id`, sans renommage.
    #[serde(rename = "id", skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Sort de `model`. La surcharge `modelId` est le point critique du
    /// fichier : sans elle le champ partirait en `model_id`, et `modelID` ne
    /// serait pas plus correct, le reste du depot ecrit les sigles en
    /// majuscules mais la source ecrit ici `Id`.
    #[serde(rename = "modelId", skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,

    /// Sort de `created` converti : secondes fois mille, ou rien du tout.
    #[serde(rename = "timestamp", skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<TimestampMs>,
}

/// Les trois cles acceptees a la lecture, dans l'ordre de la source.
pub const CLES_ENTREE: [&str; 3] = ["id", "created", "model"];

/// Pourquoi une charge utile n'est pas une `CopilotResponseInput`.
///
/// La source ne valide rien : c'est un objet anonyme, destructure par
/// l'appelant. Ces erreurs servent donc au **controle de portage**, pas a la
/// validation d'une reponse de production.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopilotResponseInputError {
    /// La racine de la charge utile n'est pas un objet JSON.
    RacineNonObjet,

    /// `id` est present, non nul, et n'est pas une chaine.
    IdNonChaine,

    /// `created` est present, non nul, et n'est pas un nombre.
    CreatedNonNombre,

    /// `model` est present, non nul, et n'est pas une chaine.
    ModelNonChaine,

    /// Une cle se rapproche d'une des trois cles attendues sans lui etre
    /// exactement : `model_id`, `modelID`, `modelid`...
    ///
    /// Cette variante n'a **pas** d'equivalent en TypeScript, ou le
    /// destructureur ignore toute cle non listee. Elle est la que se loge la
    /// faute de frappe la plus dangereuse du fichier, celle qui ne casse ni la
    /// compilation ni aucun test.
    CleMalEcrite {
        /// La cle recue, telle qu'elle est ecrite dans le JSON.
        cle: String,
        /// La cle attendue, orthographe exacte de la source.
        attendue: &'static str,
    },
}

impl CopilotResponseInputError {
    /// Le chemin du champ fautif. La racine n'a pas de champ, donc une chaine
    /// vide, comme dans le reste du portage.
    pub fn path(&self) -> &'static str {
        match self {
            CopilotResponseInputError::RacineNonObjet => "",
            CopilotResponseInputError::IdNonChaine => "id",
            CopilotResponseInputError::CreatedNonNombre => "created",
            CopilotResponseInputError::ModelNonChaine => "model",
            CopilotResponseInputError::CleMalEcrite { attendue, .. } => *attendue,
        }
    }
}

/// `valeur ?? undefined`, c'est-a-dire le **coalescent nul**.
///
/// Ne compare pas a `""` et ne compare pas a `0` : la chaine vide est
/// falsy en JavaScript et doit malgre tout survivre, puisque `??` teste la
/// nullite.
pub fn coalescent_nullish(valeur: Option<&str>) -> Option<String> {
    valeur.map(String::from)
}

/// Contrepartie de comparaison de `coalescent_nullish`, **jamais appelee** par
/// `get_response_metadata`.
///
/// C'est ce que donnerait un portage par test de veracite : `""` disparait.
/// Sert uniquement a verrouiller la difference entre les deux operateurs.
pub fn coalescent_si_veracite(valeur: Option<&str>) -> Option<String> {
    match valeur {
        Some(v) if !v.is_empty() => Some(String::from(v)),
        _ => None,
    }
}

/// `created != null ? new Date(created * 1000) : undefined`.
///
/// C'est un test de **nullite** ecrit en ternaire, pas un test de veracite :
/// `0` produit donc l'epoch et non une absence. La conversion tronque vers
/// zero, comme `ToIntegerOrInfinity` de `Date`.
pub fn ternaire_nullish(created: Option<f64>) -> Option<TimestampMs> {
    created.map(|secondes| (secondes * 1000.0) as TimestampMs)
}

/// Contrepartie de comparaison de `ternaire_nullish`, **jamais appelee** par
/// `get_response_metadata`.
///
/// C'est ce que donnerait un portage par test de veracite : `0` disparait,
/// alors que la source garde la date epoch.
pub fn ternaire_si_veracite(created: Option<f64>) -> Option<TimestampMs> {
    match created {
        Some(secondes) if secondes != 0.0 => Some((secondes * 1000.0) as TimestampMs),
        _ => None,
    }
}

/// `getResponseMetadata({ id, model, created })`.
///
/// Les trois champs passent par leur operateur d'origine : `id` et `model`
/// par le coalescent nul, `created` par le ternaire de nullite. Aucun champ ne
/// est normalise, ni taille, ni complete, ni remplace par une valeur de
/// repli.
pub fn get_response_metadata(entree: &CopilotResponseInput) -> CopilotResponseMetadata {
    CopilotResponseMetadata {
        id: coalescent_nullish(entree.id.as_deref()),
        model_id: coalescent_nullish(entree.model.as_deref()),
        timestamp: ternaire_nullish(entree.created),
    }
}

/// Minuscules, sans tiret bas ni tiret, pour comparer deux orthographes.
///
/// C'est ce calcul qui fait tomber `model_id` et `modelID` sur `modelId`, et
/// qui reconnait le suffixe `Id` que le reste du depot ecrit `ID`.
fn cle_normalisee(cle: &str) -> String {
    cle.chars()
        .filter(|c| *c != '_' && *c != '-')
        .flat_map(char::to_lowercase)
        .collect()
}

/// La cle attendue qui ressemble a `cle` sans lui etre exactement.
///
/// Deux ressemblances sont reconnues : la casse et le tiret bas
/// (`Model_id` -> `model`), et le suffixe du sigle (`modelID` -> `modelId`),
/// qui est le piege propre a ce fichier.
fn cle_attendue_si_rapprochee(cle: &str) -> Option<&'static str> {
    let normalisee = cle_normalisee(cle);
    // Iteration par valeur sur le const : chaque element est &'static str,
    // donc le retour porte la bonne duree sans emprunt temporaire.
    for attendue in CLES_ENTREE {
        let attendue_normalisee = cle_normalisee(attendue);
        if normalisee == attendue_normalisee
            || normalisee == format!("{}id", attendue_normalisee)
        {
            return Some(attendue);
        }
    }
    None
}

/// Lit une charge utile JSON comme la source lit son objet d'entree.
///
/// Divergence assumee et la seule de ce parseur : la source ignore toute
/// cle qu'elle ne liste pas, y compris une faute de frappe, et n'en fait ni
/// une erreur ni un champ. Ici, une cle qui **ressemble** a l'une des trois
/// attendues est refusee, parce que c'est un defaut de portage et pas un
/// evenement d'execution. Une cle etrangere sans rapport est toujours
/// ignoree, comme en TypeScript.
pub fn parse_copilot_response_input(
    valeur: &Value,
) -> Result<CopilotResponseInput, CopilotResponseInputError> {
    let racine = match valeur.as_object() {
        Some(objet) => objet,
        None => return Err(CopilotResponseInputError::RacineNonObjet),
    };

    let mut entree = CopilotResponseInput::default();
    for (cle, valeur) in racine {
        match cle.as_str() {
            "id" => {
                entree.id = match valeur {
                    Value::String(id) => Some(id.clone()),
                    Value::Null => None,
                    _ => return Err(CopilotResponseInputError::IdNonChaine),
                }
            }
            "created" => {
                entree.created = match valeur {
                    Value::Number(secondes) => Some(secondes.as_f64().ok_or(
                        CopilotResponseInputError::CreatedNonNombre,
                    )?),
                    Value::Null => None,
                    _ => return Err(CopilotResponseInputError::CreatedNonNombre),
                }
            }
            "model" => {
                entree.model = match valeur {
                    Value::String(model) => Some(model.clone()),
                    Value::Null => None,
                    _ => return Err(CopilotResponseInputError::ModelNonChaine),
                }
            }
            _ => {
                if let Some(attendue) = cle_attendue_si_rapprochee(cle) {
                    return Err(CopilotResponseInputError::CleMalEcrite {
                        cle: cle.clone(),
                        attendue,
                    });
                }
                // Cle etrangere : ignoree, comme le destructureur de la source.
            }
        }
    }
    Ok(entree)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Un objet vide est valide : les trois champs sont optionnels, et le
    /// resultat ne contient aucune cle.
    #[test]
    fn un_entree_vide_donne_trois_metadonnees_absentes() {
        let vide = CopilotResponseInput::default();
        let metadata = get_response_metadata(&vide);
        assert_eq!(metadata, CopilotResponseMetadata::default());
        assert_eq!(metadata.id, None);
        assert_eq!(metadata.model_id, None);
        assert_eq!(metadata.timestamp, None);
        assert_eq!(serde_json::to_string(&metadata).unwrap(), "{}");
        assert_eq!(parse_copilot_response_input(&json!({})).unwrap(), vide);
    }

    /// Le verrou principal : les trois cles sortent avec l'orthographe exacte
    /// de la source, dans l'ordre de l'objet litteral d'origine.
    #[test]
    fn les_noms_json_de_sortie_sont_exactement_ceux_de_la_source() {
        let metadata = get_response_metadata(&CopilotResponseInput {
            id: Some("chatcmpl-1".to_string()),
            created: Some(1_700_000_000.0),
            model: Some("gpt-4o".to_string()),
        });

        assert_eq!(
            serde_json::to_string(&metadata).unwrap(),
            r#"{"id":"chatcmpl-1","modelId":"gpt-4o","timestamp":1700000000000}"#
        );
    }

    /// La forme snake_case et la forme `modelID` ne doivent exister ni en
    /// sortie ni dans le JSON relu. Le reste du depot ecrit les sigles en
    /// majuscules (`sessionID`, `providerID`) : c'est cette reflexe qui ferait
    /// ecrire `modelID` ici, alors que la source ecrit `modelId`.
    #[test]
    fn ni_snake_case_ni_majuscule_ne_peuvent_pas_sortir() {
        let metadata = get_response_metadata(&CopilotResponseInput {
            id: Some("x".to_string()),
            created: Some(0.0),
            model: Some("m".to_string()),
        });
        let sortie = serde_json::to_string(&metadata).unwrap();

        assert!(sortie.contains(r#""modelId":"m""#));
        for forme_interdite in [
            "\"model_id\":",
            "\"modelID\":",
            "\"modelid\":",
            "\"created\":",
            "\"model\":",
        ] {
            assert!(
                !sortie.contains(forme_interdite),
                "{} ne doit pas apparaitre dans {}",
                forme_interdite,
                sortie
            );
        }

        // Le relu, lui, ne doit reconnaitre que l'orthographe exacte.
        let relu: CopilotResponseMetadata =
            serde_json::from_str(r#"{"id":"x","modelId":"m","timestamp":0}"#).unwrap();
        assert_eq!(relu.model_id.as_deref(), Some("m"));
        assert_eq!(relu.timestamp, Some(0));
    }

    /// Le piege `model_id` **a l'entree** du systeme, c'est-a-dire la ou la
    /// faute se glisse reellement : le JSON de la reponse Copilot tel que le
    /// modele de langue le transmet. `serde` seul ignorerait la cle et
    /// rendrait `None` sans rien dire, donc le parseur explicite la refuse.
    #[test]
    fn la_forme_snake_case_est_refusee_a_la_lecture() {
        for (texte, attendue) in [
            (r#"{"id":"x","model_id":"m"}"#, "model"),
            (r#"{"id":"x","modelID":"m"}"#, "model"),
            (r#"{"id":"x","modelid":"m"}"#, "model"),
            (r#"{"id":"x","Model":"m"}"#, "model"),
            (r#"{"id":"x","model-id":"m"}"#, "model"),
        ] {
            let charge: Value = serde_json::from_str(texte).unwrap();
            let erreur = parse_copilot_response_input(&charge)
                .expect_err("une forme rapprochee doit etre refusee");
            assert_eq!(erreur.path(), attendue);
            match erreur {
                CopilotResponseInputError::CleMalEcrite { cle, attendue: a } => {
                    assert_eq!(a, attendue);
                    assert!(!cle.is_empty());
                }
                autre => panic!("forme acceptee a tort : {:?}", autre),
            }
        }
    }

    /// Une cle etrangere qui ne ressemble a rien est ignoree, comme le
    /// destructureur de la source, qui ne garde que les trois champs qu'il
    /// liste.
    #[test]
    fn une_cle_etrangere_est_ignoree() {
        let entree = parse_copilot_response_input(&json!({
            "object": "chat.completion",
            "choices": [],
            "model": "gpt-4o",
            "usage": { "total_tokens": 12 },
        }))
        .expect("une cle inconnue ne doit pas faire echouer la lecture");

        assert_eq!(entree.model.as_deref(), Some("gpt-4o"));
        assert_eq!(entree.id, None);
        assert_eq!(entree.created, None);
    }

    /// Coalescent nul : la chaine vide est une chaine, elle survit. C'est le
    /// piege de veracite, et le plus evident des deux parce qu'il porte sur
    /// une chaine.
    #[test]
    fn une_chaine_vide_survit_le_coalescent() {
        let metadata = get_response_metadata(&CopilotResponseInput {
            id: Some(String::new()),
            model: Some(String::new()),
            created: None,
        });

        assert_eq!(metadata.id.as_deref(), Some(""));
        assert_eq!(metadata.model_id.as_deref(), Some(""));
        assert_ne!(metadata.id, None);
        assert_ne!(metadata.model_id, None);
        assert_eq!(
            serde_json::to_string(&metadata).unwrap(),
            r#"{"id":"","modelId":""}"#
        );
    }

    /// Les deux coalescents sont deux fonctions et non une seule : c'est la
    /// difference entre `??` et un test de veracite, et elle se voit sur la
    /// seule entree ou ils divergent, la chaine vide.
    #[test]
    fn les_deux_coalescents_ne_sont_pas_un_seul() {
        assert_eq!(coalescent_nullish(Some("")), Some(String::new()));
        assert_eq!(coalescent_nullish(Some("m")), Some("m".to_string()));
        assert_eq!(coalescent_nullish(None), None);

        assert_eq!(coalescent_si_veracite(Some("")), None);
        assert_eq!(coalescent_si_veracite(Some("m")), Some("m".to_string()));
        assert_eq!(coalescent_si_veracite(None), None);

        // Et la source utilise bien le premier.
        let metadata = get_response_metadata(&CopilotResponseInput {
            id: Some(String::new()),
            ..Default::default()
        });
        assert_eq!(metadata.id, coalescent_nullish(Some("")));
    }

    /// Le meme piege, sur le nombre : `created: 0` donne la date epoch et non
    /// une absence. `0` est falsy en JavaScript, donc c'est la valeur qu'un
    /// portage par veracite perdrait en silence.
    #[test]
    fn un_created_a_zero_donne_l_epoch_et_non_l_absence() {
        let metadata = get_response_metadata(&CopilotResponseInput {
            created: Some(0.0),
            ..Default::default()
        });

        assert_eq!(metadata.timestamp, Some(0));
        assert_ne!(metadata.timestamp, None);
        assert_eq!(serde_json::to_string(&metadata).unwrap(), r#"{"timestamp":0}"#);
    }

    /// Les deux ternaires sont deux fonctions et non une seule : `0` les
    /// separe, `None` et une vraie date les rejoignent.
    #[test]
    fn les_deux_ternaires_ne_sont_pas_un_seul() {
        assert_eq!(ternaire_nullish(Some(0.0)), Some(0));
        assert_eq!(ternaire_nullish(Some(1_700_000_000.0)), Some(1_700_000_000_000));
        assert_eq!(ternaire_nullish(None), None);

        assert_eq!(ternaire_si_veracite(Some(0.0)), None);
        assert_eq!(
            ternaire_si_veracite(Some(1_700_000_000.0)),
            Some(1_700_000_000_000)
        );
        assert_eq!(ternaire_si_veracite(None), None);

        // Et la source utilise bien le premier.
        let metadata = get_response_metadata(&CopilotResponseInput {
            created: Some(0.0),
            ..Default::default()
        });
        assert_eq!(metadata.timestamp, ternaire_nullish(Some(0.0)));
    }

    /// Un `created` non nul devient un `Date` en millisecondes. Le nom change,
    /// l'unite aussi : `created` est en secondes, `timestamp` en millisecondes.
    #[test]
    fn un_created_non_nul_devient_un_timestamp_en_millisecondes() {
        let metadata = get_response_metadata(&CopilotResponseInput {
            created: Some(1_700_000_000.0),
            ..Default::default()
        });
        assert_eq!(metadata.timestamp, Some(1_700_000_000_000));

        // `created` est un `number`, donc un flottant : la conversion tronque
        // vers zero, comme `ToIntegerOrInfinity` de `Date`.
        assert_eq!(ternaire_nullish(Some(1.5)), Some(1_500));
        assert_eq!(ternaire_nullish(Some(-1.5)), Some(-1_500));
        assert_eq!(ternaire_nullish(Some(0.0004)), Some(0));
    }

    /// `created` absent ou `null` : pas de `timestamp`. Le ternaire teste la
    /// nullite, donc rien d'autre ne peut donner cette absence.
    #[test]
    fn un_created_absent_ou_null_donne_un_timestamp_absent() {
        let sans = get_response_metadata(&CopilotResponseInput::default());
        assert_eq!(sans.timestamp, None);

        let nul = parse_copilot_response_input(&json!({ "created": null })).unwrap();
        assert_eq!(nul.created, None);
        assert_eq!(get_response_metadata(&nul).timestamp, None);
    }

    /// `null` explicite vaut absence, pour les trois champs : c'est
    /// exactement ce que fait `?? undefined` dans la source.
    #[test]
    fn un_null_explicite_vaut_une_absence() {
        let entree = parse_copilot_response_input(
            &json!({ "id": null, "created": null, "model": null }),
        )
        .expect("null est accepte par l'entree de la source");

        assert_eq!(entree, CopilotResponseInput::default());
        assert_eq!(
            serde_json::to_string(&get_response_metadata(&entree)).unwrap(),
            "{}"
        );
    }

    /// Une valeur d'un mauvais type est refusee. Le type de la source est
    /// compile, il n'y a donc pas d'equivalent exact : le parseur se contente
    /// de ne pas inventer de conversion la ou le TypeScript n'en fait pas.
    #[test]
    fn une_valeur_de_mauvais_type_est_refusee() {
        for (texte, attendue) in [
            (r#"{"id":42}"#, CopilotResponseInputError::IdNonChaine),
            (
                r#"{"model":{"a":1}}"#,
                CopilotResponseInputError::ModelNonChaine,
            ),
            (
                r#"{"created":"1700000000"}"#,
                CopilotResponseInputError::CreatedNonNombre,
            ),
            (
                r#"{"created":true}"#,
                CopilotResponseInputError::CreatedNonNombre,
            ),
        ] {
            let charge: Value = serde_json::from_str(texte).unwrap();
            let erreur = parse_copilot_response_input(&charge)
                .expect_err("un type incorrect doit etre refuse");
            assert_eq!(erreur, attendue);
        }

        assert_eq!(CopilotResponseInputError::RacineNonObjet.path(), "");
        assert!(parse_copilot_response_input(&json!([1, 2])).is_err());
        assert!(parse_copilot_response_input(&json!("nope")).is_err());
    }

    /// La lecture par derives de Serde accepte la forme de la source, et
    /// l'ignore silencieusement pour une forme fausse. C'est exactement
    /// l'ecart que `parse_copilot_response_input` comble, et ce test le
    /// documente au lieu de le cacher.
    #[test]
    fn la_lecture_serde_accepte_la_source_et_ignore_une_forme_fausse() {
        let relu: CopilotResponseInput =
            serde_json::from_str(r#"{"id":"x","model":"gpt-4o","created":1700000000}"#)
                .expect("la forme de la source doit etre valide");
        assert_eq!(relu.id.as_deref(), Some("x"));
        assert_eq!(relu.model.as_deref(), Some("gpt-4o"));
        assert_eq!(relu.created, Some(1_700_000_000.0));

        // Forme fausse : Serde ne dit rien, le parseur refuse.
        let faux: CopilotResponseInput =
            serde_json::from_str(r#"{"model_id":"gpt-4o"}"#).unwrap();
        assert_eq!(faux.model, None);
        let charge: Value = serde_json::from_str(r#"{"model_id":"gpt-4o"}"#).unwrap();
        assert!(parse_copilot_response_input(&charge).is_err());
    }

    /// Aller-retour : l'entree complete, la sortie, la relecture par le
    /// parseur, et le meme resultat des deux cotes.
    #[test]
    fn un_all_et_ret_complet_ne_change_rien() {
        let charge = json!({ "id": "chatcmpl-1", "created": 1_700_000_000, "model": "gpt-4o" });
        let entree = parse_copilot_response_input(&charge).unwrap();
        let metadata = get_response_metadata(&entree);

        assert_eq!(
            serde_json::to_value(&metadata).unwrap(),
            json!({ "id": "chatcmpl-1", "modelId": "gpt-4o", "timestamp": 1_700_000_000_000i64 })
        );

        // Les metadonnees ne se relisent pas comme une entree : la source ne
        // le fait pas non plus, elle decompose le resultat dans `response`.
        let relues: CopilotResponseMetadata = serde_json::from_value(
            json!({ "id": "chatcmpl-1", "modelId": "gpt-4o", "timestamp": 1_700_000_000_000i64 }),
        )
        .unwrap();
        assert_eq!(relues, metadata);
    }

    /// Une seule metadonnee presente : les deux autres cles n'apparaissent
    /// pas dans le JSON au lieu de sortir a `null`.
    #[test]
    fn une_seule_metadonnee_presente_ne_produit_qu_une_cle() {
        let metadata = get_response_metadata(&CopilotResponseInput {
            model: Some("gpt-4o".to_string()),
            ..Default::default()
        });
        assert_eq!(serde_json::to_string(&metadata).unwrap(), r#"{"modelId":"gpt-4o"}"#);
    }

    /// La liste des cles lues reste synchrone avec le parseur, et la
    /// normalisation repere bien les deux pieges de casse du fichier.
    ///
    /// Une cle exacte se reconnait aussi : le parseur ne consulte cette
    /// fonction que pour une cle qu'il ne connait pas, donc le cas n'est
    /// jamais atteint en production, mais il doit rester coherent.
    #[test]
    fn la_liste_des_cles_et_la_detection_de_casse_sont_synchronisees() {
        for cle in CLES_ENTREE {
            assert_eq!(cle_attendue_si_rapprochee(cle), Some(cle));
        }
        assert_eq!(
            cle_attendue_si_rapprochee("model_id"),
            Some("model")
        );
        assert_eq!(
            cle_attendue_si_rapprochee("modelID"),
            Some("model")
        );
        assert_eq!(cle_attendue_si_rapprochee("modelId"), Some("model"));
        assert_eq!(cle_attendue_si_rapprochee("id"), Some("id"));
        assert_eq!(cle_attendue_si_rapprochee("providerMetadata"), None);
        assert_eq!(cle_attendue_si_rapprochee("choices"), None);
        assert_eq!(cle_normalisee("Model_ID"), "modelid");
    }

    /// Le `NaN` est le seul cas que le portage ne peut pas rendre comme
    /// JavaScript : il donnerait ici l'epoch la ou la source produit une
    /// `Invalid Date`. JSON ne peut pas contenir `NaN` et le parseur refuse
    /// toute valeur non numerique, donc le cas n'est atteignable que par un
    /// `CopilotResponseInput` construit a la main.
    #[test]
    fn un_nan_construit_a_la_main_ne_panique_pas() {
        let manuel = get_response_metadata(&CopilotResponseInput {
            created: Some(f64::NAN),
            ..Default::default()
        });
        assert_eq!(manuel.timestamp, Some(0));
    }
}
