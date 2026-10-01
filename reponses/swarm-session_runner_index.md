# swarm-session_runner_index

===DEBUT===
fichier : src/swarm/session_runner_index.rs
source  : packages/core/src/session/runner/index.ts
taille  : 15659 octets
tests   : 12

CONFIANCE : haute
POINT FAIBLE : `RunError` est porte comme un enum a six variantes UNITAIRES alors que l'union TypeScript aplatie en dix classes. Ce n'est pas un oubli : les dix classes sont definies dans cinq modules non portes, donc les payloads (`reason`, `operation`, `keys`, `cause`...) ne sont pas observables ici. Le vrai risque est la consequence, signalee dans le fichier : des qu'un module proprietaire existe, `RunError::Llm` devra devenir `RunError::Llm(LlmError)`, ce qui cassera tous les appelants de la variante nue. C'est le premier point que la relecture doit decide.
A VERIFIER :
1. **Convention de nommage du trait.** Le trait s'appelle `Interface`, pas `Service`, parce que `session/execution.rs` definit deja un `trait Service` pour le service `"@opencode/v2/SessionExecution"`. C'est un choix, pas une obligation : si la revue prefere un `SessionRunnerService` (ou un `pub use` de l'alias), c'est une ligne a changer.
2. **Les derives serde de `RunInput` sont un pont ajoute**, pas une traduction : la source ne donne aucun schema d'execution pour `{ sessionID, force }`, c'est un type TypeScript pur. Le `#[serde(rename = "sessionID")]` est donc correct par prudence, mais il n'y a dans le TypeScript aucun aller-retour JSON qui pourrait le mettre a l'epreuve.
3. **`from_name` et non `from_str`** : renomme a la revision pour ne pas collisionner avec le trait `std::str::FromStr` (lint clippy `should_implement_trait`). Verifier qu'aucun autre module du lot n'attend `RunError::from_str`.
4. **Aucun type n'est redefini** : `SessionId` est IMPORTE de `crate::core::session::schema` (`pub type SessionId = String`), pas recree ici. `RunError`, `Service`, `Entry` : rien de tout cela n'est declare dans mon fichier. Aucun nom en collision avec `src/core/session/run_coordinator.rs` ni `src/core/session/execution.rs` (les deux lus avant ecriture, ni modifies).
5. **Les tests sont purs et instantanes** : aucun thread, aucun `Mutex`, aucun `Condvar`, aucune attente, aucune boucle infinie. Le seul `for` boucle sur un tableau de 6 elements. Le seul etat est un `RefCell` local au module de test. Contrainte de dureur de la CI respectee.
6. **Un fait corrige a la revision** : la premiere version du fichier annoncait "onze classes" et "huit raisons". Verifie sur `packages/llm/src/schema/errors.ts:160-171` : `LLMErrorReason` est une union de DIX classes, et l'aplatie de `RunError` compte DIX classes, pas onze. Le reste des affirmations du fichier a ete verifie sur la source : `SessionRunnerModel.Error` (5 membres, `model.ts:67`), `ToolOutputStore.Error` = `StorageError` (`tool-output-store.ts:40`), tags `"Session.MessageDecodeError"` et `"Session.ContextSnapshotDecodeError"` (`session/error.ts:5,14`), `"SystemContext.InitializationBlocked"` (`system-context/index.ts:83`).

Fait utile pour les autres : `export * as SessionRunner from "./index"` (ligne 1) est un reexport de l'espace de noms du module sur LUI-MEME, donc du code mort. C'est le meme motif que `export * as ConfigCommand from "./command"` et `export * as Wildcard from "./wildcard"` : tres recurrent dans opencode, ce n'est pas un import vers un fichier voisin.

Integration : `pub mod session_runner_index;` est deja present ligne 67 de `src/swarm/mod.rs`, je n'y ai pas touche. Aucun commit, aucun push. Aucune compilation locale (interdit).
===FIN===
