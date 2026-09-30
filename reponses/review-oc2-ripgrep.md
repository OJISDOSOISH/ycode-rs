# review-oc2-ripgrep (directe, le relecteur apparie avait fini en EN ATTENTE)

Source : ripgrep.ts (284 lignes, lue via revue appariee). Cible : rapport
swarm-oc2-ripgrep.md (541 lignes, 8 tests).

## VERDICT : APPROUVE

## Points verifies OK
1. Constantes exactes (ERROR_BYTES, MAX_RECORD_BYTES, MAX_SUBMATCHES,
   MATCH_TEXT_MAX_CHARS=2000, GIT_EXCLUDE_GLOB, ENTRY_FILE).
2. Schemas bruts --json fideles (TextWrapper, RawSubmatch match/start/end,
   RawMatchData line_number/absolute_offset, RawMatch type/data).
3. Erreurs taggees _tag avec renames exacts (Ripgrep.Error,
   Ripgrep.InvalidPatternError). Teste.
4. decide_run : ordre exact (troncature > motif invalide > echec > 1 > 2
   partiel). Teste cas par cas.
5. UTF-16 (encode_utf16, surrogate documente no-op), byteLength=as_bytes,
   strip/normalize regex repliquees, args builders exacts (find * sans glob,
   grep file ?? "."). Testes.
6. Spawn/effects non portes, signales. Perimetre respecte.

## Nits non bloquants
- cause: Option<String> au lieu du Defect opaque (documente, structure gardee).
- Compilation reelle : CI en cours.
