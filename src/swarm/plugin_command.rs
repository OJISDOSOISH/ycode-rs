//! Portage Rust de `opencode/packages/core/src/plugin/command.ts`.
//!
//! ## Ce que la source est vraiment
//!
//! Vingt-cinq lignes : un reexport, deux imports d'invites texte, et un `define`
//! qui enregistre deux commandes integrees.
//!
//! ```ts
//! export const Plugin = define({
//!   id: "command",
//!   effect: Effect.fn(function* (ctx) {
//!     const location = yield* Location.Service
//!     yield* ctx.command.transform((draft) => {
//!       draft.update("init", (command) => {
//!         command.template = PROMPT_INITIALIZE.replace("${path}", location.project.directory)
//!         command.description = "guided AGENTS.md setup"
//!       })
//!       draft.update("review", (command) => {
//!         command.template = PROMPT_REVIEW.replace("${path}", location.project.directory)
//!         command.description = "review changes [commit|branch|pr], defaults to uncommitted"
//!         command.subtask = true
//!       })
//!     })
//!   }),
//! })
//! ```
//!
//! Le plugin **surcharge** deux entrees du registre des commandes, `init` et
//! `review`. Il n'ajoute aucun mecanisme : le registre, son tri et sa regle de
//! creation d'entree appartiennent a `core/command.ts`, et la transformation
//! elle-meme est le seul code du fichier.
//!
//! ## Le reexport de la ligne 1 est du code mort
//!
//! `export * as CommandPlugin from "./command"` pointe sur le fichier
//! lui-meme : depuis l'interieur de `plugin/command.ts`, le specificateur
//! `"./command"` se resout sur `plugin/command.ts`. C'est un espace de noms
//! circulaire, que le TypeScript tolere et qui n'apporte rien. En Rust le
//! module **est** l'espace de noms, donc il n'y a rien a retranscrire. Le meme
//! motif se retrouve ligne 1 de `core/command.ts` (`export * as CommandV2`).
//!
//! ## Les invites texte sont portees ici, pas importees
//!
//! `PROMPT_INITIALIZE` et `PROMPT_REVIEW` viennent de deux fichiers `.txt`
//! situes dans un **autre depot** que celui qui compile. Un `include_str!`
//! pointing vers `C:\Users\AI\Projects\Ycode\opencode\...` casserait la CI,
//! qui n'a pas ce depot sous la main. Les deux textes sont donc recopies dans
//! des constantes, verbatim, avec les fins de ligne de leur fichier d'origine.
//!
//! Deux caracteres non-ASCII sont conserves dans `PROMPT_REVIEW` : le tiret
//! cadratin de la phrase sur le contexte, et le deux en exposant de la notation
//! quadratique O(n2). Ce sont de la **donnee**, pas de la documentation : les
//! remplacer par `-` et `2` changerait le texte lu par le modele. C'est un
//! choix delibere, a verifier en relecture.
//!
//! ## Le piege 1 : `${path}` n'existe que dans une seule des deux invites
//!
//! `initialize.txt` contient `${path}` **une fois**, ligne 65. `review.txt` n'en
//! contient **aucune**. Les deux appels de la source ont donc des effets
//! franchement differents :
//!
//! - `PROMPT_INITIALIZE.replace("${path}", dir)` remplace le jeton, une fois ;
//! - `PROMPT_REVIEW.replace("${path}", dir)` ne trouve rien et renvoie le texte
//!   **rigoureusement inchange**, quel que soit le repertoire du projet.
//!
//! Un portage qui appliquerait le meme traitement aux deux prompts en
//! different, et un test verrouille cette asymetrie. Le `location` du service
//! est donc lu une fois et utilise une seule fois, ce qui est exact.
//!
//! ## Le piege 2 : `String.prototype.replace` n'est pas `str::replace`
//!
//! L'appel JavaScript a trois comportements qu'un portage Rust presse perd :
//!
//! 1. le motif est une **chaine**, pas une expression reguliere : il est
//!    recherche litteralement ;
//! 2. avec un motif chaine, seule la **premiere** occurrence est remplacee
//!    (`str::replace` remplace toutes les occurrences) ;
//! 3. dans le **remplacement**, les sequences `$` ont un sens : `$$` donne `$`,
//!    `$&` redonne la correspondance, `` $` `` redonne ce qui precede,
//!    `$'` redonne ce qui suit. Un `$` non suivi d'un de ces caracteres reste
//!    litteral, et `$1` reste litteral aussi puisqu'un motif chaine n'a aucun
//!    groupe capture.
//!
//! Le point 3 n'est pas academique : le remplacement est un chemin de systeme
//! de fichiers, et un repertoire nomme `C:\projet$$x` ou `C:\projet$&x`
//! produirait un invite faux avec un portage Rust naif. C'est la raison
//! d'etre de [`remplacer_premier`].
//!
//! ## Le piege des noms de champs
//!
//! Ce plugin n'ecrit que `template`, `description` et `subtask`, les trois en
//! minuscules des deux cotes : aucun `#[serde(rename)]` n'est requis ici. Le
//! champ renomme du contrat, c'est `providerID` porte par `Model.Ref`, qui
//! appartient a `core/command.rs` et n'est donc pas declare dans ce fichier.
//!
//! Un test verifie quand meme la sortie JSON exacte des deux commandes, et un
//! autre verifie que la forme snake_case n'est ni produite a l'ecriture, ni
//! honoree a la lecture.
//!
//! ## Aucun ternaire `?` et aucun coalescent `??` dans la source
//!
//! Aucun des deux operateurs n'apparait dans `plugin/command.ts`, donc aucune
//! chaine vide ne peut disparaitre par un test de veracite. Le point de
//! decision le plus proche est ailleurs : les quatre affectations sont des `=`
//! simples, jamais des `??=`. Une valeur deja presente est donc **remplacee**,
//! y compris quand elle vaut la chaine vide. Un `entry().or_insert()` ou un
//! `get_or_insert` serait faux ici.
//!
//! Le second point de decision est le repertoire vide. Il n'est pas teste par
//! la source : `.replace("${path}", "")` retire simplement le jeton et laisse
//! le reste du texte. Un portage qui filtrerait la chaine vide, ou qui la
//! convertirait en `None`, ajouterait un comportement absent de l'original.
//!
//! ## Aucun type n'est redeclare : deux modules deja sur le disque sont reutilises
//!
//! - Le registre et son editeur viennent de `crate::core::command`, qui porte
//!   deja `CommandInfo` (= `CommandV2.Info` de `packages/schema/src/command.ts`,
//!   expose par le SDK sous le nom `CommandV2Info`) et `CommandStore` (le
//!   `Draft` de `core/command.ts`, regle de creation comprise).
//! - La localisation vient de `crate::swarm::location`, dont `Interface`
//! expose deja `project.directory`.
//!
//! Redeklarer l'un des deux dans ce fichier creerait un troisieme contrat pour
//! la meme forme, capable de diverger en silence. C'est le doublon que ce
//! portage cherche a eviter, pas une economie de lignes.
//!
//! ### Ce que ce plugin ne fait pas, et que le registre fait a sa place
//!
//! `Draft.update` (`core/command.ts:37`) fait trois choses :
//!
//! ```ts
//! const current = draft.commands.get(name) ?? ({ name, template: "" })
//! if (!draft.commands.has(name)) draft.commands.set(name, current)
//! update(current)
//! current.name = name
//! ```
//!
//! Donc `update` **cree** l'entree si elle est absente, et force `name` a la
//! cle meme si la fonction de mise a jour essaie de le changer. `init` et
//! `review` n'existent pas au depart : ce plugin les cree. C'est le comportement
//! du registre, porte une seule fois, et un test le verifie sur le resultat
//! plutot que sur l'implementation.
//!
//! ## Les invites ne sont pas nettoyees de leurs `$ARGUMENTS`
//!
//! Les deux textes contiennent `$ARGUMENTS`. C'est un autre jeton, traite plus
//! tard par le moteur de commandes, et ce fichier n'y touche pas. Le
//! delimiteur `$$` de la substitution ne s'y applique donc pas : seule la
//! chaine de remplacement passe par les substitutions `$`.
//!
//! ## Note d integration
//!
//! Ce fichier attend `pub mod plugin_command;` dans `src/swarm/mod.rs`, que je
//! ne touche pas. Il depend de `crate::core::command` et de
//! `crate::swarm::location`, tous deux deja `pub`.

use crate::core::command::CommandStore;
use crate::swarm::location::Interface as LocationInterface;

/// Identifiant du plugin tel qu'il est enregistre par le moteur interne.
///
/// En TS : la propriete `id` de l'objet passe a `define`.
///
/// C'est la valeur sous laquelle `CommandPlugin.Plugin` est ajoute au registre
/// dans `plugin/internal.ts:112`, entre `AgentPlugin.Plugin` et
/// `SkillPlugin.Plugin`. L'ordre de cet appel est significatif : c'est lui qui
/// fixe l'ordre de rejeu des transformations du domaine commandes.
pub const ID: &str = "command";

/// Nom de la commande d'initialisation.
pub const COMMANDE_INIT: &str = "init";

/// Nom de la commande de relecture.
pub const COMMANDE_REVIEW: &str = "review";

/// Le jeton remplace dans les invites.
///
/// En TS : la chaine litterale `"${path}"` ecrite deux fois dans le fichier.
/// Elle n'apparait qu'une fois dans `PROMPT_INITIALIZE` et jamais dans
/// `PROMPT_REVIEW`.
pub const JETON_CHEMIN: &str = "${path}";

/// Description affichee pour `init`.
///
/// En TS : `"guided AGENTS.md setup"`. Affectation simple, donc elle ecrase
/// toute description preexistante, chaine vide comprise.
pub const DESCRIPTION_INIT: &str = "guided AGENTS.md setup";

/// Description affichee pour `review`.
///
/// En TS : `"review changes [commit|branch|pr], defaults to uncommitted"`.
/// Les crochets sont dans la chaine, pas dans le formatage.
pub const DESCRIPTION_REVIEW: &str = "review changes [commit|branch|pr], defaults to uncommitted";

/// Invite de la commande `init`, recopie de `plugin/command/initialize.txt`.
///
/// Le `${path}` de la derniere ligne est le seul jeton du fichier, et le
/// plugin le remplace par le repertoire du **projet** (`location.project
/// .directory`), pas par le repertoire de travail.
pub const PROMPT_INITIALIZE: &str = r#"Create or update `AGENTS.md` for this repository.

The goal is a compact instruction file that helps future OpenCode sessions avoid mistakes and ramp up quickly. Every line should answer: "Would an agent likely miss this without help?" If not, leave it out.

User-provided focus or constraints (honor these):
$ARGUMENTS

## How to investigate

Read the highest-value sources first:
- `README*`, root manifests, workspace config, lockfiles
- build, test, lint, formatter, typecheck, and codegen config
- CI workflows and pre-commit / task runner config
- existing instruction files (`AGENTS.md`, `CLAUDE.md`, `.cursor/rules/`, `.cursorrules`, `.github/copilot-instructions.md`)
- repo-local OpenCode config such as `opencode.json`

If architecture is still unclear after reading config and docs, inspect a small number of representative code files to find the real entrypoints, package boundaries, and execution flow. Prefer reading the files that explain how the system is wired together over random leaf files.

Prefer executable sources of truth over prose. If docs conflict with config or scripts, trust the executable source and only keep what you can verify.

## What to extract

Look for the highest-signal facts for an agent working in this repo:
- exact developer commands, especially non-obvious ones
- how to run a single test, a single package, or a focused verification step
- required command order when it matters, such as `lint -> typecheck -> test`
- monorepo or multi-package boundaries, ownership of major directories, and the real app/library entrypoints
- framework or toolchain quirks: generated code, migrations, codegen, build artifacts, special env loading, dev servers, infra deploy flow
- testing quirks: fixtures, integration test prerequisites, snapshot workflows, required services, flaky or expensive suites
- important constraints from existing instruction files worth preserving

Good `AGENTS.md` content is usually hard-earned context that took reading multiple files to infer.

## Questions

Only ask the user questions if the repo cannot answer something important. Use the `question` tool for one short batch at most.

Good questions:
- undocumented team conventions
- branch / PR / release expectations
- missing setup or test prerequisites that are known but not written down

Do not ask about anything the repo already makes clear.

## Writing rules

Include only high-signal, repo-specific guidance such as:
- exact commands and shortcuts the agent would otherwise guess wrong
- architecture notes that are not obvious from filenames
- conventions that differ from language or framework defaults
- setup requirements, environment quirks, and operational gotchas
- references to existing instruction sources that matter

Exclude:
- generic software advice
- long tutorials or exhaustive file trees
- obvious language conventions
- speculative claims or anything you could not verify
- content better stored in another file referenced via `opencode.json` `instructions`

When in doubt, omit.

Prefer short sections and bullets. If the repo is simple, keep the file simple. If the repo is large, summarize the few structural facts that actually change how an agent should work.

If `AGENTS.md` already exists at `${path}`, improve it in place rather than rewriting blindly. Preserve verified useful guidance, delete fluff or stale claims, and reconcile it with the current codebase.
"#;

/// Invite de la commande `review`, recopie de `plugin/command/review.txt`.
///
/// Contredit au fichier voisin, ce texte ne contient **aucun** `${path}`. Le
/// `.replace` de la source ne trouve donc rien et le renvoie tel quel : le
/// repertoire du projet n'a aucune influence sur cette invite.
pub const PROMPT_REVIEW: &str = r#"You are a code reviewer. Your job is to review code changes and provide actionable feedback.

---

Input: $ARGUMENTS

---

## Determining What to Review

Based on the input provided, determine which type of review to perform:

1. **No arguments (default)**: Review all uncommitted changes
   - Run: `git diff` for unstaged changes
   - Run: `git diff --cached` for staged changes
   - Run: `git status --short` to identify untracked (net new) files

2. **Commit hash** (40-char SHA or short hash): Review that specific commit
   - Run: `git show $ARGUMENTS`

3. **Branch name**: Compare current branch to the specified branch
   - Run: `git diff $ARGUMENTS...HEAD`

4. **PR URL or number** (contains "github.com" or "pull" or looks like a PR number): Review the pull request
   - Run: `gh pr view $ARGUMENTS` to get PR context
   - Run: `gh pr diff $ARGUMENTS` to get the diff

Use best judgement when processing input.

---

## Gathering Context

**Diffs alone are not enough.** After getting the diff, read the entire file(s) being modified to understand the full context. Code that looks wrong in isolation may be correct given surrounding logic—and vice versa.

- Use the diff to identify which files changed
- Use `git status --short` to identify untracked files, then read their full contents
- Read the full file to understand existing patterns, control flow, and error handling
- Check for existing style guide or conventions files (CONVENTIONS.md, AGENTS.md, .editorconfig, etc.)

---

## What to Look For

**Bugs** - Your primary focus.
- Logic errors, off-by-one mistakes, incorrect conditionals
- If-else guards: missing guards, incorrect branching, unreachable code paths
- Edge cases: null/empty/undefined inputs, error conditions, race conditions
- Security issues: injection, auth bypass, data exposure
- Broken error handling that swallows failures, throws unexpectedly or returns error types that are not caught.

**Structure** - Does the code fit the codebase?
- Does it follow existing patterns and conventions?
- Are there established abstractions it should use but doesn't?
- Excessive nesting that could be flattened with early returns or extraction

**Performance** - Only flag if obviously problematic.
- O(n²) on unbounded data, N+1 queries, blocking I/O on hot paths

**Behavior Changes** - If a behavioral change is introduced, raise it (especially if it's possibly unintentional).

---

## Before You Flag Something

**Be certain.** If you're going to call something a bug, you need to be confident it actually is one.

- Only review the changes - do not review pre-existing code that wasn't modified
- Don't flag something as a bug if you're unsure - investigate first
- Don't invent hypothetical problems - if an edge case matters, explain the realistic scenario where it breaks
- If you need more context to be sure, use the tools below to get it

**Don't be a zealot about style.** When checking code against conventions:

- Verify the code is *actually* in violation. Don't complain about else statements if early returns are already being used correctly.
- Some "violations" are acceptable when they're the simplest option. A `let` statement is fine if the alternative is convoluted.
- Excessive nesting is a legitimate concern regardless of other style choices.

---

## Tools

Use these to inform your review:

- **Explore agent** - Find how existing code handles similar problems. Check patterns, conventions, and prior art before claiming something doesn't fit.
- **Exa Code Context** - Verify correct usage of libraries/APIs before flagging something as wrong.
- **Web Search** - Research best practices if you're unsure about a pattern.

If you're uncertain about something and can't verify it with these tools, say "I'm not sure about X" rather than flagging it as a definite issue.

---

## Output

1. If there is a bug, be direct and clear about why it is a bug.
2. Clearly communicate severity of issues. Do not overstate severity.
3. Critiques should clearly and explicitly communicate the scenarios, environments, or inputs that are necessary for the bug to arise. The comment should immediately indicate that the issue's severity depends on these factors.
4. Your tone should be matter-of-fact and not accusatory or overly positive. It should read as a helpful AI assistant suggestion without sounding too much like a human reviewer.
5. Write so the reader can quickly understand the issue without reading too closely.
6. AVOID flattery, do not give any comments that are not helpful to the reader.
"#;

/// Le plugin de commandes integrees.
///
/// En TS : `export const Plugin = define({ id: "command", effect })`.
///
/// `define` (`plugin/internal.ts:59`) se contente de retourner son argument,
/// donc le type `Plugin` se resume a un identifiant et a une fonction. Le champ
/// `effect` est porte par [`CommandPlugin::transformer`], qui est l'appel
/// unique a `ctx.command.transform`.
///
/// La valeur de retour de cet appel, une inscription de transformation, n'est
/// reliee a rien dans la source. La transformation survit au plugin parce
/// qu'elle est enregistree dans le service commandes, pas dans le plugin. Une
/// fonction qui ne renvoie rien est donc le comportement observable exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CommandPlugin;

impl CommandPlugin {
    /// Le meme identifiant que le [`ID`] du module.
    pub const ID: &'static str = ID;

    /// Enregistre la transformation des commandes.
    ///
    /// Equivalent de l'effet complet du plugin, moins la lecture du service
    /// de localisation, qui est remplacee par la localisation recue en
    /// parametre : le service lui-meme n'est pas porte par ce fichier.
    pub fn transformer(commandes: &mut CommandStore, location: &LocationInterface) {
        appliquer(commandes, location);
    }
}

/// Corps du template de la commande `init`.
///
/// Equivalent de
/// `PROMPT_INITIALIZE.replace("${path}", location.project.directory)`.
///
/// Le motif est une chaine, donc la recherche est litterale et seule la
/// premiere occurrence est remplacee. Voir [`remplacer_premier`] pour le
/// traitement des `$` dans la chaine de remplacement.
pub fn template_init(dossier_projet: &str) -> String {
    remplacer_premier(PROMPT_INITIALIZE, JETON_CHEMIN, dossier_projet)
}

/// Corps du template de la commande `review`.
///
/// Equivalent de
/// `PROMPT_REVIEW.replace("${path}", location.project.directory)`.
///
/// Le prompt de relecture ne contient aucun `${path}`, donc cette fonction
/// renvoie le texte inchange pour n'importe quel repertoire. Elle est portee
/// quand meme, parce que c'est l'appel de la source et que le jour ou le
/// prompt gagne un jeton, elle commencera a produire un effet sans qu'on ait a
/// y revenir.
pub fn template_review(dossier_projet: &str) -> String {
    remplacer_premier(PROMPT_REVIEW, JETON_CHEMIN, dossier_projet)
}

/// Applique au registre la transformation des deux commandes integrees.
///
/// Equivalent du corps de la fonction passee a `ctx.command.transform`.
///
/// L'ordre est celui de la source : `init` d'abord, `review` ensuite. Comme
/// les deux visent des cles differentes, l'ordre n'est pas observable sur le
/// resultat ; il reste lisible.
///
/// Les quatre affectations sont des egalites simples. Elles ecrasent donc
/// toute valeur preexistante, chaine vide comprise, et jamais une valeur par
/// defaut. `init` ne touche pas `subtask`, `review` le met a `true` : les deux
/// commandes n'ont pas le meme comportement sur ce champ.
pub fn appliquer(commandes: &mut CommandStore, location: &LocationInterface) {
    let dossier = location.info.project.directory.as_str();

    commandes.update(COMMANDE_INIT, |commande| {
        commande.template = template_init(dossier);
        commande.description = Some(DESCRIPTION_INIT.to_string());
    });

    commandes.update(COMMANDE_REVIEW, |commande| {
        commande.template = template_review(dossier);
        commande.description = Some(DESCRIPTION_REVIEW.to_string());
        commande.subtask = Some(true);
    });
}

/// Premiere occurrence d'un motif chaine, remplacee comme en JavaScript.
///
/// Equivalent de `String.prototype.replace(texte, remplacement)` quand le
/// motif est une chaine, ce qui est le cas de l'appel de la source.
///
/// Deux departures de `str::replace` sont volontaires :
///
/// 1. seule la **premiere** occurrence est remplacee, comme le fait un motif
///    chaine en JavaScript ;
/// 2. les sequences `$` de la chaine de remplacement sont interpretees :
///    `$$` donne un `$`, `$&` redonne le motif trouve, `` $` `` redonne la
///    partie qui precede, `$'` redonne celle qui suit. Tout autre `$` reste
///    litteral, y compris `$1`, puisqu'un motif chaine n'a aucun groupe
///    capture.
///
/// Si le motif est absent, le texte est rendu tel quel : c'est le cas de la
/// commande `review`, dont le prompt ne contient aucun jeton.
pub fn remplacer_premier(texte: &str, motif: &str, remplacement: &str) -> String {
    let debut = match texte.find(motif) {
        Some(position) => position,
        None => return texte.to_string(),
    };
    let fin = debut + motif.len();
    let avant = &texte[..debut];
    let trouve = &texte[debut..fin];
    let apres = &texte[fin..];

    let substitue = substituer(remplacement, avant, trouve, apres);
    let mut sortie = String::with_capacity(avant.len() + substitue.len() + apres.len());
    sortie.push_str(avant);
    sortie.push_str(&substitue);
    sortie.push_str(apres);
    sortie
}

/// Interprete les sequences `$` d'une chaine de remplacement.
///
/// Regle de `GetSubstitution`, reduite au cas d'un motif chaine : les quatre
/// sequences speciaux sont traitees, tout le reste est copie tel quel. Les
/// trois morceaux de la correspondance sont passes en parametres parce que
/// `$&`, `` $` `` et `$'` y font reference.
fn substituer(remplacement: &str, avant: &str, trouve: &str, apres: &str) -> String {
    let octets = remplacement.as_bytes();
    let mut sortie = String::with_capacity(remplacement.len());
    let mut indexe = 0;

    while indexe < octets.len() {
        if octets[indexe] != b'$' {
            let largeur = largeur_du_caractere(octets[indexe]);
            sortie.push_str(&remplacement[indexe..indexe + largeur]);
            indexe += largeur;
            continue;
        }
        match octets.get(indexe + 1) {
            Some(b'$') => {
                sortie.push('$');
                indexe += 2;
            }
            Some(b'&') => {
                sortie.push_str(trouve);
                indexe += 2;
            }
            Some(b'`') => {
                sortie.push_str(avant);
                indexe += 2;
            }
            Some(b'\'') => {
                sortie.push_str(apres);
                indexe += 2;
            }
            // `$` final, `$1` ou `$x` : aucun groupe capture n'existe avec un
            // motif chaine, donc le `$` reste litteral et le caractere suivant
            // est copie au tour suivant.
            _ => {
                sortie.push('$');
                indexe += 1;
            }
        }
    }

    sortie
}

/// Nombre d'octets du caractere UTF-8 commencant par `premier`.
///
/// Le parcours de `substituer` avance octet par octet, donc il faut sauter
/// d'un bloc a la fois pour ne pas couper un caractere en son milieu. La
/// branche par defaut ne sert qu'a rester total : sur une chaine Rust valide,
/// un octet de continuite n'arrive jamais en debut de position.
fn largeur_du_caractere(premier: u8) -> usize {
    match premier {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::command::CommandInfo;
    use crate::swarm::location::{Info as LocationInfo, ProjectInfo};

    /// Localisation de test, dont le repertoire de **projet** est donne.
    fn localisation(dossier_projet: &str) -> LocationInterface {
        LocationInterface {
            info: LocationInfo {
                directory: "/depot/sous/dossier".to_string(),
                workspace_id: None,
                project: ProjectInfo {
                    id: "pro_1".to_string(),
                    directory: dossier_projet.to_string(),
                },
            },
            vcs: None,
        }
    }

    /// Le plugin enregistre bien les deux commandes, avec le texte, la
    /// description et le drapeau de sous-tache de la source.
    #[test]
    fn les_deux_commandes_integrees_sont_enregistrees_avec_leur_texte() {
        let mut commandes = CommandStore::new();

        appliquer(&mut commandes, &localisation("/depot"));

        let init = commandes.get(COMMANDE_INIT).expect("commande init absente");
        assert_eq!(init.name, "init");
        assert_eq!(init.description.as_deref(), Some("guided AGENTS.md setup"));
        assert_eq!(init.template, PROMPT_INITIALIZE.replace("${path}", "/depot"));
        assert_eq!(init.subtask, None, "init ne touche pas subtask");
        assert_eq!(init.agent, None);
        assert_eq!(init.model, None);

        let review = commandes.get(COMMANDE_REVIEW).expect("commande review absente");
        assert_eq!(review.name, "review");
        assert_eq!(review.description.as_deref(), Some(DESCRIPTION_REVIEW));
        assert_eq!(review.subtask, Some(true));
    }

    /// Le jeton de chemin disparait de l'invite `init` et y laisse le
    /// repertoire du projet, entoure des crochets graves de la phrase source.
    #[test]
    fn le_jeton_de_chemin_vient_a_la_place_dans_le_texte_init() {
        let mut commandes = CommandStore::new();

        appliquer(&mut commandes, &localisation("/depot/racine"));

        let init = commandes.get(COMMANDE_INIT).expect("commande init absente");
        assert!(!init.template.contains(JETON_CHEMIN), "le jeton doit avoir disparu");
        assert!(
            init.template.contains("`/depot/racine`"),
            "le repertoire du projet doit apparaitre entre crochets graves"
        );
    }

    /// L'invite `review` ne contient aucun jeton : le repertoire du projet
    /// n'a donc aucune prise sur elle, et deux projets differents donnent le
    /// meme texte. C'est l'asimetrie entre les deux prompts.
    #[test]
    fn le_texte_review_est_identique_pour_n_importe_quel_repertoire() {
        assert!(!PROMPT_REVIEW.contains(JETON_CHEMIN), "le prompt review ne porte pas de jeton");
        assert_eq!(PROMPT_INITIALIZE.matches(JETON_CHEMIN).count(), 1);
        assert_eq!(PROMPT_REVIEW.matches(JETON_CHEMIN).count(), 0);

        assert_eq!(template_review("/depot"), PROMPT_REVIEW);
        assert_eq!(template_review(""), PROMPT_REVIEW);
        assert_eq!(template_review("C:\\ailleurs"), PROMPT_REVIEW);

        let mut premier = CommandStore::new();
        let mut second = CommandStore::new();
        appliquer(&mut premier, &localisation("/un"));
        appliquer(&mut second, &localisation("/deux"));

        assert_eq!(
            premier.get(COMMANDE_REVIEW).expect("review absente").template,
            second.get(COMMANDE_REVIEW).expect("review absente").template
        );
    }

    /// Un repertoire de projet vide retire le jeton et laisse le reste du
    /// texte intact. Rien n'est teste en veracite dans la source : le
    /// remplacement a lieu, il est simplement vide.
    #[test]
    fn un_repertoire_vide_retire_le_jetet_sans_abrire_une_autre_valeur() {
        let mut commandes = CommandStore::new();

        appliquer(&mut commandes, &localisation(""));

        let init = commandes.get(COMMANDE_INIT).expect("commande init absente");
        assert!(!init.template.contains(JETON_CHEMIN));
        assert!(
            init.template.contains("already exists at , improve it in place"),
            "le jeton disparait, la phrase reste : {}",
            init.template.lines().last().unwrap_or("")
        );
        for interdit in ["undefined", "null", "None", "Optional"] {
            assert!(!init.template.contains(interdit), "le repertoire vide ne doit pas laisser {interdit}");
        }
    }

    /// Les affectations sont des egalites simples : une valeur deja presente
    /// est ecrasee, chaine vide comprise. Et `init` laisse passer un `subtask`
    /// preexistant-la alors que `review` le force a `true`.
    #[test]
    fn les_valeurs_preexistantes_sont_ecrasees_sauf_subtask_pour_init() {
        let mut commandes = CommandStore::new();
        commandes.update(COMMANDE_INIT, |commande| {
            commande.template = String::new();
            commande.description = Some(String::new());
            commande.subtask = Some(false);
        });
        commandes.update(COMMANDE_REVIEW, |commande| {
            commande.template = "vieille invite".to_string();
            commande.description = Some("vieille description".to_string());
            commande.subtask = Some(false);
        });

        appliquer(&mut commandes, &localisation("/depot"));

        let init = commandes.get(COMMANDE_INIT).expect("init absente");
        assert_ne!(init.template, "", "le template vide est ecrase");
        assert_eq!(init.description.as_deref(), Some(DESCRIPTION_INIT));
        assert_eq!(
            init.subtask,
            Some(false),
            "le bloc init n'ecrit pas subtask, donc false survit"
        );

        let review = commandes.get(COMMANDE_REVIEW).expect("review absente");
        assert_ne!(review.template, "vieille invite");
        assert_eq!(review.description.as_deref(), Some(DESCRIPTION_REVIEW));
        assert_eq!(review.subtask, Some(true), "le bloc review force subtask a true");
    }

    /// Les transformations sont rejouables : les passer deux fois donne le
    /// meme registre, au caractere pres.
    #[test]
    fn deux_pass_successives_donnent_le_meme_registre() {
        let mut premier = CommandStore::new();
        appliquer(&mut premier, &localisation("/depot"));
        let avant = premier.list();

        let mut second = CommandStore::new();
        appliquer(&mut second, &localisation("/depot"));
        appliquer(&mut second, &localisation("/depot"));

        assert_eq!(premier.list(), second.list());
        assert_eq!(avant, second.list());
    }

    /// Un registre vide recoit exactement les deux commandes, et une commande
    /// tierce deja presente n'est pas touchee. La creation des entrees vient
    /// du registre, pas de ce plugin.
    #[test]
    fn un_registre_vide_recoit_deux_commandes_et_laisse_tiers() {
        let mut commandes = CommandStore::new();
        commandes.update("deploy", |commande| {
            commande.template = "deploiement".to_string();
            commande.description = Some("envoyer le site".to_string());
        });

        appliquer(&mut commandes, &localisation("/depot"));

        let noms: Vec<String> = commandes.list().iter().map(|c| c.name.clone()).collect();
        assert_eq!(noms, vec!["deploy", "init", "review"]);

        let deploy = commandes.get("deploy").expect("deploy absent");
        assert_eq!(deploy.template, "deploiement");
        assert_eq!(deploy.description.as_deref(), Some("envoyer le site"));
        assert_eq!(deploy.subtask, None);
    }

    /// Les noms de champs produits sont exactement ceux du TypeScript : six
    /// cles possibles, en minuscules, et aucune variante snake_case. Ce test
    /// verrouille l'echange avec le TypeScript.
    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        let mut commandes = CommandStore::new();
        appliquer(&mut commandes, &localisation("/depot"));

        let init = serde_json::to_value(commandes.get(COMMANDE_INIT).expect("init absente")).unwrap();
        let objet = init.as_object().unwrap();
        assert_eq!(objet.len(), 3, "init ne porte que trois cles : {objet:?}");
        for nom in ["name", "template", "description"] {
            assert!(objet.contains_key(nom), "cle absente du JSON : {nom}");
        }
        assert_eq!(objet.get("name").and_then(|v| v.as_str()), Some("init"));
        assert_eq!(objet.get("description").and_then(|v| v.as_str()), Some(DESCRIPTION_INIT));
        assert!(objet.get("subtask").is_none(), "subtask absent quand il vaut undefined");
        assert!(objet.get("agent").is_none());
        assert!(objet.get("model").is_none());
        assert!(objet.get("sub_task").is_none());
        assert!(objet.get("Subtask").is_none());

        let review = serde_json::to_value(commandes.get(COMMANDE_REVIEW).expect("review absente")).unwrap();
        let objet = review.as_object().unwrap();
        assert_eq!(objet.len(), 4, "review porte une cle de plus : {objet:?}");
        for nom in ["name", "template", "description", "subtask"] {
            assert!(objet.contains_key(nom), "cle absente du JSON : {nom}");
        }
        assert_eq!(objet.get("subtask").and_then(|v| v.as_bool()), Some(true));
    }

    /// Une cle snake_case lue n'est jamais honoree : elle est ignoree, et la
    /// reecriture ne la reproduit pas. Le contrat porte `subtask`, pas
    /// `sub_task` : taper la mauvaise casse ne fait pas lever d'erreur, ce qui
    /// la rend d'autant plus dangereuse.
    #[test]
    fn une_cle_snake_case_lue_est_ignoree_et_non_reprise_a_l_ecriture() {
        let lue: CommandInfo =
            serde_json::from_str(r#"{"name":"review","template":"t","sub_task":true}"#).expect("lecture");

        assert_eq!(lue.subtask, None, "sub_task n'alimente pas subtask");
        assert_eq!(lue.name, "review");

        let objet = serde_json::to_value(&lue).unwrap();
        let objet = objet.as_object().unwrap();
        assert!(objet.get("subtask").is_none());
        assert!(objet.get("sub_task").is_none());
    }

    /// Le seul champ renomme du contrat des commandes est `providerID`, porte
    /// par `Model.Ref` dans `core/command.rs`. Sa forme snake_case est donc
    /// refusee a la lecture, la vraie forme etant obligatoire.
    #[test]
    fn la_forme_snake_case_du_champ_provider_id_est_refusee_a_la_lecture() {
        // La forme correcte passe.
        let bonne = r#"{"name":"x","template":"t","model":{"id":"m1","providerID":"p1"}}"#;
        let info: CommandInfo = serde_json::from_str(bonne).expect("la forme providerID doit passer");
        let objet = serde_json::to_value(&info).unwrap();
        assert_eq!(objet["model"]["providerID"], "p1");
        assert!(objet["model"].get("providerId").is_none());
        assert!(objet["model"].get("provider_id").is_none());

        // Les deux formes erronees sont refusees : `providerID` est
        // obligatoire, son absence fait echouer la lecture.
        for mauvais in [
            r#"{"name":"x","template":"t","model":{"id":"m1","provider_id":"p1"}}"#,
            r#"{"name":"x","template":"t","model":{"id":"m1","providerId":"p1"}}"#,
        ] {
            assert!(
                serde_json::from_str::<CommandInfo>(mauvais).is_err(),
                "cette forme ne doit pas etre acceptee : {mauvais}"
            );
        }

        // Et `subtask` reste un booleen, jamais une chaine.
        assert!(serde_json::from_str::<CommandInfo>(r#"{"name":"x","template":"t","subtask":"oui"}"#).is_err());
        assert!(serde_json::from_str::<CommandInfo>(r#"{"template":"t"}"#).is_err());
    }

    /// Les sequences `$` de la chaine de remplacement sont interpretees comme
    /// en JavaScript, ce qui est la seule raison d'etre de [`remplacer_premier`]
    /// : le remplacement est un chemin, et un chemin peut contenir `$$`.
    #[test]
    fn les_dollars_du_remplacement_sont_interpretes_comme_en_javascript() {
        let texte = "a${path}b";

        // `$$` donne un seul `$`.
        assert_eq!(remplacer_premier(texte, JETON_CHEMIN, "$$"), "a$b");
        // `$&` redonne le motif trouve : le resultat est identique a l'entree.
        assert_eq!(remplacer_premier(texte, JETON_CHEMIN, "$&"), "a${path}b");
        // `` $` `` redonne ce qui precede, `$'` ce qui suit.
        assert_eq!(remplacer_premier(texte, JETON_CHEMIN, "$`"), "a" + "a" + "b");
        assert_eq!(remplacer_premier(texte, JETON_CHEMIN, "$'"), "a" + "b" + "b");
        // Aucun groupe capture n'existe avec un motif chaine : `$1` reste
        // litteral, comme un `$` suivi de n'importe quoi.
        assert_eq!(remplacer_premier(texte, JETON_CHEMIN, "$1"), "a$1b");
        assert_eq!(remplacer_premier(texte, JETON_CHEMIN, "$"), "a$b");
        // Un remplacement vide retire simplement le motif.
        assert_eq!(remplacer_premier(texte, JETON_CHEMIN, ""), "ab");
        // Un motif absent rend le texte inchange.
        assert_eq!(remplacer_premier("rien ici", JETON_CHEMIN, "/depot"), "rien ici");
    }

    /// Le remplacement de la premiere occurrence seulement, et la coupure
    /// correcte des caracteres multi-octets dans la chaine de remplacement.
    ///
    /// Les accents et les emojis sont ecrits en echappement Unicode : le
    /// fichier reste en ASCII, mais la chaine construite fait bien deux,
    /// trois et quatre octets par caractere.
    #[test]
    fn seule_la_premiere_occurrence_est_remplacee() {
        let texte = "${path} puis ${path} encore";

        assert_eq!(remplacer_premier(texte, JETON_CHEMIN, "/un"), "/un puis ${path} encore");

        // Deux octets, devant et derriere le motif.
        let accents = "d\u{e9}but ${path} \u{e9}uee";
        assert_eq!(remplacer_premier(accents, JETON_CHEMIN, "ee"), "d\u{e9}but ee \u{e9}uee");

        // Trois octets puis quatre octets, de part et d'autre du motif.
        let large = "\u{20ac} ${path} \u{1f600}";
        assert_eq!(remplacer_premier(large, JETON_CHEMIN, "$"), "\u{20ac} $ \u{1f600}");

        // Un motif vide se comporte comme en JavaScript : insertion en tete.
        assert_eq!(remplacer_premier("abc", "", "X"), "Xabc");
    }

    /// Le plugin s'annonce sous le bon identifiant, et le construire ne
    /// touche a rien tant qu'on ne l'appelle pas.
    #[test]
    fn le_plugin_s_annonce_sous_le_bon_identifiant() {
        assert_eq!(ID, "command");
        assert_eq!(CommandPlugin::ID, "command");
        assert_eq!(CommandPlugin, CommandPlugin::default());

        let mut commandes = CommandStore::new();
        assert!(commandes.list().is_empty(), "construire le plugin ne doit rien modifier");

        CommandPlugin::transformer(&mut commandes, &localisation("/depot"));
        assert_eq!(commandes.list().len(), 2);
    }
}
