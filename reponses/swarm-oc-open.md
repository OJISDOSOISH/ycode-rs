# swarm-oc-open

===DEBUT===
fichier : src/core/open.rs
source : opencode/packages/core/src/open.ts
taille : 8 lignes TS
tests : 7

```rust
//! Portage Rust de `opencode/packages/core/src/open.ts`.
//!
//! La source ne fait qu'une chose : ouvrir une URL dans le navigateur, mais
//! seulement si c'est un lien `http` ou `https` valide. Sinon elle rejette
//! avec le message `Only http and https links can be opened in the browser: ...`.
//!
//! Le portage separe la logique pure (valider et normaliser) de l'effet de
//! bord (ouvrir le navigateur) :
//!
//! - `normalize_open_url` : logique metier pure sur tranche de donnees, sans
//!   I/O, sans `async`. C'est l'equivalent de `URL.canParse` + `new URL().href`
//!   + test du protocole.
//! - `UrlOpener` : `trait` minimal qui represente l'effet `open`. L'appelant
//!   fournit l'implementation concrete (commande systeme, navigateur, mock).
//! - `open_url` : valide puis delegue au `trait`. Fonction synchrone : ouvrir
//!   un lien n'a pas besoin d'`async` ici.
//!
//! Pas de crate `url` au catalogue (`Cargo.toml` ne declare que `serde`,
//! `serde_json`, `tokio`, `reqwest`, `anyhow`, `thiserror`, `async-trait`,
//! `uuid`), donc la validation est reecrite en std seul. Elle approxime
//! WHATWG : scheme insensible a la casse, `://` obligatoire, autorite
//! non vide, puis normalisation legere (scheme en minuscules + `/` final si
//! aucun chemin). La normalisation complete WHATWG (minuscules de l'hote,
//! suppression du port par defaut, encodage des espaces du chemin) n'est pas
//! reproduite.

use std::fmt;

/// Erreur renvoyee quand l'entree n'est pas un lien http/https ouvrable.
///
/// Le message reprend exactement celui du TypeScript d'origine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenError {
    message: String,
}

impl OpenError {
    /// Construit l'erreur avec le message d'origine pour `input`.
    pub fn not_openable(input: &str) -> Self {
        Self {
            message: format!("Only http and https links can be opened in the browser: {input}"),
        }
    }

    /// Message d'erreur, identique au texte rejete par le TypeScript.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for OpenError {}

/// Valide `input` et renvoie l'URL normalisee a ouvrir.
///
/// Regles, dans l'ordre du TypeScript :
/// 1. `input` doit se parser comme une URL absolue (ici : `scheme://autorite...`).
/// 2. Le protocole doit etre `http:` ou `https:` (comparaison insensible a la casse).
/// 3. Sinon, renvoie `OpenError` avec le message d'origine.
///
/// La normalisation imite `new URL(input).href` au minimum : scheme en
/// minuscules, et ajout d'un `/` final quand il n'y a ni chemin, ni requete,
/// ni fragment (ex : `https://example.com` devient `https://example.com/`).
pub fn normalize_open_url(input: &str) -> Result<String, OpenError> {
    let fail = || OpenError::not_openable(input);

    // `URL.canParse("")` vaut faux : chaine vide rejetee.
    if input.is_empty() {
        return Err(fail());
    }
    // Un controle ASCII (saut de ligne, tabulation, ...) rend l'URL invalide.
    if input.chars().any(|c| c.is_control()) {
        return Err(fail());
    }

    // Decoupe `scheme : reste`. Il faut un `:` suivi de `//`.
    let colon = input.find(':').ok_or_else(&fail)?;
    let scheme = &input[..colon];
    let after = &input[colon + 1..];
    if scheme.is_empty() || !after.starts_with("//") {
        // URL relative (`/foo`, `example.com`, ...) : `URL.canParse` vaut faux.
        return Err(fail());
    }
    // Le scheme suit `[A-Za-z][A-Za-z0-9+.-]*`, comme WHATWG.
    let mut chars = scheme.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return Err(fail()),
    }
    if !scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return Err(fail());
    }
    // Protocole : seulement http et https, sans tenir compte de la casse car
    // WHATWG minusculise le scheme avant comparaison.
    let lower = scheme.to_ascii_lowercase();
    if lower != "http" && lower != "https" {
        return Err(fail());
    }

    // Autorite = entre `://` et le premier `/`, `?` ou `#`. Elle ne doit pas
    // etre vide et ne doit pas contenir d'espace brut.
    let rest = &after[2..];
    let end = rest.find(|c| c == '/' || c == '?' || c == '#').unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() {
        return Err(fail());
    }
    if authority.chars().any(|c| c.is_control() || c == ' ' || c == '\t') {
        return Err(fail());
    }

    // Normalisation legere de `href` : scheme en minuscules + `/` final si
    // l'URL n'a ni chemin, ni requete, ni fragment.
    let mut out = String::with_capacity(input.len() + 1);
    out.push_str(&lower);
    out.push_str("://");
    out.push_str(rest);
    if end == rest.len() {
        out.push('/');
    }
    Ok(out)
}

/// Effet d'ouverture du navigateur, a implementer par l'appelant.
///
/// Equivalent du `open(url.href)` du TypeScript. Une implementation typique
/// lance la commande systeme (`xdg-open`, `open`, `cmd /c start`, ...).
/// Les tests fournissent un mock en memoire.
pub trait UrlOpener {
    /// Ouvre `url` (deja normalisee par `normalize_open_url`).
    fn open(&self, url: &str) -> Result<(), OpenError>;
}

/// Valide `input` puis l'ouvre via `opener`.
///
/// Fonction synchrone : pas d'`async`, pas de runtime, juste valider puis
/// deleguer. Toute entree non http/https renvoie `OpenError` sans jamais
/// appeler `opener`.
pub fn open_url(input: &str, opener: &dyn UrlOpener) -> Result<(), OpenError> {
    let url = normalize_open_url(input)?;
    opener.open(&url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Mock en memoire qui enregistre la derniere URL ouverte.
    struct MockOpener {
        opened: RefCell<Vec<String>>,
    }

    impl MockOpener {
        fn new() -> Self {
            Self {
                opened: RefCell::new(Vec::new()),
            }
        }
    }

    impl UrlOpener for MockOpener {
        fn open(&self, url: &str) -> Result<(), OpenError> {
            self.opened.borrow_mut().push(url.to_string());
            Ok(())
        }
    }

    #[test]
    fn une_entree_vide_est_rejetee_avec_le_message_d_origine() {
        let err = normalize_open_url("").unwrap_err();
        assert_eq!(
            err.message(),
            "Only http and https links can be opened in the browser: "
        );
    }

    #[test]
    fn une_chaine_qui_n_est_pas_une_url_est_rejetee() {
        // Cas limite singleton : un seul mot sans scheme ni `://`.
        let err = normalize_open_url("pas-une-url").unwrap_err();
        assert!(err.message().ends_with("pas-une-url"));
        // Les URL relatives sont rejetees comme `URL.canParse` les rejette.
        assert!(normalize_open_url("/juste/un/chemin").is_err());
        assert!(normalize_open_url("example.com").is_err());
    }

    #[test]
    fn un_protocole_non_http_est_rejetee_et_n_ouvre_rien() {
        // Cas inverse : scheme valide mais protocole interdit.
        let mock = MockOpener::new();
        for input in ["ftp://example.com/fichier", "file:///etc/hosts", "javascript:alert(1)"] {
            let res = open_url(input, &mock);
            assert!(res.is_err(), "aurait du rejeter : {input}");
        }
        assert!(mock.opened.borrow().is_empty());
    }

    #[test]
    fn un_lien_https_simple_est_accepte_et_normalise() {
        // `new URL("https://example.com").href` ajoute le `/` final.
        assert_eq!(
            normalize_open_url("https://example.com").unwrap(),
            "https://example.com/"
        );
        // Un chemin existant est conserve tel quel.
        assert_eq!(
            normalize_open_url("http://example.com/a?b=1#frag").unwrap(),
            "http://example.com/a?b=1#frag"
        );
    }

    #[test]
    fn le_scheme_insensible_a_la_casse_est_accepte() {
        // WHATWG minusculise le protocole avant comparaison.
        assert_eq!(
            normalize_open_url("HTTP://example.com").unwrap(),
            "http://example.com/"
        );
        assert_eq!(
            normalize_open_url("Https://example.com/x").unwrap(),
            "https://example.com/x"
        );
    }

    #[test]
    fn une_autorite_vide_est_rejetee() {
        assert!(normalize_open_url("https://").is_err());
        assert!(normalize_open_url("http:///chemin").is_err());
    }

    #[test]
    fn open_url_transmet_l_url_normalisee_a_l_opener() {
        let mock = MockOpener::new();
        open_url("https://example.com", &mock).unwrap();
        assert_eq!(mock.opened.borrow().as_slice(), ["https://example.com/"]);
    }
}
```

===FIN===

CONFIANCE : moyenne
POINT FAIBLE : la normalisation WHATWG complete (minuscules de l'hote, port par defaut, encodage du chemin) n'est pas reproduite, seul le scheme et le "/" final le sont ; et aucune crate d'ouverture systeme n'est appelee, l'effet passe par le trait UrlOpener a cabler.
A VERIFIER : confirmer que l'absence de la crate `url` est voulue et que le trait UrlOpener convient comme point de branchement pour la commande systeme.
