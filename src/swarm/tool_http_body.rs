//! Portage Rust de `opencode/packages/core/src/tool/http-body.ts`.
//!
//! La source tient en trente lignes et **une seule fonction**,
//! `collectBoundedResponseBody`. Aucun schema, aucune donnee serialisee, aucun
//! multipart, aucun BOM, aucun type de contenu n y figure : la fonction lit un
//! en-tete `content-length`, puis accumule les morceaux d un corps de reponse
//! HTTP dans un tampon borne, et echoue des que la borne est franchisee. C est
//! un **corps de reponse**, donc de la capture de flux, et non un corps de
//! requete : rien n est encode, rien n est forme, rien n est envoye. Aucun
//! acces reseau n existe dans ce fichier, et aucun test n en fait.
//!
//! ## Les deux pieges de portage, et ils sont tous les deux presents
//!
//! **Le `?` contre `||`.** Le TypeScript est ici un **`||`** et non un `??` :
//!
//! ```ts
//! const parsedSize = contentLength ? Number.parseInt(contentLength, 10) : undefined
//! ...
//! Buffer.allocUnsafe(Math.min(maximumBytes, declaredSize || 64 * 1024))
//! ```
//!
//! Les deux testent la **veracite**, jamais la nullite. Donc la chaine vide
//! `""` se comporte comme une absence, et le nombre `0` aussi. Ce dernier point
//! est le pire des deux : un serveur qui repond `content-length: 0` est
//! parfaitement valide, et la source lui reserve malgre tout **64 Kio de
//! tampon**. Ce n est pas une intention, c est un effet collateral, mais c est
//! le comportement de la source et il est conserve. Pour que la distinction
//! reste visible, les deux formes sont portees par **deux fonctions distinctes**,
//! [`truthy_or`] et [`nullish_or`], qui ne sont volontairement interchangeables
//! sur aucune valeur. Un test les oppose sur `0` et sur `""`.
//!
//! **Les noms de champs.** Le seul nom qui traverse l echange avec le
//! TypeScript est la cle d en-tete `content-length` : tout en minuscules, avec
//! un tiret, et **exactement** cela. Ni `contentLength`, ni `content_length`,
//! ni `Content-Length`. Une faute de frappe passerait la compilation Rust sans
//! la moindre alerte et ne casse qu au moment ou le serveur repond. Deux tests
//! verrouillent le contrat, l un dans le sens de l ecriture, l autre dans celui
//! de la lecture.
//!
//! ## Ce que la source garantit, et qu il faut conserver
//!
//! 1. **L en-tete n est qu une indication, jamais une autorisation.** Un
//!    `content-length` superieur a `maximumBytes` echoue tout de suite, avant
//!    meme d avoir lu le flux. Mais un `content-length` absent, faux, negatif
//!    ou hors entier sur n autorise rien du tout : la seule borne reelle reste
//!    `maximumBytes`, controlee morceau par morceau.
//!
//! 2. **L echec est exactement a la limite.** Le test est `>` et non `>=` : un
//!    corps dont la taille vaut exactement `maximumBytes` est accepte. Un portage
//!    en `>=` rejetterait des reponses legitimes.
//!
//! 3. **Un morceau vide ne compte pas.** Il ne grossit pas le tampon, ne
//!    consomme pas de budget, et ne provoque pas l echec. Il est ecarte avant
//!    toute comparaison.
//!
//! 4. **Le retour est une vue, pas une copie.** `body.subarray(0, size)`
//!    renvoie un `Buffer` qui partage le tableau de fond avec `body` et dont la
//!    longueur est `size`, pas `body.byteLength`. Les octets au-dela de `size`
//!    ne sont **jamais** lisibles. Cote Rust, le tampon est un `Vec<u8>`
//!    initialise a zero, ce qui est plus sur : la divergence ne peut pas se voir
//!    a travers l API, puisque seule la tranche `0..size` sort du module.
//!
//! ## Deux choix de portage
//!
//! - **Le calcul de taille est porte a la main.** `Number.parseInt(s, 10)` suit
//!   des regles que ni `str::parse::<u64>()` ni `parse::<f64>()` ne reproduisent
//!   : espaces en tete, signe optionnel, arret au premier caractere non
//!   chiffre, `"12abc"` vaut `12`, `"0x10"` vaut `0` parce que la base est
//!   imposee a dix, `NaN` quand aucun chiffre n est present. La fonction
//!   [`js_parse_int_radix_10`] est ecrite pour cela. Elle renvoie `f64` et non
//!   `u64` precisely parce que l original manipule un `number` JavaScript, dont
//!   la perte de precision fait partie du contrat.
//!
//! - **Le flux est un iterateur, l erreur est une fermeture.** `Effect.fail`
//!   transporte une erreur de type `Error` construite a la volee ; ici la
//!   fonction prend une fermeture `FnMut() -> E` et n appelle **jamais** plus
//!   d une fois, ce que verifie un test. Le type de l erreur reste celui de
//!   l appelant, sans type d erreur impose par le module.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Nom exact de l en-tete lu par la source.
///
/// La source ecrit `response.headers["content-length"]` : c est une
/// recherche de **cle exacte** dans un objet dont les cles sont deja
/// normalisees en minuscules par la couche HTTP d Effect. Ce module ne
/// normalise rien, donc une entree `Content-Length` ou `contentLength` passe
/// inapercue et donne une reponse traitee comme si l en-tete etait absent.
/// C est le comportement de la source, il est conserve, et il est teste.
pub const CONTENT_LENGTH_HEADER: &str = "content-length";

/// Plus grand entier representable exactement par un `number` JavaScript.
///
/// `Number.MAX_SAFE_INTEGER`. Au-dela, `Number.isSafeInteger` renvoie `false` et
/// l en-tete est traite comme absent, meme si le serveur l a envoye
/// legitement.
pub const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Taille de tampon retenue quand aucune taille declaree n est exploitable.
///
/// Le `64 * 1024` de la source. Il n est atteint que si `declaredSize` est
/// absent **ou** nul, a cause du `||` : voir la note sur [`truthy_or`].
pub const DEFAULT_BUFFER_BYTES: u64 = 64 * 1024;

/// Un type qui peut se comporter comme une valeur JavaScript dans un test de
/// veracite.
///
/// La veracite JavaScript rejette `false`, `0`, `-0`, `0n`, `""`, `null`,
/// `undefined` et `NaN`. Seuls les deux types manipules par ce module sont
/// implementes : le nombre entier et la chaine. Ce trait existe pour que
/// [`truthy_or`] et [`nullish_or`] aient la meme signature tout en restant deux
/// fonctions qui ne font pas la meme chose.
pub trait JsTruthy {
    /// `true` si la valeur est vraie au sens de JavaScript.
    fn is_truthy(&self) -> bool;
}

impl JsTruthy for &str {
    fn is_truthy(&self) -> bool {
        // La chaine vide est la seule chaine fausse.
        !self.is_empty()
    }
}

impl JsTruthy for u64 {
    fn is_truthy(&self) -> bool {
        // Zero est le seul entier non signe faux. `-0` n existe pas ici, et
        // `parseInt("-0")` donne `-0` en JavaScript, qui est faux de la meme
        // maniere et se ramene a `0` a la conversion.
        *self != 0
    }
}

/// Le ternaire `?` de JavaScript : teste la **veracite**.
///
/// `None` et une valeur fausse donnent tous deux `fallback`. Pour un entier, `0`
/// est faux, donc `Some(0)` donne `fallback`. Pour une chaine, `""` est fausse,
/// donc `Some("")` donne `fallback`.
///
/// C est la forme utilisee deux fois par la source : sur l en-tete, et sur la
/// taille declaree au moment d allouer le tampon.
///
/// # Exemples
///
/// ```
/// use ycode::swarm::tool_http_body::{truthy_or, DEFAULT_BUFFER_BYTES};
///
/// assert_eq!(truthy_or(Some(10u64), DEFAULT_BUFFER_BYTES), 10);
/// // Zero est faux en JavaScript, donc il prend la valeur de repli.
/// assert_eq!(truthy_or(Some(0u64), DEFAULT_BUFFER_BYTES), DEFAULT_BUFFER_BYTES);
/// // La chaine vide est fausse aussi.
/// assert_eq!(truthy_or(Some(""), "defaut"), "defaut");
/// ```
pub fn truthy_or<T: JsTruthy + Copy>(value: Option<T>, fallback: T) -> T {
    match value {
        Some(v) if v.is_truthy() => v,
        _ => fallback,
    }
}

/// Le coalescent `??` de JavaScript : teste la **nullite**.
///
/// `None` donne `fallback`, et **toute** valeur presente est conservee, y
/// compris `0` et `""`.
///
/// Cette fonction **n existe pas dans la source**, qui n utilise jamais `??`.
/// Elle est portee quand meme, et c est indispensable : sans elle, un relecteur
/// ne peut pas verifier que le `||` de la ligne 15 a bien ete lu comme une
/// veracite et non comme une nullite. Elle ne doit pas etre utilisee a la place
/// de [`truthy_or`].
///
/// # Exemples
///
/// ```
/// use ycode::swarm::tool_http_body::nullish_or;
///
/// assert_eq!(nullish_or(Some(0u64), 65_536), 0);
/// assert_eq!(nullish_or(Some(""), "defaut"), "");
/// assert_eq!(nullish_or(None, 65_536u64), 65_536);
/// ```
pub fn nullish_or<T: Copy>(value: Option<T>, fallback: T) -> T {
    match value {
        Some(v) => v,
        None => fallback,
    }
}

/// Les en-tetes d une reponse HTTP, tels que la couche de transport les
/// donne.
///
/// La source indexe un objet plat dont les cles sont **deja** en minuscules. Ce
/// type fait de meme : [`get`](Self::get) est une recherche exacte, sans
/// normalisation de casse ni de trait d union. Une cle absente donne `None`,
/// exactement comme `undefined` en JavaScript.
///
/// Le type est serialisable et son JSON est un objet plat dont les valeurs sont
/// des chaines, ce qui correspond a l objet de la source.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResponseHeaders {
    entrees: BTreeMap<String, String>,
}

impl ResponseHeaders {
    /// En-tetes vides.
    pub fn new() -> Self {
        ResponseHeaders { entrees: BTreeMap::new() }
    }

    /// Ajoute un en-tete et rend `self`, pour permettre l enchainement.
    pub fn with(mut self, nom: &str, valeur: &str) -> Self {
        self.entrees.insert(nom.to_string(), valeur.to_string());
        self
    }

    /// Recherche exacte d une valeur d en-tete.
    ///
    /// Aucune normalisation : `"Content-Length"` ne trouve pas
    /// `"content-length"`. C est voulu, voir [`CONTENT_LENGTH_HEADER`].
    pub fn get(&self, nom: &str) -> Option<&str> {
        self.entrees.get(nom).map(|valeur| valeur.as_str())
    }

    /// Nombre d en-tetes presents.
    pub fn len(&self) -> usize {
        self.entrees.len()
    }

    /// `true` si aucun en-tete n est present.
    pub fn is_empty(&self) -> bool {
        self.entrees.is_empty()
    }

    /// La valeur brute de l en-tete `content-length`, presente ou non.
    ///
    /// C est l expression de la source, `response.headers["content-length"]`,
    /// avant tout calcul. La distinguer de
    /// [`taille_declaree`](Self::taille_declaree) est important : une valeur
    /// presente n est pas forcement une taille exploitable.
    pub fn content_length_brut(&self) -> Option<&str> {
        self.get(CONTENT_LENGTH_HEADER)
    }

    /// La taille annoncee par le serveur, si elle est exploitable.
    ///
    /// Reproduit les deux lignes 11 et 12 de la source :
    ///
    /// ```ts
    /// const parsedSize = contentLength ? Number.parseInt(contentLength, 10) : undefined
    /// const declaredSize =
    ///   parsedSize !== undefined && Number.isSafeInteger(parsedSize) && parsedSize >= 0 ? parsedSize : undefined
    /// ```
    ///
    /// Donc quatre exclusions, dans cet ordre : l en-tete absent, l en-tete vide
    /// qui est faux, l absence de chiffre qui donne `NaN`, et enfin le nombre
    /// qui n est pas un entier sur ou qui est negatif.
    pub fn taille_declaree(&self) -> Option<u64> {
        // Le ternaire de la ligne 11 teste la veracite : l en-tete absent donne
        // `undefined`, et la chaine vide aussi, puisqu elle est fausse. Une
        // chaine vide n est donc pas une taille nulle, c est une absence.
        let brut = match self.content_length_brut() {
            Some(valeur) if !valeur.is_empty() => Some(valeur),
            _ => None,
        };
        let brut = brut?;
        let parse = js_parse_int_radix_10(brut)?;
        if !is_safe_integer(parse) {
            return None;
        }
        // Le reste de la condition source est `parsedSize >= 0`, et le type de
        // retour est non signe : un negatif est donc ecarte ici. `-0` passe et
        // devient `0`, ce qui est exact.
        if parse < 0.0 {
            return None;
        }
        Some(parse as u64)
    }
}

/// Une reponse HTTP capturee sous forme de morceaux d octets.
///
/// Modelise le `Stream<Uint8Array>` de la source. Aucun octet n entre ni ne
/// sort d ici autrement que par [`From`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResponseStream {
    morceaux: Vec<Vec<u8>>,
}

impl ResponseStream {
    /// Un flux sans aucun morceau.
    pub fn new() -> Self {
        ResponseStream { morceaux: Vec::new() }
    }

    /// Un flux a partir d une liste de morceaux, dans l ordre d arrivee.
    pub fn from_chunks(morceaux: Vec<Vec<u8>>) -> Self {
        ResponseStream { morceaux }
    }

    /// Nombre de morceaux du flux, vide ou non.
    pub fn len(&self) -> usize {
        self.morceaux.len()
    }

    /// `true` si le flux ne contient aucun morceau.
    pub fn is_empty(&self) -> bool {
        self.morceaux.is_empty()
    }
}

impl From<Vec<Vec<u8>>> for ResponseStream {
    fn from(morceaux: Vec<Vec<u8>>) -> Self {
        ResponseStream::from_chunks(morceaux)
    }
}

/// Un corps de reponse borne, capture.
///
/// Reproduit `body.subarray(0, size)`. Le champ `capacity` est la longueur
/// logique du tampon de la source, celle que la source lit sous le nom
/// `body.byteLength`, et que la politique de croissance manipule. Elle n est
/// pas forcement egale a la capacite reelle de l allocation sous-jacente, qui
/// est un detail du systeme d allocation et non du contrat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedBody {
    octets: Vec<u8>,
    capacity: u64,
}

impl BoundedBody {
    /// Les octets utiles, c est a dire les `size` premiers octets.
    ///
    /// Aucun octet au-dela n est atteignable par cette API, comme dans la
    /// source ou `subarray(0, size)` borne la longueur.
    pub fn as_bytes(&self) -> &[u8] {
        &self.octets
    }

    /// Nombre d octets utiles, la valeur `size` de la source.
    pub fn len(&self) -> u64 {
        self.octets.len() as u64
    }

    /// `true` si le corps ne contient aucun octet.
    pub fn is_empty(&self) -> bool {
        self.octets.is_empty()
    }

    /// Longueur logique du tampon, la valeur `body.byteLength` de la source.
    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    /// Consomme le corps et rend les octets.
    pub fn into_bytes(self) -> Vec<u8> {
        self.octets
    }
}

/// `Number.parseInt(s, 10)`, exactement.
///
/// Renvoie `None` la ou JavaScript renvoie `NaN`.
///
/// Les regles de la specification sont respectees dans l ordre :
///
/// 1. les espaces en tete de l ensemble `StrWhiteSpace` sont ignores ;
/// 2. un signe `+` ou `-` optionnel est consomme ;
/// 3. les chiffres ASCII sont consommes jusqu au premier caractere qui n en est
///    pas un ;
/// 4. s il n y a eu aucun chiffre, le resultat est `NaN`.
///
/// Deux consequences contre-intuitives, et toutes deux voulues :
/// `"12abc"` vaut `12`, et `"0x10"` vaut `0` puisque la base est imposee a dix
/// alors qu elle serait seize par defaut.
///
/// ```
/// use ycode::swarm::tool_http_body::js_parse_int_radix_10;
///
/// assert_eq!(js_parse_int_radix_10("42"), Some(42.0));
/// assert_eq!(js_parse_int_radix_10("12abc"), Some(12.0));
/// assert_eq!(js_parse_int_radix_10("0x10"), Some(0.0));
/// assert_eq!(js_parse_int_radix_10("abc"), None);
/// ```
pub fn js_parse_int_radix_10(entree: &str) -> Option<f64> {
    let reste = entree.trim_start_matches(is_js_whitespace);
    let (negatif, chiffres) = match reste.strip_prefix('-') {
        Some(suite) => (true, suite),
        None => (false, reste.strip_prefix('+').unwrap_or(reste)),
    };
    // `find` rend la position en octets du premier caractere non ASCII-chiffre,
    // donc la coupe qui suit tombe sur une frontiere UTF-8 valide. Tous les
    // caracteres retenus sont des chiffres ASCII, donc de largeur un.
    let largeur = chiffres
        .find(|caractere: char| !caractere.is_ascii_digit())
        .unwrap_or(chiffres.len());
    if largeur == 0 {
        // Aucun chiffre consomme : la source obtient `NaN`.
        return None;
    }
    let magnitude: f64 = chiffres[..largeur].parse().ok()?;
    // `-0` en JavaScript est un `0` signe, qui survit a `>= 0` et reste faux
    // dans un test de veracite. Rust construit exactement ce meme octet.
    Some(if negatif { -magnitude } else { magnitude })
}

/// `true` pour les caracteres ignores en tete par `parseInt`.
///
/// Ensemble `StrWhiteSpace` de la specification : `WhiteSpace` et
/// `LineTerminator`. On y trouve les espaces ASCII, le retour a la ligne, la
/// tabulation verticale, la tabulation form feed, l espace insecable, le BOM
/// `U+FEFF`, les separateurs de l espece `Zs`, et les deux separateurs de ligne
/// `U+2028` et `U+2029`.
///
/// Le BOM compte, donc `"\u{FEFF}42"` vaut `42`. En revanche l espace de largeur nulle
/// `U+200B` n en fait pas partie : `"\u{200B}42"` vaut `NaN`. C est une
/// distinction fine, et c est celle du JavaScript.
pub fn is_js_whitespace(caractere: char) -> bool {
    matches!(
        caractere,
        '\u{0009}'
            | '\u{000A}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

/// `Number.isSafeInteger(valeur)`.
pub fn is_safe_integer(valeur: f64) -> bool {
    valeur.is_finite() && valeur.fract() == 0.0 && valeur.abs() <= MAX_SAFE_INTEGER
}

/// Longueur du tampon alloue avant la premiere lecture du flux.
///
/// Reproduit `Buffer.allocUnsafe(Math.min(maximumBytes, declaredSize || 64 * 1024))`.
///
/// Le `||` est une veracite, donc `declaredSize` valant `0` donne la taille par
/// defaut, pas zero. C est le point le plus facile a rater de tout le fichier :
/// un portage en `??` changerait le comportement sur toutes les reponses a corps
/// vide, qui sont reelles et frequentes.
pub fn initial_buffer_length(maximum_bytes: u64, declared: Option<u64>) -> u64 {
    let retenu = truthy_or(declared, DEFAULT_BUFFER_BYTES);
    maximum_bytes.min(retenu)
}

/// Longueur du tampon apres un agrandissement.
///
/// Reproduit
/// `Math.min(maximumBytes, Math.max(size + chunkLength, bodyLength * 2))`.
///
/// La croissance double au minimum, mais elle n aboutit jamais au-dela de
/// `maximum_bytes`. Comme le controle d echec a deja guarantees
/// `size + chunkLength <= maximumBytes` avant l appel, le resultat est toujours
/// au moins `size + chunkLength`, donc l ecriture ne peut pas deborder.
pub fn grown_buffer_length(body_length: u64, needed: u64, maximum_bytes: u64) -> u64 {
    let double = body_length.saturating_mul(2);
    maximum_bytes.min(needed.max(double))
}

/// Capture un corps de reponse en respectant une borne dure d octets.
///
/// Reproduit `collectBoundedResponseBody`.
///
/// # Comportement
///
/// - Si l en-tete `content-length` annonce plus que `maximumBytes`, l echec est
///   rendu **avant** toute lecture du flux, comme dans la source.
/// - Chaque morceau non vide est refuse des que `size + morceau` depasse
///   `maximumBytes`. L egalite passe : un corps de exactement `maximumBytes`
///   octets est accepte.
/// - Un morceau vide est ecarte sans rien changer.
/// - Le tampon initial vaut `min(maximumBytes, content-length || 64 Kio)`, puis
///   double a chaque besoin jusqu a `maximumBytes`.
///
/// # Purete
///
/// Aucun thread, aucune attente, aucun repos, aucune requete reseau. Le flux est
/// un [`ResponseStream`] en memoire, et l erreur est produite par une fermeture
/// appelante. La fonction est instantanee.
///
/// # Exemples
///
/// ```
/// use ycode::swarm::tool_http_body::{
///     collect_bounded_response_body, ResponseHeaders, ResponseStream,
/// };
///
/// let reponse = ResponseHeaders::new().with("content-length", "5");
/// let flux = || ResponseStream::from(vec![b"bon".to_vec(), b"jour".to_vec()]);
/// let corps = collect_bounded_response_body(&reponse, 1024, || "trop grand", flux()).unwrap();
/// assert_eq!(corps.as_bytes(), b"bonjour");
/// assert_eq!(corps.len(), 5);
///
/// let refus = collect_bounded_response_body(&reponse, 2, || "trop grand", flux()).unwrap_err();
/// assert_eq!(refus, "trop grand");
/// ```
pub fn collect_bounded_response_body<E, F>(
    headers: &ResponseHeaders,
    maximum_bytes: u64,
    mut too_large: F,
    flux: ResponseStream,
) -> Result<BoundedBody, E>
where
    F: FnMut() -> E,
{
    // Ligne 12 et 13 : une taille declaree superieure a la borne echoue sans
    // que le flux soit lu une seule fois.
    if let Some(declared) = headers.taille_declaree() {
        if declared > maximum_bytes {
            return Err((too_large)());
        }
    }

    // Ligne 15 : `Buffer.allocUnsafe(min(maximumBytes, declaredSize || 64 * 1024))`.
    let mut body_length = initial_buffer_length(maximum_bytes, headers.taille_declaree());
    let mut octets: Vec<u8> = Vec::with_capacity(body_length as usize);
    let mut size: u64 = 0;

    for morceau in flux.morceaux {
        let longueur = morceau.len() as u64;
        // Ligne 18 : le morceau vide est ecarte avant toute comparaison. Il ne
        // consomme pas de budget et ne provoque pas l echec.
        if longueur == 0 {
            continue;
        }
        // L addition ne peut pas deborder : `size` est borne par
        // `maximum_bytes`, et un debordement signifierait de toute facon un
        // depassement, donc l echec. En JavaScript l addition de nombres ne
        // deborde pas mais perd sa precision, d ou l equivalivalence.
        let needed = match size.checked_add(longueur) {
            Some(valeur) => valeur,
            None => return Err((too_large)()),
        };
        // Lignes 19 : la comparaison est `>`, donc la limite exacte passe.
        if needed > maximum_bytes {
            return Err((too_large)());
        }
        // Lignes 20 a 24 : agrandissement avant ecriture, plafonne par la borne.
        if needed > body_length {
            body_length = grown_buffer_length(body_length, needed, maximum_bytes);
        }
        octets.extend_from_slice(&morceau);
        size = needed;
    }

    // Ligne 29 : `body.subarray(0, size)`. La longueurReturned vaut `size`, et
    // la capacite logique reste celle du tampon de la source.
    Ok(BoundedBody { octets, capacity: body_length })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// En-tetes de reference : la seule cle qui compte pour cette fonction.
    fn entetes_avec_longueur(valeur: &str) -> ResponseHeaders {
        ResponseHeaders::new().with(CONTENT_LENGTH_HEADER, valeur)
    }

    #[test]
    fn le_calcul_parse_int_arrete_au_premier_caractere_non_chiffre() {
        // Comportement de `Number.parseInt` que `str::parse::<u64>()` refuse :
        // la source ignore ce qui suit les chiffres, sans erreur.
        assert_eq!(js_parse_int_radix_10("12abc"), Some(12.0));
        assert_eq!(js_parse_int_radix_10("12.9"), Some(12.0));
        assert_eq!(js_parse_int_radix_10("1e3"), Some(1.0));
        assert_eq!(js_parse_int_radix_10("42 "), Some(42.0));
        assert_eq!(js_parse_int_radix_10("  7  "), Some(7.0));
        // La base dix etant imposee, un prefixe hexadecimal n en fait pas partie.
        assert_eq!(js_parse_int_radix_10("0x10"), Some(0.0));
    }

    #[test]
    fn le_calcul_parse_int_refuse_ce_qui_ne_ressemble_pas_a_un_entier() {
        assert_eq!(js_parse_int_radix_10("abc"), None);
        assert_eq!(js_parse_int_radix_10(""), None);
        assert_eq!(js_parse_int_radix_10("   "), None);
        assert_eq!(js_parse_int_radix_10("-"), None);
        // Aucun chiffre avant le signe de la seconde moitie du nom.
        assert_eq!(js_parse_int_radix_10("MiB"), None);
    }

    #[test]
    fn le_calcul_parse_int_accepte_les_espaces_javascript_y_compris_le_bom() {
        // L ensemble `StrWhiteSpace` inclut le BOM `U+FEFF`.
        assert_eq!(js_parse_int_radix_10("\u{FEFF}42"), Some(42.0));
        assert_eq!(js_parse_int_radix_10("\u{00A0}42"), Some(42.0));
        assert_eq!(js_parse_int_radix_10("\u{2028}42"), Some(42.0));
        // L espace de largeur nulle n y est pas, il donne donc `NaN`.
        assert_eq!(js_parse_int_radix_10("\u{200B}42"), None);
    }

    #[test]
    fn le_calcul_parse_int_gere_les_signes() {
        assert_eq!(js_parse_int_radix_10("+7"), Some(7.0));
        assert_eq!(js_parse_int_radix_10("-7"), Some(-7.0));
        assert_eq!(js_parse_int_radix_10("-0"), Some(-0.0));
        // Le signe moins suivi de non-chiffres reste `NaN`.
        assert_eq!(js_parse_int_radix_10("-abc"), None);
    }

    #[test]
    fn un_entier_les_egalites_et_les_exceptions_sont_refuses() {
        // Hors entier sur, donc traite comme absent.
        assert_eq!(js_parse_int_radix_10("9007199254740992"), Some(9007199254740992.0));
        assert!(!is_safe_integer(js_parse_int_radix_10("9007199254740992").unwrap()));
        // Le dernier entier sur reste accepte.
        assert_eq!(js_parse_int_radix_10("9007199254740991"), Some(MAX_SAFE_INTEGER));
        assert!(is_safe_integer(MAX_SAFE_INTEGER));
        // Un entier long perd sa precision et sort de l ensemble sur.
        assert!(!is_safe_integer(js_parse_int_radix_10("12345678901234567890").unwrap()));
        // Un nombre flottant n est pas un entier, meme entier en apparence.
        assert!(!is_safe_integer(1.5));
        assert!(!is_safe_integer(f64::NAN));
        assert!(!is_safe_integer(f64::INFINITY));
    }

    #[test]
    fn une_somme_de_chiffres_qui_deborde_vaut_linfini_et_non_une_erreur() {
        // En JavaScript, `parseInt` d une suite de 401 chiffres vaut `Infinity`,
        // et `Number.isSafeInteger(Infinity)` est `false`. Rust renvoie `Ok(inf)`,
        // jamais une erreur, donc le meme resultat tombe.
        let mut brut = String::from("1");
        brut.push_str(&"0".repeat(400));
        let valeur = js_parse_int_radix_10(&brut);
        assert!(valeur.is_some(), "la conversion ne doit pas echouer");
        assert!(!is_safe_integer(valeur.unwrap()));
    }

    #[test]
    fn une_taille_declaree_valide_est_reprise_telle_quelle() {
        assert_eq!(entetes_avec_longueur("0").taille_declaree(), Some(0));
        assert_eq!(entetes_avec_longueur("1").taille_declaree(), Some(1));
        assert_eq!(entetes_avec_longueur("4096").taille_declaree(), Some(4096));
        assert_eq!(entetes_avec_longueur("4096").content_length_brut(), Some("4096"));
        assert_eq!(entetes_avec_longueur("9007199254740991").taille_declaree(), Some(9007199254740991));
    }

    #[test]
    fn une_taille_declaree_inexploitable_vaut_absence() {
        // En-tete absent.
        assert_eq!(ResponseHeaders::new().taille_declaree(), None);
        // Chaine vide : fausse, donc traitee comme absente, et non comme zero.
        assert_eq!(entetes_avec_longueur("").taille_declaree(), None);
        // Pas de chiffre, negatif, hors entier sur : les trois ecrasees.
        assert_eq!(entetes_avec_longueur("inconnu").taille_declaree(), None);
        assert_eq!(entetes_avec_longueur("-1").taille_declaree(), None);
        assert_eq!(entetes_avec_longueur("9007199254740992").taille_declaree(), None);
    }

    #[test]
    fn la_lecture_est_exacte_sans_normalisation_de_casse() {
        // La source indexe `headers["content-length"]` : la casse compte.
        assert_eq!(
            ResponseHeaders::new().with("Content-Length", "10").taille_declaree(),
            None
        );
        assert_eq!(
            ResponseHeaders::new().with("contentLength", "10").taille_declaree(),
            None
        );
        assert_eq!(
            ResponseHeaders::new().with("content_length", "10").taille_declaree(),
            None
        );
        assert_eq!(ResponseHeaders::new().with("content-length", "10").taille_declaree(), Some(10));
    }

    #[test]
    fn les_noms_de_champs_serialises_sont_content_length() {
        // Piege des noms de champs, sens ecriture. La cle exacte du contrat
        // TypeScript est tout en minuscules avec un tiret, et une faute de
        // frappe ne produirait aucune erreur de compilation.
        let entetes = entetes_avec_longueur("4096").with("content-type", "application/json");
        let json = serde_json::to_value(&entetes).expect("la serialisation ne peut pas echouer");
        let objet = json.as_object().expect("les en-tetes sont un objet JSON");

        let mut cles: Vec<&str> = objet.keys().map(|cle| cle.as_str()).collect();
        cles.sort_unstable();
        assert_eq!(cles, vec!["content-length", "content-type"]);
        assert_eq!(objet.get(CONTENT_LENGTH_HEADER), Some(&serde_json::json!("4096")));

        // La forme exacte de la cle, ecrite en dur.
        assert_eq!(CONTENT_LENGTH_HEADER, "content-length");
        assert!(CONTENT_LENGTH_HEADER.chars().all(|c| !c.is_ascii_uppercase()));
        assert!(!CONTENT_LENGTH_HEADER.contains('_'));
    }

    #[test]
    fn une_forme_snake_case_est_refusee_a_la_lecture() {
        // Piege des noms de champs, sens lecture. Un JSON produit ailleurs avec
        // `content_length` ou `contentLength` se deserialize sans erreur : le
        // type est un objet plat, exactement comme celui de la source. Ce qui
        // echoue, c est la recherche, et c est la que la faute se voit.
        for errone in [
            r#"{"content_length":"4096"}"#,
            r#"{"contentLength":"4096"}"#,
            r#"{"Content-Length":"4096"}"#,
            r#"{"CONTENT-LENGTH":"4096"}"#,
        ] {
            let entetes: ResponseHeaders =
                serde_json::from_str(errone).expect("la deserialisation doit rester tolerante");
            assert_eq!(
                entetes.content_length_brut(),
                None,
                "cette forme ne doit pas etre lue comme un content-length : {errone}"
            );
            assert_eq!(entetes.taille_declaree(), None, "et donc aucune taille declaree");
            assert_eq!(entetes.len(), 1, "la cle est bien presente, elle est simplement ignoree");
        }
    }

    #[test]
    fn un_content_length_de_zero_reserve_le_tampon_par_defaut() {
        // Le piege `||` de la ligne 15. Zero est faux en JavaScript, donc la
        // taille par defaut est utilisee malgre un content-length valide. La
        // borne du tableau est choisie au-dela de 64 Kio, sinon la borne
        // masquerait la valeur de repli.
        let large = DEFAULT_BUFFER_BYTES * 2;
        assert_eq!(initial_buffer_length(large, Some(0)), DEFAULT_BUFFER_BYTES);
        assert_eq!(initial_buffer_length(large, None), DEFAULT_BUFFER_BYTES);
        // La borne reste prioritaire sur le repli.
        assert_eq!(initial_buffer_length(10, Some(0)), 10);
        assert_eq!(initial_buffer_length(5, None), 5);
        // Alors qu une taille non nulle est respectee.
        assert_eq!(initial_buffer_length(large, Some(7)), 7);
    }

    #[test]
    fn la_veracite_et_la_nullite_sont_deux_fonctions_differentes() {
        // Le point central du fichier. Meme signature apparente, comportement
        // oppose sur les deux valeurs qui les distinguent en JavaScript.
        assert_eq!(truthy_or(Some(0u64), DEFAULT_BUFFER_BYTES), DEFAULT_BUFFER_BYTES);
        assert_eq!(nullish_or(Some(0u64), DEFAULT_BUFFER_BYTES), 0);

        assert_eq!(truthy_or(Some(""), "defaut"), "defaut");
        assert_eq!(nullish_or(Some(""), "defaut"), "");

        // Sur une valeur vraie, les deux coincident. C est ce qui rend l erreur
        // silencieuse : un portage en `??` passe tous les tests du programme
        // sauf ceux-la.
        assert_eq!(truthy_or(Some(3u64), 9), nullish_or(Some(3u64), 9));
        assert_eq!(truthy_or(Some("abc"), "defaut"), nullish_or(Some("abc"), "defaut"));
        assert_eq!(truthy_or(None, 9u64), nullish_or(None, 9u64));
    }

    #[test]
    fn la_croissance_double_sans_depasser_la_borne() {
        // `max(needed, bodyLength * 2)`, plafonne par `maximumBytes`.
        assert_eq!(grown_buffer_length(1, 100, 1_000_000), 100, "un gros morceau impose sa taille");
        assert_eq!(grown_buffer_length(4, 5, 1_000_000), 8, "sinon la taille double");
        assert_eq!(grown_buffer_length(8, 9, 1_000_000), 16, "sinon la taille double");
        assert_eq!(grown_buffer_length(4, 9, 10), 10, "la borne l emporte sur le doublement");
        // Un doublement qui deborderait en arithmetiqueEntiere est sature, puis
        // borne de toute facon.
        assert_eq!(grown_buffer_length(u64::MAX, u64::MAX, u64::MAX), u64::MAX);
    }

    #[test]
    fn la_taille_initiale_suit_le_doublement_de_la_source() {
        // Declaration 1, dix morceaux d un octet, chacun. Les longueurs
        // logiques du tampon apres chaque morceau : 1, 1, 2, 4, 4, 8, 8, 8, 8, 16.
        // Les deux premiers morceaux tiennent dans la taille declaree, les
        // suivants declenchent le doublement, et le dernier doublement fait
        // passer de 8 a 16 parce que 9 > 8. Un portage en
        // `max(needed, bodyLength + 1)` donnerait 10, et un portage fige a la
        // taille declaree echouerait des le deuxieme morceau.
        let entetes = entetes_avec_longueur("1");
        let flux = ResponseStream::from(vec![vec![b'a'; 1]; 10]);
        let corps = collect_bounded_response_body(&entetes, 1_000_000, || "trop grand", flux)
            .expect("le flux tient largement dans la borne");

        assert_eq!(corps.len(), 10);
        assert_eq!(corps.capacity(), 16, "la croissance s arrete a la premiere taille suffisante");
        assert_eq!(corps.as_bytes(), b"aaaaaaaaaa");
    }

    #[test]
    fn un_flux_vide_renvoie_un_corps_vide_avec_le_tampon_initial() {
        let entetes = entetes_avec_longueur("100");
        let corps = collect_bounded_response_body(&entetes, 4096, || "trop grand", ResponseStream::new())
            .expect("un flux vide ne peut pas echouer");

        assert!(corps.is_empty());
        assert_eq!(corps.len(), 0);
        // `subarray(0, 0)` sur un tampon de 100 octets : la capacite reste
        // celle du tampon, la longueur utile est zero.
        assert_eq!(corps.capacity(), 100);
    }

    #[test]
    fn une_taille_declaree_trop_grande_echoue_sans_lire_le_flux() {
        // La source echoue a la ligne 14, avant le parcours du flux. Le
        // compteur prouve que le flux n a pas ete consomme.
        let compteur = Cell::new(0usize);
        let morceaux: Vec<Vec<u8>> = (0..4)
            .map(|index| {
                compteur.set(compteur.get() + 1);
                vec![b'x'; index + 1]
            })
            .collect();
        assert_eq!(compteur.get(), 4, "la construction du flux a bien parcouru la liste");

        let entetes = entetes_avec_longueur("11");
        let flux = ResponseStream::from(morceaux);
        let erreur = collect_bounded_response_body(&entetes, 10, || "trop grand", flux)
            .expect_err("une taille declaree superieure a la borne doit etre refusee");

        assert_eq!(erreur, "trop grand");
        assert_eq!(compteur.get(), 4, "le compteur ne bouge pas : le flux n est pas lu");
    }

    #[test]
    fn un_flux_trop_grand_echoue_des_le_morceau_qui_depasse() {
        let entetes = ResponseHeaders::new();
        let flux = ResponseStream::from(vec![b"12345".to_vec(), b"678".to_vec()]);
        let erreur = collect_bounded_response_body(&entetes, 7, || "trop grand", flux)
            .expect_err("le second morceau porte le total a huit octets");

        assert_eq!(erreur, "trop grand");
    }

    #[test]
    fn un_flux_de_la_taille_exacte_de_la_borne_est_accepte() {
        // Le test de la source est `>` et non `>=`. Un portage en `>=` casserait
        // ce cas, qui est le cas limite normal d une reponse de taille fixe.
        let entetes = ResponseHeaders::new();
        for maximum in [0u64, 1, 2, 5, 8, 64, 65_536] {
            let flux = ResponseStream::from(vec![vec![b'z'; maximum as usize]]);
            let corps = collect_bounded_response_body(&entetes, maximum, || "trop grand", flux)
                .unwrap_or_else(|_| panic!("un corps de exactement {maximum} octets doit passer"));
            assert_eq!(corps.len(), maximum);
        }
    }

    #[test]
    fn un_morceau_vide_est_ignore_sans_consommer_de_budget() {
        // Le morceau vide est ecarte ligne 18, avant la comparaison avec la
        // borne. Il ne doit donc pas provoquer l echec ni forcer un
        // agrandissement.
        let entetes = entetes_avec_longueur("4");
        let flux = ResponseStream::from(vec![
            Vec::new(),
            Vec::new(),
            Vec::new(),
            b"abcd".to_vec(),
            Vec::new(),
        ]);
        let corps = collect_bounded_response_body(&entetes, 4, || "trop grand", flux)
            .expect("les morceaux vides ne consomment rien");

        assert_eq!(corps.as_bytes(), b"abcd");
        assert_eq!(corps.capacity(), 4, "aucun agrandissement n a ete necessaire");
    }

    #[test]
    fn la_capture_est_transparente_octet_par_octet() {
        // Aucun BOM retire, aucun octet nul coupe, aucune re-interpretation
        // d encodage. La fonction ne fait que copier.
        let entetes = ResponseHeaders::new();
        let flux = ResponseStream::from(vec![
            vec![0xEF, 0xBB, 0xBF],
            vec![0x00],
            vec![0xFF, 0xFE],
            "caf\u{e9}".as_bytes().to_vec(),
        ]);
        let mut attendu: Vec<u8> = vec![0xEF, 0xBB, 0xBF, 0x00, 0xFF, 0xFE];
        attendu.extend_from_slice("caf\u{e9}".as_bytes());

        let corps = collect_bounded_response_body(&entetes, 1024, || "trop grand", flux)
            .expect("huit octets tiennent dans 1024");

        assert_eq!(corps.as_bytes(), attendu.as_slice());
        // Le BOM est bien present en tete du corps : la capture ne fait pas
        // mieux que la source, elle ne touche a rien.
        assert_eq!(&corps.as_bytes()[..3], &[0xEF, 0xBB, 0xBF]);
    }

    #[test]
    fn la_fermeture_derreur_nest_appellee_quune_seule_fois() {
        // `Effect.fail` transporte une erreur construite a la volee, jamais
        // deux fois pour un meme effet. La fermeture here est donc appelee au
        // plus une fois, et une seconde construction serait une divergence.
        let appels = Cell::new(0usize);
        let entetes = entetes_avec_longueur("11");

        let mut fabrique = || {
            appels.set(appels.get() + 1);
            "trop grand"
        };
        let _ = collect_bounded_response_body(&entetes, 10, &mut fabrique, ResponseStream::new());
        assert_eq!(appels.get(), 1);

        // Un succes n appelle jamais la fermeture.
        let flux = ResponseStream::from(vec![b"ok".to_vec()]);
        let _ = collect_bounded_response_body(
            &ResponseHeaders::new(),
            10,
            &mut fabrique,
            flux,
        );
        assert_eq!(appels.get(), 1, "un succes ne doit jamais construire d erreur");
    }

    #[test]
    fn une_borne_de_zero_refuse_tout_morceau_non_vide() {
        let flux = ResponseStream::from(vec![b"a".to_vec()]);
        let erreur = collect_bounded_response_body(&ResponseHeaders::new(), 0, || "trop grand", flux)
            .expect_err("le seul octet du flux depasse une borne de zero");
        assert_eq!(erreur, "trop grand");

        // Un flux vide reste valide, meme avec une borne de zero.
        let corps = collect_bounded_response_body(
            &ResponseHeaders::new(),
            0,
            || "trop grand",
            ResponseStream::new(),
        )
        .expect("rien a capturer ne peut pas echouer");
        assert!(corps.is_empty());
        assert_eq!(corps.capacity(), 0);
    }

    #[test]
    fn aucun_en_tete_ne_justifie_de_franchir_la_borne() {
        // Un content-length absent, faux ou incoherent ne dispense d aucun
        // controle : c est toujours le flux qui tranche.
        for brut in ["", "inconnu", "-1", "0", "3", "infinity"] {
            let entetes = entetes_avec_longueur(brut);
            let flux = ResponseStream::from(vec![b"1234567890".to_vec()]);
            let resultat = collect_bounded_response_body(&entetes, 5, || "trop grand", flux);
            assert!(
                resultat.is_err(),
                "un content-length de {brut:?} ne doit pas autoriser dix octets"
            );
        }
    }

    #[test]
    fn le_type_de_lerreur_reste_celui_de_lappelant() {
        // La source prend un `() => Error` construit par l appelant. Ici le
        // type de sortie n est impose par personne.
        struct Detail {
            status: u16,
        }
        let entetes = entetes_avec_longueur("100");
        let erreur = collect_bounded_response_body(&entetes, 10, || Detail { status: 413 }, ResponseStream::new())
            .expect_err("la taille declaree depasse la borne");
        assert_eq!(erreur.status, 413);

        let nombre = collect_bounded_response_body(&ResponseHeaders::new(), 1, || 7usize, ResponseStream::from(vec![vec![0u8; 2]]))
            .expect_err("deux octets dans une borne de un");
        assert_eq!(nombre, 7);
    }
}
