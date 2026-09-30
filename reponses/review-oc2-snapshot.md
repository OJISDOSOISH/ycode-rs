# review-oc2-snapshot (directe, le relecteur apparie avait fini en EN ATTENTE)

Source : snapshot.ts (10265 o). Cible : rapport swarm-oc2-snapshot.md (442
lignes, 7 tests).

## VERDICT : APPROUVE

## Points verifies OK
1. Scope/plan anti-echappement : scope_relative + plan_restore +
   contains_path forment un ensemble coherent (prefixe + trim), teste
   (egalite, dehors, ../).
2. wrap_error : meme-operation gardee, sinon re-emballee. Teste.
3. is_enabled : git + flag!=false, absent vaut actif. Teste les 5 cas.
4. checkout -> Restore : piege documente et teste.
5. LegacyFileDiff : forme legacy {file?,patch?,additions,deletions,status?},
   distincte du Diff vivant de core_file (path/status/additions u64) :
   DEUX formes coexistent dans le TS, les confondre serait faux. Le rapport
   demande le croisement avec file.ts : fait ici, pas de conflit (champs et
   types differents, usages differents).
6. resolve_diff_paths, filter_ignored, noop : fideles, testes.
7. FS/git non portes, listes. Perimetre tenu.

## Nits non bloquants
- join_and_normalize lexical (pas de realpath/symlinks) : assume et documente.
- Compilation reelle : CI.
