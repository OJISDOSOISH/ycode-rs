# Sous-taches de portage

Chaque tache est autonome : un fichier TS -> un fichier Rust equivalent, avec tests.
Regle absolue : ne jamais modifier un fichier deja porte, sauf pour corriger un bug
signale. Chaque agent travaille sur son propre fichier.

## FAIT
- schema/session-message.ts -> schema/session_message.rs
- core/session/history.ts -> core/session/history.rs
- core/session/context-epoch.ts -> core/session/context_epoch.rs
- core/session/compaction.ts -> core/session/compaction.rs
- core/session/revert.ts -> core/session/revert.rs
- core/session/runner/llm.ts -> core/session/turn.rs
- core/fs-util.ts -> tool.rs (partiel)
- packages/llm (client) -> llm.rs (partiel)

## A PORTER (par ordre de valeur)
1. core/session/compaction/serialize.ts -> serialisation du contexte pour le resume
2. core/session/compaction/summary.ts -> redaction du resume
3. core/session/compaction/prune.ts -> elagage des vieux messages
4. core/session/compaction/select.ts -> selection des messages a compacter
5. core/session/todo.ts -> liste de taches de la session
6. core/session/compaction/estimate.ts -> estimation de tokens
7. core/session/context-epoch.ts (suite) -> remplacement de snapshot
8. core/permission.ts -> systeme de permissions
9. core/question.ts -> questions posees a l utilisateur
10. core/policy.ts -> politique d execution
