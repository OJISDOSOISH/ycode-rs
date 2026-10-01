# swarm-installation_version

===DEBUT===
fichier : src/swarm/installation_version.rs
source  : packages/core/src/installation/version.ts
taille  : 11430 octets
tests   : 16

CONFIANCE : haute
POINT FAIBLE : la source NE COMPARE AUCUNE VERSION, j'en ai donc pas ecrit ; et j'ai du transformer `InstallationLocal` (const booleen en TS) en fonction, parce que je ne parie pas sur la compilation de `==` entre `&str` dans un contexte constant.
A VERIFIER : (1) que `option_env!("OPENCODE_VERSION")` dans un initialiseur de `const` est bien compile par la CI, c'est le seul point que je n'ai pas pu tester ; (2) qu'on veut bien la fonction `installation_local()` plutot qu'une const ; (3) le reste du fichier n'a aucun serde, donc zero piege de nom de champ.
===FIN===

## Detail pour la relecture

### La source (8 lignes) ne fait que lire deux globales de build

```ts
declare global { const OPENCODE_VERSION: string; const OPENCODE_CHANNEL: string }
export const InstallationVersion  = typeof OPENCODE_VERSION  === "string" ? OPENCODE_VERSION  : "local"
export const InstallationChannel = typeof OPENCODE_CHANNEL === "string" ? OPENCODE_CHANNEL : "local"
export const InstallationLocal   = InstallationChannel === "local"
```

J'ai cherche : `grep semver|localeCompare|compare` sur `packages/core/src` ne trouve **aucune** comparaison de version. Les seuls usages de `InstallationVersion` sont de l'interpolation (`opencode/${InstallationVersion}` dans des `User-Agent`, `version: InstallationVersion` dans `/health`), et les seules comparaisons de canal sont `=== "local"` et `!== "latest"`, dans `packages/opencode/src/installation/index.ts:48,52` — **un autre fichier, revendique par un autre agent** (je ne l'ai pas porte, je le signale).

Donc le sujet de la mission ("si la source compare des versions, un comparateur est un excellent candidat") **ne se realise pas ici**. Conformement a la regle "ne fabrique rien", je n'ai pas ecrit de comparateur semver : il n'aurait aucune source, aucun appelant, et il serait lu comme du comportement officiel par les agents suivants. A la place j'ai verrouille en test les formes de chaine que la source manipule tel quel (prefixe `v`, prerelease, 2 ou 4 segments, version vide, version non numerique), puisque c'est la ou une version se ferait normaliser par megarde.

### Le portage

| TS | Rust |
|---|---|
| `typeof OPENCODE_VERSION === "string"` | `option_env!("OPENCODE_VERSION")` : `Some(..)` / `None` |
| ternaire sur le type | `valeur_ou_defaut` (`const fn`, `match` sur `Option`) |
| `InstallationVersion` / `Channel` | `const &str`, figee a la compilation |
| `InstallationLocal` | `installation_local()` + predicat testable `est_canal_local()` |

`option_env!` est le bon equivalent de la substitution esbuild : meme semantique sur les trois cas (definie / absente / definie mais vide), et la valeur reste figee a la compilation, ce qu'attendent les appelants. Lire `std::env::var` a l'execution aurait ete faux : la valeur pourrait changer au milieu d'un processus, ce que le TS ne peut pas faire.

### Le piege `typeof` (le point le plus interessant du fichier)

`typeof OPENCODE_VERSION === "string"` teste le **type**, pas la veracite, et il ne leve pas de `ReferenceError` sur une globale jamais declaree. C'est exactement ce qui rend le fichier utilisable quand il est charge depuis les sources (le cas de `installation.test.ts`), ou la substitution n'a pas eu lieu.

Consequence concrete : `InstallationVersion` peut valoir la chaine **vide**, et c'est voulu. Un portage par test de veracite (`.filter(|s| !s.is_empty()).unwrap_or("local")`) aurait produit "local" a la place. C'est le piege `?` contre `??` du contexte, et le test `une_version_vide_est_conservee_et_non_remplacee` verrouille le resultat. Idem pour `est_canal_local("")` qui renvoie `false` et non `true`.

### Deux reserves, a trancher en relecture

1. **`installation_local()` est une fonction, pas une `const`.** `==` entre `&str` n'est pas utilisable dans un contexte constant sur stable (l'operateur d'egalite de `str` n'est pas `const fn`). Je n'ai pas voulu poser un `const fn maison` a base de `as_bytes()` : c'est du code fragile pour une valeur que personne ne branche en `const`. Consequence : le seul appelant reel (`Installation.isLocal`, dans le fichier d'un autre agent) devra ecrire `installation_local()` la ou il ecrivait `InstallationLocal`.

2. **`option_env!` non teste.** Aucun toolchain sur ce poste. C'est le seul endroit du fichier ou je ne peux pas garantir la compilation d'un coup d'oeil. Le patron `const X: &str = match option_env!("FOO") { Some(v) => v, None => "local" };` est repandu, mais j ai passe le match dans une `const fn` pour que la logique soit couverte par mes tests. Si la CI refuse, le correctif est de re-inline le `match`.

### Integration

- `pub mod installation_version;` est **deja** dans `src/swarm/mod.rs` ligne 25. Je n'y ai pas touche.
- Aucun struct, aucun enum, aucun `serde` : le piege des noms de champs JSON est sans objet ici (aucun nom camelCase a renommer, les seules chaines sont `"local"`, `"OPENCODE_VERSION"`, `"OPENCODE_CHANNEL"`).
- Aucune dependance ajoutee, aucun autre crate requis, aucun fichier du lot utilise.
- Un doctest utilise `ycode::swarm::installation_version::valeur_ou_defaut`, donc il ne compile que si le module reste declare dans `mod.rs` (il l'est).
- 0 octet non ASCII, 278 lignes, accolades et parentheses equilibrees.
- Aucun commit, aucun push.
