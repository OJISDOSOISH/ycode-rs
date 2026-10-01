# swarm-core_file

===DEBUT===
fichier : src/swarm/core_file.rs
source  : packages/core/src/file.ts
taille  : 9855 octets
tests   : 9

CONFIANCE : haute
POINT FAIBLE : le fichier TS source ne contient aucun type ; il ne fait que
reexporter `Revert.FileDiff`. J'ai donc porte la definition reellee trouvee dans
`packages/schema/src/revert.ts`, pas le contenu de `file.ts`. Si un autre agent
porte ce `revert.ts`, on aura deux definitions de `FileDiff` concurrentes.
A VERIFIER : rien de compile sur cette machine, donc la relecture doit
(1) confirmer que `packages/schema/src/revert.ts:8-14` est bien la seule
definition du type, (2) verifier les 3 tags de `DiffStatus` (`"added"`,
`"modified"`, `"deleted"`) contre `Schema.Literals` ligne 10, (3) verifier
qu'il n'y a bien AUCUN `#[serde(tag = "...")]` sur `DiffStatus` — voir la note
ci-dessous, c'est le piege principal.

--- TRUC A NE PAS MANQUER ---
`status` n'est pas un objet balise. `Schema.Literals(["added","modified","deleted"])`
produit une **chaine nue** en JSON (`"status": "added"`), pas
`{"type":"added"}`. Mettre `#[serde(tag = "...")]` sur l'enum aurait produit
`{"Added": null}` et casse l'echange avec le TypeScript. Les 3 renoms sont
donc explicites, un par variante, sans `rename_all`, pour qu'ils soient
verifiables a l'oeil et insensibles a tout changement de convention de casse.

--- AUTRES CHOIX ---
- Nom `Diff` conserve pour le struct (le TS exporte valeur et type sous le
  meme nom, impossible en Rust).
- `additions` / `deletions` : `u64` et non `i64`, car `NonNegativeInt`
  (schema.ts:4) est positif ou nul. La contrainte TS devient une contrainte de
  compilation ; un JSON negatif est refuse a la lecture des deux cotes.
- `path` : `String`. Le `brand("RelativePath")` du TS est une verification de
  type a la compilation, invisible a l'execution et donc absente du JSON.
- Aucun nom de champ en camelCase dans ce type (`path`, `status`, `additions`,
  `deletions`, `patch`) donc aucun `#[serde(rename)]` necessaire. Verrouille
  par un test qui compare la serialisation JSON complete, chaine par chaine.
- `total_changed()` est le SEUL ajout au modele. Il n'existe pas dans le TS ;
  signale ici pour que la relecture puisse le supprimer s'il le juge hors
  perimetre.
- Pas de conflit de nom avec `src/core/session/revert.rs` (qui ne definit
  aucun type `FileDiff`, seulement un `RevertPlan`).

--- A NE PAS MODIFIER ICI ---
`src/swarm/mod.rs` (contient deja `pub mod core_file;`) et `Cargo.toml`
(deja `serde` + `serde_json` avec la derive) : rien a ajouter, je n'y ai pas
touche.
===FIN===
