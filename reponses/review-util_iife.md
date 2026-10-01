# review-util_iife

Source TS : opencode/packages/core/src/util/iife.ts (3 lignes, fonction unique)
Cible Rust : ycode-rs/src/swarm/util_iife.rs (105 lignes, dont 65 de tests)
Rapport : ycode-rs/reponses/swarm-util_iife.md
Charte relue : ycode-rs/src/core/session/compaction.rs

## VERDICT : APPROUVE

Portage fidele et complet. Aucun BUG. Aucune divergence fonctionnelle.

## Points verifies OK

1. Semantique exacte : TS `iife.ts:1-3` (`export function iife<T>(fn: () => T) { return fn() }`) -> Rust `util_iife.rs:33-38` (`pub fn iife<T, F>(f: F) -> T where F: FnOnce() -> T { f() }`). Appel immediat, retour direct, zero logique ajoutee ou perdue.
2. Generique preserve : `T` reste generique des deux cotes. Ajout de `F: FnOnce()` cote Rust impose par le systeme de types, sans changer la semantique. `FnOnce` est le bon choix (accepte `Fn`, `FnMut`, `FnOnce` et les closures `move`), la ou `Fn` seul aurait ete trop restrictif.
3. Rename `fn` -> `f` : impose car `fn` est un mot cle reserve en Rust. Documente en `util_iife.rs:27-28`. OK.
4. JSON / serde : neant des deux cotes. Aucun struct, aucun enum, aucun champ. Donc aucun `serde(rename = ...)` a verifier. Le rapport le dit explicitement, c est exact.
5. Variantes perdues : neant. Aucun union type, aucun enum, aucun overload TS.
6. Pieges operateurs JS : neant sur cette source. Pas de `?` / `??` / `?.`, pas de test de truthiness, pas de `length` / UTF-16, pas d ordre d insertion, pas d egalite `==` vs `===`, pas d arithmetique flottante. Rien a porter, rien a tester.
7. Types numeriques f64 / i64 / u64 : neant. Aucun nombre dans la source.
8. Propagation d erreur : TS propage l exception de `fn()` ; Rust propage le panic de `f()`. Comportement equivalent. Test `util_iife.rs:81-83` (`Err` renvoye tel quel) couvre le cas valeur d erreur sans panic.
9. Cas async : TS `iife(async () => ...)` renvoie une `Promise` ; Rust `iife(|| async { ... })` renvoie la `Future` telle quelle (doc `util_iife.rs:24-25`, test `util_iife.rs:86-104`). Equivalent direct, sans `await` ajoute. OK.
10. Effet unique : test `util_iife.rs:50-59` verifie zero execution avant l appel et exactement une execution apres. Couvre la semantique "immediately invoked".
11. Cas `move` : test `util_iife.rs:74-78` verifie la capture par deplacement, point sensible Rust sans equivalent JS. OK.
12. Compilation du test async : `use std::future::Future`, `Wake`, `Waker`, `Context::from_waker`, `Box::pin(...).as_mut().poll(...)` : imports presents, API stable, pas d appel nocturne. Le doute emis dans le rapport sur l absence de compilateur local est honnete mais le code utilise uniquement du `std` stable et doit compiler.
13. Declaration module : `pub mod util_iife;` present dans `src/swarm/mod.rs:53`. Le rapport annonce "ligne 26", numero faux (26 = `provider_cerebras`), mais le fond est vrai : rien a ajouter, fichier bien rattache.

## Nits non bloquants

- Rapport : numero de ligne `mod.rs` inexact (26 au lieu de 53). Corriger le chiffre si le rapport est regenere, sans impact sur le code.
- Rapport : mention d un `mod.rs.tmp` de 491 octets non rattache au portage. A nettoyer par le proprietaire du lot, hors perimetre de cette revue.
- Style doc : la doc Rust explique IIFE et la portee de bloc (lignes 11-19). Utile mais plus longue que la source (3 lignes). Acceptable au regard de la charte `compaction.rs` lue en reference (docs riches en francais), pas de demande de reduction.
- Test `util_iife.rs:68-71` (liste vide) et `62-65` (unit `()`) se recouvrent partiellement ; redondance inoffensive, on garde.
