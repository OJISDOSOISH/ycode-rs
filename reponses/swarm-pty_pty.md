# swarm-pty_pty

===DEBUT===
fichier : src/swarm/pty_pty.rs
source  : packages/core/src/pty/pty.ts
taille  : 25119 octets
tests   : 16

CONFIANCE : haute
POINT FAIBLE : la source ne contient que des types, donc mon fichier ne fait que decrire un contrat et n'ouvre aucun pseudo terminal ; si la relecture attendait un `spawn` fonctionnel, tout le fichier est incomplet par construction. Second point, plus technique : j'ai remplace le `number` TypeScript (double) par des entiers (`i64`, `u32`), donc une valeur fractionnaire venue du TypeScript echoue a la deserialisation au lieu d'etre tronquee, et j'ai ajoute `Send + 'static` + `FnMut` aux type d'ecouteur, ce que la source n'exige pas et qui peut generer une erreur de compilation chez un futur implementateur.
A VERIFIER : (1) que `pub mod pty_pty;` est bien present dans `src/swarm/mod.rs` (je ne l'ai pas touche, mais je l'ai lu : ligne 47) ; (2) que la seule relecture de nom de champ a faire est `exitCode` (les cinq champs de `Opts` sont des mots uniques en minuscules, aucun renommage) ; (3) la coercion des fermettes de test vers `Box<dyn FnMut(&str) + Send + 'static>` : c'est le seul endroit du fichier ou une erreur d'inference ferait echouer la compilation, et je n'ai aucun toolchain pour le verifier ; (4) si la relecture veut un `pid` en champ plutot qu'en accesseur, il faut repenser `Proc` (voir detail plus bas).
===FIN===

## Detail pour la relecture

**Ce que fait la source (25 lignes).** Quatre `export type`, et rien d'autre :
`Disp` (`dispose(): void`), `Exit` (`exitCode: number`, `signal?: number | string`),
`Opts` (`name`, `cols?`, `rows?`, `cwd?`, `env?`), `Proc` (`pid`, `onData`, `onExit`,
`write`, `resize`, `kill?`). Zéro instruction, zéro constante, zéro conversion
d'arguments : rien de ce que la mission mentionnait n'existe dans ce fichier.

**Pourquoi il n'y a pas de `spawn`.** Un pseudo terminal reel demande une
dependance systeme : `forkpty` ou `posix_openpt` sous Unix, `CreatePseudoConsole`
(ConPTY) sous Windows. Aucun de ces Elements n'est dans la bibliotheque standard
de Rust, et `Cargo.toml` ne declare aucune crate de pseudo terminal ; je n'ai
pas le droit de le modifier. J'ai donc refuse d'ecrire une approximation avec
`std::process::Command` : `Command` ouvre des canaux, pas un terminal, donc
`resize` n'aurait aucun effet et le contrat `Proc` serait faux. Le fichier
l'explique en tete de module. Les implementations reelles de la source sont
dans `pty.node.ts` (`@lydell/node-pty`) et `pty.bun.ts`, qui ne sont claims par
personne dans le lot : si le lot veut un backend, c'est un autre fichier et
peut-etre une autre mission.

**Ce qui est porte.** `Disp` et `Proc` en `trait` (le plus proche d'un type
structurel sans implementation), `Exit` et `Opts` en `struct` derive
`Serialize, Deserialize`, l'union `number | string` de `signal` en
`enum Signal { Nombre(i64), Nom(String) }` avec `#[serde(untagged)]` pour que
le JSON reste `9` ou `"SIGKILL"` et pas `{ "nombre": 9 }`.

**Le piege des noms de champs.** Un seul nom camelCase dans tout le fichier :
`exitCode`, porte par `#[serde(rename = "exitCode")]` explicite, avec un test qui
verifie que la cle sort et que `exit_code` n'apparait jamais. `name`, `cols`,
`rows`, `cwd`, `env` et `signal` sont des mots uniques en minuscules. Un test
serealise `Opts` au complet et relit les cinq cles.

**Le piege `?` contre `??`, traite en trois points.** La source n'a ni
ternaire ni coalescent, mais elle a trois champs optionnels ou l'absence est
une information et ne doit jamais devenir une valeur neutre :
- `kill(signal?)` : `None` ne doit pas devenir `""`. Test dedie
  (`un_arret_sans_signal_n_est_pas_un_arret_avec_un_nom_vide`) ou le faux
  journal distingue `kill(aucun)`, `kill()` et `kill(SIGKILL)`.
- `cols?` / `rows?` absents : taille par defaut, pas `0`. Test dedie.
- `env?` absent : heritage de l'environnement, alors qu'un objet vide demande
  un processus sans aucune variable. Test dedie sur les deux formes.
- et `signal: 0` reste **present** (valeur, pas absence), `signal: ""` reste
  une chaine vide distincte d'une cle absente. Deux tests dedies.

**Les tests.** 16 tests. Neuf sur `Opts` et `Exit` (serde, noms de cles, valeurs
absentes, environnement vide contre absent, tri des cles, `signal` en nombre
ou en nom, `0` contre absent, chaine vide contre absent) et cinq sur `Proc` avec
un faux qui note les appels : ecriture, redimensionnement, arret avec et sans
signal, abonnement aux donnees puis `dispose`, abonnement a la sortie, lecture
du `pid`. Le faux ne simule ni terminal ni processus fils, il est dit comme tel
dans un commentaire.

**Choix a valider.**
- `pid` en methode plutot qu'en champ : un `trait` ne porte pas de champ. La
  valeur ne change pas pendant la vie du processus, donc pas d'etat interieur.
  L'autre solution aurait ete une `struct` qui encapsule un `Box<dyn ...>`, ce
  qui aurait invente une forme absente de la source.
- `FnMut + Send + 'static` sur `DataListener` et `ExitListener` : adaptation au
  monde Rust (la fermeture doit survivre a l'enregistrement et peut etre
  deplacee sur la tache qui lit le terminal). Contrainte en plus de la source,
  donc signalee.
- `BTreeMap` pour `env` : conforme aux conventions du portage, mais les cles
  sortent triees alors que JavaScript les sort dans l'ordre d'insertion. Sans
  consequence sur la semantique d'un objet JSON, seulement sur une comparaison
  octet pour octet.
- `Opts::new` est une commodite qui n'existe pas dans la source ; elle est
  documentee comme telle.

**Controles faits.** 100 % ASCII verifie (scan sur le fichier, 0 caractere
> 126), aucun accent, aucun guillemet typographique, accolades et parentheses
equilibrees. `src/swarm/mod.rs` contient deja `pub mod pty_pty;` (non modifie).
`Cargo.toml` n'a pas ete touche, `serde` et `serde_json` y sont deja declares.
Aucun commit, aucun push.
