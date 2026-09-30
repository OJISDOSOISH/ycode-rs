//! Portage Rust de `opencode/packages/core/src/session/message.ts`.
//!
//! ## Ce que porte vraiment la source
//!
//! Le fichier d'origine fait **deux lignes**, et aucune ne declare quoi que ce
//! soit :
//!
//! ```ts
//! export * as SessionMessage from "./message"
//! export * from "@opencode-ai/schema/session-message"
//! ```
//!
//! Ce sont deux facades. La seconde est la seule qui parle du schema de
//! messages : `@opencode-ai/schema/session-message` designe
//! `packages/schema/src/session-message.ts`. La premiere est un alias de
//! namespace — en JavaScript, `export * as X` empaquette un module sous un nom
//! plutot que d'en lister les membres ; cote Rust, le module lui-meme joue
//! deja ce role, ce module *est* le namespace `SessionMessage`.
//!
//! Conclusion : il n'y a **aucun** contenu a porter ici. Tout ce que le fichier
//! expose vient d'ailleurs.
//!
//! ## Pourquoi un reexport, et pas une copie
//!
//! Le portage complet et riche de `session-message.ts` existe deja :
//! `crate::schema::session_message`. Il fait 689 lignes — l'union `Message`, ses
//! huit variantes, les etats d'appel d'outil, la compaction, les bornes de
//! snapshot, et une dizaine de tests de compatibilite JSON.
//!
//! Recopier ces types ici donnerait **deux definitions independantes du meme
//! contrat**. Elles compileraient toutes les deux, et divergeraient en silence :
//! le jour ou le schema gagne une variante, ou corrige un `rename`, seule une
//! des deux serait mise a jour, et les messages.serialises par l'une seraient
//! illisibles par l'autre. C'est exactement le defaut que la revue croisee a
//! deja attrape une fois, sur `sessionID` / `callID` (voir
//! `les_noms_de_champs_json_sont_ceux_du_typescript` dans le module d'origine).
//!
//! Un `pub use` rend les deux noms litteralement le meme type. Il n'y a rien a
//! resynchroniser, parce qu'il n'y a rien duplique.
//!
//! ## Pourquoi `_v1` dans le nom du fichier
//!
//! `src/schema/session_message.rs` occupe deja le nom `session_message`. Ce
//! fichier porte le meme contenu sous un autre nom, il ne peut donc pas
//! s'appeler pareil : c'est `session_message_v1`, et non `session_message`.
//!
//! ## Le point piege : les noms de champs sont en MAJUSCULES
//!
//! `projectID`, `sessionID`, `callID`, `providerID`. Ce ne sont pas des
//! `projectId` / `sessionId` : Serde emettrait alors un JSON que le
//! TypeScript d'origine ne sait pas lire. Le module porte ces `rename` sur ses
//! structs ; ici on se contente de ne pas les perdre au passage, ce que
//! verifie le test `les_noms_de_champs_reste_en_majuscules`.
//!
//! ## Couplage assume
//!
//! Ce module depend de `crate::schema::session_message`. Le cout est connu et
//! borne : si ce dernier bouge de chemin, la seule ligne a corriger ici est le
//! `pub use`. C'est un renommage de chemin, pas une reecriture de contrat.

pub use crate::schema::session_message::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Prend le type par le chemin **local**, celui passe par le `pub use`.
    fn accepte_le_type_local(_: Message) {}

    /// Prend le type par le chemin **d'origine**, celui du module de schema.
    fn accepte_le_type_d_origine(_: crate::schema::session_message::Message) {}

    #[test]
    fn le_type_vient_du_portage_schema_et_non_d_une_copie() {
        // Cette fonction ne compile que si `Message` ici et
        // `schema::session_message::Message` sont le MEME type. Une copie
        // divergente passerait tous les autres tests de ce fichier et
        // echouerait sur celui-ci, a la compilation.
        let m = Message::User(User {
            base: MessageBase::new("msg_v1", 1_700_000_000_000),
            prompt: Prompt {
                text: "bonjour".to_string(),
                files: vec![],
                agents: vec![],
            },
        });

        accepte_le_type_local(m.clone());
        accepte_le_type_d_origine(m);
    }

    #[test]
    fn type_name_designe_le_module_de_schema_comme_origine() {
        // Le chemin de definition affiche par le compilateur. Pour un
        // `pub use`, il reste celui du module qui *definit* le type, jamais
        // celui du reexport : c'est la preuve textuelle que rien n'a ete
        // recopie ici.
        let path = std::any::type_name::<Message>();
        assert!(
            path.contains("schema::session_message"),
            "Message doit venir de schema::session_message, vu comme : {path}"
        );
        assert!(
            !path.contains("swarm::session_message_v1"),
            "le type ne doit pas etre defini dans le reexport : {path}"
        );
    }

    #[test]
    fn les_noms_de_champs_reste_en_majuscules() {
        // Piege 1 du portage : `sessionID`, `callID`, `providerID`. Pas
        // `session_id`, pas `call_id`, pas `provider_id` au JSON. Ces noms
        // viennent du module de schema, le reexport doit les laisser passer
        // tels quels.
        let syn = Message::Synthetic(Synthetic {
            base: MessageBase::new("msg_v2", 1),
            session_id: "ses_1".to_string(),
            text: "x".to_string(),
        });
        let v = serde_json::to_value(&syn).unwrap();
        assert_eq!(v["sessionID"], "ses_1", "le champ doit s'appeler sessionID");
        assert!(v.get("session_id").is_none(), "snake_case interdit");

        let sh = Message::Shell(Shell::new("msg_v3", "call_9", "ls", "a", 5));
        let v = serde_json::to_value(&sh).unwrap();
        assert_eq!(v["callID"], "call_9", "le champ doit s'appeler callID");
        assert!(v.get("call_id").is_none(), "snake_case interdit");

        let mr = ModelRef::new("claude-sonnet-4-5", "opencode");
        let v = serde_json::to_value(&mr).unwrap();
        assert_eq!(v["providerID"], "opencode");
        assert!(v.get("provider_id").is_none(), "snake_case interdit");
    }

    #[test]
    fn la_surface_reexportee_est_la_surface_du_schema() {
        // Les deux lignes du TS sont des `export *` : tout ce que le module
        // de schema expose doit etre joignable par le chemin court. On
        // verifie les entree principales de la surface.
        assert_eq!(MSG_ID_PREFIX, "msg_");
        assert!(is_valid_msg_id("msg_v4"));
        assert!(!is_valid_msg_id("ses_v4"));

        let created: Millis = 42;
        let a = Message::Assistant(Assistant::new(
            "msg_v5",
            created,
            "build",
            ModelRef::new("claude-sonnet-4-5", "opencode"),
        ));
        assert_eq!(a.id(), "msg_v5");
        assert_eq!(a.type_str(), "assistant");

        // Discrimination d'union, et le piege du `time` unique.
        match &a {
            Message::Assistant(inner) => assert!(inner.base.time.is_none()),
            other => panic!("mauvaise variante : {}", other.type_str()),
        }

        assert!(ToolState::Completed {
            input: serde_json::json!({}),
            attachments: None,
            content: vec![],
            output_paths: None,
            structured: serde_json::json!({}),
            result: None,
        }
        .is_terminal());

        let compaction = Message::Compaction(Compaction {
            base: MessageBase::new("msg_v6", 1),
            reason: CompactionReason::Auto,
            summary: "s".to_string(),
            recent: "r".to_string(),
        });
        assert!(!compaction.counts_against_context());
    }
}