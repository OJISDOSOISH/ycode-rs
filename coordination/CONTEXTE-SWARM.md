# CONTEXTE SWARM — portage d'OpenCode (TypeScript) vers Rust

**Lis ce fichier EN ENTIER avant d'ecrire la moindre ligne.** Il contient tout :
ta mission, les regles, la maniere de communiquer, et le format de ton rapport.
Tu n'as pas besoin d'autre chose.

---

## 1. D'ou ca vient, pourquoi on fait ca

Voici le message du responsable du projet, mot pour mot. Il t'est adresse
directement, et tu dois savoir ce qu'il attend de toi.

> "ecouter bien voici ce que on va faire c'est un test donc pas de souci vous
> aller chaucn  balancer 20 sous agents bien innformer chacun je suis fattiguer
> decrire a chacun donc tu lui envoie mon mssage aussi chaucn soit 10 chaaun
> soit 20 chqacun ou soit 5 chacun c'est un choix entre ces triox vous faites
> en sortes quil comprends le contxte bien ensuite vous leur dites comment
> comminiquer moi je surveille et puis vous les abalancer dessus parceque moi je
> travaille toi tu attends c'est concest pas ca moi je veux mon swarm mode et
> puis on va ver"

Points a retenir de ce message :
- C'est **volontairement un test de swarm**. La rapidite compte autant que la
  qualite.
- Tu ne dois pas **redemander** quoi que ce soit. Tout ce qu'il faut est ici.
- Le format de ton rapport doit etre court, pour ne pas noyer le canal.

Le projet, en une phrase : on reecrit en Rust le depot `opencode`, ecrit en
TypeScript, parce que le binaire produit est trop lourd et trop lent. La source
fait environ 640 000 lignes.

**Ou sont les choses**
- Source TypeScript : `C:\Users\AI\Projects\Ycode\opencode\packages\core\src`
- Projet Rust : `C:\Users\AI\Projects\Ycode\ycode-rs`
- Regles de portage detaillees : `coordination\BRIEF-PORTAGE.md`
- Regles de coordination : `coordination\LIVE-PROTOCOL.md`

---

## 2. Ce que tu fais

Tu traduis **UN** fichier TypeScript en **UN** fichier Rust. Un seul. Pas deux.
Le chemin exact de ta source et de ta cible t'est donne dans ta mission
personnelle, plus bas.

Lis la source **entierement** avant d'ecrire. Pas en diagonale : les details qui
cassent le portage sont presque toujours a la fin du fichier.

---

## 3. Regles absolues

1. **Tu ne touches QU'A ton fichier cible.** Pas un autre. En particulier tu ne
   touches **pas** a `mod.rs`, **pas** a `lib.rs`, **pas** a `Cargo.toml`, et
   **pas** aux fichiers des autres agents du swarm. Si ton fichier doit etre
   declare quelque part, tu le signales dans ton rapport, tu ne le fais pas.

2. **Aucun compilateur Rust n'est utilisé sur cette machine, et c'est
   INTERDIT.** Ni `cargo`, ni `cargo check`, ni `cargo test`, ni `rustc`, ni
   `rustup`. La règle vient de `LISEZ-MOI.md` : « Build via GitHub Actions
   uniquement », « Jamais en local ». Elle a été énoncée par le responsable et
   il l'a répétée.

   Un agent a déjà enfreint cette règle : il a installé une toolchain et lancé
   la compilation en local, créant 2,7 Go sur le poste. Ce n'est pas un détail
   de forme, c'est un ordre explicite.

   Concrètement, votre travail s'arrête à l'écriture du fichier sur disque.
   Vous ne l'exécutez pas, vous ne le typez pas, vous ne le compilez pas. Pour
   vérifier votre code, vous relisez, et vous signalez ce qui vous paraît
   douteux. La seule exécution se fait sur GitHub Actions.

   L'API HTTP du serveur opencode reste utilisable pour communiquer. Ce qui est
   interdit, c'est uniquement la compilation locale.

3. **Commentaires et documentation en francais SANS ACCENTS.** Pas d'accent, pas
   de guillemet typographique, pas de caractere chinois ou japonais. Seuls les
   accents et le CJK ont ete vus dans le code existant ; le reste du jeu de
   caracteres latin passe. Les tests de la regle sont dans le fichier de
   compilation, mais n'y pense pas trop.

4. **Au moins 3 tests `#[cfg(test)]`.** Couvre les cas limites que tu deduis en
   lisant la source : liste vide, un seul element, ordre inverse, valeurs par
   defaut, entrees invalides. Nomme tes tests en francais sans accents, en
   `snake_case`, et decris **le comportement** ("une liste vide donne une liste
   vide") plutot que l'implementation ("teste map").

5. **Ne fabrique rien.** Si la source n'est qu'un export vide, un reexport, ou
   une declaration d'interface sans implementation, ecris un fichier minimal
   qui l'explique en commentaire. Un fichier de 20 lignes honnete vaut mieux
   qu'un fichier de 200 lignes invente.

---

## 4. Les conversions

| TypeScript | Rust |
|---|---|
| `Schema.Class` / `Schema.Struct` | `struct` derive `Serialize, Deserialize` |
| ADT a tag (`TaggedErrorClass`, `toTaggedUnion`) | `enum` avec `#[serde(tag = "...")]` |
| `optional` / `optionalKey` | `Option<T>` + `#[serde(skip_serializing_if = "Option::is_none")]` |
| `Effect.fn` / `Effect.gen` | fonction Rust pure. Pas d'`async` si ce n'est pas necessaire |
| `Context.Service` | `trait` |
| `Layer.succeed` | une implementation concrete du trait |
| `readonly Set<A>` | `BTreeSet<A>` (deterministe) |
| `readonly Map<K, V>` | `BTreeMap<K, V>` (deterministe) |
| `Schema.DateTimeUtc` | `i64`, millisecondes depuis l'epoch |
| `Schema.Finite` | `f64` |
| `Schema.Int` | `i64` ou `u64` selon le sens |

---

## 5. Les deux pieges qui cassent le portage

### Le piege 1 : `?` contre `??`

Ce sont deux operateurs differents, et le TypeScript d'origine les melange.

- Le **ternaire** `x ? a : b` teste la **veracite**. En JavaScript, `""` est
  falsy et `0` est falsy. Donc `row.parent_id ? ... : undefined` transforme une
  chaine vide en `undefined`.
- Le **coalescent** `x ?? y` teste la **nullite**. Seuls `null` et `undefined`
  declenchent `y`. Une chaine vide **survit** et n'est PAS remplacee.

Une meme chaine vide doit donc disparaitre a un endroit et survivre a l'autre.
Traduire les deux operateurs par la meme fonction garantit une divergence.
Traduire un ternaire par un simple `.map()` sur un `Option` garantit aussi une
divergence, parce que `Some("")` resterait `Some("")`.

### Le piege 2 : les noms de champs

Tout nom `camelCase` en TypeScript porte un `#[serde(rename = "camelCase")]`
**explicite** en Rust. C'est l'erreur la plus frequente de tout ce portage, et
elle est invisible de l'interieur du code Rust : elle n'apparait qu'a l'echange
avec le TypeScript.

`projectID` n'est pas `projectId`. `messageID` n'est pas `messageId`.
`workspaceID` n'est pas `workspaceId`. Ce sont des majuscules, pas des
minuscules. Verifie champ par champ, et ecris un test qui serialize ton struct
et qui verifie les noms.

---

## 6. Comment communiquer

Trois canaux, et ils ne se Melangent pas.

### Vers les autres agents du swarm : aucun canal direct

Vous etes 20 a ecrire en meme temps. Par construction vous ne vous parlez pas.
La collision est evitee mecaniquement, pas par discussion :

- Chaque agent a un **claim** : `claims\<ton-nom>.claim.json`, cree pour toi,
  qui dit que le fichier est a toi.
- Un fichier deja porte n'est jamais reecrit.

Si tu penses qu'un autre agent a le mauvais fichier, **ne le modifie pas** et
**ne le contourne pas**. Ecris-le dans ton rapport. C'est tout.

### Vers l'agent principal et vers l'autre agent du projet

Il y a un serveur `opencode` qui tourne sur `127.0.0.1:49374` et qui expose une
API. Le wrapper est `C:\Users\AI\Projects\Ycode\COURRIER\kilo.ps1` :

    powershell -NoProfile -File "C:\Users\AI\Projects\Ycode\COURRIER\kilo.ps1" Sessions
    powershell -NoProfile -File "C:\Users\AI\Projects\Ycode\COURRIER\kilo.ps1" Read -Last 5
    powershell -NoProfile -File "C:\Users\AI\Projects\Ycode\COURRIER\kilo.ps1" Send -Text "..."

Deux pieges de cette API, documentes dans `PROTOCOLE.md` : le tableau des
messages n'est pas trie, et le serveur ne renvoie pas de charset, donc les
accents arrivent en mojibake si on ne force pas l'UTF-8.

**Tu n'as normalement pas besoin de l'appeler.** Ecris ton rapport sur disque,
c'est la voie normale.

### Vers l'agent principal : ton rapport, sur disque

C'est ce qu'on attend de toi. Un fichier :
`C:\Users\AI\Projects\Ycode\ycode-rs\reponses\swarm-<ton-nom>.md`, entre les
marqueurs `===DEBUT===` et `===FIN===`.

    # swarm-<ton-nom>

    ===DEBUT===
    fichier : src/swarm/<ton-fichier>.rs
    source  : packages/core/src/<ta-source>.ts
    taille  : NNN octets
    tests   : N

    CONFIANCE : haute | moyenne | basse
    POINT FAIBLE : <une phrase, la chose que tu n'es pas sur d'avoir bien portee>
    A VERIFIER : <ce que la relecture doit verifier en priorite>
    ===FIN===

Le champ **POINT FAIBLE** est le plus important de tout ton rapport. C'est ce que
la relecture croisee va aller verifier en premier. Ne le minimise pas pour
paraitre sur de toi : un agent qui sait dire ou il n'est pas sur est dix fois
plus utile qu'un agent qui dit que tout va bien.

**Commite rien. Pousse rien.** Seuls l'agent principal et l'autre agent du
projet touchent a git.

---

## 7. Ce qui arrive apres toi

Ton fichier part en relecture croisee. La relecture a deja attrape, sur un
portage precedent, **deux incompatibilites de noms de champs** (`sessionID` /
`callID`) qui etaient invisibles de l'interieur du code Rust et qui n'auraient
casse qu'a l'echange avec le TypeScript. C'est exactement pour ça que le
POINT FAIBLE existe.

Ne t'inquiete pas si la relecture te reprend. Elle cherche les vraies erreurs,
pas a te travaille.

---

## 8. Ta mission

Cette section est la seule propre a toi. Elle est remplie par l'agent
principal qui t'a lance.
