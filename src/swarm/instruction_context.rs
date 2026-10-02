//! Portage Rust de `opencode/packages/core/src/instruction-context.ts`.
//!
//! La source tient en quatre-vingt-douze lignes et declare un noeud de
//! contexte systeme (`makeLocationNode`) nomme `"instruction-context"` : a
//! chaque session il decouvre les fichiers `AGENTS.md` ambiants (un global
//! sous `global.config`, puis ceux remotes entre le repertoire de travail et
//! la racine du projet), les lit, et publie leur contenu rendu sous la cle
//! `"core/instructions"`.
//!
//! Tout le câblage `Effect` (`FSUtil`, `Global`, `Location`,
//! `SystemContextRegistry`, `Layer.effectDiscard`) n'a pas d'equivalent dans
//! ce crate : `Cargo.toml` ne declare pas `Effect` et ce fichier n'ouvre aucun
//! fichier, ne touche ni l'horloge ni le disque. Il porte ce qui se decide
//! sans effet : la forme du fichier, les trois gabarits de message, le
//! predicat d'appartenance au projet, la deduplication des chemins et la
//! classification du resultat d'observation.

use serde::{Deserialize, Serialize};

/// La cle du contexte publie, `SystemContext.Key.make("core/instructions")`.
pub const CONTEXT_KEY: &str = "core/instructions";

/// Le nom du noeud, premier argument de `makeLocationNode`.
pub const NODE_NAME: &str = "instruction-context";

/// Le seul nom de fichier decouvert par la montee `fs.up`.
pub const DISCOVERED_FILENAME: &str = "AGENTS.md";

/// Les dependances du noeud, champ `deps` de `makeLocationNode`, dans l'ordre
/// de la source.
pub const NODE_DEPS: [&str; 4] = ["FSUtil", "Global", "Location", "SystemContextRegistry"];

/// Le message qui accompagne une mise a jour : les nouvelles instructions
/// remplacent toutes celles chargees avant.
pub const UPDATE_PREFIX: &str =
    "These instructions replace all previously loaded ambient instructions.\n\n";

/// Le message publie quand les instructions precedemment chargees ne
/// s'appliquent plus.
pub const REMOVED_MESSAGE: &str = "Previously loaded instructions no longer apply.";

/// Un fichier d'instructions ambiantes.
///
/// Portage de la classe `File` (`path: AbsolutePath`, `content: string`).
 /// `AbsolutePath` est un `Schema.String` : a l'execution c'est une chaine,
 /// d'ou le `String`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstructionFile {
    /// Chemin absolu du fichier, tel que `fs.resolve` l'a rendu.
    #[serde(rename = "path")]
    pub path: String,
    /// Contenu brut du fichier.
    #[serde(rename = "content")]
    pub content: String,
}

impl InstructionFile {
    /// Construit un fichier d'instructions.
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self { path: path.into(), content: content.into() }
    }
}

/// Le rendu d'une liste de fichiers, fonction `render` de la source.
///
/// Chaque fichier donne `"Instructions from: {path}\n{content}"`, les blocs
/// sont joints par `"\n\n"`. Une liste vide rend `""`.
pub fn render(files: &[InstructionFile]) -> String {
    files
        .iter()
        .map(|file| format!("Instructions from: {}\n{}", file.path, file.content))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Le message de base publie au chargement : c'est exactement `render`.
pub fn baseline_message(files: &[InstructionFile]) -> String {
    render(files)
}

/// Le message publie a la mise a jour : le prefixe de remplacement suivi du
/// rendu des nouveaux fichiers.
pub fn update_message(files: &[InstructionFile]) -> String {
    format!("{UPDATE_PREFIX}{}", render(files))
}

/// Le repertoire de travail est-il a l'interieur du projet ?
///
/// Portage du predicat calcule dans `observe` :
/// `relative(stop, start)` est vide (meme repertoire), ou bien il ne vaut pas
/// `".."` , ne commence pas par `"..<sep>"` et n'est pas absolu. `relative`
/// ne rend jamais un chemin absolu en pratique, mais la source teste quand
/// meme `isAbsolute`, donc le predicat garde ce troisieme parametre au lieu
/// de l'omettre.
pub fn is_inside_project(from_project: &str, is_absolute: bool, sep: char) -> bool {
    if from_project.is_empty() {
        return true;
    }
    if from_project == ".." {
        return false;
    }
    let mut parent_prefix = String::from("..");
    parent_prefix.push(sep);
    if from_project.starts_with(&parent_prefix) {
        return false;
    }
    !is_absolute
}

/// Dedupplique des chemins en preservant l'ordre de premiere apparition.
///
/// Portage de `Array.dedupe` applique a `[global, ...discovered]` : le chemin
/// global garde sa place en tete, chaque decouvert n'apparait qu'une fois.
pub fn dedupe_paths<'a>(paths: &[&'a str]) -> Vec<&'a str> {
    let mut seen = std::collections::BTreeSet::new();
    let mut kept = Vec::new();
    for path in paths {
        if seen.insert(*path) {
            kept.push(*path);
        }
    }
    kept
}

/// Le resultat de l'observation, avant enregistrement au registre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveOutcome {
    /// Au moins un fichier decouvert est illisible : la source rend
    /// `SystemContext.unavailable`, et le registre publie `source(unavailable)`.
    Unavailable,
    /// Rien a publier : le registre publie `SystemContext.empty`.
    Empty,
    /// Les fichiers lisibles, dans l'ordre des chemins.
    Files(Vec<InstructionFile>),
}

/// Classe le resultat de la lecture des chemins.
///
/// `paths` est la liste dedupliquee, `discovered` l'ensemble des chemins
/// remontes par `fs.up` (resolus), et `contents` le contenu lu pour chaque
/// chemin (`None` quand `readFileStringSafe` rend `undefined`).
///
/// La regle est celle de la source : si un contenu manque **et** que son
/// chemin appartient aux decouverts, tout est indisponible ; sinon les
/// contenus manquants sont filtres et ce qui reste decide entre `Empty` et
/// `Files`. Un chemin global illisible ne rend donc jamais l'ensemble
/// indisponible, seul un decouvert le fait.
pub fn classify_observation(
    paths: &[String],
    discovered: &std::collections::BTreeSet<String>,
    contents: &[Option<String>],
) -> ObserveOutcome {
    debug_assert_eq!(paths.len(), contents.len());
    let mut files = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        match contents.get(index).and_then(|content| content.as_ref()) {
            Some(content) => files.push(InstructionFile::new(path.clone(), content.clone())),
            None => {
                if discovered.contains(path) {
                    return ObserveOutcome::Unavailable;
                }
            }
        }
    }
    if files.is_empty() {
        ObserveOutcome::Empty
    } else {
        ObserveOutcome::Files(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn fichier(chemin: &str, contenu: &str) -> InstructionFile {
        InstructionFile::new(chemin.to_string(), contenu.to_string())
    }

    fn ensemble(chemins: &[&str]) -> BTreeSet<String> {
        chemins.iter().map(|chemin| chemin.to_string()).collect()
    }

    #[test]
    fn la_cle_et_le_noeud_reprennent_les_constantes_de_la_source() {
        assert_eq!(CONTEXT_KEY, "core/instructions");
        assert_eq!(NODE_NAME, "instruction-context");
        assert_eq!(DISCOVERED_FILENAME, "AGENTS.md");
        assert_eq!(NODE_DEPS, ["FSUtil", "Global", "Location", "SystemContextRegistry"]);
    }

    #[test]
    fn le_rendu_d_un_fichier_suit_le_gabarit_de_la_source() {
        let rendu = render(&[fichier("/projet/AGENTS.md", "Ecrire des tests.")]);
        assert_eq!(rendu, "Instructions from: /projet/AGENTS.md\nEcrire des tests.");
    }

    #[test]
    fn le_rendu_joint_les_fichiers_par_une_ligne_vide() {
        let rendu = render(&[fichier("/a/AGENTS.md", "Un."), fichier("/b/AGENTS.md", "Deux.")]);
        assert_eq!(
            rendu,
            "Instructions from: /a/AGENTS.md\nUn.\n\nInstructions from: /b/AGENTS.md\nDeux."
        );
    }

    #[test]
    fn le_rendu_d_une_liste_vide_est_vide() {
        assert_eq!(render(&[]), "");
        assert_eq!(baseline_message(&[]), "");
    }

    #[test]
    fn la_mise_a_jour_prefixe_le_rendu_par_le_remplacement() {
        let fichiers = vec![fichier("/a/AGENTS.md", "Un.")];
        let message = update_message(&fichiers);
        assert!(message.starts_with(UPDATE_PREFIX));
        assert!(message.ends_with(&render(&fichiers)));
        assert_eq!(REMOVED_MESSAGE, "Previously loaded instructions no longer apply.");
    }

    #[test]
    fn le_meme_repertoire_est_dans_le_projet() {
        assert!(is_inside_project("", false, '/'));
    }

    #[test]
    fn le_parent_et_ses_chemins_sortent_du_projet() {
        assert!(!is_inside_project("..", false, '/'));
        assert!(!is_inside_project("../autre", false, '/'));
        assert!(!is_inside_project("..\\autre", false, '\\'));
    }

    #[test]
    fn un_sous_repertoire_reste_dans_le_projet() {
        assert!(is_inside_project("paquet", false, '/'));
        assert!(is_inside_project("paquet/sous", false, '/'));
        assert!(!is_inside_project("paquet", true, '/'), "absolu reste exclu");
    }

    #[test]
    fn un_nom_qui_commence_par_deux_points_n_est_pas_un_parent() {
        assert!(is_inside_project("..paquet", false, '/'));
    }

    #[test]
    fn la_deduplication_garde_le_global_en_tete_une_seule_fois() {
        let chemins = vec!["/global/AGENTS.md", "/projet/AGENTS.md", "/global/AGENTS.md"];
        assert_eq!(dedupe_paths(&chemins), vec!["/global/AGENTS.md", "/projet/AGENTS.md"]);
    }

    #[test]
    fn un_decouvert_illisible_rend_tout_indisponible() {
        let chemins = vec!["/global/AGENTS.md".to_string(), "/projet/AGENTS.md".to_string()];
        let decouverts = ensemble(&["/projet/AGENTS.md"]);
        let contenus = vec![Some("Global.".to_string()), None];
        assert_eq!(classify_observation(&chemins, &decouverts, &contenus), ObserveOutcome::Unavailable);
    }

    #[test]
    fn un_global_illisible_est_filtre_sans_tout_invalider() {
        let chemins = vec!["/global/AGENTS.md".to_string(), "/projet/AGENTS.md".to_string()];
        let decouverts = ensemble(&["/projet/AGENTS.md"]);
        let contenus = vec![None, Some("Projet.".to_string())];
        assert_eq!(
            classify_observation(&chemins, &decouverts, &contenus),
            ObserveOutcome::Files(vec![fichier("/projet/AGENTS.md", "Projet.")])
        );
    }

    #[test]
    fn sans_fichier_lisible_l_observation_est_vide() {
        let chemins = vec!["/global/AGENTS.md".to_string()];
        let decouverts = ensemble(&[]);
        let contenus = vec![None];
        assert_eq!(classify_observation(&chemins, &decouverts, &contenus), ObserveOutcome::Empty);
        assert_eq!(
            classify_observation(&[], &ensemble(&[]), &[]),
            ObserveOutcome::Empty
        );
    }

    #[test]
    fn un_fichier_serialise_garde_ses_deux_champs() {
        let json = serde_json::to_value(fichier("/a/AGENTS.md", "Un.")).unwrap();
        assert_eq!(json["path"], "/a/AGENTS.md");
        assert_eq!(json["content"], "Un.");
        let relu: InstructionFile = serde_json::from_value(json).unwrap();
        assert_eq!(relu, fichier("/a/AGENTS.md", "Un."));
    }
}
