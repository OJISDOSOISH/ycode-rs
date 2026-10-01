# Review config_compaction - TS config/compaction.ts vs Rust swarm/config_compaction.rs

Objet : `opencode/packages/core/src/config/compaction.ts` (15 lignes, pur schema) vs `ycode-rs/src/swarm/config_compaction.rs` (338 lignes).
Rapport swarm : absent (`ycode-rs/reponses/swarm-config_compaction.md` n existe pas). Code present et revu directement.
Charte relue : `ycode-rs/src/core/session/compaction.rs` (171 lignes, hors perimetre, utilisee seulement pour les valeurs par defaut).
Tests non executes ici (cargo absent sur cette machine) : relecture statique + verification des sources TS croisees.

## VERDICT : APPROUVE

Aucun bug bloquant. Schema fidele, piege `??` vs veracite correctement traite, tests couvrant les pieges.

## Points verifies OK

1. Champs JSON exacts, sans rename piegeux.
   - TS `config/compaction.ts` lignes 6-15 : `Keep { tokens }`, `Info { auto, prune, keep, buffer }`, tous en minuscules simples.
   - Rust lignes 118, 152, 156, 160, 164 : `#[serde(rename = "...")]` explicites, identiques au TS. Aucun `ID`/`Id`/camelCase dans ce fichier, rien a perdre.
   - Test lignes 224-241 : les 5 noms serialises, tailles d objets exactes (4 et 1, aucun champ en trop). Test roundtrip lignes 328-337.

2. Caractere optionnel de chaque champ.
   - TS : `Schema.optional` sur les 5 champs (lignes 7, 11-14).
   - Rust : `Option<...>` + `default` + `skip_serializing_if = "Option::is_none"` partout. `Info::default()` donne tout a `None`, serialise en `"{}"` (test lignes 211-221). `Keep::default()` serialise en `"{}"` (test lignes 243-248). Conforme : `Schema.optional` retire la cle.

3. `NonNegativeInt` : traduction exacte.
   - Source : `packages/schema/src/schema.ts` ligne 4 = `Schema.Int.check(Schema.isGreaterThanOrEqualTo(0))`, re-exporte par `packages/core/src/schema.ts` ligne 12, utilise en `config/compaction.ts` lignes 7 et 14.
   - Rust ligne 106 : `pub type NonNegativeInt = u64`. Le non-signe refuse les negatifs et accepte `0`, exactement comme le `check(>= 0)`. Pas de `est_positif` recopie a tort (le doc lignes 47-53 le signale explicitement).
   - Tests lignes 289-295 (negatifs refuses), 297-303 (flottants et chaines refuses).

4. Piege `?` contre `??` (le point central du fichier) : correct.
   - Consommateur TS `session/compaction.ts` lignes 129-131 : `current.auto ?? result.auto`, `current.buffer ?? result.buffer`, `current.keep?.tokens ?? result.tokens`. Coalescents : seul `null`/`undefined` declenche le defaut, `0` et `false` survivent.
   - Rust : `tokens_ou` (lignes 133-139), `auto_ou` (175-180), `buffer_ou` (186-191), `Info::tokens_ou` avec `?.` equivalent (lignes 199-204, `match &self.keep`). `match Some/None`, jamais de test de veracite. `Some(0)` et `Some(false)` renvoies tels quels.
   - Tests : zero explicite conserve a la lecture et a l ecriture (lignes 251-259), zero ne donne pas le defaut (lignes 261-267), `keep: {}` redonne le defaut comme `keep` absent (lignes 269-277, equivalent du `?.` qui court-circuite), `auto: false` honore face au defaut `true` (lignes 280-287).

5. Defauts non figes dans la config : correct.
   - TS : `config/compaction.ts` est purement declaratif, les defauts vivent dans `session/compaction.ts` lignes 12-13 (`DEFAULT_BUFFER = 20_000`, `DEFAULT_KEEP_TOKENS = 8_000`, `auto: true` ligne 133).
   - Rust : defauts passes en parametre (`defaut`), jamais ecrits en dur. Test lignes 305-317 le prouve (`tokens_ou(0) == 0` avec une section vide).
   - Valeurs citees en doc (20_000 / 8_000, lignes 77-78) conformes a la source TS.

6. `prune` porte sans comportement invente : correct.
   - `Settings` de `session/compaction.ts` lignes 62-66 ne contient que `auto`/`buffer`/`tokens`. `prune` n est recopie que dans `v1/config/migrate.ts` lignes 54-61. Le Rust le declare (ligne 156) sans lui donner de methode `_ou`, conforme au doc lignes 80-89.

7. Proprietes inconnues ignorees : correct.
   - Pas de `deny_unknown_fields`, test lignes 319-325. Conforme a `Schema.Class` sans controle d exces.

8. Integration : `pub mod config_compaction;` present en `src/swarm/mod.rs` ligne 16.

## Nits non bloquants

1. `null` explicite accepte comme `None` cote Rust (ex. `{"auto": null}` -> `None`), alors que `Schema.optional` TS sans `NullOr` refuse `null` (seuls manque/`undefined` passent). Tolerance supplementaire sans impact sur des configs reelles. Laisser tel quel, ou durcir avec un `deserialize_with` si la stricte equivalence est visee.
2. `1.0` : `Schema.Int` TS l accepte (`Number.isInteger(1.0)` vrai) mais `u64` serde le refuse. Cas marginal des configs ecrites a la main, ne pas corriger sauf exigence de parite totale.
3. Pas de helper de fusion multi-documents : le `reduce` de `session/compaction.ts` lignes 123-135 (plusieurs `Config.Document`, les suivants ecrasent les precedents champ par champ via `??`) n a pas d equivalent `fusionner` cote Rust. Les accesseurs sont mono-`Info`, le fold reste a l appelant. Hors perimetre de ce fichier schema, a documenter cote appelant quand il sera porte.
4. Coquilles de doc : ligne 156 `lesvieilles` (espace manquante), ligne 127 chemin `src/core/session::compaction` (melange `/` et `::`). Cosmetique.
5. Compilation et `cargo test` non lances ici (toolchain absente) : a faire tourner par le main avant merge.
