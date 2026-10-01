# swarm-config_tool_output

===DEBUT===
fichier : src/swarm/config_tool_output.rs
source  : packages/core/src/config/tool-output.ts
taille  : 7787 octets
tests   : 9

CONFIANCE : haute

POINT FAIBLE : la seule divergence de comportement que je vois est `0`.
`PositiveInt` vaut `Schema.Int.check(Schema.isGreaterThan(0))`, donc le
TypeScript refuse 0 ; j'ai porte le type par `u64`, qui refuse les negatifs
mais accepte 0. J'ai ajoute `est_positif(valeur) -> bool` pour retrouver le
controle, mais je ne l'ai cable a rien : un `deserialize_with` qui refuserait
0 ala deserialisation serait plus fidele mais multiplies le risque de
compilation sans compilateur local, et personne ne l'appellera de toute facon
tant que la config n'est pas branchee. Si la relecture veut la fidelite
stricte, c'est la qu'il faut trancher.

A VERIFIER : les deux noms de champs. Ils sont DEJA en `snake_case` dans la
source (`max_lines`, `max_bytes`) : c'est le seul fichier de ma mission ou le
piege du `serde(rename)` joue a l'envers. J'ai pose `#[serde(rename =
"max_lines")]` et `#[serde(rename = "max_bytes")]` — volontairement redondants,
identiques au nom du champ, comme le fait deja `src/question.rs` — donc le
JSON reste `{"max_lines": ..., "max_bytes": ...}`. A verifier aussi qu'il ne
faut surtout pas de `deny_unknown_fields` : `Schema.Class` ignore les
proprietes en trop, et j'ai un test qui le prouve.

AUTRE POINT A VERIFIER : la coherence avec `compaction.rs`. Je ne l'ai PAS
modifie, comme demande. Je signale que la correspondance proposee dans la
mission est de surface, pas de fond : `TOOL_OUTPUT_MAX_CHARS = 2_000` compte
des CARACTERES injectes dans le contexte, alors que `max_lines` compte des
LIGNES conservees dans le magasin de sorties. Les 2 000 se ressemblent parce
que `tool-output-store.ts` declare `MAX_LINES = 2_000`, mais ce sont deux
regulations differentes. Je l'ai ecrit en commentaire dans le fichier pour
que personne ne fusionne les deux seuils par erreur.

RAPPEL POUR L'AGENT PRINCIPAL : `pub mod config_tool_output;` est deja present
dans `src/swarm/mod.rs` ligne 15, donc rien a declarer (je n'y ai pas touche).
`PositiveInt = u64` est declare comme alias de type DANS mon fichier, donc
si un autre agent porte aussi `packages/schema/src/schema.ts`, on aura deux
declarations du meme alias : ca compile, mais c'est a arbitrer.
===FIN===
