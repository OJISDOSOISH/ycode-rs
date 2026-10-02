//! Portage de `github-copilot/chat/openai-compatible-chat-options.ts`.
//!
//! ## Ce que dit la source
//!
//! Vingt-huit lignes, dont **une seule** declaration de donnee : un objet
//! `z.object` de quatre champs, tous optionnels. Aucun calcul, aucune fonction,
//! aucun effet de bord. C'est une **table de configuration de fournisseur** :
//! sa seule raison d'exister est la liste de ses quatre cles, parce que le
//! consommateur y accede par reflexion.
//!
//! La preuve que ces cles sont une donnee publique et non une commodite
//! interne se trouve dans `openai-compatible-chat-language-model.ts:171` :
//!
//! ```ts
//! ...Object.fromEntries(
//!   Object.entries(providerOptions?.[this.providerOptionsName] ?? {}).filter(
//!     ([key]) => !Object.keys(openaiCompatibleProviderOptions.shape).includes(key),
//!   ),
//! )
//! ```
//!
//! Le modele construit son corps de requete en reprenant les options du
//! fournisseur et en **debarrassant** les quatre cles consommees ici, pour que
//! le reste parte tel quel vers l'API. Une faute de frappe dans une de ces
//! quatre cles ne casse rien a la compilation, ne casse aucun test du code
//! Rust, et fait party d'une cle parasite dans le corps de chaque requete
//! envoyee a Copilot en production. C'est exactement le piege que ce fichier
//! traite de tete.
//!
//! ## Le piege des noms : DEUX conventions melangees
//!
//! Le champ le plus piege de tout le portage TypeScript vers Rust, parce que
//! c'est le seul du depot ou les deux styles coexistent **dans la meme table** :
//!
//! | cle TypeScript | style      | champ Rust          | surcharge |
//! |---|---|---|---|
//! | `user`              | mot seul   | `user`          | `user`          |
//! | `reasoningEffort`   | camelCase  | `reasoning_effort` | `reasoningEffort` |
//! | `textVerbosity`     | camelCase  | `text_verbosity`   | `textVerbosity`   |
//! | `thinking_budget`   | snake_case | `thinking_budget`   | `thinking_budget` |
//!
//! Trois consequences, toutes testees :
//!
//! 1. **Aucun `rename_all` sur ce struct.** Un `#[serde(rename_all =
//!    "camelCase")` donnerait `thinkingBudget` a la place de
//!    `thinking_budget` et casserait le budget de reflexion des modeles
//!    Anthropic de Copilot. Inversement, un `rename_all = "snake_case"`
//!    donnerait `reasoning_effort` et `text_verbosity` au lieu de
//!    `reasoningEffort` et `textVerbosity`. Les deux sont faux. Chaque champ
//!    porte donc son `#[serde(rename = "...")]` **explicite**, y compris
//!    `user`, ou le nom est identique : l'explicite sert a la relecture, qui
//!    peut ainsi verifier les quatre cles d'un coup d'oeil sans en deduire
//!    aucune.
//! 2. Le nom de champ `thinking_budget` est identique en TS et en Rust, ce qui
//!    rend la faute invisible : rien ne signale que ce champ doit porter une
//!    surcharge. Elle est posee quand meme, et un test verrouille le nom.
//! 3. `OpenAICompatibleChatModelId` est un alias de `string` : aucun sigle
//!    en majuscules ici, donc rien de ce piege ne s'applique aux types.
//!
//! ## Le piege de la veracite : il ne se pose pas, et c'est notable
//!
//! La source ne contient **ni ternaire ni coalescent**. Le piege numero un du
//! contexte ne s'applique donc pas mecaniquement a ce fichier. Mais le schema
//! lui-meme l'evoque, parce que `z.string().optional()` et `z.number()` sont
//! des validateurs de **nullite**, pas de veracite :
//!
//! - `user: ""` est **valide** et doit survivre. La chaine vide est falsy en
//!   JavaScript, donc un portage par test de veracite (un `?` transcrit en
//!   `.filter(|v| !v.is_empty())`) la supprimerait. Elle est portee ici.
//! - `thinking_budget: 0` est **valide** et doit survivre. `0` est falsy en
//!   JavaScript, un portage par veracite le supprimerait aussi. Il est porte
//!   ici.
//! - `z.number()` est un flottant, pas un entier : `1024.5` est accepte. Seul
//!   `NaN` est refuse, et JSON ne peut pas en contenir.
//!
//! Concretement, `Some("")` et `Some(0.0)` sont distincts de `None` ici, et
//! trois tests le verrouillent.
//!
//! ## Ce qui differe de zod, et pourquoi c'est acceptable
//!
//! - **`null` explicite.** En zod, `z.string().optional()` refuse `null` :
//!   seule l'absence de cle est acceptee, pas une valeur `null`. Le `Option<T>`
//! de Serde, lui, confond les deux et transforme `null` en `None`. C'est la
//!   meme divergence que le reste du portage zod vers Rust ; elle est invisible
//!   pour du JSON ecrit par le TypeScript, qui ne produit jamais `null` ici
//!   puisque les champs absents disparaissent a la serialisation. La
//!   consequence, elle, est reelle et assumee : sur une saisie manuelle,
//!   `{"user": null}` est accepte par le portage et refuse par l'original.
//! - **Cles inconnues.** `z.object` ne rejette pas les cles inconnues, il les
//!   **retire silencieusement** du resultat. Serde les ignore aussi, et c'est
//!   la seule propriete qui compte ici puisque le struct ne les stocke pas.
//!   `deny_unknown_fields` serait donc un changement de comportement, et n'est
//!   pas pose.

use serde::{Deserialize, Serialize};

/// Les quatre cles de `openaiCompatibleProviderOptions`, dans l'ordre de la
/// source.
///
/// En TypeScript le consommateur fait `Object.keys(...shape)` pour savoir
/// quelles options sont consommees par le modele et lesquelles doivent etre
/// transmises telles quelles a l'API. Cette constante est l'equivalent Rust de
/// cette reflexion, et un test verifie qu'elle ne derive pas du struct.
pub const PROVIDER_OPTIONS_SHAPE: [&str; 4] = [
    "user",
    "reasoningEffort",
    "textVerbosity",
    "thinking_budget",
];

/// Alias de `string` en TypeScript, sans contrainte ni branding.
///
/// Nom TS : `OpenAICompatibleChatModelId`. Ecrit ici `OpenAi...` car c'est la
/// forme Rust usuelle pour un sigle, et parce que `src/swarm/
/// provider_openai_compatible.rs` a deja pose `OpenAiCompatiblePlugin` pour le
/// meme sigle. Aucune donnee ne porte ce nom, donc le renommage n'a aucun effet
/// sur l'echange avec le TypeScript.
pub type OpenAiCompatibleChatModelId = String;

/// `OpenAICompatibleProviderOptions` : les options du fournisseur pour le
/// chat compatible OpenAI de GitHub Copilot.
///
/// Les quatre champs sont optionnels, et **les quatre noms de cle sont
/// contractuels** : ils sont lus par reflexion dans le modele de langue et
/// discrimines de toutes les autres options du fournisseur. Chaque champ porte
/// une surcharge serde explicite, et `PROVIDER_OPTIONS_SHAPE` doit rester
/// synchrone avec elles.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OpenAiCompatibleProviderOptions {
    /// Identifiant unique de l'utilisateur final. Aide le fournisseur a
    /// surveiller et a detecter les abus.
    #[serde(rename = "user", skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Effort de raisonnement des modeles qui raisonnent. Defaut `medium`.
    #[serde(
        rename = "reasoningEffort",
        skip_serializing_if = "Option::is_none"
    )]
    pub reasoning_effort: Option<String>,

    /// Verbosite du texte genere. Defaut `medium`.
    #[serde(
        rename = "textVerbosity",
        skip_serializing_if = "Option::is_none"
    )]
    pub text_verbosity: Option<String>,

    /// Budget de reflexion utilise par les modeles Anthropic de Copilot.
    ///
    /// Attention : cette cle est en **tiret bas** en TypeScript, alors que ses
    /// deux voisines sont en camelCase. C'est une exception deliberee du
    /// schema d'origine, pas une faute de frappe du portage.
    #[serde(
        rename = "thinking_budget",
        skip_serializing_if = "Option::is_none"
    )]
    pub thinking_budget: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    /// Un objet vide est valide : les quatre champs sont optionnels, et il
    /// ressort vide, sans aucune cle.
    #[test]
    fn un_objet_vide_donne_quatre_options_absentes() {
        let options: OpenAiCompatibleProviderOptions =
            serde_json::from_str("{}").expect("un objet vide doit etre valide");

        assert_eq!(options, OpenAiCompatibleProviderOptions::default());
        assert_eq!(options.user, None);
        assert_eq!(options.reasoning_effort, None);
        assert_eq!(options.text_verbosity, None);
        assert_eq!(options.thinking_budget, None);
        assert_eq!(serde_json::to_string(&options).unwrap(), "{}");
    }

    /// Les quatre cles sortent avec l'orthographe exacte de la source. Ce test
    /// est le verrou principal du fichier : une surcharge oubliee ou une faute
    /// de frappe ici produirait une requete Copilot fausse sans jamais lever une
    /// erreur de compilation.
    #[test]
    fn les_quatre_cles_sortent_avec_leur_orthographe_exacte() {
        let options = OpenAiCompatibleProviderOptions {
            user: Some("utilisateur-42".to_string()),
            reasoning_effort: Some("high".to_string()),
            text_verbosity: Some("low".to_string()),
            thinking_budget: Some(1024.5),
        };

        let valeur = serde_json::to_value(&options).unwrap();
        let objet = valeur.as_object().expect("le resultat est un objet");

        let mut cles: Vec<&str> = objet.keys().map(String::as_str).collect();
        cles.sort_unstable();
        assert_eq!(
            cles,
            vec![
                "reasoningEffort",
                "textVerbosity",
                "thinking_budget",
                "user"
            ],
            "les quatre cles doivent etre exactement celles de la source"
        );

        assert_eq!(objet["user"], json!("utilisateur-42"));
        assert_eq!(objet["reasoningEffort"], json!("high"));
        assert_eq!(objet["textVerbosity"], json!("low"));
        assert_eq!(objet["thinking_budget"], json!(1024.5));

        // Les formes qui ne doivent surtout PAS apparaitre : le tiret bas
        // n'a pas le droit de se glisser sur les deux cles camelCase, et le
        // camelCase n'a pas le droit de gagner sur la cle en tiret bas.
        let serialise = serde_json::to_string(&options).unwrap();
        assert!(serialise.contains("\"reasoningEffort\":"));
        assert!(serialise.contains("\"textVerbosity\":"));
        assert!(serialise.contains("\"thinking_budget\":"));
        assert!(!serialise.contains("\"reasoning_effort\":"));
        assert!(!serialise.contains("\"text_verbosity\":"));
        assert!(!serialise.contains("\"thinkingBudget\":"));
    }

    /// Un objet complet relu puis reecrit redonne exactement le meme JSON.
    #[test]
    fn un_all_et_ret_complet_ne_change_rien() {
        let source = r#"{
            "user": "utilisateur-42",
            "reasoningEffort": "medium",
            "textVerbosity": "high",
            "thinking_budget": 0
        }"#;

        let options: OpenAiCompatibleProviderOptions =
            serde_json::from_str(source).expect("l'objet complet doit etre valide");

        assert_eq!(options.user.as_deref(), Some("utilisateur-42"));
        assert_eq!(options.reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(options.text_verbosity.as_deref(), Some("high"));
        assert_eq!(options.thinking_budget, Some(0.0));

        let relu = serde_json::to_value(&options).unwrap();
        let attendu: Value = serde_json::from_str(source).unwrap();
        // Field by field: `thinking_budget` deserialises into an f64, so a
        // whole-Value comparison would oppose Number(0.0) to Number(0) and
        // fail on the integer/float representation, not on the data.
        assert_eq!(relu["user"], attendu["user"]);
        assert_eq!(relu["reasoningEffort"], attendu["reasoningEffort"]);
        assert_eq!(relu["textVerbosity"], attendu["textVerbosity"]);
        assert_eq!(relu["thinking_budget"].as_f64(), Some(0.0));
    }

    /// Une chaine vide est une valeur, pas une absence. `z.string()` accepte
    /// `""`, donc `user: ""` doit survivre : un portage par test de veracite
    /// le jetterait, puisque la chaine vide est falsy en JavaScript.
    #[test]
    fn une_chaine_vide_survit_et_ne_vaut_pas_une_absence() {
        let options: OpenAiCompatibleProviderOptions =
            serde_json::from_str(r#"{"user": "", "textVerbosity": ""}"#)
                .expect("une chaine vide est une chaine valide");

        assert_eq!(options.user.as_deref(), Some(""));
        assert_eq!(options.text_verbosity.as_deref(), Some(""));
        assert_ne!(options.user, None);

        // Et elle ressort : la chaine vide n'est pas un champ absent.
        let valeur = serde_json::to_value(&options).unwrap();
        assert_eq!(valeur["user"], json!(""));
        assert_eq!(valeur["textVerbosity"], json!(""));
        assert!(valeur.as_object().unwrap().contains_key("user"));
    }

    /// Un budget de zero est une valeur, pas une absence. `0` est falsy en
    /// JavaScript : c'est le second cas que le test de veracite perdrait, et le
    /// moins evident des deux parce qu'il ne s'agit pas d'une chaine.
    #[test]
    fn un_budget_de_zero_survit_et_ne_vaut_pas_une_absence() {
        let options: OpenAiCompatibleProviderOptions =
            serde_json::from_str(r#"{"thinking_budget": 0}"#)
                .expect("zero est un nombre valide");

        assert_eq!(options.thinking_budget, Some(0.0));
        assert_ne!(options.thinking_budget, None);
        // Compare as f64: the value serialises to Number(0.0), which never
        // equals the integer Number(0) in a whole-Value comparison.
        assert_eq!(
            serde_json::to_value(&options).unwrap()["thinking_budget"].as_f64(),
            Some(0.0)
        );
    }

    /// `z.number()` accepte les decimales : un budget de `1024.5` est valide.
    /// Le schema n'est pas un `z.int()`, le portage ne doit pas devenir un
    /// entier.
    #[test]
    fn un_budget_decimal_est_accepte() {
        let options: OpenAiCompatibleProviderOptions =
            serde_json::from_str(r#"{"thinking_budget": 1024.5}"#).unwrap();
        assert_eq!(options.thinking_budget, Some(1024.5));

        let options: OpenAiCompatibleProviderOptions =
            serde_json::from_str(r#"{"thinking_budget": -1}"#).unwrap();
        assert_eq!(options.thinking_budget, Some(-1.0));
    }

    /// Une entree invalide est refusee, cle par cle : une chaine la ou un
    /// nombre est attendu, ou un nombre la ou une chaine est attendue.
    #[test]
    fn une_entree_invalide_est_refusee() {
        // thinking_budget recoit une chaine.
        assert!(serde_json::from_str::<OpenAiCompatibleProviderOptions>(
            r#"{"thinking_budget": "1024"}"#
        )
        .is_err());

        // user recoit un nombre.
        assert!(
            serde_json::from_str::<OpenAiCompatibleProviderOptions>(r#"{"user": 42}"#)
                .is_err()
        );

        // reasoningEffort recoit un objet.
        assert!(serde_json::from_str::<OpenAiCompatibleProviderOptions>(
            r#"{"reasoningEffort": {"effort": "high"}}"#
        )
        .is_err());
    }

    /// Une cle inconnue est ignoree, jamais refusee. `z.object` la retire
    /// silencieusement du resultat, et le resultat ne la contient donc pas.
    /// C'est ce qui permet au modele de langue de distinguer les options
    /// consommees ici des options a transmettre telles quelles.
    #[test]
    fn une_cle_inconnue_est_ignoree_et_absente_du_resultat() {
        let options: OpenAiCompatibleProviderOptions = serde_json::from_str(
            r#"{"user": "u", "includeUsage": true, "top_logprobs": 3}"#,
        )
        .expect("une cle inconnue ne doit pas faire echouer la lecture");

        assert_eq!(options.user.as_deref(), Some("u"));

        let valeur = serde_json::to_value(&options).unwrap();
        let objet = valeur.as_object().unwrap();
        assert_eq!(objet.len(), 1);
        assert!(!objet.contains_key("includeUsage"));
        assert!(!objet.contains_key("top_logprobs"));
    }

    /// Un `null` explicite se lit comme une absence. C'est la seule divergence
    /// connue avec zod, qui refuserait cette entree : elle est documentee
    /// volontairement, et verrouillee ici pour qu'on la voie.
    #[test]
    fn un_null_explicite_vaut_une_absence() {
        let options: OpenAiCompatibleProviderOptions =
            serde_json::from_str(r#"{"user": null, "thinking_budget": null}"#)
                .expect("serde lit null comme une absence");

        assert_eq!(options.user, None);
        assert_eq!(options.thinking_budget, None);
        assert_eq!(serde_json::to_string(&options).unwrap(), "{}");
    }

    /// La liste des cles consommees par le modele de langue reste synchrone
    /// avec les quatre champs du struct. C'est la reflexion
    /// `Object.keys(shape)` du TypeScript, portee ici.
    #[test]
    fn la_liste_des_cles_du_schema_ressort_du_struct() {
        assert_eq!(PROVIDER_OPTIONS_SHAPE.len(), 4);
        for cle in PROVIDER_OPTIONS_SHAPE {
            let objet = serde_json::to_value(OpenAiCompatibleProviderOptions {
                user: Some("present".to_string()),
                reasoning_effort: Some("present".to_string()),
                text_verbosity: Some("present".to_string()),
                thinking_budget: Some(1.0),
            })
            .unwrap();
            assert!(
                objet.as_object().unwrap().contains_key(cle),
                "la cle {cle} doit exister dans le JSON produit"
            );
        }
        assert_eq!(PROVIDER_OPTIONS_SHAPE[0], "user");
        assert_eq!(PROVIDER_OPTIONS_SHAPE[1], "reasoningEffort");
        assert_eq!(PROVIDER_OPTIONS_SHAPE[2], "textVerbosity");
        assert_eq!(PROVIDER_OPTIONS_SHAPE[3], "thinking_budget");
    }

    /// Une seule option presente ne fait apparaitre qu'elle, et les trois
    /// autres restent absentes du JSON au lieu de sortir a `null`.
    #[test]
    fn une_seule_option_presente_ne_produit_qu_une_cle() {
        let options = OpenAiCompatibleProviderOptions {
            text_verbosity: Some("low".to_string()),
            ..Default::default()
        };

        assert_eq!(
            serde_json::to_string(&options).unwrap(),
            r#"{"textVerbosity":"low"}"#
        );
    }
}
