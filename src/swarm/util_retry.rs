//! Portage de `packages/core/src/util/retry.ts`.
//!
//! La source fait 42 lignes : une interface d'options, la liste des messages
//! d'erreur consignes comme transitoires, la fonction qui les reconnait, et la
//! boucle de reessai.
//!
//! ```ts
//! export interface RetryOptions {
//!   attempts?: number
//!   delay?: number
//!   factor?: number
//!   maxDelay?: number
//!   retryIf?: (error: unknown) => boolean
//! }
//!
//! export async function retry<T>(fn: () => Promise<T>, options: RetryOptions = {}): Promise<T> {
//!   const { attempts = 3, delay = 500, factor = 2, maxDelay = 10000, retryIf = isTransientError } = options
//!
//!   let lastError: unknown
//!   for (let attempt = 0; attempt < attempts; attempt++) {
//!     try {
//!       return await fn()
//!     } catch (error) {
//!       lastError = error
//!       if (attempt === attempts - 1 || !retryIf(error)) throw error
//!       const wait = Math.min(delay * Math.pow(factor, attempt), maxDelay)
//!       await new Promise((resolve) => setTimeout(resolve, wait))
//!     }
//!   }
//!   throw lastError
//! }
//! ```
//!
//! Trois decisions a lire avant de relire ce fichier.
//!
//! 1. `attempts` est un `f64`, pas un entier. La source s'en sert deux fois :
//!    comme borne de boucle (`attempt < attempts`) et comme soustraite
//!    (`attempt === attempts - 1`). `attempts: 0` et `attempts: NaN` font
//!    tous les deux echouer l'appel sans jamais invoquer `fn`, mais
//!    `attempts: 2.5` invoque `fn` trois fois et dort trois fois. Un entier
//!    en Rust devrait choisir un entier pour representer `NaN`, donc le `f64`
//!    garde le comportement de la source sans cas particulier.
//!
//! 2. L'attente est injectee par le trait [`Sleeper`] au lieu d'etre ecrite
//!    dans la boucle. C'est ce qui permet de tester le nombre d'appels et la
//!    suite des delais sans jamais dormir : aucun test de ce fichier n'attend
//!    une seule milliseconde. La temporisation reelle, elle, existe dans
//!    [`ThreadSleeper`].
//!
//! 3. La boucle se termine par [`RetryFailure`] et non par le type d'erreur de
//!    l'operation. Quand `attempts` vaut 0 ou `NaN`, la source sort de sa
//!    boucle et execute `throw lastError` alors que `lastError` n'a jamais ete
//!    affecte : elle leve donc `undefined`. Il n'existe pas d'image de "aucune
//!    valeur" dans un `Result<T, E>`, alors la variante `NoAttempt` rend le
//!    cas explicite au lieu de fabriquer une erreur qui n'a pas eu lieu.
//!
//! Aucun struct n'est serialise dans ce fichier, donc aucun nom de champ et
//! aucun `serde(rename)` : le piege des majuscules de `projectID` n'a pas ou
//! s'appliquer ici.

/// Les messages qui font qualifier une erreur de transitoire.
///
/// L'ordre est celui de la source, et il est significatif seulement pour la
/// comparaison : `isTransientMessage` teste une inclusion, pas une egalite.
pub const TRANSIENT_MESSAGES: [&str; 8] = [
    "load failed",
    "network connection was lost",
    "network request failed",
    "failed to fetch",
    "econnreset",
    "econnrefused",
    "etimedout",
    "socket hang up",
];

/// Corps de `isTransientError`, une fois le message obtenu.
///
/// La source ecrit `String(error instanceof Error ? error.message : error)` puis
/// `.toLowerCase()`, et cherche un des marqueurs par `includes`. La
/// normalisation en minuscules vit ici, pas dans `retry`, parce que la
/// normalisation de la source fait partie du predicat par defaut et non de la
/// boucle.
pub fn is_transient_message(message: &str) -> bool {
    let message = message.to_lowercase();
    TRANSIENT_MESSAGES.iter().any(|marqueur| message.contains(marqueur))
}

/// `isTransientError(error)` de la source, dont le parametre est `unknown`.
///
/// Rust n'a pas d'equivalent de `unknown`, donc les deux formes que la source
/// distingue reellement sont exposees ici :
///
/// - `None` represente `null` et `undefined`, les seules valeurs pour
///   lesquelles `if (!error) return false` s'arrete avant meme de chercher un
///   message.
/// - `Some(message)` represente le message deja obtenu.
///
/// Le test de veracite `!error` n'est donc imite que partiellement, et c'est
/// sans consequence : les autres valeurs fausses de JavaScript (`0`, `-0`,
/// `NaN`, `""`, `false`, `0n`) ont pour forme textuelle `"0"`, `"NaN"`, `""`,
/// `"false"` et `"undefined"`, dont aucune ne contient un marqueur transitoire.
/// Toutes passeraient donc le test de veracite et retomberaient ensuite sur la
/// meme reponse `false`.
pub fn is_transient_error(message: Option<&str>) -> bool {
    match message {
        None => false,
        Some(message) => is_transient_message(message),
    }
}

/// Le `retryIf` par defaut de la source, c'est-a-dire `isTransientError`.
///
/// Le type de la source est `(error: unknown) => boolean`, donc un predicat
/// libre qui recoit l'erreur entiere. Ici le predicat recoit le message, parce
/// que c'est la seule partie de l'erreur que le defaut lit et qu'un
/// `Display` n'offre pas de moyen generique d'obtenir `instanceof Error` puis
/// `.message`.
pub fn default_retry_if(message: &str) -> bool {
    is_transient_error(Some(message))
}

/// `attempts = 3` dans la source.
pub const DEFAULT_ATTEMPTS: f64 = 3.0;
/// `delay = 500` dans la source, en millisecondes.
pub const DEFAULT_DELAY_MS: f64 = 500.0;
/// `factor = 2` dans la source.
pub const DEFAULT_FACTOR: f64 = 2.0;
/// `maxDelay = 10000` dans la source, en millisecondes.
pub const DEFAULT_MAX_DELAY_MS: f64 = 10_000.0;

/// Les options de `retry`, sous la forme exacte de l'interface TypeScript :
/// tous les champs sont optionnels, et `None` signifie `undefined`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RetryOptions {
    pub attempts: Option<f64>,
    pub delay: Option<f64>,
    pub factor: Option<f64>,
    pub max_delay: Option<f64>,
    pub retry_if: Option<fn(&str) -> bool>,
}

impl RetryOptions {
    /// Les options vides, c'est-a-dire l'argument par defaut `{}`.
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attempts(mut self, attempts: f64) -> Self {
        self.attempts = Some(attempts);
        self
    }

    pub fn delay(mut self, delay: f64) -> Self {
        self.delay = Some(delay);
        self
    }

    pub fn factor(mut self, factor: f64) -> Self {
        self.factor = Some(factor);
        self
    }

    pub fn max_delay(mut self, max_delay: f64) -> Self {
        self.max_delay = Some(max_delay);
        self
    }

    pub fn retry_if(mut self, retry_if: fn(&str) -> bool) -> Self {
        self.retry_if = Some(retry_if);
        self
    }
}

/// Les options apres application des valeurs par defaut de la source, c'est-a-
/// dire le resultat de `RetrySettings::resolve`.
#[derive(Debug, Clone, Copy)]
pub struct RetrySettings {
    pub attempts: f64,
    pub delay: f64,
    pub factor: f64,
    pub max_delay: f64,
    pub retry_if: fn(&str) -> bool,
}

impl RetrySettings {
    /// Applique `{ attempts = 3, delay = 500, factor = 2, maxDelay = 10000,
    /// retryIf = isTransientError }`.
    ///
    /// Ces valeurs par defaut de la destruction JavaScript sont une substitution
    /// de `undefined`, donc une coalescence sur la NULLITE, pas sur la
    /// veracite. Une option presentee qui vaut `0` est donc conservee : ici
    /// `attempts: Some(0.0)` donne `0.0` et non `3.0`, et
    /// `delay: Some(0.0)` donne `0.0` et non `500.0`. Un test de veracite
    /// appliede avant le defaut aurait mange ces deux zeros, ce qui serait une
    /// autre fonction.
    pub fn resolve(options: &RetryOptions) -> Self {
        Self {
            attempts: options.attempts.unwrap_or(DEFAULT_ATTEMPTS),
            delay: options.delay.unwrap_or(DEFAULT_DELAY_MS),
            factor: options.factor.unwrap_or(DEFAULT_FACTOR),
            max_delay: options.max_delay.unwrap_or(DEFAULT_MAX_DELAY_MS),
            retry_if: options.retry_if.unwrap_or(default_retry_if as fn(&str) -> bool),
        }
    }
}

/// `Math.min` de JavaScript.
///
/// La difference avec `f64::min` de Rust est unique et volontaire : quand un
/// operande vaut `NaN`, `Math.min` renvoie `NaN` alors que `f64::min` renvoie
/// l'autre operande. C'est observable ici, donc c'est conserve.
fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a < b {
        a
    } else {
        b
    }
}

/// Le delai d'attente avant la tentative suivante :
/// `Math.min(delay * Math.pow(factor, attempt), maxDelay)`.
///
/// Fonction pure, sans boucle et sans horloge : c'est elle qu'il faut appeler
/// pour tester la courbe de backoff.
///
/// Deux details de la source sont conserves :
///
/// - le plafond n'est pas un plancher, donc un `delay` negatif est renvoye tel
///   quel (`Math.min(-100 * 1, 10000)` vaut `-100`) ;
/// - `Math.pow` peut rendre `NaN` ou `Infinity`, le second etant ramene a
///   `max_delay` par le minimum, le premier restant `NaN`.
pub fn backoff_delay(attempt: f64, delay: f64, factor: f64, max_delay: f64) -> f64 {
    js_min(delay * factor.powf(attempt), max_delay)
}

/// L'attente reelle, injectee dans [`retry`].
///
/// La source ecrit `await new Promise((resolve) => setTimeout(resolve, wait))`.
/// Le nom du delai est en millisecondes, comme `setTimeout`.
pub trait Sleeper {
    fn sleep_ms(&mut self, ms: f64);
}

/// Un [`Sleeper`] qui n'attend rien et ne note rien.
///
/// Utile pour un appelant qui veut la boucle de reessai sans ses delais, et
/// pour les tests qui ne regardent que le nombre d'appels.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoSleeper;

impl Sleeper for NoSleeper {
    fn sleep_ms(&mut self, _ms: f64) {
        // Volontairement vide.
    }
}

/// Le [`Sleeper`] qui attend, comme le `setTimeout` de la source.
///
/// C'est le seul endroit de ce fichier qui dort reellement. Aucun test de ce
/// fichier ne l'utilise : tous passent par un petit journal qui enregistre les
/// delais dans un `Vec` sans les attendre.
///
/// Les valeurs negatives, infinies et `NaN` sont ramenees a 0, comme le fait
/// `setTimeout` en JavaScript, et surtout pour que la conversion en `Duration`
/// ne puisse pas paniquer.
#[derive(Debug, Default, Clone, Copy)]
pub struct ThreadSleeper;

impl Sleeper for ThreadSleeper {
    fn sleep_ms(&mut self, ms: f64) {
        let ms = if ms.is_finite() && ms > 0.0 { ms } else { 0.0 };
        std::thread::sleep(std::time::Duration::from_micros((ms * 1000.0) as u64));
    }
}

/// Comment se termine [`retry`].
///
/// La source leve `error` (dernier ecueil rencontre) ou `lastError`, qui peut
/// valoir `undefined` si la boucle n'a jamais tourne.
#[derive(Debug, Clone, PartialEq)]
pub enum RetryFailure<E> {
    /// La boucle a echappe par un `throw error`, apres `invocations` appels de
    /// l'operation.
    Failed { error: E, invocations: u32 },
    /// La boucle n'a jamais tourne, donc la source leve `undefined` et
    /// `lastError` ne vaut rien. `attempts` est la valeur configuree, qui peut
    /// etre `0`, negative ou `NaN`.
    NoAttempt { attempts: f64 },
}

impl<E> RetryFailure<E> {
    /// L'erreur de l'operation, si l'operation a ete appelee au moins une fois.
    pub fn error(self) -> Option<E> {
        match self {
            RetryFailure::Failed { error, .. } => Some(error),
            RetryFailure::NoAttempt { .. } => None,
        }
    }
}

/// La boucle de reessai de la source.
///
/// `operation` est l'appel a reessayer, comme `fn` dans la source : il ne prend
/// pas d'argument, et le numero de tentative ne lui est pas transmis non plus,
/// exactement comme en JavaScript.
///
/// La temporisation passe par `sleeper`, ce qui rend la fonction testable sans
/// horloge : un [`Sleeper`] de test peut enregistrer la suite des delais au lieu
/// de les attendre.
///
/// Le `Display` de `E` sert a construire le message passe a `retry_if`, comme
/// `String(error)` de la source. Pour une erreur qui implemente `std::error::Error`,
/// `to_string` rend le message, pas `Error: message`, ce qui correspond bien a
/// la branche `error instanceof Error ? error.message : error`.
pub fn retry<T, E, F, S>(
    mut operation: F,
    options: &RetryOptions,
    sleeper: &mut S,
) -> Result<T, RetryFailure<E>>
where
    F: FnMut() -> Result<T, E>,
    E: std::fmt::Display,
    S: Sleeper + ?Sized,
{
    let settings = RetrySettings::resolve(options);

    let mut last_error: Option<E> = None;
    let mut invocations: u32 = 0;
    let mut attempt: f64 = 0.0;

    while attempt < settings.attempts {
        let resultat = operation();
        invocations += 1;
        match resultat {
            Ok(valeur) => return Ok(valeur),
            Err(erreur) => {
                // Le `||` court-circuite comme en JavaScript : sur la derniere
                // tentative, `retryIf` n'est pas appele du tout et aucune attente
                // n'est demandee.
                if attempt == settings.attempts - 1.0 || !(settings.retry_if)(&erreur.to_string()) {
                    return Err(RetryFailure::Failed { error: erreur, invocations });
                }
                last_error = Some(erreur);
                let wait = backoff_delay(
                    attempt,
                    settings.delay,
                    settings.factor,
                    settings.max_delay,
                );
                sleeper.sleep_ms(wait);
            }
        }
        attempt += 1.0;
    }

    match last_error {
        Some(error) => Err(RetryFailure::Failed { error, invocations }),
        None => Err(RetryFailure::NoAttempt {
            attempts: settings.attempts,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        backoff_delay, default_retry_if, is_transient_error, is_transient_message,
        retry, NoSleeper, RetryFailure, RetryOptions, RetrySettings, Sleeper, ThreadSleeper,
        TRANSIENT_MESSAGES, DEFAULT_ATTEMPTS, DEFAULT_DELAY_MS, DEFAULT_FACTOR,
        DEFAULT_MAX_DELAY_MS,
    };

    /// Un `retryIf` qui refuse systematiquement, ecrit en fonction pour ne pas
    /// dependre d'une conversion de closure en pointeur de fonction.
    fn jamais(message: &str) -> bool {
        let _ = message;
        false
    }

    /// Un `retryIf` qui accepte systematiquement, pour verifier que la derniere
    /// tentative arrete la boucle quand meme.
    fn toujours(message: &str) -> bool {
        let _ = message;
        true
    }

    /// Un `retryIf` qui ne reconnait qu'un message precis : il prouve que le
    /// message de l'erreur, et non la structure de l'erreur, est transmis.
    fn seulement_reseau(message: &str) -> bool {
        message.contains("reseau")
    }

    /// Enregistre les delais au lieu de les attendre. Aucun thread, aucune
    /// horloge, aucun temps reel.
    #[derive(Debug, Default)]
    struct Journal {
        delais: Vec<f64>,
    }

    impl Sleeper for Journal {
        fn sleep_ms(&mut self, ms: f64) {
            self.delais.push(ms);
        }
    }

    /// Une erreur d'operation minimale, avec un message au choix.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Echec(String);

    impl std::fmt::Display for Echec {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.0)
        }
    }

    // ------------------------------------------------------------------ liste

    #[test]
    fn la_liste_des_marqueurs_est_celle_de_la_source_dans_ordre() {
        assert_eq!(
            TRANSIENT_MESSAGES,
            [
                "load failed",
                "network connection was lost",
                "network request failed",
                "failed to fetch",
                "econnreset",
                "econnrefused",
                "etimedout",
                "socket hang up",
            ]
        );
    }

    // ------------------------------------------------- reconnaissance d'erreur

    #[test]
    fn chacun_des_huit_marqueurs_est_reconnu_dans_son_texte() {
        for marqueur in TRANSIENT_MESSAGES {
            assert!(is_transient_message(marqueur), "marqueur non reconnu : {}", marqueur);
        }
    }

    #[test]
    fn le_message_est_compare_en_minuscules() {
        assert!(is_transient_message("NETWORK REQUEST FAILED"));
        assert!(is_transient_message("EconnReset"));
        assert!(is_transient_message("Socket Hang Up"));
        assert!(is_transient_message("FETCH FAILED"));
    }

    #[test]
    fn le_marqueur_compte_meme_au_milieu_d_un_texte_plus_long() {
        assert!(is_transient_message("Error: read ECONNRESET at 12.0.0.1:443"));
        assert!(is_transient_message("request failed to fetch after 3 tries"));
    }

    #[test]
    fn un_message_non_transitoire_est_refuse() {
        assert!(!is_transient_message("boom"));
        assert!(!is_transient_message("internal server error"));
        assert!(!is_transient_message("401 unauthorized"));
        assert!(!is_transient_message("connection closed by peer"));
    }

    #[test]
    fn un_http_500_n_est_pas_transitoire_pour_la_source() {
        // La source ne liste que huit messages, aucun code HTTP n'y est.
        assert!(!is_transient_message("500 Internal Server Error"));
        assert!(!is_transient_message("502 Bad Gateway"));
        assert!(!is_transient_message("503 Service Unavailable"));
        assert!(!is_transient_message("504 Gateway Timeout"));
    }

    #[test]
    fn une_erreur_absente_n_est_jamais_transitoire() {
        assert!(!is_transient_error(None));
    }

    #[test]
    fn le_message_vide_est_non_transitoire() {
        assert!(!is_transient_error(Some("")));
        assert!(!is_transient_message(""));
    }

    #[test]
    fn les_valeurs_fausies_de_javascript_tombent_toutes_en_faux() {
        // `!error` est vrai pour ces valeurs, mais leur forme textuelle ne peut
        // contenir aucun marqueur : la source les rejetterait de toute facon.
        let valeurs_fausies = ["", "0", "-0", "NaN", "false", "null", "undefined", "0n"];
        for forme in valeurs_fausies {
            assert!(!is_transient_error(Some(forme)), "forme textuelle acceptee : {}", forme);
        }
    }

    #[test]
    fn le_test_de_veracite_de_la_source_est_inobservable() {
        // C'est la justification de `is_transient_error` : si le court-circuit
        // `if (!error) return false` ne change rien, on peut le modeliser par un
        // simple `Option<&str>`. Le court-circuit renvoie `false` pour toutes
        // ces formes, et la suite du code les renvoie aussi parce qu'aucune ne
        // contient un marqueur transitoire.
        let formes = ["", "0", "-0", "NaN", "false", "null", "undefined"];
        let avec_court_circuit = vec![false; 7];
        let sans_court_circuit: Vec<bool> = formes.iter().copied().map(is_transient_message).collect();
        assert_eq!(avec_court_circuit, sans_court_circuit);
    }

    #[test]
    fn le_predicat_par_defaut_delegue_a_is_transient_error() {
        assert!(default_retry_if("failed to fetch"));
        assert!(!default_retry_if("boom"));
        assert!(default_retry_if("ETIMEDOUT") == is_transient_error(Some("ETIMEDOUT")));
    }

    // ------------------------------------------------------ calcul du delai

    #[test]
    fn le_backoff_par_defaut_double_a_partir_de_500_ms() {
        assert_eq!(backoff_delay(0.0, 500.0, 2.0, 10_000.0), 500.0);
        assert_eq!(backoff_delay(1.0, 500.0, 2.0, 10_000.0), 1000.0);
        assert_eq!(backoff_delay(2.0, 500.0, 2.0, 10_000.0), 2000.0);
        assert_eq!(backoff_delay(3.0, 500.0, 2.0, 10_000.0), 4000.0);
        assert_eq!(backoff_delay(4.0, 500.0, 2.0, 10_000.0), 8000.0);
    }

    #[test]
    fn le_backoff_avec_les_constantes_du_module_vaut_la_source() {
        let delais: Vec<f64> = (0..6)
            .map(|tentative| {
                backoff_delay(
                    f64::from(tentative),
                    DEFAULT_DELAY_MS,
                    DEFAULT_FACTOR,
                    DEFAULT_MAX_DELAY_MS,
                )
            })
            .collect();
        assert_eq!(delais, vec![500.0, 1000.0, 2000.0, 4000.0, 8000.0, 10_000.0]);
    }

    #[test]
    fn le_delai_est_plafonne_par_max_delay() {
        assert_eq!(backoff_delay(5.0, 500.0, 2.0, 10_000.0), 10_000.0);
        assert_eq!(backoff_delay(50.0, 500.0, 2.0, 10_000.0), 10_000.0);
        assert_eq!(backoff_delay(1.0, 500.0, 10.0, 1_000.0), 1_000.0);
    }

    #[test]
    fn un_exposant_qui_derive_en_infinite_atterrit_sur_le_plafond() {
        // `Math.pow(2, 5000)` vaut `Infinity` en JavaScript comme en Rust, et
        // le minimum le ramene a `max_delay`.
        assert_eq!(backoff_delay(5000.0, 500.0, 2.0, 10_000.0), 10_000.0);
    }

    #[test]
    fn un_max_delay_nan_reste_nan_comme_math_min() {
        // C'est la seule divergence entre `js_min` et `f64::min` : ce dernier
        // renverrait 500.0.
        assert!(backoff_delay(0.0, 500.0, 2.0, f64::NAN).is_nan());
    }

    #[test]
    fn un_facteur_nan_ou_une_puissance_impossible_donne_nan() {
        assert!(backoff_delay(1.0, 500.0, f64::NAN, 10_000.0).is_nan());
        assert!(backoff_delay(0.5, 500.0, -1.0, 10_000.0).is_nan());
    }

    #[test]
    fn un_delai_negatif_est_conserve_par_le_minimum_de_la_source() {
        assert_eq!(backoff_delay(0.0, -100.0, 2.0, 10_000.0), -100.0);
        assert_eq!(backoff_delay(0.0, 0.0, 2.0, 10_000.0), 0.0);
    }

    #[test]
    fn un_facteur_negatif_avec_un_exposant_entier_est_calcule() {
        assert_eq!(backoff_delay(1.0, 100.0, -2.0, 10_000.0), -200.0);
    }

    // --------------------------------------------------------------- options

    #[test]
    fn les_defauts_sont_ceux_de_la_source() {
        let settings = RetrySettings::resolve(&RetryOptions::new());
        assert_eq!(settings.attempts, 3.0);
        assert_eq!(settings.delay, 500.0);
        assert_eq!(settings.factor, 2.0);
        assert_eq!(settings.max_delay, 10_000.0);
        assert!((settings.retry_if)("failed to fetch"));
        assert!(!(settings.retry_if)("boom"));
    }

    #[test]
    fn les_constantes_de_defaut_valent_les_litteraux_de_la_source() {
        assert_eq!(DEFAULT_ATTEMPTS, 3.0);
        assert_eq!(DEFAULT_DELAY_MS, 500.0);
        assert_eq!(DEFAULT_FACTOR, 2.0);
        assert_eq!(DEFAULT_MAX_DELAY_MS, 10_000.0);
    }

    #[test]
    fn une_option_presente_remplace_son_defaut() {
        let settings = RetrySettings::resolve(
            &RetryOptions::new()
                .attempts(7.0)
                .delay(25.0)
                .factor(3.0)
                .max_delay(400.0)
                .retry_if(jamais),
        );
        assert_eq!(settings.attempts, 7.0);
        assert_eq!(settings.delay, 25.0);
        assert_eq!(settings.factor, 3.0);
        assert_eq!(settings.max_delay, 400.0);
        assert!(!(settings.retry_if)("failed to fetch"));
    }

    #[test]
    fn une_option_a_zero_survit_au_defaut_au_lieu_de_le_remplacer() {
        // La destruction `{ attempts = 3 }` teste `undefined`, donc la NULLITE.
        // Un test de veracite aurait remplace 0.0 par 3.0 pour `attempts` et par
        // 500.0 pour `delay`.
        let settings = RetrySettings::resolve(&RetryOptions::new().attempts(0.0).delay(0.0));
        assert_eq!(settings.attempts, 0.0, "0 doit survivre, pas 3");
        assert_eq!(settings.delay, 0.0, "0 doit survivre, pas 500");
    }

    #[test]
    fn une_option_negative_ou_nulle_survit_au_defaut() {
        // Mememe piege de coalescence : `-0.0` est present, donc il n'est pas
        // remplace par 10000.0, et il survit au plancher de `js_min` comme le
        // ferait `Math.min` en JavaScript.
        let settings = RetrySettings::resolve(&RetryOptions::new().max_delay(-0.0));
        assert!(settings.max_delay.is_sign_negative());
        assert_eq!(settings.max_delay, 0.0);
        assert_eq!(backoff_delay(0.0, -1.0, 2.0, settings.max_delay), -1.0);
    }

    #[test]
    fn une_option_absente_et_une_option_a_none_sont_la_meme_chose() {
        let settings = RetrySettings::resolve(&RetryOptions::default());
        assert_eq!(settings.attempts, 3.0);
        assert_eq!(settings.delay, 500.0);
        assert_eq!(settings.factor, 2.0);
        assert_eq!(settings.max_delay, 10_000.0);
    }

    // --------------------------------------------------------- boucle de reessai

    #[test]
    fn un_succes_immediat_ne_dort_rien_et_n_appelle_qu_une_fois() {
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                Ok::<i32, Echec>(42)
            },
            &RetryOptions::new(),
            &mut journal,
        );
        assert_eq!(resultat, Ok(42));
        assert_eq!(appels, 1);
        assert!(journal.delais.is_empty());
    }

    #[test]
    fn un_premier_echec_transitoire_dort_500_ms_avant_la_tentative_suivante() {
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                if appels == 1 {
                    Err(Echec("failed to fetch".to_string()))
                } else {
                    Ok("ok")
                }
            },
            &RetryOptions::new(),
            &mut journal,
        );
        assert_eq!(resultat, Ok("ok"));
        assert_eq!(appels, 2);
        assert_eq!(journal.delais, vec![500.0]);
    }

    #[test]
    fn une_erreur_non_transitoire_arrete_immediatement_sans_dormir() {
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                Err::<&str, _>(Echec("boom".to_string()))
            },
            &RetryOptions::new(),
            &mut journal,
        );
        assert_eq!(
            resultat,
            Err(RetryFailure::Failed {
                error: Echec("boom".to_string()),
                invocations: 1
            })
        );
        assert_eq!(appels, 1);
        assert!(journal.delais.is_empty());
    }

    #[test]
    fn une_erreur_transitoire_epuise_les_trois_tentatives_avec_deux_dormies() {
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                Err::<(), _>(Echec("network request failed".to_string()))
            },
            &RetryOptions::new(),
            &mut journal,
        );
        match resultat {
            Err(RetryFailure::Failed { invocations, error }) => {
                assert_eq!(invocations, 3);
                assert_eq!(error, Echec("network request failed".to_string()));
            }
            autre => panic!("echec attendu, obtenu {:?}", autre),
        }
        assert_eq!(appels, 3);
        assert_eq!(journal.delais, vec![500.0, 1000.0]);
    }

    #[test]
    fn cinq_tentatives_donnent_quatre_dormies_avec_le_plafond_au_quatrieme_delai() {
        let mut journal = Journal::default();
        let resultat = retry(
            || Err::<(), _>(Echec("etimedout".to_string())),
            &RetryOptions::new().attempts(5.0).delay(500.0).factor(2.0).max_delay(3_000.0),
            &mut journal,
        );
        assert_eq!(
            resultat.unwrap_err().error(),
            Some(Echec("etimedout".to_string()))
        );
        assert_eq!(journal.delais, vec![500.0, 1000.0, 2000.0, 3_000.0]);
    }

    #[test]
    fn une_seule_tentative_ne_dort_jamais() {
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                Err::<(), _>(Echec("load failed".to_string()))
            },
            &RetryOptions::new().attempts(1.0),
            &mut journal,
        );
        assert_eq!(appels, 1);
        assert_eq!(resultat.unwrap_err().error(), Some(Echec("load failed".to_string())));
        assert!(journal.delais.is_empty());
    }

    #[test]
    fn le_delai_de_la_tentative_suivante_suit_le_numero_de_tentative() {
        let mut journal = Journal::default();
        let _ = retry(
            || Err::<(), _>(Echec("econnrefused".to_string())),
            &RetryOptions::new().attempts(4.0).delay(10.0).factor(3.0),
            &mut journal,
        );
        assert_eq!(journal.delais, vec![10.0, 30.0, 90.0]);
    }

    #[test]
    fn un_predicat_personnalise_remplace_le_defaut() {
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                Err::<(), _>(Echec("load failed".to_string()))
            },
            &RetryOptions::new().attempts(5.0).retry_if(jamais),
            &mut journal,
        );
        assert_eq!(appels, 1, "un predicat qui refuse tout doit arreter la boucle");
        assert!(resultat.is_err());
        assert!(journal.delais.is_empty());
    }

    #[test]
    fn un_predicat_personnalise_recoit_le_message_de_l_erreur() {
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                Err::<(), _>(Echec("panne reseau".to_string()))
            },
            &RetryOptions::new().retry_if(seulement_reseau),
            &mut journal,
        );
        assert_eq!(appels, 3, "le message de l'erreur atteint bien le predicat");
        assert_eq!(journal.delais, vec![500.0, 1000.0]);
        assert!(resultat.is_err());
    }

    #[test]
    fn un_predicat_qui_accepte_tout_ne_prolonge_pas_la_derniere_tentative() {
        // La condition est `attempt === attempts - 1 || !retryIf(error)`. Avec un
        // predicat qui accepte tout, seul le court-circuit du `||` peut arreter
        // la derniere tentative : une inversion des deux termes ajouterait une
        // attente de 2000 ms qui n'existe pas dans la source.
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                Err::<(), _>(Echec("boom".to_string()))
            },
            &RetryOptions::new().attempts(3.0).retry_if(toujours),
            &mut journal,
        );
        assert_eq!(appels, 3);
        assert_eq!(journal.delais, vec![500.0, 1000.0]);
        assert_eq!(resultat.unwrap_err().error(), Some(Echec("boom".to_string())));
    }

    #[test]
    fn le_predicat_par_defaut_reconnait_un_message_mis_en_majuscules() {
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let _ = retry(
            || {
                appels += 1;
                Err::<(), _>(Echec("NETWORK CONNECTION WAS LOST".to_string()))
            },
            &RetryOptions::new(),
            &mut journal,
        );
        assert_eq!(appels, 3);
        assert_eq!(journal.delais, vec![500.0, 1000.0]);
    }

    // --------------------------------------------------- cas degeneres de la boucle

    #[test]
    fn zero_tentative_echoue_sans_jamais_appeler_l_operation() {
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                Ok::<i32, Echec>(1)
            },
            &RetryOptions::new().attempts(0.0),
            &mut journal,
        );
        // `NoAttempt` carries `attempts: f64`, and `PartialEq` on `f64` gives
// `NaN != NaN` and `-0.0 == 0.0`. That matches the JavaScript `===` the
// source uses, BUT it means an `assert_eq!` on this variant silently fails
// whenever `attempts` is `NaN`, and the module doc lists `NaN` as a supported
// state. Matching on the discriminant, then comparing the field separately,
// is therefore the honest form: it asserts the variant without depending on
// how a float compares to itself.
let echec = resultat.unwrap_err();
match echec {
    RetryFailure::NoAttempt { attempts } => {
        assert_eq!(attempts, 0.0, "zero attempts, so attempts must be 0");
    }
    autre => panic!("expected NoAttempt, got {autre:?}"),
}
        assert_eq!(appels, 0);
        assert!(journal.delais.is_empty());
        assert_eq!(resultat.unwrap_err().error(), None);
    }

    #[test]
    fn un_nombre_de_tentatives_negatif_ou_nan_echoue_sans_appeler_l_operation() {
        for configure in [-1.0, f64::NAN, f64::NEG_INFINITY] {
            let mut journal = Journal::default();
            let mut appels = 0u32;
            let resultat = retry(
                || {
                    appels += 1;
                    Ok::<i32, Echec>(1)
                },
                &RetryOptions::new().attempts(configure),
                &mut journal,
            );
            assert_eq!(appels, 0, "aucun appel pour {}", configure);
            assert!(matches!(resultat, Err(RetryFailure::NoAttempt { .. })));
            assert!(journal.delais.is_empty());
        }
    }

    #[test]
    fn un_nombre_de_tentatives_fractionnaire_sort_de_la_boucle_avec_la_derniere_erreur() {
        // `attempts = 2.5` : la condition `attempt < attempts` passe pour 0, 1
        // et 2, jamais pour 3, donc la source dort trois fois et leve ensuite
        // son `lastError`.
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                Err::<(), _>(Echec("econnreset".to_string()))
            },
            &RetryOptions::new().attempts(2.5),
            &mut journal,
        );
        assert_eq!(appels, 3);
        assert_eq!(journal.delais, vec![500.0, 1000.0, 2000.0]);
        assert_eq!(resultat.unwrap_err().error(), Some(Echec("econnreset".to_string())));
    }

    #[test]
    fn un_nombre_de_tentatives_fractionnaire_appelle_au_moins_une_fois() {
        // `attempts = 0.5` : `0 < 0.5` est vrai, donc l'operation est appelee
        // une fois, puis la boucle s'arrete et la source leve `lastError`.
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                Err::<(), _>(Echec("econnreset".to_string()))
            },
            &RetryOptions::new().attempts(0.5),
            &mut journal,
        );
        assert_eq!(appels, 1);
        assert_eq!(journal.delais, vec![500.0]);
        assert_eq!(resultat.unwrap_err().error(), Some(Echec("econnreset".to_string())));
    }

    #[test]
    fn un_nombre_de_tentatives_fractionnaire_avec_une_erreur_non_transitoire_ne_dort_pas() {
        let mut journal = Journal::default();
        let mut appels = 0u32;
        let resultat = retry(
            || {
                appels += 1;
                Err::<(), _>(Echec("boom".to_string()))
            },
            &RetryOptions::new().attempts(0.5),
            &mut journal,
        );
        assert_eq!(appels, 1);
        assert!(journal.delais.is_empty());
        assert_eq!(resultat.unwrap_err().error(), Some(Echec("boom".to_string())));
    }

    #[test]
    fn un_delai_de_zero_enregistre_des_attentes_nulles() {
        // `delay: 0` est une option presente, donc elle survit au defaut de 500
        // et la boucle demande quand meme trois fois une attente de zero.
        let mut journal = Journal::default();
        let resultat = retry(
            || Err::<(), _>(Echec("etimedout".to_string())),
            &RetryOptions::new().delay(0.0),
            &mut journal,
        );
        assert!(resultat.is_err());
        assert_eq!(journal.delais, vec![0.0, 0.0]);
    }

    // ------------------------------------------------------------- sleepers

    #[test]
    fn le_sleeper_inerte_accepte_une_valeur_absurde_sans_attendre() {
        let mut sleeper = NoSleeper;
        sleeper.sleep_ms(0.0);
        sleeper.sleep_ms(f64::NAN);
        sleeper.sleep_ms(f64::INFINITY);
        sleeper.sleep_ms(-1.0);
    }

    #[test]
    fn le_sleeper_reel_est_un_zero_par_defaut_il_ne_peut_pas_paniquer() {
        // On n'appelle surtout pas `sleep_ms` ici : ce test verifie seulement
        // que le type n'a aucun etat a perdre et qu'il se laisse construire.
        assert_eq!(std::mem::size_of::<ThreadSleeper>(), 0);
        let _ = ThreadSleeper::default();
    }

    #[test]
    fn le_journal_de_test_enregistre_sans_dormir() {
        let mut journal = Journal::default();
        journal.sleep_ms(1.5);
        journal.sleep_ms(-2.0);
        assert_eq!(journal.delais, vec![1.5, -2.0]);
    }
}
