# review-hash (directe, hors verificateurs)

Source : opencode/packages/core/src/util/hash.ts (11 lignes). Cible :
src/swarm/util_hash.rs (320 lignes). Rapport : aucun (revue directe du code).

## VERDICT : APPROUVE

## Points verifies OK
1. Constantes : IV SHA-1 (67452301...), IV SHA-256 (6a09e667...), table K
   64 entrees : conformes FIPS 180-4 de memoire.
2. Tours SHA-1 (Cho/Majority/parity + rotations 5/30) et SHA-256 (Sigma,
   Choice, Majority) : canoniques. Remplissage Merkle-Damgard partage : OK.
3. Vecteurs : vide/abc/56-octets reconnus (a9993e36, ba7816bf, 84983e44,
   248d6a61). Methode differentielle de l'agent (40 cas vs JS) : caution
   supplementaire credible.
4. `fast` = SHA-1 malgre le nom : documente (noms de lockfiles). Hex
   minuscule, entree &str en UTF-8 comme Node : OK.
5. Pas de crate sha1/sha2 au catalogue : implementation locale justifiee,
   point de substitution indique (sha1_digest/sha256_digest).

## Nits non bloquants
- Compilation reelle : CI ou check local a venir.
