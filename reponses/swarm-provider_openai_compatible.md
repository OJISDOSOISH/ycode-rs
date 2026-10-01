# swarm-provider_openai_compatible

===DEBUT===
fichier : src/swarm/provider_openai_compatible.rs
source  : packages/core/src/plugin/provider/openai-compatible.ts
taille  : 22809 octets
tests   : 16

CONFIANCE : haute
POINT FAIBLE : le type d'evenement `SdkEvent` est declare LOCALEMENT dans mon fichier alors qu'il est partage par les 30 plugins `provider/*.ts` (le vrai type est `AISDK.SDKEvent` dans `packages/core/src/aisdk.ts`, qui n'est claim par PERSONNE dans ce lot, verifie dans claims/). Les 19 autres agents `provider_*` vont chacun redeclarer le meme evenement, et les plugins qui lisent `evt.model` (cloudflare-workers-ai.ts:26, google-vertex.ts:89, variant.ts:31))n'ont pas de champ du tout chez moi. C'est un risque de doublon, pas un risque de nom de champ.
A VERIFIER : (1) que `includeUsage` reste ecrit avec cette casse exacte dans le dictionnaire d'options, cle par cle ; (2) que `PLUGIN_ID` et `PACKAGE` gardent bien le TIRET et ne sont jamais utilises comme identifiants Rust ; (3) le `#[serde(skip)]` sur `sdk`, choix que j'ai fait et qui demande un arbitrage.
===FIN===

## Ce que fait la source

Dix-sept lignes, une seule de logique. C'est un plugin de fournisseur **generique** :
il ne parle a aucun fournisseur en particulier. Il enregistre un seul crochet sur
`ctx.aisdk.sdk`, qui pour tout modele dont le paquet AI SDK **contient** la
sous-chaine `@ai-sdk/openai-compatible` :

1. force `options.includeUsage = true`, sauf si la valeur deja presente vaut
   exactement le booleen `false` ;
2. construit le fournisseur via `createOpenAICompatible(options)` et l'ecrit dans
   `evt.sdk`.

Le crochet commence par `if (evt.sdk) return`. Ce n'est pas une optimisation :
c'est une **reservation**. `aisdk.ts:179` parcourt les crochets dans l'ordre
d'enregistrement, et `cloudflare-workers-ai.ts:32` appelle le meme
`createOpenAICompatible` **avant** ce plugin dans `provider.ts` (ligne 44 contre
ligne 60). Premier arrive, premier servi. Test dedie.

## Piege 1 : `?` contre `??` -- le vrai piege de ce fichier

`if (evt.options.includeUsage !== false)` n'est **ni** un ternaire **ni** un
coalescent. C'est une inegalite stricte contre le litteral `false`. Le tableau
complet de ce que la condition fait :

| valeur de `includeUsage` | `!== false` | resultat |
|---|---|---|
| *(cle absente)* | vrai | ecrase par `true` |
| `false` | **faux** | **survit** |
| `true` | vrai | `true` (inchange) |
| `null` | vrai | **ecrase par `true`** |
| `0` | vrai | **ecrase par `true`** |
| `""` | vrai | **ecrase par `true`** |
| `[]`, `{}`, `42` | vrai | **ecrase par `true`** |

Un portage par `??` aurait laisse `null` en place. Un portage par un test de
veracite (`if (!options.includeUsage)`) aurait laisse `0` et `""` en place, et
aurait aussi ecrase `false` par `true`, ce qui est faux. Le code porte est :

```rust
pub fn include_usage_is_forced(options: &Options) -> bool {
    !matches!(options.get(INCLUDE_USAGE_KEY), Some(Value::Bool(false)))
}
```

Deux tests verrouillent les deux moities (`la_valeur_false_est_la_seule_qui_survit`
et `une_valeur_absorbante_vaut_mieux_que_faux`, ce dernier sur 7 valeurs).

## Piege 2 : les noms de champs, et le TIRET

Le nom de la source contient un tiret, et le tiret apparait a trois niveaux
differents. C'etait la consigne de la vague 2, j'ai traite les trois :

1. **Le module** : `openai-compatible.ts` devient `provider_openai_compatible`.
   Les soulins ne sont ici QUE dans l'identifiant de fichier. Aucune constante
   ni aucune chaine du fichier ne contient de soulin.
2. **Les chaines** : `PLUGIN_ID = "openai-compatible"` et
   `PACKAGE = "@ai-sdk/openai-compatible"` gardent le TIRET, parce que ce sont
   des donnees comparees a des chaines construites ailleurs. Deux tests
   verifient explicitement `contains('-')` et `!contains('_')` sur les deux.
3. **La cle d'option** : `includeUsage`. C'est le nom de champ le plus sensible du
   fichier, parce qu'il vit dans un dictionnaire libre (`Record<string, any>`)
   ou le compilateur Rust ne verifie rien. Il passe par la constante
   `INCLUDE_USAGE_KEY`, et un test verifie que `include_usage`, `IncludeUsage`
   et `Package` n'apparaissent pas dans le JSON produit.

Les trois champs de la structure (`package`, `options`, `sdk`) sont deja en
minuscules et en un seul mot dans le TS : **aucun `#[serde(rename)]` n'est
necessaire**, et un test le verifie quand meme.

## Ce que j'ai choisi, et pourquoi

- **`options` est `readonly` en TS mais le crochet le modifie.** Le `readonly`
  porte sur le *champ* de l'evenement, pas sur le *contenu* de l'objet :
  `evt.options.includeUsage = true` mute l'objet en place. D'ou un champ
  `options: BTreeMap<String, Value>` modifie sur place, et non reattribue.
- **`evt.package.includes(...)` est une recherche de sous-chaine**, pas une
  egalite. Volontaire cote TS (`google-vertex.ts:89` fait pareil). Une chaine
  vide ne contient rien, donc un nom de paquet vide ne declenche pas le
  plugin : test dedie.
- **Le `import()` dynamique n'a pas d'equivalent Rust.** Le paquet
  `@ai-sdk/openai-compatible` est du JavaScript, et `Cargo.toml` ne declare
  aucun SDK AI (regle 1 : je n'y touche pas). Le crochet prend donc la
  **fabrique en parametre** : `FnOnce(Options) -> Sdk`. Aucun comportement
  invente, l'appelant fournit ce que seul le SDK sait faire.
- **La fabrique recoit les options par valeur, donc une copie.** En TS la
  fabrique est une fermeture qui garde une reference sur le meme objet que
  `evt.options`. Divergence assumee et signalee dans le fichier : elle n'est
  observable que si quelqu'un modifie `evt.options` apres le retour du crochet,
  ce que le code source ne fait jamais.
- **`#[serde(skip)]` sur `sdk`.** En TS c'est un objet vivant avec des methodes
  (`aisdk.ts:224` appelle `sdk.languageModel(id)`), il ne peut pas survivre a un
  aller-retour JSON. Plutot que de fabriquer une forme serialisable pour `Sdk`,
  je l'exclus du JSON et je le dis dans un test. **C'est le point que je veux
  faire arbitrer** : une autre lecture peut preferer serialiser `Sdk` pour que
  le nom du champ `sdk` apparaisse dans le JSON.
- **Le champ `model` n'est pas porte.** `AISDK.SDKEvent` (`aisdk.ts:12`) porte
  aussi `model: ModelV2.Info`. Ce plugin ne le lit ni ne l'ecrit **jamais**.
  Definir le type ici serait inventer une forme, donc je ne le porte pas, et je
  m'assure qu'un evenement qui en contient un se lit quand meme (Serde ignore
  les champs en trop, comme le decodeur de Schema). Test dedie.
- **`package` reste obligatoire** a la deserialization (pas de `default`), parce
  que `readonly package: string` l'est en TS. `options` et `sdk` portent
  `default`. Test dedie des deux moities.

## Integration

- `pub mod provider_openai_compatible;` est deja present dans
  `src/swarm/mod.rs` (ligne 40). Je n'y ai pas touche.
- Aucune dependance ajoutee : `serde`, `serde_json` et `std` seulement, deja
  dans `Cargo.toml`.
- Aucun commit, aucun push.
- 0 octet non ASCII dans le fichier (verifie octet par octet).
