# swarm-provider_cerebras

===DEBUT===
fichier : src/swarm/provider_cerebras.rs
source  : packages/core/src/plugin/provider/cerebras.ts
taille  : 23610 octets
tests   : 20

CONFIANCE : moyenne
POINT FAIBLE : la source n'est PAS un objet de configuration litteral mais l'enregistrement de deux hooks, donc j'ai du inventer deux traits d'interface (`CatalogProviderDraft` avec `list`/`update`, `CerebrasSdkFactory` avec `create_cerebras`) pour rendre la logique testable, et j'ai recopie localement les types `ProviderV2Info` / `ProviderApi` / `ProviderRequest` qui sont declares dans `packages/sdk/js/src/v2/gen/types.gen.ts` (hors lot, ports par personne) : c'est du doublon possible. De plus aucune ligne n'a pu etre compilee sur ce poste, et c'est la premiere fois que j'ecris `&dyn Fn(&mut T)` passe en position d'argument de methode de trait (ligne 189) avec une coercition de closure non capturante (ligne 275) : c'est le point de syntaxe le plus suspect du fichier.
A VERIFIER : dans cet ordre. (1) Que `update(&mut self, id: &str, update: &dyn Fn(&mut ProviderV2Info))` se compile bien, et que `draft.update(&provider_id, &apply)` coerce la closure en `&dyn Fn`. (2) Que `#[serde(tag = "type")]` sur `ProviderApi` reste compatible avec `BTreeMap<String, Value>` pour le deserialissage. (3) Que les noms de champs sont exacts : `integrationID` est le SEUL nom avec un `ID` en majuscules dans tout le fichier, les 10 autres sont des mots uniques en minuscules, verifies par 3 tests de cles. (4) Que `pub mod provider_cerebras;` n'est pas declare deux fois : j'ai verifie, mod.rs ligne 31 le declare deja, je n'y ai pas touche.
===FIN===

## Ce que la source est

26 lignes, un seul export : `CerebrasPlugin = define({ id: "cerebras", effect })`.
`define` est l'identite, l'objet n'a que `id` et `effect`. L'effet enregistre
deux callbacks :

1. `ctx.catalog.transform` : pour chaque fournisseur du catalogue, si
   `api.type === "aisdk"` ET `api.package === "@ai-sdk/cerebras"`, alors
   `provider.request.headers["X-Cerebras-3rd-Party-Integration"] = "opencode"`.
2. `ctx.aisdk.sdk` : si `evt.package === "@ai-sdk/cerebras"`, alors
   `import("@ai-sdk/cerebras")` puis `evt.sdk = mod.createCerebras(evt.options)`.

## Decisions de portage, avec leur statut

- **Le `import()` dynamique n'est pas portable.** Un paquet npm JavaScript n'a
  pas d'equivalent Rust et n'est pas installable ici. Je n'ai pas simule de SDK :
  l'appel est isole dans le trait `CerebrasSdkFactory`, fourni par l'appelant.
  Ce qui est porte, c'est la decision (le paquet correspond ou non) et la
  transmission des options, c'est-a-dire tout ce que la source decide.
  **Arbitrage a confirmer** par la relecture.
- **Aucun `async`, aucun `Effect`.** Les deux branches sont pures.
- **Les `Registration` rendues par les raccourcis sont abandonnees** dans la
  source, donc le mecanisme de registration n'est pas retranscrit.
- **Types recopies localement** : `ProviderV2Info`, `ProviderApi` (union tagguee
  sur `type`), `ProviderRequest`, `CatalogProviderRecord`, tous declares hors de
  ce fichier. `ModelV2Info` n'est JAMAIS lu par le plugin, donc il est type par
  `serde_json::Value` opaque plutot que recopie en entier (`CatalogProviderRecord.models`
  et `SdkEvent.model`).
- **`CerebrasPlugin` est une structure marqueur** (struct vide) : le champ `id`
  devient une constante associee, et `effect` devient les deux fonctions
  directement appelables. Je n'ai pas invente de champs.

## Les pieges, traites et testes

- **Noms de champs** (le piege de la mission). Un seul nom est piege :
  `integrationID`, avec le `ID` en majuscules. Le nom Rust est `integration_id`
  et porte `#[serde(rename = "integrationID")]` explicite. Trois tests le
  verifient : les cles serialisees du fournisseur, la forme exacte de la cle
  (`"integrationID":"int_1"`, ni `integrationId` ni `integration_id`), et la
  lecture au deserialisage. Un test montre qu'une cle mal ecrite
  (`integrationId`) est **ignoree** et donne `None` (serde ignore les cles
  inconnues par defaut) : c'est le comportement reel, mais ca veut dire qu'une
  faute de frappe a l'ecriture passerait inapercue des deux cotes.
- **Ecriture et non ajout.** L'entete est posee par une affectation simple, donc
  une valeur deja presente est **ecrasee**. Le plugin voisin `nvidia.ts` utilise
  `??=` au meme endroit et laisse la valeur en place : ne pas confondre. Test
  dedie `une_entete_deja_presente_est_ecrasee`.
- **`!==` strict, pas `!=`.** Pas de coercition, donc un paquet vide ne
  correspond pas. Test `un_paquet_vide_ne_declenche_pas_le_sdk`.
- **Pas de `?` contre `??` dans cette source** : zero ternaire, zero coalescent.
  Les deux conditions sont deux `continue` successifs, equivalents a un
  `if !(a && b) { continue }`, ce que fait `CerebrasPlugin::targets`.

## Integration

- `pub mod provider_cerebras;` est deja present dans `src/swarm/mod.rs` ligne 31.
  Je n'y ai pas touche.
- `serde` et `serde_json` sont deja dans `Cargo.toml`, rien a ajouter.
- Fichier autonome : aucune dependance a un autre module du lot, donc il ne peut
  pas casser les 19 autres agents.
- Aucun commit, aucun push.
