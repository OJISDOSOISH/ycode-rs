# swarm-core_workspace

===DEBUT===
fichier : src/swarm/core_workspace.rs
source  : packages/core/src/workspace.ts
taille  : 19531 octets
tests   : 18

CONFIANCE : moyenne
POINT FAIBLE : la source ne fait que reexporter, j'ai donc du descendre jusqu'a `packages/schema/src/workspace-id.ts` et `packages/schema/src/identifier.ts` pour savoir ce que vaut `Workspace.ID`, et j'ai alors du choisir entre rester minimal et rendre le type reellement utilisable : j'ai implemente le generateur d'identifiants en local (SplitMix64 + Mutex) au lieu d'appeler un futur `util_identifier`, et j'ai ajoute une validation a la deserialisation qui n'est pas un `#[derive]` tout simple. Ces deux la sont des jugements, pas des traductions.
A VERIFIER : en priorite (1) que `#[serde(deserialize_with = ...)]` au niveau CONTAINER d'un newtype est bien compile par serde_derive (c'est le seul endroit du fichier ou j'ai utilise une mecanique que je n'ai pas vue dans le depot) ; (2) le piege `!id` : `WorkspaceId::ascendant(Some(""))` doit fabriquer un identifiant neuf et NON renvoyer une erreur ; (3) le format du generateur : 12 caracteres hexadecimaux de `millis * 4096 + compteur` puis 14 caracteres de l'alphabet de 62, soit 26 caracteres apres `wrk_` ; (4) qu'aucun `descending()` n'a ete invente, la source n'utilisant que la branche croissante.

DETAIL :
- `packages/core/src/workspace.ts` fait 6 lignes : un `export * as WorkspaceV2 from "./workspace"` (reexport du fichier sur lui-meme = code mort, non retranscrit) et `export const ID = Workspace.ID`.
- Chaine de resolution lue integralement : `schema/src/workspace.ts` -> `schema/src/workspace-id.ts` -> `schema/src/identifier.ts`. Le contenu reel de `ID` est donc `WorkspaceID` : une chaine **validee** par `Schema.isStartsWith("wrk")`, de marque `Schema.brand("WorkspaceV2.ID")`, avec deux statiques (`create`, `ascending(id?)`).
- Portage : `struct WorkspaceId(String)` derive `Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash` + `Display` + `AsRef<str>` + `From<WorkspaceId> for String` ; constantes `ID_PREFIX` et `SCHEMA_IDENTIFIER` ; `erreur` = `struct WorkspaceIdError` (thiserror) qui reproduit le message exact `ID {id} does not start with wrk`.
- JSON : la source est une chaine de marque, donc le newtype est transparent (`serde_json` donne `"wrk_..."`, pas `{"id":...}`). Aucun nom de champ, donc **aucun `serde(rename)` a poser** : le piege 2 ne s'applique pas.
- PIEGE 1 (`?` contre `??`) : il est present, et traite. La source ecrit `if (!id) return create()`, c'est un test de VERACITE, donc `""` est traite comme absent et devient un identifiant neuf. Mon `match` a une branche dediee `Some(id) if id.is_empty() => Ok(Self::create())`. Un portage naif `Some(id) => Self::new(id)` aurait refuse `""` : c'est faux.
- Validation a la deserialisation : le schema TS refuse de decoder une chaine qui ne commence pas par `wrk`, un `#[derive(Deserialize)]` tout simple l'accepterait. J'ai donc ecrit `deserialize_workspace_id` (7 lignes) qui appelle `WorkspaceId::new` et renvoie `D::Error::custom`. C'est un choix, pas une traduction.
- Aleatoire : `crypto.getRandomValues` n'a pas d'equivalent en std et `Cargo.toml` n'a aucune dependance generative (`rand` absent, et `uuid` n'a que les features `v7` et `serde`, donc pas `new_v4`). Les 14 octets viennent d'un SplitMix64 amorce sur l'horloge, protege par le meme `Mutex` que le compteur. Le format produit est identique a la source.
- Etat partage : les deux variables de module `lastTimestamp` et `counter` de `identifier.ts` deviennent un `static ETAT_GENERATEUR: Mutex<EtatGenerateur>` (le mutex empoisonne est recupere au lieu de paniquer).
- Non porte : `descending()` et `create(descending, timestamp)` de `identifier.ts` (la source n'utilise que la branche croissante) ; le `WorkspaceEvent` de `schema/src/workspace.ts` (sans rapport avec `core/workspace.ts`, qui n'exporte que `ID`).
- Heritage de la source signale dans le fichier : la partie temporelle ne sort que les 48 bits de poids faible de `millis * 4096 + compteur`, soit 53 bits au total, donc 5 bits sont tronques PAR LA SOURCE. L'identifiant reste ordonne sur une fenetre d'environ 2,2 ans. Mon test d'ordre est donc theoriquement susceptible de tomber sur un basculement, avec une probabilite de l'ordre de 1e-13 par execution.
- Tests (18) : prefixe, longueur 30, alphabet autorise, partie temporelle en hexadecimal minuscule, unicite, ordre croissant, `wrk`/`wrkXYZ` acceptes, `""`/`autre`/`Wwrk` refuses, chaine vide traitee comme absente, identifiant fourni reutilise tel quel, identifiant fourni invalide refuse, `None` fabrique un neuf, serialisation en chaine simple, deserialisation qui refuse `"autre"`/`""`/`42`, liste de deux identifiants, melangeur, horodatage en millisecondes, nom du schema.
- Integration : `pub mod core_workspace;` est deja present dans `src/swarm/mod.rs` ligne 22. Je n'y ai rien touche. Aucun commit, aucun push.
- Aucun caractere non-ASCII (verifie octet par octet : 0 occurrence ; un accent qui s'etait glisse dans le doc du module a ete retire).
===FIN===
