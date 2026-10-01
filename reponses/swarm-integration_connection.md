# swarm-integration_connection

===DEBUT===
fichier : src/swarm/integration_connection.rs
source  : packages/core/src/integration/connection.ts
taille  : 18853 octets
tests   : 10

CONFIANCE : haute
POINT FAIBLE : l'union `Info` est `#[serde(untagged)]` et non `#[serde(tag = "type")]` ; j'ai fait ce choix parce que le tag `type` est deja un champ de chaque struct, donc un tag serde ajouter l'ecrirait deux fois. Le JSON est identique, mais le message d'erreur sur une entree invalide est nettement moins precis, et je n'ai pas de toolchain pour le verifier.
A VERIFIER : (1) `untagged` contre `tag = "type"` sur `Info`, c'est le point a trancher en premier ; (2) `#[serde(transparent)]` sur `CredentialId` et le fait que je n'applique AUCUNE validation de prefixe `cred_`, parce que `Credential.ID` est un `Schema.brand` sans filtre de lecture, donc le TS accepte aussi n'importe quelle chaine ; (3) le nom du champ `type`, nomme `kind` en Rust avec `#[serde(rename = "type")]` ; (4) le nom `nouveau` au lieu de `new`, pose partout dans le fichier.
===FIN===

## Ce que la source est

Douze lignes, aucun code executable. Un reexport d'espace de noms
(`export * as IntegrationConnection from "./connection"`, qui pointe sur le
fichier lui-meme, donc rien a porter) et trois paires
`export const X` / `export type X` vers `@opencode-ai/schema/connection`.

Aucun agent du lot ne porte `packages/schema/src/connection.ts` : j'ai verifie
le dossier `claims`, il n'y a pas de `connection_schema.claim.json`. Ecrire un
fichier de dix lignes " pur reexport " aurait donc fait disparaitre les trois
types du projet. J'ai porte les formes, en m'en tenant a ce que le schema
declare.

Le schema reel (`packages/schema/src/connection.ts`) :

- `CredentialInfo = Struct({ type: Literal("credential"), id: Credential.ID, label: String })`
- `EnvInfo = Struct({ type: Literal("env"), name: String })`
- `Info = Union([CredentialInfo, EnvInfo]).pipe(toTaggedUnion("type"))`

## Secrets

Aucun. La source ne contient ni jeton, ni cle, ni donnee OAuth, et je n'en ai
ecrit nulle part, pas meme dans les tests. Le fichier ne porte que la forme :
`id`, `label`, `name`. Les valeurs sensibles vivent dans `Credential.Value`
(`key` / `oauth`), qui n'est pas dans ma source et que je n'ai pas porte. Les
deux tests qui contiennent une valeur, `"Compte principal"` et
`"OPENCODE_KEY"`, sont des libelles et un nom de variable, pas des secrets.

## Piege des noms de champs : sans risque ici

Les quatre noms de champs de la source (`type`, `id`, `label`, `name`) sont des
mots uniques en minuscules. Aucun nom camelCase, donc **aucun `serde(rename)`
n'est requis** et le piege `projectID` / `projectId` ne se pose pas. Le seul
`rename` pose est `#[serde(rename = "type")]`, parce que `type` est un mot cle
reserve en Rust ; le champ s'appelle `kind`, comme le fait deja
`src/schema/session_message.rs`. Un test verifie les quatre noms dans le JSON
produit, y compris l'absence de `kind`.

## Piege `?` contre `??` : sans risque ici

Aucun ternaire, aucun coalescent dans la source ni dans le schema. Il n'y a donc
aucune divergence possible entre chaine vide et `undefined`. Aucun champ n'est
optionnel non plus, donc aucun `skip_serializing_if`. Un test verrouille qu'une
chaine vide survit des deux cotes.

## Le detail que je signale

`Credential.ID` est `Schema.String.pipe(Schema.brand("Credential.ID"))`. Le
brand est une verification de type a la compilation, sans contrepartie dans le
JSON. Je l'ai rendu par un newtype `#[serde(transparent)]` sur `String`, qui
semble exactement une chaine. Je n'ai **pas** ajoute de verification de prefixe
`cred_` a la lecture, parce que le schema n'en a pas non plus : le prefixe
n'est produit que par le statique `create`. C'est volontaire et c'est teste.
Compare avec `MSG_ID_PREFIX` de `session_message.rs`, qui lui correspond a un
`Schema.isStartsWith` reel : la situation est differente et le traitement doit
rester different.

## Integration

`pub mod integration_connection;` est deja present dans `src/swarm/mod.rs`
(ligne 24). Je n'y ai pas touche. Le fichier ne depend d'aucun autre module du
lot, donc il se compile isolement. `serde` et `serde_json` sont deja dans
`Cargo.toml`. Aucune declaration a ajouter nulle part.

## Choix qui s'ecartent de la lettre de la source

- Les constructeurs `CredentialInfo::nouveau` et `EnvInfo::nouveau`, et
  `CredentialId::nouveau_avec_prefixe` : purement ergonomiques, meme precede que
  celui de `config_command.rs`.
- Les constantes `SCHEMA_IDENTIFIER_CREDENTIAL_INFO`, `SCHEMA_IDENTIFIER_ENV_INFO`
  et `SCHEMA_IDENTIFIER_INFO` : elles recopient les `.annotate({ identifier })`
  du schema, qui n'ont aucun effet sur les donnees.
- `CredentialId` est defini ici et pas ailleurs : `util_identifier.rs` est
  reclame par un autre agent, dont le fichier n'existait pas encore quand j'ai
  commence. Je n'importe rien de lui, mon fichier est autonome.

Aucun commit, aucun push.
