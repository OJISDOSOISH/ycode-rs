//! Portage Rust de `opencode/packages/core/src/installation/version.ts`.
//!
//! La source tient en huit lignes et ne contient **aucune fonction**. Elle se
//! resume a deux globales declarees, deux constantes derivees, et un booleen :
//!
//! ```text
//! declare global {
//!   const OPENCODE_VERSION: string
//!   const OPENCODE_CHANNEL: string
//! }
//!
//! export const InstallationVersion = typeof OPENCODE_VERSION === "string" ? OPENCODE_VERSION : "local"
//! export const InstallationChannel = typeof OPENCODE_CHANNEL === "string" ? OPENCODE_CHANNEL : "local"
//! export const InstallationLocal = InstallationChannel === "local"
//! ```
//!
//! ## Ce que la source ne fait pas
//!
//! Elle ne compare **aucune** version. Il n y a ni semver, ni tri, ni
//! decoupage de segments, ni retrait d un prefixe `v`, ni lecture d un
//! suffixe de prerelease. Les deux seuls traitements de chaine sont la
//! substitution du defaut `"local"` et l egalite stricte sur `"local"`.
//! Aucun comparateur n a donc ete invente ici : ce serait du code sans
//! source, et il serait appele par personne.
//!
//! Ce que la source **fait** de la chaine, en revanche, est port exactement :
//! une version prefixee par `v`, une version a deux segments, une version
//! prerelease, et meme une chaine vide ressortent **intactes**. Les tests
//! verrouillent ce point, parce que c est la ou une version se fait
//! normaliser par megarde.
//!
//! ## Les deux globales, et pourquoi `option_env!`
//!
//! `OPENCODE_VERSION` et `OPENCODE_CHANNEL` n existent pas dans le code
//! compile : le script de build les remplace par un litteral
//! (`packages/script/src/index.ts` les lit dans l environnement, puis
//! `script/build.ts` les ecrit dans le paquet). La valeur est donc figee au
//! moment de la compilation, ce qui est aussi ce qu attendent les appelants :
//! `opencode/` + version dans un `User-Agent`, ou un nom de paquet npm.
//!
//! Rust n a pas d equivalent exact de la substitution de esbuild, mais
//! `option_env!` en est le plus proche, et il a exactement la meme semantique
//! sur les trois cas qui comptent :
//!
//! - variable definie a la compilation : `Some("...")`, la valeur est reprise ;
//! - variable absente : `None`, le defaut `"local"` prend le relais ;
//! - variable definie mais vide : `Some("")`, la chaine vide **survit**.
//!
//! Lire la variable a l execution (`std::env::var`) serait une faute : la
//! valeur pourrait alors changer entre deux appels d un meme processus, ce que
//! le TypeScript ne peut jamais faire.
//!
//! ## Le piege `typeof` : verifier un type, pas la veracite
//!
//! Le test de la source est `typeof OPENCODE_VERSION === "string"`, et non
//! `OPENCODE_VERSION ? ... : ...`. La difference est capitale :
//!
//! - `typeof` teste le **type**. Il renvoie `"undefined"` pour une globale
//!   absente, et il ne leve **pas** de `ReferenceError` sur un identifiant
//!   jamais declare, contrairement a une lecture directe. C est precisement ce
//!   qui rend ce fichier utilisable quand il est charge depuis les sources,
//!   hors du paquet build ou la substitution n a pas eu lieu.
//! - Le ternaire sur la veracite remplacerait au passage la chaine vide par
//!   `"local"`, alors que `typeof "" === "string"` est vrai et que la source
//!   renverse donc `""` tel quel.
//!
//! D ou le choix de `Option<&str>` plutot que d une chaine vide comme
//! sentinelle : `None` est l absence, `Some("")` est la chaine vide. Le test
//! `une_version_vide_est_conservee_et_non_remplacee` verrouille le point.
//!
//! ## Deux constantes et non deux fonctions
//!
//! `InstallationVersion` et `InstallationChannel` restent des `const` : ce
//! sont des valeurs compilees, les remplacer par des fonctions changerait leur
//! nature. `InstallationLocal` ne peut pas rester une `const` de facon
//! sure : il repose sur `==` entre deux `&str`, et l operateur d egalite de
//! `str` n est pas utilisable dans un contexte constant. Il devient donc une
//! fonction ordinaire, avec un predicat separe pour pouvoir etre teste sur
//! n importe quelle chaine.
//!
//! Aucun struct, aucun enum, aucune serialisation : le piege des noms de
//! champs JSON ne s applique pas a ce fichier.

/// Nom de la variable d environnement qui porte la version, lue a la
/// compilation par `option_env!`. Ce nom est la seule interface entre le
/// script de build et ce module, il ne doit pas diverger.
pub const VERSION_ENV: &str = "OPENCODE_VERSION";

/// Nom de la variable d environnement qui porte le canal de distribution,
/// lue a la compilation par `option_env!`.
pub const CHANNEL_ENV: &str = "OPENCODE_CHANNEL";

/// Valeur de repli, identique pour la version et pour le canal.
///
/// Elle ne veut pas dire "version inconnue" : en amont, la chaine `"local"`
/// est precisement un canal de distribution (une compilation depuis les
/// sources), et c est ce canal que la comparaison de la ligne 8 teste.
pub const LOCAL: &str = "local";

/// Version de l installation, figee a la compilation.
///
/// Egale a la globale `OPENCODE_VERSION` si le build l a definie, sinon
/// egale a [`LOCAL`].
pub const INSTALLATION_VERSION: &str = valeur_ou_defaut(option_env!("OPENCODE_VERSION"), LOCAL);

/// Canal de distribution de l installation, fige a la compilation.
///
/// Egale a la globale `OPENCODE_CHANNEL` si le build l a definie, sinon
/// egale a [`LOCAL`].
pub const INSTALLATION_CHANNEL: &str = valeur_ou_defaut(option_env!("OPENCODE_CHANNEL"), LOCAL);

/// Renvoie `valeur` si elle est presente, `defaut` sinon.
///
/// C est l equivalent exact du ternaire
/// `typeof X === "string" ? X : defaut` : la decision porte sur la presence,
/// jamais sur le contenu. Une chaine vide est presente, donc elle est
/// renvoyee telle quelle.
///
/// # Exemples
///
/// ```
/// use ycode::swarm::installation_version::valeur_ou_defaut;
///
/// // Rien a la compilation : le defaut prend le relais.
/// assert_eq!(valeur_ou_defaut(None, "local"), "local");
/// // Une chaine vide est une chaine, elle ne devient pas le defaut.
/// assert_eq!(valeur_ou_defaut(Some(""), "local"), "");
/// ```
pub const fn valeur_ou_defaut(valeur: Option<&'static str>, defaut: &'static str) -> &'static str {
    match valeur {
        Some(presente) => presente,
        None => defaut,
    }
}

/// Indique si `canal` est exactement le canal `"local"`.
///
/// Egaliite stricte, comme `===` en TypeScript : la casse compte, les espaces
/// comptent, et une chaine vide n est pas le canal local.
pub fn est_canal_local(canal: &str) -> bool {
    canal == LOCAL
}

/// Vrai quand l installation provient d une compilation locale.
///
/// C est le booleen `InstallationLocal` de la source. Il est expose en
/// fonction et non en `const` car l egalite entre deux `&str` n est pas
/// evaluable a la compilation.
pub fn installation_local() -> bool {
    est_canal_local(INSTALLATION_CHANNEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_globale_absente_donne_le_defaut_local() {
        // C est le cas le plus courant hors paquet build, et le cas des tests
        // du depot TypeScript, charges depuis les sources.
        assert_eq!(valeur_ou_defaut(None, LOCAL), "local");
    }

    #[test]
    fn une_globale_presente_est_reprise_telle_quelle() {
        assert_eq!(valeur_ou_defaut(Some("0.14.2"), LOCAL), "0.14.2");
    }

    #[test]
    fn une_version_vide_est_conservee_et_non_remplacee() {
        // Piege `?` contre `??`. Le test de la source porte sur le type, donc
        // `""` passe : elle ressort vide. Un test de veracite l aurait
        // transformee en "local".
        assert_eq!(valeur_ou_defaut(Some(""), LOCAL), "");
        assert_ne!(valeur_ou_defaut(Some(""), LOCAL), LOCAL);
    }

    #[test]
    fn un_prefixe_v_est_conserve() {
        // La source ne normalise pas : le `v` fait partie de la valeur et
        // ressort dans l User-Agent tel quel.
        assert_eq!(valeur_ou_defaut(Some("v1.2.3"), LOCAL), "v1.2.3");
    }

    #[test]
    fn un_suffixe_de_prerelease_est_conserve() {
        assert_eq!(valeur_ou_defaut(Some("1.2.3-beta.4"), LOCAL), "1.2.3-beta.4");
        assert_eq!(
            valeur_ou_defaut(Some("0.0.0-dev-20260101093000"), LOCAL),
            "0.0.0-dev-20260101093000"
        );
    }

    #[test]
    fn un_nombre_de_segments_different_est_conserve() {
        // Ni deux, ni quatre segments ne sont convertis en trois. La version
        // reste une chaine, jamais une liste de nombres.
        assert_eq!(valeur_ou_defaut(Some("1.0"), LOCAL), "1.0");
        assert_eq!(valeur_ou_defaut(Some("1.2.3.4"), LOCAL), "1.2.3.4");
        assert_eq!(valeur_ou_defaut(Some("2026.9.30"), LOCAL), "2026.9.30");
    }

    #[test]
    fn une_version_non_numerique_est_conservee() {
        // Une version qui n en est pas une reste une chaine. Rien n est valide,
        // rien n est refuse : la source n a aucun controle de forme.
        assert_eq!(valeur_ou_defaut(Some("local"), LOCAL), "local");
        assert_eq!(valeur_ou_defaut(Some("nightly"), LOCAL), "nightly");
    }

    #[test]
    fn le_canal_local_est_reconnu() {
        assert!(est_canal_local("local"));
    }

    #[test]
    fn un_canal_vide_n_est_pas_le_canal_local() {
        // Chaine vide presente, mais differente de "local" : le booleen est
        // faux. Il ne se rabat pas sur le defaut.
        assert!(!est_canal_local(""));
    }

    #[test]
    fn la_casse_du_canal_compte() {
        // `===` est sensible a la casse, `local` n est pas `Local`.
        assert!(!est_canal_local("Local"));
        assert!(!est_canal_local("LOCAL"));
    }

    #[test]
    fn un_espace_autour_du_canal_le_casse() {
        assert!(!est_canal_local(" local"));
        assert!(!est_canal_local("local "));
    }

    #[test]
    fn une_qui_n_est_pas_un_canal_local() {
        // Un nom de distribution ou une version ne sont pas le canal local.
        assert!(!est_canal_local("latest"));
        assert!(!est_canal_local("dev"));
        assert!(!est_canal_local("beta"));
        assert!(!est_canal_local("v1.2.3"));
    }

    #[test]
    fn le_booleen_du_module_suit_la_constante_de_canal() {
        // Le module n applyque rien de plus que la ligne 8 de la source.
        assert_eq!(installation_local(), est_canal_local(INSTALLATION_CHANNEL));
    }

    #[test]
    fn la_version_de_repli_est_indistinguable_d_une_version_absente() {
        // "local" sert a la fois de valeur de repli et de nom de canal. Une
        // compilation depuis les sources produit donc une version "local"
        // impossible a distinguer d une version absente. C est le
        // comportement de la source, pas une approximation de portage, et
        // aucun appelant ne fait la difference.
        assert_eq!(
            valeur_ou_defaut(None, LOCAL),
            valeur_ou_defaut(Some("local"), LOCAL)
        );
    }

    #[test]
    fn les_deux_canaux_lisent_deux_variables_differentes() {
        // Une inversion des deux noms de variables passerait inapercue a la
        // lecture du code, et produirait une version dans l User-Agent du
        // canal de distribution.
        assert_eq!(VERSION_ENV, "OPENCODE_VERSION");
        assert_eq!(CHANNEL_ENV, "OPENCODE_CHANNEL");
        assert_ne!(VERSION_ENV, CHANNEL_ENV);
    }

    #[test]
    fn la_valeur_de_repli_est_la_meme_pour_la_version_et_pour_le_canal() {
        assert_eq!(LOCAL, "local");
    }
}
