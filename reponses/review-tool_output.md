# review-tool_output (directe, hors verificateurs)

Source : opencode/packages/core/src/config/tool-output.ts (9 lignes).
Cible : src/swarm/config_tool_output.rs (182 lignes, 9 tests).

## VERDICT : APPROUVE

## Points verifies OK
1. Pas de defauts inventes : MAX_LINES/MAX_BYTES vivent dans
   tool-output-store.ts, non dupliques. Discipline source-de-verite OK.
2. PositiveInt -> u64 + `est_positif` const pour le cas 0 (seule divergence,
   documentee ET testee). Negatifs et flottants refuses a la lecture.
3. snake_case conserve, rename redondant volontaire (garde-fou), verrouille
   par test anti-maxLines/maxBytes.
4. Optionnels absents non serialises (test `{}`), `??` non applicable ici
   (pas de defauts dans ce fichier) : correct de ne rien inventer.
5. Champs inconnus ignores (pas de deny_unknown_fields), conforme
   Schema.Class. Test present.
6. Pas de confusion avec compaction.rs TOOL_OUTPUT_MAX_CHARS (caracteres vs
   lignes) : note anti-fusion presente.

## Nits non bloquants
- `skip_serializing_if` en chemin absolu (`std::option::Option::is_none`)
  sur 2 champs : fonctionne, style incoherent avec le reste, sans effet.
- Compilation reelle : CI ou check local a venir.
