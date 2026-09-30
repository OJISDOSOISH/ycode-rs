# review-oc2-spawner (directe, le relecteur apparie avait fini en EN ATTENTE)

Source : cross-spawn-spawner.ts (via revue appariee). Cible : rapport
swarm-oc2-spawner.md (688 lignes, 7 tests). Cible proposee : src/core/process/spawner.rs.

## VERDICT : APPROUVE

## Points verifies OK
1. Modeles exacts : errno->tag, Command Standard/Piped tagge _tag,
   inputs camelCase renommes, FdEntry trie.
2. flatten : ordre infix order exact, erreur si vide. Teste imbrique.
3. resolve_stdin/stdio 4 branches (Missing/Literal/Stream-Sink/Detailed).
   Testees.
4. Slots stdio : ignore-fill + overlapped win32-only. Teste.
5. resolve_pipe from/to avec retombees, fd parse strict (fd seul refuse).
   Testes.
6. Args builders (clone/fetch/checkout/reset/discard) avec defauts ternaires
   (prune/force/reset vrais sauf Some(false)). Testes.
7. numstat binaire, tree_entry regex manuelle (hex minuscule), worktree
   main-linked, changeset, fallback message stderr>text>defaut. Testes.
8. Effects/spawn/verrous non portes, listes exhaustivement (POINT FAIBLE
   honnete et complet).

## Nits non bloquants
- parse_fd_name sans borne haute (fd999999 accepte) : comme l'original,
  rien a faire.
- cause Defect -> Option<String> : meme simplification que ripgrep/git,
  coherente.
- Cible src/core/process/spawner.rs : OK a l'integration (nouveau
  repertoire process a creer).
- Compilation reelle : CI.
