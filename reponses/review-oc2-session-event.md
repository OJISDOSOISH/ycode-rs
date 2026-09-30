# review-oc2-session-event (directe)

Source : schema/session-event.ts. Cible : rapport swarm-oc2-session-event.md
(980 lignes, 8 tests).

## VERDICT : APPROUVE

## Points verifies OK
1. Delta live-only exclus du Durable, teste dans les DEUX sens (accepte
   en All, refuse en Durable). Point critique verrouille.
2. Renames ID verrouilles en positif ET negatif (callID pas callId/call_ID,
   isRetryable/statusCode/outputPaths/assistantMessageID pareil).
3. EventSource u64 vs PromptSource f64 distingues. Finite=f64, millis=i64.
4. Enveloppe generique : factorisation assumee, risque documente.
5. Roundtrips complets sur 4 familles.

## Pour la passe C (liste à jour)
Ce fichier redefinit : SessionId, MessageId, EventId, RelativePath,
ModelRef, LocationRef. Tous en doublon avec schema existant. A unifier
dans src/schema/ (le ModelRef ici est deja canonique, simple re-export).
