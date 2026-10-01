# swarm-provider_anthropic

===DEBUT===
fichier : src/swarm/provider_anthropic.rs
source  : packages/core/src/plugin/provider/anthropic.ts
taille  : 33038 octets
tests   : 15

CONFIANCE : moyenne

POINT FAIBLE : la source ne declare aucun type, tout vient de paquets externes absents
du lot (`@opencode-ai/sdk/v2/types`, `@opencode-ai/plugin/v2/effect`), donc j'ai du
ecrire des formes locales minimales. Le risque n est pas le comportement, il est dans
`ProviderV2Info` : j ai suivi le type **genere** par le serveur
(`packages/sdk/js/src/v2/gen/types.gen.ts` lignes 4853-4880) et treats exactement, ou
je le declare, que ce qu il dit. Si le vrai `Catalog.Service` de `packages/core` produit
`settings` absent pour la variante `native`, ma deserialization echoue. A verifier en
priorite.

A VERIFIER :
1. **`integrationID`** : c est le piege du lot sur ce fichier, deux majuscules. Champ
   `integration_id` avec `#[serde(rename = "integrationID")]` ligne 198. Test dedie
   `les_noms_de_champs_serialises_sont_ceux_du_typescript` qui verifie aussi que
   `integrationId` et `integration_id` sont ABSENTS du JSON. A verifier aussi que le
   nom vient bien du type genere et pas d`oc_provider.claim.json` (`packages/core/src/
   provider.ts`, qui n est qu une liste de plugins, aucun type).
2. **`#[serde(tag = "type")]` sur des variantes STRUCTURE, pas `newtype`.** Si la
   relecture prefere `enum Api { Aisdk(ProviderAisdk), Native(ProviderNative) }` avec
   des structs qui portent elles-memes un champ `type`, serde ecrira deux fois la cle
   `type` dans le JSON. J ai choisi les variantes structure justement pour l eviter, et
   le test compte les cles (`api.len() == 2`) pour le prouver. C est le meme sujet que
   le `#[serde(untagged)]` signale par swarm-integration_connection, mais ici l union
   EST taggee dans le TypeScript, donc `untagged` serait faux.
3. **`r#package` + `#[serde(rename = "package")]`.** Le renommage est redondant
   (serde retire deja `r#`), je l ai garde pour que le nom de la cle soit visible. Le
   test verifie que `r#package` n apparait PAS dans le JSON.
4. **`await import("@ai-sdk/anthropic")` n a pas d equivalent Rust.** C est le seul
   point non litteral du fichier. Je ne l ai pas simule : `apply_aisdk_sdk` construit un
   `AnthropicSdk { package, options }`, c est a dire qu elle retient les options qui
   auraient ete passees a `createAnthropic`. A valider, ou a remplacer par une fabrique
   injectee si l agent principal prefere.
5. **Choix de portage a valider** : `catalog_transform` ne touche que les
   fournisseurs dont `api.package()` vaut `@ai-sdk/anthropic`. Les deux `continue` de la
   source sont donc fusionnes en un predicat. Aucune condition ajoutee, notamment
   `disabled` n est pas teste : test dedie
   `un_fournisseur_desactive_recoit_len_tete_comme_les_autres`.
6. Champs laisses de cote, tous documentes dans le fichier : `CatalogProviderRecord
   .models` et `SdkHookEvent.model` (le plugin ne les lit pas et `ModelV2Info` n est
   porte par personne dans le lot), `CatalogDraft.provider.get/remove` et tout
   l objet `model`, `PluginContext` lui-meme. `Effect.fn` et `yield*` deviennent deux
   appels de fonctions pures que l appelant declenche.

Integration : `pub mod provider_anthropic;` est deja la ligne 30 de `src/swarm/mod.rs`,
je n y ai pas touche. `serde` et `serde_json` sont deja dans `Cargo.toml`, rien a
ajouter. Aucun commit, aucun push.

Fait utile aux 19 autres agents `provider_*` du lot : ces 19 fichiers ont la meme
forme, `{ id, effect }` ou l effet enregistre un ou deux callbacks
`ctx.aisdk.sdk` / `ctx.catalog.transform`. Deux conclusions reutilisables :
(a) aucune de ces sources ne contient de nom de champ, le piege `integrationID` vient
de `ProviderV2Info` dans le SDK, donc il n existe que la ou un catalogue est manipule ;
(b) les constantes a comparer sont toujours `@ai-sdk/<nom>` et le nom du plugin
souvent identique au nom de fichier. Attention pour les fichiers qui filtrent sur un
sous-texte, genre `provider_openai_compatible.ts` qui fait
`evt.package.includes("@ai-sdk/openai-compatible")` : c est un `includes`, pas une
egalite, et c est le seul fichier du lot ou un `evt.sdk` deja present doit etre teste
avant d ecrire.
===FIN===
