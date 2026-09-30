# review-oc2-websearch (directe, le relecteur apparie avait fini en EN ATTENTE)

Source : tool/websearch.ts (via revue appariee). Cible : rapport
swarm-oc2-websearch.md (568 lignes, 7 tests).

## VERDICT : APPROUVE

## Points verifies OK
1. parseInt(_,36)%2 reproduit : trim, signe, casse indifferente, arret au
   premier invalide, NaN->Parallel. Tests parite ("0"->exa, "1"/"!!!"->parallel).
2. Priorites selectProvider (surcharge > parallel > exa > checksum). Testees.
3. SSE : prefixe exact "data: " (6 octets), premier non-vide gagne, direct
   d'abord. Falsy "" ignore, ?? survit. Testes les deux cotes.
4. Renames exacts (numResults, contextMaxCharacters, type, exaApiKey...),
   roundtrip + absence des snake. Testes.
5. exa_url falsy ""->base, cle non encodee : divergence DECLAREE (URL.searchParams).
6. Reseau/effects non portes, listes. Perimetre tenu.

## Nits non bloquants
- Echec decodage -> None total sans repli SSE quand corps commence par "{"
  (documente ; cas JSON-valide-sans-texte bien replie, lui).
- Compilation reelle : CI.
