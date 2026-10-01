# swarm-provider_mistral

===DEBUT===
fichier : src/swarm/provider_mistral.rs
source  : packages/core/src/plugin/provider/mistral.ts
taille  : 11433 octets
tests   : 13

CONFIANCE : haute
POINT FAIBLE : la fabrique `createMistral` vient du paquet npm `@ai-sdk/mistral`, dont
le code JavaScript n'existe pas dans le depot Rust. Je ne l'ai pas inventee : dans
`on_sdk_event`, le second parametre est une fabrique injectee `FnOnce(&Value) -> Value`
qui tient lieu de `mod.createMistral(evt.options)`. Si l'integration finale prefere un
`trait` pour cette fabrique, ou un `serde_json::Value` opaque direct dans `evt.sdk`, c'est
la seule chose a redecider. Le reste du fichier (l'identite du plugin, le filtre sur le
nom de paquet, la transmission des options) est une transcription directe.
A VERIFIER : les quatre noms de champs de `SdkHookEvent` (`model`, `package`, `options`,
`sdk`). Ils sont des mots simples en minuscules, tous avec `#[serde(rename = ...)]`
explicite, et un test compare la chaine JSON exacte
(`{"model":null,"package":"@ai-sdk/mistral","options":{"name":"mistral"},"sdk":null}`)
ainsi que l'absence de la cle `sdk` quand le champ est absent. A verifier aussi que
`package` n'a pas ete renomme en `package_name` ou `package_npm` : `package` n'est pas un
mot reserve de Rust, c'est legal tel quel.

REMARQUES :
- `pub mod provider_mistral;` est deja present dans `src/swarm/mod.rs` (ligne 38). Rien a
  signaler de ce cote.
- Le source fait 15 lignes : `define({ id: "mistral", effect })` ou l'effet enregistre un
  seul crochet `ctx.aisdk.sdk`. Le champ `effect` etant une fonction, il n'a pas de
  representation JSON : le plugin est decoupe en une donnee (`MistralPlugin`, qui porte
  l'`id`) et le comportement (`on_sdk_event`).
- Mistral n'enregistre **que** ce crochet. Contrairement a `vercel.ts`, il n'ajoute aucun
  `ctx.catalog.transform`, aucun en-tete HTTP, aucune valeur par defaut. Je n'ai rien
  ajoute.
- Le test TypeScript `test/plugin/provider-mistral.test.ts` parle d'un nom de fournisseur
  `"mistral.chat"`. Cette normalisation est faite dans `core/src/aisdk.ts`, pas dans
  `mistral.ts` : elle n'est volontairement pas portee ici.
- Pas de piege `?` contre `??` : la source utilise une egalite stricte
  `evt.package !== "@ai-sdk/mistral"`. Un test verifie que la casse compte et que la chaine
  vide ne declenche rien, un autre que la fabrique n'est pas appelee pour un autre paquet.
- `test/provider-mistral.test.ts` (le second) teste l'interieur du paquet npm
  (`prompt_cache_key`, reasoning natif). Cela n'appartient pas a `mistral.ts`, rien n'en a
  ete porte.
- Fichier jamais compile : aucun toolchain Rust sur ce poste. Ecrit sans accent, verifie
  sur place : zero caractere non ASCII dans le fichier.

===FIN===