//! Portage de `packages/core/src/filesystem/ignore.ts`.
//!
//! ## Ce que contient reallyment la source
//!
//! Soixante-sept lignes, dont **quarante-huit sont des donnees** : vingt-huit
//! noms de repertoires et onze motifs de fichiers. Le reste tient en une
//! fonction de quinze lignes.
//!
//! ```ts
//! export function match(filepath: string, opts?: { extra?: string[]; whitelist?: string[] }) {
//!   for (const pattern of opts?.whitelist || []) if (Glob.match(pattern, filepath)) return false
//!   const parts = filepath.split(/[/\\]/)
//!   for (const part of parts) if (FOLDERS.has(part)) return true
//!   for (const pattern of [...FILES, ...(opts?.extra || [])]) if (Glob.match(pattern, filepath)) return true
//!   return false
//! }
//! ```
//!
//! La derniere ligne de la source, `export * as Ignore from "./ignore"`, est un
//! reexport de l espace de noms du module sur lui-meme : c est du code mort, il
//! n y a rien a porter la-dessus. Le module Rust joue deja ce role.
//!
//! ## Les deux moities de la fonction ne se ressemblent pas
//!
//! C est le point central de ce fichier, et ce qui le distingue de
//! `util_glob`, dont il consomme la frontiere.
//!
//! - **La boucle des repertoires est de l algorithmie pure.** C est un `Set`
//!   de JavaScript consulte par egalite exacte sur chaque segment du chemin.
//!   Aucun moteur de motif n entre en jeu. C est 28 des 39 motifs du fichier,
//!   et c est integralement porte ici.
//! - **Les boucles de motifs sont une delegation.** Elles appellent
//!   `Glob.match(pattern, filepath)`, c est-a-dire
//!   `minimatch(filepath, pattern, { dot: true })`. `minimatch` n est pas une
//!   dependance de `Cargo.toml` ; `util_glob::r#match` le declare donc comme
//!   frontiere, et ce fichier n invente pas de moteur.
//!
//! Consequence concrete et verifiable : `r#match("node_modules/a", ...)` rend
//! `Ok(true)` **sans jamais appeler le moteur de motif**, parce que la source
//! sort a la deuxieme boucle. Un chemin comme `"src/index.ts"` doit atteindre la
//! troisieme et rend donc `Err`. Cette difference n est pas une approximation,
//! c est la structure de controle de la source elle-meme, et elle est testee.
//!
//! Un retour `Ok(false)` pour tout le monde aurait ete pire : il est
//! indiscernable d un `minimatch` qui ne trouve rien, exactement comme une
//! liste de glob vide l est pour `scan`.
//!
//! ## `PATTERNS` et `match` n appliquent pas la meme regle
//!
//! `export const PATTERNS = [...FILES, ...FOLDERS]` melange deux natures de
//! motifs dans une seule liste de chaines : onze motifs a jokers, puis
//! vingt-huit **noms nus**. Cette liste est consommee par
//! `packages/core/src/filesystem/watcher.ts` ligne 111, qui la passe a
//! l options `ignore` d un observateur de fichiers natif.
//!
//! Or `match` n utilise **jamais** `PATTERNS`, et n applique jamais les noms de
//! repertoires comme des motifs : il les teste par egalite de segment. Les deux
//! fonctions de la meme source obeissent donc a des regles differentes. Il ne
//! faut pas lire `PATTERNS` comme "la liste que `match` essaie".
//!
//! `FOLDERS` est un `Set`, donc `[...FOLDERS]` donne l ordre d insertion et
//! supprime les doublons. Les vingt-huit noms sont distincts : la deduplication
//! est donc inobservable ici, et l ordre de [`PATTERNS`] est bien celui de
//! [`FILES`] suivi de celui de [`FOLDERS`]. Test dedie.
//!
//! ## Le decoupage du chemin n est pas un parcours de `Path`
//!
//! `filepath.split(/[/\\]/)` est une **expression reguliere** : elle produit
//! une liste ou les segments vides sont **conserves**.
//!
//! - `""` donne `[""]`, soit un segment vide, pas une liste vide.
//! - `"node_modules/"` donne `["node_modules", ""]`.
//! - `"/node_modules"` donne `["", "node_modules"]`.
//! - `"a//b"` donne `["a", "", "b"]`.
//!
//! `std::path::Path::components` fait exactement l inverse sur ces cas : il
//! supprime les composants vides et renvoie `None` sur un chemin vide. Le
//! remplacer par lui ferait echouer `Ignore.match("node_modules/")`, qui est
//! explicitement attendu par `packages/core/test/filesystem/ignore.test.ts`
//! ligne 7. C est une regexp, pas un parcours de systeme de fichiers, et c est
//! aussi pourquoi cette fonction n touche **jamais** le disque.
//!
//! ## La casse : les deux moities n obeissent pas a la meme regle
//!
//! `FOLDERS.has(part)` est une recherche de `Set` : elle est **sensible a la
//! casse sur toutes les plateformes**, Windows comprise. `Node_Modules/a` ne
//! trouve donc aucun segment de repertoire, meme sur une machine Windows.
//!
//! Les motifs de [`FILES`] passent par `minimatch`, qui est insensible a la
//! casse sous Windows. La source est donc asymetrique, et elle l est par
//! construction : l un des deux tests est du JavaScript natif, l autre est une
//! bibliotheque tierce. Ce portage reproduit l asymetrie telle quelle, sans la
//! lisser. Test dedie.
//!
//! ## `opts?.x || []` est un test de VERACITE, pas un `??`
//!
//! Les deux occurrences de la source sont `opts?.whitelist || []` et
//! `opts?.extra || []`. L operateur est `||`, qui teste la **veracite**, et non
//! `??`, qui teste la **nullite**. Ce n est pas la meme expression.
//!
//! Sur le type reel de la source, `string[] | undefined`, les deux donnent le
//! meme resultat : la seule valeur fausse du type est `undefined`. Mais la
//! difference est reelle des le premier jour ou le type change, et elle se voit
//! deja sur un cas : un tableau **vide** est **truthy** en JavaScript, donc
//! `[] || []` rend le tableau d origine, pas un nouveau. `Some(vec![])` survit
//! donc ici, et n est pas remplace par une valeur par defaut. C est la
//! traduction de `unwrap_or_default()` sur l `Option`.
//!
//! ## L argument `opts` ne peut pas valoir "absent"
//!
//! La source declare `opts?: {...}`, donc `undefined` est un etat
//! representable, et le corps y acccede par `opts?.`. Mais le corps ne fait
//! **que** de l acces facultatif : jamais de test de presence, jamais de
//! reecriture. `opts === undefined` et `opts === {}` produisent donc des
//! executions strictement identiques.
//!
//! Cote Rust, `&Options` avec `&Options::default()` est l unique traduction
//! exacte, et aucune signature n autorise a passer "absent" : l etat
//! `undefined` de la source est ainsi rendu impossible a exprimer, au lieu d
//! etre accepte puis traite par un `if`. Meme arbitrage que dans `util_glob`,
//! pour la meme raison. Test dedie.
//!
//! ## Le piege des accolades, et pourquoi il ne s applique pas ici
//!
//! Un motif entre accolades comme `{tool,tools}/*.ts` n a de sens que si la
//! bibliotheque sait le developper. Le crate `glob` ne le fait pas, et le
//! piege est **silencieux** : une liste vide sans erreur, indiscernable d un
//! motif qui ne correspond a rien.
//!
//! **Aucun des trente-neuf motifs de ce fichier ne contient d accolade.** C est
//! verifie par un test, motif par motif, precisely pour que le fait reste vrai
//! si quelqu un ajoute un jour un motif `{a,b}` a [`FILES`], [`FOLDERS`] ou aux
//! options de l appelant.
//!
//! Le piege reste donc ouvert **ailleurs**, et il faut savoir ou :
//!
//! - Les entrees de `extra` et `whitelist` viennent de l appelant. En pratique
//!   elles sont **mortes** : sur tout le depot, le seul appelant est
//!   `packages/core/test/filesystem/ignore.test.ts`, et il ne passe aucune
//!   option. Mais des motifs comme `{a,b}/*.ts`{yesterday}` pourraient y
//!   arriver demain.
//! - `watcher.ts` ligne 111 concatene `PATTERNS` avec la configuration
//!   utilisateur `watcher.ignore`, qui est une liste de motifs **ecrits a la
//!   main**. C est le chemin par lequel des accolades entreraient vraiment en
//!   production, et il ne passe pas par ce fichier.
//!
//! Le correctif, le jour ou la fonction sera completee, est `minimatch` et rien
//! d autre : c est la bibliotheque que la source appelle, et elle developpe les
//! accolades. Une crate qui les ignore introduirait un defaut invisible.
//!
//! ## Rappel pour l agent principal
//!
//! Le module **n est pas encore declare** : `src/swarm/mod.rs` ne porte ni
//! `pub mod filesystem_ignore;` ni aucun `filesystem*`. Le fichier est donc
//! ecrit mais non compile tant que la ligne n est pas ajoutee, ce qui n a pas
//! ete fait ici (regle : un seul fichier touche).
//!
//! Les tests sont purs et instantanes : aucun thread, aucune attente, aucun
//! `sleep`, et **aucun acces au systeme de fichiers**. La source elle-meme
//! n en fait aucun non plus, ce qui permet de la porter entierement hors ligne.

use serde::{Deserialize, Serialize};

use crate::swarm::util_glob;

/// Les vingt-huit noms de repertoires ignores.
///
/// Cote TypeScript : un `Set` consulte par egalite exacte. Ce n est pas une
/// liste de motifs, et ces noms ne sont **jamais** testes comme tels ; ils
/// sont compares segment par segment, ce qui suppose que le chemin a ete
/// decoupe.
///
/// L ensemble ne contient aucun doublon, donc l usage de `Set` n'a aucun effet
/// observable sur cette liste et un tableau suffit. La recherche est lineaire :
/// vingt-huit elements, sans construction a l execution, donc sans
/// initialisation paresseuse ni `OnceLock`.
pub const FOLDERS: &[&str] = &[
    "node_modules",
    "bower_components",
    ".pnpm-store",
    "vendor",
    ".npm",
    "dist",
    "build",
    "out",
    ".next",
    "target",
    "bin",
    "obj",
    ".git",
    ".svn",
    ".hg",
    ".vscode",
    ".idea",
    ".turbo",
    ".output",
    "desktop",
    ".sst",
    ".cache",
    ".webkit-cache",
    "__pycache__",
    ".pytest_cache",
    "mypy_cache",
    ".history",
    ".gradle",
];

/// Les onze motifs de fichiers ignores, dans l ordre de la source.
///
/// Ce sont les seuls motifs a jokers du fichier, et les seuls qui passent par
/// `Glob.match`, donc par `minimatch`. Aucun ne contient d accolade.
pub const FILES: &[&str] = &[
    "**/*.swp",
    "**/*.swo",
    "**/*.pyc",
    "**/.DS_Store",
    "**/Thumbs.db",
    "**/logs/**",
    "**/tmp/**",
    "**/temp/**",
    "**/*.log",
    "**/coverage/**",
    "**/.nyc_output/**",
];

/// La liste exportee par la source : `[...FILES, ...FOLDERS]`.
///
/// C est une **copie**, faite a chaque lecture, comme le fait le langage
/// source. Un tableau `&'static [T]` ne peut pas etre copie implicitement, donc
/// la concatenation vit dans [`patterns`], et cette constante n est la que pour
/// fixer l ordre et permettre de le verifier par compilation.
///
/// Ces entrees sont de deux natures : onze motifs a jokers, puis vingt-huit
/// noms nus. Le seul appelant du depot,
/// `packages/core/src/filesystem/watcher.ts` ligne 111, la concatene a la
/// configuration utilisateur avant de la remettre a un observateur de
/// fichiers.
pub const PATTERNS: &[&str] = &[
    "**/*.swp",
    "**/*.swo",
    "**/*.pyc",
    "**/.DS_Store",
    "**/Thumbs.db",
    "**/logs/**",
    "**/tmp/**",
    "**/temp/**",
    "**/*.log",
    "**/coverage/**",
    "**/.nyc_output/**",
    "node_modules",
    "bower_components",
    ".pnpm-store",
    "vendor",
    ".npm",
    "dist",
    "build",
    "out",
    ".next",
    "target",
    "bin",
    "obj",
    ".git",
    ".svn",
    ".hg",
    ".vscode",
    ".idea",
    ".turbo",
    ".output",
    "desktop",
    ".sst",
    ".cache",
    ".webkit-cache",
    "__pycache__",
    ".pytest_cache",
    "mypy_cache",
    ".history",
    ".gradle",
];

/// Rend une copie de [`PATTERNS`], comme le fait l etalement `...` de la
/// source.
///
/// Le seul appelant du depot ecrit `[...Ignore.PATTERNS, ...config,
/// ...protecteds(...)]`, donc il lui faut une collection proprietaire qu il
/// peut extender. Rendre `Vec` plutot qu un slice evite que l appelant doive
/// connaitre la longueur.
pub fn patterns() -> Vec<&'static str> {
    PATTERNS.to_vec()
}

/// Dit si `part` est un nom de repertoire de la source.
///
/// Equivalent direct de `FOLDERS.has(part)` : egalite exacte, **sensible a la
/// casse sur toute plateforme**. Un segment vide n est jamais un repertoire, et
/// un segment qui contient un separateur ne peut pas en etre un puisque
/// [`path_parts`] les a deja retires.
pub fn is_folder(part: &str) -> bool {
    FOLDERS.contains(&part)
}

/// Decoupe `filepath` sur chaque separateur, en conservant les segments vides.
///
/// Portage de `filepath.split(/[/\\]/)`. C est une expression reguliere et non
/// un parcours de chemin : les deux separateurs sont acceptes simultanement,
/// un chemin melangeant les deux se decoupe normalement, et les segments vides
/// sont **conserves**.
///
/// C est la seule fonction de ce fichier qui transforme une chaine, et elle ne
/// touche pas le disque.
pub fn path_parts(filepath: &str) -> Vec<&str> {
    filepath
        .split(|c: char| c == '/' || c == '\\')
        .collect()
}

/// Les options presenteses par l appelant de [`r#match`].
///
/// Cote TypeScript : `{ extra?: string[]; whitelist?: string[] }`, un type
/// litteral en ligne, declare dans la signature de la fonction. Il ne porte
/// aucun nom de type, n est utilise par aucun autre module, et n est jamais
/// serialise par la source.
///
/// Le derive `serde` est donc un **ajout de portage**, utile parce que ces
/// options viennent de fichiers de configuration. Il enleve cependant une
/// verification : `null` se decode comme absent, la ou la source refuserait
/// `null` (seul `undefined` est admis, et l option elle-meme est facultative).
/// Consequence verifiable : `{"extra":null}` et `{}` donnent le meme objet.
///
/// Les deux noms sont en minuscules dans la source, donc aucun `serde(rename)`
/// n est necessaire. C est verifie par un test, parce que la majorite des
/// erreurs de nom de champ du portage sont invisibles de l interieur du code.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// Motifs supplementaires a tester **apres** ceux de [`FILES`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Vec<String>>,
    /// Motifs qui annulent l exclusion. Le premier qui correspond rend le
    /// fichier **non** ignore, et la fonction s arrete la.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whitelist: Option<Vec<String>>,
}

impl Options {
    /// Les motifs de la liste blanche, sans jamais en fabriquer un.
    ///
    /// Traduction de `opts?.whitelist || []`. L operateur source est `||`, donc
    /// un test de **veracite** : un tableau vide est truthy en JavaScript et
    /// survit, seule l absence est remplacee. Le resultat est donc un
    /// iterateur vide dans les deux cas, mais par deux chemins differents, et
    /// l option d origine n est jamais modifiee.
    pub fn whitelist_patterns<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        self.whitelist.iter().flatten().map(String::as_str)
    }

    /// Les motifs additionnels, sans jamais en fabriquer un.
    ///
    /// Traduction de `opts?.extra || []`, avec la meme remarque que pour
    /// [`Options::whitelist_patterns`].
    pub fn extra_patterns<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        self.extra.iter().flatten().map(String::as_str)
    }
}

/// Raison pour laquelle ce module ne peut pas repondre.
///
/// Elle ne signale pas un cheminAbsent, ni un motif qui ne correspond pas :
/// elle signifie que le moteur d appariement manque et que la reponse n est
/// pas connue. C est indispensable, parce qu un `Ok(false)` serait lu comme
/// "ce fichier n est pas ignore", ce qui est une **affirmation**, alors que la
/// seule verite est "je ne sais pas".
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IgnoreError {
    /// Le crate `minimatch` n est pas declare dans `Cargo.toml`. Les deux
    /// boucles de motifs de [`r#match`] ne peuvent donc pas etre executees.
    ///
    /// La boucle des repertoires, elle, n en a pas besoin : elle est entiere
    /// portee et repond `Ok(true)` sans jamais passer par cette variante.
    #[error("le crate minimatch n est pas declare dans Cargo.toml : les motifs de fichiers ne sont pas evaluables")]
    BibliothequeMinimatchAbsente,
}

/// Corps de [`r#match`], avec l appariement de motifs branche par l appelant.
///
/// Le parametre `glob_match` occupe la place de `Glob.match` : il recoit le
/// **motif en premier et le chemin en second**, comme l appel de la source
/// ligne 52 et 61. C est l ordre inverse de l appel a `minimatch` lui-meme,
/// qui prend `(filepath, pattern)`, et l ordre inverse de `util_wildcard`, qui
/// prend `(input, pattern)`. Les deux conventions cohabitent deja dans le
/// depot : le nom des parametres est ici le seul garde-fou, d ou la presence de
/// `motif` puis `chemin` dans la signature.
///
/// Ce n est pas une API de la source, c est une couture : elle rend testable
/// l ordre d evaluation, qui est la seule logique reellement portee, sans
/// invoquer de moteur de motif. Un test y branche un [`r#match`] factice qui
/// note les motifs recus.
pub fn r#match_with<F>(filepath: &str, opts: &Options, mut glob_match: F) -> Result<bool, IgnoreError>
where
    F: FnMut(&str, &str) -> Result<bool, IgnoreError>,
{
    // 1. liste blanche : le premier motif qui correspond dit "ne pas ignorer",
    //    et la fonction s arrete immediatement, avant meme de regarder les
    //    repertoires. C est ce qui permet a un motif de liste blanche de
    //    franchir un repertoire interdit.
    for motif in opts.whitelist_patterns() {
        if glob_match(motif, filepath)? {
            return Ok(false);
        }
    }

    // 2. segments du chemin : egalite exacte, insensible a rien.
    for part in path_parts(filepath) {
        if is_folder(part) {
            return Ok(true);
        }
    }

    // 3. motifs de fichiers, puis ceux de l appelant. L ordre n est pas
    //    observable : chaque branche rend le meme booleen, et la fonction
    //    s arrete au premier succes. Il est conserve parce qu il decide par
    //    combien d appels le moteur est sollicite.
    for motif in FILES.iter().copied().chain(opts.extra_patterns()) {
        if glob_match(motif, filepath)? {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Dit si `filepath` doit etre ignore.
///
/// La fonction respecte l ordre de la source et s'arrete des qu'une reponse est
/// connue :
///
/// 1. un motif de `opts.whitelist` qui correspond rend `Ok(false)` ;
/// 2. un segment du chemin egal a un nom de [`FOLDERS`] rend
///    `Ok(true)`, **sans jamais appeler le moteur de motif** ;
/// 3. sinon, les onze motifs de [`FILES`] puis ceux de `opts.extra` doivent
///    etre evalues par `minimatch`, qui manque, et la fonction rend
///    [`IgnoreError::BibliothequeMinimatchAbsente`].
///
/// Les points 1 et 2 sont integralement ports. Le point 3 est une frontiere,
/// declaree dans `util_glob` comme dans ce fichier, et le jour ou `minimatch`
/// arrive il n y a que le branchement de `glob_match` a refaire.
///
/// Consequence a connaitre de l appelant : la reponse depend du chemin, pas
/// seulement des motifs. Un chemin sous `node_modules` est decide sans
/// `minimatch`, un chemin ordinaire ne l est pas. C est un accident de
/// l ordre d evaluation de la source, reproduit tel quel.
pub fn r#match(filepath: &str, opts: &Options) -> Result<bool, IgnoreError> {
    r#match_with(filepath, opts, |motif: &str, chemin: &str| -> Result<bool, IgnoreError> {
        // `util_glob::r#match` declare cette frontiere et ne leve que
        // `BibliothequeMinimatchAbsente` : l autre variante de son enum
        // appartient a `scan` et `scan_sync`, que ce fichier n appelle pas.
        // La conversion est donc totale, sans perte.
        util_glob::r#match(motif, chemin).map_err(|_| IgnoreError::BibliothequeMinimatchAbsente)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Appariement factice qui note les motifs recus et repond toujours non.
    ///
    /// Il permet d observer la liste et l ordre des motifs testes sans
    /// moteur, donc sans `minimatch` et sans disque.
    fn jamais(notes: &mut Vec<String>) -> impl FnMut(&str, &str) -> Result<bool, IgnoreError> + '_ {
        move |motif: &str, _: &str| -> Result<bool, IgnoreError> {
            notes.push(motif.to_string());
            Ok(false)
        }
    }

    #[test]
    fn les_cinq_cas_du_test_de_la_source_sont_decides_par_un_segment_de_repertoire() {
        // `packages/core/test/filesystem/ignore.test.ts` lignes 5 a 9. Les
        // cinq doivent passer par la boucle des repertoires, donc sans
        // minimatch. Les deux qui finissent par un separateur ne sont justes
        // que si les segments vides sont conserves.
        for chemin in [
            "node_modules/index.js",
            "node_modules",
            "node_modules/",
            "node_modules/bar",
            "node_modules/bar/",
        ] {
            assert_eq!(
                r#match(chemin, &Options::default()),
                Ok::<bool, IgnoreError>(true),
                "{:?} doit etre ignore",
                chemin
            );
        }
    }

    #[test]
    fn le_decoupage_accepte_la_barre_oblique_et_la_contre_oblique() {
        // La classe de caracteres `[/\\]` de la regexp. Un chemin Windows et
        // un chemin POSIX se comportent identiquement.
        assert_eq!(path_parts("a/b/c"), vec!["a", "b", "c"]);
        assert_eq!(path_parts("a\\b\\c"), vec!["a", "b", "c"]);
        assert_eq!(path_parts("a/b\\c"), vec!["a", "b", "c"]);
        assert_eq!(path_parts("C:\\projet\\src"), vec!["C:", "projet", "src"]);
    }

    #[test]
    fn les_separateurs_consecutifs_produisent_des_segments_vides_qui_sont_conserves() {
        // C'est la difference avec `Path::components`, qui les supprimerait.
        // Le chemin vide du milieu compte donc comme segment, et un segment
        // vide n'est pas un repertoire.
        assert_eq!(path_parts("a//b"), vec!["a", "", "b"]);
        assert_eq!(path_parts("/node_modules"), vec!["", "node_modules"]);
        assert_eq!(path_parts("node_modules/"), vec!["node_modules", ""]);
        assert_eq!(path_parts("///"), vec!["", "", "", ""]);
        assert_eq!(r#match("a//node_modules//b", &Options::default()), Ok(true));
    }

    #[test]
    fn un_chemin_vide_produit_un_seul_segment_vide_et_pas_une_liste_vide() {
        // `"".split(/[/\\]/)` rend `[""]`. Le resultat ne peut donc pas etre
        // confondu avec un chemin qui n aurait aucun segment.
        assert_eq!(path_parts(""), vec![""]);
        assert!(!is_folder(""));
        assert_ne!(path_parts(""), Vec::<&str>::new());
    }

    #[test]
    fn un_repertoire_ignore_est_reconnu_a_la_premiere_au_milieu_et_a_la_derniere_position() {
        // La source teste **tous** les segments, sans s'arreter au premier
        // comme le ferait une comparaison de prefixe.
        for chemin in [
            "dist/a.ts",
            "a/dist/b.ts",
            "a/b/dist",
            "/dist",
            "dist/",
            "a\\dist\\b",
        ] {
            assert!(
                path_parts(chemin).iter().any(|p| is_folder(p)),
                "{:?} doit contenir un segment de repertoire",
                chemin
            );
            assert_eq!(
                r#match(chemin, &Options::default()),
                Ok::<bool, IgnoreError>(true),
                "{:?} doit etre ignore",
                chemin
            );
        }
    }

    #[test]
    fn la_recherche_d_un_repertoire_est_sensible_a_la_casse_sur_toute_plateforme() {
        // `FOLDERS.has` est une recherche de `Set`, donc sensible a la casse,
        // Windows comprise. C'est le contraste avec `minimatch`, qui est
        // insensible a la casse sous Windows : la source est asymetrique et le
        // portage ne lissage rien.
        assert!(is_folder("node_modules"));
        assert!(!is_folder("Node_Modules"));
        assert!(!is_folder("NODE_MODULES"));
        assert!(!is_folder("node_modules "));
        assert!(!is_folder(" node_modules"));
        assert!(!is_folder("node_modules.js"));
    }

    #[test]
    fn les_tous_repertoires_de_la_source_sont_reconnus_seuls_et_accompagnes_d_un_fichier() {
        // Vingt-huit entrees, testees une par une, pour qu un nom fausse dans
        // la liste soit designe par le test et non par la CI.
        assert_eq!(FOLDERS.len(), 28);
        for nom in FOLDERS {
            assert!(is_folder(nom), "{:?} doit etre un repertoire ignore", nom);
            assert_eq!(
                r#match(nom, &Options::default()),
                Ok::<bool, IgnoreError>(true),
                "{:?} seul doit etre ignore",
                nom
            );
            let imbrique = format!("projet/{}/fichier.ts", nom);
            assert_eq!(
                r#match(&imbrique, &Options::default()),
                Ok::<bool, IgnoreError>(true),
                "{:?} imbrique doit etre ignore",
                nom
            );
        }
    }

    #[test]
    fn aucun_nom_de_repertoire_ne_contient_de_separateur_et_les_listes_ne_se_recoupent_pas() {
        // Deux invariants de la source qui rendent le decoupage equivalent a
        // une recherche de sous-chaine, et qui justifient de ne pas comparer
        // autrement. `bin` est le seul nom qui soit un fragment d'un autre
        // motif, jamais d'un autre nom.
        for nom in FOLDERS {
            assert!(
                !nom.contains('/') && !nom.contains('\\'),
                "{:?} ne doit pas contenir de separateur",
                nom
            );
        }
        for motif in FILES {
            assert!(
                !FOLDERS.contains(motif),
                "{:?} ne doit pas etre aussi un nom de repertoire",
                motif
            );
        }
    }

    #[test]
    fn les_motifs_de_fichiers_sont_dans_l_ordre_de_la_source() {
        // Onze motifs, et surtout pas dans un autre ordre : meme si le
        // booleen final ne depend pas de l'ordre, le nombre d'appels au
        // moteur, lui, en depend.
        assert_eq!(FILES.len(), 11);
        assert_eq!(
            FILES.to_vec(),
            vec![
                "**/*.swp",
                "**/*.swo",
                "**/*.pyc",
                "**/.DS_Store",
                "**/Thumbs.db",
                "**/logs/**",
                "**/tmp/**",
                "**/temp/**",
                "**/*.log",
                "**/coverage/**",
                "**/.nyc_output/**",
            ]
        );
    }

    #[test]
    fn patterns_conserve_l_ordre_fichiers_puis_repertoires() {
        // `[...FILES, ...FOLDERS]`. Le `Set` rend l'ordre d'insertion et
        // supprime les doublons ; il n'y a aucun doublon, donc l'ordre est
        // exactement celui des deux listes, l'une apres l'autre.
        let attendu: Vec<&str> = FILES.iter().chain(FOLDERS.iter()).copied().collect();
        assert_eq!(PATTERNS.to_vec(), attendu);
        assert_eq!(PATTERNS.len(), 39);
        assert_eq!(patterns().len(), 39);
        assert_eq!(patterns(), PATTERNS.to_vec());
        // Les onze premiers sont des motifs a jokers, les vingt-huit suivants
        // sont des noms nus : les deux natures cohabitent dans la meme liste.
        assert_eq!(&PATTERNS[..11], FILES);
        assert_eq!(&PATTERNS[11..], FOLDERS);
    }

    #[test]
    fn patterns_rend_une_copie_modifiable_independante() {
        // Le seul appelant fait `[...Ignore.PATTERNS, ...config, ...]`, donc il
        // lui faut une collection qu'il peut etendre sans toucher la source.
        let mut copie = patterns();
        copie.push("motif-de-l-appelant");
        assert_eq!(copie.len(), 40);
        assert_eq!(PATTERNS.len(), 39, "la constante ne doit pas avoir bouge");
    }

    #[test]
    fn la_liste_blanche_est_evaluee_avant_meme_la_recherche_des_repertoires() {
        // C'est l'ordre de la source, et il est observable : un motif de liste
        // blanche qui correspond renvoie "ne pas ignorer" meme pour un chemin
        // sous `node_modules`, et la boucle des repertoires n'est jamais
        // atteinte.
        let opts = Options {
            extra: None,
            whitelist: Some(vec!["**/a.ts".to_string()]),
        };
        let mut notes: Vec<String> = Vec::new();
        let resultat = r#match_with("node_modules/a.ts", &opts, |motif: &str, _: &str| -> Result<bool, IgnoreError> {
            notes.push(motif.to_string());
            Ok(true)
        });
        assert_eq!(resultat, Ok(false), "la liste blanche l'emporte");
        assert_eq!(notes, vec!["**/a.ts"], "les motifs de fichiers ne sont pas testes");
    }

    #[test]
    fn une_liste_blanche_vide_survit_et_ne_bloque_rien() {
        // `[] || []` rend le tableau d'origine : un tableau vide est truthy en
        // JavaScript. Le resultat observable est le meme qu'avec l'absence, mais
        // l chemin est celui de la veracite, pas celui de la nullite.
        let opts = Options {
            extra: Some(Vec::new()),
            whitelist: Some(Vec::new()),
        };
        let mut notes: Vec<String> = Vec::new();
        assert_eq!(
            r#match_with("node_modules/a", &opts, jamais(&mut notes)),
            Ok::<bool, IgnoreError>(true)
        );
        assert!(
            notes.is_empty(),
            "aucun motif ne doit etre teste, la liste blanche etant vide : {:?}",
            notes
        );
    }

    #[test]
    fn une_liste_blanche_non_correspondante_laisse_la_recherche_continuer() {
        // Le second motif de la liste blanche gagne, sinon le chemin est
        // decide par son segment de repertoire. Les deux branches rendent donc
        // le meme booleen pour une raison opposee, ce qui est la source.
        let opts = Options {
            extra: None,
            whitelist: Some(vec![
                "**/absent/**".to_string(),
                "**/node_modules/a".to_string(),
            ]),
        };
        let mut notes: Vec<String> = Vec::new();
        let resultat = r#match_with("node_modules/a", &opts, |motif: &str, chemin: &str| -> Result<bool, IgnoreError> {
            notes.push(motif.to_string());
            Ok(motif.ends_with("node_modules/a") && chemin == "node_modules/a")
        });
        assert_eq!(resultat, Ok(false));
        assert_eq!(notes, vec!["**/absent/**", "**/node_modules/a"]);
    }

    #[test]
    fn l_appariement_recoit_le_motif_puis_le_chemin_dans_ce_ordre() {
        // Le piege d'ordre de la source. `Glob.match(pattern, filepath)` est
        // l'inverse de l'appel a `minimatch(filepath, pattern)`, et l'inverse
        // aussi de `util_wildcard::r#match(input, pattern)`. Inverser les deux
        //ici ne produirait aucun avertissement du compilateur.
        let mut notes: Vec<(String, String)> = Vec::new();
        let resultat = r#match_with("a/b.log", &Options::default(), |motif: &str, chemin: &str| -> Result<bool, IgnoreError> {
            notes.push((motif.to_string(), chemin.to_string()));
            Ok(false)
        });
        assert_eq!(resultat, Ok(false));
        assert_eq!(notes[0], ("**/*.swp".to_string(), "a/b.log".to_string()));
        assert!(
            notes.iter().all(|(_, chemin)| chemin == "a/b.log"),
            "le second argument est toujours le chemin : {:?}",
            notes
        );
    }

    #[test]
    fn les_motifs_optionnels_viennent_apres_les_onze_motifs_de_fichiers() {
        // `[...FILES, ...(opts?.extra || [])]`. L'ordre n'est pas observable
        // dans le booleen, mais il l'est dans le nombre d'appels, donc il est
        // conserve et verifie.
        let opts = Options {
            extra: Some(vec!["**/extra-un/**".to_string(), "**/extra-deux/**".to_string()]),
            whitelist: None,
        };
        let mut notes: Vec<String> = Vec::new();
        let resultat = r#match_with("a/b", &opts, jamais(&mut notes));
        assert_eq!(resultat, Ok(false));
        assert_eq!(notes.len(), 13, "onze motifs de fichiers plus deux motifs optionnels");
        let attendus: Vec<String> = FILES.iter().map(|m| m.to_string()).collect();
        assert_eq!(notes[..11], attendus);
        assert_eq!(notes[11], "**/extra-un/**");
        assert_eq!(notes[12], "**/extra-deux/**");
    }

    #[test]
    fn un_motif_optionnel_peut_porter_la_decision_a_lui_seul() {
        // Le cas ou `extra` est la seule source d appariement du fichier : c
        // est le role de l option dans la source, et il ne fonctionne qu avec
        // un moteur, donc ici avec la couture.
        let opts = Options {
            extra: Some(vec!["**/genere/**".to_string()]),
            whitelist: None,
        };
        let mut notes: Vec<String> = Vec::new();
        let resultat = r#match_with("src/genere/x.ts", &opts, |motif: &str, _: &str| -> Result<bool, IgnoreError> {
            notes.push(motif.to_string());
            Ok(motif == "**/genere/**")
        });
        assert_eq!(resultat, Ok(true));
        assert_eq!(notes.last().map(String::as_str), Some("**/genere/**"));
    }

    #[test]
    fn sans_le_crate_minimatch_un_chemin_ordinaire_signale_l_indisponibilite() {
        // Un chemin sans segment de repertoire atteint la troisieme boucle et
        // ne peut pas etre tranche. Le message doit le dire, et non rendre un
        // `false` qui serait lu comme une affirmation.
        for chemin in [
            "src/index.ts",
            "a/b/c",
            "a/b.log",
            ".gitignore",
            "node_modules.js",
            "",
            "C:\\projet\\src\\index.ts",
        ] {
            assert_eq!(
                r#match(chemin, &Options::default()),
                Err::<bool, IgnoreError>(IgnoreError::BibliothequeMinimatchAbsente),
                "{:?} exige minimatch",
                chemin
            );
        }
    }

    #[test]
    fn l_indisponibilite_du_crate_ne_se_confond_pas_avec_un_faux() {
        // Le point le plus important du fichier. `Ok(false)` dirait "ne pas
        // ignorer" ; l'erreur dit "je ne sais pas". C'est la difference entre
        // une reponse et une frontiere.
        let resultat = r#match("src/index.ts", &Options::default());
        assert!(resultat.is_err());
        assert!(
            !matches!(resultat, Ok(false)),
            "un faux serait indiscernable d'une reponse Negative reelle"
        );
        assert!(!matches!(resultat, Ok(true)));
    }

    #[test]
    fn un_repertoire_connu_se_repond_sans_le_crate_minimatch() {
        // La structure de controle de la source, pas une approximation : la
        // deuxieme boucle sort avant d'appeler le moteur. C'est ce qui permet
        // aux cinq cas du test d'origine de se comporter comme en JavaScript.
        assert_eq!(r#match("node_modules", &Options::default()), Ok(true));
        assert_eq!(r#match("a/.git/b", &Options::default()), Ok(true));
        assert_eq!(r#match("C:\\p\\node_modules\\q", &Options::default()), Ok(true));
        assert_eq!(r#match("__pycache__", &Options::default()), Ok(true));
        assert_eq!(r#match("projet/.gradle/caches", &Options::default()), Ok(true));
    }

    #[test]
    fn une_liste_blanche_non_vide_bloque_meme_un_repertoire_connu() {
        // L'absence du moteur se voit des la premiere boucle. C'est la preuve
        // que la liste blanche est bien evaluee en premier, et qu'un chemin
        // decide par son segment ne l'echappe pas.
        let opts = Options {
            extra: None,
            whitelist: Some(vec!["**/a.ts".to_string()]),
        };
        assert_eq!(
            r#match("node_modules/a.ts", &opts),
            Err::<bool, IgnoreError>(IgnoreError::BibliothequeMinimatchAbsente)
        );
    }

    #[test]
    fn l_option_extra_ne_permet_pas_de_contourner_l_indisponibilite_du_moteur() {
        // Meme avec des motifs additionnels, la reponse ne peut pas etre
        // connue : c'est le moteur qui manque, pas les motifs.
        let opts = Options {
            extra: Some(vec!["**/tout/**".to_string()]),
            whitelist: None,
        };
        assert_eq!(
            r#match("src/index.ts", &opts),
            Err::<bool, IgnoreError>(IgnoreError::BibliothequeMinimatchAbsente)
        );
    }

    #[test]
    fn aucun_motif_du_depot_ne_contient_d_accolades() {
        // Le piege du lot. Un motif `{a,b}` demande un moteur qui developpe les
        // accolades ; le crate `glob` ne le fait pas et echoue en silence sur
        // une liste vide. Aucun des trente-neuf motifs de ce fichier n'est
        // concerne, et ce test existe pour le prouver maintenant plutot que le
        // jour ou quelqu un en ajoutera un.
        for motif in PATTERNS {
            assert!(
                !motif.contains('{') && !motif.contains('}'),
                "{:?} contient des accolades : minimatch les developpe, pas glob",
                motif
            );
        }
        // Les motifs reels du depot qui, eux, en contiennent. Ils ne passent
        // pas par ce fichier aujourd'hui, et c'est bien le probleme : ils
        // confirment que le developpement d'accolades reste une frontiere pour
        // tout ce qui touche aux motifs du projet.
        for motif in ["{tool,tools}/*.{js,ts}", "{command,commands}/**/*.md"] {
            assert!(motif.contains('{'));
        }
    }

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        // `extra` et `whitelist` sont tout en minuscules dans la source, donc
        // aucun `serde(rename)` n est requis. Le test est la au cas ou : c'est
        // l'erreur la plus frequente du portage, et elle est invisible de
        // l'interieur du code Rust.
        let options = Options {
            extra: Some(vec!["**/a/**".to_string()]),
            whitelist: Some(vec!["**/b/**".to_string()]),
        };
        let json = serde_json::to_string(&options).expect("la serialisation ne doit pas echouer");
        assert_eq!(
            json,
            "{\"extra\":[\"**/a/**\"],\"whitelist\":[\"**/b/**\"]}"
        );
    }

    #[test]
    fn une_option_absente_disparait_du_json_et_un_null_vaut_absence() {
        // `skip_serializing_if` fait disparaitre l option, comme le fait
        // `undefined` cote source. Et `null` se lit comme absent, ce que la
        // source refuserait : c'est l prix du derive ajoute par le portage.
        let json = serde_json::to_string(&Options::default()).expect("serialisation");
        assert_eq!(json, "{}", "aucune option ne doit etre ecrite");
        let avec_null: Options =
            serde_json::from_str("{\"extra\":null,\"whitelist\":null}").expect("decodage");
        assert_eq!(avec_null, Options::default());
    }

    #[test]
    fn un_objet_json_vide_et_une_option_vide_donnent_les_memes_absences() {
        // `#[serde(default)]` reproduit le comportement de `opts?.` : une cle
        // manquante devient une absence, pas une liste vide. Les deux restent
        // neanmoins des valeurs distinctes du type.
        let vide: Options = serde_json::from_str("{}").expect("decodage");
        assert_eq!(vide, Options::default());
        assert_eq!(vide.whitelist, None);
        assert_eq!(vide.extra, None);
        let explicite = Options {
            extra: Some(Vec::new()),
            whitelist: Some(Vec::new()),
        };
        assert_ne!(vide, explicite, "absent et liste vide sont deux valeurs");
    }

    #[test]
    fn une_option_absente_agit_comme_un_objet_vide() {
        // La source declare `opts?`, donc `undefined` est un etat
        // representable. Le corps ne fait que de l'acces facultatif, donc
        // `undefined` et `{}` sont indiscernables. C'est ce qui autorise la
        // signature `&Options` sans perte.
        let mut notes_absente: Vec<String> = Vec::new();
        let mut notes_vide: Vec<String> = Vec::new();
        let a = r#match_with("a/b", &Options::default(), jamais(&mut notes_absente));
        let b = r#match_with("a/b", &Options::default(), jamais(&mut notes_vide));
        assert_eq!(a, b);
        assert_eq!(notes_absente, notes_vide);
        // Une option en plus ne fait pas de mal non plus : elle ne change que
        // la liste des motifs testes.
        let c = r#match_with("a/b", &Options { extra: None, whitelist: None }, jamais(&mut notes_vide));
        assert_eq!(a, c);
    }

    #[test]
    fn les_iterateurs_d_options_ne_fabriquent_jamais_de_liste_par_defaut() {
        // `opts?.x || []` ne remplace que l'absence. Les methodes rendent un
        // iterateur, donc il n y a pas de `Vec` temporaire a construire, et le
        // nombre d'appels au moteur est exactement le nombre de motifs
        // fournis.
        let options = Options::default();
        assert_eq!(options.whitelist_patterns().count(), 0);
        assert_eq!(options.extra_patterns().count(), 0);
        let options = Options {
            extra: Some(vec!["a".to_string(), "b".to_string()]),
            whitelist: Some(vec!["c".to_string()]),
        };
        assert_eq!(options.whitelist_patterns().collect::<Vec<&str>>(), vec!["c"]);
        assert_eq!(options.extra_patterns().collect::<Vec<&str>>(), vec!["a", "b"]);
    }

    #[test]
    fn la_fonction_est_pure_et_ne_modifie_pas_les_options() {
        // Aucune ecriture, aucune temporisation, aucun acces disque : deux
        // appels avec la meme entree rendent la meme reponse, et l'objet
        // d'entree reste intact. C'est ce qui permet a la CI de faire tourner
        // ces tests en quelques millisecondes.
        let options = Options {
            extra: Some(vec!["**/a/**".to_string()]),
            whitelist: Some(vec!["**/b/**".to_string()]),
        };
        let avant = options.clone();
        let mut notes1: Vec<String> = Vec::new();
        let mut notes2: Vec<String> = Vec::new();
        let r1 = r#match_with("src/a/c.ts", &options, jamais(&mut notes1));
        let r2 = r#match_with("src/a/c.ts", &options, jamais(&mut notes2));
        assert_eq!(r1, r2);
        assert_eq!(notes1, notes2);
        assert_eq!(options, avant, "les options d'origine restent inchangees");
    }

    #[test]
    fn les_deux_appariements_partagent_la_meme_erreur_et_elle_est_unique() {
        // `r#match` branche `util_glob::r#match`, dont la conversion est
        // totale : une seule et meme erreur, quel que soit le point d'entree.
        let options = Options::default();
        let par_chemin = r#match("src/a.ts", &options);
        let par_couture = r#match_with(
            "src/a.ts",
            &options,
            |_: &str, _: &str| -> Result<bool, IgnoreError> {
                Err(IgnoreError::BibliothequeMinimatchAbsente)
            },
        );
        assert_eq!(par_chemin, par_couture);
        // Le message nomme la bibliotheque manquante, ce qui est la seule
        // information actionnable pour un appelant.
        let texte = par_chemin.unwrap_err().to_string();
        assert!(texte.contains("minimatch"), "message : {}", texte);
        assert!(!texte.is_empty());
    }

    #[test]
    fn un_nom_de_repertoire_inconnu_ne_declenche_rien() {
        // Le bruit le plus courant dans les chemins reels. Aucun de ces
        // segments n est un nom de la liste, donc la boucle des repertoires
        // sort sans rien trouver, et le moteur est alors requis. `desktop` et
        // `target` sont dans la liste, `packages` et `src` non : c est une
        // liste de mots entiers, pas une liste de sous-chaines.
        for chemin in [
            "src",
            "packages",
            "node_modules_extra",
            "test",
            "Cargo.toml",
            "src/swarm",
            "a/.github/b",
            "distribution",
        ] {
            assert!(
                !path_parts(chemin).iter().any(|p| is_folder(p)),
                "{:?} ne doit contenir aucun repertoire de la liste",
                chemin
            );
        }
    }
}
