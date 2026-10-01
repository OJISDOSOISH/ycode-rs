# swarm-provider_alibaba

===DEBUT===
fichier : src/swarm/provider_alibaba.rs
source  : packages/core/src/plugin/provider/alibaba.ts
taille  : 13994 octets
tests   : 14

CONFIANCE : haute
POINT FAIBLE : `evt.sdk` vaut `any` en TS et recoit l instance renvoyee par `createAlibaba(evt.options)` ; j'ai remplace cette instance par une poignee `AlibabaSdk` (paquet, fabrique, options) qui DECRIT la liaison au lieu de l'instancier, parce que l'appel est un import dynamique d un paquet npm, impossible a compiler en Rust. C'est le seul ecart de mon portage, mais c'est un ecart de type, donc personne ne le voit dans le code.
A VERIFIER :
 1. Que la relecture accepte `AlibabaSdk` comme representation de `sdk?: any`. Si elle prefere `serde_json::Value`, c'est trois lignes a changer ; si elle prefere un trait d'objet SDK, c'est plus long mais plus fidele au `any`.
 2. Le `model` : la source le type `ModelV2Info` du SDK, que le crochet ne lit JAMAIS. Comme swarm-copilot_finish_reason l'a signale, `@ai-sdk/provider` n'est pas installe sur ce poste, donc ce type n'est pas consultable. Je l'ai donc garde opaque en `serde_json::Value` plutot que d inventer un struct. A valider comme choix, pas comme oubli.
 3. La compilation : aucune toolchain Rust sur la machine. Le point le plus risque du fichier est `apply(&self, event: &mut AlibabaSdkEvent)` qui passe `event` a `self.resolve(event)` attendu en `&AlibabaSdkEvent` : c'est un reborrow `&mut` vers `&`, ca compile, mais c'est le seul endroit ou j'ai joue avec le borrow checker.
 4. Cohérence avec les 17 autres `provider_*.rs` du lot (18 en tout, lignes 29 a 46 de mod.rs) : j'ai aligne `#[serde(default, skip_serializing_if = "Option::is_none")]` sur `provider_xai.rs`, qui traite le meme evenement `AISDKHooks.sdk`. Si la vague impose une forme unique pour cet evenement partage, il faudra peut-etre fusionner les deux representations plutot que de garder deux structs.
===FIN===
