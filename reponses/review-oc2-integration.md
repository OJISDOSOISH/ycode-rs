# review-oc2-integration (directe)

Source : integration.ts (520 lignes) + schema. Cible : rapport
swarm-oc2-integration.md (946 lignes, 9 tests).

## VERDICT : APPROUVE

## Points verifies OK
1. Garde check_complete dans l'ordre TS (code d'abord, completing
   ensuite). Teste NotFound/Settled/AlreadyCompleting/CodeRequired/Ready.
2. Veracite env (vide ignoree), .toReversed() (recents d'abord), tri
   localeCompare approxime et DECLARE (ordre caracteres).
3. Cycle tentatives : completing auto d'entree, settle, scrub (expire +
   purge retention), refresh marge 5 min. Constantes temps exactes.
4. Tags exacts (type oauth/key/env, status pending/complete/failed/
   expired, _tag Integration.*, attemptID). Serialisation verrouillee.
5. method_matches (OAuth par id, Key/Env par type), id force non renommable.
   Testes.
6. Effects/callbacks/SQL non portes, listes. Suffixe AttemptID delegue
   (ascending non reproductible en pur) : honnete.

## Nits non bloquants
- Compilation reelle : CI.
