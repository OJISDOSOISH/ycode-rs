# swarm-provider_deepinfra

===DEBUT===
fichier : src/swarm/provider_deepinfra.rs
source  : packages/core/src/plugin/provider/deepinfra.ts
taille  : 11283 octets (272 lignes)
tests   : 10

CONFIANCE : moyenne
POINT FAIBLE : la source n'est PAS un objet de configuration litteral, c'est un
enregistrement de crochet (`{ id, effect }`), et j'ai du choisir quoi en
porter. J'ai porte les constantes (`id`, `package`, la fabrique) et la seule
decision reelle du fichier, la garde `evt.package !== "@ai-sdk/deepinfra"`,
traduite en `handles(&self, package: &str) -> bool`. Ce que je n'ai PAS porte,
et que la relecture doit juger : (a) l'`import()` dynamique et l'appel a
`createDeepInfra`, qui sont des liens vers un paquet JS, donc inimplementables
en Rust sans inventer un SDK ; (b) l'objet `SDKEvent`, qui est defini dans
`aisdk.ts` et donc appartient a un autre module du portage, recopie ici il
serait une seconde verite pour le meme type. Resultat : mon fichier ne dit rien
du `evt.sdk` que le TS ecrit, alors que c'est tout l'objet du plugin. Si la
vague 3 veut un portage executable, il manque un trait `AisdkSdk` et une
fonction qui enregistre le crochet ; ce n'est pas dans mon scope.

A VERIFIER : par ordre de priorite.
1. Le nom de champ `package` de mon struct est-il le bon, ou la relecture
   prefere-t-elle `package_name` avec un `#[serde(rename = "package")]` ? Le
   champ TS est `evt.package` (aisdk.ts:15), j'ai donc garde `package`. Aucun
   `#[serde(rename)]` n'est present : les trois noms (`id`, `package`,
   `factory`) sont en minuscules, donc Serde produit deja les bons noms. Le
   test `les_noms_de_champs_json_sont_ceux_du_typescript` verifie les trois
   cles, refuse `Id`/`ID`/`Package`/`Factory`, et compte 3 cles. C'est le point
   a relire en premier, meme si je pense avoir raison.
2. `createDeepInfra` : la casse. C'est bien `DeepInfra` avec un D majuscule
   dans le nom de la fabrique, alors que le paquet et l'id sont en minuscules
   (`deepinfra`). J'ai copie la valeur depuis `deepinfra.ts:11`, je ne l'ai pas
   devinee, mais c'est exactement le genre d'intermittence (`nPm`) que le lot
   signale.
3. La ligne 130, `package == self.package.as_str()`. J'ai mis `.as_str()`
   explicitement plutot que de compter sur l'implementation de
   `impl PartialEq<String> for &str`. Si la relecture voit `== self.package`
   ailleurs dans le lot, c'est equivalent, pas une erreur.
4. Le nom des constantes publiques `ID`, `SDK_PACKAGE`, `FACTORY`, en
   majuscules, au niveau du module. Aucun autre fichier du lot ne les
   exporte encore, donc personne ne peut encore avoir une convention opposee ;
   mais 18 autres plugins fournisseurs vont les porter aussi, et une
   harmonisation sera necessaire. A trancher au niveau du lot, pas par moi.

NOTE : `src/swarm/mod.rs` contient deja `pub mod provider_deepinfra;`
(ligne 33). Rien a signaler de ce cote, je n'y ai pas touche.

NOTE 2 : les 34 fichiers de `packages/core/src/plugin/provider/` ont exactement
la meme forme que ma source (registre de crochet, comparaison de paquet, import
dynamique, appel d'une fabrique). `cohere.ts`, `togetherai.ts` et `venice.ts`
sont identiques ligne pour ligne. Chacun de ces fichiers a donc produit la meme
question de perimetre que ci-dessus, et il serait utile que la vague 2
s'accorde sur une seule reponse plutot que sur 34 variantes.
===FIN===
