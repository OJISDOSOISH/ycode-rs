# swarm-provider_groq

===DEBUT===
fichier : src/swarm/provider_groq.rs
source  : packages/core/src/plugin/provider/groq.ts
taille  : 18962 octets
tests   : 15

CONFIANCE : moyenne
POINT FAIBLE : le type de l'evenement n'est pas dans ma source. Il vient du hook
partage `packages/plugin/src/v2/effect/aisdk.ts` (duplique dans
`packages/core/src/aisdk.ts` sous le nom `SDKEvent`), que j'ai redeclare LOCALEMENT
comme `SdkEvent { model, package, options, sdk }`. Les 12 autres agents qui
portent un fichier de `plugin/provider/` vont chacun redeclarer le meme evenement
et le meme contrat `ctx.aisdk.sdk(...)` : il y aura 13 doublons. C'est moi qui
devrait ceder si le lot cree un module commun, mais je ne peux pas le deplacer
(regle 1 : je ne touche qu'a mon fichier). Deuxieme doute, plus techniques :
`model: ModelV2Info` est rendu par un `serde_json::Value` opaque, donc sans
aucune validation, alors que le TS rejette un modele mal forme.
A VERIFIER : en priorite (1) que les quatre noms de champs sont bien
`model` / `package` / `options` / `sdk`, en les comparant a `SDKEvent` dans
`aisdk.ts:12-17` (test de nommage ligne 447) ; (2) que `sdk?: any` implique bien
`Option` + `skip_serializing_if` et non un champ obligatoire ; (3) que la
coercion `&mut CrochetGroq<M>` vers `&mut dyn CrochetSdk` et la borne
`M: ?Sized` (ligne 243) compilent, c'est le seul endroit du fichier ou je n'ai
pas pu eliminer une mecanique non triviale ; (4) que l'injection du module npm
par le trait `GroqModule` est acceptee comme portage fidele, plutot que de
considerer que le chargement dynamique etait hors perimetre.
===FIN===
