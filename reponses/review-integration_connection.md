# Review : swarm-integration_connection

## VERDICT : APPROUVE

Le portage est fidele. Aucun BUG bloquant. Les choix qui s ecartent de la
lettre (untagged, newtype transparent sans validation de prefixe,
constructeurs ergonomiques) sont justifies et testes.

## Sources relues integralement

- `opencode/packages/core/src/integration/connection.ts:1-12` : 12 lignes,
  aucun code executable. Un reexport d espace de noms vers lui-meme
  (ligne 1, rien a porter) + trois paires `export const` / `export type`
  vers `@opencode-ai/schema/connection` (lignes 5-12).
- `opencode/packages/schema/src/connection.ts:1-22` : formes reelles.
  `CredentialInfo` = Struct(type: Literal("credential"), id, label)
  (lignes 7-11). `EnvInfo` = Struct(type: Literal("env"), name)
  (lignes 14-17). `Info` = Union + toTaggedUnion("type") (lignes 19-21).
- `opencode/packages/schema/src/credential.ts:9-12` : `Credential.ID` =
  `Schema.String` + `brand` + statique `create` (`"cred_" + ascending()`).
  Aucun `check` / `isStartsWith` a la lecture. Le prefixe `cred_` est une
  convention de construction, pas une regle de lecture.
- `ycode-rs/src/swarm/integration_connection.rs:1-463` : portage + 10 tests.
- `ycode-rs/reponses/swarm-integration_connection.md:1-90` : rapport exact,
  relire section "A VERIFIER" ci-dessous tranchee point par point.
- Charte `ycode-rs/src/core/session/compaction.rs:1-171` : francais sans
  accents, docs `//!`, constructeurs `nouveau`, tests en francais. Le fichier
  porte suit la meme charte.

## Points verifies OK

1. Champs JSON exacts, piege ID/Id/majuscules : OK.
   - `CredentialInfo` Rust (`integration_connection.rs:193-203`) : `kind`
     + `#[serde(rename = "type")]` (`:195`), `id` (`:199`), `label` (`:202`).
     JSON produit = `type`, `id`, `label`, verrouille par le test
     `:319-341` (3 champs, absence de `kind` / `Kind`).
   - `EnvInfo` Rust (`:222-229`) : `kind` + rename (`:224`), `name` (`:228`).
     JSON = `type`, `name`, verrouille par le meme test (2 champs).
   - Litteraux `TypeCredential::Credential` -> `"credential"` (`:157-161`) et
     `TypeEnv::Env` -> `"env"` (`:173-177`) via `serde(rename)`. Conformes aux
     `Schema.Literal` du schema.
   - Aucun champ camelCase dans la source (`type`, `id`, `label`, `name`) :
     aucun `rename` manquant, aucun piege `projectID` / `projectId`.

2. `CredentialId` transparent sans validation de prefixe : OK, volontaire.
   - `#[serde(transparent)]` (`:119-121`) : se serialise comme une chaine nue,
     teste en `:347-353`. Conforme au `brand` TS qui n existe pas en JSON.
   - Pas de refus sans prefixe `cred_` : teste en `:359-371` (chaine
     `"abcdef"` et chaine vide acceptees, seul
     `nouveau_avec_prefixe` (`:134-139`) ajoute le prefixe). Conforme a
     `credential.ts:9-12` qui n a aucun filtre de lecture. Le contraste avec
     `session_message.rs` (`isStartsWith("msg_")` reel) est correctement
     analyse dans le rapport.

3. Union `Info` en `untagged` (`:255`) au lieu de `tag = "type"` : OK.
   - Justification exacte : chaque variante porte deja son champ `type` dans
     son payload ; un tag serde ajoute ecrirait la cle `type` deux fois.
     Le test `:417-429` verrouille `matches("\"type\"").count() == 1` plus
     roundtrip. Le JSON produit est identique au `toTaggedUnion("type")` TS.
   - Deserialisation : sur les entrees valides, `untagged` trie sur les
     litteraux de chaque struct et donne la meme variante que le tag TS,
     teste en `:397-412`. Sur les entrees invalides (tag inconnu `oauth`,
     champ obligatoire absent, `name: 42`, `id` objet), refus teste en
     `:434-447`. L ecart residuel (message d erreur moins precis sur entree
     invalide) est signale par le rapport lui-meme, non bloquant.

4. Operateurs JS pieges : sans objet, correctement neutralises.
   - Aucun `?` / `??` dans la source ni le schema. Aucun champ optionnel,
     aucun `skip_serializing_if`. Chaine vide conservee comme valeur presente
     des deux cotes, teste en `:378-391` (deserialisation + reserialisation
     avec `len() == 2`).

5. Types et edge cases : OK.
   - Que des `String` / newtype / enum unitaire : aucun piege `f64` / `i64` /
     `u64`, aucune longueur UTF-16, aucun ordre d insertion significatif
     (tests par acces de cle, pas par comparaison de chaine, sauf le comptage
     de `"type"` qui est volontaire).
   - Variantes : 2/2 portees, aucune perdue. `Credential.Value` (oauth/key,
     les vrais secrets) volontairement hors portee, rappele en `:53-61` et
     dans les tests (`:290-291`). Aucun secret ecrit.
   - Cas limites couverts par 10 tests : credential, env, noms de champs,
     chaine nue, sans-prefixe, chaine vide, tag, roundtrip sans doublon,
     invalides (5 cas), liste vide / singleton.

6. Hygiene : zero non-ASCII verifie par grep sur le fichier (aucun match
   `[^\\x00-\\x7F]`). Module declare dans `src/swarm/mod.rs` (ligne 26
   constatee). Aucun commit/push.

## Nits non bloquants

- Rapport annonce `pub mod integration_connection;` en `mod.rs` "ligne 24" :
  constate ligne 26. Simple decalage, sans effet.
- `untagged` : message d erreur de deserialisation moins precis qu un tag
  serde ou que le TS. Accepte tel quel ; ne pas "corriger" vers
  `tag = "type"` sans traiter le doublon de cle `type`.
- Ajouts ergonomiques hors source (`nouveau`, `nouveau_avec_prefixe`,
  `tag()`, `From`, constantes `SCHEMA_IDENTIFIER_*`, `as_str`,
  `into_inner`) : inoffensifs, documentes, ne changent pas le JSON.
- `CredentialId` defini localement au lieu d un import `util_identifier` :
  justification de course donnee dans le rapport ; a dedupliquer plus tard si
  l autre module pose un doublon, sans changer le JSON.
- Tests non executes dans cet environnement (`cargo` indisponible ici).
  Lancer `cargo test swarm::integration_connection` sur une machine outillee ;
  revue ci-dessus purement statique mais exhaustive ligne a ligne.
