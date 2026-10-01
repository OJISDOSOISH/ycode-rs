# review-util_token

Source TS : opencode/packages/core/src/util/token.ts (5 lignes)
Cible Rust : ycode-rs/src/swarm/util_token.rs (137 lignes)
Rapport : ycode-rs/reponses/swarm-util_token.md
Charte : ycode-rs/src/core/session/compaction.rs (lu pour contexte, hors scope)

## VERDICT : APPROUVE

Pas de BUG bloquant. La semantique TS est reproduite exactement pour toute entree UTF-8 valide.

## Points verifies OK

1. Reexport ligne 1 TS `export * as Token from "./token"` : reexport de soi-meme, aucun autre fichier token sous util/. Pas de traduction en Rust, documente en tete de util_token.rs lignes 19-27. OK. Conforme au rapport.
2. Constante `CHARS_PER_TOKEN = 4` TS ligne 3 non exportee -> `const CHARS_PER_TOKEN: f64 = 4.0` privee Rust ligne 46. Privee des deux cotes, division flottante identique. OK.
3. Piege UTF-16 length : TS ligne 5 `input.length` = unites UTF-16. Rust ligne 64 `input.encode_utf16().count()` = equivalent exact. `str::len()` (octets) et `chars().count()` (points de code) auraient ete faux. Choix correct, documente lignes 29-40 et 52-53. Tests lignes 110-136 pinnent le piege : 4x U+00E9 -> 1 jeton (8 octets mais 4 unites), 4x U+1F600 -> 2 jetons (4 points de code mais 8 unites), mixte 5 + 3 + 2x2 = 12 -> 3 jetons. OK.
4. Operateur `Math.round` TS ligne 5 vs `f64::round` Rust ligne 65 : identiques sur valeurs >= 0 (moities 0.5/1.5/2.5 montent). Negatifs impossibles car longueur >= 0. Tests lignes 94-107 pinnent : len 2 -> 1, len 3 -> 1, len 6 -> 2, len 10 -> 3, len 4 -> 1, len 8 -> 2. Table conforme a `Math.round(len/4)`. OK.
5. `Math.max(0, ...)` TS ligne 5 reporte ligne 65 `.max(0.0)` : defensif des deux cotes, jamais actif. Conserve a l'identique, documente. OK.
6. Types : TS `number` >= 0 -> Rust `i64` ligne 63. Justifie lignes 59-62 : appelants session/compaction.ts lignes 190 (`Token.estimate(summaryPrompt) > context - summaryOutput`, context - summaryOutput peut etre negatif en JS) et 238 (`estimate(...) <= context - Math.max(...)`) font des soustractions signees. `i64` evite un underflow/saturation qu'un `u64`/`usize` introduirait. Aucun appelant n'attend `f64`. Conversion `longueur as f64` puis `as i64` apres `round` : exacte aux tailles realistes, saturation Rust definie au-dela de i64::MAX. OK.
7. Champs JSON / renames / variantes / insertion order / truthiness / `?` vs `??` : sans objet, aucune struct/enum/serialisation/optionnel dans ce fichier. Aucun `serde` pose, a juste titre. OK.
8. Point A VERIFIER du rapport leve : `pub mod util_token;` present dans src/swarm/mod.rs ligne 56. OK.
9. Fichier 100 % ASCII verifie (0 octet > 127), echappements `\u{00e9}` et `\u{1F600}` dans les tests, pas de litteraux non-ASCII. OK.
10. Charte compaction.rs `estimate_tokens` (chars().count + ceil) differe de ce module (UTF-16 + round) : hors scope, pas de conflit. Ce module est le seul portage fidele de util/token.ts. OK.

## Nits non bloquants

- Rapport dit "Non compile : aucun toolchain" : syntaxe relue OK (`encode_utf16().count()`, `round().max()`, cast `as i64`), rien a signaler.
- Precision `as f64` au-dela de 2^53 : perte theorique pour chaines > 9 Po, sans impact pratique.
- `CHARS_PER_TOKEN: f64` au lieu d'entier : OK fonctionnellement, alternative `const CHARS_PER_TOKEN: usize` + cast a l'usage serait aussi valide.
