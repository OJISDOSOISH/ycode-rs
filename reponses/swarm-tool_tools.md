# swarm-tool_tools

===DEBUT===
fichier : src/swarm/tool_tools.rs
source  : packages/core/src/tool/tools.ts
taille  : 10456 octets (240 lignes)
tests   : 9

CONFIANCE : haute
POINT FAIBLE : `Tool.AnyTool` et `Tool.RegistrationError` sont declares dans
`tool/tool.ts`, qui n'est pas porte dans ce lot, et `src/tool.rs` est une autre
conception qui ne contient ni l'un ni l'autre. Je les ai donc declares localement,
dans mon fichier, pour garder la signature du `trait` exacte. Si le portage de
`tool/tool.ts` atterrit plus tard, il y aura un doublon a resoudre, et c'est le
seculier de mon fichier qui devrait disparaitre.
A VERIFIER : (1) ce doublon `AnyTool` / `RegistrationError`, et le fait que j'ai
refuse d'introduire derives `Serialize` sur `RegistrationError` alors que le
`Schema.TaggedErrorClass` d'origine produit un objet `{_tag, name, message}` --
je n'ai pas serialise parce que l'erreur ne sort jamais en JSON dans le code
source, mais une relecture peut juger que le tag doit etre produit ; (2) le
choix de nommer le `trait` `Service` (comme la classe TS et comme le precedent
`src/core/session/execution.rs`) plutot que `Tools` (comme le namespace de la
ligne 1) ; (3) la disparition de `Scope.Scope` : l'exigence de duree de vie est
reportee sur le `&mut self` de l'appelant, ce qui n'a pas de traduction
mecanique et n'est donc pas verifiable a la lecture.

Note d'integration : `pub mod tool_tools;` est deja present dans
`src/swarm/mod.rs` ligne 28. Je n'y ai pas touche. Aucun commit, aucun push.
Aucun nom de champ camelCase en cause dans ce fichier (`name` et `message`
seulement), donc aucun `#[serde(rename)]` a poser.
===FIN===
