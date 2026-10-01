# review-config_plugin

## VERDICT : APPROUVE

Sources lues en entier :
- TS : `opencode/packages/core/src/config/plugin.ts` (13 lignes)
- Rust : `ycode-rs/src/swarm/config_plugin.rs` (301 lignes)
- Rapport : `ycode-rs/reponses/swarm-config_plugin.md`
- Charte : `ycode-rs/src/core/session/compaction.rs` (style, tests, commentaires)
- Module : `ycode-rs/src/swarm/mod.rs` ligne 18 (`pub mod config_plugin;` present)

Portage fidele pour un fichier sans logique. Aucune divergence bloquante.

## Points verifies OK

1. Champs JSON exacts, sans rename piege :
   - TS lignes 6-7 : `package: string`, `options?: Record<string, Unknown>`.
   - Rust lignes 55-67 : `pub package: String`, `pub options: Option<Options>` sans `rename`, noms en minuscules.
   - Test lignes 226-237 `les_noms_de_champs_json_sont_ceux_du_typescript` verifie `package` / `options` et l absence de `Package` / `Options`. OK.
   - Pas de piege `ID` / `Id` / `sessionID` / `callID` dans cette source. Rien a preserver d autre.

2. Variantes d union conservees, ordre conserve :
   - TS ligne 10 : `Schema.Union([Schema.String, Entry])`, non taggee, String en premier.
   - Rust lignes 95-102 : `#[serde(untagged)] enum Plugin { Name(String), Entry(Entry) }`, meme ordre. OK.
   - Test lignes 153-175 : chaine -> `Name`, objet `{package, options}` -> `Entry`. OK.
   - Test lignes 292-300 `une_valeur_qui_n_est_ni_chaine_ni_entree_est_refusee` : `42`, `null`, `{}` et `{"autre":1}` refusent, comme attendu d une union non taggee a deux branches. OK.

3. Optional TS -> Option Rust :
   - TS lignes 7 : `Schema.Record(...).pipe(Schema.optional)` = cle absente autorisee.
   - Rust ligne 65 : `#[serde(default, skip_serializing_if = "Option::is_none")]`. Lecture sans cle OK, ecriture sans cle `null` OK.
   - Test lignes 178-187 `une_entree_sans_options_a_la_forme_la_plus_courte` verifie `{"package":"..."}` sans cle `options`. OK.

4. Operateurs JS pieges :
   - Truthiness / `?` vs `??` : rien n est teste en veracite. Test lignes 282-290 `un_nom_de_plugin_vide_reste_un_nom_valide` : `""` reste `Name("")` et survit a l aller-retour. OK.
   - UTF-16 length : sans objet ici, pas de `.length` sur chaine dans la source. Pas de conversion `chars().count()` a verifier. OK.
   - Insertion order du tableau : `Plugins` TS ligne 13 = `Schema.Array`, ordre significatif. Rust ligne 143 `pub type Plugins = Vec<Plugin>` preserve l ordre. Tests lignes 200-221 (ordre `["a",{package:b},"c"]` et `["c","b","a"]` conserves a l aller-retour). OK.

5. Types et valeurs libres :
   - `Schema.Unknown` TS ligne 7 -> `serde_json::Value` Rust ligne 39-47. Aucune perte : nombre, booleen, null, tableau, objet imbrique.
   - Test lignes 240-256 `les_options_acceptent_toute_forme_de_valeur` couvre `1`, `1.5`, `false`, `null`, `[1,2]`, `{"k":"v"}`. OK. Pas de `f64` / `i64` / `u64` a trancher ici, `Value` garde le nombre tel quel.

6. Edge cases :
   - Liste vide `[]` -> `vec![]` aller-retour, test lignes 192-197. OK.
   - Champ inconnu ignore, test lignes 259-267 : `{"package":"pkg","inconnu":42}` donne `pkg`. Coherent avec le decodage Effect qui ignore l excedent par defaut et avec Serde qui ignore par defaut. OK.
   - `options: {}` vide distingue de absent au niveau struct (`Some(empty)` vs `None`), test lignes 270-280. OK.

## Nits non bloquants

1. `Options = BTreeMap<String, Value>` (ligne 47) trie les cles, objet JS garde l ordre d insertion. Divergence reelle mais sans effet semantique ici : les options sont un dictionnaire d arguments, pas une liste ordonnee. Le choix est deterministe et coherent avec le reste du portage (Map -> BTreeMap). De plus `serde_json::Map` par defaut trie aussi, donc passer a `IndexMap` pour `Options` seul ne preserverait pas l ordre imbrique sans activer `preserve_order`. Ne pas bloquer. Au pire, documenter dans un test que la serialisation trie les cles si on veut figer le comportement.

2. `{"package":"x","options":null}` : Rust donne `None` (Serde mappe `null` vers `None`), TS avec `Schema.optional` seul rejetterait `null` (seul `undefined` / absent est admis, sauf `nullable`). Tolerance en lecture un peu plus large cote Rust. Cas limite non rencontre en config reelle. Ne pas bloquer. Un test `options_null_est_accepte_comme_absent` pourrait figer le choix.

3. Commentaire ligne 84 : dit que TS "ne distingue pas `undefined` de `{}`". Formule inexacte : TS distingue absent (`None`) de present-vide (`Some(empty)`), comme le fait `Option`. Le code est juste (`options_or_empty` ne fait que faciliter la lecture sans detruire la distinction dans le struct), seul le commentaire est a corriger si on y touche.

4. Sucre hors source : `Entry::new`, `with_options`, `options_or_empty` (lignes 69-88), `Plugin::name`, `entry`, `package_name`, `options` (lignes 104-136). Helpers de lecture sans logique ajoutee, deja signales dans le rapport points 4. Garder : usages pratiques, aucun risque. Suppression possible si portage strict minimal exige, mais non requis.

5. Non porte volontairement, OK : identifiant de schema `"ConfigV2.Plugin.Entry"` (TS ligne 5) et `export * as ConfigPlugin` (TS ligne 1). Pas d equivalent en Rust, aucun impact sur JSON.
