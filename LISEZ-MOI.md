# Ycode — réécriture d'OpenCode en Rust

Portage du dépôt `opencode` (TypeScript, ~640 000 lignes) vers Rust.

## État d'avancement

Le projet comporte 32 packages TypeScript. On a choisi de commencer par le noyau,
parce que tout le reste en dépend, et parce que c'est la partie qui fait
réellement tourner un agent.

| Module porté | Origine | État |
|---|---|---|
| Schéma de messages | `packages/schema/src/session-message.ts` | fait, testé |
| Fenêtre de contexte | `packages/core/src/session/history.ts` | fait, testé |
| Époque de contexte | `packages/core/src/session/context-epoch.ts` | fait, testé |
| Politique de compaction | `packages/core/src/session/compaction.ts` | fait, testé |
| Boucle d'agent | assemblage des modules ci-dessus | fait, **vérifié en exécution** |
| Client LLM | `packages/llm` | fait |
| Outils fichiers | `packages/core/src/fs-util.ts` | fait |

**36 tests verts.** L'agent a été exécuté pour de vrai : il a compris une
consigne en français, appelé l'outil d'écriture et produit un fichier Rust
valide.

## Ce qui reste

Les 27 autres packages, dont l'essentiel du poids :

- `core` : git, événements, permission, plugins, config, LSP
- `llm` : streaming, les 15 providers
- `app`, `tui`, `console`, `ui` : interface (environ 450 000 lignes)
- `sdk`, `client`, `protocol`

À ce rythme, la partie fonctionnelle est à quelques mois. L'interface
représente la majorité des lignes et le moins de logique.

## Compilation

**Jamais en local.** Le poste de travail n'a pas les Build Tools MSVC et sa
compilation locale est trop lente. Tout passe par GitHub Actions, qui produit un
binaire Windows, Linux et macOS à chaque push.

## Méthode

1. Lire le TypeScript d'origine avant d'écrire la moindre ligne.
2. Porter en conservant les noms de champs : la compatibilité JSON avec
   l'original est une exigence, pas un détail.
3. Tester, y compris pour les pièges que le TypeScript laisse passer et que Rust
   refuse (`time` défini deux fois, unions discriminées incomplètes).
4. Faire relire par un second agent, et vérifier ses remarques avant de les
   appliquer.

Cette dernière étape a payé : la revue a trouvé deux incompatibilités de noms
de champs (`sessionID` / `callID`) qui étaient invisibles de l'interieur du
code Rust et n'auraient cassé qu'a l'echange avec le TypeScript.
