//! Portage Rust de `opencode/packages/core/src/session/prompt.ts`.
//!
//! ## La source : 94 octets, un seul reexport
//!
//! ```ts
//! export { AgentAttachment, FileAttachment, Prompt, Source } from "@opencode-ai/schema/prompt"
//! ```
//!
//! Il n'y a aucun code a traduire. Le contrat reel est deux niveaux plus bas,
//! dans `packages/schema/src/prompt.ts` (57 lignes), qui declare `Source`,
//! `FileAttachment`, `AgentAttachment` et `Prompt`, plus trois methodes posees
//! par `statics(...)` : `FileAttachment.create`, `Prompt.equivalence` et
//! `Prompt.fromUserMessage`.
//!
//! ## Pourquoi ce fichier n'est pas vide
//!
//! Un reexport n'a pas d'equivalent Rust : il n'y a pas de `pub use` a ecrire
//! depuis un nom de paquet npm. Se contenter du reexport ferait disparaitre les
//! quatre types du projet. On porte donc ce qui est reexporte, et surtout les
//! trois methodes, que le reexport de la source rend lui-memo accessibles.
//!
//! ## D'ou viennent les types : ce fichier ne les REDECLARE PAS
//!
//! Les quatre existent deja, a l'identique, dans `crate::core::session_event`.
//! Ils y ont ete ecrits pour porter `packages/schema/src/session-event.ts`, qui
//! consomme exactement le meme contrat. Les redeclarer ici produirait deux
//! definitions d'un meme contrat qui ne peuvent pas diverger bruyamment : Rust
//! autorise le meme nom dans deux modules, et rien ne signale l'ecart. C'est
//! exactement le piege signale sur `src/core/session/schema.rs` et
//! `src/schema/session_message.rs`.
//!
//! On les importe donc, et le test `les_types_viennent_de_core_session_event`
//! le verifie en comparant `type_name`, qui resout jusqu'au module de
//! DEFINITION et pas jusqu'au chemin d'import utilise.
//!
//! ## Les deux collisions voisines, et pourquoi on ne les touche pas
//!
//! - `crate::schema::session_message::Prompt` et `::FileAttachment` existent
//!   aussi, mais ce sont d'AUTRES contrats : `Prompt { text, files:
//!   Vec<FileAttachment>, agents: Vec<String> }` avec `FileAttachment { uri,
//!   mime, name?, description? }`. Aucun des deux n'a de champ `source`, ni de
//!   `Vec<AgentAttachment>`. Importer les aurait produit un fichier qui se
//!   compile et qui echange faux.
//! - `crate::core::session_event` definit aussi `EventSource`, avec `start` et
//!   `end` en `u64`. Le TS distingue bien `Session.Next.Event.Source` de
//!   `Prompt.Source`, dont `start` et `end` sont des `Schema.Finite`, donc des
//!   `f64`. Les deux noms sont conserves pour le meme type : `PromptSource`
//!   (nom du module d'origine) et `Source` (nom du TS).
//!
//! ## Le piege des majuscules : ici il ne se presente pas
//!
//! Aucune propriete de ce contrat ne porte de majuscule. Dix noms, tous des
//! mots simples en minuscules : `start`, `end`, `text`, `uri`, `mime`, `name`,
//! `description`, `source`, `files`, `agents`. Il n'y a donc **aucun**
//! `#[serde(rename)]` a poser ici, contrairement a `session/schema.rs` qui en
//! porte sur `projectID`, `workspaceID` ou `providerID`. Le test
//! `aucun_nom_de_champ_ne_contient_de_majuscule` verrouille ce constat, pour
//! qu'un `projectID` n'entre pas par habitude venue d'un autre fichier du lot.
//!
//! ## Ce que `optional` veut dire
//!
//! `Schema.String.pipe(optional)` vient de `packages/schema/src/schema.ts` :
//! `Schema.optionalKey` decode vers `Schema.optional`, et l'encodage passe par
//! `Option.filter((value) => value !== undefined)`. Donc cote TS :
//! cle absente donne `undefined`, `undefined` donne `undefined`, et `null` est
//! refuse. Cote Rust : `Option<T>`, avec `skip_serializing_if = "Option::is_none"`
//! a l'encodage.
//!
//! Une liste vide n'est pas une absence : `files: []` reste `[]`, et le test
//! `une_liste_vide_explicite_conserve_la_cle` le fixe.
//!
//! Seule divergence connue, et inherente a `Option` : serde lit `null` comme
//! `None`, la ou le TS refuse `null`. Invisible sur du JSON produit par le TS,
//! visible sur une saisie manuelle. Le test
//! `une_cle_nulle_atterrit_dans_le_meme_bucket_qu_une_cle_absente` la nomme.

pub use crate::core::session_event::{AgentAttachment, FileAttachment, Prompt, PromptSource};

/// Nom publie par le TypeScript : `Source`.
///
/// Le module d'origine nomme ce type `PromptSource`, parce qu'il porte aussi
/// un `EventSource` qui n'a rien a voir. Le TS, lui, n'a qu'un `Source`. On
/// expose les deux noms pour le meme type, afin qu'un lecteur qui compare avec
/// l'original ne cherche pas un `Source` absent.
pub use crate::core::session_event::PromptSource as Source;

/// `FileAttachment.create`, methode posee par `statics(...)` sur le schema.
///
/// En TypeScript, `create` est un constructeur validant :
/// `schema.make({ uri, mime, name, description, source })`.
///
/// En Rust, la validation EST le type : impossible de construire un
/// `FileAttachment` dont `uri` ou `mime` manquent, puisque ce sont des `String`
/// et non des `Option`. La fonction est donc l'identite, et sert de point
/// d'entree unique pour ceux qui construisent une piece jointe. Elle ne perd
/// aucun champ et n'en invente aucun.
#[allow(clippy::identity)]
pub fn create(input: FileAttachment) -> FileAttachment {
    input
}

/// Convenience de construction : les deux seuls champs obligatoires du schema.
///
/// `name`, `description` et `source` sont absents, donc absents du JSON.
/// N'est pas un export du TypeScript, c'est la lecture directe de
/// `Schema.Struct({ uri, mime, ... })`.
pub fn new_file_attachment(uri: impl Into<String>, mime: impl Into<String>) -> FileAttachment {
    FileAttachment { uri: uri.into(), mime: mime.into(), name: None, description: None, source: None }
}

/// `Prompt.fromUserMessage`, methode posee par `statics(...)`.
///
/// ```text
/// // packages/schema/src/prompt.ts
/// fromUserMessage: (input: Pick<Prompt, "text" | "files" | "agents">) =>
///   schema.make({
///     text: input.text,
///     ...(input.files === undefined ? {} : { files: input.files }),
///     ...(input.agents === undefined ? {} : { agents: input.agents }),
///   })
/// ```
///
/// Le point non trivial est la ternaire `... (input.files === undefined ? {} :
/// { files: input.files })`. La cle n'est pas mise a `undefined`, elle n'est pas
/// du tout posee : l'objet produit ne contient pas `files`. Un portage qui
/// ecrivait `files: Some(Vec::new())` pour une absence donnerait `"files": []`,
/// que le TS ne produit jamais.
///
/// `files` et `agents` sont donc des `Option`, et `None` donne une cle absente
/// grace a `skip_serializing_if`.
pub fn from_user_message(
    text: impl Into<String>,
    files: Option<Vec<FileAttachment>>,
    agents: Option<Vec<AgentAttachment>>,
) -> Prompt {
    Prompt { text: text.into(), files, agents }
}

/// `Prompt.equivalence`, methode posee par `statics(...)`.
///
/// En TypeScript, `equivalence: Schema.toEquivalence(schema)` construit une
/// fonction d'egalite structurelle a partir du schema.
///
/// En Rust, les quatre types derivent `PartialEq`, donc
/// l'equivalence du TS EST le `PartialEq` derive. La fonction n'ajoute rien,
/// elle donne le nom publie par le contrat.
pub fn equivalence(gauche: &Prompt, droite: &Prompt) -> bool {
    gauche == droite
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les cles d'un objet JSON, triees, pour comparer des jeux de cles.
    fn cles(valeur: &serde_json::Value) -> Vec<String> {
        let mut trouvees: Vec<String> = valeur
            .as_object()
            .expect("valeur JSON objet")
            .keys()
            .cloned()
            .collect();
        trouvees.sort();
        trouvees
    }

    fn source_de_test() -> PromptSource {
        PromptSource { start: 0.0, end: 4.0, text: "salut".to_string() }
    }

    #[test]
    fn les_types_viennent_de_core_session_event() {
        // Le test le plus important du fichier. Si les quatre types etaient
        // REDECLARES ici, `type_name` renverrait `...::session_prompt::Prompt`
        // au lieu de `...::core::session_event::Prompt`, et les deux
        // definitions divergeraient en silence. `type_name` resout jusqu'au
        // module de definition, pas jusqu'au chemin d'import : c'est donc
        // exactement la preuve qu'il n'y a qu'une seule definition.
        assert!(std::any::type_name::<Prompt>().ends_with("core::session_event::Prompt"));
        assert!(std::any::type_name::<PromptSource>().ends_with("core::session_event::PromptSource"));
        assert!(std::any::type_name::<FileAttachment>().ends_with("core::session_event::FileAttachment"));
        assert!(std::any::type_name::<AgentAttachment>().ends_with("core::session_event::AgentAttachment"));

        // Comparaison directe des deux cotes, sans rien ecrire en dur.
        assert_eq!(
            std::any::type_name::<Prompt>(),
            std::any::type_name::<crate::core::session_event::Prompt>()
        );
        assert_eq!(
            std::any::type_name::<AgentAttachment>(),
            std::any::type_name::<crate::core::session_event::AgentAttachment>()
        );
    }

    #[test]
    fn le_nom_du_typescript_source_designe_le_meme_type() {
        // `Source` et `PromptSource` sont deux noms du meme type. Cette
        // affectation ne compile que s'il en est ainsi : c'est une preuve
        // compile-time, la seule disponible sans build.
        let publie: Source = PromptSource { start: 1.5, end: 2.5, text: "x".to_string() };
        let interne: PromptSource = publie;
        assert_eq!(interne.start, 1.5);
        assert_eq!(std::any::type_name::<Source>(), std::any::type_name::<PromptSource>());
    }

    #[test]
    fn les_cles_du_json_sont_exactement_celles_du_typescript() {
        // Les quatre structures, champ par champ, dans l'ordre du schema.
        let source = serde_json::to_value(source_de_test()).expect("serialisation");
        assert_eq!(cles(&source), vec!["end", "start", "text"]);

        let fichier = FileAttachment {
            uri: "file:///a.png".to_string(),
            mime: "image/png".to_string(),
            name: Some("a.png".to_string()),
            description: Some("capture".to_string()),
            source: Some(source_de_test()),
        };
        let json_fichier = serde_json::to_value(&fichier).expect("serialisation");
        assert_eq!(cles(&json_fichier), vec!["description", "mime", "name", "source", "uri"]);

        let agent = AgentAttachment { name: "build".to_string(), source: Some(source_de_test()) };
        let json_agent = serde_json::to_value(&agent).expect("serialisation");
        assert_eq!(cles(&json_agent), vec!["name", "source"]);

        let invite = Prompt {
            text: "salut".to_string(),
            files: Some(vec![fichier]),
            agents: Some(vec![agent]),
        };
        let json_invite = serde_json::to_value(&invite).expect("serialisation");
        assert_eq!(cles(&json_invite), vec!["agents", "files", "text"]);
    }

    #[test]
    fn aucun_nom_de_champ_ne_contient_de_majuscule() {
        // Contraste explicite avec `core/session/schema.rs`, qui porte
        // `projectID`, `workspaceID`, `parentID`, `messageID`. Ce contrat n'a
        // que des mots simples, donc aucun `#[serde(rename)]`. Si quelqu'un
        // ajoute un champ, ce test doit rester vert : c'est la preuve qu'il
        // n'a pas introduit de majuscule fantaisiste.
        let invite = Prompt {
            text: "t".to_string(),
            files: Some(vec![new_file_attachment("file:///a", "image/png")]),
            agents: Some(vec![AgentAttachment { name: "build".to_string(), source: Some(source_de_test()) }]),
        };
        let json = serde_json::to_value(&invite).expect("serialisation");

        fn visiter(valeur: &serde_json::Value, chemin: &str) {
            match valeur {
                serde_json::Value::Object(carte) => {
                    for (cle, fils) in carte {
                        assert!(
                            !cle.chars().any(|c: char| c.is_uppercase()),
                            "{chemin}.{cle} porte une majuscule, absente du TypeScript"
                        );
                        visiter(fils, &format!("{chemin}.{cle}"));
                    }
                }
                serde_json::Value::Array(tableau) => {
                    for (rang, fils) in tableau.iter().enumerate() {
                        visiter(fils, &format!("{chemin}[{rang}]"));
                    }
                }
                _ => {}
            }
        }
        visiter(&json, "prompt");
    }

    #[test]
    fn une_piece_jointe_sans_optionnel_ne_serialise_que_uri_et_mime() {
        // `Schema.Struct({ uri, mime, name?, description?, source? })` : deux
        // obligatoires, trois absents. `optional` retire la cle, il ne la met
        // pas a null.
        let fichier = new_file_attachment("file:///a.png", "image/png");
        let json = serde_json::to_value(&fichier).expect("serialisation");
        assert_eq!(cles(&json), vec!["mime", "uri"]);
        assert!(json.get("name").is_none());
        assert!(json.get("source").is_none());

        // `create` ne perd rien et n'ajoute rien.
        assert_eq!(create(fichier.clone()), fichier);
        assert_eq!(create(new_file_attachment("u", "m")), new_file_attachment("u", "m"));
    }

    #[test]
    fn from_user_message_omet_la_cle_quand_l_argument_est_absent() {
        // LA ternaire de la source. `input.files === undefined ? {} :
        // { files: input.files }` ne pose pas la cle. Un `Some(Vec::new())`
        // naif aurait produit `{"text":"salut","files":[],"agents":[]}`, que le
        // TypeScript ne produit jamais.
        let invite = from_user_message("salut", None, None);
        let json = serde_json::to_value(&invite).expect("serialisation");
        assert_eq!(cles(&json), vec!["text"]);
        assert_eq!(serde_json::to_string(&invite).expect("json"), r#"{"text":"salut"}"#);
    }

    #[test]
    fn une_liste_vide_explicite_conserve_la_cle() {
        // Le contraire du test precedent, et tout aussi piegeux : `[]` n'est
        // pas une absence. Le TS distingue `undefined` de `[]`, et le portage
        // doit le faire aussi.
        let invite = from_user_message("salut", Some(Vec::new()), Some(Vec::new()));
        let json = serde_json::to_value(&invite).expect("serialisation");
        assert_eq!(cles(&json), vec!["agents", "files", "text"]);
        assert_eq!(json["files"].as_array().expect("tableau").len(), 0);
        assert_eq!(json["agents"].as_array().expect("tableau").len(), 0);
    }

    #[test]
    fn les_positions_de_source_sont_des_flottants_pas_des_entiers() {
        // `Schema.Finite` est un double JS. Une position fractionnaire est
        // donc licite, et doit survivre a l'aller-retour. Attention aussi :
        // serde_json ecrit `0.0` la ou le TS ecrit `0`, ce qui est le meme
        // nombre pour une comparaison en JS.
        let invite = from_user_message(
            "salut",
            Some(vec![FileAttachment {
                uri: "file:///a.png".to_string(),
                mime: "image/png".to_string(),
                name: None,
                description: None,
                source: Some(PromptSource { start: 0.5, end: 3.25, text: "salut".to_string() }),
            }]),
            None,
        );
        let json = serde_json::to_value(&invite).expect("serialisation");
        assert_eq!(json["files"][0]["source"]["start"].as_f64(), Some(0.5));
        assert_eq!(json["files"][0]["source"]["end"].as_f64(), Some(3.25));

        let relu: Prompt = serde_json::from_value(json).expect("relecture");
        // `relu.files` est consomme, pas lu : l'indexation borrowerait et
        // laisserait `.source` derriere. On prend donc le premier element par
        // valeur, et `relu.agents` reste lisible, champ disjoint.
        let premier = relu.files.expect("files").into_iter().next().expect("files");
        assert_eq!(premier.source.expect("source").end, 3.25);
        assert!(relu.agents.is_none());
    }

    #[test]
    fn equivalence_correspond_a_l_egalite_structurelle_et_au_json() {
        // `Schema.toEquivalence` donne l'egalite structurelle. Comme les
        // quatre types deriving `PartialEq`, la fonction doit se confondre avec
        // le derive, et l'egalite des JSON doit suivre : deux `Prompt`
        // structurelement inegaux ne doivent pas non plus produire le meme
        // JSON. Le couple `None` contre `Some(vec![])` est le cas qui casse si
        // un `skip_serializing_if` est oublie.
        let gauche = from_user_message("salut", Some(Vec::new()), None);
        let droite = from_user_message("salut", None, None);
        assert!(!equivalence(&gauche, &droite));
        assert_ne!(
            serde_json::to_string(&gauche).expect("json"),
            serde_json::to_string(&droite).expect("json")
        );

        let meme_texte = from_user_message("salut", None, None);
        assert!(equivalence(&gauche, &gauche));
        assert!(equivalence(&droite, &meme_texte));
        assert_eq!(
            serde_json::to_string(&droite).expect("json"),
            serde_json::to_string(&meme_texte).expect("json")
        );

        let avec_agent = from_user_message(
            "salut",
            None,
            Some(vec![AgentAttachment { name: "build".to_string(), source: None }]),
        );
        assert!(!equivalence(&avec_agent, &droite));
        assert!(equivalence(&avec_agent, &avec_agent));
    }

    #[test]
    fn un_prompt_venus_du_typescript_se_relit_tel_quel() {
        // Sens inverse : ce que le TS ecrit doit etre lisible sans
        // transformation, et les cles absentes doivent devenir `None`.
        let brut = r#"{
            "text": "regarde ca",
            "files": [
                {
                    "uri": "file:///a.png",
                    "mime": "image/png",
                    "name": "a.png",
                    "source": {"start": 0, "end": 12, "text": "regarde ca"}
                }
            ]
        }"#;
        let invite: Prompt = serde_json::from_str(brut).expect("relecture");
        assert_eq!(invite.text, "regarde ca");
        assert!(invite.agents.is_none());

        // `clone` et non un deplacement du champ : deplacer `invite.files`
        // laisserait `invite` partiellement deplace, et sa re-serialisation
        // plus bas serait alors refusee par le compilateur.
        let fichiers = invite.files.clone().expect("files");
        assert_eq!(fichiers.len(), 1);
        assert_eq!(fichiers[0].uri, "file:///a.png");
        assert_eq!(fichiers[0].mime, "image/png");
        assert_eq!(fichiers[0].name.as_deref(), Some("a.png"));
        assert!(fichiers[0].description.is_none());
        assert_eq!(fichiers[0].source.as_ref().expect("source").text, "regarde ca");

        // Le source d'une piece jointe est lui-meme optionnel, y compris
        // quand la piece est presente : `source` n'est pas `uri`.
        let sans_source = r#"{"uri":"file:///b.bin","mime":"application/octet-stream"}"#;
        let minimal: FileAttachment = serde_json::from_str(sans_source).expect("relecture");
        assert_eq!(minimal.uri, "file:///b.bin");
        assert!(minimal.source.is_none());
        assert!(minimal.name.is_none());

        // Re-encodage : les cles absentes ne reviennent pas.
        let json = serde_json::to_value(&invite).expect("serialisation");
        assert!(json.get("agents").is_none());
        assert_eq!(cles(&json), vec!["files", "text"]);
        assert!(json["files"][0].get("description").is_none());
    }

    #[test]
    fn une_cle_nulle_atterrit_dans_le_meme_bucket_qu_une_cle_absente() {
        // Divergence nommee et assumee : `Option<T>` ne distingue pas `null`
        // de l'absence, alors que `Schema.optional` refuse `null`. Sans effet
        // sur du JSON ecrit par le TS, qui n'en produit jamais ; visible sur
        // une saisie manuelle. Le test la fixe pour qu'elle ne change pas en
        // silence.
        let avec_null = r#"{"text":"t","files":null,"agents":null}"#;
        let invite: Prompt = serde_json::from_str(avec_null).expect("relecture");
        assert!(invite.files.is_none());
        assert!(invite.agents.is_none());
        assert_eq!(serde_json::to_string(&invite).expect("json"), r#"{"text":"t"}"#);
    }
}