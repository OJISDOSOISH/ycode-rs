# review-util_identifier

Source reelle : `opencode/packages/core/src/util/identifier.ts` (1 ligne de reexport) -> `opencode/packages/schema/src/identifier.ts` (30 lignes, logique reelle).
Cible : `ycode-rs/src/swarm/util_identifier.rs` (277 lignes, 6 tests).
Rapport : `ycode-rs/reponses/swarm-util_identifier.md` (present, confiance moyenne).
Charte : `ycode-rs/src/core/session/compaction.rs` (lue, style sans accents respecte dans la cible).

## VERDICT : APPROUVE

Pas de BUG bloquant. La semantique est fidelement portee. L ecart crypto -> SplitMix64 est declare dans le rapport et dans le code, et acceptable comme dette temporaire vu l interdiction de toucher `Cargo.toml`.

## Points verifies OK

1. Longueurs : TS `length = 26`, temps 6 octets -> 12 hex, random `length - 12` = 14. Rust `LENGTH = 26`, `TIME_LENGTH = 12`, `RANDOM_LENGTH = 14` (lignes 40-46). OK.
2. Alphabet : TS `"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"` (schema/identifier.ts:2). Rust `CHARS` ligne 49 identique, ordre exact. Test ligne 235-237 verifie le mapping `% 62` y compris le biais (62 -> '0', 63 -> '1', 255 -> '7'). OK.
3. Formule temps : TS `BigInt(timestamp) * 0x1000n + BigInt(counter)` (ligne 21). Rust `(timestamp as i128) * BASE + counter as i128` avec `BASE = 0x1000` (lignes 52, 136). Compteur dans les 12 bits faibles. OK.
4. Compteur global : TS `lastTimestamp = 0, counter = 0`, reset si `timestamp !== lastTimestamp` puis `counter++` (lignes 3-4, 15-19). Rust `State { last_timestamp: 0, counter: 0 }`, meme test `!=` puis `+= 1` (lignes 57-67, 116-120). Premier appel avec timestamp 0 donne compteur 1 des deux cotes. Tests lignes 243-267 couvrent increment same-ms (`01` -> `02`) et reset sur ms suivante. OK.
5. Descending / BigInt negatif : TS `descending ? ~current : current` puis `>> BigInt(40 - 8 * index)) & 0xffn` + `toString(16).padStart(2, "0")` (lignes 22-27). Rust `!current` sur `i128` puis `>> shift & 0xff` + `format!("{:02x}")` (lignes 139, 146-147). Pour les valeurs en jeu (timestamp ~2^41, shift max 40) l extension de signe i128 128 bits coincide avec le BigInt infini. Test ligne 226 `ffffffffeffe` pour timestamp 1 / compteur 1 valide exactement ce piege. OK.
6. Hex minuscule, zero-pad 2 : `toString(16).padStart(2,"0")` vs `{:02x}`. Test ligne 217 `000000001001` (1*4096+1 = 0x1001) confirme ordre big-endian des 6 octets et pad. OK.
7. Random : TS `crypto.getRandomValues(new Uint8Array(14))` puis `chars[byte % 62]` (lignes 28-29). Rust genere 14 octets puis `CHARS[(octet % 62)]` (lignes 121-124, 149-151). Biais modulo conserve volontairement. OK fonctionnellement.
8. Concurrence : TS mono-thread. Rust `Mutex<State>` avec increment + tirage sous verrou (lignes 114-126), `poisoned -> into_inner` (lignes 72-77). Deux threads meme ms obtiennent des compteurs distincts. Changement legitime, pas de regression. OK.
9. JSON / renames : aucune struct/enum serialisee, aucun `serde(rename)` en jeu. Rien a verifier. OK.
10. Types : `Date.now()` (f64 entier) -> `now_millis()` via `as_millis() as i64` (lignes 80-85). Plage actuelle ~1.7e12 tient dans i64. Calcul en i128 sans overflow. `as_millis` renvoie u128, cast OK pour les dates reelles. OK.
11. UTF-16 / insertion order : alphabet 100% ASCII, pas de `length` JS piege, pas d objet. N/A.
12. API : `ascending()`, `descending()`, `create(descending_order)` + helper additif `create_at(desc, ts)` pour tests (lignes 89-128). Renommage param `descending` -> `descending_order` impose par le masquage de la fonction `descending()` en Rust, sans effet pour les appelants positionnels. `mod.rs` ligne 54 declare bien `pub mod util_identifier;`. OK.
13. Tests pieges couverts (6/6) : format ascendant exact, complement descendant exact, alphabet+biais, increment same-ms, reset nouvelle ms, format via horloge reelle. Verrou `VERROU` anti-flakiness parallele (lignes 200-207). OK.

## Nits non bloquants

1. Hasard non cryptographique : SplitMix64 + amorce `RandomState` (lignes 159-191) remplace `crypto.getRandomValues`. Unicite preservee, imprevisibilite non garantie (bits faibles de SplitMix + `& 0xff`). A remplacer par `getrandom`/`rand` des que `Cargo.toml` l autorise. Suivi recommande si ces IDs servent de secrets.
2. Sentinelle `seed == 0` = "non amorce" (ligne 187) : collision si `initial_seed()` ou le compteur retombe exactement a 0 (proba 2^-64, negligeable) -> re-amorcage silencieux au lieu de continuer la sequence. Preparer un flag `seeded: bool` lors d une retouche.
3. `now_millis()` renvoie 0 si `SystemTime` avant epoch (ligne 83) ; `Date.now()` renverrait un negatif. Cas hors realite, pas de test attendu.
4. Troncature implicite a 48 bits bas (6 octets) en cas de timestamp geant identique des deux cotes, non testee. Comportement commun, pas d action.
