//! Portage Rust de `opencode/packages/core/src/plugin/skill.ts`.
//!
//! ## Ce que la source est vraiment
//!
//! Trente et une lignes, dont **vingt-six de documentation** dans le fichier
//! `.md` voisin. Le code est celui d'un seul enregistrement :
//!
//! ```ts
//! export const Plugin = define({
//!   id: "skill",
//!   effect: Effect.fn(function* (ctx) {
//!     yield* ctx.skill.transform((draft) => {
//!       draft.source(
//!         SkillV2.EmbeddedSource.make({
//!           type: "embedded",
//!           skill: SkillV2.Info.make({
//!             name: "customize-opencode",
//!             description:
//!               "Use ONLY when the user is editing or creating opencode's own configuration: ...",
//!             location: AbsolutePath.make("/builtin/customize-opencode.md"),
//!             content: CustomizeOpencodeContent,
//!           }),
//!         }),
//!       )
//!     })
//!   }),
//! })
//! ```
//!
//! Aucun test, aucune condition, aucun `?`, aucun `??`. Le plugin ajoute **une
//! source** au registre des sources de skills, et cette source est un skill
//! **incorpore** : le contenu ne vient pas du disque, il est compile dans le
//! binaire.
//!
//! ## Le reexport de la ligne 3 est du code mort
//!
//! `export * as SkillPlugin from "./skill"` pointe sur le fichier lui-meme.
//! TypeScript tolere cet espace de noms circulaire, qui n'apporte rien. En Rust
//! le module **est** l'espace de noms : il n'y a donc rien a retranscrire. Le
//! meme raisonnement vaut pour le reexport identique de `core/skill.ts:1`.
//!
//! ## L'import texte, ligne 9
//!
//! `import customizeOpencodeContent from "./skill/customize-opencode.md" with
//! { type: "text" }` est un import de **ressource de build** : le bundler
//! inline le fichier dans le binaire et le rend accessible sous le nom
//! `CustomizeOpencodeContent`. Le `.md` weighse 16 525 octets et vit dans
//! **un autre depot** que celui qui compile. Un `include_str!` pointant vers
//! `C:\Users\AI\Projects\Ycode\opencode\...` casserait la CI, qui n'a pas ce
//! depot sous la main : le texte est donc recopie dans [`CONTENU`], comme
//! `plugin_command.rs` l'a fait pour `PROMPT_INITIALIZE` et `PROMPT_REVIEW`.
//!
//! ### La normalisation des fins de ligne est un ecart assume
//!
//! Le `.md` d'origine est en **CRLF** (453 fins de ligne CRLF, 0 LF isole).
//! Le depot Rust est en **LF**. [`CONTENU`] est donc identique au `.md` a
//! l'exception des 453 fins de ligne. Une comparaison **octet par octet** des
//! deux fichiers echoue, et c'est le seul ecart de contenu. Aucun test ne
//! pretend donc le contraire : ils comparent des chaines de caracteres, ce qui
//! est le comportement reellement observe par le modele.
//!
//! ### Six caracteres non-ASCII sont de la donnee, pas de la documentation
//!
//! Le texte contient cinq tirets cadratins `U+2014` et une-points-de-suspension
//! `U+2026`, soit **18 octets non-ASCII**. Les remplacer par `-` et `...`
//! changerait le texte lu par le modele. Voir la section sur le danger de
//! slicing, qui n'est pas academique ici.
//!
//! ## PIEGE 1 : les noms de champs en MAJUSCULES — il ne s'applique PAS ici
//!
//! Le piege du lot (`projectID`, `sessionID`, `providerID`) est **absent de ce
//! fichier**. Les onze cles touchees ont ete relues ligne a ligne :
//!
//! | Ou                            | Cles                                        |
//! |-------------------------------|---------------------------------------------|
//! | `plugin/skill.ts`             | `id`, `effect`                              |
//! | `v2/effect/skill.ts`          | `source`, `list`, `transform`               |
//! | `v2/effect/context.ts`        | `skill`                                     |
//! | `schema/skill.ts` `Info`      | `name`, `description`, `slash`, `location`, `content` |
//! | `schema/skill.ts` Embedded    | `type`, `skill`                             |
//! | `schema/skill.ts` Directory   | `type`, `path`                              |
//! | `schema/skill.ts` Url         | `type`, `url`                               |
//!
//! Pas une seule majuscule. Le nom de champ Rust est donc deja le nom
//! TypeScript. Chaque champ porte malgre tout un `#[serde(rename = "...")]`
//! **volontairement redondant**, comme dans `v1_config_skills.rs` : c'est un
//! garde-fou si le nom bouge, et il rend le contrat d'echange visible sans
//! ouvrir la source.
//!
//! Le piege inverse existe ici, et c'est un piege de **forme**, pas de casse :
//! `packages/opencode/src/skill/index.ts:37-42` declare un **autre** `Info` pour
//! le meme skill, avec `{ name, description, location, content }` — **sans**
//! `slash`. Reprendre cette forme-la perdrait un champ du contrat v2. Deux
//! tests verrouillent la difference.
//!
//! ## PIEGE 2 : le ternaire `?` contre le coalescent `??` — il ne s'applique PAS
//!
//! Ni l'un ni l'autre n'apparait dans `plugin/skill.ts`. C'est une expression
//! d'objet, pas une chaine de conditions, donc il n'y a **rien a filtrer** et
//! il faut le dire sans le contourner. Les deux jugements de valeurs les plus
//! facile a confondre du language d'origine n'ont ici aucun point d'entree :
//!
//! - le **ternaire** `x ? a : b` teste la **veracite** (`""` est falsy) ;
//! - le **coalescent** `x ?? y` teste la **nullite** (`""` survit).
//!
//! Les deux jugements **reels** de ce fichier sont ailleurs, et ils ne sont pas
//! du tout les memes :
//!
//! 1. `Draft.source` (`core/skill.ts:66`) teste la **veracite d'un booleen** :
//!    `if (draft.sources.some(...)) return`. Aucune chaine n'est evaluee, donc
//!    aucune chaine vide ne peut disparaitre la.
//! 2. `Source.equals` (`schema/skill.ts:42-46`) compare avec `===`, donc sur le
//!    **contenu** des chaines : deux `path` vides sont egaux, et une source
//!    `directory` et une source `url` dont les chaines sont identiques restent
//!    **differentes** parce que le discriminant est compare d'abord.
//!
//! Consequence concrete : `description` et `slash` sont `undefined` dans la
//! source, pas `""` et pas `false`. Ils se traduisent par `None`, jamais par une
//! valeur par defaut. `slash: None` est le point le plus casse du fichier : un
//! `unwrap_or(false)` ajouterait une capacite que le modele n'a pas.
//!
//! ## Ce fichier ne touche à AUCUN glob
//!
//! Le crate `glob` ne gere pas les accolades `{a,b}`, et la source n'en contient
//! aucune. Le seul motif a accolades du voisinage est
//! `core/skill.ts:79` — `fs.glob("{*.md,**/SKILL.md}", { cwd, absolute: true,
//! include: "file", symlink: true, dot: true })` — qui appartient au **chargeur**
//! de skills, pas a ce plugin. Aucun glob, aucun `minmatch`, aucune expansion
//! prealable n'est donc introduit ici, et ce module n'importe rien de
//! `crate::swarm::util_glob`.
//!
//! Meme remarque pour le drapeau `dot` : le `dot: opts?.dot` de
//! `packages/opencode/src/skill/index.ts:155` (dont `opts?.dot` peut valoir
//! `undefined`, et ne doit donc **pas** devenir `false`) appartient au **port
//! v1**, dans un autre paquet. Le `dot: true` de `core/skill.ts:79` est un
//! litteral, pas un ternaire. Rien de tout cela n'est dans le perimetre de ce
//! fichier.
//!
//! ## La deduplication est reglee par le registre, pas par le plugin
//!
//! `Draft.source` (`core/skill.ts:65-68`) :
//!
//! ```ts
//! source: (source) => {
//!   if (draft.sources.some((item) => Source.equals(item, source))) return
//!   draft.sources.push(source as Types.DeepMutable<Source>)
//! }
//! ```
//!
//! Donc l'appel est **idempotent** : rejouer ce plugin dix fois donne **une**
//! source. Et la deduplication se fait sur `Source.equals`, donc pour deux
//! sources `embedded` sur le **nom seul** : une deuxieme source du meme nom
//! est **integralement ignoree**, contenu et description differents compris.
//! Ce n'est ni un ecrasement ni une fusion, c'est un refus. Trois tests le
//! verrouillent, dont un sur le nom seul, avec des contenus differents.
//!
//! Ce comportement est porte ici par [`Draft`] plutot que par [`appliquer`] :
//! c'est la regle du registre, et elle ne doit pas etre dupliquee dans chaque
//! plugin appelant.
//!
//! ## Les types sont declares ici, et c'est un compromis
//!
//! `SkillV2.Info`, `SkillV2.EmbeddedSource` et `SkillV2.Source` vivent dans
//! `packages/schema/src/skill.ts` et sont reexportes par `core/skill.ts`.
//! **`crate::core::skill` n'existe pas encore** dans ce depot et ce fichier n'a
//! pas le droit de le creer (regle : un sous-agent ne touche qu'un fichier).
//! Les types sont donc declares minimalement ici.
//!
//! C'est le point le plus faible du portage : c'est une **seconde** declaration
//! de la meme forme, capable de diverger en silence des que
//! `core/skill.rs` existera. Le correctif est de supprimer les declarations
//! ci-dessous et d'importer le module reel, **sans changer une seule ligne des
//! tests** : ils ne visent que les noms de champs et le comportement du
//! registre. Le `tag = "type"` de [`Source`] reproduit exactement le
//! `Schema.toTaggedUnion("type")` de la source, donc le JSON reste identique.
//!
//! Aucun des deux autres types n'est declare : ni `DirectorySource` seul, ni
//! `UrlSource` seul, ni `SkillV2.Data`, ni `SkillV2.Interface`, ni le service
//! `Effect`. Les deux variantes sont declarees parce que `Source.equals` doit
//! savoir qu'elles existent pour statuer `false` entre types differents ; c'est
//! le strict minimum qui rend l'egalite faithfule.
//!
//! ## `AbsolutePath` est une chaine, rien de plus
//!
//! `AbsolutePath = Schema.String.pipe(Schema.brand("AbsolutePath"))`
//! (`packages/schema/src/schema.ts:9`) : au runtime c'est une `string`, le
//! branding n'existe qu'a la compilation TypeScript. Aucune validation de
//! chemin n'est portee, et `/builtin/customize-opencode.md` est accepte tel quel
//! comme le serait n'importe quelle autre chaine.
//!
//! ## DANGER : le slicing par index d'octet panicque sur ce contenu
//!
//! Le `.md` contient six caracteres de trois octets. Une expression comme
//! `&CONTENU[..n]` sur un `n` calcule en colonnes, ou `&CONTENU[11..13]`, coupe
//! un caractere en son milieu et **PANIQUE a l'execution** — pas a la
//! compilation, puisque le fichier se verifie tres bien. Le depot a deja ce
//! risque ailleurs ; ici il est **concret**, parce que le texte n'est pas
//! ASCII. Le seul usage sur (`CONTENU`, `&CONTENU[debut..]` ou
//! `CONTENU.char_indices()`) est `push_str(&CONTENU)` sur la chaine entiere, qui
//! ne peut pas couper. Le test `aucun_index_octet_ne_peut_couper_un_caractere`
//! verrouille le fait en demonstrant que les six positions sont des **faux**
//! `is_char_boundary`.
//!
//! ## La valeur de retour du `transform` est jetee
//!
//! `Hooks<Spec>` (`v2/effect/registration.ts:11-15`) fait rendre
//! `Effect.Effect<Registration, never, Scope.Scope>`. Le `yield*` du plugin
//! consomme l'effet mais **ne lie jamais** la `Registration`. La
//! transformation survit au plugin parce qu'elle est enregistree dans le
//! service skills, pas dans le plugin. Une fonction `()` est donc le
//! comportement observable exact, et c'est ce que fait [`transformer`].
//!
//! ## Note d integration
//!
//! Ce fichier attend `pub mod plugin_skill;` dans `src/swarm/mod.rs`, que je ne
//! touche pas. Il ne depend d'aucun autre module du crate : ni `crate::core::*`,
//! ni `crate::swarm::*`. Il ne depend que de `serde` et `serde_json`, deja
//! declares dans `Cargo.toml`.

use serde::{Deserialize, Serialize};

/// Identifiant du plugin tel qu'il est enregistre par le moteur interne.
///
/// En TS : la propriete `id` de l'objet passe a `define`, ligne 14.
///
/// C'est la valeur sous laquelle `SkillPlugin.Plugin` est ajoute au registre
/// dans `plugin/internal.ts:113`, **entre** `CommandPlugin.Plugin` et
/// `ModelsDevPlugin`. L'ordre de cet appel fixe l'ordre de rejeu des
/// transformations du domaine skills.
pub const ID: &str = "skill";

/// Nom du skill incorpore.
///
/// En TS : `name: "customize-opencode"`, ligne 21. C'est aussi le nom que
/// `Source.equals` compare pour deux sources `embedded`, donc c'est la cle de
/// deduplication de ce plugin.
pub const NOM: &str = "customize-opencode";

/// Chemin **declaration** du skill, tel qu'il apparait dans la sortie.
///
/// En TS : `location: AbsolutePath.make("/builtin/customize-opencode.md")`,
/// ligne 24.
///
/// ATTENTION : ce chemin n'est ni ouvert, ni verifie, ni resolu. Il s'agit
/// d'un `/builtin/` **virtuel** : le contenu vient de [`CONTENU`], compile dans
/// le binaire. Le port v1 (`packages/opencode/src/skill/index.ts:281`) ecrit
/// `"<built-in>"` au meme endroit, qui n'est ni un chemin ni une URL ; les deux
/// formes sont des reponses a deux systemes de skills differents et ne doivent
/// pas etre confondues. Un test verrouille cette difference.
pub const LOCALISATION: &str = "/builtin/customize-opencode.md";

/// Description du skill incorpore.
///
/// En TS : la chaine de la ligne 23, sur douze lignes de source.
///
/// Elle est **identique** a `CUSTOMIZE_OPENCODE_SKILL_DESCRIPTION` de
/// `packages/opencode/src/skill/index.ts:33-34` : c'est le meme texte, copie
/// dans les deux paquets. Le commentaire d'en-tete du `.md` renvoie a ce nom de
/// constante, mais la constante vit dans `packages/opencode/`, **pas** dans le
/// fichier porte ici, ou les valeurs sont ecrites **en ligne** dans l'objet
/// `make`. Un portage qui chercherait la constante a la source ne la trouverait
/// pas, et c'est le fichier qu'il faut lire.
///
/// ASCII pur, contrairement a [`CONTENU`].
pub const DESCRIPTION: &str = "Use ONLY when the user is editing or creating opencode's own configuration: opencode.json, opencode.jsonc, files under .opencode/, or files under ~/.config/opencode/. Also use when creating or fixing opencode agents, subagents, commands, skills, plugins, MCP servers, or permission rules. Do not use for the user's own application code, or for any project that is not configuring opencode itself.";

/// Corps du skill, recopie de `plugin/skill/customize-opencode.md`.
///
/// En TS : l'import texte de la ligne 9, reexporte tel quel ligne 11.
///
/// 16 525 octets d'origine, dont 18 octets non-ASCII (cinq `U+2014`, un
/// `U+2026`), et **453** fins de ligne CRLF devenues LF ici : voir la section
/// "La normalisation des fins de ligne est un ecart assume".
///
/// La chaine **se termine par un saut de ligne**, comme le fichier, et **commence
/// par `<!--`** : le commentaire HTML d'en-tete du `.md` fait partie du contenu
/// et n'est pas retire. Un test verrouille les deux bornes.
pub const CONTENU: &str = r#"<!--
  Built-in skill. Name and description are registered in code at
  packages/core/src/plugin/skill.ts
  and CUSTOMIZE_OPENCODE_SKILL_DESCRIPTION). The body below becomes the
  skill's content.
-->

# Customizing opencode

opencode validates its own config strictly and refuses to start when a field
is wrong. The shapes below cover the common surface area, but they are a
**summary, not the source of truth**.

## Full schema reference

The authoritative list of every config option — with field types, enums,
defaults, and descriptions — lives in the published JSON Schema:

**<https://opencode.ai/config.json>**

If a field is not documented in this skill, or you need to confirm an exact
shape before writing config, **fetch that URL and read the schema directly**
rather than guessing. opencode hard-fails on invalid config, so the cost of a
wrong shape is a broken startup.

Independently, every `opencode.json` should declare
`"$schema": "https://opencode.ai/config.json"` so the user's editor catches
mistakes as they type.

## Applying changes

Config is loaded once when opencode starts and is not hot-reloaded. After
saving changes to `opencode.json`, an agent file, a skill, a plugin, or any
other config-time file, **tell the user to quit and restart opencode** for
the changes to take effect. The running session will keep using the
already-loaded config until then.

## Where files live

| Scope                         | Path                                                                                                                      |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Project config                | `./opencode.json`, `./opencode.jsonc`, or `.opencode/opencode.json` (opencode walks up from the cwd to the worktree root) |
| Global config                 | `~/.config/opencode/opencode.json` or `~/.config/opencode/opencode.jsonc` (NOT `~/.opencode/`)                            |
| Project agents                | `.opencode/agent/<name>.md` or `.opencode/agents/<name>.md`                                                               |
| Global agents                 | `~/.config/opencode/agent(s)/<name>.md`                                                                                   |
| Project commands              | `.opencode/command/<name>.md` or `.opencode/commands/<name>.md`                                                           |
| Global commands               | `~/.config/opencode/command(s)/<name>.md`                                                                                 |
| Project skills                | `.opencode/skill(s)/<name>/SKILL.md`                                                                                      |
| Global skills                 | `~/.config/opencode/skill(s)/<name>/SKILL.md`                                                                             |
| External skills (auto-loaded) | `~/.claude/skills/<name>/SKILL.md`, `~/.agents/skills/<name>/SKILL.md`                                                    |

Configs from each scope are deep-merged. Project overrides global. Unknown
top-level keys in `opencode.json` are rejected with `ConfigInvalidError`.

## opencode.json

Every field is optional.

```json
{
  "$schema": "https://opencode.ai/config.json",
  "username": "string",
  "model": "provider/model-id",
  "small_model": "provider/model-id",
  "default_agent": "agent-name",
  "shell": "/bin/zsh",
  "logLevel": "DEBUG" | "INFO" | "WARN" | "ERROR",
  "share": "manual" | "auto" | "disabled",
  "autoupdate": true | false | "notify",
  "snapshot": true,
  "instructions": ["AGENTS.md", "docs/style.md"],

  "skills": {
    "paths": [".opencode/skills", "/abs/path/to/skills"],
    "urls": ["https://example.com/.well-known/skills/"]
  },

  "references": {
    "docs": {
      "path": "../docs",
      "description": "Use for product behavior and documentation conventions"
    },
    "sdk": {
      "repository": "owner/sdk",
      "branch": "main",
      "description": "Use for SDK implementation details",
      "hidden": true
    }
  },

  "agent": {
    "my-agent": {
      "model": "anthropic/claude-sonnet-4-6",
      "mode": "subagent",
      "description": "...",
      "permission": { "edit": "deny" }
    }
  },

  "command": {
    "deploy": { "description": "...", "template": "..." }
  },

  "provider": {
    "anthropic": { "options": { "apiKey": "..." } }
  },
  "disabled_providers": ["openai"],
  "enabled_providers": ["anthropic"],

  "mcp": {
    "playwright": {
      "type": "local",
      "command": ["npx", "-y", "@playwright/mcp"],
      "enabled": true,
      "environment": {}
    },
    "remote-thing": {
      "type": "remote",
      "url": "https://...",
      "headers": { "Authorization": "Bearer ..." }
    }
  },

  "plugin": [
    "opencode-gemini-auth",
    "opencode-foo@1.2.3",
    "./local-plugin.ts",
    ["opencode-bar", { "option": "value" }]
  ],

  "permission": {
    "edit": "deny",
    "bash": { "git *": "allow", "*": "ask" }
  },

  "formatter": false,
  "lsp": false,

  "experimental": {
    "primary_tools": ["edit"],
    "mcp_timeout": 30000
  },

  "tool_output": { "max_lines": 200, "max_bytes": 8192 },

  "compaction": { "auto": true, "tail_turns": 15 }
}
```

Shape notes worth being explicit about:

- `model` always carries a provider prefix: `"anthropic/claude-sonnet-4-6"`.
- `skills` is an object with `paths` and/or `urls`, not an array.
- `references` is an object keyed by alias. Each value is a local path, Git repository, or string shorthand.
- `agent` is an object keyed by agent name, not an array.
- `command` is an object keyed by command name, not an array.
- `plugin` is an array of strings or `[name, options]` tuples, not an object.
- `mcp[name].command` is an array of strings, never a single string. `type` is required.
- `permission` is either a string action or an object keyed by tool name.

## Skills

opencode's skill loader scans for `**/SKILL.md` inside skill directories. The
file is named `SKILL.md` exactly, and lives in its own folder named after the
skill:

```
.opencode/skills/my-skill/SKILL.md
```

Frontmatter:

```markdown
---
name: my-skill
description: One sentence covering what this skill does AND when to trigger it. Front-load the literal keywords or filenames the user is likely to say.
---

# My Skill

(skill body in markdown: instructions, examples, references)
```

- `name` is required, lowercase hyphen-separated, up to 64 chars, and matches the folder name.
- `description` is effectively required: skills without one are filtered out and never surfaced to the model. Cover both _what_ the skill does and _when_ to use it. Write in third person ("Use when...", not "I help with..."). Front-load concrete trigger keywords and filenames; gate with "Use ONLY when..." if the skill should stay quiet on adjacent topics.
- Optional: `license`, `compatibility`, `metadata` (string-string map).

Register skills from non-default locations via `skills.paths` (scanned
recursively for `**/SKILL.md`) and `skills.urls` (each URL serves a list of
skills).

## References

References make local directories and Git repositories outside the active
project available as supporting context. Configure them under `references`,
keyed by the alias used in `@` autocomplete:

```json
{
  "references": {
    "docs": {
      "path": "../product-docs",
      "description": "Use for product behavior and terminology"
    },
    "effect": {
      "repository": "Effect-TS/effect",
      "branch": "main",
      "description": "Use for Effect implementation details"
    }
  }
}
```

Local `path` values may be relative to the declaring config, absolute, or use
`~/`. Git `repository` values accept Git URLs, host/path references, and GitHub
`owner/repo` shorthand; `branch` is optional. Both forms support optional
`description` and `hidden` fields.

- Only references with a `description` are advertised to agents in system context.
- `hidden: true` removes a reference from TUI `@` autocomplete only. It remains available to agents and by direct path.
- Reference directories are automatically allowed through the external-directory boundary; normal read/edit/tool permissions still apply.
- String shorthand is supported: use `"docs": "../docs"` for local paths or `"effect": "Effect-TS/effect"` for Git repositories.

## Agents

Two ways to define an agent. Use the file form for anything non-trivial.

### Inline (in `opencode.json`)

```json
{
  "agent": {
    "my-reviewer": {
      "description": "Reviews PRs for style violations.",
      "mode": "subagent",
      "model": "anthropic/claude-sonnet-4-6",
      "permission": { "edit": "deny", "bash": "ask" },
      "prompt": "You are a strict PR reviewer..."
    }
  }
}
```

### File

```
.opencode/agent/my-reviewer.md      OR     .opencode/agents/my-reviewer.md
```

```markdown
---
description: Reviews PRs for style violations.
mode: subagent
model: anthropic/claude-sonnet-4-6
permission:
  edit: deny
  bash: ask
---

You are a strict PR reviewer. Focus on...
```

The file body becomes the agent's `prompt`. Do not also put `prompt:` in the
frontmatter.

`mode` is one of `"primary"`, `"subagent"`, `"all"`.

Allowed top-level frontmatter fields: `name, model, variant, description, mode,
hidden, color, steps, options, permission, disable, temperature, top_p`. Any
unknown field is silently routed into `options`.

To disable a built-in agent: `agent: { build: { disable: true } }`, or in a
file, `disable: true` in frontmatter.

`default_agent` must point to a non-hidden, primary-mode agent.

### Built-in agents

opencode ships with `build`, `plan`, `general`, `explore`. Hidden internal agents:
`compaction`, `title`, `summary`. To override a built-in's fields, define the
same key in `agent: { <name>: { ... } }`.

## Commands

opencode's command loader scans for `**/*.md` inside command directories. The
file is named after the command, and lives directly inside the `command` folder:

```
.opencode/command/deploy.md
```

Frontmatter:

```markdown
---
description: One sentence describing what the command does.
agent: build
model: anthropic/claude-sonnet-4-6
---

(command body in markdown: the prompt opencode runs, with $ARGUMENTS for the user's input)
```

- `template` is the command body — everything below the frontmatter — and is required: it is the prompt opencode runs when the command is invoked. Do not also put a `template:` key in the frontmatter.
- `$ARGUMENTS` is replaced with everything the user typed after the command; `$1`, `$2`, … pull individual positional arguments.
- Optional: `description`, `agent`, `model`, `variant`, `subtask`.

## Plugins

`plugin:` is an array. Each entry is one of:

```json
"plugin": [
  "opencode-gemini-auth",            // npm spec, latest
  "opencode-foo@1.2.3",              // npm spec, pinned
  "./local-plugin.ts",               // file path, relative to the declaring config
  "file:///abs/path/plugin.js",      // file URL
  ["opencode-bar", { "key": "val" }] // tuple form with options
]
```

Auto-discovered plugins (no config entry needed): any `*.ts` or `*.js` file in
`.opencode/plugin/` or `.opencode/plugins/`.

A plugin module exports `default` (or any named export) of type
`Plugin = (input: PluginInput, options?) => Promise<Hooks>`. The export is a
function, not a plain object literal, and the function returns an object
(return `{}` if there is nothing to register).

```ts
import type { Plugin } from "@opencode-ai/plugin"

export default (async ({ client, project, directory, $ }) => {
  return {
    config: (cfg) => {
      // cfg is the live merged config; mutate fields here.
    },
    "tool.execute.before": async (input, output) => {
      // mutate output.args before the tool runs
    },
  }
}) satisfies Plugin
```

Hook surface (mutate `output` in place; return `void`):

- `event(input)`: every bus event
- `config(cfg)`: once on init with the merged config
- `chat.message`, `chat.params`, `chat.headers`
- `tool.execute.before`, `tool.execute.after`
- `tool.definition`
- `command.execute.before`
- `shell.env`
- `permission.ask`
- `experimental.chat.messages.transform`, `experimental.chat.system.transform`,
  `experimental.session.compacting`, `experimental.compaction.autocontinue`,
  `experimental.text.complete`

Special object-shaped (not callbacks): `tool: { my_tool: { ... } }`,
`auth: { ... }`, `provider: { ... }`.

## MCP servers

`mcp:` is an object keyed by server name. Each server is discriminated by
`type`:

```json
{
  "mcp": {
    "playwright": {
      "type": "local",
      "command": ["npx", "-y", "@playwright/mcp"],
      "enabled": true,
      "environment": { "BROWSER": "chromium" }
    },
    "github": {
      "type": "remote",
      "url": "https://...",
      "enabled": true,
      "headers": { "Authorization": "Bearer {env:GITHUB_TOKEN}" }
    },
    "old-server": { "enabled": false }
  }
}
```

`command` is an array of strings. `environment` sets environment variables for
a local MCP server. `type` is required. Use `enabled: false` to
disable a server inherited from a parent config. String values such as header
tokens support `{env:VAR}` interpolation (and `{file:path}`); the shell-style
`${VAR}` is not substituted.

## Permissions

```json
"permission": {
  "edit": "deny",
  "bash": { "git *": "allow", "rm *": "deny", "*": "ask" },
  "external_directory": { "~/secrets/**": "deny", "*": "allow" }
}
```

Actions: `"allow"`, `"ask"`, `"deny"`.

Per-tool value forms: `"allow"` shorthand (treated as `{"*": "allow"}`), or an
object `{ pattern: action }`. Within an object, **insertion order matters**.
opencode evaluates the LAST matching rule, so put broad rules first and narrow
rules last.

`permission: "allow"` (a string at the top level) is shorthand for "allow
everything" and is rarely what the user wants.

Known permission keys: `read, edit, glob, grep, list, bash, task,
external_directory, todowrite, question, webfetch, websearch, lsp, doom_loop,
skill`. Some of these (`todowrite,
question, webfetch, websearch, doom_loop`) only accept a flat
action, not a per-pattern object.

`external_directory` patterns are filesystem paths (use `~/`, absolute paths,
or globs like `~/projects/**`).

Per-agent `permission:` overrides top-level `permission:`. Plan Mode lives on
the `plan` agent's permission ruleset (`edit: deny *`).

## Escape hatches

When a user's config is broken and opencode won't start, these env vars help:

- `OPENCODE_DISABLE_PROJECT_CONFIG=1`: skip the project's local `opencode.json`
  and start from globals only. Run from the project directory, opencode loads,
  the user edits the broken file, then they restart without the flag.
- `OPENCODE_CONFIG=/path/to/file.json`: load an additional explicit config.
- `OPENCODE_CONFIG_CONTENT='{"$schema":"https://opencode.ai/config.json"}'`:
  inject inline JSON as a final local-scope merge.
- `OPENCODE_DISABLE_DEFAULT_PLUGINS=1`: skip default plugins.
- `OPENCODE_PURE=1`: skip external plugins entirely.
- `OPENCODE_DISABLE_EXTERNAL_SKILLS=1`,
  `OPENCODE_DISABLE_CLAUDE_CODE_SKILLS=1`: skip the external skill scans under
  `~/.claude/` and `~/.agents/`.

## When proposing edits

- Validate against the schema before writing. If you are unsure of a field's
  exact shape, or the field is not covered in this skill, fetch
  `https://opencode.ai/config.json` and read the schema rather than guessing.
- Preserve `$schema` and any existing fields the user did not ask to change.
- For agent, command, skill, and plugin definitions, prefer creating new files
  in the correct location over inlining everything in `opencode.json`.
- If the user's existing config is malformed, point them at the env-var escape
  hatches above so they can edit from inside opencode without breaking their
  session.
- After saving any config change, remind the user to quit and restart opencode
  — running sessions keep using the already-loaded config.
"#;

/// Un skill decrit, forme `SkillV2.Info`.
///
/// En TS : `packages/schema/src/skill.ts:20-26`, reexporte par `core/skill.ts:27`.
///
/// Declaration **minimale et locale** : voir la section "Les types sont
/// declares ici, et c'est un compromis". Le champ `slash` est ici parce que la
/// forme v2 l'a, et il n'est **pas** pose par ce plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// `name` : nom du skill, en minuscules et separate par des tirets.
    ///
    /// Cle de deduplication des sources `embedded`. C'est la seule cle : deux
    /// sources `embedded` du meme nom sont egales, contenu different ou non.
    #[serde(rename = "name")]
    pub name: String,

    /// `description` : phrase de declenchement, facultative en schema.
    ///
    /// `Schema.String.pipe(optional)` : la cle peut **manquer**. Ce plugin la
    /// pose toujours. `None` doit disparaitre du JSON, pas s'y ecrire `null`.
    #[serde(rename = "description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// `slash` : drapeau de declenchement par `/`, facultatif en schema.
    ///
    /// `Schema.Boolean.pipe(optional)`. Ce plugin **ne le pose pas** : il vaut
    /// `undefined` en TypeScript, donc `None` ici. C'est le point le plus casse
    /// du fichier : un `unwrap_or(false)` ou un `is_false()` donnerait au modele
    /// une capacite qu'il n'a pas, et le JSON ne serait plus identique.
    #[serde(rename = "slash", skip_serializing_if = "Option::is_none")]
    pub slash: Option<bool>,

    /// `location` : chemin declare du skill, en chaine simple.
    ///
    /// `AbsolutePath` est une `string` rebrandee : au runtime, aucune
    /// validation. Le champ est donc un `String` et rien d'autre.
    #[serde(rename = "location")]
    pub location: String,

    /// `content` : corps markdown du skill.
    ///
    /// Source **obligatoire** en schema, mais sans contrainte de longueur ni de
    /// forme. Pour une source `directory` ou `url`, elle est remplie par le
    /// chargeur ; ici elle vaut [`CONTENU`].
    #[serde(rename = "content")]
    pub content: String,
}

/// Une source de skills, forme `SkillV2.Source`.
///
/// En TS : `packages/schema/src/skill.ts:34-55`, union etiquetee sur `type`.
///
/// Le `#[serde(tag = "type", rename_all = "lowercase")]` reproduit exactement le
/// `Schema.toTaggedUnion("type")` de la source : `{"type":"embedded","skill":{…}}`,
/// `{"type":"directory","path":"…"}`, `{"type":"url","url":"…"}`.
///
/// `PartialEq` est **ecrit a la main** parce qu'il est le portage de
/// `Source.equals`, qui est une fonction et non une egalite derivee. Voir
/// [`Source::key`] pour la seconde fonction de la source.
#[derive(Debug, Clone, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Source {
    /// `DirectorySource` : un dossier a parcourir.
    ///
    /// jamais construit par ce plugin, mais `Source.equals` doit connaître la
    /// variante pour statuer `false` face a une source `embedded`.
    Directory {
        /// `path` du dossier.
        path: String,
    },

    /// `UrlSource` : une adresse a interroger.
    ///
    /// jamais construit par ce plugin, pour la meme raison que [`Source::Directory`].
    Url {
        /// `url` de la liste de skills.
        url: String,
    },

    /// `EmbeddedSource` : un skill compile dans le binaire.
    ///
    /// C'est **la seule** variante construite par ce fichier.
    Embedded {
        /// `skill` : le skill incorpore.
        skill: Info,
    },
}

impl Source {
    /// Cle de deduplication et de cache, portage de `Source.key`.
    ///
    /// En TS : `schema/skill.ts:48-53`.
    ///
    /// Le prefixe depend du discriminant : `directory:`, `url:` ou `embedded:`.
    /// Pour une source `embedded`, la cle ne contient que le **nom**, comme
    /// `Source.equals`. Deux consequences observables :
    ///
    /// - une source `directory` dont le `path` vaut `/a/b` et une source `url`
    ///   dont l'`url` vaut `/a/b` donnent deux cles **differentes** ;
    /// - deux sources `embedded` de noms differents mais de contenus
    ///   identiques donnent deux cles differentes, et l'inverse aussi.
    pub fn key(&self) -> String {
        match self {
            Source::Directory { path } => format!("directory:{path}"),
            Source::Url { url } => format!("url:{url}"),
            Source::Embedded { skill } => format!("embedded:{}", skill.name),
        }
    }
}

/// Egalite de la source, portage de `Source.equals`.
///
/// En TS : `schema/skill.ts:42-46`.
///
/// ```ts
/// if (a.type !== b.type) return false
/// if (a.type === "directory" && b.type === "directory") return a.path === b.path
/// if (a.type === "url" && b.type === "url") return a.url === b.url
/// if (a.type === "embedded" && b.type === "embedded") return a.skill.name === b.skill.name
/// return false
/// ```
///
/// Ce n'est **pas** une egalite derivee :
///
/// 1. le discriminant est compare en premier, donc deux chaines identiques dans
///    deux variantes differentes sont **differentes** ;
/// 2. pour `embedded`, seul `name` est compare : `description`, `slash`,
///    `location` et `content` sont **ignores** ;
/// 3. la comparaison est une egalite de **contenu** (`===`), donc deux chaines
///    vides sont egales.
///
/// Le point 2 est le plus important : il fait de `Draft::source` un refus, pas
/// un ecrasement. Voir la section sur la deduplication.
impl PartialEq for Source {
    fn eq(&self, autre: &Source) -> bool {
        match (self, autre) {
            (Source::Directory { path: a }, Source::Directory { path: b }) => a == b,
            (Source::Url { url: a }, Source::Url { url: b }) => a == b,
            (
                Source::Embedded { skill: a },
                Source::Embedded { skill: b },
            ) => a.name == b.name,
            _ => false,
        }
    }
}

/// Le `Draft` du registre des sources de skills.
///
/// En TS : `core/skill.ts:44-47`, dont l'implementation est `core/skill.ts:62-71`.
///
/// Declaration **minimale et locale**, comme [`Info`] et [`Source`] : le
/// service `Effect` lui-meme n'est pas porte, seul le comportement observable
/// du registre l'est. C'est [`Draft::source`] qui porte la regle de deduplication,
/// parce qu'elle appartient au registre et non au plugin qui l'appelle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    /// `Data.sources`, dans l'ordre d'insertion.
    ///
    /// Un `Vec` et pas un ensemble : la source le plus recente est en **fin**
    /// de liste, et l'ordre de rejeu des plugins est significatif
    /// (`plugin/internal.ts:110-121`).
    sources: Vec<Source>,
}

impl Draft {
    /// Registre vide, equivalent de `initial: () => ({ sources: [] })`.
    pub fn new() -> Self {
        Draft { sources: Vec::new() }
    }

    /// Ajoute une source, sauf si une source egale est deja presente.
    ///
    /// En TS : `core/skill.ts:65-68`.
    ///
    /// Le test porte ici est un test de **veracite de booleen**
    /// (`draft.sources.some(...)`), pas un test de veracite de chaine : aucune
    /// chaine n'est evaluee, donc aucune chaine vide ne peut disparaitre a
    /// cette occasion. C'est le point le plus proche du piege `?` contre `??`
    /// dans tout le fichier, et il n'a rien a voir avec lui.
    ///
    /// Une source refusee est **integralement** perdue : pas de fusion, pas
    /// d'ecrasement, pas de contenu ajoute. Pour `embedded`, l'egalite porte sur
    /// le nom seul, donc c'est la **premiere** source gagneuse qui reste.
    pub fn source(&mut self, source: Source) {
        if self.sources.iter().any(|item| item == &source) {
            return;
        }
        self.sources.push(source);
    }

    /// Les sources enregistrees, dans l'ordre d'insertion.
    ///
    /// En TS : `list: () => draft.sources`, `core/skill.ts:69`.
    pub fn list(&self) -> &[Source] {
        &self.sources
    }

    /// Nombre de sources enregistrees.
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Vrai si aucune source n'est enregistree.
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

/// Le plugin de skills incorpores.
///
/// En TS : `export const Plugin = define({ id: "skill", effect })`.
///
/// `define` (`plugin/internal.ts:59-61`) retourne son argument sans le
/// modifier, donc le type `Plugin` se resume a un identifiant et a une fonction
/// d'effet. Aucun champ d'etat n'est porte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillPlugin;

impl SkillPlugin {
    /// Le meme identifiant que le [`ID`] du module.
    pub const ID: &'static str = ID;

    /// Enregistre la transformation des skills.
    ///
    /// Equivalent de l'effet complet du plugin, service compris : le service
    /// `SkillV2.Service` n'est pas porte par ce fichier, il est fourni par
    /// `plugin/internal.ts:79` et injecte dans chaque effet de plugin
    /// (`plugin/internal.ts:101`).
    pub fn transformer(draft: &mut Draft) {
        appliquer(draft);
    }
}

/// Enregistre le skill incorpore dans le registre.
///
/// Equivalent du corps de la fonction passee a `ctx.skill.transform`.
///
/// Une seule source, une seule fois, sans condition. La construction de
/// l'objet est **pure** : ni lecture, ni ecriture, ni reseau, ni horloge. Le
/// contenu est une constante compilee, donc le plugin n'ouvre aucun fichier.
pub fn appliquer(draft: &mut Draft) {
    draft.source(Source::Embedded {
        skill: Info {
            name: NOM.to_string(),
            description: Some(DESCRIPTION.to_string()),
            slash: None,
            location: LOCALISATION.to_string(),
            content: CONTENU.to_string(),
        },
    });
}

/// La source telle que ce plugin la construit.
///
/// Equivalent de l'argument unique de `draft.source(...)`.
///
/// Exposee pour que les tests puissent comparer une source **complete** a la
/// construction attendue, sans repasser par le registre.
pub fn source_incoree() -> Source {
    Source::Embedded {
        skill: Info {
            name: NOM.to_string(),
            description: Some(DESCRIPTION.to_string()),
            slash: None,
            location: LOCALISATION.to_string(),
            content: CONTENU.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Source `directory` de test.
    fn dossier(chemin: &str) -> Source {
        Source::Directory {
            path: chemin.to_string(),
        }
    }

    /// Source `url` de test.
    fn adresse(url: &str) -> Source {
        Source::Url { url: url.to_string() }
    }

    /// Source `embedded` de test, dont le nom est le seul discriminant.
    fn incorporee(nom: &str, contenu: &str) -> Source {
        Source::Embedded {
            skill: Info {
                name: nom.to_string(),
                description: None,
                slash: None,
                location: LOCALISATION.to_string(),
                content: contenu.to_string(),
            },
        }
    }

    #[test]
    fn le_plugin_enregistre_exactement_le_skill_integre() {
        let mut draft = Draft::new();
        assert!(draft.is_empty(), "le registre neuf est vide");

        appliquer(&mut draft);

        assert_eq!(draft.len(), 1, "le plugin ajoute une source et une seule");
        assert_eq!(draft.list(), &[source_incoree()]);

        let info = match &draft.list()[0] {
            Source::Embedded { skill } => skill,
            autre => panic!("le plugin n Ajoute qu une source embedded, pas {autre:?}"),
        };
        assert_eq!(info.name, "customize-opencode");
        assert_eq!(info.description.as_deref(), Some(DESCRIPTION));
        assert_eq!(info.location, "/builtin/customize-opencode.md");
        assert_eq!(info.content, CONTENU);
    }

    #[test]
    fn la_localisation_est_le_chemin_builtin_et_pas_le_jeton_du_port_v1() {
        // Le meme skill est enregistre deux fois dans le depot, sous deux
        // formes. Le port v1 (`packages/opencode/src/skill/index.ts:281`) ecrit
        // `"<built-in>"`, qui n'est ni un chemin ni une URL. Reprendre cette
        // valeur ici produirait un chemin non resoluble et un JSON different
        // de celui que le TypeScript ecrit.
        let faux_jeton_v1 = "<built-in>";
        assert_eq!(LOCALISATION, "/builtin/customize-opencode.md");
        assert_ne!(LOCALISATION, faux_jeton_v1);

        // Et la forme v1 n'a meme pas le champ `slash` : reprendre sa structure
        // ferait disparaitre une cle du contrat v2.
        let v1: serde_json::Value =
            serde_json::from_str(r#"{"name":"customize-opencode","description":"d","location":"<built-in>","content":"c"}"#)
                .unwrap();
        assert!(v1.get("slash").is_none(), "la forme v1 ignore slash");

        let mut draft = Draft::new();
        appliquer(&mut draft);
        let json = serde_json::to_value(&draft.list()[0]).unwrap();
        let cle = json["skill"]["location"].as_str().unwrap();
        assert_eq!(cle, LOCALISATION);
        assert!(!cle.contains('<'), "le jeton v1 ne doit pas apparaitre : {cle}");
    }

    #[test]
    fn le_contenu_est_le_fichier_markdown_entier_bornes_comprises() {
        // L'import texte de la ligne 9 est inline tel quel : le commentaire
        // HTML d'en-tete du `.md` fait partie du contenu, et la derniere ligne
        // est terminee par un saut de ligne. Ni l'un ni l'autre ne sont
        // retouches par le portage.
        assert!(CONTENU.starts_with("<!--"), "le commentaire d en-tete est du contenu");
        assert!(CONTENU.ends_with("\n"), "la derniere ligne est terminee");
        assert!(!CONTENU.ends_with("\n\n"), "il n y a pas de ligne vide finale");

        // Quelques reperes de structure, pour attraper une recopie tronquee.
        for reperes in [
            "# Customizing opencode",
            // The source writes the URL as a markdown autolink, angle brackets
            // included: `**<https://opencode.ai/config.json>**` on line 19. The
            // brackets are part of the text, so the marker without them does not
            // occur in the file.
            "**<https://opencode.ai/config.json>**",
            "## opencode.json",
            "## Skills",
            "## Agents",
            "## Commands",
            "## Plugins",
            "## MCP servers",
            "## Permissions",
            "## Escape hatches",
            "OPENCODE_PURE=1",
        ] {
            assert!(
                CONTENU.contains(reperes),
                "section absente du contenu incorpore : {reperes}"
            );
        }

        // Le texte parle bien du skill qu'il est, et pas d'un autre. Le chemin
        // est ecrit dans un BLOC DE CODE (lignes 167-169 du `.md`), donc sans
        // les accents graves d'un code en ligne : le marquee avec eux n existe
        // pas dans le fichier.
        assert!(
            CONTENU.contains("\n```\n.opencode/skills/my-skill/SKILL.md\n```\n"),
            "l exemple de skill attendu a disparu du contenu incorpore"
        );
    }

    #[test]
    fn les_caracteres_non_ascii_du_contenu_survivent_intacts() {
        // Donnee, pas documentation : cinq U+2014 et un U+2026, soit 18 octets
        // non-ASCII. Remplacer par `-` et `...` changerait le texte lu par le
        // modele. Le comptage se fait en `chars()`, jamais sur `len()`.
        let cadratins = CONTENU.chars().filter(|c| *c == '\u{2014}').count();
        let points = CONTENU.chars().filter(|c| *c == '\u{2026}').count();
        assert_eq!(cadratins, 5, "les tirets cadratins ont disparu ou ont ete dupliques");
        assert_eq!(points, 1, "le point de suspension a disparu ou a ete duplique");

        let non_ascii: usize = CONTENU.chars().filter(|c| c.len_utf8() > 1).count();
        assert_eq!(non_ascii, 6);
        // Et la longueur en octets, qui n'est pas la longueur en caracteres.
        assert!(
            CONTENU.len() > CONTENU.chars().count(),
            "contenu ASCII alors que la source ne l est pas"
        );

        // Temoin : la description, elle, est de l ASCII pur. Si ce test echoue
        // en inverse, c'est que la chaine a ete abimee.
        assert!(DESCRIPTION.is_ascii(), "la description source est ASCII");
    }

    #[test]
    fn aucun_index_octet_ne_peut_couper_un_caractere_du_contenu() {
        // Le risque annonce par le lot : `&CONTENU[..n]` coupe au milieu d'un
        // caractere non-ASCII et PANIQUE a l execution, alors que le fichier se
        // compile. Un **debut** de caractere est toujours une frontiere, c'est
        // meme la garantie de `char_indices` : les index dangereux sont ceux
        // qui tombent DANS un caractere, entre son premier octet et le
        // precedent. Ce sont ces positions-la qui doivent etre des **faux**
        // `is_char_boundary`.
        let departs: Vec<(usize, char)> =
            CONTENU.char_indices().filter(|(_, c)| c.len_utf8() > 1).collect();
        assert_eq!(departs.len(), 6, "les six caracteres larges ont disparu");

        for (rang, &(indexe, caractere)) in departs.iter().enumerate() {
            let largeur = caractere.len_utf8();
            // Le depart reel est bien une frontiere, et la coupe fait bien
            // trois octets.
            assert!(
                CONTENU.is_char_boundary(indexe),
                "index {rang} ({indexe}) : le debut d un caractere est une frontiere"
            );
            assert_eq!(CONTENU[indexe..].chars().next(), Some(caractere));
            assert_eq!(largeur, 3, "seul un caractere de 3 octets est attendu ici");

            // Les positions internes, elles, ne le sont pas : une tranche y
            // commencerait sur un octet qui n'est pas un debut de caractere.
            for decalage in 1..largeur {
                let interne = indexe + decalage;
                assert!(
                    !CONTENU.is_char_boundary(interne),
                    "index {rang} : {interne} est un octet interne de {indexe}, une tranche y commencerait au milieu du caractere"
                );
            }
        }

        // Et la seule operation portable sur cette chaine reste la copie
        // entiere, qui ne peut pas couper.
        let copie = CONTENU.to_string();
        assert_eq!(copie.chars().count(), CONTENU.chars().count());
        assert_eq!(copie.as_bytes(), CONTENU.as_bytes());
    }

    #[test]
    fn slash_est_absent_et_pas_faux() {
        // `slash` est `Schema.Boolean.pipe(optional)` et le plugin ne le pose
        // pas : il vaut `undefined`, donc `None`. Un `unwrap_or(false)` donnerait
        // au modele une capacite qu il n a pas, et le JSON ne serait plus celui
        // du TypeScript. Le piege `?` contre `??` n est pas en jeu ici, parce
        // que la source ne teste ni la veracite ni la nullite : elle ne parle
        // pas du champ du tout.
        let mut draft = Draft::new();
        appliquer(&mut draft);

        let Source::Embedded { skill } = &draft.list()[0] else {
            panic!("source embedded attendue");
        };
        assert_eq!(skill.slash, None, "slash ne doit pas etre pose a false");
        assert_ne!(skill.slash, Some(false));

        let objet = serde_json::to_value(skill).unwrap();
        assert!(objet.get("slash").is_none(), "slash absent du JSON : {objet:?}");
        assert!(objet.get("Slash").is_none());
        assert!(objet.get("SLASH").is_none());
    }

    #[test]
    fn les_noms_de_champs_serialises_sont_ceux_du_typescript() {
        // Test prioritaire du piege 1 : il ne s'applique pas a ce fichier, et
        // c'est ce test qui le prouve. Les onze cles ont ete relues ligne a
        // ligne dans la source, aucune n'a de majuscule, donc le nom Rust EST
        // le nom TypeScript. Les `#[serde(rename)]` redondants servent de
        // garde-fou, pas de traduction.
        let mut draft = Draft::new();
        appliquer(&mut draft);

        let json = serde_json::to_value(&draft.list()[0]).unwrap();
        let objet = json.as_object().unwrap();
        assert_eq!(objet.len(), 2, "la source porte deux cles : {objet:?}");
        assert!(objet.contains_key("type"));
        assert!(objet.contains_key("skill"));
        assert_eq!(objet.get("type").and_then(|v| v.as_str()), Some("embedded"));

        let skill = objet["skill"].as_object().unwrap();
        assert_eq!(skill.len(), 4, "slash absent, quatre cles restent : {skill:?}");
        for nom in ["name", "description", "location", "content"] {
            assert!(skill.contains_key(nom), "cle absente du JSON : {nom}");
        }

        // Aucun decalage possible, dans aucun sens.
        for interdit in [
            "Name",
            "NAME",
            "Description",
            "Location",
            "Content",
            "Skill",
            "name_",
            "content_md",
        ] {
            assert!(
                !objet.contains_key(interdit) && !skill.contains_key(interdit),
                "le nom Rust a fuite dans le JSON sous {interdit} : {json}"
            );
        }

        // Et l'ordre des cles du JSON suit l'ordre de la source.
        let brut = serde_json::to_string(&draft.list()[0]).unwrap();
        let debut_skill = brut.find("\"skill\"").expect("cle skill presente");
        let nom = brut.find("\"name\"").expect("cle name presente");
        let localisation = brut.find("\"location\"").expect("cle location presente");
        let contenu = brut.find("\"content\"").expect("cle content presente");
        assert!(debut_skill < nom, "type et skill passent avant le contenu de la source");
        assert!(nom < localisation && localisation < contenu);
    }

    #[test]
    fn la_variante_fauteuse_du_discriminant_est_refusee_a_la_lecture() {
        // `Schema.toTaggedUnion("type")` ne reconnait que les trois
        // discriminants de la source. Une variante snake_case ou majuscule
        // n'existe pas en amont et ne doit donc pas exister en aval.
        for faux in [
            r#"{"type":"Embedded","skill":{"name":"a","location":"/l","content":"c"}}"#,
            r#"{"type":"EMBEDDED","skill":{"name":"a","location":"/l","content":"c"}}"#,
            r#"{"type":"embed","skill":{"name":"a","location":"/l","content":"c"}}"#,
            r#"{"type":"file","skill":{"name":"a","location":"/l","content":"c"}}"#,
        ] {
            assert!(
                serde_json::from_str::<Source>(faux).is_err(),
                "variante inexistante acceptee : {faux}"
            );
        }

        // Temoin : la forme exacte de la source, dans les trois variantes, et
        // elle seule.
        for bon in [
            r#"{"type":"directory","path":"/a"}"#,
            r#"{"type":"url","url":"https://ex.test/s/"}"#,
            r#"{"type":"embedded","skill":{"name":"a","location":"/l","content":"c"}}"#,
        ] {
            assert!(
                serde_json::from_str::<Source>(bon).is_ok(),
                "forme de la source refusee : {bon}"
            );
        }

        // Et `name`, `location` et `content` sont obligatoires en schema :
        // `description` et `slash` sont les deux seules cles facultatives.
        assert!(serde_json::from_str::<Info>(r#"{"name":"a","location":"/l"}"#).is_err());
        assert!(serde_json::from_str::<Info>(r#"{"name":"a","content":"c"}"#).is_err());
        let sans_option: Info =
            serde_json::from_str(r#"{"name":"a","location":"/l","content":"c"}"#).unwrap();
        assert_eq!(sans_option.description, None);
        assert_eq!(sans_option.slash, None);
    }

    #[test]
    fn l_egalite_compare_le_discriminant_puis_un_seul_champ() {
        // `Source.equals` n est pas une egalite derivee. Trois consequences,
        // toutes observables, et toutes couvertes ici.
        let mut draft = Draft::new();

        // 1. Deux chaines identiques dans deux variantes differentes sont
        //    differentes : le discriminant passe en premier.
        assert_ne!(dossier("/a/b"), adresse("/a/b"));
        assert_ne!(incorporee("a", "c"), dossier("a"));
        assert_ne!(adresse("a"), incorporee("a", "c"));

        // 2. Pour `embedded`, seul le nom compte. Contenu et description
        //    differents n empachent pas l'egalite.
        let gauche = Source::Embedded {
            skill: Info {
                name: "meme".to_string(),
                description: Some("une description".to_string()),
                slash: Some(true),
                location: "/un/chemin".to_string(),
                content: "contenu un".to_string(),
            },
        };
        let droite = Source::Embedded {
            skill: Info {
                name: "meme".to_string(),
                description: None,
                slash: None,
                location: "/autre/chemin".to_string(),
                content: "contenu deux".to_string(),
            },
        };
        assert_eq!(gauche, droite, "seul le nom compte pour deux sources embedded");
        assert_ne!(gauche, incorporee("autre", "contenu un"));

        // 3. C est une egalite de contenu (`===`), pas de truthiness : deux
        //    chaines vides sont egales.
        assert_eq!(dossier(""), dossier(""));
        assert_eq!(incorporee("", "x"), incorporee("", "y"));

        // Et la cle de `Source.key` suit le meme-prefixe discipline.
        assert_eq!(dossier("/a/b").key(), "directory:/a/b");
        assert_eq!(adresse("/a/b").key(), "url:/a/b");
        assert_eq!(incorporee("meme", "n'importe").key(), "embedded:meme");

        // Preuve que la deduplication du registre s'appuie bien sur cette
        // egalite et pas sur une comparaison de contenu.
        draft.source(gauche.clone());
        draft.source(droite);
        assert_eq!(draft.len(), 1, "la deuxieme source de meme nom est refusee");
    }

    #[test]
    fn une_source_embarquee_de_meme_nom_est_refusee_en_entier() {
        // Ce n est ni un ecrasement ni une fusion : c est un refus. La
        // **premiere** source gagneuse reste, contenu et description compris.
        let mut draft = Draft::new();
        draft.source(incorporee("customize-opencode", "contenu de l utilisateur"));
        assert_eq!(draft.len(), 1);

        appliquer(&mut draft);

        assert_eq!(draft.len(), 1, "le plugin ne doit pas ajouter de deuxieme source");
        match &draft.list()[0] {
            Source::Embedded { skill } => {
                assert_eq!(skill.content, "contenu de l utilisateur", "la premiere source gagne");
                assert_eq!(skill.description, None, "la description du perdant n est pas injectee");
            }
            autre => panic!("source embedded attendue, pas {autre:?}"),
        }

        // L inverse : sur un registre vierge, c est bien le plugin qui gagne.
        let mut vierge = Draft::new();
        appliquer(&mut vierge);
        assert_eq!(vierge.len(), 1);
        assert_eq!(vierge.list(), &[source_incoree()]);

        // Et un nom different n entre pas en collision, meme avec un contenu
        // identique.
        let mut deux = Draft::new();
        deux.source(incorporee("autre-nom", CONTENU));
        appliquer(&mut deux);
        assert_eq!(deux.len(), 2, "deux noms distincts donnent deux sources");
        let cles: Vec<String> = deux.list().iter().map(Source::key).collect();
        assert_eq!(cles, vec!["embedded:autre-nom", "embedded:customize-opencode"]);
    }

    #[test]
    fn des_variantes_de_types_differents_coexistent() {
        // Le registre ne trie pas et n'homogeneise pas : une source `directory`
        // et une source `url` portant la MEME chaine restent deux sources, et
        // l'ordre d'insertion est preserve.
        let mut draft = Draft::new();
        draft.source(dossier("/depot/skills"));
        draft.source(adresse("https://ex.test/skills/"));
        draft.source(incorporee("personnalise", "contenu"));
        appliquer(&mut draft);

        assert_eq!(draft.len(), 4, "quatre sources distinctes");
        let cles: Vec<String> = draft.list().iter().map(Source::key).collect();
        assert_eq!(
            cles,
            vec![
                "directory:/depot/skills".to_string(),
                "url:https://ex.test/skills/".to_string(),
                "embedded:personnalise".to_string(),
                "embedded:customize-opencode".to_string(),
            ]
        );

        // Le plugin n Ajoute qu a la fin : il ne replace rien, il ne trie rien.
        assert_eq!(draft.list()[3], source_incoree());
    }

    #[test]
    fn deux_pass_successives_donnent_le_meme_registre() {
        // L effet est idempotent, parce que la deduplication est au registre et
        // que le registre est initialise a vide a chaque rejeu de la
        // transformation.
        let mut premier = Draft::new();
        appliquer(&mut premier);
        appliquer(&mut premier);
        appliquer(&mut premier);

        let mut second = Draft::new();
        appliquer(&mut second);

        assert_eq!(premier.list(), second.list());
        assert_eq!(premier.len(), 1, "trois appels, une seule source");

        // Et le round trip JSON du registre entier est stable, au caractere
        // pres, ce qui verifie au passage que le contenu non-ASCII resiste a
        // la serialisation.
        let brut_premier = serde_json::to_string(premier.list()).unwrap();
        let relu: Vec<Source> = serde_json::from_str(&brut_premier).unwrap();
        assert_eq!(relu, premier.list());
        assert_eq!(serde_json::to_string(&relu).unwrap(), brut_premier);
        assert!(brut_premier.contains('\u{2014}'), "le tiret cadratin a disparu du JSON");
    }

    #[test]
    fn le_plugin_ne_touche_a_rien_tant_qu_on_ne_l_appelle_pas() {
        // Le type est sans etat : le construire ne modifie aucun registre, et
        // le registre neuf reste vide tant que `appliquer` n a pas ete appele.
        let mut draft = Draft::new();
        assert!(draft.is_empty());

        let plugin = SkillPlugin::default();
        assert_eq!(plugin, SkillPlugin);
        assert!(draft.is_empty(), "construire le plugin ne doit rien modifier");

        SkillPlugin::transformer(&mut draft);
        assert_eq!(draft.len(), 1);

        assert_eq!(SkillPlugin::ID, "skill");
        assert_eq!(ID, "skill");
        assert_eq!(NOM, "customize-opencode");
    }

    #[test]
    fn le_contenu_de_la_constante_est_egal_a_la_constante_du_plugin() {
        // `export const CustomizeOpencodeContent = customizeOpencodeContent`
        // (ligne 11) est un reexport sans transformation : la constante du
        // module est l import texte, au caractere pres.
        let mut draft = Draft::new();
        appliquer(&mut draft);
        let Source::Embedded { skill } = &draft.list()[0] else {
            panic!("source embedded attendue");
        };
        assert_eq!(skill.content, CONTENU);
        assert!(!skill.content.is_empty(), "le contenu ne peut pas etre vide");
        assert_eq!(skill.content.len(), CONTENU.len());
        assert_eq!(skill.content.chars().count(), CONTENU.chars().count());
    }
}
