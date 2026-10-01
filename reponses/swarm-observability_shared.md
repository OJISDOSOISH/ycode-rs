# swarm-observability_shared

===DEBUT===
fichier : src/swarm/observability_shared.rs
source  : packages/core/src/observability/shared.ts
taille  : 5364 octets
tests   : 7

CONFIANCE : haute
POINT FAIBLE : la source exporte une constante calculee au chargement du
module ; je l ai rendue par `pub fn run_id() -> &'static str` avec un
`OnceLock`, parce que Rust interdit un `const` calcule a l execution. Si la
relecture prefere un nom de symbole plus proche de la source (constante
exportee plutot que fonction), c'est le seul ecart d API du fichier.
A VERIFIER : (1) que `run_id()` est bien utilise une seule fois par processus
et pas seulement comme valeur par defaut, et (2) le nom `run_id` contre
`runID` : aucun nom de champ n'est serialise ici, donc aucun
`serde(rename)` n'a ete pose, ce qui est le bon comportement pour un module
qui ne porte aucun struct.

NOTE : la source est une seule ligne,
`export const runID = crypto.randomUUID().slice(0, 8)`. Il n'y a ni type, ni
classe, ni fonction a porter, donc le fichier est court et se limite a
l'identifiant de run. Le format est fige par la source : 8 caracteres
hexadecimaux minuscules, le separateur `-` d'un UUID canonique n'arrivant
qu'en 9e position.

DEVIATION ASSUMEE : `Uuid::new_v4()` serait la traduction directe, mais la
crate `uuid` du `Cargo.toml` n'active que les features `v7` et `serde`, donc
`new_v4` n'existe pas, et je ne touche pas a `Cargo.toml`. Les 32 bits sont
tires de `RandomState` (cle fournie par le systeme) melanges a l'horloge. Si
quelqu'un ajoute la feature `v4` a `Cargo.toml`, `generer_run_id` pourra etre
remplacee par `Uuid::new_v4().simple().to_string()[0..8].to_string()` sans
changer le reste du fichier ni le format produit.

===FIN===
