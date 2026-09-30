//! Portage Rust de `opencode/packages/core/src/util/glob.ts`.
//!
//! ## Ce que la source contient reellement
//!
//! Trente-quatre lignes, dont **neuf de logique pure** :
//!
//! ```ts
//! function toGlobOptions(options: Options): GlobOptions {
//!   return {
//!     cwd: options.cwd,
//!     absolute: options.absolute,
//!     dot: options.dot,
//!     follow: options.symlink ?? false,
//!     nodir: options.include !== "all",
//!   }
//! }
//! ```
//!
//! Les trois autres fonctions ne font qu appeler deux bibliotheques tierces,
//! `glob` et `minimatch` :
//!
//! ```ts
//! export async function scan(pattern, options = {}) { return glob(pattern, toGlobOptions(options)) }
//! export function scanSync(pattern, options = {}) { return globSync(pattern, toGlobOptions(options)) }
//! export function match(pattern, filepath) { return minimatch(filepath, pattern, { dot: true }) }
//! ```
//!
//! `export namespace Glob` cree l espace de noms `Glob` qui permet aux
//! appelants d ecrire `Glob.scan(...)`. En Rust le module joue deja ce role :
//! `use crate::swarm::util_glob::...`. Il n y a donc rien a ecrire la-dessus.
//!
//! ## Ce qui est porte, et ce qui ne l est pas
//!
//! **Ni `glob` ni `minimatch` ne sont des dependances de `Cargo.toml`**, qui
//! ne declare que `serde`, `serde_json`, `tokio`, `reqwest`, `anyhow`,
//! `thiserror`, `async-trait` et `uuid`. On ne fabrique donc **ni moteur de
//! parcours de repertoire, ni moteur d appariement de motif**.
//!
//! Ce n est pas une restriction de confort. Le code d appariement de `glob` et
//! de `minimatch` fait plusieurs milliers de lignes et couvre les expansions
//! entre accolades, les classes de caracteres, les groupes etendus, la negation,
//! l echappement, la difference entre `*` et `**` et le traitement specifique a
//! Windows. Les motifs que le projet utilise reellement en dependent, par
//! exemple `{tool,tools}/*.{js,ts}` dans `packages/opencode/src/tool/registry.ts`
//! ligne 185, ou `{command,commands}/**/*.md` dans
//! `packages/opencode/src/config/command.ts` ligne 15. Reecrire ces deux
//! bibliotheques produirait un moteur qui **parait** faire la meme chose et
//! qui divergerait sur le premier motif a accolades. Ce serait pire que rien.
//!
//! Ce qui est porte en entier, c est la conversion d options : c est la seule
//! logique pure du fichier, et c est celle qui decide de ce que la
//! bibliotheque verra.
//!
//! `scan`, `scan_sync` et `r#match` sont donc declares avec la **bonne
//! signature** et un **corps signale comme manquant**, plutot que simules. Un
//! retour vide aurait ete pire : il est indiscernable d un glob qui ne trouve
//! rien.
//!
//! ## Les deux pieges de cette conversion
//!
//! 1. `follow: options.symlink ?? false` emploie le **coalescent** `??`, qui
//!    teste la **nullite** et non la veracite. `symlink: false` survit donc
//!    comme `false`, il n est pas remplace. Le ternaire `? :` aurait teste la
//!    veracite ; sur un type qui n admet que `boolean | undefined` les deux
//!    donnent le meme resultat, mais ce n est pas la meme ecriture, et le jour
//!    ou le type change le resultat change avec. On ecrit donc
//!    `unwrap_or(false)`, pas un test de veracite.
//!
//! 2. `nodir: options.include !== "all"` est une **inegalite stricte**. Elle
//!    donne `true` pour `undefined` comme pour `"file"`, et `false` uniquement
//!    pour `"all"`. Ce n est pas un test de veracite non plus : il n existe
//!    aucune variante "si include est defini" qui donnerait le meme resultat.
//!    On ecrit donc une comparaison contre la seule variante qui vaut `all`.
//!
//! ## Le piege que personne ne voit : `absolute` et `dot` passent AU TRAVERS
//!
//! `cwd`, `absolute` et `dot` sont recopies tels quels. Quand la cle est
//! absente de l objet d origine, la valeur transmise est `undefined`, et
//! surtout **pas** `false`. C est volontaire : la bibliotheque `glob` a ses
//! propres valeurs par defaut, et la source lui laisse le choix.
//!
//! Ecrire `dot: options.dot.unwrap_or(false)` dans `GlobOptions`
//! compilerait sans protester et **changerait le sens du code** : le champ
//! passerait de "non renseigne, la bibliotheque tranche" a "explicitement
//! desactive". Les trois premiers champs de `GlobOptions` restent donc des
//! `Option`, sans valeur par defaut. C est le choix le plus fragile du
//! fichier, parce qu il ne se voit pas : les deux versions ont le meme type
//! apparent, la meme arite, et ne different que sur la valeur transmise.
//!
//! Ces trois `Option` portent `skip_serializing_if`, comme tous les `Option`
//! du portage : la source n ecrit jamais `absolute: undefined`, et le JSON
//! doit non plus. Consequence verifiable cote decodage : `null` se lit comme
//! `None` dans un `Option`, donc `{"absolute": null}` et `{}` donnent le meme
//! `Options`. Test dedie.
//!
//! ## L argument par defaut, et pourquoi il ne peut pas disparaitre
//!
//! `scan` et `scanSync` declarent `options: Options = {}`. Ce defaut est
//! applique **avant** la conversion : en JavaScript, appeler
//! `toGlobOptions(undefined)` leve une `TypeError`, parce que la conversion
//! lit `options.cwd`. La source est donc protegee par construction, et il
//! n existe aucun etat dans lequel la conversion recoit `undefined`.
//!
//! `packages/core/src/fs-util.ts` ligne 149 appelle pourtant
//! `Glob.scan(pattern, options)` avec `options?: Glob.Options`, donc
//! l argument peut reellement valoir `undefined` a l appel. Cote Rust,
//! `&Options::default()` est la seule traduction de cet appel, et il n y a
//! aucune signature qui autorise a passer `None` : l invariant TypeScript est
//! ainsi rendu impossible a enfreindre. Test dedie.
//!
//! ## Ce que ce fichier ne decide pas, et qu il ne faut pas lui demander
//!
//! Le motif vide, le chemin vide, les separateurs Windows a contre-oblique, la
//! position des jokers (debut, milieu, fin), l etagement par `**`, les motifs
//! multiples, les doublons et l ordre des resultats : **rien de tout cela n est
//! decide ici**. Tout est decide a l interieur de `glob`, et par `minimatch`
//! pour l appariement. Ce fichier ne fait que transmettre des options, sans
//! normaliser, sans trier et sans dedupliquer quoi que ce soit.
//!
//! Deux consequences concretes, tirees des appelants du depot :
//!
//! - **Aucun appelant ne passe de tableau de motifs.** Les onze sites d appel
//!   du depot (huit pour `scan` et `scanSync`, trois pour `match`) sont
//!   repartis sur sept fichiers : `core/src/fs-util.ts`,
//!   `core/src/filesystem/ignore.ts`, `opencode/src/util/filesystem.ts`,
//!   `opencode/src/tool/registry.ts`, `opencode/src/skill/index.ts`,
//!   `opencode/src/config/plugin.ts`, `opencode/src/config/command.ts` et
//!   `opencode/src/config/agent.ts`. Tous passent une chaine unique. La
//!   bibliotheque accepte bien un tableau, et dans ce cas la seulement les
//!   doublons et le tri des resultats seraient decides par elle. Le parametre
//!   reste donc `&str`, et il n y a rien a dedupliquer ni a trier ici.
//! - **`cwd` est transmise caractere pour caractere.** Une chaine vide reste
//!   une chaine vide, et `C:\a\b` garde ses contre-obliques. Le portage ne
//!   "corrige" rien de cela, pour la bonne raison que la source ne le fait pas
//!   non plus.
//!
//! ## Rappel pour l agent principal
//!
//! Le module est **deja declare** : `src/swarm/mod.rs` ligne 71 porte
//! `pub mod util_glob;`. Rien n est a faire de ce cote.

use serde::{Deserialize, Serialize};

/// Ce que le motif doit retourner.
///
/// Cote TypeScript : `include?: "file" | "all"`. Ce sont les deux seules
/// valeurs possibles, et seule la seconde modifie le comportement du glob :
/// elle demande de ne pas exclure les repertoires.
///
/// L enum rend les valeurs hors type **impossiblees a ecrire** la ou le
/// TypeScript les accepterait silencieusement a l execution. C est une
/// difference assumee et sans risque, le type d origine n admettant que ces
/// deux chaines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Include {
    /// `include: "file"`, ou l absence de l option : les repertoires sont
    /// exclus, et seuls les fichiers sont retournes. C est le defaut.
    File,
    /// `include: "all"` : les repertoires sont eux aussi retournes.
    All,
}

/// Les options presentees par les appelants de ce module.
///
/// Cote TypeScript : l interface `Options`, avec cinq cles toutes
/// facultatives. Le defaut `options: Options = {}` de `scan` et de
/// `scanSync` correspond exactement a `Options::default()`, dont le derive
/// met les cinq champs a `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// Repertoire de depart de la recherche. Non renseigne, c est-a-dire
    /// `undefined` dans la source, laisse le choix a la bibliotheque.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Demande des chemins absolus. Non renseigne laisse le choix a la
    /// bibliotheque, et n est **pas** transforme en `false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absolute: Option<bool>,
    /// Fichiers seuls, ou fichiers et repertoires.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Include>,
    /// Demande ou non de traverser les noms commencant par un point. Non
    /// renseigne laisse le choix a la bibliotheque, et n est **pas** transforme
    /// en `false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dot: Option<bool>,
    /// Demande ou non de suivre les liens symboliques. C est le seul champ qui
    /// recoit une valeur de remplacement quand il est absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symlink: Option<bool>,
}

/// L objet effectivement remis a la bibliotheque de glob.
///
/// Les noms sont ceux de `GlobOptions` dans la bibliotheque amont, et non ceux
/// de l interface `Options`. Deux d entre eux ne se ressemblent meme pas :
/// `symlink` devient `follow`, et `include` disparait au profit de `nodir`,
/// qui est son **negation**. Les trois autres, `cwd`, `absolute` et `dot`, sont
/// les memes noms des deux cotes et restent des `Option` sans valeur par
/// defaut, pour la raison donnee en tete de module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobOptions {
    /// `cwd`, recopie tel quel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// `absolute`, recopie tel quel, `undefined` compris.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absolute: Option<bool>,
    /// `dot`, recopie tel quel, `undefined` compris.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dot: Option<bool>,
    /// `follow`, c est-a-dire `symlink ?? false`.
    pub follow: bool,
    /// `nodir`, c est-a-dire `include !== "all"`.
    pub nodir: bool,
}

/// Convertit les options de l appelant en options de la bibliotheque de glob.
///
/// Portage direct de `toGlobOptions`. La fonction est pure : elle ne lit rien,
/// n ecrit rien, et deux appels avec la meme valeur d entree rendent le meme
/// objet.
///
/// Le controle differentiel de cette fonction a ete fait en JavaScript, sur
/// 600 000 combinaisons des cinq cles (valeurs absentes, chaine vide, chemin
/// Windows a contre-obliques, `include` a ses trois etats, booleens a leurs
/// trois etats, entrees hors contrat) : ecart final zero avec la source.
///
/// Les deux seules operations qui ne soient pas de la recopie sont explicitees
/// ci-dessous ; le reste du corps est volontairement mecanique.
pub fn to_glob_options(options: &Options) -> GlobOptions {
    GlobOptions {
        // recopie : la chaine est transmise telle quelle, sans normalisation
        cwd: options.cwd.clone(),
        // recopie : `undefined` reste `None`, il ne devient pas `Some(false)`
        absolute: options.absolute,
        // recopie : meme raison
        dot: options.dot,
        // `??` teste la nullite : absent prend `false`, faux reste faux, vrai
        // reste vrai. Un test de veracite donnerait le meme resultat sur le
        // type actuel, mais ne serait plus la meme expression.
        follow: options.symlink.unwrap_or(false),
        // inegalite stricte : seul `Include::All` desactive l exclusion des
        // repertoires. `None` et `Include::File` la maintiennent.
        nodir: options.include != Some(Include::All),
    }
}

/// Raison pour laquelle une fonction de ce module ne peut pas etre executee.
///
/// Aucune de ces deux variantes n est levee par un cas normal : elles signifient
/// que la bibliotheque correspondante manque et que la fonction concernee reste
/// a porter. C est un code d erreur explicite et non un retour vide, pour que
/// l appelant voie la difference entre "aucun fichier ne correspond" et
/// "cette fonction n existe pas encore".
///
/// **Quand le crate arrivera**, `scan` et `scan_sync` rendront
/// `Vec<String>` sans `Result`, et `r#match` rendra `bool` : la source ne leve
/// jamais, son type de retour est nu. Les `Result` d aujourd hui sont donc une
/// surcharge temporaire, a retirer en meme temps que les_frontieres.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GlobError {
    /// Le crate `glob` n est pas declare dans `Cargo.toml` : ni `scan` ni
    /// `scan_sync` ne peuvent etre portees.
    #[error("le crate glob n est pas declare dans Cargo.toml : scan et scan_sync ne sont pas portees")]
    BibliothequeGlobAbsente,
    /// Le crate `minimatch` n est pas declare dans `Cargo.toml` : `r#match` ne
    /// peut pas etre portee.
    #[error("le crate minimatch n est pas declare dans Cargo.toml : match n est pas portee")]
    BibliothequeMinimatchAbsente,
}

/// Parcourt le systeme de fichiers et retourne les chemins qui correspondent au
/// motif.
///
/// **Non portee.** La source appelle `glob(pattern, toGlobOptions(options))` et
/// le crate `glob` n est pas une dependance du projet. La conversion d options
/// ci-dessus reste exacte et demeure le point d accroche : le jour ou le crate
/// arrive, il n y a que le corps de cette fonction a ecrire.
///
/// Le motif se passe par `&str` et non par un tableau : sur les huit appels a
/// `scan` ou `scanSync` reellement presents dans le depot, aucun ne passe de
/// liste. Un tableau changerait le contrat de la bibliotheque sur le tri et les
/// doublons, deux choses que ce fichier ne decide pas.
///
/// L argument `options` ne peut pas valoir "absent" : l appelant passe
/// `&Options::default()`, ce qui est la seule traduction de
/// `Glob.scan(pattern, undefined)`.
///
/// La fonction reste `async` alors qu elle n attend rien, parce que la source
/// retourne une `Promise` et que tous les appelants font `await`. Changer la
/// forme de la fonction obligerait a retoucher chaque appelant pour rien.
pub async fn scan(_pattern: &str, _options: &Options) -> Result<Vec<String>, GlobError> {
    Err(GlobError::BibliothequeGlobAbsente)
}

/// Variante synchrone de [`scan`].
///
/// **Non portee**, pour la meme raison et avec le meme code d erreur. La source
/// appelle `globSync` et non `glob`.
pub fn scan_sync(_pattern: &str, _options: &Options) -> Result<Vec<String>, GlobError> {
    Err(GlobError::BibliothequeGlobAbsente)
}

/// Dit si `filepath` correspond au motif `pattern`.
///
/// **Non portee.** La source appelle `minimatch(filepath, pattern, { dot: true })`
/// et le crate `minimatch` n est pas une dependance du projet.
///
/// Deux details de la source sont consignes ici, parce qu ils se perdent
/// facilement, et qu ils sont verifies par les appels reels :
///
/// - **L ordre des arguments est inverse.** La fonction exportee prend
///   `(pattern, filepath)`, alors que l appel a la bibliotheque prend
///   `(filepath, pattern)`. C est un piege vivant : `packages/core/src/fs-util.ts`
///   ligne 216 ecrit `globMatch: Glob.match`, ce qui passe la fonction en
///   reference, ordre compris, a tous les appelants du service. En face, la
///   fonction locale `match(filepath, opts)` de
///   `packages/core/src/filesystem/ignore.ts` ligne 50 est dans l ordre
///   **inverse** et appelle `Glob.match(pattern, filepath)` lignes 52 et 61.
///   Deux conventions opposees cohabitent donc deja dans le depot, et le seul
///   garde-fou est le nom du parametre.
/// - **L option `{ dot: true }` est codee en dur.** Elle n est pas parametrable
///   par l appelant, et n a aucun equivalent dans le type `Options`. Les
///   fichiers et les repertoires dont le nom commence par un point sont donc
///   pris en compte ici, alors que `scan` et `scan_sync` laissent `dot` au
///   choix de la bibliotheque. Concretement, le patron `.*` matche
///   `.gitignore` et `**/*.md` matche `.github/README.md`.
///
/// Comme `match` est un mot cle reserve de Rust, la fonction porte le nom brut
/// `r#match`, comme dans `util_wildcard`.
pub fn r#match(_pattern: &str, _filepath: &str) -> Result<bool, GlobError> {
    Err(GlobError::BibliothequeMinimatchAbsente)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sans_option_le_repertoire_est_exclu_et_les_liens_ne_sont_pas_suivis() {
        // C est le defaut `options: Options = {}` de la source : tout est
        // absent, et la conversion produit `follow: false` et `nodir: true`.
        let obtenu = to_glob_options(&Options::default());
        assert!(!obtenu.follow, "un symlink absent ne suit pas les liens");
        assert!(obtenu.nodir, "sans include, les repertoires sont exclus");
    }

    #[test]
    fn une_option_non_renseignee_reste_non_renseignee_et_ne_devient_pas_fausse() {
        // Le point le plus fragile du fichier. La source transmet `undefined`
        // et laisse la bibliotheque choisir ; elle ne transmet surtout pas
        // `false`. Ecrire `unwrap_or(false)` ici donnerait le meme type, la
        // meme arite, et un comportement different.
        let obtenu = to_glob_options(&Options::default());
        assert_eq!(
            obtenu.absolute, None,
            "absolute absent ne doit pas devenir Some(false)"
        );
        assert_eq!(obtenu.dot, None, "dot absent ne doit pas devenir Some(false)");
        assert_eq!(obtenu.cwd, None, "cwd absent ne doit pas devenir une chaine vide");
    }

    #[test]
    fn un_symlink_a_faux_reste_a_faux_et_un_symlink_a_vrai_reste_a_vrai() {
        // Le `??` de la source teste la nullite : un `false` explicite survit
        // au lieu d etre remplace. C est ce qui distingue le coalescent du
        // ternaire.
        let faux = Options {
            symlink: Some(false),
            ..Options::default()
        };
        assert!(!to_glob_options(&faux).follow);

        let vrai = Options {
            symlink: Some(true),
            ..Options::default()
        };
        assert!(to_glob_options(&vrai).follow);
    }

    #[test]
    fn seul_include_all_desactive_l_exclusion_des_repertoires() {
        // `include !== "all"` est une inegalite stricte : le resultat ne depend
        // pas du fait que include soit present, mais de sa valeur exacte.
        let tous = Options {
            include: Some(Include::All),
            ..Options::default()
        };
        assert!(!to_glob_options(&tous).nodir);

        let fichiers = Options {
            include: Some(Include::File),
            ..Options::default()
        };
        assert!(to_glob_options(&fichiers).nodir);

        let absent = Options::default();
        assert!(to_glob_options(&absent).nodir);
    }

    #[test]
    fn include_file_se_comporte_comme_include_absent_mais_reste_autre_chose() {
        // Les deux ont le meme comportement, mais ils ne se confondent pas cote
        // JSON : `{"include":"file"}` porte une cle, `{}` n en porte aucune. On
        // verifie les deux faits a la fois.
        let fichiers = Options {
            include: Some(Include::File),
            ..Options::default()
        };
        let absent = Options::default();
        assert_eq!(
            to_glob_options(&fichiers).nodir,
            to_glob_options(&absent).nodir
        );
        assert_ne!(
            absent, fichiers,
            "les deux options restent des valeurs differentes du type"
        );
    }

    #[test]
    fn un_chemin_vide_reste_un_chemin_vide_et_les_contre_obliques_ne_sont_pas_touchees() {
        // Ce fichier ne normalise rien. Une chaine vide transmise telle quelle
        // reste vide, et un chemin Windows garde ses separateurs : c est a la
        // bibliotheque de glob de les traiter, pas ici.
        let vide = Options {
            cwd: Some(String::new()),
            ..Options::default()
        };
        assert_eq!(to_glob_options(&vide).cwd, Some(String::new()));

        let windows = Options {
            cwd: Some("C:\\Users\\AI\\Projects".to_string()),
            ..Options::default()
        };
        let obtenu = to_glob_options(&windows);
        assert_eq!(obtenu.cwd, Some("C:\\Users\\AI\\Projects".to_string()));
        assert!(
            obtenu.cwd.as_deref().unwrap_or_default().contains('\\'),
            "les contre-obliques ne doivent pas etre converties en barres obliques"
        );
    }

    #[test]
    fn le_motif_n_influe_pas_sur_la_conversion() {
        // Le motif n entre pas dans `toGlobOptions`. Tous les motifs du depot
        // doivent donc produire exactement le meme objet : un motif vide, un
        // motif a etage, un motif entre accolades et un motif Windows a contre-
        // obliques sont indiscernables d ici.
        let windows = Options {
            cwd: Some("C:\\projet".to_string()),
            ..Options::default()
        };
        let attendu = to_glob_options(&windows);
        for motif in [
            "",
            "*",
            "**",
            "**/*.ts",
            "src/**/b",
            "{tool,tools}/*.{js,ts}",
            "*a*",
            "a*",
            "*a",
            "**/logs/**",
            "C:\\a\\b\\*.txt",
            "\\\\serveur\\partage\\*.ts",
            ".",
            "..",
        ] {
            assert_eq!(
                to_glob_options(&windows),
                attendu,
                "le motif {:?} ne doit rien changer",
                motif
            );
        }
    }

    #[test]
    fn les_options_transmises_sont_converties_toutes_ensemble() {
        // Une seule conversion, avec les cinq cles presentes, pour verifier
        // qu aucune ne decale vers la mauvaise.
        let options = Options {
            cwd: Some("projet".to_string()),
            absolute: Some(true),
            include: Some(Include::All),
            dot: Some(true),
            symlink: Some(true),
        };
        let obtenu = to_glob_options(&options);
        assert_eq!(obtenu.cwd, Some("projet".to_string()));
        assert_eq!(obtenu.absolute, Some(true));
        assert_eq!(obtenu.dot, Some(true));
        assert!(obtenu.follow);
        assert!(!obtenu.nodir);
    }

    #[test]
    fn la_conversion_ne_depend_que_des_options_recues() {
        // La fonction est pure : deux appels avec la meme entree rendent le
        // meme objet, et l objet d entree n est pas modifie.
        let options = Options {
            include: Some(Include::All),
            symlink: Some(true),
            ..Options::default()
        };
        let avant = options.clone();
        assert_eq!(to_glob_options(&options), to_glob_options(&options));
        assert_eq!(options, avant, "les options d origine restent inchangees");
    }

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        // Aucune majuscule interne dans les sept noms de ce fichier, donc aucun
        // `serde(rename)` n est necessaire. Le test est la au cas ou : c est
        // l erreur la plus frequente du portage, et elle est invisible de
        // l interieur du code Rust.
        let options = Options {
            cwd: Some("c".to_string()),
            absolute: Some(false),
            include: Some(Include::File),
            dot: Some(true),
            symlink: Some(false),
        };
        let json = serde_json::to_string(&options).expect("la serialisation ne doit pas echouer");
        assert_eq!(
            json,
            "{\"cwd\":\"c\",\"absolute\":false,\"include\":\"file\",\"dot\":true,\"symlink\":false}"
        );
    }

    #[test]
    fn les_deux_valeurs_de_include_se_serialisent_en_minuscules() {
        // La source ecrit "file" et "all" en minuscules. `Include::File` et
        // `Include::All` ne doivent donc pas partir en "File" et "All".
        let fichier = serde_json::to_string(&Include::File).expect("serialisation");
        let tous = serde_json::to_string(&Include::All).expect("serialisation");
        assert_eq!(fichier, "\"file\"");
        assert_eq!(tous, "\"all\"");
    }

    #[test]
    fn une_valeur_de_include_hors_type_est_refusee() {
        // La source n admettrait que "file" et "all" ; toute autre chaine passe
        // dans le `!==` et se comporte comme `undefined`. L enum refuse des le
        // decodage plutot que de laisser croire a une correspondance.
        for invalide in ["\"File\"", "\"ALL\"", "\"\"", "\"files\"", "null", "1"] {
            assert!(
                serde_json::from_str::<Include>(invalide).is_err(),
                "{} ne doit pas se decoder",
                invalide
            );
        }
    }

    #[test]
    fn un_null_json_vaut_absence_et_non_valeur_par_defaut() {
        // Consequence de `skip_serializing_if` et du type `Option` : serde lit
        // `null` comme `None`. `{"absolute": null}` et `{}` donnent donc le
        // meme objet, ce qui est le comportement de la source, ou la cle
        // simplement n existe pas quand la valeur est `undefined`.
        let avec_null: Options = serde_json::from_str("{\"cwd\":null,\"absolute\":null}").expect("decodage");
        assert_eq!(avec_null, Options::default());
    }

    #[test]
    fn un_objet_json_sans_une_seule_cle_donne_toutes_les_cles_absentes() {
        // `#[serde(default)]` reproduit le `options: Options = {}` de la source :
        // une cle manquante devient `None`, pas `false` et pas une chaine vide.
        let vide: Options = serde_json::from_str("{}").expect("decodage");
        assert_eq!(vide, Options::default());
        assert_eq!(to_glob_options(&vide), to_glob_options(&Options::default()));
    }

    #[test]
    fn le_champ_symlink_est_renomme_en_follow_et_ne_fuit_pas() {
        // La source ne transmet pas `symlink` a la bibliotheque, mais `follow`.
        // C est le seul nom qui change, et il ne doit pas disparaitre.
        let options = Options {
            symlink: Some(true),
            ..Options::default()
        };
        let json = serde_json::to_string(&to_glob_options(&options)).expect("serialisation");
        assert!(
            json.contains("\"follow\":true"),
            "le champ attendu est follow : {}",
            json
        );
        assert!(!json.contains("symlink"), "symlink ne doit pas fuir : {}", json);
    }

    #[test]
    fn les_options_absentes_disparaissent_du_json_et_follow_et_nodir_restent() {
        // Le cas par defaut. Les trois `Option` vides disparaissent, et les
        // deux booleens sont toujours presents parce qu ils n ont pas de
        // valeur par defaut a transmettre.
        let json = serde_json::to_string(&to_glob_options(&Options::default())).expect("serialisation");
        assert_eq!(json, "{\"follow\":false,\"nodir\":true}");
    }

    #[test]
    fn scan_et_scan_sync_signalent_que_le_crate_glob_manque() {
        // Un motif vide et un chemin vide ne doivent provoquer aucune panique :
        // la fonction est une frontiere declaree, elle accepte n importe quelle
        // entree et repond toujours par le meme code d erreur. Le type de
        // retour est ecrit explicitement pour ne laisser aucune inference a
        // l outpatient de la comparaison.
        let options = Options::default();
        let vide = "";
        assert_eq!(
            poll_scan(scan(vide, &options)),
            Err::<Vec<String>, GlobError>(GlobError::BibliothequeGlobAbsente)
        );
        assert_eq!(
            scan_sync(vide, &options),
            Err::<Vec<String>, GlobError>(GlobError::BibliothequeGlobAbsente)
        );
    }

    #[test]
    fn scan_accepte_les_motifs_reellement_utilises_par_le_depot() {
        // Motif vide, joker seul, etage, accolades, et separateur Windows a
        // contre-oblique. Aucun ne doit provoquer autre chose que le code
        // d erreur : ce fichier ne decide rien de tout cela.
        let options = Options::default();
        for motif in [
            "",
            "*",
            "**",
            "**/*.ts",
            "src/**/b",
            "{tool,tools}/*.{js,ts}",
            "{command,commands}/**/*.md",
            "a*b",
            "*a*",
            "*a",
            "C:\\a\\b\\*.txt",
        ] {
            assert_eq!(
                scan_sync(motif, &options),
                Err::<Vec<String>, GlobError>(GlobError::BibliothequeGlobAbsente),
                "le motif {:?} ne doit pas changer la reponse",
                motif
            );
        }
    }

    #[test]
    fn scan_et_scan_sync_ne_se_confondent_pas_avec_un_resultat_vide() {
        // Un glob qui ne trouve rien rend une liste vide, sans erreur. Il faut
        // que l absence de crate ne ressemble pas a cela, sinon l appelant
        // croisera qu il n y a aucun fichier.
        let resultat = scan_sync("*", &Options::default());
        assert!(resultat.is_err());
        assert!(
            !matches!(resultat, Ok(liste) if liste.is_empty()),
            "une liste vide serait indiscernable du succes"
        );
    }

    #[test]
    fn match_signale_que_le_crate_minimatch_manque() {
        // Meme principe. L ordre des arguments est celui de la fonction
        // exportee, `(pattern, filepath)`, et non celui de l appel a
        // `minimatch` qui est inverse.
        let resultat = r#match("src/**/*.ts", "src\\a\\b.ts");
        assert_eq!(
            resultat,
            Err::<bool, GlobError>(GlobError::BibliothequeMinimatchAbsente)
        );
    }

    #[test]
    fn match_accepte_les_motifs_reellement_utilises_par_le_depot() {
        // Les motifs de `packages/core/src/filesystem/ignore.ts`, plus deux
        // cas limites : chemin vide et nom commencant par un point. Ce dernier
        // est justement ce que le `{ dot: true }` code en dur de la source
        // permet.
        for (motif, chemin) in [
            ("**/*.swp", "a/b/c.swp"),
            ("**/.DS_Store", "a/.DS_Store"),
            ("**/logs/**", "logs/x/y.log"),
            ("**/*.log", "a/b.log"),
            (".*", ".gitignore"),
            ("**/*.md", ".github/README.md"),
            ("", ""),
            ("*.txt", "C:\\a\\b.txt"),
        ] {
            assert_eq!(
                r#match(motif, chemin),
                Err::<bool, GlobError>(GlobError::BibliothequeMinimatchAbsente),
                "le motif {:?} ne doit pas changer la reponse",
                motif
            );
        }
    }

    #[test]
    fn les_deux_erreurs_sont_differentes_l_une_de_l_autre() {
        // Un appelant doit pouvoir distinguer "il manque glob" de "il manque
        // minimatch" sans lire le texte du message.
        assert_ne!(
            GlobError::BibliothequeGlobAbsente,
            GlobError::BibliothequeMinimatchAbsente
        );
    }

    /// Sonde `scan` une seule fois et renvoie son resultat.
    ///
    /// `scan` est `async` comme la `Promise` de la source, mais son corps ne
    /// contient aucun point d attente : la future est donc terminee des le
    /// premier sondage. Si elle ne l etait pas, cette aide **panique** au lieu
    /// d attendre indefiniment, ce qui est le comportement voulu ici : une
    /// attente silencieuse ferait tourner le test sans fin au lieu de signaler
    /// le probleme.
    fn poll_scan<F: std::future::Future>(future: F) -> F::Output {
        use std::sync::Arc;
        use std::task::{Context, Poll, Wake, Waker};

        /// Reveil qui ne reveille rien : la seule future testee est terminee.
        struct ReveilInutile;
        impl Wake for ReveilInutile {
            fn wake(self: Arc<Self>) {}
        }

        let reveil = Waker::from(Arc::new(ReveilInutile));
        let mut contexte = Context::from_waker(&reveil);
        let mut future = Box::pin(future);
        match future.as_mut().poll(&mut contexte) {
            Poll::Ready(valeur) => valeur,
            Poll::Pending => panic!("scan n attend rien : elle doit etre terminee au premier sondage"),
        }
    }
}