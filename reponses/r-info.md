# Relecture croisee : src/core/session/info.rs

===DEBUT===
originator: kilo
target: src/core/session/info.rs
source: opencode/packages/core/src/session/info.ts
claims/info.claim.json

## Ce que j'ai fait

Portage de la fonction `fromRow` de l'original. Une seule fonction de
conversion, plus les types necessaires pour etre serialisable.

## Point 1 : divergence `?` contre `??` (le coeur du fichier)

C'est le seul endroit du fichier ou je pense m'etre trompe, donc je le signale
en premier.

L'original melange deux operateurs qui ne se comportent pas pareil, et je les ai
traduits par deux fonctions distinctes :

    row.parent_id ? ... : undefined     -> fn truthy(&Option<String>)
    row.model.variant ?? "default"      -> fn present_or(Option<&str>, &str)

Le ternaire teste la **veracite** : `""` est falsy en JavaScript, donc
`parent_id = Some("")` doit produire `None`. Le coalescent teste la **nullite** :
`variant = Some("")` doit rester `Some("")`.

Verifie bien que ces deux comportements sont distincts dans mon code, et que
`present_or` n'a pas recu le traitement de `truthy` par erreur. Testes
concernes : `la_chaine_vide_disparait_sur_les_ternaires` et
`la_chaine_vide_survit_sur_le_coalescent_du_variant`.

Meme piege sur les entiers : `row.time_archived ? ... : undefined`, donc `0` doit
disparaitre. Teste par `un_horodatage_nul_disparait_aussi`.

## Point 2 : `tokens` est imbrique cote TS, plat en base

La table a cinq colonnes (`tokens_input`, `tokens_output`, `tokens_reasoning`,
`tokens_cache_read`, `tokens_cache_write`) et le schema a deux niveaux
(`tokens.cache.read`). Je regroupe dans `from_row`. Confirme que je n'ai pas
inverse un nom de colonne en route.

## Point 3 : le type des compteurs

`Schema.Finite` en TS, `real` et `integer` en SQL. J'ai utilise `f64` partout,
y compris pour `cost`. Est-ce correct, ou le portage attendu prefere-t-il `i64`
pour les jetons ?

## Point 4 : CHEVAUCHEMENT AVEC schema.ts — c'est le point qui m'inquiete

Je **decline** de `schema.rs` pour l'instant. Raison :

- `core/src/session/schema.ts` ne fait que 9 lignes : il reexporte
  `Session.ID` et `Session.Info` depuis `@opencode-ai/schema/session`.
- Comme la regle dit de ne pas fabriquer ce qui n'est pas dans la source, le
  portage naturel de `schema.ts` est un reexport, pas une redéfinition.
- Or j'ai defini `Info`, `ModelRef`, `RevertState`, `Tokens`, `Time`,
  `LocationRef` dans `info.rs`. Si `schema.rs` redéfinit les memes types, on a
  deux definitions divergentes du meme contrat, et c'est exactement le piege que
  la revue precedente a déjà attrapé sur `sessionID` / `callID`.

Ma proposition : `schema.rs` fait `pub use super::info::{Info, ...}` et
n'introduit aucun type nouveau. Dis-moi si tu prefères l'inverse, c'est-a-dire
definir `Info` dans `schema.rs` et faire dependre `info.rs` de lui. La
deuxieme option est plus propre architecturalement, mais elle m'oblige a
refaire `info.rs` ; dis-le maintenant plutot qu'apres.

## Ce que je n'ai pas fait

- Rien compile, rien execute : pas de toolchain Rust sur ce poste. Ces 13
  tests n'ont donc jamais tourne. Ils sont ecrits, pas verifies.
- J'ai modifie `src/core/session/mod.rs` pour declarer `pub mod info;`. C'est le
  seul fichier touche hors cible.
===FIN===
