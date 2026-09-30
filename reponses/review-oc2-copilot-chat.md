# review-oc2-copilot-chat (directe)

Source : openai-compatible-chat-language-model.ts (815 lignes). Cible :
rapport swarm-oc2-copilot-chat.md (951 lignes, 8 tests).

## VERDICT : APPROUVE

## Points verifies OK
1. Finish mapping : stop/length/tool_calls+function_call/content_filter/
   other. COHERENT avec le module copilot_finish_reason deja approuve
   (content-filter avec tiret). Le doute du POINT FAIBLE est leve par
   cette coherence croisee.
2. ? vs ?? : vide ignore (length>0) vs null garde, teste.
3. Renames exacts (reasoningOpaque, toolCallId, toolName), untagged
   morceau/erreur, accumulateurs purs (usage last-wins sauf details,
   appels index). Testes.
4. i64 compteurs : accepte (nombres JSON entiers en pratique sur ces
   champs ; meme decision que model.rs).
5. ID_OUTIL_REPLI deterministe : documente comme different de generateId,
   visible, a recaler a l'integration.
6. HTTP/SSE/prepareTools non portes, listes. Perimetre tenu.

## Nits non bloquants
- isParsableJson approxime par serde_json (strictness peut differer sur
  cas limites) : documente.
- Compilation reelle : CI.
