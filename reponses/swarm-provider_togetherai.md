# swarm-provider_togetherai

===DEBUT===
fichier : src/swarm/provider_togetherai.rs
source  : packages/core/src/plugin/provider/togetherai.ts
taille  : 20930 octets
tests   : 14

CONFIANCE : moyenne
POINT FAIBLE : la source n'est PAS un objet de configuration litteral, c'est un plugin `Effect.fn`, donc j'ai du choisir un modele Rust qui n'existe pas en TypeScript : le `import()` dynamique de `@ai-sdk/togetherai` n'a aucun equivalent, et je l'ai remplace par une fabrique `Fn(&Options) -> Value` Injectee par l'appelant. Je ne suis pas sur que ce choix convienne au reste du lot, et je ne suis pas sur non plus que l'objet evenement `SdkEvent` (meme forme dans les 20 fichiers `plugin/provider/*.ts`) ne doive pas etre partage plutot que duplique dans chaque module.
A VERIFIER : (1) la syntaxe Rust n'a jamais ete compilee, aucun toolchain sur ce poste : verifier en premier `Box<SdkFactory>` / `&*create` / l'appel `create(&evt.options)` sur un `&dyn Fn`, et la closure `impl Fn(&Options) -> Value + 'static` du helper de test ; (2) le renommage du champ Rust `package_` avec `#[serde(rename = "package")]` : c'est le seul nom du fichier qui ne soit pas un mot simple, et `package` est un mot reserve de Rust, donc un autre agent a pu choisir `r#package` ou un nom different ; (3) le fait que `options` soit un champ OBLIGATOIRE de l'evenement alors que le reste du portage traite beaucoup de `options` en option.
===FIN===

## Ce que fait la source

Quinze lignes, dont trois de code utile :

```ts
export const TogetherAIPlugin = define({
  id: "togetherai",
  effect: Effect.fn(function* (ctx) {
    yield* ctx.aisdk.sdk(Effect.fn(function* (evt) {
      if (evt.package !== "@ai-sdk/togetherai") return
      const mod = yield* Effect.promise(() => import("@ai-sdk/togetherai"))
      evt.sdk = mod.createTogetherAI(evt.options)
    }))
  }),
})
```

- `define` renvoie son argument sans le modifier (`plugin/internal.ts:59`) : le plugin
  se reduit a un `id` et a un effet.
- Le callback est enregistre sur le crochet `aisdk.sdk`, dont l'evenement est
  `{ model, package, options, sdk? }` (`packages/plugin/src/v2/effect/aisdk.ts`).
- Le tri est une comparaison **stricte de chaines** (`!==`), pas un test de veracite.
- L'affectation `evt.sdk = ...` est **inconditionnelle**.

## Noms de champs, champ par champ

La forme de l'evenement vient de `AISDKHooks` (package `plugin`, `v2/effect/aisdk.ts:5`).
Les quatre noms ont ete relus un par un contre le `.d.ts` :

| TypeScript | Rust | `rename` pose |
|---|---|---|
| `readonly model: ModelV2Info` | `model: Value` | oui |
| `readonly package: string` | `package_: String` | oui, `"package"` |
| `readonly options: Record<string, any>` | `options: Options` | oui |
| `sdk?: any` | `sdk: Option<Value>` | oui, `"sdk"` |
| `readonly id: string` (plugin) | `id: String` | oui |

Aucun de ces noms n'est en camelCase : le piege de la vague 2 ne s'active donc pas
ici. Le seul vrai risque est `package`, mot reserve de Rust. Le test
`les_noms_de_champs_json_sont_ceux_du_typescript` verifie chaque nom positivement
**et** verifie que `Package`, `Options`, `Sdk`, `Model` et `package_` sont absents du
JSON produit, donc qu'aucune faute de frappe ne puisse passer inapercue.

Piege confirme, traite, et teste : `sdk?: any` devient `Option<Value>` avec `default`
et `skip_serializing_if = "Option::is_none"`. En JavaScript un champ a `undefined`
disparait de `JSON.stringify` ; le Rust doit faire pareil, sinon il ecrirait
`sdk: null`, que le TypeScript ne produit jamais. Test :
`un_sdk_absent_ne_produit_pas_de_cle_nulle`.

## Choix de portage

1. **`import()` dynamique non simule.** Il n'y a pas de paquet npm a charger a
   l'execution en Rust, et la consigne interdit d'inventer un chargeur de modules.
   Le point de branchement est donc explicite et injecte :
   `pub type SdkFactory = dyn Fn(&Options) -> Value`, et
   `TogetherAiPlugin::effect(&self, ctx, create)`.
2. **Pas d'async.** `Effect.fn` devient une fonction ordinaire ; dans le code de
   decision il n'y a aucune attente.
3. **Contexte reduit.** `PluginContext.aisdk` n'a qu'une cle utilisee ici (`sdk`).
   Le trait `AisdkContext` ne declare donc que `on_aisdk_sdk`, et le callback recoit
   `&mut SdkEvent` parce que `evt.sdk = ...` l'exige.
4. **`model` reste un `Value`.** `ModelV2Info` vient du SDK genere
   (`packages/sdk/js/src/v2/gen/types.gen.ts:4815`), il n'est pas porte par ce lot.
   L'inventer ici serait faux. **Attention a la relecture** : ce type contient
   `providerID`, en majuscules : c'est exactement le piege `sessionID`/`callID` de la
   vague 1, et il se reproduira chez l'agent qui le portera.
5. **Aucune divergence `?` / `??`.** La source n'a ni ternaire ni coalescent sur ce
   fichier, mais le piege reapparait sous une autre forme : `evt.sdk = ...` ne teste
   rien. Un `sdk` deja present, meme falsy (`0`), est ecrase. Un portage naif avec
   `if evt.sdk.is_some() { return }` ou `.map()` aurait change ce comportement.
   Test dedie : `un_sdk_deja_present_est_ecrase meme_sil_est_faux`.
6. **`set_sdk_if_matching` renvoie un `bool`.** Le TS ne retourne rien. C'est un
   supplement d'information pour les tests, documente comme tel dans le fichier ; il
   ne change aucun comportement.
7. **Aucune dependance ajoutee.** Le crate n'a que `serde` et `serde_json`, deja
   presents dans `Cargo.toml`, plus `std` (`Box`, `Arc`, `Mutex`, `AtomicUsize`) pour
   les tests. Rien d'autre, rien de nouveau a declarer.

## Integration

- `pub mod provider_togetherai;` est deja dans `src/swarm/mod.rs` ligne 42. Je n'y ai
  pas touche.
- Aucun commit, aucun push.
- 0 caractere non ASCII dans le fichier, verifie octet par octet.
- **Point d'integration pour l'agent principal** : les ~20 fichiers
  `plugin/provider/*.ts` ont la meme structure (un `id`, un crochet `aisdk.sdk`, un
  `import()` dynamique, une fabrique de SDK). Chacun de mes homologues va donc
  redeclarer `SdkEvent`, `Options`, `AisdkContext` et `SdkFactory` dans son module.
  Aucun conflit Rust a l'etat (ce sont des modules prives, pas des reexports), mais
  c'est de la duplication. Un module partage du type `plugin_aisdk.rs` serait le bon
  geste. Je ne l'ai pas cree : regle 1, un seul fichier. Si l'integration veut
  centraliser, c'est mon fichier qui doit ceder, pas l'inverse.

## Tests

14 tests. Les 5 premiers sont le comportement de la source, les suivants les noms de
champs et les entrees invalides.

| Test | Ce qu'il verrouille |
|---|---|
| `un_evenement_du_paquet_togetherai_recoit_le_sdk_construit` | le chemin nominal, fabrique appelee une fois |
| `un_evenement_d_un_autre_paquet_est_laisse_entierement_intact` | le tri, et la fabrique **non** appelee |
| `les_options_de_l_evenement_sont_passees_telles_quelles_a_la_constructrice` | `evt.options` transmis intact, cles `apiKey` et `baseURL` incluses |
| `un_sdk_deja_present_est_ecrase meme_sil_est_faux` | l'affectation inconditionnelle, piege `?` contre `??` |
| `un_nom_de_paquet_vide_ne_correspond_pas` | chaine vide : comparaison stricte, pas de veracite |
| `un_seul_crochet_est_enregistre_et_chaque_evenement_passe_par_lui` | un enregistrement par plugin, ordre des evenements |
| `les_noms_de_champs_json_sont_ceux_du_typescript` | les 4 noms, plus les 5 variantes fautives absentes |
| `l_evenement_se_relit_depuis_le_meme_json_quil_a_ecrit` | aller-retour JSON, y compris options non vides et `sdk` present |
| `un_evenement_sans_nom_de_paquet_est_refuse` | `package` obligatoire, pas de chaine vide fabriquee |
| `un_evenement_sans_options_est_refuse` | `options` obligatoire, comme dans `AISDKHooks` |
| `un_sdk_absent_ne_produit_pas_de_cle_nulle` | pas de `sdk: null`, exactement 3 cles |
| `le_plugin_s_enregistre_sous_le_nom_togetherai` | `id` et sa casse |
| `le_nom_du_paquet_vise_est_ce_du_typescript` | le `@` devant `ai-sdk` |
| `seul_le_sdk_change_les_autres_champs_restent_intacts` | `model`, `options` et `package` ne bougent pas |
