# review-pty_schema (directe, hors verificateurs)

Source : pty/schema.ts (1 ligne) + schema/pty.ts + identifier.ts.
Cible : src/swarm/pty_schema.rs (381 lignes).

## VERDICT : APPROUVE, avec 1 incoherence signalee ci-dessous

## Points verifies OK
1. Piege === undefined correctement tranche : Some("") part en validation
   et echoue (inverse du cas workspace). Teste explicitement (345-351).
   Distinction par call-site exacte.
2. Prefixe isStartsWith("pty") sans exiger le tiret, casse significative.
   Teste.
3. transparent serde, Display/From, create/ascending fideles. Ticket ptyID
   camelCase verifie contre ticket.ts.
4. Upstream non porte -> def locale + futur pub use : coherent avec C.

## INCOHERENCE signalee (pas bloquante, a trancher)
La deserialisation ne valide PAS (derive transparent) alors que workspace
valide via try_from. Le TS refuserait `"xxx"` au decode des deux cotes.
Recommandation : aligner sur workspace (try_from) a l'integration. Fichier
Kilo : correction chez lui.

## Integration : queue aleatoire xorshift -> OS comme workspace, en gardant
la structure temporelle ordonnee.
