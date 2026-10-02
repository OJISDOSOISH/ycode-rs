//! Portage Rust de `packages/core/src/database/database.ts`.
//!
//! ## Ce que contient la source
//!
//! Cinquante-sept lignes, trois exports utiles et un service :
//!
//! ```ts
//! export class Service extends Context.Service<Service, Interface>()("@opencode/v2/storage/Database") {}
//! export function layerFromPath(filename: string) { ... }
//! export function path() { ... }
//! export const node = makeGlobalNode({ service: Service, layer: layerFromPath(path()), deps: [] })
//! ```
//!
//! Le corps du layer applique six `PRAGMA` puis `DatabaseMigration.apply(db)`.
//! Ce fichier ne porte ni le pilote SQLite (voir `sqlite.ts`, `sqlite.bun.ts`,
//! `sqlite.node.ts`), ni les migrations (voir `migration.ts`) : il porte le
//! contrat du service, l'ordre des pragmas et la resolution du chemin.
//!
//! ## Ce qui n'a pas de traduction ici, et pourquoi
//!
//! - `EffectDrizzleSqlite.makeWithDefaults()`, `Layer.effect`, `Effect.gen` :
//!   l'appareil Effect de construction du layer. Sans runtime Effect en Rust,
//!   la seule chose honnete est la configuration (`ConfigCouche`) et le tag.
//! - `makeGlobalNode` : le registre global des services vit dans
//!   `effect/app-node.ts`, pas ici. Seule l'absence de dependances (`deps: []`)
//!   est portee, comme une constante.
//! - `Global.Path.data` : le repertoire de donnees XDG est calcule dans
//!   `global.ts`. Ici il est un parametre, pas une lecture.
//!
//! ## La resolution du chemin, transcrite a l'identique
//!
//! ```ts
//! export function path() {
//!   if (Flag.OPENCODE_DB) {
//!     if (Flag.OPENCODE_DB === ":memory:" || isAbsolute(Flag.OPENCODE_DB)) return Flag.OPENCODE_DB
//!     return join(Global.Path.data, Flag.OPENCODE_DB)
//!   }
//!   if (["latest", "beta", "prod"].includes(InstallationChannel) ||
//!     process.env.OPENCODE_DISABLE_CHANNEL_DB === "1" ||
//!     process.env.OPENCODE_DISABLE_CHANNEL_DB === "true")
//!     return join(Global.Path.data, "opencode.db")
//!   return join(Global.Path.data, `opencode-${InstallationChannel.replace(/[^a-zA-Z0-9._-]/g, "-")}.db`)
//! }
//! ```
//!
//! Trois details qui comptent, portes tels quels :
//!
//! - `if (Flag.OPENCODE_DB)` teste la valeur de verite JavaScript : une chaine
//!   vide vaut absence de flag. [`resoudre_chemin`] fait de meme.
//! - `isAbsolute` vient de `path`, donc depend de la plateforme a l'execution.
//!   [`est_absolu`] couvre les deux familles (POSIX et Windows) de facon
//!   deterministe, et le choix est documente plutot que cache.
//! - Le remplacement du canal est un regex global caractere par caractere :
//!   `[^a-zA-Z0-9._-]` ne connait que l'ASCII, donc `e` accentue devient `-`.
//!   [`assainir_canal`] applique exactement cette table.

/// Tag du service, premier argument de `Context.Service`.
pub const SERVICE_TAG: &str = "@opencode/v2/storage/Database";

/// Nom du fichier partage par les canaux stables et les bases sans canal.
pub const DEFAULT_FILENAME: &str = "opencode.db";

/// Valeur speciale qui designe une base en memoire, jamais jointe a un repertoire.
pub const MEMORY_FILENAME: &str = ":memory:";

/// Canaux qui partagent le fichier `opencode.db` sans suffixe.
pub const CANAUX_STABLES: [&str; 3] = ["latest", "beta", "prod"];

/// Les six pragmas appliques a l'ouverture, dans l'ordre de la source.
pub const PRAGMAS: [&str; 6] = [
    "PRAGMA journal_mode = WAL",
    "PRAGMA synchronous = NORMAL",
    "PRAGMA busy_timeout = 5000",
    "PRAGMA cache_size = -64000",
    "PRAGMA foreign_keys = ON",
    "PRAGMA wal_checkpoint(PASSIVE)",
];

/// Dependances du noeud global : la source ecrit `deps: []`.
pub const NODE_DEPS: [&str; 0] = [];

/// Dit si un chemin est absolu, au sens de `path.isAbsolute` reuni sur les
/// deux plateformes.
///
/// `":memory:"` n'est pas absolu : c'est une valeur speciale traitee avant,
/// dans [`resoudre_chemin`]. Un chemin UNC (`\\serveur\...`) et un chemin a
/// lettre de lecteur (`C:/...`, `C:\...`) sont absolus, comme sur Windows ;
/// un chemin qui commence par `/` est absolu, comme sur POSIX.
pub fn est_absolu(chemin: &str) -> bool {
    if chemin.starts_with('/') {
        return true;
    }
    let octets = chemin.as_bytes();
    if octets.len() >= 3
        && octets[0].is_ascii_alphabetic()
        && octets[1] == b':'
        && (octets[2] == b'/' || octets[2] == b'\\')
    {
        return true;
    }
    if chemin.starts_with("\\\\") {
        return true;
    }
    false
}

/// Joint un repertoire et un nom de fichier, comme `path.join` pour ce cas.
///
/// `path.join` n'est appele dans la source qu'avec un second argument relatif
/// (l'absolu est retourne avant), donc la simple concatenation avec un seul
/// separateur suffit. Un separateur final sur le repertoire n'est pas duplique.
pub fn joindre(repertoire: &str, fichier: &str) -> String {
    let base = repertoire.trim_end_matches(['/', '\\']);
    format!("{base}/{fichier}")
}

/// Assainit un nom de canal pour en faire un suffixe de fichier.
///
/// Transcription exacte de
/// `InstallationChannel.replace(/[^a-zA-Z0-9._-]/g, "-")` : chaque caractere
/// hors de `[a-zA-Z0-9._-]` devient `-`. La classe est ASCII seule, donc un
/// caractere accentue est remplace, pas conserve.
pub fn assainir_canal(canal: &str) -> String {
    canal
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Lit la variable `OPENCODE_DISABLE_CHANNEL_DB` comme la source.
///
/// Seules les valeurs exactes `"1"` et `"true"` (minuscules) desactivent le
/// fichier par canal. `"True"`, `"0"` ou l'absence valent `false`.
pub fn canal_desactive_par_env(valeur: Option<&str>) -> bool {
    matches!(valeur, Some("1") | Some("true"))
}

/// Nom du fichier par defaut pour un canal donne.
///
/// Les canaux stables (`latest`, `beta`, `prod`) et le mode desactive
/// partagent `opencode.db`. Les autres canaux recoivent
/// `opencode-<canal assaini>.db`.
pub fn nom_fichier_defaut(canal: &str, canal_desactive: bool) -> String {
    if CANAUX_STABLES.contains(&canal) || canal_desactive {
        DEFAULT_FILENAME.to_string()
    } else {
        format!("opencode-{}.db", assainir_canal(canal))
    }
}

/// Resout le chemin de la base, transcription de `path()`.
///
/// - `Some(":memory:")` ou un chemin absolu traverse tel quel ;
/// - un flag relatif est joint au repertoire de donnees ;
/// - `None` (ou `Some("")`, falsy en JavaScript) mene au fichier par defaut.
pub fn resoudre_chemin(
    flag: Option<&str>,
    repertoire_donnees: &str,
    canal: &str,
    canal_desactive: bool,
) -> String {
    if let Some(valeur) = flag {
        if !valeur.is_empty() {
            if valeur == MEMORY_FILENAME || est_absolu(valeur) {
                return valeur.to_string();
            }
            return joindre(repertoire_donnees, valeur);
        }
    }
    joindre(
        repertoire_donnees,
        &nom_fichier_defaut(canal, canal_desactive),
    )
}

/// Configuration du layer pour un fichier donne, transcription de
/// `layerFromPath(filename) : layer.pipe(Layer.provide(sqliteLayer({ filename })))`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigCouche {
    /// Chemin du fichier SQLite fourni au layer natif.
    pub filename: String,
}

/// Construit la configuration du layer depuis un chemin, comme
/// `layerFromPath`.
pub fn couche_depuis_chemin(filename: &str) -> ConfigCouche {
    ConfigCouche {
        filename: filename.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_tag_de_service_est_celui_declare_dans_la_source() {
        assert_eq!(SERVICE_TAG, "@opencode/v2/storage/Database");
    }

    #[test]
    fn le_noeud_global_ne_depend_de_rien() {
        assert!(NODE_DEPS.is_empty(), "la source ecrit deps: []");
    }

    #[test]
    fn les_six_pragmas_sont_dans_l_ordre_de_la_source() {
        assert_eq!(PRAGMAS.len(), 6);
        assert_eq!(PRAGMAS[0], "PRAGMA journal_mode = WAL");
        assert_eq!(PRAGMAS[1], "PRAGMA synchronous = NORMAL");
        assert_eq!(PRAGMAS[2], "PRAGMA busy_timeout = 5000");
        assert_eq!(PRAGMAS[3], "PRAGMA cache_size = -64000");
        assert_eq!(PRAGMAS[4], "PRAGMA foreign_keys = ON");
        assert_eq!(PRAGMAS[5], "PRAGMA wal_checkpoint(PASSIVE)");
    }

    #[test]
    fn la_memoire_traverse_telle_quelle_sans_repertoire() {
        assert_eq!(
            resoudre_chemin(Some(":memory:"), "/data", "local", false),
            ":memory:"
        );
    }

    #[test]
    fn un_flag_absolu_traverse_tel_quel() {
        assert_eq!(
            resoudre_chemin(Some("/abs/base.db"), "/data", "local", false),
            "/abs/base.db"
        );
        assert_eq!(
            resoudre_chemin(Some("C:/abs/base.db"), "/data", "local", false),
            "C:/abs/base.db"
        );
    }

    #[test]
    fn un_flag_relatif_est_joint_au_repertoire_de_donnees() {
        assert_eq!(
            resoudre_chemin(Some("perso.db"), "/data", "local", false),
            "/data/perso.db"
        );
    }

    #[test]
    fn une_chaine_vide_vaut_absence_de_flag_comme_en_javascript() {
        // `if (Flag.OPENCODE_DB)` est falsy sur "" : on retombe sur le defaut.
        assert_eq!(
            resoudre_chemin(Some(""), "/data", "local", false),
            "/data/opencode-local.db"
        );
        assert_eq!(
            resoudre_chemin(None, "/data", "local", false),
            "/data/opencode-local.db"
        );
    }

    #[test]
    fn les_canaux_stables_partagent_le_meme_fichier() {
        for canal in ["latest", "beta", "prod"] {
            assert_eq!(nom_fichier_defaut(canal, false), "opencode.db");
            assert_eq!(
                resoudre_chemin(None, "/data", canal, false),
                "/data/opencode.db"
            );
        }
    }

    #[test]
    fn un_canal_local_suffixe_le_fichier() {
        assert_eq!(nom_fichier_defaut("local", false), "opencode-local.db");
        assert_eq!(nom_fichier_defaut("dev", false), "opencode-dev.db");
    }

    #[test]
    fn l_assainissement_remplace_tout_hors_classe_ascii() {
        assert_eq!(assainir_canal("feat/branche test"), "feat-branche-test");
        assert_eq!(assainir_canal("a.b_c-d"), "a.b_c-d");
        assert_eq!(assainir_canal("canal:123"), "canal-123");
        // La classe est ASCII seule : l'accent ne passe pas.
        assert_eq!(assainir_canal("ete"), "et-");
    }

    #[test]
    fn seules_les_valeurs_un_et_true_desactivent_le_fichier_par_canal() {
        assert!(canal_desactive_par_env(Some("1")));
        assert!(canal_desactive_par_env(Some("true")));
        assert!(!canal_desactive_par_env(Some("True")));
        assert!(!canal_desactive_par_env(Some("0")));
        assert!(!canal_desactive_par_env(None));
    }

    #[test]
    fn le_mode_desactive_force_le_fichier_commun() {
        assert_eq!(nom_fichier_defaut("local", true), "opencode.db");
        assert_eq!(
            resoudre_chemin(None, "/data", "local", true),
            "/data/opencode.db"
        );
    }

    #[test]
    fn la_jonction_ne_duplique_pas_le_separateur_final() {
        assert_eq!(joindre("/data", "a.db"), "/data/a.db");
        assert_eq!(joindre("/data/", "a.db"), "/data/a.db");
    }

    #[test]
    fn l_absolu_reconnait_posix_windows_et_unc() {
        assert!(est_absolu("/a/b.db"));
        assert!(est_absolu("C:/a/b.db"));
        assert!(est_absolu("D:\\a\\b.db"));
        assert!(est_absolu("\\\\serveur\\partage"));
        assert!(!est_absolu("relatif/b.db"));
        assert!(!est_absolu("base.db"));
        assert!(!est_absolu(":memory:"));
    }

    #[test]
    fn la_couche_retient_le_nom_de_fichier_fourni() {
        let config = couche_depuis_chemin("/data/opencode.db");
        assert_eq!(config.filename, "/data/opencode.db");
    }
}
