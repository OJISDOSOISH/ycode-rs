# swarm-util_glob

===DEBUT===
fichier : src/swarm/util_glob.rs
source  : packages/core/src/util/glob.ts
taille  : 32801 octets (737 lignes)
tests   : 22

CONFIANCE : haute sur la conversion d options, **basse sur le contrat de signature**

POINT FAIBLE : `scan`, `scan_sync` et `r#match` rendent `Result<_, GlobError>`
alors que la source rend `Vec<string>` et `boolean`, sans jamais lever. C est
la seule surcharge que j ai introduite, et elle est provisoire par nature.
Concretement, `Glob.match` est passe **en reference** dans
`packages/core/src/fs-util.ts:216` (`globMatch: Glob.match`), donc tous les
appelants du service `FileSystem` heriteront de ce `Result`. Si le lot veut une
signature definitive plutot qu une frontiere, il faut trancher ici : soit on
ajoute `glob` au `Cargo.toml` et on implemente, soit on assume le `Result` et
on l accepte dans les 11 sites d appel. Je n ai pas pu trancher : le choix
appartient a l agent principal.

A VERIFIER dans cet ordre :
1. **`le_motif_n_influe_pas_sur_la_conversion`** et les deux tests `*_accepte_les_motifs_*`
   : ils affirment que le motif n entre pas dans la conversion. C est vrai dans
   la source, mais c est la place ou une erreur de sens passerait inapercue, car
   ces tests passent a la fois sur une implementation correcte et sur une qui
   ignore l option `dot` ou `include`.
2. **`une_option_non_renseignee_reste_non_renseignee_et_ne_devient_pas_fausse`** :
   le point le plus fragile. `absolute` et `dot` doivent rester `None`, jamais
   `Some(false)`. Ecrire `unwrap_or(false)` compilerait et changerait le sens.
3. **`un_null_json_vaut_absence_et_non_valeur_par_defaut`** : le seul endroit ou
   mon portage est **deliberement** plus laxiste que la source. Serde lit
   `null` comme `None`, la source refuse `null`. C est la convention du lot,
   mais c est une divergence, pas une equivalence.
4. `poll_scan` : sonde unique, **panic nomme** si la future rend `Pending`.
   Meme convention que le `wait_timeout` applique aux 6 waits, et c est
   volontaire : une boucle d attente silencieuse ferait pendre la CI au lieu de
   nommer le probleme. Si `scan` devient reellement `async` plus tard, ce test
   panic et il faut le remplacer.

CE QUI EST PORTE, CE QUI NE L EST PAS :
- `to_glob_options` : portage exact, **verifie par differentiel**. J ai
  translittere la fonction en JavaScript et l ai comparee a l implementation
  d origine sur **600 000 combinaisons** des cinq cles (cles absentes, chaine
  vide, chemin Windows a contre-obliques, `include` a ses trois etats,
  booleens a leurs trois etats, entrees hors contrat, balayage aleatoire
  deterministe). Ecart final **zero**. Aucun Rust n a ete compile ni execute.
- `Options`, `Include`, `GlobOptions` : portage exact.
- `scan`, `scan_sync`, `r#match` : **non portees**. `glob` et `minimatch` ne
  sont pas dans `Cargo.toml`. Conformement a la consigne de mission je n ai
  pas fabrique de moteur de motif : les motifs reellement utilises par le
  depot passent par l expansion entre accolades (`{tool,tools}/*.{js,ts}` dans
  `tool/registry.ts:185`), donc une implementation maison divergerait en
  silence. Les trois fonctions rendent une erreur explicite plutot qu une
  liste vide, pour ne pas etre confondues avec un glob sans resultat.

FINDING UTILE AU LOT : `glob` et `minimatch` manquent au `Cargo.toml`. Neuf
fichiers de la source en dependent (`core/src/fs-util.ts`,
`core/src/filesystem/ignore.ts`, `opencode/src/util/filesystem.ts`,
`tool/registry.ts`, `skill/index.ts`, `config/plugin.ts`, `config/command.ts`,
`config/agent.ts`). Leurs appels a `Glob.scan` restent donc non portes. Ajouter
`glob = "0.3"` est une ligne, mais ce n est pas mon fichier.

DEUX FAITS VERIFIES DANS LA SOURCE, absents du fichier d origine :
- Aucun appelant ne passe de **tableau** de motifs (8 appels a `scan`/
  `scanSync`, tous une chaine unique). Les doublons et le tri des resultats
  n ont donc aucune source reelle dans ce lot.
- `Glob.scan(pattern, options)` est appele avec `options?: Glob.Options` en
  `fs-util.ts:149`, donc l argument peut valoir `undefined`. Le defaut `{}` est
  applique **avant** la conversion : `toGlobOptions(undefined)` leverait une
  `TypeError` en JS. En Rust, `&Options::default()` est la seule traduction, et
  aucune signature ne permet de passer `None` : l invariant est rendu
  infranchissable.

PIEGE DES NOMS DE CHAMPS : **sans objet sur ce fichier**, et c est verifie.
Les sept noms sont `cwd`, `absolute`, `include`, `dot`, `symlink`, `follow`,
`nodir` : aucune majuscule interne, donc aucun `serde(rename)` n est requis.
Un test verifie quand meme la chaine JSON exacte, et un autre verifie que
`symlink` ne fuit jamais et que `follow` apparait bien.

PIEGE `?` CONTRE `??` : les deux occurrences sont traitees explicitement.
`follow: options.symlink ?? false` est un coalescent (teste la nullite), donc
`symlink: false` survit : `unwrap_or(false)`, pas un test de veracite.
`nodir: options.include !== "all"` est une inegalite stricte, pas un test de
presence : `None` et `Include::File` la maintiennent tous deux.

MODULES : `pub mod util_glob;` est **deja** dans `src/swarm/mod.rs` ligne 71.
Je n ai touche ni `mod.rs`, ni `lib.rs`, ni `Cargo.toml`. Aucun commit, aucun
push. 0 octet non-ASCII dans le fichier.
===FIN===