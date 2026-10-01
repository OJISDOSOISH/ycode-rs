# swarm-provider_gateway

===DEBUT===
fichier : src/swarm/provider_gateway.rs
source  : packages/core/src/plugin/provider/gateway.ts
taille  : 18644 octets (454 lignes)
tests   : 9

CONFIANCE : haute
POINT FAIBLE : la source est un `define({ id, effect })` ou tout le comportement tient en trois instructions, dont une est un `import()` dynamique d'un paquet npm. J'ai remplace cet import par un trait `GatewayModule` que l'integrateur devra implementer : si le lot prefere une autre couture (par exemple un nom de fonction passe en parametre, ou un `Box<dyn Fn>` plutot qu'un trait), la forme de l'API change, meme si `on_sdk` reste identique. Deuxieme point, non compile : j'ai du corriger trois erreurs a la main (voir A VERIFIER 1), donc une quatrieme m'a peut-ete echappe.
A VERIFIER : (1) priorite absolue, `fn run_sdk(&mut self, event: &mut SdkEvent) -> &mut SdkEvent` que j'avais ecrit d'abord ne compile PAS : l'elision donne au retour la duree de vie de `&mut self`, et retourner le second argument exige `event: 'self` que la signature ne garantit pas. Je l'ai fait returning `()` et les tests lisent `event` apres l'appel. A verifier que l'integrateur n'a pas reintroduit la forme a double duree de vie ; (2) `#[derive(Debug, Default)]` sur `AisdkHooks` ne compile pas non plus, car `Box<dyn FnMut(..)>` n'implemente pas `Debug` - j'ai retire `Debug` ; (3) la borne `M: GatewayModule` n'est posee que sur le second bloc `impl` de `GatewayPlugin` (celui de `effect`), `new` reste non borne ; (4) verifier qu'aucun `#[serde(rename)]` n'est requis, un par un.

NOMS DE CHAMPS, mission de la vague 2 :
- Les quatre champs de `SdkEvent` sont `model`, `package`, `options`, `sdk`. Ce sont quatre mots uniques en minuscules, deja identiques en Rust, et **aucun n'est camelCase**. Zero `#[serde(rename)]` n'est pose dans le fichier (verifie : 0 attribut reel, 2 mentions dans la documentation).
- Le test `les_noms_de_champs_serialises_sont_ceux_du_typescript` verrouille la sortie : 3 cles (`model`, `package`, `options`) quand `sdk` est absent, 4 cles quand il est present, puis aller-retour de deserialization. Il verifie aussi que `sdk` **disparait** du JSON quand il est `None`, comme le `sdk?: any` du TypeScript.
- Le test de la chaine verifie le comportement du `evt.sdk = ...` : ecriture inconditionnelle, un `sdk` deja present est ecrase (un `get_or_insert_with` serait faux).

DETAIL :
- Source relue entiere, 15 lignes. `define` vient de `plugin/internal.ts` ligne 59 et se resume a `return plugin` : fonction d'identite, rien a porter. C'est le seul "objet de configuration litteral" du fichier, et son champ `effect` est une fonction, pas une donnee.
- La forme de l'evenement n'est pas devinee : elle est lue dans `packages/core/src/aisdk.ts` lignes 12-17 et confirmee, a l'identique, dans `packages/plugin/src/v2/effect/aisdk.ts` lignes 5-11. `model` est un `ModelV2.Info` qui n'est defini ni dans ce fichier ni dans un fichier que je porte, et que le gestionnaire ne lit jamais : il est donc transporte comme `serde_json::Value` opaque, et non materialise en struct invente.
- `Record<string, any>` devient `BTreeMap<String, Value>` (table des conversions : `readonly Map<K, V>` -> `BTreeMap`, donc ordre deterministe).
- `Effect.fn` devient une fonction Rust pure, sans `async` : l'import dynamique est le seul effet de bord et il est represente par le trait.
- Piege `?` contre `??` : le source ecrit `evt.package !== "@ai-sdk/gateway"`, une comparaison stricte de chaines, PAS un ternaire. La chaine vide `""` ne correspond donc pas et le rappel sort. Aucun `Option::is_none`, aucun test de veracite. Le test `un_paquet_ressemblant_nest_pas_confondu_avec_le_paquet_attendu` couvre 6 cas, dont la chaine vide, un suffixe en trop, une espace final et une difference de casse.
- Point de semantique ajoute : la chaine de crochets est la boucle de `aisdk.ts` ligne 178, qui passe **le meme** objet `event` a chaque rappel. Le `return` du gestionnaire gateway ne sort donc que de son propre rappel et n'interrompt pas les autres plugins. Reproduit dans `AisdkHooks::run_sdk` et teste.
- `AisdkHooks` ne porte que la famille `sdk`. La famille `language` existe dans la source mais aucun plugin fournisseur ne s'y inscrit, donc elle n'est pas portee (rien d'invente).
- Etat du paquet : `@ai-sdk/gateway` est declare en `packages/core/package.json` ligne 71 (version `3.0.191`) mais **n'est pas installe** sur cette machine (`node_modules\@ai-sdk` est absent, comme l'a signale swarm-copilot_finish_reason pour `@ai-sdk/provider`). La signature reelle de `createGateway` n'est donc pas verifiable, seule l'appel est lisible. Le trait reprend exactement l'appel, rien de plus.
- Tests (9) : paquet attendu, paquet different, 6 paquets ressemblants, transmission exacte des options, options vides qui donnent quand meme un SDK, ecrase d'un SDK deja present, noms de champs serialises, chaine de crochets qui continue, identifiant et enregistrement du plugin.
- Integration : `pub mod provider_gateway;` est deja dans `src/swarm/mod.rs` ligne 34. Je n'y ai pas touche. Aucune dependance ajoutee, `serde` et `serde_json` sont deja dans `Cargo.toml`.
- Aucun accente, aucun caractere non-ASCII (verifie : 0 occurrence sur 18644 octets). Accolades et parentheses equilibrees (63/63 et 217/217).
- Aucun commit, aucun push. Je n'ai touche que `src/swarm/provider_gateway.rs` et ce rapport.
===FIN===
