# review-oc2-git (directe, le relecteur apparie avait fini en EN ATTENTE)

Source : git.ts (987 lignes). Cible : rapport swarm-oc2-git.md (904 lignes,
7 tests).

## VERDICT : APPROUVE

## Points verifies OK
1. Modeles exacts : Repository/Worktree (gitDirectory/commonDirectory),
   3 erreurs taggees Git.*, FileDiff miroir (chaines nues, pas de tag),
   inputs camelCase, DiscardIndex/Untracked.
2. Defauts ?? fideles (depth 100, origin, context 3, Some(0) survit) ;
   veracite || correcte (trim vide -> None).
3. Parsers : numstat binaire, tree_entry (hex minuscule comme la regex),
   worktree main-linked, roots tries, remoteHead prefixe.
4. Args builders : prune/force/reset vrais sauf Some(false) explicite.
5. resolve_path/windows_path : approximation DOCUMENTEE de path.resolve
   (pas de test plateforme). Divergence connue, pas d'erreur cachee.
6. Effects/process/verrous non portes, listes exhaustivement.

## Nits non bloquants
- cause Defect -> Option<String> (coherent avec ripgrep).
- Compilation reelle : CI.
