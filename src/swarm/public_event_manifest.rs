//! Portage de `packages/core/src/public-event-manifest.ts`.
//!
//! Le fichier TypeScript d'origine ne tient que sept lignes :
//!
//! ```text
//! export * as PublicEventManifest from "./public-event-manifest"
//!
//! import { Event } from "@opencode-ai/schema/event"
//! import { EventManifest } from "@opencode-ai/schema/event-manifest"
//!
//! export const Definitions = EventManifest.ServerDefinitions
//! export const Latest = Event.latest(Definitions)
//! ```
//!
//! Donc deux exports seulement, tous deux calcules a partir du paquet
//! `@opencode-ai/schema`. C'est ce contenu la qui est porte ici, parce qu'un
//! reexport n'a pas d'equivalent direct en Rust et parce que ce sont les NOMS
//! qui compte : une faute de recopie sur une seule clef casse l'echange avec le
//! TypeScript sans jamais se voir a la compilation.
//!
//! Assemblage, dans l'ordre exact ou le TypeScript ecrit ses `Event.inventory`.
//! Chaque bloc est un `Event.inventory` d'un autre fichier du paquet schema :
//!
//! ```text
//! core        = sessionV1 durable (7)  puis session-event (32)     -> 39
//! foundation  = models-dev (1) integration (2) catalog (1) core (39) -> 43
//! feature     = file (1) reference (1) permission v2 (2) plugin (1)
//!               project.directories (1) file.watcher (1) pty (4)
//!               question v2 (3)                                          -> 14
//! server      = foundation (43) feature (14) todo (1)                 -> 58
//! public      = foundation (43) sessionV1 live (3) installation (2)
//!               feature (14) todo (1) lsp (1) permission v1 (2)
//!               tui (4) mcp (2) legacy (1) project (1) session.status (2)
//!               question v1 (3) compaction (1) vcs (1) workspace (3)
//!               worktree (2) server (2)                                 -> 88
//! ```
//!
//! Point de vigilance pour la relecture : le test TypeScript
//! `packages/schema/test/event-manifest.test.ts` annonce 55, 85 et 32. Ces
//! trois nombres sont anterieurs a l'ajout de trois definitions
//! `session.next.*` (trois de plus dans `Definitions`, dans
//! `SessionEvent.Definitions` et donc dans `ServerDefinitions` et `Durable`),
//! ils ne correspondent plus a la source. Les compteurs ci-dessous sont
//! recalcules depuis la source, pas depuis ce test.

use std::collections::BTreeMap;

/// Toutes les definitions d'evenements publics, dans l'ordre du manifeste.
///
/// 88 entrees. L'ordre compte autant que le contenu : c'est lui qui permet de
/// comparer deux manifestes champ par champ.
pub const DEFINITIONS: [&str; 88] = [
    // foundation : models-dev, integration, catalog
    "models-dev.refreshed",
    "integration.updated",
    "integration.connection.updated",
    "catalog.updated",
    // foundation : les 7 evenements session v1 durables
    "session.created",
    "session.updated",
    "session.deleted",
    "message.updated",
    "message.removed",
    "message.part.updated",
    "message.part.removed",
    // foundation : session-event
    "session.next.agent.switched",
    "session.next.model.switched",
    "session.next.moved",
    "session.next.prompted",
    "session.next.prompt.admitted",
    "session.next.context.updated",
    "session.next.synthetic",
    "session.next.shell.started",
    "session.next.shell.ended",
    "session.next.step.started",
    "session.next.step.ended",
    "session.next.step.failed",
    "session.next.text.started",
    "session.next.text.delta",
    "session.next.text.ended",
    "session.next.reasoning.started",
    "session.next.reasoning.delta",
    "session.next.reasoning.ended",
    "session.next.tool.input.started",
    "session.next.tool.input.delta",
    "session.next.tool.input.ended",
    "session.next.tool.called",
    "session.next.tool.progress",
    "session.next.tool.success",
    "session.next.tool.failed",
    "session.next.retried",
    "session.next.compaction.started",
    "session.next.compaction.delta",
    "session.next.compaction.ended",
    "session.next.revert.staged",
    "session.next.revert.cleared",
    "session.next.revert.committed",
    // les 3 evenements session v1 non durables
    "message.part.delta",
    "session.diff",
    "session.error",
    // installation
    "installation.updated",
    "installation.update-available",
    // feature : file, reference, permission v2, plugin, project.directories,
    // file.watcher, pty, question v2
    "file.edited",
    "reference.updated",
    "permission.v2.asked",
    "permission.v2.replied",
    "plugin.added",
    "project.directories.updated",
    "file.watcher.updated",
    "pty.created",
    "pty.updated",
    "pty.exited",
    "pty.deleted",
    "question.v2.asked",
    "question.v2.replied",
    "question.v2.rejected",
    // todo
    "todo.updated",
    // lsp
    "lsp.updated",
    // permission v1
    "permission.asked",
    "permission.replied",
    // tui
    "tui.prompt.append",
    "tui.command.execute",
    "tui.toast.show",
    "tui.session.select",
    // mcp
    "mcp.tools.changed",
    "mcp.browser.open.failed",
    // legacy
    "command.executed",
    // project
    "project.updated",
    // session status
    "session.status",
    "session.idle",
    // question v1
    "question.asked",
    "question.replied",
    "question.rejected",
    // session compaction
    "session.compacted",
    // vcs
    "vcs.branch.updated",
    // workspace
    "workspace.ready",
    "workspace.failed",
    "workspace.status",
    // worktree
    "worktree.ready",
    "worktree.failed",
    // server
    "server.connected",
    "global.disposed",
];

/// Les definitions reservees au serveur, dans l'ordre du manifeste.
///
/// 58 entrees. C'est la valeur que le fichier d'origine expose sous le nom
/// `Definitions`, et c'est donc celle qui alimente `Latest`.
pub const SERVER_DEFINITIONS: [&str; 58] = [
    // foundation : 43 entrees, identiques au debut du manifeste public
    "models-dev.refreshed",
    "integration.updated",
    "integration.connection.updated",
    "catalog.updated",
    "session.created",
    "session.updated",
    "session.deleted",
    "message.updated",
    "message.removed",
    "message.part.updated",
    "message.part.removed",
    "session.next.agent.switched",
    "session.next.model.switched",
    "session.next.moved",
    "session.next.prompted",
    "session.next.prompt.admitted",
    "session.next.context.updated",
    "session.next.synthetic",
    "session.next.shell.started",
    "session.next.shell.ended",
    "session.next.step.started",
    "session.next.step.ended",
    "session.next.step.failed",
    "session.next.text.started",
    "session.next.text.delta",
    "session.next.text.ended",
    "session.next.reasoning.started",
    "session.next.reasoning.delta",
    "session.next.reasoning.ended",
    "session.next.tool.input.started",
    "session.next.tool.input.delta",
    "session.next.tool.input.ended",
    "session.next.tool.called",
    "session.next.tool.progress",
    "session.next.tool.success",
    "session.next.tool.failed",
    "session.next.retried",
    "session.next.compaction.started",
    "session.next.compaction.delta",
    "session.next.compaction.ended",
    "session.next.revert.staged",
    "session.next.revert.cleared",
    "session.next.revert.committed",
    // feature : 14 entrees
    "file.edited",
    "reference.updated",
    "permission.v2.asked",
    "permission.v2.replied",
    "plugin.added",
    "project.directories.updated",
    "file.watcher.updated",
    "pty.created",
    "pty.updated",
    "pty.exited",
    "pty.deleted",
    "question.v2.asked",
    "question.v2.replied",
    "question.v2.rejected",
    // todo
    "todo.updated",
];

/// Les evenements durables, avec leur version, dans l'ordre du manifeste.
///
/// 35 entrees. Une version par evenement parce que le stockage exige un
/// identifiant unique : la cle est `type.version`. Seul `session.next` en a
/// deux, la version 2 pour les evenements de fin de tour, nommes par
/// `stepSettlementOptions` en amont.
pub const DURABLE_DEFINITIONS: [(&str, u64); 35] = [
    ("session.created", 1),
    ("session.updated", 1),
    ("session.deleted", 1),
    ("message.updated", 1),
    ("message.removed", 1),
    ("message.part.updated", 1),
    ("message.part.removed", 1),
    ("session.next.agent.switched", 1),
    ("session.next.model.switched", 1),
    ("session.next.moved", 1),
    ("session.next.prompted", 1),
    ("session.next.prompt.admitted", 1),
    ("session.next.context.updated", 1),
    ("session.next.synthetic", 1),
    ("session.next.shell.started", 1),
    ("session.next.shell.ended", 1),
    ("session.next.step.started", 1),
    ("session.next.step.ended", 2),
    ("session.next.step.failed", 2),
    ("session.next.text.started", 1),
    ("session.next.text.ended", 1),
    ("session.next.tool.input.started", 1),
    ("session.next.tool.input.ended", 1),
    ("session.next.tool.called", 1),
    ("session.next.tool.progress", 1),
    ("session.next.tool.success", 1),
    ("session.next.tool.failed", 1),
    ("session.next.reasoning.started", 1),
    ("session.next.reasoning.ended", 1),
    ("session.next.retried", 1),
    ("session.next.compaction.started", 1),
    ("session.next.compaction.ended", 1),
    ("session.next.revert.staged", 1),
    ("session.next.revert.cleared", 1),
    ("session.next.revert.committed", 1),
];

/// La cle de stockage d'un evenement durable, sous la forme `type.version`.
///
/// C'est `Event.versionedType` d'origine. Le type reste intact, seule la
/// version change, donc un evenement durable est toujours reconnaissable a son
/// nom.
pub fn versioned_type(ty: &str, version: u64) -> String {
    format!("{}.{}", ty, version)
}

/// Dit si un nom d'evenement figure dans le manifeste public.
pub fn is_public(ty: &str) -> bool {
    DEFINITIONS.contains(&ty)
}

/// Dit si un nom d'evenement figure dans le manifeste du serveur.
pub fn is_server(ty: &str) -> bool {
    SERVER_DEFINITIONS.contains(&ty)
}

/// La version d'un evenement durable, ou `None` s'il ne l'est pas.
///
/// Un evenement qui existe mais n'est pas durable donne `None` : l'absence
/// d'information ne se confond pas avec un nom inconnu.
pub fn durable_version(ty: &str) -> Option<u64> {
    DURABLE_DEFINITIONS
        .iter()
        .find(|(name, _)| *name == ty)
        .map(|(_, version)| *version)
}

/// L'inventaire durable sous forme de table, cle `type.version`.
///
/// C'est `Event.durable` d'origine : les evenements non durables sont ignores,
/// et deux entrees de meme cle feraient echouer la construction en amont.
pub fn durable() -> BTreeMap<String, &'static str> {
    DURABLE_DEFINITIONS
        .iter()
        .map(|(name, version)| (versioned_type(name, *version), *name))
        .collect()
}

/// L'inventaire `latest` sous forme de table : nom d'evenement vers sa position
/// dans le manifeste.
///
/// C'est `Event.latest` d'origine, qui ne garde que la version la plus haute
/// d'un type donne et refuse les doublons. Ici aucune clef n'apparait deux
/// fois, donc la table a exactement autant d'entrees que le tableau de depart.
pub fn latest() -> BTreeMap<&'static str, usize> {
    DEFINITIONS
        .iter()
        .enumerate()
        .map(|(index, ty)| (*ty, index))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_manifeste_public_contient_toutes_les_entrees_attendues() {
        assert_eq!(DEFINITIONS.len(), 88);
        assert_eq!(latest().len(), 88);
    }

    #[test]
    fn une_cle_tiree_au_hasard_du_manifeste_public_est_reconnue() {
        // Echantillon volontairement reparti sur tout le tableau, pour attraper
        // une faute de recopie qui ne se verrait pas sur la longueur.
        assert_eq!(DEFINITIONS[0], "models-dev.refreshed");
        assert_eq!(DEFINITIONS[4], "session.created");
        assert_eq!(DEFINITIONS[10], "message.part.removed");
        assert_eq!(DEFINITIONS[21], "session.next.step.ended");
        assert_eq!(DEFINITIONS[40], "session.next.revert.staged");
        assert_eq!(DEFINITIONS[43], "message.part.delta");
        assert_eq!(DEFINITIONS[44], "session.diff");
        assert_eq!(DEFINITIONS[45], "session.error");
        assert_eq!(DEFINITIONS[47], "installation.update-available");
        assert_eq!(DEFINITIONS[58], "pty.deleted");
        assert_eq!(DEFINITIONS[62], "todo.updated");
        assert_eq!(DEFINITIONS[65], "permission.replied");
        assert_eq!(DEFINITIONS[72], "command.executed");
        assert_eq!(DEFINITIONS[79], "session.compacted");
        assert_eq!(DEFINITIONS[87], "global.disposed");
    }

    #[test]
    fn le_manifeste_du_serveur_commence_par_le_meme_bloc_que_le_manifeste_public() {
        assert_eq!(SERVER_DEFINITIONS.len(), 58);
        let table = latest();
        for ty in SERVER_DEFINITIONS {
            assert!(is_public(ty), "{} absent du manifeste public", ty);
            assert!(table.contains_key(ty));
        }
        for (index, ty) in SERVER_DEFINITIONS.iter().enumerate().take(43) {
            assert_eq!(DEFINITIONS[index], *ty, "ordre different a la position {}", index);
        }
    }

    #[test]
    fn le_manifeste_public_contient_des_evenements_que_le_serveur_ne_publie_pas() {
        assert!(is_public("session.status"));
        assert!(!is_server("session.status"));
        assert!(!is_server("lsp.updated"));
        assert!(is_public("mcp.browser.open.failed"));
    }

    #[test]
    fn un_nom_inconnu_n_appartient_a_aucun_manifeste() {
        assert!(!is_public("ide.installed"));
        assert!(!is_server("ide.installed"));
        assert!(!latest().contains_key("session.inexistant"));
        assert_eq!(durable_version("session.inexistant"), None);
    }

    #[test]
    fn un_evenement_non_durable_present_dans_le_manifeste_n_a_pas_de_version() {
        assert!(is_public("message.part.delta"));
        assert_eq!(durable_version("message.part.delta"), None);
        assert!(!durable().contains_key("message.part.delta.1"));
    }

    #[test]
    fn la_table_durable_a_une_cle_par_evenement() {
        assert_eq!(DURABLE_DEFINITIONS.len(), 35);
        assert_eq!(durable().len(), 35);
    }

    #[test]
    fn la_cle_durable_porte_le_type_puis_la_version() {
        assert_eq!(versioned_type("session.created", 1), "session.created.1");
        assert_eq!(
            versioned_type("session.next.step.ended", 2),
            "session.next.step.ended.2"
        );
        let table = durable();
        assert_eq!(table.get("session.next.step.ended.2"), Some(&"session.next.step.ended"));
        assert!(!table.contains_key("session.next.step.ended.1"));
        assert_eq!(table.get("session.next.step.failed.2"), Some(&"session.next.step.failed"));
    }

    #[test]
    fn la_version_par_defaut_des_evenements_durables_est_un() {
        assert_eq!(durable_version("session.next.agent.switched"), Some(1));
        assert_eq!(durable_version("session.next.revert.committed"), Some(1));
        // Seuls les evenements de fin de tour sont en version 2.
        assert_eq!(durable_version("session.next.step.ended"), Some(2));
        assert_eq!(durable_version("session.next.step.failed"), Some(2));
    }

    #[test]
    fn une_liste_vide_ne_donne_aucune_entree_dans_la_table_latest() {
        let vide: [&str; 0] = [];
        let table: BTreeMap<&str, usize> = vide.iter().enumerate().map(|(i, t)| (*t, i)).collect();
        assert!(table.is_empty());
        assert!(!table.contains_key("session.created"));
    }
}
