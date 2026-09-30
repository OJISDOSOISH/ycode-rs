//! Portage Rust de `opencode/packages/core/src/util/module.ts`.
//!
//! ## Ce que la source contient reellement
//!
//! Dix lignes, dont **une seule instruction** :
//!
//! ```ts
//! export namespace Module {
//!   export function resolve(id: string, dir: string) {
//!     try {
//!       return createRequire(path.join(dir, "package.json")).resolve(id)
//!     } catch {}
//!   }
//! }
//! ```
//!
//! Trois points se lisent avant tout le reste.
//!
//! 1. **C est une resolution, pas un chargement.** La source n appelle jamais
//!    `require(id)`. Elle demande a Node *ou se trouve* un fichier, et c est tout.
//!    Aucune valeur de module n est produite, aucun cache n est consulte, aucun
//!    `import()` n est emis. Ce fichier ne charge donc **aucun** code
//!    JavaScript, et ce portage ne fabrique **aucun** chargeur de modules Rust :
//!    il n y a rien a charger.
//! 2. **Le `catch` est vide, et il porte tout le contrat.** Un echec ne leve
//!    rien et ne renvoie rien : la fonction rend `string | undefined`. Le type
//!    de retour est donc [`Option<String>`] et **pas** un `Result`. `None`
//!    signifie `introuvable`, ce qui est une reponse normale du contrat, pas
//!    une erreur a remonter. Un portage qui aurait rendu `Result<_, E>` aurait
//!    change la signature pour rien.
//! 3. **`createRequire` ne fait qu ancrer un repertoire.** Il transforme
//!    `path.join(dir, "package.json")` en chemin de fichier, puis prend son
//!    `dirname` comme point de depart de la resolution.reduire cela a une
//!    normalisation de `dir` est exact : le `package.json` n est jamais lu, et
//!    son absence n a aucune importance.
//!
//! `export namespace Module` cree l espace de noms qui permet d ecrire
//! `Module.resolve(...)`. En Rust le module joue deja ce role, via
//! `use crate::swarm::util_module::resolve`. Il n y a donc rien a ecrire
//! dessus, exactement comme dans `util_glob`.
//!
//! ## Ce qui est porte
//!
//! L algorithme de resolution CommonJS, tel que Node le **documente** dans la
//! documentation du chargeur, et non tel qu il est cable en interne :
//!
//! - un specifiant commence par `./`, `../` ou est absolu : on sonde le
//!   fichier, puis son dossier ;
//! - sinon le nom du paquet est le premier segment (deux si le nom commence par
//!   `@`), le reste est un sous-chemin a l interieur du paquet ;
//! - le paquet est cherche dans `<base>/node_modules`, puis dans le
//!   `node_modules` de chaque ancetre, du plus proche au plus lointain ;
//! - un fichier se sonde tel quel, puis `.js`, `.json`, `.node` ;
//! - un dossier se sonde via `package.json` puis `main`, puis via `index` avec
//!   les memes extensions.
//!
//! Cette partie est **de la logique de chemins** : elle est portable, elle est
//! publique, et elle est verifiee par le fichier de tests du depot lui-meme,
//! `packages/opencode/test/util/module.test.ts`, dont les quatre cas sont
//! reproduits ici. Le seul acces au systeme de fichiers est l existence d un
//! chemin (`is_file`, `is_dir`) et la lecture de `package.json`.
//!
//! ## Ce qui n est PAS porte, et pourquoi
//!
//! Aucun des points suivants n a d equivalent portable ici. Aucun n est
//! simule : ce sont des trous declares, pas des approximations.
//!
//! - **`exports`, `imports`, et l auto-reference de paquet.** Node refuse
//!   aujourd hui de resoudre un sous-chemin absent de `exports`, et un paquet
//!   dont `exports` existe ignore `main`. Ce portage **utilise `main`**. C est
//!   la divergence la plus visible, et elle ne se voit pas sur les quatre
//!   tests du depot : voir la section "point faible" du rapport.
//! - **Les conditions d export** (`import` / `require` / `default` / `node`),
//!   les jokers de motif `"."` et le blocage par `null` : ce sont des donnees
//!   pilotant un moteur, pas de la logique de chemin.
//! - **Les modules integres** : `Module.resolve("node:fs", dir)` rend
//!   `"node:fs"` cote Node, ici `None`. Aucune liste de modules integres n est
//!   ecrite a la main, parce qu elle serait fausse des la premiere version.
//! - **Les racines additionnelles** de `NODE_PATH` et de `node_modules`
//!   globaux, que Node ajoute a celles du repertoire de depart.
//! - **La resolution ESM** : `import`, les extensions `.mjs`, `.cjs`, `.wasm`,
//!   `.ts`, les URL `file:` / `data:` / `node:`, et le mode
//!   `--preserve-symlinks` avec sa resolution de liens.
//!
//! Reimplementer le moteur `exports` seul est tentant, parce qu il tient en
//! quelques centaines de lignes. Ce serait exactement l'erreur de `util_glob`
//! : un moteur qui **parait** refaire Node et qui diverge sur le premier
//! paquet dont `exports` porte une condition. Un fichier honnete et partiel
//! vaut mieux qu un fichier complet et invente.
//!
//! ## Les deux pieges de cette conversion
//!
//! 1. **`path.join(dir, "package.json")` normalise, et le nom de fichier
//!    disparait ensuite.** Le `dirname` du resultat redonne `dir` *normalise* :
//!    `"a/b/.."` devient `"a"`, `"a//b"` devient `"a/b"`, `""` et `"."`
//!    deviennent le repertoire courant. Le point de depart de la resolution
//!    n est donc jamais exactement la chaine recue. C est
//!    [`base_dir`] qui porte ce comportement, et il est teste.
//! 2. **Un dossier de base relatif est ancre sur le repertoire courant du
//!    processus.** `path.resolve("projet")` vaut `<cwd>/projet` en JavaScript,
//!    et `normalize` + `current_dir` font la meme chose ici. Un dossier de base
//!    absolu, lui, ne consulte jamais le repertoire courant : c est ce qui rend
//!    le resultat reproductible.
//!
//! ## Ce que ce fichier ne decide pas
//!
//! Il ne charge rien, il ne lit pas le contenu du fichier trouve, il ne
//! verifie pas que c est du JavaScript valide, et il ne tient aucun cache.
//! Deux resolutions successives relisent le systeme de fichiers. Les quatre
//! sites d appel du depot (`packages/opencode/src/lsp/server.ts` lignes 123,
//! 178, 338 et 1101) consomment la chaine rendue pour y chercher un binaire
//! ou un serveur, et tous traitent `undefined` comme "cette dependance n est
//! pas installee" : voir [`resolve`].
//!
//! `resolve` est donc le seul point d entree du module. Les fonctions de
//! chemin sont publiques parce qu elles sont purses et testables sans
//! disque, et parce qu un appelant peut vouloir la liste des candidats sans
//! toucher au systeme de fichiers.

use serde::Deserialize;
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Extensions employees par Node pour un fichier, dans l ordre d essai.
///
/// C est l ordre des cles de `require.extensions`, donc l'ordre de
/// resolution d un nom sans extension. Les trois suffisent : Node ne teste
/// aucune autre extension pour un `require`.
const EXTENSIONS: [&str; 3] = [".js", ".json", ".node"];

/// Borne de recursion sur les dossiers.
///
/// Node s'en sort sans borne, en comptant sur `toRealPath` pour casser les
/// cycles de liens symboliques. Cette resolution ne fait pas de `realpath`,
/// alors qu'une boucle `node_modules/a/node_modules/b/node_modules/...` ferait
/// tourner la fonction. Vingt-quatre niveaux sont tres au dela de ce qu'atteint
/// `main` dans un paquet reel, et la borne est documentee plutot que
/// silencieuse.
const PROFONDEUR_MAX: usize = 24;

/// Ce que la source demande au systeme de fichiers.
///
/// Un seul champ est lu : `main`. Les autres cles d un `package.json` sont
/// hors de portee, en particulier `exports`, voir [`resolve`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Manifeste {
    /// Champ `main` de `package.json`. Absent ou vide : le dossier se resout
    /// alors par `index`.
    ///
    /// Le nom est deja en minuscules cote source, donc aucun `serde(rename)`
    /// n est necessaire. Le type ne sort jamais du module et n est jamais
    /// serialise : il n existe que pour etre decode.
    main: Option<String>,
}

/// Forme d'un specifiant de module, une fois le nom du paquet isole.
///
/// C'est la premiere et la seule decision de structure de la resolution : tout
/// le reste porte sur des chemins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Specifier {
    /// Le specifiant designe un chemin : il commence par `./`, `../`, ou il
    /// est absolu. Il se resout depuis le repertoire de base.
    ///
    /// Sur Windows, `.\x` et `..\x` comptent aussi, parce que Node utilise le
    /// separateur natif pour ces deux prefixes.
    Relative(String),
    /// Le specifiant nomme un paquet. `package` est le nom de dossier dans
    /// `node_modules`, `subpath` est le reste, **precede d'une barre oblique**
    /// et donc vide quand le specifiant ne nomme que le paquet.
    ///
    /// La barre oblique est posee ici, et pas plus tard, pour que le chemin du
    /// paquet soit toujours `package + subpath`.
    Bare { package: String, subpath: String },
}

/// Nature d'un candidat, c'est-a-dire la maniere dont il est sonde.
///
/// Elle est indispensable : un dossier et un fichier ne se testent pas de la
/// meme facon, et l'ordre compte. Un fichier absent et un dossier present ne
/// doivent pas etre confondus, sinon le premier `index` d un dossier voisin
/// gagnerait contre le fichier exact demande.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `LOAD_AS_FILE` : le chemin est teste tel quel, puis avec chaque
    /// extension de [`EXTENSIONS`].
    File,
    /// `LOAD_AS_DIRECTORY` : le chemin doit etre un dossier, et on y lit
    /// `package.json`, puis `index`.
    Directory,
}

/// Un point de sondage, dans l'ordre exact ou Node le sonde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Chemin teste sur le systeme de fichiers.
    pub path: PathBuf,
    /// Comment tester ce chemin.
    pub kind: Kind,
}

/// Dit ou se trouve le fichier d un specifiant de module.
///
/// Renvoie `Some(chemin)` si la resolution aboutit, `None` si elle echoue, ce
/// qui est le resultat normal du `catch {}` vide de la source.
///
/// Les trois appelants reels du depot sont tous du meme type : le serveur
/// TypeScript (`lsp/server.ts:123` et `:1101`) cherche
/// `typescript/lib/tsserver.js`, le serveur ESLint (`:178`) cherche `eslint`,
/// et Biome (`:338`) cherche `biome`. Les quatre traitent `undefined` comme
/// `dependance absente` et continuent. Aucun des quatre ne leve.
///
/// La resolution touche le systeme de fichiers : deux appels successifs avec
/// les memes arguments peuvent differer si le disque change entre les deux,
/// exactement comme en JavaScript.
///
/// # Ce qui n est pas resolu
///
/// Le champ `exports` d un paquet n est pas lu. Si un paquet declare
/// `exports`, Node exige que le sous-chemin y figure et ignore `main` ; ici,
/// `main` est utilise. C'est la seule divergence de comportement connue, et
/// elle est signalee plutot que masquee.
pub fn resolve(id: &str, dir: &str) -> Option<String> {
    for candidate in candidates(id, dir) {
        match candidate.kind {
            Kind::File => {
                if candidate.path.is_file() {
                    return Some(candidate.path.to_string_lossy().into_owned());
                }
            }
            Kind::Directory => {
                if candidate.path.is_dir() {
                    if let Some(trouve) = resolve_directory(&candidate.path, 0) {
                        return Some(trouve);
                    }
                }
            }
        }
    }
    None
}

/// Classe un speciteur sans toucher au systeme de fichiers.
///
/// La fonction est pure : elle depend du seul `id`. Elle est publique parce
/// que c'est la decision la plus structurante de la resolution, et parce que
/// la tester isole le piege du paquet a portee.
///
/// # Portee
///
/// Un nom commence par `@` seulement s'il est suivi d'un nom de paquet :
/// `@scope/pkg/sub` donne `package: "@scope/pkg"`, alors que `@scope` seul
/// donne `package: "@scope"` avec un sous-chemin vide. Une chaine vide reste
/// un paquet de nom vide, et l'appelant verra `None` plutot qu'une panne.
pub fn specifier(id: &str) -> Specifier {
    let relatif = id.starts_with("./")
        || id.starts_with("../")
        || id == "."
        || id == ".."
        || Path::new(id).is_absolute()
        || (cfg!(windows) && (id.starts_with(".\\") || id.starts_with("..\\")));
    if relatif {
        return Specifier::Relative(id.to_string());
    }
    let package = match id.strip_prefix('@') {
        Some(reste) => {
            let mut parties = reste.split('/');
            match (parties.next(), parties.next()) {
                (Some(scope), Some(nom)) if !scope.is_empty() && !nom.is_empty() => {
                    format!("@{}/{}", scope, nom)
                }
                (Some(scope), _) if !scope.is_empty() => format!("@{}", scope),
                _ => id.to_string(),
            }
        }
        None => id.split('/').next().unwrap_or("").to_string(),
    };
    let subpath = id.get(package.len()..).unwrap_or("").to_string();
    Specifier::Bare { package, subpath }
}

/// Repertoire de depart de la resolution, une fois ancre.
///
/// C'est le `dirname` de `path.join(dir, "package.json")`, donc `dir` normalise.
/// Si le resultat n'est pas absolu, il est ancre sur le repertoire courant du
/// processus, comme le fait `path.resolve`.
///
/// Le nom `package.json` n'apparait dans aucun resultat : il sert uniquement a
/// faire porter la normalisation a `dir`.
///
/// # Normalisation
///
/// Les segments `.` sont retires et les `..` remontent d'un cran. Un `..` qui
/// n'a pas de segment normal a remonter est conserve, et un `..` au sommet
/// d'un chemin absolu est ignore, comme sur un systeme de fichiers.
pub fn base_dir(dir: &str) -> PathBuf {
    let avec_fichier = Path::new(dir).join("package.json");
    let parent = avec_fichier
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(PathBuf::new);
    let base_norm = normalise(&parent);
    if base_norm.is_absolute() {
        return base_norm;
    }
    match std::env::current_dir() {
        Ok(cwd) => normalise(&joindre(&cwd, &base_norm)),
        // Repertoire courant inaccessible : la source leverait une erreur,
        // que son `catch` avalerait, ce qui donne `undefined`.
        Err(_) => base_norm,
    }
}

/// Tous les points de sondage de la resolution, dans leur ordre.
///
/// Fonction pure : elle ne touche pas au systeme de fichiers. Pour un chemin
/// introuvable, `resolve` la parcourt en entier et rend `None` ; lister les
/// candidats permet de verifier cet ordre sans disque.
///
/// L'ordre est significatif. Pour un specifiant nu, les repertoires
/// `node_modules` sont parcourus du plus proche au plus lointain, et pour chaque
/// paquet on tente d'abord le fichier exact, puis le dossier. Le premier
/// succes gagne, donc un paquet installe au plus proche des ancetres l emporte
/// sur un homonyme plus haut.
pub fn candidates(id: &str, dir: &str) -> Vec<Candidate> {
    let base = base_dir(dir);
    match specifier(id) {
        Specifier::Relative(cible) => {
            // Un specifiant absolu ignore le repertoire de base.
            let depart = if Path::new(&cible).is_absolute() {
                normalise(Path::new(&cible))
            } else {
                joindre(&base, Path::new(&cible))
            };
            let mut liste = file_candidates(&depart);
            liste.push(Candidate {
                path: depart,
                kind: Kind::Directory,
            });
            liste
        }
        Specifier::Bare { package, subpath } => {
            let mut liste = Vec::new();
            for node_modules in node_modules_dirs(&base) {
                let chemin = joindre(&node_modules, Path::new(&format!("{}{}", package, subpath)));
                liste.extend(file_candidates(&chemin));
                liste.push(Candidate {
                    path: chemin,
                    kind: Kind::Directory,
                });
            }
            liste
        }
    }
}

/// Les dossiers `node_modules` a parcourir, du plus proche au plus lointain.
///
/// Pour chaque ancetre du repertoire de base, du plus profond vers la racine,
/// on ajoute `<ancetre>/node_modules`. Un ancetre dont le dernier segment est
/// deja `node_modules` est saute, ce qui evite un `node_modules/node_modules`
/// sans effet.
///
/// Pour une base `/p/proj/apps/web`, la liste est, dans cet ordre :
/// `/p/proj/apps/web/node_modules`, `/p/proj/apps/node_modules`,
/// `/p/proj/node_modules`, `/p/node_modules`, `/node_modules`.
///
/// La racine est donc examinee en dernier, jamais en premier : c'est elle qui
/// donne le paquet partage par tout l'arbre, et elle doit rester la moins
/// prioritaire.
fn node_modules_dirs(base: &Path) -> Vec<PathBuf> {
    let composants: Vec<Component<'_>> = base.components().collect();
    let mut dirs = Vec::new();
    for index in (0..composants.len()).rev() {
        let prefixe: PathBuf = composants[..=index].iter().copied().collect();
        if dernier_nomme(&prefixe, "node_modules") {
            continue;
        }
        dirs.push(joindre(&prefixe, Path::new("node_modules")));
    }
    dirs
}

/// Les candidats de fichier d'un chemin : tel quel, puis chaque extension.
fn file_candidates(path: &Path) -> Vec<Candidate> {
    let mut liste = Vec::with_capacity(EXTENSIONS.len() + 1);
    liste.push(Candidate {
        path: path.to_path_buf(),
        kind: Kind::File,
    });
    for extension in EXTENSIONS {
        let mut nom = path.as_os_str().to_os_string();
        nom.push(extension);
        liste.push(Candidate {
            path: PathBuf::from(nom),
            kind: Kind::File,
        });
    }
    liste
}

/// Resout un dossier : `package.json` puis `main`, puis `index`.
///
/// L'ordre est celui de Node : si `main` est present et pointe un fichier, il
/// gagne sur `index`, et le contenu de `main` n'est jamais combine a `index`.
/// Si `main` pointe un dossier, ce dossier est resout a son tour, mais avec la
/// meme borne de recursion que partout ailleurs.
///
/// Un `package.json` illisible ou invalide est traite comme absent : la source
/// emettrait `ERR_INVALID_PACKAGE_JSON`, que le `catch {}` de l appelant
/// convertirait en `undefined`, donc en `None`. Le resultat observable est le
/// meme.
fn resolve_directory(dir: &Path, depth: usize) -> Option<String> {
    if depth >= PROFONDEUR_MAX {
        return None;
    }
    if let Some(main) = read_main(dir) {
            let chemin = joindre(dir, Path::new(&main));
        for candidat in file_candidates(&chemin) {
            if candidat.path.is_file() {
                return Some(candidat.path.to_string_lossy().into_owned());
            }
        }
        if chemin.is_dir() {
            if let Some(trouve) = resolve_directory(&chemin, depth + 1) {
                return Some(trouve);
            }
        }
    }
    let index = joindre(dir, Path::new("index"));
    for candidat in file_candidates(&index) {
        if candidat.path.is_file() {
            return Some(candidat.path.to_string_lossy().into_owned());
        }
    }
    // Node termine `LOAD_AS_DIRECTORY` par `LOAD_AS_FILE(index) ||
    // LOAD_AS_DIRECTORY(index)` : un dossier nomme `index` est donc explore a
    // son tour, ce qui reste borne par la profondeur.
    if index.is_dir() {
        return resolve_directory(&index, depth + 1);
    }
    None
}

/// Lit `main` d'un `package.json`, en ignorant un `main` vide.
///
/// Node considere `main: ""` comme absent, et un `main` qui n'est pas une
/// chaine comme absent. Le deuxieme cas est traite ici par l'echec du
/// decodage, puisque le champ est un `Option<String>`.
fn read_main(dir: &Path) -> Option<String> {
    let brut = fs::read_to_string(dir.join("package.json")).ok()?;
    let manifeste: Manifeste = serde_json::from_str(&brut).ok()?;
    manifeste.main.filter(|main| !main.is_empty())
}

/// Concatene puis normalise, en laissant un chemin absolu ecraser la base.
fn joindre(base: &Path, cible: &Path) -> PathBuf {
    if cible.is_absolute() {
        return normalise(cible);
    }
    let mut sortie = base.to_path_buf();
    for composant in cible.components() {
        sortie.push(composant.as_os_str());
    }
    normalise(&sortie)
}

/// Normalise un chemin : `.` retire, `..` remonte, separateurs reduits.
///
/// Le resultat est vide seulement pour un chemin vide, auquel cas `"."` est
/// rendu, comme `path.join(".")` cote Node.
fn normalise(path: &Path) -> PathBuf {
    let mut sortie = PathBuf::new();
    for composant in path.components() {
        match composant {
            Component::CurDir => {}
            Component::ParentDir => {
                if dernier_normal(&sortie) {
                    sortie.pop();
                } else if !a_racine(&sortie) {
                    sortie.push("..");
                }
            }
            autre => sortie.push(autre.as_os_str()),
        }
    }
    if sortie.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        sortie
    }
}

/// Le dernier segment est-il un segment nomme ?
fn dernier_normal(path: &Path) -> bool {
    matches!(path.components().last(), Some(Component::Normal(_)))
}

/// Le chemin porte-t-il une racine, Windows ou POSIX ?
fn a_racine(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::RootDir) | Some(Component::Prefix(_))
    )
}

/// Le dernier segment porte-t-il exactement ce nom ?
fn dernier_nomme(path: &Path, nom: &str) -> bool {
    matches!(
        path.components().last(),
        Some(Component::Normal(dernier)) if dernier == OsStr::new(nom)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Compteur de suffixe, pour que deux tests n obtiennent pas le meme
    /// repertoire temporaire.
    static COMPTEUR: AtomicUsize = AtomicUsize::new(0);

    /// Cree un repertoire temporaire propre et le detruit a la fin du test.
    ///
    /// Le nom est unique par processus et par appel, donc deux executions
    /// paralleles ne se genent pas. Le `remove_dir_all` final est une
    /// opportunite de nettoyage, pas une garantie : si un test echoue, son
    /// repertoire reste sur le disque, ce qui est preferable a un `panic`
    /// supplementaire.
    struct Tmp(PathBuf);

    impl Tmp {
        fn nouveau(nom: &str) -> Self {
            let suffixe = COMPTEUR.fetch_add(1, Ordering::SeqCst);
            let racine = std::env::temp_dir().join(format!(
                "ycode-util-module-{}-{}-{}",
                std::process::id(),
                nom,
                suffixe
            ));
            let _ = fs::remove_dir_all(&racine);
            fs::create_dir_all(&racine).expect("le repertoire temporaire doit etre creatable");
            Tmp(racine)
        }

        /// Cree un fichier, avec ses dossiers parents.
        fn fichier(&self, relatif: &str, contenu: &str) -> PathBuf {
            let chemin = self.0.join(relatif);
            fs::create_dir_all(chemin.parent().expect("un fichier a toujours un parent"))
                .expect("creation des dossiers parents");
            fs::write(&chemin, contenu).expect("ecriture du fichier");
            chemin
        }

        fn json(&self, relatif: &str, contenu: &str) -> PathBuf {
            self.fichier(relatif, contenu)
        }

        fn chemin(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    // --- Partie pure : aucune de ces fonctions ne touche le disque ---

    #[test]
    fn a_bare_specifier_yields_the_package_and_the_subpath() {
        // Le cas des trois appelants du depot. Le sous-chemin porte la barre
        // oblique, donc le chemin du paquet est toujours `package + subpath`.
        assert_eq!(
            specifier("typescript/lib/tsserver.js"),
            Specifier::Bare {
                package: "typescript".to_string(),
                subpath: "/lib/tsserver.js".to_string(),
            }
        );
        assert_eq!(
            specifier("eslint"),
            Specifier::Bare {
                package: "eslint".to_string(),
                subpath: String::new(),
            }
        );
    }

    #[test]
    fn a_scoped_package_keeps_its_full_name() {
        // Piege : `@scope/pkg/sub` ne doit pas donner `@scope` comme paquet.
        // `node_modules/@scope/pkg` est un seul dossier, donc couper trop tot
        // chercherait un dossier inexistant et ne trouverait jamais rien.
        assert_eq!(
            specifier("@scope/pkg/sub"),
            Specifier::Bare {
                package: "@scope/pkg".to_string(),
                subpath: "/sub".to_string(),
            }
        );
        assert_eq!(
            specifier("@scope/pkg"),
            Specifier::Bare {
                package: "@scope/pkg".to_string(),
                subpath: String::new(),
            }
        );
        assert_eq!(
            specifier("@scope"),
            Specifier::Bare {
                package: "@scope".to_string(),
                subpath: String::new(),
            }
        );
    }

    #[test]
    fn a_relative_or_absolute_specifier_is_not_a_package() {
        for relatif in ["./x", "../x", ".", "..", "/x/y"] {
            assert_eq!(
                specifier(relatif),
                Specifier::Relative(relatif.to_string()),
                "{:?} doit rester un chemin",
                relatif
            );
        }
        // Seule la premiere lettre compte : `a/./b` et `a/../b` sont des noms
        // de paquets pour Node, dont le sous-chemin contient les segments. Les
        // qualifier de relatifs ici rendrait la resolution fausse.
        for paquet in ["a/./b", "a/../b", "@scope/pkg/../x"] {
            assert!(
                matches!(specifier(paquet), Specifier::Bare { .. }),
                "{:?} doit rester un paquet",
                paquet
            );
        }
        // Les separateurs Windows ne comptent qu'a cote Windows : sur Linux,
        // `.\\x` est un nom de paquet, et c'est bien ce que Node en fait.
        let windows = Specifier::Relative(".\\x".to_string());
        if cfg!(windows) {
            assert_eq!(specifier(".\\x"), windows);
            assert_eq!(specifier("..\\x"), Specifier::Relative("..\\x".to_string()));
        } else {
            assert_ne!(specifier(".\\x"), windows);
        }
        // Un chemin Windows avec une lettre de lecteur est absolu sur Windows
        // et ne l est pas sur Linux, ou Node le traite aussi comme un nom de
        // paquet. Aucune des deux lectures ne doit paniquer.
        let lecteur = Path::new("C:\\x").is_absolute();
        if cfg!(windows) {
            assert_eq!(specifier("C:\\x"), Specifier::Relative("C:\\x".to_string()));
        } else {
            assert_eq!(
                specifier("C:\\x"),
                Specifier::Bare {
                    package: "C:\\x".to_string(),
                    subpath: String::new(),
                },
                "sur Linux, C:\\x n est pas absolu"
            );
        }
        assert!(lecteur == cfg!(windows));
    }

    #[test]
    fn an_empty_or_near_empty_string_does_not_panic() {
        // Hors contrat, mais la source ne leve pas non plus : elle rend
        // `undefined`. Aucune de ces entrees ne doit paniquer ni produire un
        // chemin qui ressemble a un resultat.
        assert_eq!(
            specifier(""),
            Specifier::Bare {
                package: String::new(),
                subpath: String::new(),
            }
        );
        assert_eq!(
            specifier("/"),
            Specifier::Relative("/".to_string()),
            "la racine seule reste un chemin"
        );
        // `//serveur/partage` n est volontairement pas teste ici : la lecture
        // d une racine UNC depend du parseur de prefixe de chaque plateforme,
        // et le porter demanderait de le comparer a Node, pas de le deviner.
    }

    #[test]
    fn node_modules_directories_are_listed_from_the_deepest_ancestor_to_the_root() {
        // L'ordre porte tout le sens de la resolution : le paquet le plus
        // proche gagne. La racine est examinee en dernier.
        let base = PathBuf::from(if cfg!(windows) {
            r"C:\p\proj\apps\web"
        } else {
            "/p/proj/apps/web"
        });
        let dirs: Vec<String> = node_modules_dirs(&base)
            .iter()
            .map(|d| d.to_string_lossy().into_owned())
            .collect();
        let attendu = if cfg!(windows) {
            vec![
                r"C:\p\proj\apps\web\node_modules",
                r"C:\p\proj\apps\node_modules",
                r"C:\p\proj\node_modules",
                r"C:\p\node_modules",
                r"C:\node_modules",
            ]
        } else {
            vec![
                "/p/proj/apps/web/node_modules",
                "/p/proj/apps/node_modules",
                "/p/proj/node_modules",
                "/p/node_modules",
                "/node_modules",
            ]
        };
        assert_eq!(dirs, attendu);
    }

    #[test]
    fn an_ancestor_named_node_modules_produces_no_duplicate() {
        // `node_modules/node_modules` ne sert a rien et Node ne le teste pas.
        let base = PathBuf::from(if cfg!(windows) {
            r"C:\p\node_modules\pkg"
        } else {
            "/p/node_modules/pkg"
        });
        for dir in node_modules_dirs(&base) {
            let fin = dir
                .file_name()
                .map(|n| n == OsStr::new("node_modules"))
                .unwrap_or(false);
            assert!(fin, "chaque candidat doit finir par node_modules");
            let parent = dir.parent().map(Path::to_path_buf);
            if let Some(parent) = parent {
                assert!(
                    !dernier_nomme(&parent, "node_modules"),
                    "{} ne doit pas contenir node_modules/node_modules",
                    dir.display()
                );
            }
        }
    }

    #[test]
    fn base_dir_normalizes_dir_and_drops_the_package_json_name() {
        // `path.join(dir, "package.json")` puis son `dirname` : le nom de
        // fichier disparait, mais la normalisation, elle, reste.
        //
        // L'entree est absolue dans les deux plateformes, avec une lettre de
        // lecteur sur Windows : `/a/b/../c` n'y est pas absolue, donc
        // `base_dir` l'ancrerait sur le repertoire courant et le test ne
        // testerait plus la normalisation du tout.
        for (entree, attendu) in [
            (r"C:\a\b\..\c", r"C:\a\c"),
            (r"C:\a\\b\", r"C:\a\b"),
            (r"C:\a\.\b", r"C:\a\b"),
        ] {
            if cfg!(windows) {
                assert_eq!(base_dir(entree), PathBuf::from(attendu));
            }
        }
        for (entree, attendu) in [("/a/b/../c", "/a/c"), ("/a//b/", "/a/b"), ("/a/./b", "/a/b")] {
            if !cfg!(windows) {
                assert_eq!(base_dir(entree), PathBuf::from(attendu));
            }
        }
        // Le nom du fichier d'ancrage ne doit apparaitre ni dans le resultat,
        // ni comme segment, quelle que soit la plateforme.
        let ancrage = base_dir(if cfg!(windows) { r"C:\a\b" } else { "/a/b" });
        let sortie = ancrage.to_string_lossy();
        assert!(!sortie.contains("package.json"), "{}", sortie);
        assert!(
            !sortie
                .split(|c| c == '\\' || c == '/')
                .any(|segment| segment == "package.json"),
            "{}",
            sortie
        );
    }

    #[test]
    fn an_empty_or_dot_base_directory_equals_the_current_directory() {
        // `path.join("", "package.json")` vaut `"package.json"`, dont le
        // `dirname` vaut `""`, que `path.resolve` ancre sur le cwd. Les deux
        // ecritures doivent donc donner le meme depart, et ce depart est le
        // repertoire courant, jamais la chaine vide.
        let cwd = std::env::current_dir().expect("repertoire courant lisible");
        assert_eq!(base_dir(""), normalise(&cwd));
        assert_eq!(base_dir("."), normalise(&cwd));
        assert_eq!(base_dir("./"), normalise(&cwd));
        // Un `..` qui n'a pas de segment normal a remonter est conserve : il
        // n'est pas perdu en route, il sort du dossier de depart.
        assert_eq!(base_dir("../x"), normalise(&joindre(&cwd, Path::new("../x"))));
    }

    #[test]
    fn extensions_are_tried_in_node_order_and_the_directory_comes_last() {
        // L'ordre est significant quand deux fichiers coexistent : un
        // dossier `x.js` ne doit pas gagner contre un fichier `x`.
        let base = if cfg!(windows) { r"C:\base" } else { "/base" };
        let liste: Vec<String> = candidates("./x", base)
            .iter()
            .map(|c| c.path.to_string_lossy().into_owned())
            .collect();
        let attendu = if cfg!(windows) {
            vec![
                r"C:\base\x",
                r"C:\base\x.js",
                r"C:\base\x.json",
                r"C:\base\x.node",
                r"C:\base\x",
            ]
        } else {
            vec![
                "/base/x",
                "/base/x.js",
                "/base/x.json",
                "/base/x.node",
                "/base/x",
            ]
        };
        assert_eq!(liste, attendu);
        // Le dernier candidat est le dossier, tous les autres des fichiers.
        let kinds: Vec<Kind> = candidates("./x", base).iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            vec![
                Kind::File,
                Kind::File,
                Kind::File,
                Kind::File,
                Kind::Directory
            ]
        );
    }

    #[test]
    fn an_absolute_specifier_ignores_the_base_directory() {
        // Node resout un chemin absolu sans passer par l ancrage.
        let absolu = if cfg!(windows) { r"C:\abs\y" } else { "/abs/y" };
        let premier = &candidates(absolu, if cfg!(windows) {
            r"C:\base"
        } else {
            "/base"
        })[0];
        assert_eq!(premier.path, PathBuf::from(absolu));
        assert_eq!(premier.kind, Kind::File);
    }

    // --- Partie disque : les quatre cas du fichier de tests du depot ---

    #[test]
    fn resolves_a_package_subpath() {
        // `packages/opencode/test/util/module.test.ts:15`.
        let tmp = Tmp::nouveau("subpath");
        let root = tmp.chemin().join("proj");
        let file = tmp.fichier("proj/node_modules/typescript/lib/tsserver.js", "export {}\n");
        tmp.json(
            "proj/node_modules/typescript/package.json",
            r#"{"name":"typescript"}"#,
        );
        assert_eq!(resolve("typescript/lib/tsserver.js", &root.to_string_lossy()), Some(
            file.to_string_lossy().into_owned()
        ));
    }

    #[test]
    fn resolves_a_package_from_an_ancestor_node_modules() {
        // `packages/opencode/test/util/module.test.ts:30`. Le paquet est pose
        // a la racine, la resolution part de `root/apps/web` : c'est le seul
        // test qui verifie la remonte des ancetres.
        let tmp = Tmp::nouveau("ancetre");
        let root = tmp.chemin().join("proj");
        let cwd = root.join("apps/web");
        fs::create_dir_all(&cwd).expect("creation du repertoire de depart");
        let file = tmp.fichier("proj/node_modules/eslint/lib/api.js", "export {}\n");
        tmp.json(
            "proj/node_modules/eslint/package.json",
            r#"{"name":"eslint","main":"lib/api.js"}"#,
        );
        assert_eq!(
            resolve("eslint", &cwd.to_string_lossy()),
            Some(file.to_string_lossy().into_owned())
        );
    }

    #[test]
    fn two_independent_base_directories_resolve_the_same_package_separately() {
        // `packages/opencode/test/module.test.ts:50` a `:52`. Deux arbres
        // distincts portant le meme paquet doivent rester distincts : c'est la
        // preuve que la resolution est relative au `dir` fourni et non au
        // repertoire courant du processus.
        let tmp = Tmp::nouveau("independants");
        let a = tmp.chemin().join("a");
        let b = tmp.chemin().join("b");
        let gauche = tmp.fichier("a/node_modules/biome/index.js", "export {}\n");
        let droite = tmp.fichier("b/node_modules/biome/index.js", "export {}\n");
        let manifeste = r#"{"name":"biome","main":"index.js"}"#;
        tmp.json("a/node_modules/biome/package.json", manifeste);
        tmp.json("b/node_modules/biome/package.json", manifeste);

        let gauche_obtenu = resolve("biome", &a.to_string_lossy());
        let droite_obtenu = resolve("biome", &b.to_string_lossy());
        assert_eq!(gauche_obtenu, Some(gauche.to_string_lossy().into_owned()));
        assert_eq!(droite_obtenu, Some(droite.to_string_lossy().into_owned()));
        assert_ne!(gauche_obtenu, droite_obtenu);
    }

    #[test]
    fn a_missing_package_returns_none() {
        // `packages/opencode/test/util/module.test.ts:57`. Le `catch {}` vide
        // de la source rend `undefined`, donc `None` ici. Un nom peu commun
        // evite de tomber par hasard sur un paquet reellement installe plus
        // haut dans l arborescence temporaire.
        let tmp = Tmp::nouveau("absent");
        assert_eq!(
            resolve("ycode-paquet-absent-9f3a", &tmp.chemin().to_string_lossy()),
            None
        );
    }

    #[test]
    fn a_failed_resolution_never_returns_an_empty_string() {
        // Le contrat le plus fragile du fichier. `Some("")` serait
        // indiscernable d une resolution qui aurait reussi sur un chemin vide,
        // et un `Err` ferait croire que l'appelant doit lever.
        let tmp = Tmp::nouveau("echec");
        for requete in [
            "",
            "ycode-paquet-absent-9f3a",
            "./absent",
            "../absent",
            "ycode-paquet-absent-9f3a/sous/chemin",
        ] {
            match resolve(requete, &tmp.chemin().to_string_lossy()) {
                None => {}
                Some(chemin) => assert!(!chemin.is_empty(), "{:?} a rendu un vide", requete),
            }
        }
        // Hors contrat aussi : la resolution ne doit jamais paniquer.
        assert_eq!(resolve("", ""), None);
        assert_eq!(resolve(".", ""), None);
    }

    // --- Ordre interne du dossier : main, puis index ---

    #[test]
    fn main_wins_over_index() {
        let tmp = Tmp::nouveau("main");
        let main = tmp.fichier("p/node_modules/pkg/lib/api.js", "x");
        tmp.json(
            "p/node_modules/pkg/package.json",
            r#"{"name":"pkg","main":"lib/api.js"}"#,
        );
        tmp.fichier("p/node_modules/pkg/index.js", "y");
        assert_eq!(
            resolve("pkg", &tmp.chemin().join("p").to_string_lossy()),
            Some(main.to_string_lossy().into_owned())
        );
    }

    #[test]
    fn index_is_used_when_main_is_missing_or_does_not_resolve() {
        // Trois cas distincts de `LOAD_AS_DIRECTORY`, qui doivent tous finir
        // sur `index` : pas de manifeste, `main` fantome, `main` vide.
        let tmp = Tmp::nouveau("index");
        // Le cas sans `main` : seul `index.js` existe. Il faut bien le nom
        // `index`, car le dossier se resout par `<dossier>/index` et jamais par
        // le dossier lui-meme.
        let index = tmp.fichier("p/node_modules/sans/index.js", "x");
        let fantome = tmp.fichier("p/node_modules/fantome/index.js", "x");
        let vide = tmp.fichier("p/node_modules/vide/index.js", "x");
        tmp.json("p/node_modules/sans/package.json", r#"{"name":"sans"}"#);
        tmp.json(
            "p/node_modules/fantome/package.json",
            r#"{"name":"fantome","main":"absent.js"}"#,
        );
        tmp.json(
            "p/node_modules/vide/package.json",
            r#"{"name":"vide","main":""}"#,
        );
        let p = tmp.chemin().join("p").to_string_lossy().into_owned();
        assert_eq!(resolve("sans", &p), Some(index.to_string_lossy().into_owned()));
        assert_eq!(
            resolve("fantome", &p),
            Some(fantome.to_string_lossy().into_owned())
        );
        assert_eq!(resolve("vide", &p), Some(vide.to_string_lossy().into_owned()));
    }

    #[test]
    fn an_invalid_manifest_is_treated_as_absent() {
        // Un `package.json` illisible ne doit pas faire paniquer, et le
        // resultat doit rester le meme que sans manifeste : c'est ce que
        // produit le `catch {}` de la source.
        let tmp = Tmp::nouveau("manifeste-invalide");
        let index = tmp.fichier("p/node_modules/pkg/index.js", "x");
        tmp.fichier("p/node_modules/pkg/package.json", "{ ceci n est pas du json");
        let p = tmp.chemin().join("p").to_string_lossy().into_owned();
        assert_eq!(
            resolve("pkg", &p),
            Some(index.to_string_lossy().into_owned())
        );
    }

    #[test]
    fn an_empty_subpath_never_yields_the_package_directory_itself() {
        // `pkg` et `pkg/` doivent produire la meme liste de candidats : le
        // dossier du paquet n est un candidat valide que par `main` ou
        // `index`, jamais directement.
        let base = if cfg!(windows) { r"C:\base" } else { "/base" };
        assert_eq!(candidates("pkg", base), candidates("pkg/", base));
    }

    #[test]
    fn resolution_is_independent_of_the_current_directory() {
        // Le point faible des tests disque : on ne peut pas changer le cwd
        // sans perturber les autres tests. Ce test verifie donc l'inverse,
        // que tout se joue sur le `dir` fourni.
        let tmp = Tmp::nouveau("cwd");
        let file = tmp.fichier("p/node_modules/pkg/index.js", "x");
        let p = tmp.chemin().join("p").to_string_lossy().into_owned();
        let attendu = Some(file.to_string_lossy().into_owned());
        assert_eq!(resolve("pkg", &p), attendu);
        assert_eq!(resolve("pkg", &p), attendu, "deux appels, meme reponse");
    }
}
