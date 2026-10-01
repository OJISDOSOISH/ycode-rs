# swarm-plugin_command

===DEBUT===
fichier : src/swarm/plugin_command.rs
source  : packages/core/src/plugin/command.ts
taille  : 39834 octets
tests   : 13

CONFIANCE : haute
POINT FAIBLE : j'ai implemente `remplacer_premier` pour reproduire les trois
  comportements de `String.prototype.replace(motifChaine, remplacementChaine)`,
  dont l'interpretation des `$` dans la chaine de remplacement (`$$`, `$&`,
  `` $` ``, `$'`). C'est la seule partie du fichier ou j'ai du choisir une
  semantique que la source n'ecrit nulle part, et c'est 60 lignes de code que la
  relecture peut a juste titre trouver surdimensionnees pour un jeton qui
  n'apparait qu'une fois. A verifier en priorite : que la regle appliquee est
  bien celle de `GetSubstitution` reduite au motif chaine (donc pas de groupes
  capture, donc `$1` litteral), et que personne ne prefere `str::replace`.
A VERIFIER :
  1. `PROMPT_REVIEW` ne contient aucun `${path}` alors que `PROMPT_INITIALIZE`
     en contient exactement un. Le `.replace` de la source ne fait donc rien
     pour `review`. Verifier ce point avant tout le reste : c'est lui qui
     explique pourquoi le meme code produit un effet pour une commande et rien
     pour l'autre. J'ai verifie les deux prompts octet par octet contre les
     `.txt` (seule difference : CRLF d'origine remplaces par LF dans le `.rs`).
  2. Les 4 affectations sont des `=` simples, pas des `??=`. Une valeur
     preexistante est donc ecrasee, chaine vide comprise. `init` n'ecrit pas
     `subtask`, `review` le force a `true` : le test
     `les_valeurs_preexistantes_sont_ecrasees_sauf_subtask_pour_init` verrouille
     cette asymetrie.
  3. Noms de champs : ce plugin n'ecrit que `template`, `description`,
     `subtask`, tous en minuscules des deux cotes, donc aucun
     `#[serde(rename)]` ici. Le seul renommage du contrat est `providerID`, qui
     appartient a `Model.Ref` dans `src/core/command.rs` ; sa forme snake_case
     est donc refusee a la lecture, ce que verifie un test dedie.
  4. Aucun ternaire `?` ni coalescent `??` dans la source. Le piege 2 ne se pose
     pas ici : le point de decision le plus proche est le repertoire de projet
     vide, qui retire le jeton sans rien laisser (`"already exists at ,"`).
  5. Choix delibere a trancher : les deux invites sont **recopiees** dans le
     fichier, pas importees par `include_str!`, parce qu'elles vivent dans un
     autre depot que celui qui compile et qu'un chemin absolu casserait la CI.
     Consequence : `PROMPT_REVIEW` conserve 2 caracteres non-ASCII (tiret
     cadratin, deux en exposant de `O(n2)`). Ce sont de la donnee, pas de la
     documentation. Si la CI refuse le non-ASCII, il faut les remplacer, et je
     prefere que ce soit une decision assumee plutot qu'un nettoyage muet.
  6. `CommandPlugin::transformer` prend la localisation en parametre au lieu
     d'aller la lire dans le service, parce que `Location.Service` n'est pas
     porte. Le reste du moteur de plugins (`ctx.command.transform`, scope,
     rejouabilite) n'est pas traduit : il appartient a `plugin/internal.ts`.

DETAIL :
- FICHIER ABSENT AU DEPART. La mission annoncait un fichier deja porte par la
  vague 3 : `src/swarm/plugin_command.rs` n'existait pas sur le disque (ni dans
  `src/swarm/`, ni ailleurs). Le claim existait, le fichier non. Il a donc ete
  ecrit de zero, en suivant le style deja pose par `location.rs` et
  `plugin_provider_llmgateway.rs`.
- A FAIRE PAR L AGENT PRINCIPAL : `pub mod plugin_command;` manque dans
  `src/swarm/mod.rs` (verifie : absent). Je n'ai pas touche `mod.rs`.
- Aucun type redeclare. Le registre vient de `crate::core::command`
  (`CommandStore` = le `Draft` de `core/command.ts`, regle de creation
  comprise ; `CommandInfo` = `CommandV2.Info`), la localisation de
  `crate::swarm::location` (`Interface`, dont `project.directory` est deja
  porte). Deux types reutilises, zero type ajoute : le fichier ne declare que
  des constantes, deux fonctions de template, la transformation et le
  manipulateur de `$`.
- CHEVAUCHEMENT AVEC `config_command.rs`, signale comme demande : il n'y en a
  pas de reel, et je n'ai rien duplique. Les deux sources sont differentes.
  - `config/command.ts` = `ConfigV2.Command`, la forme **du fichier de
    configuration** : `{ template, description?, agent?, model?: string,
    variant? }`, sans `name`. C'est ce que l'utilisateur ecrit dans
    `opencode.json`.
  - `plugin/command.ts` = un **plugin** qui ecrit dans le registre d'execution,
    dont l'entree est `CommandV2.Info` = `{ name, template, description?,
    agent?, model?: Model.Ref, subtask? }`.
  Les quatre noms de champs communs (`template`, `description`, `agent`,
  `subtask`) sont identiques des deux cotes, mais les deux formes ne sont pas
  interchangeables : l'une a `variant` et pas `name`, l'autre a `name` et
  `model` en `Model.Ref` (avec `providerID`). **Ce n'est pas un doublon a
  supprimer, c'est deux etages distincts** (configuration d'un cote, etat
  compile de l'autre), et il serait faux de les fusionner. Le vrai point de
  vigilance pour la suite du lot : le depot contient desormais deux structs
  decrivant « une commande ». C'est le meme piege que `sessionID`/`callID`, mais
  structurel et non typographique ; la deduplication.eventuelle appartient a un
  arbitrage, pas a un agent de portage.
- Tests (13) : les deux commandes enregistrees, substitution du jeton, prompt
  `review` insensible au repertoire, repertoire vide, ecrasement des valeurs
  preexistantes, rejeu idempotent, registre tiers preserve, noms de champs
  serialises (3 cles pour `init`, 4 pour `review`, aucune variante
  snake_case), cle snake_case ignoree a la lecture, forme snake_case de
  `providerID` refusee, substitutions `$`, premiere occurrence seulement,
  identifiant du plugin.
- Verification sans compilation : invites comparees octet par octet aux `.txt`
  d'origine, zero accent, zero caractere non-ASCII hors des deux invites,
  accolades et parentheses equilibrees, 13 `#[test]`.
- Aucun compilateur lance : ni cargo, ni rustc, ni rustup. Aucun commit, aucun
  push.
===FIN===
