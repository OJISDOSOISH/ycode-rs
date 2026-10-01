# swarm-project_schema

===DEBUT===
fichier : src/swarm/project_schema.rs
source  : packages/core/src/project/schema.ts
taille  : 11167 octets
tests   : 7

CONFIANCE : moyenne
POINT FAIBLE : le choix de reexporter `ProjectId` et `AbsolutePath` depuis `crate::core::session::schema` au lieu de les redeclarer — c'est faithful a `export const ID = Project.ID` et ca evite la duplication que la revue a deja attrapee, mais ca couple ce fichier a un module que sa propre doc dit etre "intermediaire" et destine a remonter dans `crate::schema`.
A VERIFIER : (1) que `crate::core::session::schema` expose bien `ProjectId` et `AbsolutePath` en `pub` au moment du build, sinon le `use` casse la compilation ; (2) que `VcsType` a variante unique est bien la traduction attendue de `Schema.Literal("git")` et non un `String` ; (3) la confusion possible avec l'AUTRE `Vcs`, celui de `packages/schema/src/project.ts` ligne 11, qui est un `Literal("git")` nu et non l'objet `{ type, store }`.
===FIN===

## Detail pour la relecture

### Ce que la source contient reellement

16 lignes, dont une seule chose a porter : le contrat `Vcs`. Le reste est
un reexport.

- `export * as ProjectSchema from "./schema"` : auto-reexport d'espace de noms,
  idiomme present dans tout le depot. En Rust c'est le module lui-meme, rien a
  ecrire.
- `export const ID = Project.ID` : `Project.ID` vient de
  `packages/schema/src/project-id.ts`, c'est `Schema.String.pipe(Schema.brand(
  "Project.ID"))`. Le nom de marque n'existe qu'a la compilation TS, a
  l'execution c'est un `string`. Traduit par
  `pub use crate::core::session::schema::ProjectId;`.
- `export const Vcs = Schema.Union([...])` : le seul vrai contenu.

### Les trois decisions non evidentes

1. **`Schema.Union` d'un seul membre devient un `struct`.** Pas un raccourci :
   l'ADT n'a aucune variante a choisir. Documente dans le fichier ce qu'il
   faudra faire le jour ou un second gestionnaire de versions arrive (struct ->
   `enum` tagge par `type`).

2. **`Schema.Literal("git")` devient `enum VcsType { Git }`, pas `String`.**
   C'est une ADT a une valeur qui **refuse** de decoder autre chose que `"git"`.
   Un `String` serialiserait pareil mais laisserait passer n'importe quoi a la
   deserialisation. Le test `un_type_vcs_inconnu_est_refuse_a_la_lecture`
   verrouille ce point des deux cotes (`"mercurial"` et `"Git"`, ce dernier
   pour prouver que la casse compte).

3. **`type` est un mot cle Rust.** Champ nomme `r#type` (identifiant brut),
   avec `#[serde(rename = "type")]` pose explicitement comme l'impose la regle
   des noms de champs.

### Deux `Vcs` distincts existent dans le depot

C'est le piege le plus dangereux de ce fichier, et il est signale en tete de
module :

- `packages/schema/src/project.ts` ligne 11 : `Vcs = Schema.Literal("git")`,
  une **chaine**. Utilise par `Project.Info.vcs`.
- `packages/core/src/project/schema.ts` : `Vcs = { type: "git", store }`, un
  **objet**. C'est celui que je porte.

Ils sont tous deux utilises cote TS, simultanement. Confondu, `Project.resolve`
et `filesystem/watcher.ts` (qui teste `location.vcs?.type === "git"`) cassent.

### `store` : le repertoire git COMMUN, pas le `.git` du worktree

`packages/core/src/project.ts` ligne 120 : `store: repo.commonDirectory` ou
`repo` vient de `simple-git`. Pour un worktree, c'est le `.git` du depot
d'origine, partage avec les autres worktrees, et non le fichier `.git` du
worktree lui-meme. C'est bien ce chemin que le file watcher parcourt.

### `GLOBAL_PROJECT_ID`

`ProjectID` porte une methode statique `global` (`project-id.ts` :
`statics((schema) => ({ global: schema.make("global") }))`), utilisee comme
valeur de repli a `project.ts` lignes 112 et 118. C'est une valeur du contrat
publiee dans le JSON, exposee ici comme constante, sur le modele de
`DEFAULT_VARIANT` dans `src/core/session/schema.rs`.

Type `&str` et non `ProjectId` : une allocation de `String` n'est pas evaluable
a la compilation, donc `const X: String = "..."` ne compile pas.

### Tests

7 tests, tous en francais sans accents, decrivant un comportement :

| test | ce qu'il verrouille |
|---|---|
| `le_vcs_serialise_le_type_et_le_depot` | noms JSON `type` et `store` |
| `le_type_vcs_vaut_toujours_git` | le constructeur fixe la valeur |
| `un_type_vcs_inconnu_est_refuse_a_la_lecture` | le `Literal` refuse, casse comprise |
| `un_vcs_lu_depuis_le_json_reprend_le_depot` | sens inverse de la deserialisation |
| `un_vcs_sans_type_est_refuse` | champ obligatoire des deux cotes |
| `l_identifiant_global_est_la_chaine_global` | la valeur de repli |
| `un_chemin_vide_est_accepte_comme_chemin` | `AbsolutePath` n'impose rien sur le contenu |

Le dernier teste un point delimite par le piege `?` / `??` : dans ce schema une
chaine vide **survit**, alors que `project.ts` ligne 67 utilise un ternaire
(`value ? ID.make(value) : undefined`) et la ferait disparaitre. Les deux sont
des choix distincts et voulus.

### Verification de forme

- Aucun caractere non-ASCII (scan sur tout le fichier) : 0.
- `src/swarm/mod.rs` declare deja `pub mod project_schema;` (ligne 26). Rien a
  signaler de ce cote, et je n'ai touche a aucun autre fichier.
- `src/core/session/schema.rs` a seulement ete lu, pas modifie.
- Aucun commit, aucun push.
