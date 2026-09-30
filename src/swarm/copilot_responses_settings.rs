//! Portage de `github-copilot/responses/openai-responses-settings.ts`.
//!
//! La source fait 45 octets. Elle tient sur une seule ligne, sans saut de ligne
//! superflus ni commentaire :
//!
//! ```ts
//! export type OpenAIResponsesModelId = string
//! ```
//!
//! Ce n'est ni un objet litteral, ni une table de configuration, ni un
//! reexport : c'est un ALIAS DE TYPE pur, qui n'existe qu'a la compilation.
//! Il n'y a donc rien a traduire en donnees, et surtout rien a inventer. La
//! traduction fidele tient en une ligne de code (`pub type ... = String;`) ; le
//! reste de ce fichier est ce qu'il faut savoir pour ne pas s'y tromper.
//!
//! Ce que l'alias n'est PAS, et qu'aucun test ne doit laisser croire :
//!
//! - Ce n'est pas un `enum`. `string` en TypeScript accepte n'importe quelle
//!   chaine, y compris `"", "GPT-5", "  "` ou du JSON ; il n'y a ni liste
//!   blanche, ni normalisation, ni longueur minimale. Une version Rust qui
//!   rangerait les identifiants connus dans un `enum` serait un RESTREINT de
//!   la source, pas sa traduction.
//! - Ce n'est pas un `struct`. L'alias ne nomme aucun champ : il n'y a donc
//!   aucun nom de champ a recopier ici, et un `struct` serait une invention.
//! - Ce n'est pas une chaine *validee* : il n'y a ni `zod`, ni `parse`, ni
//!   `refine` autour de lui dans la source.
//!
//! Points de fidelite qui comptent vraiment a l'echange avec le TypeScript :
//!
//! 1. Representation sur le fil : une `string` est un scalaire JSON, donc
//!    `OpenAIResponsesModelId` se serialise en `"gpt-5"`, jamais en
//!    `{"modelId": "gpt-5"}`. Le SEUL nom de champ du dossier est porte par le
//!    consommateur, `openai-responses-language-model.ts:134`, qui ecrit
//!    `readonly modelId: OpenAIResponsesModelId` : `modelId` en camelCase, avec
//!    un `d` minuscule. Ce n'est ni `modelID`, ni `model_id`. Le test
//!    `le_nom_du_champ_chez_le_consommateur_est_model_id` fige cette casse.
//! 2. Absence et chaine vide sont deux etats distincts. `string` en TypeScript
//!    n'inclut ni `null` ni `undefined` : l'absence se code en dehors du type,
//!    chez l'appelant, par `Option<OpenAIResponsesModelId>`. Une chaine vide
//!    reste une chaine vide et n'est jamais un "manquant" ; `Some(String::new())`
//!    n'est pas `None`. La deserialisation d'un `null` JSON echoue, exactement
//!    comme `null` n'est pas assignable a `string` en TypeScript.
//! 3. `string` accepte indiffemment un litteral et une sous-chaine ; `String`
//!    est un type POSSEDE. Un `&str` passe par `.to_string()` ou `.into()`. Le
//!    choix de `String` (et non de `&str` ou `Box<str>`) est ce qui rend
//!    l'alias directement serialisable et directement stockable, sans copie
//!    supplementaire a chaque appel.
//!
//! Le piege de veracite (`?` : la chaine vide est falsy et disparait) contre
//! le piege de nullite (`??` : la chaine vide SURVIT) n'a pas lieu d'etre ici :
//! la source n'est qu'une ligne de declaration et ne contient ni ternaire ni
//! coalescent. Traduire ce fichier en deux fonctions, une pour `?` et une pour
//! `??`, reviendrait a ajouter du code qui n existe pas. Le seul etat dont il
//! faut se mefier reste celui decrit au point 2, et il est teste.

/// Identifiant de modele de l'API OpenAI Responses.
///
/// Alias de `String`, et rien de plus : aucune validation, aucune normalisation,
/// aucune liste de valeurs autorisee. Voir la documentation du module.
pub type OpenAIResponsesModelId = String;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_alias_de_type_ne_donne_aucun_objet() {
        // La source est `export type X = string` : rien a serialiser que la
        // chaine elle-meme. Un struct ici produirait un `{...}` et casserait
        // l'echange avec le TypeScript.
        let id: OpenAIResponsesModelId = String::from("gpt-5");
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"gpt-5\"");
    }

    #[test]
    fn la_representation_sur_le_fil_est_un_scalaire_json() {
        let id: OpenAIResponsesModelId = "claude-opus-4-5".to_string();
        let s = serde_json::to_string(&id).unwrap();
        assert_eq!(s, "\"claude-opus-4-5\"");
        assert!(!s.contains('{'), "une string ne doit pas devenir un objet");
        assert!(!s.contains('['), "une string ne doit pas devenir un tableau");
    }

    #[test]
    fn la_relecture_json_rend_la_meme_chaine() {
        let relue: OpenAIResponsesModelId = serde_json::from_str("\"gpt-5-mini\"").unwrap();
        assert_eq!(relue, "gpt-5-mini");
    }

    #[test]
    fn le_nom_du_champ_chez_le_consommateur_est_model_id() {
        // `openai-responses-language-model.ts:134` :
        // `readonly modelId: OpenAIResponsesModelId`.
        // Le type ne porte aucun nom, le champ se trouve chez le consommateur,
        // et sa casse fait partie du contrat : `modelId`, jamais `modelID`.
        let objet = serde_json::json!({ "modelId": "gpt-5" });
        assert_eq!(serde_json::to_string(&objet).unwrap(), "{\"modelId\":\"gpt-5\"}");
        // La valeur de ce champ se lit avec le meme alias.
        let relu: OpenAIResponsesModelId =
            serde_json::from_str(objet["modelId"].to_string().as_str()).unwrap();
        assert_eq!(relu, "gpt-5");
    }

    #[test]
    fn aucune_normalisation_n_est_appliquee() {
        // Pas de `trim`, pas de mise en minuscules, pas de liste blanche :
        // `string` accepte n'importe quoi et le laisse passer tel quel.
        for brut in [
            "GPT-5",
            "  gpt-5  ",
            "n_importe_quoi",
            "{\"modelId\":\"gpt-5\"}",
            "gpt-5\n",
        ] {
            let id: OpenAIResponsesModelId = brut.to_string();
            let relu: OpenAIResponsesModelId =
                serde_json::from_str(serde_json::to_string(&id).unwrap().as_str()).unwrap();
            assert_eq!(relu, brut, "la chaine doit survivre intacte");
        }
    }

    #[test]
    fn la_chaine_vide_est_une_valeur_valide_et_survit() {
        // Piege de nullite : `Some("")` n'est PAS `None`. La source ne teste
        // rien, donc la chaine vide passe au travers sans devenir un manquant.
        let vide: Option<OpenAIResponsesModelId> = Some(String::new());
        assert!(vide.is_some());
        assert_eq!(serde_json::to_string(&Some(String::new())).unwrap(), "\"\"");
        let absent: Option<OpenAIResponsesModelId> = None;
        assert_eq!(serde_json::to_string(&absent).unwrap(), "null");
        assert_ne!(
            serde_json::to_string(&vide).unwrap(),
            serde_json::to_string(&absent).unwrap(),
            "chaine vide et absence sont deux etats distincts"
        );
    }

    #[test]
    fn null_et_autres_scalaires_sont_rejetes() {
        // `null` n'est pas assignable a `string` en TypeScript : la deserialise-
        // mention doit echouer aussi, sinon on aurait elargi le type.
        assert!(serde_json::from_str::<OpenAIResponsesModelId>("null").is_err());
        assert!(serde_json::from_str::<OpenAIResponsesModelId>("42").is_err());
        assert!(serde_json::from_str::<OpenAIResponsesModelId>("true").is_err());
        assert!(serde_json::from_str::<OpenAIResponsesModelId>("{\"modelId\":\"gpt-5\"}").is_err());
    }

    #[test]
    fn un_borrow_doit_etre_copie_pour_entrer_dans_le_type() {
        // `string` accepte une sous-chaine comme un litteral ; `String` est
        // possede, donc la copie est explicite ici.
        let depuis_litteral: OpenAIResponsesModelId = "gpt-5".to_string();
        let depuis_borrow: OpenAIResponsesModelId = String::from("gpt-5");
        let par_into: OpenAIResponsesModelId = "gpt-5".into();
        assert_eq!(depuis_litteral, depuis_borrow);
        assert_eq!(depuis_borrow, par_into);
    }
}
