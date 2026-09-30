# review-oc2-copilot-resp (directe)

Source : openai-responses-language-model.ts (60 Ko). Cible : rapport
swarm-oc2-copilot-resp.md (1498 lignes, 8 tests).

## VERDICT : APPROUVE

## Points verifies OK
1. Schemas mecaniques fideles (items, logprobs, usage, messages, chunks
   tagges type, ElementTermine). Echantillons lus conformes.
2. ? vs ?? traite avec test dedie (falsy computer vs nullish titre).
3. Parties non portees HONNETEMENT listees : doGenerate/doStream/getArgs,
   TransformStream, fetch, convertToOpenAIResponsesInput,
   prepareResponsesTools, mapOpenAIResponseFinishReason, postJsonToApi.
   Pas de finish mapping reconstitue : le doute du resume est leve, la
   fonction est dans les sautes, pas dans le porte.
4. Divergence loose-fallback documentee (chunks inconnus echouent au lieu
   du loose-pass zod). Comportement different assume et dit.
5. statics (flex/priorite, includes) et gardes est_* portes en pur.

## Nits non bloquants
- Compteurs i64 vs number zod : accepte (nombres JSON non fractionnaires
  en pratique sur ces champs).
- id deterministe vs generateId : a recaler quand l'integration branchera
  un generateur (meme note que workspace/pty : garder l'ordre si pertinent).
- Compilation reelle : CI.
