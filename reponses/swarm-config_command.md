# swarm-config_command

===DEBUT===
fichier : src/swarm/config_command.rs
source  : packages/core/src/config/command.ts
taille  : 7197 octets
tests   : 5

CONFIANCE : haute
POINT FAIBLE : la source fait 12 lignes et ne contient aucun ternaire `?` ni coalescent `??`, donc aucune decision d'interpretation n'a ete a prendre ; mon risque reel est d'avoir ajoute du contenu non demande (constructeur `Info::new`, constante `SCHEMA_IDENTIFIER`) que la relecture jugera superflu, et non une erreur de comportement.
A VERIFIER : (1) que `skip_serializing_if = "Option::is_none"` sur les cinq champs optionnels correspond bien a `Schema.optional` (cle facultative ET absente de la sortie) plutot qu'a un `undefined` serialise ; (2) qu'aucun `#[serde(rename)]` n'est requis, les cinq champs optionnels etant des mots uniques en minuscules - verifier un par un ; (3) que `export * as ConfigCommand from "./command"` (reexport du fichier sur lui-meme) est bien du code mort que je n'ai pas a retranscrire.

DETAIL :
- struct `Info` derive `Serialize, Deserialize`, `Eq` (tous les champs sont String/Option<String>/Option<bool>).
- `template` obligatoire, cinq champs optionnels.
- Aucun nom camelCase dans ce fichier : le piege 2 du contexte ne s'applique pas ici.
- Tests : cas minimal, cas complet, noms de champs serialises, chaine vide preservee, entrees invalides (template manquant, subtask en chaine).
- Fichier declare : deja present dans `src/swarm/mod.rs` ligne 11 (`pub mod config_command;`). Je n'y ai rien touche.
- Aucun accented, aucun caractere non-ASCII (verifie : 0 occurrence).
- Aucun commit, aucun push.
===FIN===
