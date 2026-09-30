# Swarm OpenCode : 7 ecrivains + 13 verificateurs

## 7 ecrivains (owner opencode-swarm, rapports swarm-oc-*.md)
open.ts, provider.ts, model.ts, global.ts, command.ts, image.ts, credential.ts.
Petits fichiers core non reclames, hors briefs b00-b19, hors perimetre Kilo.
Integration en src/ par moi apres relecture. Ils ne touchent a rien d'autre.

## 13 verificateurs (fichiers review-*.md)
Couvrent 13 des 21 rapports swarm Kilo : util_identifier, util_lazy, util_iife,
util_token, config_command, config_compaction, config_formatter, config_plugin,
effect_memo_map, integration_connection, observability_shared, tool_tools,
copilot_finish_reason.
Je garde en direct les 5 prios (wildcard, hash, array, tool_output, file)
+ workspace, project_schema, pty_schema.
Si le rapport n'a pas atterri : pre-lecture + liste de pieges, puis re-passage.
