# swarm-pty_schema

===DEBUT===
fichier : src/swarm/pty_schema.rs
source  : packages/core/src/pty/schema.ts
taille  : 14264 octets
tests   : 16

CONFIANCE : moyenne

POINT FAIBLE : la source fait UNE ligne et ne definit rien
(`export { ID as PtyID } from "@opencode-ai/schema/pty"`). Comme aucun agent
du lot ne porte `packages/schema/src/pty.ts`, j'ai du DEFINIR `PtyID` plutot
que le reexporter, et recopier en prive le generateur
`packages/schema/src/identifier.ts` dont `create()` a besoin. C'est la que je
ne suis pas sur d'avoir bien fait : deux possibilities que la relecture doit
trancher.
  1. Semantique : le controle amont est `Schema.isStartsWith("pty")`, donc
     prefixe "pty" et NON "pty_". J'ai accepte la chaine nue "pty". Si le
     relecteur lit "prefixe pty_" quelque part, c'est moi qu'il faut corriger.
  2. `PtyID::ascending` renvoie `Result<PtyID, InvalidPtyID>` la ou le TS leve
     une exception. C'est un changement de signature, assume : `core/src/pty.ts`
     appelle `PtyID.ascending()` partout et devra ajouter un `?` ou un
     `unwrap()`. Personne ne porte ce fichier pour l'instant, mais il faut le
     savoir.

A VERIFIER dans cet ordre :
  - le piege `?` contre `??` : le TS teste `id === undefined`, donc
    `ascending("")` doit REFUSER la chaine vide, pas fabriquer un identifiant.
    Test dedie : `ascending_avec_une_chaine_vide_refuse_la_chaine_vide_au_lieu_de_fabriquer`.
  - le nom camelCase cote JSON : `packages/core/src/pty/ticket.ts` expose un
    champ `ptyID` (majuscules). Test dedie avec un struct local qui verifie
    `{"ptyID":"pty_abc"}`.
  - le format de sortie du generateur recopie : 26 caracteres = 12 chiffres
    hexadecimaux d'horodatage + 14 caracteres base 62, puis prefixe "pty_"
    pour un total de 30. Trois tests verifient la longueur, l'alphabet et les
    12 hexadecimaux.
  - ECART ASSUME : `crypto.getRandomValues` est remplace par un xorshift sans
    dependance, car `Cargo.toml` ne declare aucune crate de hasard et je ne
    touche pas a `Cargo.toml`. Le format est identique, seule la source du
    hasard change. Si la relecture veut `uuid` ( deja present, v7) ou une vraie
    crate de hasard, c'est un changement de 3 lignes.

AUTRE :
  - Serde ne revalide PAS le prefixe a la deserialisation : `PtyID` est un
    newtype `#[serde(transparent)]`, comme `QuestionId` dans `src/question.rs`.
    Une chaine sans "pty" entree par le JSON passe. L'invariant est impose a la
    construction, par `PtyID::new` et `PtyID::ascending`.
  - Duplication connue : si le paquet `@opencode-ai/schema` est porte un jour,
    ce fichier doit devenir un `pub use` et la recopie du generateur doit
    disparaitre. C'est ecrit en tete de fichier.
  - `src/swarm/mod.rs` ligne 27 declare deja `pub mod pty_schema;`. Je n'ai
    touche a aucun autre fichier. 0 octet non ASCII, 0 accent.
  - Aucun commit, aucun push.
===FIN===
