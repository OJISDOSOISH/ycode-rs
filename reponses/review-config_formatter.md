# review-config_formatter

Source TS : opencode/packages/core/src/config/formatter.ts (12 lignes, lues en entier)
Cible Rust : ycode-rs/src/swarm/config_formatter.rs (303 lignes, lues en entier)
Rapport : ycode-rs/reponses/swarm-config_formatter.md (lu en entier)
Charte : ycode-rs/src/core/session/compaction.rs (lue, hors perimetre, style de place)

## VERDICT : APPROUVE

Pas de BUG bloquant. La semantique JSON du schema est preservee.

## Points verifies OK

1. Champs JSON exacts (TS lignes 5-10 vs Rust lignes 62-78) :
   - `disabled` -> Option<bool>, rename exact, skip_serializing_if OK.
   - `command` -> Option<Vec<String>>, rename exact. Le TS fait Schema.String.pipe(Schema.Array) donc liste de chaines, pas de split sur espaces. Le Rust ne decoupe pas, test ligne 184-199 OK.
   - `environment` -> Option<BTreeMap<String,String>>, rename exact. Record<string,string> bien traduit, pas de HashMap non deterministe.
   - `extensions` -> Option<Vec<String>>, rename exact.
   - Aucun camelCase dans la source, piege ID/Id/majuscules sans objet ici. Les quatre rename sont redondants mais inoffensifs, verrou anti-renommage assume.

2. Variantes union (TS ligne 12 vs Rust lignes 91-99) :
   - Info = Union([Boolean, Record(String, Entry)]) -> enum untagged Toggle(bool) puis Overrides(BTreeMap<String,Entry>).
   - Choix untagged correct, pas de tag discriminant. Ordre Toggle d abord conserve, meme ordre d essai que Schema.Union.
   - Table vide {} -> Overrides(vide) et pas Toggle, test lignes 217-224 OK. Bool false/true -> Toggle, tests lignes 202-214 et 249-259 OK.
   - Valeurs hors schema refusees : string, number, null, array, {prettier:3}, test lignes 287-302 OK.

3. Operateurs JS pieges :
   - Pas de `?` vs `??`, pas de ternaire, pas de truthiness, pas de length UTF-16, pas d arithmetique f64/i64/u64 dans la source. Rien a porter, rien perdu.
   - Insertion order : Record TS vs BTreeMap Rust. BTreeMap trie, TS garde l ordre d insertion, mais l ordre des cles d un objet JSON n a pas de sens semantique. Test lignes 262-284 verrouille le determinisme. OK.

4. Optional / absent / undefined / null :
   - TS Schema.optional = cle absente ou undefined (JSON.stringify supprime undefined). Rust Option + skip_serializing_if = meme JSON. Entree vide vaut {}, test lignes 113-123 OK.
   - Distinction [] vs absent et {} vs absent preservee : Some(vec![]) serialize garde la cle, None la supprime, tests lignes 162-181 OK.
   - Divergence connue et documentee : Rust accepte {"disabled":null} comme None (Rust lignes 56-60, 150-159) alors que Schema.optional TS refuserait null. Aucun JSON ecrit par le TS ne contient null ici, impact limite a une saisie manuelle plus tolerante cote Rust. Classe en nit, pas en BUG bloquant, car documente et verrouille par test.

5. Reexport ligne 1 TS (`export * as ConfigFormatter from "./formatter"`, auto-reexport) : rien a porter, absence cote Rust correcte.

6. Fichiers jumeaux hors lot : packages/core/src/config.ts:72 (consommateur `formatter` optional, parent hors lot) et packages/core/src/v1/config/formatter.ts (Struct vs Class, hors lot). Non touches, OK.

## Nits non bloquants

- N1 : tolerance null sur champs Option (Rust lignes 56-60, test 150-159). Si parite stricte exigee un jour, ajouter deny null via deserializeur custom ou `#[serde(deserialize_with=...)]`. Aujourd hui documente, test verrouille, ne pas changer sans decision.
- N2 : `#[serde(rename=...)]` redondants (Rust lignes 64,68,72,76). Garder comme verrou ou supprimer, au choix, sans effet JSON.
- N3 : politique champs inconnus non testee. Serde ignore par defaut. Effect strip par defaut sauf onExcessProperty:error. Sans preuve de mode strict cote TS, le comportement actuel est l hypothese raisonnable. Ajouter eventuellement un test `{"disabled":true,"inconnu":1}` pour verrouiller le choix.
- N4 : BTreeMap impose l ordre alphabetique a la serialisation (test ligne 282). Deterministe et voulu, mais un diff byte-a-byte avec un JSON TS garde-ordre peut differer sur l ordre des cles. Semantiquement equivalent.
