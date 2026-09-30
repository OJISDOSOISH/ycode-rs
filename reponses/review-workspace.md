# review-workspace (directe, hors verificateurs)

Source : workspace.ts (re-export) + schema/workspace-id.ts (19 lignes) +
schema/identifier.ts (branche ascending). Cible :
src/swarm/core_workspace.rs (486 lignes).

## VERDICT : APPROUVE, avec 1 point d'integration important ci-dessous

## Points verifies OK
1. ascendant : None->create, Some("")->create, Some(valide)->reuse,
   Some(invalide)->erreur. Fidele a `if (!id)` (veracite). Teste.
2. Validation isStartsWith("wrk") sans exiger le tiret : fidele, teste
   ("wrk" seul accepte, "Wwrk" refuse car casse significative).
3. try_from[String] au lieu de deserialize_with sur container : idiome
   correct signale par l'agent lui-meme. JSON transparent (chaine nue).
4. Mutex + compteur/ms comme identifier.ts, poison gere. Ordre
   lexicographique teste.
5. ascendant vs fromRow : deux call-sites, deux destins pour "" (create vs
   None), les deux fideles. Pas de contradiction.

## IMPORTANT pour l'integration (entropie)
La queue aleatoire (14 chars) vient de SplitMix64 amorce horloge+compteur :
PREVISIBLE. Mais attention : remplacer par un simple new_v4() CASSERAIT le
test d'ordre (379-385), car les v4 ne sont pas ordonnes. Resolution correcte :
garder la structure timestamp+compteur (12 hex) et ne prendre de l'OS
(new_v4/getrandom) que les octets de la queue aleatoire mappes %62.
Ni SplitMix64 maison, ni v4 brut : hybride.

## Nits non bloquants
- L375 : "identifiantsont" (espace manquante, message d'assert seulement).
- Compilation reelle : CI.
