# swarm-oc-global

===DEBUT===
// fichier : src/core/global.rs (a creer par l agent principal, voir A VERIFIER)
// source  : opencode/packages/core/src/global.ts (87 lignes, lu en entier)
// taille  : 13896 octets
// tests   : 8

//! Portage Rust de `opencode/packages/core/src/global.ts`.
//!
//! Ce module calcule les repertoires globaux de l application : donnees,
//! cache, config, etat, temporaire, binaires, journaux et depots clones.
//! L original fait trois choses au chargement du module : resoudre les
//! chemins XDG, creer les repertoires (`mkdir -p`), puis publier un service
//! `Effect` / `Layer`.
//!
//! Le portage separe le pur de l effet de bord :
//! - `resolve_home`, `xdg_base`, `resolve_bases`, `build_paths` et
//!   `make_paths` sont des fonctions pures : les variables d environnement
//!   arrivent en parametres (`Option<&str>`) au lieu d etre lues ;
//! - `from_env` lit l environnement reel, comme `make()` sans argument ;
//! - la creation des repertoires n est pas faite ici : `required_dirs`
//!   renvoie la liste exacte des sept repertoires crees par l original, dans
//!   le meme ordre, et c est a l appelant de les creer ;
//! - le `Context.Service` + `Layer` devient le trait `GlobalService` et son
//!   implementation `GlobalLive` ;
//! - l appel `Flock.setGlobal({ state })` appartient au module `flock`, il
//!   n est pas reproduit ici pour ne rien inventer.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Nom d application ajoute a chaque base XDG, comme `path.join(xdg, app)`.
pub const APP_NAME: &str = "opencode";

/// Variable lue par le getter `home` de l original (`?? os.homedir()`).
pub const ENV_TEST_HOME: &str = "OPENCODE_TEST_HOME";

/// Variable lue via `Flag.OPENCODE_CONFIG_DIR` (voir `flag/flag.ts`) pour le
/// repertoire config.
pub const ENV_CONFIG_DIR: &str = "OPENCODE_CONFIG_DIR";

/// Repertoires globaux resolus, equivalent de `Interface` de l original.
///
/// Tous les champs sont en un seul mot minuscule, donc aucun
/// `#[serde(rename)]` n est necessaire (piege des noms de champs : rien a
/// renommer ici, et un test le verifie).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalPaths {
    pub home: String,
    pub data: String,
    pub cache: String,
    pub config: String,
    pub state: String,
    pub tmp: String,
    pub bin: String,
    pub log: String,
    pub repos: String,
}

/// Surcharge partielle, equivalent du `Partial<Interface>` de `make(input)`.
///
/// Chaque champ `Some` remplace le defaut, chaque champ `None` le garde, ce
/// qui reproduit exactement la fusion `{ ...defauts, ...input }`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repos: Option<String>,
}

/// Bases XDG brutes, avant ajout du segment `opencode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct XdgBases {
    pub data: String,
    pub cache: String,
    pub config: String,
    pub state: String,
}

/// Service global, equivalent du `Context.Service("@opencode/Global")`.
pub trait GlobalService {
    fn paths(&self) -> &GlobalPaths;
}

/// Implementation concrete du service, equivalent du `Layer.effect`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalLive {
    paths: GlobalPaths,
}

impl GlobalLive {
    pub fn new(paths: GlobalPaths) -> Self {
        Self { paths }
    }

    /// Construit le service depuis des valeurs brutes, comme `make(input)`.
    pub fn from_parts(
        home: &str,
        bases: &XdgBases,
        tmp_dir: &str,
        config_flag: Option<&str>,
        overrides: GlobalOverride,
    ) -> Self {
        Self {
            paths: make_paths(build_paths(home, bases, tmp_dir, config_flag), overrides),
        }
    }
}

impl GlobalService for GlobalLive {
    fn paths(&self) -> &GlobalPaths {
        &self.paths
    }
}

/// Joint deux segments comme `path.join`, avec le separateur du systeme.
pub fn join(base: &str, segment: &str) -> String {
    PathBuf::from(base)
        .join(segment)
        .to_string_lossy()
        .into_owned()
}

/// Resout `home` : `OPENCODE_TEST_HOME ?? os.homedir()`.
///
/// L operateur `??` teste la nullite, pas la veracite : une chaine vide
/// definie survit et n est PAS remplacee par `os_home` (piege `?` contre
/// `??`). `Some("")` donne donc `""`, seul `None` donne `os_home`.
pub fn resolve_home(test_home: Option<&str>, os_home: &str) -> String {
    test_home.unwrap_or(os_home).to_string()
}

/// Resout une base XDG : valeur d environnement sinon repli sous `home`.
///
/// Contrairement a `resolve_home`, une valeur vide retombe sur le defaut :
/// un chemin de base vide produirait des chemins invalides, et le paquet
/// `xdg-basedir` de l original ignore les valeurs vides.
pub fn xdg_base(env_val: Option<&str>, home: &str, fallback_rel: &str) -> String {
    match env_val {
        Some(v) if !v.is_empty() => v.to_string(),
        _ => join(home, fallback_rel),
    }
}

/// Resout les quatre bases XDG avec les replis standards de `xdg-basedir`.
pub fn resolve_bases(
    data_home: Option<&str>,
    cache_home: Option<&str>,
    config_home: Option<&str>,
    state_home: Option<&str>,
    home: &str,
) -> XdgBases {
    XdgBases {
        data: xdg_base(data_home, home, ".local/share"),
        cache: xdg_base(cache_home, home, ".cache"),
        config: xdg_base(config_home, home, ".config"),
        state: xdg_base(state_home, home, ".local/state"),
    }
}

/// Resout le repertoire config : `Flag.OPENCODE_CONFIG_DIR ?? Path.config`.
///
/// Meme semantique `??` que `resolve_home` : `Some("")` survit tel quel.
pub fn resolve_config(config_flag: Option<&str>, default_config: &str) -> String {
    config_flag.unwrap_or(default_config).to_string()
}

/// Construit les chemins complets depuis `home`, les bases XDG et `tmp`.
///
/// Derive `bin = cache/bin`, `log = data/log`, `repos = data/repos` comme
/// l objet `paths` de l original.
pub fn build_paths(
    home: &str,
    bases: &XdgBases,
    tmp_dir: &str,
    config_flag: Option<&str>,
) -> GlobalPaths {
    let data = join(&bases.data, APP_NAME);
    let cache = join(&bases.cache, APP_NAME);
    let config_default = join(&bases.config, APP_NAME);
    GlobalPaths {
        home: home.to_string(),
        data: data.clone(),
        cache: cache.clone(),
        config: resolve_config(config_flag, &config_default),
        state: join(&bases.state, APP_NAME),
        tmp: join(tmp_dir, APP_NAME),
        bin: join(&cache, "bin"),
        log: join(&data, "log"),
        repos: join(&data, "repos"),
    }
}

/// Fusionne des defauts avec une surcharge, comme `make(input)`.
///
/// Equivalent de `{ ...defauts, ...input }` : chaque `Some` gagne, chaque
/// `None` garde le defaut. Prend `base` par valeur pour eviter un clone
/// quand l appelant n en a plus besoin.
pub fn make_paths(base: GlobalPaths, overrides: GlobalOverride) -> GlobalPaths {
    GlobalPaths {
        home: overrides.home.unwrap_or(base.home),
        data: overrides.data.unwrap_or(base.data),
        cache: overrides.cache.unwrap_or(base.cache),
        config: overrides.config.unwrap_or(base.config),
        state: overrides.state.unwrap_or(base.state),
        tmp: overrides.tmp.unwrap_or(base.tmp),
        bin: overrides.bin.unwrap_or(base.bin),
        log: overrides.log.unwrap_or(base.log),
        repos: overrides.repos.unwrap_or(base.repos),
    }
}

/// Les sept repertoires que l original cree avec `mkdir -p`, dans le meme
/// ordre : data, config, state, tmp, log, bin, repos.
///
/// Ni `home` ni la racine `cache` ne sont crees par l original : la liste
/// les exclut pour rester fidele. C est a l appelant de creer ces
/// repertoires (effet de bord volontairement sorti du code pur).
pub fn required_dirs(paths: &GlobalPaths) -> Vec<String> {
    vec![
        paths.data.clone(),
        paths.config.clone(),
        paths.state.clone(),
        paths.tmp.clone(),
        paths.log.clone(),
        paths.bin.clone(),
        paths.repos.clone(),
    ]
}

/// Construit les chemins depuis l environnement reel, comme `make()`.
///
/// Lit `OPENCODE_TEST_HOME`, les quatre `XDG_*_HOME`, `OPENCODE_CONFIG_DIR`,
/// et les dossiers systeme passes en parametres. Fonction synchrone : aucun
/// `async` n est necessaire, il n y a aucune attente.
pub fn from_env(os_home: &str, tmp_dir: &str) -> GlobalPaths {
    let test_home = std::env::var(ENV_TEST_HOME).ok();
    let data_home = std::env::var("XDG_DATA_HOME").ok();
    let cache_home = std::env::var("XDG_CACHE_HOME").ok();
    let config_home = std::env::var("XDG_CONFIG_HOME").ok();
    let state_home = std::env::var("XDG_STATE_HOME").ok();
    let config_flag = std::env::var(ENV_CONFIG_DIR).ok();
    let home = resolve_home(test_home.as_deref(), os_home);
    let bases = resolve_bases(
        data_home.as_deref(),
        cache_home.as_deref(),
        config_home.as_deref(),
        state_home.as_deref(),
        &home,
    );
    build_paths(&home, &bases, tmp_dir, config_flag.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bases() -> XdgBases {
        XdgBases {
            data: "/x/data".to_string(),
            cache: "/x/cache".to_string(),
            config: "/x/config".to_string(),
            state: "/x/state".to_string(),
        }
    }

    #[test]
    fn une_chaine_test_home_vide_survit_au_coalescent() {
        // `??` teste la nullite : `Some("")` donne `""`, seul `None` replie.
        assert_eq!(resolve_home(Some(""), "/home/u"), "");
        assert_eq!(resolve_home(None, "/home/u"), "/home/u");
        assert_eq!(resolve_home(Some("/test"), "/home/u"), "/test");
    }

    #[test]
    fn une_base_xdg_vide_retombe_sur_le_defaut() {
        assert_eq!(xdg_base(None, "/home/u", ".cache"), "/home/u/.cache");
        assert_eq!(xdg_base(Some(""), "/home/u", ".cache"), "/home/u/.cache");
        assert_eq!(xdg_base(Some("/perso"), "/home/u", ".cache"), "/perso");
    }

    #[test]
    fn build_derive_bin_log_repos_et_garde_le_config_par_defaut() {
        let p = build_paths("/home/u", &sample_bases(), "/tmp", None);
        assert_eq!(p.home, "/home/u");
        assert_eq!(p.data, "/x/data/opencode");
        assert_eq!(p.cache, "/x/cache/opencode");
        assert_eq!(p.config, "/x/config/opencode");
        assert_eq!(p.state, "/x/state/opencode");
        assert_eq!(p.tmp, "/tmp/opencode");
        assert_eq!(p.bin, "/x/cache/opencode/bin");
        assert_eq!(p.log, "/x/data/opencode/log");
        assert_eq!(p.repos, "/x/data/opencode/repos");
    }

    #[test]
    fn le_flag_config_remplace_le_config_par_defaut() {
        let p = build_paths("/home/u", &sample_bases(), "/tmp", Some("/flag"));
        assert_eq!(p.config, "/flag");
        // Le reste ne bouge pas.
        assert_eq!(p.data, "/x/data/opencode");
    }

    #[test]
    fn make_sans_override_garde_tous_les_defauts() {
        let base = build_paths("/home/u", &sample_bases(), "/tmp", None);
        let out = make_paths(base.clone(), GlobalOverride::default());
        assert_eq!(out, base);
    }

    #[test]
    fn make_override_partiel_ne_touche_que_le_champ_vise() {
        let base = build_paths("/home/u", &sample_bases(), "/tmp", None);
        let out = make_paths(
            base.clone(),
            GlobalOverride {
                data: Some("/ailleurs".to_string()),
                ..Default::default()
            },
        );
        assert_eq!(out.data, "/ailleurs");
        assert_eq!(out.cache, base.cache);
        assert_eq!(out.config, base.config);
        assert_eq!(out.home, base.home);
    }

    #[test]
    fn required_dirs_rend_les_sept_repertoires_dans_l_ordre_origine() {
        let p = build_paths("/home/u", &sample_bases(), "/tmp", None);
        let dirs = required_dirs(&p);
        assert_eq!(dirs.len(), 7);
        assert_eq!(dirs[0], p.data);
        assert_eq!(dirs[1], p.config);
        assert_eq!(dirs[2], p.state);
        assert_eq!(dirs[3], p.tmp);
        assert_eq!(dirs[4], p.log);
        assert_eq!(dirs[5], p.bin);
        assert_eq!(dirs[6], p.repos);
        // Ni `home` ni la racine `cache` ne sont dans la liste.
        assert!(!dirs.contains(&p.home));
        assert!(!dirs.contains(&p.cache));
    }

    #[test]
    fn la_serialisation_garde_les_noms_de_champs_tels_quels() {
        // Aucun champ camelCase dans la source, donc aucun `rename` attendu :
        // le JSON doit contenir les noms exacts.
        let p = build_paths("/home/u", &sample_bases(), "/tmp", None);
        let json = serde_json::to_string(&p).unwrap();
        for field in ["home", "data", "cache", "config", "state", "tmp", "bin", "log", "repos"] {
            assert!(json.contains(&format!("\"{field}\":"), "champ {field} absent du JSON"));
        }
        let back: GlobalPaths = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
        // Un override vide ne se serialise en aucun champ.
        let empty = serde_json::to_string(&GlobalOverride::default()).unwrap();
        assert_eq!(empty, "{}");
    }
}
===FIN===

CONFIANCE : moyenne
POINT FAIBLE : la semantique `||` (falsy) de `xdg-basedir` pour les valeurs XDG vides est supposee sans avoir lu ce paquet, et `os.tmpdir()` / `os.homedir()` du TS ne sont pas reimplementes ici mais passes en parametres.
A VERIFIER : (1) absence de tout caractere non ASCII dans le bloc (a controler a la CI) ; (2) le chemin cible `src/core/global.rs` n existe pas encore, a declarer dans `mod.rs` par l agent principal ; (3) `Flock.setGlobal` et `makeGlobalNode` restent du ressort des modules `flock` et `effect/app-node`, non traites ici.
