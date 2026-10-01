# swarm-provider_perplexity

===DEBUT===
fichier : src/swarm/provider_perplexity.rs
source  : packages/core/src/plugin/provider/perplexity.ts
taille  : 13604 octets
tests   : 7

CONFIANCE : haute
POINT FAIBLE : le champ `sdk` ne contient pas un SDK fonctionnel mais la trace de l'appel (`factory` + `options`), parce que le paquet npm `@ai-sdk/perplexity` n'est pas installe sur cette machine (ni lui, ni `@ai-sdk/provider` : verifie) et n'a pas d'equivalent Rust. C'est le seul endroit ou j'ai du choisir une representation plutot que de transcrire. La consequence a assumer est qu'un appelant qui veut `sdk.languageModel(id).provider` n'a rien a appeler : ce comportement-la vit dans le paquet npm, pas dans le fichier porte, et je ne l'ai pas invente. Verifier aussi `model: serde_json::Value`, qui remplace le type `ModelV2Info` de `@opencode-ai/sdk`, absent de ce lot.
A VERIFIER :
  1. `package` est-il vraiment utilisable comme nom de champ Rust ? Oui : `package` n'est ni mot cle ni mot reserve en Rust (aucune edition reservee ne le contient), et un test serialise le payload pour prouver que la cle JSON est bien `package` et non `r#package`/`Package`. C'est le piege de noms de champs le plus probable de ce fichier, et il est ecarte.
  2. Les quatre noms de champs du payload (`model`, `package`, `options`, `sdk`) sont des mots uniques en minuscules, donc **aucun `#[serde(rename)]` n'est necessaire**. Verifie champ par champ, et verrouille par `les_noms_de_champs_serialises_sont_ceux_du_typescript`. Ne pas ajouter de rename "par habitude" : ce serait un bruit qui cache une future vraie erreur.
  3. `REGISTERED_HOOKS: [&str; 1] = ["sdk"]` est la seule surface ajoutee qui ne soit pas un renommage direct de la source. Elle encode un fait reel et testable (ce plugin n'enregistre que `aisdk.sdk`, jamais `aisdk.language`) mais c'est mon choix, pas celui du TS. A confirmer ou a supprimer.
  4. Non compile : aucun toolchain Rust sur ce poste. Le code le plus douteux est `on_aisdk_sdk`, ou j'ai volontairement passe par une variable locale (`let options = event.options.clone();`) au lieu d'ecrire `event.sdk = Some(create_perplexity(event.options.clone()))`, pour ne pas dependre des emprunts a deux phases. C'est compilable a coup sur, mais un peu moins elegant ; si la revue prefere la forme courte, c'est un changement d'une ligne.
===FIN===

## Ce que fait reellement la source

Quinze lignes, et **ce n'est pas** un objet de configuration litteral : c'est un
plugin. Le mission annoncait le cas "un struct" ; ce n'est pas celui-la, donc
voici le contenu exact.

```ts
export const PerplexityPlugin = define({
  id: "perplexity",
  effect: Effect.fn(function* (ctx) {
    yield* ctx.aisdk.sdk(Effect.fn(function* (evt) {
      if (evt.package !== "@ai-sdk/perplexity") return
      const mod = yield* Effect.promise(() => import("@ai-sdk/perplexity"))
      evt.sdk = mod.createPerplexity(evt.options)
    }))
  }),
})
```

Deux choses, et rien d'autre : un identifiant, et un gestionnaire qui filtre sur
`evt.package` puis ecrase `evt.sdk`.

## Ce qui est porte, et ce qui ne l'est pas

| Element TS | Traitement Rust |
|---|---|
| `id: "perplexity"` | `pub const PLUGIN_ID` |
| `"@ai-sdk/perplexity"` | `pub const AI_SDK_PACKAGE` |
| `mod.createPerplexity` | `pub const PERPLEXITY_FACTORY` + `SdkInstance::create_perplexity` |
| `yield* ctx.aisdk.sdk(...)` | `pub fn on_aisdk_sdk(&mut AisdkSdkEvent)` + `REGISTERED_HOOKS` |
| `evt.package`, `evt.options`, `evt.sdk`, `evt.model` | champs de `AisdkSdkEvent` |
| `import("@ai-sdk/perplexity")` | **non porte**, paquet absent de la machine |

`Effect.fn`, `yield*` et le `Scope` n'ont pas de traduction : le gestionnaire
est une fonction Rust qui modifie son evenement sur place, ce qui est
exactement ce que fait la source (`evt.sdk = ...` est une ecriture, pas un
retour).

## Le test TypeScript existe, et je m'en suis servi

`packages/core/test/plugin/provider-perplexity.test.ts` (122 lignes) documente
le comportement attendu, dont un cas que j'ai repris tel quel : le paquet
`"@ai-sdk/perplexity-compatible"` doit etre **ignore**. C'est le genre de
proche-du-coup que la comparaison par prefixe aurait rate. Ma table de rejet
couvre ce cas plus `"perplexity"` seul, la casse differente, et l'espace en
tete et en queue.

Ce test verifie aussi `result.sdk.languageModel("sonar").provider === "perplexity"`,
y compris pour un fournisseur `custom-perplexity`. Ce comportement **n'est pas
dans le fichier porte** : il est dans `createPerplexity`, donc dans le paquet
npm. Je ne l'ai pas porte. C'est dit explicitement dans le fichier .rs, pour
que personne ne le cherche dans le vide.

## Les deux pieges du contexte, traites

**`?` contre `??`** : aucun ternaire, aucun coalescent dans la source. Le seul
test de veracite est une comparaison de chaine stricte, donc la chaine vide est
rejetee, et ne doit pas etre traitee comme un paquet absent tolerable. Test
dedie.

**Noms de champs** : les quatre champs sont des mots uniques en minuscules. Le
piege est **absent**, et c'est dit dans l'en-tete du fichier pour que la
relecture ne le cherche pas. Un test serialise le payload et compare les cles
une a une, avant et apres remplissage de `sdk`, plus un aller-retour de
deserialization quand la cle `sdk` manque.

## Integration

- `pub mod provider_perplexity;` est deja `src/swarm/mod.rs` ligne 41. Je n'y
  ai pas touche.
- Aucune dependance ajoutee : `serde` et `serde_json` sont deja dans
  `Cargo.toml`.
- Aucun commit, aucun push.
- Fichier autonome : il ne reference aucun autre module du lot, donc il ne peut
  pas entrer en collision avec les 19 autres `provider_*.rs`. Chacun definira
  son propre `AisdkSdkEvent`, ce qui est dupliquation assumee mais sans risque
  de compilation. Si l'integrateur prefere un module partage, c'est un
  regroupement de code, pas une correction.

## Signalement pour les 19 autres agents du lot

Les fichiers `plugin/provider/*.ts` sont presque tous le meme gabarit
`define({ id, effect: ctx.aisdk.sdk(...) })` ou `ctx.catalog.transform(...)`.
Trois constats qui les interessent :

1. Le nom de champ **`package`** apparait partout dans ce gabarit. Ce n'est pas
   un mot cle Rust, mais il ressemble a un mot reserve : c'est le piege de noms
   le plus piege du lot, alors qu'il n'en est aucun.
2. **La comparaison est une egalite stricte**, jamais un `startsWith`. Un
   `"@ai-sdk/xxx-compatible"` doit etre rejete. Si vous utilisez `starts_with`,
   vous inversez le test du fichier TypeScript.
3. `@ai-sdk/provider` et les paquets `@ai-sdk/*` **ne sont pas installes** sur
   cette machine (verifie par moi, confirme par swarm-copilot_finish_reason).
   Le comportement interne de chaque `create*` est donc deduit, jamais lu. Ne
   pretendez pas l'avoir consulte, et n'inventez pas sa logique interne.
