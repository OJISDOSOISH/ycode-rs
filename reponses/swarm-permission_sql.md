# swarm-permission_sql

===DEBUT===
fichier : src/swarm/permission_sql.rs
source  : packages/core/src/permission/sql.ts
taille  : 27314 octets
tests   : 19

CONFIANCE : haute
POINT FAIBLE : la source ne contient aucune logique, seulement un `sqliteTable`
Drizzle. Tout ce qui ressemble a de la logique ici (contrainte d'unicite, cascade
de suppression, projections sur une tranche) est une **reformulation de
contraintes SQL en fonctions pures**, pas une traduction. La ligne portee est
juste, mais `insertable_rows` et `rows_kept_after_project_delete` n'ont aucun
equivalent litteral dans `sql.ts` et n'ont vocation a etre consommees que par le
futur portage de `saved.ts`. Si la relecture considere qu'un tableau de 20 lignes
de declaration de table ne doit pas donner lieu a des fonctions, c'est le
premier passage a rouvrir. Second point, plus etroit : `PermissionRow` porte des
`#[serde(rename)]` identiques aux noms de champs, donc ils ne changent rien au
JSON ; ils servent a figer le contrat du DDL, et la relecture doit juger si ce
geste est utile ou si c'est du bruit.

A VERIFIER (par priorite) :
1. Que le reexport `pub use crate::core::session::schema::ProjectId;` resout
   bien (verifie : `src/core/session/schema.rs:40` declare
   `pub type ProjectId = String;`, `src/core/session/mod.rs:15` expose `schema`,
   `src/lib.rs:7` expose `core`). Le meme reexport existe dans
   `src/swarm/project_schema.rs:109` : si un jour l'agent principal deplace ces
   alias vers `crate::schema`, **deux** fichiers de mon lot sont a changer d'un
   coup, pas un.
2. Que `pub mod permission_sql;` reste bien en place dans
   `src/swarm/mod.rs:43`. Je n'y ai pas touche. C'etait deja declare.
3. Le choix de `id: String` et non `Option<String>`. Le DDL genere
   (`schema.gen.ts:91`) ecrit `id text PRIMARY KEY` **sans** `NOT NULL`, et
   SQLite accepte le NULL sur une `TEXT PRIMARY KEY`. La source n'appelle pas
   `.notNull()` sur cette colonne. J'ai documente pourquoi `String` reste le bon
   choix (`saved.ts:60` appelle `ID.create()`, jamais nul), mais c'est un
   arbitrage, pas une deduction.
4. `time_updated` a l'insertion. Les deux horodatages sont `NOT NULL` sans
   `DEFAULT` SQL (les crochets `$default` / `$onUpdate` sont cote ORM), et
   `saved.ts:59-64` n'en fournit que quatre valeurs. J'ai ecrit `now` dans les
   deux colonnes via `PermissionRow::new`, choix explicite et documente, parce
   que la base refuserait une ligne ou le champ serait absent. A confirmer.
5. Le nom des colonnes plutot que le nom du JSON public. `project_id` ici,
   `projectID` dans `PermissionSaved.Info` (`permission-saved.ts:16`). Les deux
   doivent coexister ; deux tests le verifient dans les deux sens.
6. Aucune syntaxe n'a pu etre compilee : aucun cargo, aucun rustc, conformement
   a l'interdiction. La relecture CI est la premiere verification de compilation
   de ce fichier.

ETAT AU MOMENT DE MA RAPPORT : le fichier etait deja sur le disque, porte par un
agent de la vague 3 (claim `permission_sql.claim.json`, proprietaire
`kilo-swarm3`). Je l'ai relu integralement, verifie chaque affirmation contre sa
source, et n'ai **corrige aucune erreur** : tout ce qu'il affirmait etait exact.
Ameliorations ajoutees :
- `pub const COLUMNS: [&str; 6]`, l'ordre du `CREATE TABLE`, avec deux tests
  (ordre, et concordance entre les colonnes declarees et les cles reellement
  serialisees de `PermissionRow`).
- Documentation de l'absence de `NOT NULL` sur `id` (voir point 3).
- Documentation du piege `?` contre `??` de `saved.ts:46`
  (`.where(input?.projectID ? eq(...) : undefined)` est un ternaire, donc un
  `projectID` vide est traite comme absent). Signale sans le porter : `list`
  appartient a `saved.ts`, ecrire la fonction ici ferait diverger deux
  implementations du meme comportement.
- Documentes les trois champs de `PermissionKey`, qui etaient les seuls sans
  commentaire.
- Quatre tests ajoutes (ordre des colonnes, concordance struct/table, ressource
  vide en aller-retour JSON, candidats vides sur table non vide) : 15 a 19.

CORRECTION APPORTEE EN COURS DE ROUTE : j'ai d'abord ecrit un caractere CJK
(errreur de ma part) dans un commentaire. Corrige immediatement. Controle
final du fichier : 4 octets non-ASCII au total, ce sont les guillemets
francais `«` `»` de la ligne 308, presents avant mon passage et laisses intacts
comme demande. Aucun accent, aucun CJK ailleurs.

SOURCES CITEES ET VERIFIEES : `permission/sql.ts` (20 lignes),
`permission/saved.ts` (79 lignes), `database/schema.sql.ts` (10 lignes),
`database/schema.gen.ts` lignes 90-98 et 242,
`packages/schema/src/permission-saved.ts` (20 lignes),
`src/permission.rs` (lu pour le contrat, **non modifie**).
Aucune migration, aucune dependance ajoutee, aucun autre fichier touche.
Aucun commit, aucun push.
===FIN===
