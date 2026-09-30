//! Portage de `packages/core/src/session/event.ts`.
//!
//! Le fichier TypeScript d'origine tient deux lignes, et deux lignes
//! seulement :
//!
//! ```text
//! export * from "@opencode-ai/schema/session-event"
//! export * as SessionEvent from "@opencode-ai/schema/session-event"
//! ```
//!
//! C'est un reexport pur : aucune structure, aucun schema, aucune table n'est
//! declaree a cet endroit. Tout le contrat `session.next.*` vit dans le paquet
//! `@opencode-ai/schema`, dont la traduction Rust existe deja et est complete :
//! `src/core/session_event.rs` (portage de `packages/schema/src/session-event.ts`).
//!
//! Ce fichier ne fait donc qu'ouvrir ce portage sous un second chemin. Il
//! n'invente rien et ne redeclare rien : `pub use ..::*` est l'equivalent direct
//! de `export *`, et c'est tout.
//!
//! ## Les deux lignes de la source
//!
//! - `export * from ...` : reporte tous les noms du module dans ce module.
//! - `export * as SessionEvent from ...` : expose le module sous le nom
//!   `SessionEvent`.
//!
//! En Rust la seconde ligne n'a pas d'equivalent distinct, et c'est heureux :
//! le schema nomme deja son union complete `SessionEvent` (le `export const All`
//! de `session-event.ts`), et le reexport en nuage lui donne ce nom la ou on
//! l'attend. Un `use swarm::session_event::SessionEvent` designe donc la meme
//! chose que `use core::session_event::SessionEvent` : c'est voulu, pas un
//! conflit. L'union durable, elle, s'appelle `SessionDurableEvent`, pour le
//! `export const Durable` de la source.
//!
//! ## Ce qui n'est pas dans ce fichier, et pourquoi
//!
//! - Les 32 noms `session.next.*` et leurs versions durables sont dans
//!   `src/swarm/public_event_manifest.rs`. Ses tables ne sont pas recopiees ici :
//!   ce module est interroge, pas duplique.
//! - Les `sessionID`, `messageID`, `assistantMessageID`, `callID`, `textID`,
//!   `reasoningID` et consorts sont des renommages `serde` poses sur les
//!   structs de `src/core/session_event.rs`. Les recopier ici les detruirait.
//! - `UnknownError`, que la source declare comme un simple alias de
//!   `SessionMessage.UnknownError`, reste le type de `core::session_event`.
//!   `src/schema/session_message.rs` en porte un homonyme au contrat identique
//!   sur le fil, mais ce sont deux structs Rust distinctes ; choisir celle du
//!   portage `session-event` evite d introduire une conversion qui n'existe pas
//!   de ce cote.
//!
//! ## Etat de l'integration
//!
//! `src/swarm/mod.rs` declare bien `pub mod session_event;` : le module est
//! cable, et `crate::swarm::session_event` est donc joignable de l'exterieur.
//!
//! Le fichier ne declare aucun type, aucune constante et aucune fonction de
//! son propre chef : uniquement le `pub use` et son module de tests. Il n'y a
//! donc rien qui puisse dupliquer un nom de `core::session_event`, et le test
//! `le_reexport_designe_le_module_d_origine_comme_definition` le prouve en
//! executable plutot qu'en commentaire.

pub use crate::core::session_event::*;

#[cfg(test)]
mod tests {
    use super::*;

    // Quatre tests verifient une chose : que le reexport ne perd rien au
    // passage, en particulier les majuscules des identifiants. Une faute sur
    // `sessionID` ou `callID` ne se voit ni a la compilation ni dans un
    // round-trip Rust vers Rust ; elle ne se voit que si l'on compare au JSON
    // que le TypeScript ecrit reellement. C'est ce que font ces tests.
    // Le cinquieme ne teste pas le JSON : il teste qu'il n'y a pas de copie.

    #[test]
    fn les_deux_unions_de_la_source_sont_atteignables_par_ce_chemin() {
        let ev = SessionEvent::AgentSwitched(EventEnvelope {
            id: "evt_1".to_string(),
            metadata: None,
            durable: None,
            location: None,
            data: AgentSwitchedData {
                timestamp: 1_700_000_000_000,
                session_id: "ses_abc".to_string(),
                message_id: "msg_1".to_string(),
                agent: "plan".to_string(),
            },
        });
        let json = serde_json::to_value(&ev).expect("serialisation JSON");
        assert_eq!(json["type"], serde_json::json!("session.next.agent.switched"));
        // Le meme evenement passe par `Durable` : les deux unions de la source
        // sont bien routees jusqu'ici.
        let durable: SessionDurableEvent =
            serde_json::from_value(json).expect("agent switched est durable");
        match durable {
            SessionDurableEvent::AgentSwitched(env) => assert_eq!(env.data.agent, "plan"),
            autre => panic!("mauvaise variante : {:?}", autre),
        }
    }

    #[test]
    fn les_identifiants_restent_en_majuscules_dans_le_json() {
        // `ID` en majuscules dans le JSON, toujours. Ni `Id`, ni `session_id`.
        let ev = SessionEvent::ToolInputStarted(EventEnvelope {
            id: "evt_2".to_string(),
            metadata: None,
            durable: None,
            location: None,
            data: ToolInputStartedData {
                timestamp: 1_700_000_000_000,
                session_id: "ses_abc".to_string(),
                assistant_message_id: "msg_a".to_string(),
                call_id: "call_1".to_string(),
                name: "bash".to_string(),
            },
        });
        let json = serde_json::to_value(&ev).expect("serialisation JSON");
        assert_eq!(json["data"]["sessionID"], serde_json::json!("ses_abc"));
        assert_eq!(json["data"]["assistantMessageID"], serde_json::json!("msg_a"));
        assert_eq!(json["data"]["callID"], serde_json::json!("call_1"));
        for interdit in ["sessionId", "assistantMessageId", "callId", "session_id"] {
            assert!(json["data"].get(interdit).is_none(), "{} ne doit pas sortir", interdit);
        }
    }

    #[test]
    fn une_orthographe_sans_majuscules_est_refusee() {
        // La contre-epreuve du test precedent : si quelqu un reintroduit un
        // `sessionId` cote TypeScript, la deserialization doit echouer plutot
        // que d'ignorer le champ en silence et de valider un evenement vide.
        let json = serde_json::json!({
            "id": "evt_3",
            "type": "session.next.agent.switched",
            "data": {
                // `1_700_000_000_000` ms is a millisecond timestamp, far past
                // `i32::MAX`. Without the suffix, integer inference picks `i32`
                // and the literal is out of range. Same trap as core/session/schema.rs.
                "timestamp": 1_700_000_000_000i64,
                "sessionId": "ses_abc",
                "messageId": "msg_1",
                "agent": "plan"
            }
        });
        assert!(serde_json::from_value::<SessionEvent>(json.clone()).is_err());
        assert!(serde_json::from_value::<SessionDurableEvent>(json).is_err());
    }

    #[test]
    fn les_tags_serialises_sont_reconnus_du_manifeste() {
        // Le manifeste reste dans son fichier : on l'interroge au lieu de
        // recopier ses 88 entrees et ses 35 versions.
        let ev = SessionEvent::StepEnded(EventEnvelope {
            id: "evt_4".to_string(),
            metadata: None,
            durable: None,
            location: None,
            data: StepEndedData {
                timestamp: 1_700_000_000_000,
                session_id: "ses_abc".to_string(),
                assistant_message_id: "msg_a".to_string(),
                finish: "stop".to_string(),
                cost: 0.5,
                tokens: StepTokens {
                    input: 10.0,
                    output: 20.0,
                    reasoning: 5.0,
                    cache: StepTokensCache { read: 1.0, write: 2.0 },
                },
                snapshot: None,
                files: None,
            },
        });
        let json = serde_json::to_value(&ev).expect("serialisation JSON");
        let tag = json["type"].as_str().expect("tag textual");
        assert!(
            crate::swarm::public_event_manifest::is_public(tag),
            "{} absent du manifeste", tag
        );
        // `step.ended` est l'un des rares evenements en version 2, ce que le
        // reexport ne doit ni lisser ni perdre.
        assert_eq!(crate::swarm::public_event_manifest::durable_version(tag), Some(2));
        // Et ses champs optionnels absents ne doivent pas apparaitre.
        assert!(json["data"].get("snapshot").is_none());
        assert!(json["data"].get("files").is_none());
    }

    #[test]
    fn le_reexport_designe_le_module_d_origine_comme_definition() {
        // Contre-epreuve du doublon. Si ce fichier redeclarait `SessionEvent`
        // au lieu de le reexporter, ce test Echouerait : `type_name` renvoie le
        // chemin ou le type est *defini*, jamais celui par ou on l'atteint.
        let path = std::any::type_name::<SessionEvent>();
        assert!(
            path.contains("core::session_event"),
            "SessionEvent doit venir de core::session_event, vu comme : {path}"
        );
        assert!(
            !path.contains("swarm::session_event"),
            "le type ne doit pas etre defini dans le reexport : {path}"
        );

        // Meme verification sur l'union durable : les deux, pas seulement la
        // premiere, doivent etre des alias et non des copies.
        let path = std::any::type_name::<SessionDurableEvent>();
        assert!(
            path.contains("core::session_event"),
            "SessionDurableEvent doit venir de core::session_event, vu comme : {path}"
        );

        // Et une preuve par le typage : le chemin court et le chemin d'origine
        // acceptent le MEME type. Une copie divergente passerait tous les
        // tests precedents et echouerait ici, a la compilation.
        fn par_le_chemin_court(_: SessionEvent) {}
        fn par_le_chemin_d_origine(_: crate::core::session_event::SessionEvent) {}
        par_le_chemin_court(SessionEvent::RevertCleared(EventEnvelope {
            id: "evt_5".to_string(),
            metadata: None,
            durable: None,
            location: None,
            data: RevertClearedData {
                timestamp: 1_700_000_000_000,
                session_id: "ses_abc".to_string(),
            },
        }));
        par_le_chemin_d_origine(SessionEvent::RevertCleared(EventEnvelope {
            id: "evt_6".to_string(),
            metadata: None,
            durable: None,
            location: None,
            data: RevertClearedData {
                timestamp: 1_700_000_000_000,
                session_id: "ses_abc".to_string(),
            },
        }));
    }
}