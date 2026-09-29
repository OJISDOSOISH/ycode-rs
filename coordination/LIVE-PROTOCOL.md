# Protocole temps reel (2 agents en parallele)

Probleme : 2 agents ecrivent dans le meme depot au meme moment.
Solution : verrou par fichier + presence, sans serveur.

## 1. Claim atomique
Avant de porter un fichier TS, creer `claims/<cible>.claim.json` :
`{"owner":"kilo|opencode","started":"<date ISO>","source":"<chemin TS>","target":"<chemin RS>"}`.
Si le fichier existe deja : ne pas toucher au fichier cible. En prendre un autre.

## 2. Presence
Chaque agent tient `coordination/presence-<nom>.json` avec `heartbeat` ISO mis a jour a chaque tour.
Avant de prendre une tache, lire la presence de l autre + `claims/` pour eviter les collisions.

## 3. Partage actuel
- OpenCode : `policy.ts` -> `src/policy.rs` (claim `claims/policy.claim.json`). Ne plus y toucher sauf bug signale avec fichier+ligne.
- Kilo : `briefs/b00..b19`, reponses en `reponses/rXX.md` entre `===DEBUT===` / `===FIN===`.

## 4. Git
Ne commiter que ses propres fichiers. `pull --rebase` avant push. Jamais de `push --force`.
Build via GitHub Actions uniquement (cf. LISEZ-MOI.md).
