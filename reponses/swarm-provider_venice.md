# swarm-provider_venice

===DEBUT===
fichier : src/swarm/provider_venice.rs
source  : packages/core/src/plugin/provider/venice.ts
taille  : 17284 octets
tests   : 19

CONFIANCE : haute
POINT FAIBLE : l'`import()` dynamique du paquet npm n'a pas d'equivalent Rust, donc j'ai passe la fabrique `createVenice` en parametre de `sdk_hook(&mut event, create)` au lieu de l'importer. Le comportement observable est intact, mais la signature n'est pas imposable par la source : c'est un choix, pas une deduction.
A VERIFIER : (1) que l'injection de la fabrique est acceptee, ou qu'un chargement de plugins fournit lui-meme `createVenice` et que `sdk_hook` devient inutile ; (2) aucun nom de champ a renommer sur ce fichier, c'est verifie par test, mais le fait que `evt.model` reste un `serde_json::Value` opaque est un choix a valider ; (3) `options` et `model` sont rendus obligatoires au decodage, comme en TS.

CE QUE FAIT LA SOURCE (15 lignes, un seul contenu)
Le plugin `venice` enregistre un unique crochet sur `ctx.aisdk.sdk` (crochet `sdk` de `AISDKHooks`, `packages/plugin/src/v2/effect/aisdk.ts`). Le crochet :
1. `if (evt.package !== "venice-ai-sdk-provider") return` ;
2. `const mod = yield* Effect.promise(() => import("venice-ai-sdk-provider"))` ;
3. `evt.sdk = mod.createVenice(evt.options)`.

Ce n'est PAS un objet de configuration litteral : la consigne de la mission ("si la source ne fait qu'un objet litteral, un struct suffit") ne s'applique pas. Il n'y a aucune donnee statique a porter, seulement l'evenement que le crochet recoit et modifie.

PORTAGE
- 3 constantes : `PLUGIN_ID = "venice"`, `PACKAGE = "venice-ai-sdk-provider"`, `FACTORY = "createVenice"`.
- `struct Plugin { id }` : le descripteur. Le champ `effect` du TS est une fonction, sans forme en Rust ; il est devenu `sdk_hook`.
- `struct SdkEvent { model, package, options, sdk }` : l'evenement du crochet, en `Serialize, Deserialize`. C'est le seul struct serialise du fichier, donc le seul expose au piege des noms de champs.
- `enum SdkOutcome { Ignore, Created }` : ce que le crochet a fait, pour rendre le `return` de la ligne 9 observable.
- `fn handles(package) -> bool` : la comparaison stricte, factorisee.
- `fn sdk_hook(&mut event, create) -> SdkOutcome` : le crochet.
- Aucun `async`, aucune concurrence, aucun `Effect` : le hook est un retour nu et une affectation, donc une fonction pure.

NOMS DE CHAMPS (verification demandee par la mission)
Les quatre noms du TS sont des mots uniques en minuscules : `model`, `package`, `options`, `sdk`. Identiques en Rust, donc **aucun `#[serde(rename)]` n'est necessaire**. Verifie quand meme par `les_noms_de_champs_json_sont_ceux_du_typescript`, qui compare la liste triee des cles (`["model","options","package"]`) et refuse `sdk_`, `Package`, `OPTIONS`. Les noms genuinely camelCase du fichier sont dans les CLES D'OPTIONS, qui viennent de la configuration utilisateur : `la_fabrique_recoit_les_options_telles_quelles` verifie que `apiKey` et `baseURL` arrivent intacts et que ni `api_key` ni `ApiKey` n'apparaissent.

PIEGE `?` CONTRE `??`
Aucun des deux operateurs n'apparait dans la source : le seul test est `!==` sur une chaine. Conséquence portee : nom de paquet vide rejete, casse comptee, espaces comptes. Trois tests dedies (`la_casse_du_nom_du_paquet_compte`, `un_espace_en_fin_de_nom_de_paquet_rompt_la_correspondance`, `un_nom_de_paquet_vide_est_rejete_comme_tout_autre`). Si la relecture prefere un traitre veracite, c'est une divergence : la chaine vide serait acceptee et construirait un SDK Venice pour un evenement sans paquet.

LE `return` DE LA LIGNE 9
Piege de comportement, pas de nom. La source rend la main **sans rien remettre a zero** : un `sdk` deja pose par un crochet enregistre avant reste en place. Symetriquement, quand le paquet correspond, l'affectation ecrase la valeur d'avant. Deux tests pour ces deux moities (`un_sdk_deja_presente_survive_a_un_paquet_etranger`, `un_sdk_deja_presente_est_remplace_par_la_fabrique_venice`). Troisieme test : sur un paquet etranger la fabrique **n'est pas appelee** (compteur a `Cell`), parce que c'est le seul symptome observable du court-circuit avant le `import()`.

CHAMPS OPTIONNELS
`sdk?: any` devient `Option<Value>` avec `#[serde(default, skip_serializing_if = "Option::is_none")]`, pour ne jamais ecrire `sdk: null` que le TS ne produit pas (`un_sdk_absent_ne_serialise_pas_une_cle_nulle` verifie aussi que l'objet fait bien 3 cles). A l'inverse `model` et `options` sont obligatoires en TS, donc ici aussi : deux tests verifient qu'un evenement sans l'un des deux est refuse au decodage plutot que complete par defaut.

ETAT DE LA MACHINE
Aucun compilateur Rust sur ce poste (regle 2), donc rien n'a pu etre compile. Relecture manuelle faite sur les points sensibles du fichier :
- `create(&event.options)` puis `event.sdk = Some(created)` : l'emprunt de `&event.options` finit a la fin de l'instruction, l'affectation mutable passe ensuite. NLL, aucune copie defensive.
- les tests utilisent `Cell<usize>` pour compter les appels de la fabrique, et `Cell<Options>` pour capturer les options recues ; `Options::clone(options)` est ecrit en forme explicite plutot que `options.clone()`, pour ne pas dependre de la resolution de methode sur une reference.
- 0 octet non ASCII dans le fichier (verifie octet par octet).

FICHIERS VOISINS UTILES AUX AUTRES FOURNISSEURS
Le crochet `aisdk.sdk` est commun a tous les `plugin/provider/*.ts`. La forme de son evenement est dans `packages/plugin/src/v2/effect/aisdk.ts` :
```
sdk:      { readonly model: ModelV2Info, readonly package: string, readonly options: Record<string, any>, sdk?: any }
language: { readonly model, readonly sdk: any, readonly options, language?: LanguageModelV3 }
```
Donc les 8 autres agents du lot (`provider_groq`, `provider_cohere`, `provider_google`, `provider_mistral`, `provider_deepinfra`, `provider_gateway`, `provider_alibaba`) ont le MEME evenement a porter, avec les memes noms de champs en minuscules. Deuxieme crochet possible : `aisdk.language`, qui ecrit `evt.language` a partir de `evt.sdk.responses(...)` — la ou la porte se pose sur `model.providerID`.

Les 7 autres fichiers du lot (`provider_groq`, `provider_cohere`, `provider_google`, `provider_mistral`, `provider_deepinfra`, `provider_gateway`, `provider_alibaba`) ont tous ete lus. Ils sont identiques ligne pour ligne a venice.ts : il ne change que trois chaines, `id`, le nom du paquet, et le nom de la fabrique. Aucun nom de champ camelCase nulle part. A noter pour `provider_google` : sa fabrique s'appelle `createGoogleGenerativeAI`, pas `createGoogle`, alors que tous les autres sont `create<Id>`.

Seule divergence dans tout le dossier : `vercel.ts` (27 lignes) enregistre un deuxieme crochet, `ctx.catalog.transform`, qui ajoute les headers `http-referer` et `x-title` sur `provider.request.headers` pour tout fournisseur dont `api.package === "@ai-sdk/vercel"`. C'est le seul fichier du lot qui touche le catalogue.

INTEGRATION
`pub mod provider_venice;` est deja present ligne 43 de `src/swarm/mod.rs`. Je n'y ai pas touche, conformement a la regle 1. Aucune dependance a un autre module du lot : `serde`, `serde_json` et `std` seulement, donc ce fichier compile isolement. `Cargo.toml` n'a pas besoin d'etre modifie. Aucun commit, aucun push.

===FIN===
