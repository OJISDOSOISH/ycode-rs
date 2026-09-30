# review-array (directe, hors verificateurs)

Source : opencode/packages/core/src/util/array.ts (10 lignes : findLast).
Cible : src/swarm/util_array.rs. Rapport : aucun (revue directe du code).

## VERDICT : APPROUVE

## Points verifies OK
1. Ordre : parcours inverse depuis la fin, dernier match gagne. Conforme
   findLast (permissions/policy en dependent : ordre critique).
2. Underflow usize evite : `(0..len).rev()` au lieu de `len()-1`. Vide OK.
3. Retourne l'ELEMENT, pas sa veracite : piege falsy (0/""/false) documente
   et teste. Predicat recoit (element, index, liste) comme le TS.
4. FnMut : sur-ensemble de Fn, aucun appelant exclu. Bon choix.
5. Doctest : chemins `ycode::swarm::util_array` coherents avec Cargo.toml
   (name=ycode) + mod declares. Types de la closure verifies a l'oeil.

## Nits non bloquants
- Compilation reelle : CI ou check local a venir.
