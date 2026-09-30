# Charte de travail — réécriture d'OpenCode en Rust

Version 1, 2026-09-30. Cette charte remplace toutes les règles dispersées. En cas
de contradiction, c'est elle qui gagne.

---

## 1. La règle qui ne se négocie pas

**Tout ce qui peut être exécuté est exécuté par GitHub Actions, jamais sur le poste
de travail.**

Dit autrement : **si GitHub peut le faire, GitHub le fait.** Un contrôle statique
qui prend deux secondes ici mais pourrait être une étape de CI doit devenir une
étape de CI.

Interdits en local : `cargo`, `cargo check`, `cargo test`, `cargo build`, `rustc`,
`rustup`, toute installation de toolchain, tout `target/`, tout script de
vérification appelable depuis la CI.

Autorisés en local : lire et écrire les fichiers du dépôt, communiquer par l'API
HTTP du serveur opencode, lire des journaux et rapports déjà produits par GitHub,
pousser sur Git.

Cette règle a déjà été enfreinte. Une toolchain de 816 Mo et un `target/` de
1856 Mo ont été créés sur le poste. Le prochain contrôle statique est donc écrit
**dans** le workflow CI, et non appelé à la main.

---

## 2. Où on en est, en chiffres

Périmètre : `packages/core/src` et `packages/schema/src`, hors tests.

| Mesure | Valeur |
|---|---|
| Fichiers TypeScript à porter | **381** |
| Fichiers Rust produits | **72** |
| Lignes TypeScript du périmètre | 33 043 |
| Lignes de code Rust de portage | 14 375 |
| Lignes de tests Rust | 10 527 |
| Tests exécutés à ce jour | **0** |

### Le taux de 19 % est trompeur, et il faut le dire

Répartition du TypeScript restant, par taille :

| Taille du fichier TS | Fichiers | Lignes |
|---|---|---|
| 0–500 o | 87 | 751 |
| 501–2000 o | 131 | 3 558 |
| 2001–8000 o | 110 | 10 976 |
| plus de 8000 o | 53 | 17 758 |

43 des 72 fichiers Rust produits sont les `plugin/provider/*.ts`, qui font
environ 450 octets chacun. Nous avons donc couvert **19 % des fichiers, mais
environ 5 % des lignes**, en attaquant le bas le plus facile du panier.

Il reste 53 fichiers de plus de 8 Ko. Aucun n'est commencé. C'est le vrai
travail, et il est devant nous.

### Trois chiffres à ne jamais confondre

- **Écrit** : le fichier Rust existe sur le disque.
- **Compilé** : GitHub Actions a confirmé que ça compile. *Jamais arrivé à ce
  jour pour le swarm.*
- **Vérifié** : les tests passent. *Jamais arrivé.*

Un fichier « écrit » qui n'est ni compilé ni vérifié est un fichier dont on
ignore tout. C'est le cas de **la totalité du swarm aujourd'hui**.

---

## 3. Organisation : 42 agents, zéro collision

Deux agents principaux, chacun avec 20 sous-agents : **42 au total**.

### Le mécanisme anti-collision est mécanique, pas négocié

1. Chaque sous-agent reçoit **un seul fichier**. Jamais deux.
2. Avant d'écrire, il crée `claims/<nom>.claim.json` avec `owner`, `started`,
   `source`, `target`. **Si le fichier existe, il s'arrête et choisit autre
   chose.** C'est le seul contrôle, et il suffit.
3. Un fichier déjà porté n'est jamais réécrit. Pour corriger un bug, on écrit un
   rapport et le propriétaire décide.
4. `src/swarm/mod.rs` est écrit **par l'agent principal uniquement**. Aucun
   sous-agent ne le touche, donc personne n'écrit la même ligne.
5. Un nom de module Rust ne peut pas contenir de tiret ni de majuscule. Les
   claims sont validés contre `^[a-z][a-z0-9_]*$` avant lancement.

### Ce qui s'est passé aux vagues précédentes, pour ne pas répéter

- **La duplication à 8.** Dix-huit fichiers `provider/*.ts` sont identiques ligne
  pour ligne. Chaque agent a inventé son propre type de payload faute
  consigne, d'où huit `SdkEvent` différents. **Correction : quand plusieurs
  fichiers source se ressemblent, il faut annoncer le module partagé AVANT de
  lancer les agents.** Ne pas répéter.
- **Le `?` contre `??`.** Un ternaire JavaScript teste la *vérité* — la chaîne
  vide est falsy. Un coalescent `??` teste la *nullité* — la chaîne vide
  survit. Même chaîne, deux opérateurs, deux comportements. C'est le piège
  numéro un de ce portage.
- **Les noms de champs.** `projectID` n'est pas `projectId`, `sessionID`
  n'est pas `sessionId`. Ce sont des majuscules. C'est invisible à la
  compilation Rust et ça ne casse qu'à l'échange avec le TypeScript.

---

## 4. Communication

### 4.0 Règle de relais — ne jamais demander l'autorisation de relayer

**Chaque message du responsable est transmis à l'autre agent principal, mot pour
mot, avant toute action.** Sans exception et sans attendre qu'on le rappelle.

Cette règle existe parce qu'il n'y a qu'un seul humain pour deux agents : s'il
n'y a que lui pour faire le relais entre nous, il est le goulot d'étranglement,
et il sature. Le relais est donc automatique, de ma responsabilité, et permanent.

En pratique : je lis le canal toutes les 10 minutes, et ma toute première action
à chaque passage est de vérifier si le responsable a parlé depuis le dernier
passage. Si oui, je transmets avant tout le reste. Le chef n'a rien à demander,
et opencode n'a rien à réclamer.

Si l'agent opencode apparaît en retard ou muet, c'est que mon intervalle de
lecture a été allongé. Le regravoir immédiatement, sans attendre qu'on le signale.

### 4.1 Les canaux

| Canal | Usage |
|---|---|
| `claims/*.json` | Qui porte quoi. Écriture unique, jamais réécrite. |
| `reponses/swarm-*.md` | Rapport d'un sous-agent, entre `===DEBUT===` et `===FIN===`. |
| `coordination/` | Cette charte, les règles de portage, les comptes rendus. |
| API opencode, port 49374 | Entre l'agent principal Kilo et l'agent principal opencode. |

Un sous-agent n'a **pas** de canal direct vers les autres sous-agents. Ce n'est
pas un oubli : c'est la garantie qu'ils ne peuvent pas se marcher dessus. La
collision se règle par le claim, pas par une discussion.

Entre Kilo et opencode : `COURRIER\kilo.ps1`, actions `Send`, `Read`, `Watch`,
`Sessions`. Deux pièges de l'API, documentés dans `PROTOCOLE.md` : le tableau des
messages n'est pas trié, et le serveur ne renvoie pas de charset, donc les
accents arrivent en mojibake si on ne force pas l'UTF-8.

### 4.2 Accords explicites avant d'exécuter

Rien ne se lance sur initiative unilatérale tant que les deux agents principaux
n'ont pas validé le plan commun : qui porte quoi, dans quel ordre, et ce qu'on
fait si un fichier est contesté. Un sous-agent ne se remplace pas par un message
de rapport ; c'est un accord.

---

## 5. Ce que doit contenir un rapport

Le champ **POINT FAIBLE** est le plus important. C'est lui que la relecture va
chercher en premier. Un agent qui sait dire où il n'est pas sûr vaut dix fois
plus qu'un agent qui dit que tout va bien.

```
===DEBUT===
fichier : src/<...>.rs
source  : packages/<...>.ts
taille  : NNN octets
tests   : N

CONFIANCE : haute | moyenne | basse
POINT FAIBLE : une phrase, la chose que vous n'êtes pas sûr d'avoir bien portée
A VERIFIER : ce que la relecture doit vérifier en priorité
===FIN===
```

---

## 6. Rythle

- Les sous-agents travaillent en arrière-plan, en parallèle, un fichier chacun.
- L'agent principal intègre, relance les griefs, et pousse.
- opencode est relecteur. Son rôle est de trouver les vraies erreurs, pas de
  réécrire les fichiers des autres.
- La CI est l'unique exécutant. On attend son verdict, on corrige, on repousse.
