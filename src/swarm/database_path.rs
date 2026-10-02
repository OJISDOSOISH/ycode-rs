//! Portage de `packages/core/src/database/path.ts`.
//!
//! La source fait 91 lignes et ne contient aucune classe : quatre fonctions
//! pures de normalisation de chemins, puis quatre descripteurs de colonnes
//! Drizzle (`customType`) qui n'en sont que l'application a l'entree et a
//! la sortie du pilote. Chaque colonne declare `dataType() { return "text" }`
//! et une paire `toDriver` / `fromDriver`.
//!
//! `Cargo.toml` ne declare ni `drizzle-orm`, ni `effect` : les descripteurs
//! `customType` n'ont donc pas d'equivalent executable ici. Ce portage en
//! retient ce qui est testable sans pilote : les quatre fonctions, et pour
//! chaque colonne ses deux conversions, sous forme de fonctions pures.
//!
//! La plateforme est un parametre (`is_windows`) plutot qu'un branchement
//! sur `process.platform` : la source lit la plateforme d'execution, ce qui
//! rendrait les deux moities du comportement inaccessibles dans un meme
//! test. Les enveloppes `*_for_current_platform` recablent `cfg!(windows)`
//! pour l'appelant qui veut le comportement de la machine courante.
//!
//! Correspondance exacte avec la source :
//!
//! - `storagePath` : hors Windows, identite ; sur Windows, chaque `\`
//!   devient `/`.
//! - `isWindowsStoragePath` : `/^[A-Za-z]:\//` ou commence par `//`
//!   (chemin UNC).
//! - `absolute` : normalise par `storagePath`, puis exige un absolu posix
//!   (`nodePath.posix.isAbsolute`, soit commencer par `/`) ou, sur Windows
//!   seulement, un chemin de stockage Windows. Sinon leve
//!   `Path is not absolute: <entree d'origine>`.
//! - `toPlatform` : hors Windows, ou entree non Windows, identite ; sinon
//!   chaque `/` devient `\`.
//! - `absoluteColumn` : `toDriver` applique `absolute`, `fromDriver`
//!   applique `toPlatform(absolute(entree))`. `AbsolutePath` est une marque
//!   `Schema.brand` sans effet a l'execution : a ce niveau c'est une chaine.
//! - `directoryColumn` : comme `absoluteColumn`, mais la chaine vide
//!   traverse dans les deux sens sans validation (les sessions historiques
//!   peuvent persister un repertoire vide).
//! - `pathColumn` : `storagePath` dans les deux sens, sans validation.
//! - `absoluteArrayColumn` : `toDriver` serialise en JSON le tableau
//!   normalise par `absolute`, `fromDriver` parse le JSON puis applique la
//!   conversion de `absoluteColumn` a chaque element.

/// Type de stockage declare par les quatre colonnes : du texte.
pub const DATA_TYPE: &str = "text";

/// Dit si un chemin est deja sous forme de stockage Windows.
///
/// C'est la transcription de `/^[A-Za-z]:\//.test(input) ||
/// input.startsWith("//")` : une lettre de lecteur suivie de `:/` en tete,
/// ou un chemin UNC en `//`.
pub fn is_windows_storage_path(input: &str) -> bool {
    let bytes = input.as_bytes();
    let lecteur = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes[2] == b'/';
    lecteur || input.starts_with("//")
}

/// Normalise un chemin vers sa forme de stockage.
///
/// Hors Windows, l'entree est rendue telle quelle. Sur Windows, chaque `\`
/// devient `/`, comme `input.replaceAll("\\", "/")` de la source.
pub fn storage_path(input: &str, is_windows: bool) -> String {
    if !is_windows {
        return input.to_string();
    }
    input.replace('\\', "/")
}

/// Dit si un chemin est absolu au sens posix.
///
/// `nodePath.posix.isAbsolute` ne regarde que la barre initiale : un chemin
/// qui commence par `/` est absolu, rien d'autre ne l'est.
pub fn is_absolute_posix(path: &str) -> bool {
    path.starts_with('/')
}

/// Normalise un chemin et exige qu'il soit absolu.
///
/// Rend la forme de stockage, ou `Err("Path is not absolute: <entree>")`
/// avec l'entree d'origine, comme le `throw new Error` de la source.
pub fn absolute(input: &str, is_windows: bool) -> Result<String, String> {
    let result = storage_path(input, is_windows);
    let posix = is_absolute_posix(&result);
    let windows = is_windows && is_windows_storage_path(&result);
    if !posix && !windows {
        return Err(format!("Path is not absolute: {input}"));
    }
    Ok(result)
}

/// Rend un chemin de stockage vers sa forme de la plateforme.
///
/// Hors Windows, ou entree qui n'est pas un chemin de stockage Windows,
/// l'entree est rendue telle quelle. Sinon chaque `/` devient `\`.
pub fn to_platform(input: &str, is_windows: bool) -> String {
    if !is_windows || !is_windows_storage_path(input) {
        return input.to_string();
    }
    input.replace('/', "\\")
}

/// Conversion d'ecriture de la colonne des chemins absolus : `absolute`.
pub fn absolute_to_driver(input: &str, is_windows: bool) -> Result<String, String> {
    absolute(input, is_windows)
}

/// Conversion de lecture de la colonne des chemins absolus.
///
/// `toPlatform(absolute(entree))`, la marque `AbsolutePath` etant sans effet
/// a l'execution.
pub fn absolute_from_driver(input: &str, is_windows: bool) -> Result<String, String> {
    Ok(to_platform(&absolute(input, is_windows)?, is_windows))
}

/// Conversion d'ecriture de la colonne des repertoires.
///
/// La chaine vide traverse sans validation, comme le ternaire `input ?
/// absolute(input) : input` de la source.
pub fn directory_to_driver(input: &str, is_windows: bool) -> Result<String, String> {
    if input.is_empty() {
        return Ok(String::new());
    }
    absolute(input, is_windows)
}

/// Conversion de lecture de la colonne des repertoires.
///
/// Comme a l'ecriture, la chaine vide traverse sans validation.
pub fn directory_from_driver(input: &str, is_windows: bool) -> Result<String, String> {
    if input.is_empty() {
        return Ok(String::new());
    }
    Ok(to_platform(&absolute(input, is_windows)?, is_windows))
}

/// Conversion d'ecriture de la colonne des chemins simples : `storagePath`.
pub fn path_to_driver(input: &str, is_windows: bool) -> String {
    storage_path(input, is_windows)
}

/// Conversion de lecture de la colonne des chemins simples : `storagePath`.
pub fn path_from_driver(input: &str, is_windows: bool) -> String {
    storage_path(input, is_windows)
}

/// Conversion d'ecriture de la colonne des tableaux de chemins absolus.
///
/// Serialise en JSON le tableau normalise par `absolute`, comme
/// `JSON.stringify(input.map(absolute))` de la source.
pub fn absolute_array_to_driver(inputs: &[&str], is_windows: bool) -> Result<String, String> {
    let mut normalises = Vec::with_capacity(inputs.len());
    for input in inputs {
        normalises.push(absolute(input, is_windows)?);
    }
    serde_json::to_string(&normalises).map_err(|erreur| erreur.to_string())
}

/// Conversion de lecture de la colonne des tableaux de chemins absolus.
///
/// Parse le JSON puis applique la conversion de lecture des chemins absolus
/// a chaque element. Un JSON invalide, ou qui n'est pas un tableau de
/// chaines, donne une erreur au lieu du crash de la source.
pub fn absolute_array_from_driver(input: &str, is_windows: bool) -> Result<Vec<String>, String> {
    let items: Vec<String> = serde_json::from_str(input).map_err(|erreur| erreur.to_string())?;
    items
        .iter()
        .map(|item| absolute_from_driver(item, is_windows))
        .collect()
}

/// Normalise un chemin vers sa forme de stockage sur la plateforme courante.
pub fn storage_path_for_current_platform(input: &str) -> String {
    storage_path(input, cfg!(windows))
}

/// Normalise et valide un chemin absolu sur la plateforme courante.
pub fn absolute_for_current_platform(input: &str) -> Result<String, String> {
    absolute(input, cfg!(windows))
}

/// Rend un chemin de stockage vers la forme de la plateforme courante.
pub fn to_platform_for_current_platform(input: &str) -> String {
    to_platform(input, cfg!(windows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hors_windows_le_stockage_laisse_la_chaine_intacte() {
        assert_eq!(storage_path("a\\b/c", false), "a\\b/c");
        assert_eq!(storage_path("", false), "");
    }

    #[test]
    fn sur_windows_les_antislashs_deviennent_des_slashs() {
        assert_eq!(storage_path("C:\\Users\\a", true), "C:/Users/a");
        assert_eq!(storage_path("deja/normalise", true), "deja/normalise");
    }

    #[test]
    fn une_lettre_de_lecteur_suivie_de_slash_est_un_chemin_windows() {
        assert!(is_windows_storage_path("C:/Users/a"));
        assert!(is_windows_storage_path("z:/"));
        assert!(!is_windows_storage_path("C:relatif"));
        assert!(!is_windows_storage_path("C:\\antislash"));
    }

    #[test]
    fn un_chemin_unc_est_un_chemin_windows() {
        assert!(is_windows_storage_path("//serveur/partage"));
        assert!(!is_windows_storage_path("/simple/posix"));
        assert!(!is_windows_storage_path("relatif/chemin"));
        assert!(!is_windows_storage_path(""));
    }

    #[test]
    fn absolute_accepte_un_chemin_posix_sur_toute_plateforme() {
        assert_eq!(absolute("/a/b", false), Ok("/a/b".to_string()));
        assert_eq!(absolute("/a/b", true), Ok("/a/b".to_string()));
    }

    #[test]
    fn absolute_refuse_un_relatif_avec_le_message_d_origine() {
        assert_eq!(
            absolute("relatif/chemin", false),
            Err("Path is not absolute: relatif/chemin".to_string())
        );
        assert_eq!(absolute("", true), Err("Path is not absolute: ".to_string()));
    }

    #[test]
    fn sur_windows_absolute_accepte_lecteur_et_unc() {
        assert_eq!(absolute("C:/a", true), Ok("C:/a".to_string()));
        assert_eq!(absolute("C:\\a", true), Ok("C:/a".to_string()));
        assert_eq!(absolute("//srv/part", true), Ok("//srv/part".to_string()));
        assert!(absolute("C:/a", false).is_err());
    }

    #[test]
    fn to_platform_ne_touche_rien_hors_windows() {
        assert_eq!(to_platform("C:/a/b", false), "C:/a/b");
        assert_eq!(to_platform("/a/b", false), "/a/b");
    }

    #[test]
    fn sur_windows_to_platform_ne_convertit_que_les_chemins_windows() {
        assert_eq!(to_platform("C:/a/b", true), "C:\\a\\b");
        assert_eq!(to_platform("//srv/part", true), "\\\\srv\\part"));
        assert_eq!(to_platform("/posix/seul", true), "/posix/seul");
    }

    #[test]
    fn la_colonne_directory_laisse_passer_la_chaine_vide() {
        assert_eq!(directory_to_driver("", false), Ok(String::new()));
        assert_eq!(directory_from_driver("", true), Ok(String::new()));
        assert_eq!(directory_to_driver("/a", false), Ok("/a".to_string()));
        assert_eq!(directory_from_driver("C:/a", true), Ok("C:\\a".to_string()));
    }

    #[test]
    fn la_colonne_path_normalise_dans_les_deux_sens_sans_valider() {
        assert_eq!(path_to_driver("C:\\a", true), "C:/a");
        assert_eq!(path_from_driver("C:\\a", true), "C:/a");
        assert_eq!(path_to_driver("relatif", true), "relatif");
        assert_eq!(path_from_driver("relatif", false), "relatif");
    }

    #[test]
    fn le_tableau_json_fait_l_aller_retour() {
        let ecrit = absolute_array_to_driver(&["/a", "/b"], false).expect("ecriture");
        assert_eq!(ecrit, "[\"/a\",\"/b\"]");
        let lu = absolute_array_from_driver(&ecrit, false).expect("lecture");
        assert_eq!(lu, vec!["/a".to_string(), "/b".to_string()]);
    }

    #[test]
    fn un_tableau_vide_donne_un_json_vide_et_reciproquement() {
        let ecrit = absolute_array_to_driver(&[], false).expect("ecriture");
        assert_eq!(ecrit, "[]");
        assert!(absolute_array_from_driver("[]", false).expect("lecture").is_empty());
    }

    #[test]
    fn un_json_invalide_ou_non_tableau_est_refuse_a_la_lecture() {
        assert!(absolute_array_from_driver("pas du json", false).is_err());
        assert!(absolute_array_from_driver("{\"a\":1}", false).is_err());
        assert!(absolute_array_from_driver("[1,2]", false).is_err());
    }

    #[test]
    fn un_element_relatif_fait_echouer_tout_le_tableau() {
        assert!(absolute_array_to_driver(&["/a", "relatif"], false).is_err());
        assert!(absolute_array_from_driver("[\"/a\",\"relatif\"]", false).is_err());
    }

    #[test]
    fn les_quatre_colonnes_declarent_du_texte() {
        assert_eq!(DATA_TYPE, "text");
    }

    #[test]
    fn les_enveloppes_courantes_suivent_la_plateforme_de_compilation() {
        let attendu = if cfg!(windows) { "a/b" } else { "a\\b" };
        assert_eq!(storage_path_for_current_platform("a\\b"), attendu);
        assert_eq!(
            to_platform_for_current_platform("C:/a").contains('\\'),
            cfg!(windows)
        );
        assert!(absolute_for_current_platform("/a").is_ok());
    }
}
