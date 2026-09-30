# Regles de portage TypeScript -> Rust (Ycode)

Ce fichier est la source de verite pour tout agent qui porte un fichier. Les
BRIEF-*.md a la racine du depot sont des variantes perimees : s'ils contredisent
ceci, c'est ceci qui gagne.

## Contexte

On reecrit en Rust le depot `opencode` (TypeScript). Le projet Rust est
`C:\Users\AI\Projects\Ycode\ycode-rs`. La source TypeScript est
`C:\Users\AI\Projects\Ycode\opencode`.

La cible est un portage **fiel**, pas une reinterpretation. La compatibilite
JSON avec le TypeScript d'origine est une exigence : le Rust doit produire
exactement les memes objets, avec les memes noms de champs, que le TypeScript
pourrait produire.

## Regles absolues

1. **Tu ne modifies QUE le fichier cible.** Aucun autre. En particulier tu ne
   touches pas a `mod.rs`, pas a `Cargo.toml`, pas a `lib.rs`. Si le fichier
   cible doit etre declare dans un `mod.rs`, signale-le dans ton rapport, ne le
   fais pas.

2. **Lis la source ENTIEREMENT** avant d'ecrire la moindre ligne. Pas de
   lecture diagonale. Les details qui cassent le portage sont a la fin des
   fichiers.

3. **Pas de toolchain Rust sur ce poste.** Ni `cargo`, ni `rustup`, ni cible
   MSVC. Ton code ne sera **ni compile ni teste** avant d'arriver sur la CI
   GitHub Actions. Sois d'autant plus rigoureux : chaque faute de frappe est un
   echec de compilation qui bloque tout le monde.

4. **Commentaires et documentation en francais SANS ACCENTS.** Contrainte du
   codebase, non negociable. Pas d'e dans les commentaires, pas dans les noms
   de tests.

5. **Au moins 3 tests `#[cfg(test)]`**, couvrant les cas limites que tu deduis :
   liste vide, un seul element, ordre inverse, valeurs par defaut. Les tests
   doivent avoir des noms en francais sans accents, en `snake_case`, et decrire
   le comportement et non l'implementation.

6. **Si la source n'est qu'un export vide ou un pur reexport, ecris un fichier
   minimal qui l'explique et ne fabrique rien.** Ne comble pas les trous par
   imagination. Un fichier de 15 lignes qui dit "ceci n'est qu'un reexport" vaut
   mieux qu'un fichier de 200 lignes plein de code invente.

## Traductions

| TypeScript | Rust |
|---|---|
| `Schema.Class` / `Schema.Struct` | `struct` derive `Serialize, Deserialize` |
| ADT a tag (`TaggedErrorClass`, `toTaggedUnion`) | `enum` avec `#[serde(tag = "...")]` |
| `Schema.optional` / `optionalKey` | `Option<T>` + `#[serde(skip_serializing_if = "Option::is_none")]` |
| `Effect.fn` / `Effect.gen` | fonction Rust pure. Pas d'`async` si ce n'est pas necessaire |
| `Context.Service` | `trait` |
| `Layer.succeed` | une implementation concrete du trait |
| `readonly Set<A>` | `BTreeSet<A>` (deterministe) |
| `readonly Map<K, V>` | `BTreeMap<K, V>` (deterministe) |
| `Schema.DateTimeUtc` | `i64`, millisecondes depuis l'epoch |

## Les deux pieges qui cassent le portage

### Le piege 1 : `?` contre `??`

Ce sont deux operateurs differents, et le TypeScript d'origine les melange.

- Le **ternaire** `x ? a : b` teste la **veracite**. En JavaScript, `""` est
  falsy, `0` est falsy, `null` est falsy. Donc `row.parent_id ? ... : undefined`
  transforme une chaine vide en `undefined`.
- Le **coalescent** `x ?? y` teste la **nullite**. Seuls `null` et `undefined`
  declenchent `y`. Une chaine vide **survit** et n'est PAS remplacee.

Une meme chaine vide doit donc disparaitre a un endroit et survivre a l'autre.
Traduire les deux operateurs par la meme fonction, c'est garantir une
divergence. Traduire le ternaire par un simple `.map()` sur un `Option`, c'est
aussi garantir une divergence : `Some("")` resterait `Some("")`.

### Le piege 2 : les noms de champs

Tout nom `camelCase` en TypeScript porte un `#[serde(rename = "camelCase")]`
**explicite** en Rust. C'est l'erreur d'incompatibilite la plus frequente de ce
portage, et elle est invisible de l'interieur du code Rust : elle n'apparait
qu'a l'echange avec le TypeScript.

`projectID` n'est pas `projectId`. `messageID` n'est pas `messageId`.
`workspaceID` n'est pas `workspaceId`. Ce sont des majuscules, pas des
minuscules. Verifie champ par champ.

## Rapports

Le livrable, c'est le fichier sur le disque. Ton reponse finale doit tenir en
trois lignes maximum : le chemin du fichier ecrit, le nombre de tests ecrits, et
**le point ou tu as le moins de confiance**. Ce dernier point est le plus
important : c'est ce que la relecture croisee ira verifier en premier. Ne
minimise pas tes doutes pour paraitre sure de toi.
