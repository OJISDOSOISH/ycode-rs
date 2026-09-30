//! Portage de `packages/core/src/pty/protocol.ts`.
//!
//! # Ce que contient la source
//!
//! Trente-sept lignes, une constante et trois fonctions, plus un reexport
//! d'espace de noms (`export * as PtyProtocol from "./protocol"`, qui n'a pas
//! d'equivalent Rust et n'est donc pas retranscrit). C'est du code, pas un
//! contrat : chaque fonction a un comportement observable.
//!
//! Le service `Pty` est independant du transport, et ces trois fonctions sont
//! l'adaptation websocket. Le format sur le fil tient en deux formes :
//!
//! - une **tranche de sortie**, envoyee telle quelle : du texte UTF-8 brut ;
//! - un unique **cadre de controle** : un octet `0x00` suivi de l'UTF-8 d'un
//!   JSON `{"cursor": n}`, qui porte le curseur de sortie absolu apres la
//!   restitution, pour que le client puisse reprendre plus tard.
//!
//! Les deux sens sont separes. En sortie : [`chunks`] borne la taille des
//! tranches, [`meta_frame`] produit le cadre de controle. En entree :
//! [`decode_input`] decode la frame recue et **abandonne** ce qui n'est pas de
//! l'UTF-8 valide. Les deux appelsants TypeScript sont dans
//! `packages/server/src/handlers/pty.ts:196-212` et
//! `packages/opencode/src/server/routes/instance/httpapi/handlers/pty.ts:244-262`.
//!
//! Ce fichier n'ouvre **ni socket, ni processus, ni fichier**, et n'exporte
//! aucun type deja porte par `pty_pty.rs` ou `pty_schema.rs` : il n'a besoin du
//! pseudo terminal que par le nom de la sortie qu il transporte, donc aucune
//! de leurs declarations n'est reimportee ni redefinie.
//!
//! # Le discriminant est un octet, pas une balise JSON
//!
//! Point d attention pour la relecture : la source n'a **aucune** union
//! taggue. Le controle se distingue des donnees par son octet de tete `0x00`,
//! pas par un champ `"type"`. Un `#[serde(tag = "type")]` ajouterait donc un
//! champ que le TypeScript n emet pas et casserait l'echange. `[Cadre]` est
//! en revanche `untagged` : `Cadre::Donnees` sérialise en chaine JSON nue,
//! comme une tranche brute, et `Cadre::Controle` en `{"cursor":n}`, comme le
//! JSON de la source. Aucun `#[serde(rename)]` de variante n'aurait de sens
//! dans une representation `untagged`, puisque le nom de variante n'apparait
//! jamais dans le JSON ; le seul nom a porter est donc la cle `cursor`, portee
//! explicitement sur [`CadreControle::curseur`].
//!
//! # `chunks` : meme unite que la source, coupure sur une frontiere de caractere
//!
//! C'est le point le plus subtil du fichier. En JavaScript, `data.slice(i, i
//! + REPLAY_CHUNK)` indexe une chaine par **unites UTF-16**, pas par octets :
//! `REPLAY_CHUNK` vaut donc 65 536 unites, soit 65 536 octets sur de l'ASCII
//! mais jusqu a 131 072 octets des que le texte contient de l'ASCII etendu.
//!
//! Un portage par index d'octet, du genre `&data[debut..fin]`, aurait ete
//! faux **deux fois** : il compte les octets la ou la source compte des
//! unites, et il **panique** des que la coupure tombe au milieu d'un
//! caractere (`byte index ... is not a char boundary`). Un accent ou un emoji
//!.place a lafrontiere suffit a le faire tomber.
//!
//! Le portage garde l'unite de la source et ne coupe que sur une frontiere de
//! caractere :
//!
//! - [`chunks`] compte les unites UTF-16 de chaque caractere via
//!   `char::len_utf16`, donc les bornes de trames sont **identiques** a celles
//!   de la source pour tout texte du plan multilingue de base, ce qui couvre
//!   l'ASCII, les accents et la plupart des symboles ;
//! - quand la coupure tomberait **dans** un caractere hors du plan de base,
//! qui pese deux unites, la trame s'arrete au caractere precedent. La source,
//! elle, livre un Tableau de `Uint8Array` contenant un pseudo surrogate
//! isole, que l encodage de sortie transforme en `U+FFFD` : elle corrupte le
//! texte. Le portage ne peut pas reproduire cette corruption, un `str` Rust
//! n'a pas de surrogate, donc il s'en tient a un decalage d'au plus un
//! caractere par trame, sans jamais paniquer et sans caractere de
//! remplacement ;
//! - les trames se recollent sans perte dans tous les cas, ce que les tests
//!   verifient sur de l'ASCII, des accents et un emoji.
//!
//! Consequence de borne a connaitre : `REPLAY_CHUNK` borne des caracteres, pas
//! des octets. Une trame pese au plus 4 `REPLAY_CHUNK` octets en presence de
//! caracteres hors du plan de base, contre 2 `REPLAY_CHUNK` en amont, ou la
//! borne implicite venait de l'unite. Un transport qui plafonne les trames en
//! octets doit donc redecouper sur les octets en amont.
//!
//! # `decode_input` : le `fatal` du decodeur, et son BOM
//!
//! La source instancie `new TextDecoder("utf-8", { fatal: true })`. Deux
//! consequences sont portees explicitement :
//!
//! - `fatal: true` signifie qu'une sequence invalide **leve** au lieu de
//!   produire un `U+FFFD`. Le `catch` transforme l exception en
//!   `undefined`, donc l'entree est abandonnee. `String::from_utf8` a la meme
//!   severite : il refuse les octets de continuation isoles, les
//!   surencodages, les points de code au-dela de `U+10FFFF` et les surrogates
//!   encodes en UTF-8 (CESU-8). Le resultat est `Option<String>`, et `None`
//!   signifie exactement `undefined`.
//! - l'option `ignoreBOM` vaut `false` par defaut, donc le decodeur **retire**
//!   un `U+FEFF` en tete d'entree. `String::from_utf8` le conserve, donc le
//!   retrait est fait ici, une seule fois et uniquement en tete. Le chemin
//!   `string` de la source ne passe pas par le decodeur : il ne retire rien.
//!
//! # Le piege `?` contre `??` se loge exactement ici
//!
//! Les appelants ecrivent `if (decoded !== undefined)`, c est a dire un test de
//! **nullite**, jamais de veracite. Une entree vide est donc ecrite dans le
//! pseudo terminal, et un portage qui testerait `!is_empty()` ecraserait ce
//! cas. Ici `decode_input` rend `Some("")` pour une entree vide, et c est
//! l appelant qui doit tester `is_some()`. Aucun `unwrap_or_default()` n est
//! employe dans ce fichier, pour la meme raison.
//!
//! # `cursor` : un entier, et ce que cela exclut
//!
//! La source type `cursor` en `number`, donc en flottant double. Le curseur
//! est un decalage de sortie, donc un entier en pratique, et il est porte en
//! `i64` comme les autres `number` de la famille `pty_pty.rs`. Le cout est
//! connu et borne : `{"cursor": 1.5}`, `{"cursor": null}` (ce que
//! `JSON.stringify` produit pour `NaN` ou `Infinity`) et les valeurs trop
//! grandes pour 64 bits echouent a la deserialisation au lieu d etre arrondies
//! en silence. Un curseur negatif est en revanche accepte et reecrit tel quel,
//! la source n en interdit aucun.
//!
//! # Ce que ce fichier ajoute, et qui n existe pas en amont
//!
//! - [`MessageEntree`] regroupe les trois formes du parametre `message`. En
//!   amont `ArrayBuffer` et `Uint8Array` sont deux types distincts, mais la
//!   source les traite de facon identique (`new Uint8Array(message)`), donc
//!   ils donnent **une seule** variante aqui : des octets.
//! - [`Cadre::vers_octets`] et [`Cadre::depuis_octets`] n ont aucun equivalent
//!   TypeScript : la source n encode que, le decodage du cadre de controle est
//!   fait par le client, en JavaScript. Ces deux methodes servent a prouver
//!   que l encodage se relit, et a verifier l aller-retour dans les tests.
//!
//! Un point de vigilance sur [`Cadre::depuis_octets`] : l octet `0x00` est
//! announceur de controle, mais une sortie de terminal peut commencer par cet
//! octet, et un nom de fichier peut s appeler `{"cursor":7}`. Le decodage
//! tente donc le controle en premier et **retombe sur les donnees** si l
//! analyse echoue, pour qu aucune sortie ne soit perdue. Cette resolution
//! n est pas imposee par l amont, qui ne decode rien.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Taille maximale d une tranche de restitution, en caracteres.
///
/// Reprend `REPLAY_CHUNK = 64 * 1024` de la source. La source compte des
/// unites UTF-16 ; ici la valeur borne des caracteres, qui coincident avec
/// ces unites pour tout caractere du plan multilingue de base. Voir la note du
/// module sur [`chunks`] pour la consequence sur la taille en octets.
pub const REPLAY_CHUNK: usize = 64 * 1024;

/// Octet de tete d un cadre de controle.
///
/// C'est le **seul** discriminant du protocole : la source ne pose aucune
/// balise JSON, donc aucun nom de variante ne doit apparaitre dans les donnees
/// echangees.
pub const OCTET_CONTROLE: u8 = 0x00;

/// Corps du cadre de controle, equivalent de l objet `{ cursor }` que
/// `JSON.stringify` produit dans `metaFrame`.
///
/// Le champ porte un renommage explicite : le nom de champ est le premier
/// piege de l echange TypeScript, et le nom du champ Rust est en francais
/// alors que le nom du fil est en anglais.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CadreControle {
    /// Curseur de sortie absolu apres restitution. Nom JSON exact `cursor`,
    /// en minuscules.
    #[serde(rename = "cursor")]
    pub curseur: i64,
}

impl CadreControle {
    /// Construit un cadre de controle pour le curseur donne.
    pub fn nouveau(curseur: i64) -> Self {
        Self { curseur }
    }
}

/// Une trame du protocole, dans l'une de ses deux formes.
///
/// La representation est `untagged` et c est volontaire : voir la note du
/// module sur le discriminant. `Donnees` sérialise en chaine nue, `Controle`
/// en `{"cursor":n}`, exactement comme la source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Cadre {
    /// Tranche de sortie, du texte UTF-8 brut, tel que la source l envoie.
    Donnees(String),
    /// Cadre de controle, seul cadre prefixe par [`OCTET_CONTROLE`].
    Controle(CadreControle),
}

impl Cadre {
    /// Fabrique un cadre de controle, comme `metaFrame(cursor)`.
    pub fn controle(curseur: i64) -> Self {
        Cadre::Controle(CadreControle::nouveau(curseur))
    }

    /// Fabrique une trame de sortie, comme une tranche de [`chunks`].
    pub fn donnees(texte: impl Into<String>) -> Self {
        Cadre::Donnees(texte.into())
    }

    /// Serialise la trame en octets prets a etre envoyes.
    ///
    /// Une trame de donnees part telle quelle, comme le `Uint8Array` brut de la
    /// source ; une trame de controle est prefixee par [`OCTET_CONTROLE`].
    /// Cette methode n'existe pas en amont, ou l encodage est ecrit en ligne
    /// dans `metaFrame`.
    pub fn vers_octets(&self) -> Vec<u8> {
        match self {
            Cadre::Donnees(texte) => texte.as_bytes().to_vec(),
            Cadre::Controle(controle) => {
                // Un entier seul ne peut pas faire echouer une serialisation.
                let json = serde_json::to_vec(controle)
                    .expect("la serialisation d un curseur entier ne peut pas echouer");
                let mut sortie = Vec::with_capacity(json.len() + 1);
                sortie.push(OCTET_CONTROLE);
                sortie.extend_from_slice(&json);
                sortie
            }
        }
    }

    /// Relit une trame recue, ou signale qu elle n est pas de l UTF-8 valide.
    ///
    /// N'existe pas en amont : la source ne decode que l'entree du client, et
    /// le client JavaScript lit lui-meme le cadre de controle. Le repli sur les
    /// donnees, quand l octet de tete annonce un controle qui n en est pas un,
    /// est un choix de ce portage, explique dans la note du module.
    pub fn depuis_octets(octets: &[u8]) -> Result<Cadre, ErreurCadre> {
        if octets.first() == Some(&OCTET_CONTROLE) {
            if let Ok(controle) = serde_json::from_slice::<CadreControle>(&octets[1..]) {
                return Ok(Cadre::Controle(controle));
            }
        }
        match String::from_utf8(octets.to_vec()) {
            Ok(texte) => Ok(Cadre::Donnees(texte)),
            Err(_) => Err(EreurCadre { taille: octets.len() }),
        }
    }
}

/// Echec de relecture d une trame.
///
/// Le seul echec possible est une trame de donnees dont les octets ne forment
/// pas de l'UTF-8 valide : le repli de [`Cadre::depuis_octets`] est exhaustif,
/// un cadre de controle non analysable redevient une trame de donnees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErreurCadre {
    /// Taille de la trame refusee, en octets, pour le diagnostic.
    pub taille: usize,
}

impl fmt::Display for ErreurCadre {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "trame illisible : {} octets qui ne sont pas de l'UTF-8 valide",
            self.taille
        )
    }
}

impl std::error::Error for ErreurCadre {}

/// Cadre de controle, equivalent de `metaFrame(cursor)`.
///
/// La source renvoie un `Uint8Array` : l'octet `0x00`, puis l'UTF-8 de
/// `JSON.stringify({ cursor })`, qui vaut exactement `{"cursor":42}` pour un
/// curseur entier. Le resultat est identique ici, octet pour octet.
pub fn meta_frame(cursor: i64) -> Vec<u8> {
    Cadre::controle(cursor).vers_octets()
}

/// Message entrant, equivalent de l union `string | Uint8Array | ArrayBuffer`
/// du parametre `message`.
///
/// Les trois formes de la source donnent **deux** variantes : la source
/// convertit un `ArrayBuffer` en `Uint8Array` avant de le decoder, donc les
/// deux sont deja le meme octet. Aucune forme serialisee n'existe pour ce type,
/// donc il ne derive ni `Serialize` ni `Deserialize` : il n'y a aucun nom de
/// champ a porter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageEntree {
    /// Message deja en texte, qui est renvoye tel quel, sans passer par un
    /// decodeur et donc sans retrait de BOM.
    Texte(String),
    /// Message binaire, a decoder en UTF-8 strict.
    Octets(Vec<u8>),
}

impl From<&str> for MessageEntree {
    fn from(texte: &str) -> Self {
        MessageEntree::Texte(texte.to_string())
    }
}

impl From<String> for MessageEntree {
    fn from(texte: String) -> Self {
        MessageEntree::Texte(texte)
    }
}

impl From<Vec<u8>> for MessageEntree {
    fn from(octets: Vec<u8>) -> Self {
        MessageEntree::Octets(octets)
    }
}

impl From<&[u8]> for MessageEntree {
    fn from(octets: &[u8]) -> Self {
        MessageEntree::Octets(octets.to_vec())
    }
}

/// Decode un message entrant, equivalent de `decodeInput(message)`.
///
/// Renvoie `Some(texte)` quand le message est exploitable, `None` quand il
/// faut l abandonner : `None` est l equivalent de l `undefined` que la source
/// renvoie sur une sequence UTF-8 invalide.
///
/// Attention, piege `?` contre `??` : un message **vide** donne `Some("")`, il
/// ne donne pas `None`. Les appelants de la source testent
/// `decoded !== undefined`, donc l appelant doit tester `is_some()` et jamais
/// `!is_empty()`.
///
/// Le chemin binaire retire un `U+FEFF` de tete, comme le fait le `TextDecoder`
/// de la source, dont l option `ignoreBOM` vaut `false` par defaut. Le chemin
/// texte ne retire rien, la source n'y passe pas de decodeur.
pub fn decode_input(message: MessageEntree) -> Option<String> {
    match message {
        MessageEntree::Texte(texte) => Some(texte),
        MessageEntree::Octets(octets) => String::from_utf8(octets).ok().map(sans_bom),
    }
}

/// Retire un eventuel BOM en tete, comme le decodeur de la source.
///
/// `strip_prefix` compare un caractere, donc il ne peut pas couper un
/// caractere multi octets, contrairement a un decoupage par index d'octet.
fn sans_bom(texte: String) -> String {
    match texte.strip_prefix('\u{feff}') {
        Some(sans) => sans.to_string(),
        None => texte,
    }
}

/// Decoupe une restitution en trames bornees, equivalent de `chunks(data)`.
///
/// Une chaine vide ne donne **aucune** trame, comme en amont : la boucle de la
/// source n entre jamais dans son corps. Les trames se recollent sans perte,
/// et aucune coupure ne tombe au milieu d'un caractere.
///
/// La borne compte des unites UTF-16 comme la source, mais ne coupe que sur
/// une frontiere de caractere. Voir la note du module pour le detail de la
/// divergence sur les caracteres hors du plan multilingue de base.
///
/// # Exemples
///
/// ```
/// use ycode::swarm::pty_protocol::chunks;
///
/// // Une chaine courte donne une seule trame.
/// assert_eq!(chunks("abc"), vec!["abc".to_string()]);
/// // Une chaine vide n'en donne aucune.
/// assert!(chunks("").is_empty());
/// ```
pub fn chunks(data: &str) -> Vec<String> {
    let mut trames: Vec<String> = Vec::new();
    let mut debut = 0usize;
    let mut unites = 0usize;

    // `char_indices` ne rend que des positions de debut de caractere, donc
    // `debut` et `index` sont des frontieres valides et la tranche ne peut pas
    // paniquer, meme si la coupure logique tombait au milieu d un caractere :
    // dans ce cas elle est reculee d un caractere entier.
    for (index, caractere) in data.char_indices() {
        let poids = caractere.len_utf16();
        if unites + poids > REPLAY_CHUNK {
            debug_assert!(data.is_char_boundary(debut) && data.is_char_boundary(index));
            trames.push(data[debut..index].to_string());
            debut = index;
            unites = 0;
        }
        unites += poids;
    }

    if debut < data.len() {
        trames.push(data[debut..].to_string());
    }
    trames
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- outils de test, purement en memoire -------------------------------

    /// Chaine de `nombre` repetitions du caractere donne, quelle que soit sa
    /// largeur en octets.
    fn repete(nombre: usize, caractere: char) -> String {
        std::iter::repeat(caractere).take(nombre).collect()
    }

    /// Largeur en unites UTF-16, l unite que compte la source.
    fn unites_utf16(texte: &str) -> usize {
        texte.chars().map(char::len_utf16).sum()
    }

    // -- REPLAY_CHUNK ------------------------------------------------------

    #[test]
    fn replay_chunk_size_is_sixty_four_kibibytes() {
        assert_eq!(REPLAY_CHUNK, 64 * 1024);
        assert_eq!(REPLAY_CHUNK, 65_536);
    }

    #[test]
    fn control_byte_is_zero() {
        assert_eq!(OCTET_CONTROLE, 0x00);
    }

    // -- chunks ------------------------------------------------------------

    #[test]
    fn empty_string_yields_no_frames() {
        // La source : la boucle `for (i = 0; i < 0)` n entre jamais, donc le
        // tableau rendu est vide, et non un tableau d une chaine vide.
        assert_eq!(chunks(""), Vec::<String>::new());
    }

    #[test]
    fn short_string_stays_in_a_single_frame() {
        assert_eq!(chunks("abc"), vec!["abc".to_string()]);
    }

    #[test]
    fn input_of_exactly_the_chunk_size_yields_one_full_frame() {
        let data = repete(REPLAY_CHUNK, 'x');
        let trames = chunks(&data);
        assert_eq!(trames.len(), 1, "la borne est atteinte exactement, pas depassee");
        assert_eq!(unites_utf16(&trames[0]), REPLAY_CHUNK);
    }

    #[test]
    fn input_one_char_over_the_chunk_size_yields_two_frames_that_rejoin() {
        let data = repete(REPLAY_CHUNK + 1, 'x');
        let trames = chunks(&data);
        assert_eq!(trames.len(), 2);
        assert_eq!(unites_utf16(&trames[0]), REPLAY_CHUNK);
        assert_eq!(unites_utf16(&trames[1]), 1);
        assert_eq!(trames.concat(), data);
    }

    #[test]
    fn frames_rejoin_without_loss_or_duplication() {
        let data = format!("{}{}{}", repete(REPLAY_CHUNK, 'a'), repete(1, 'b'), repete(2 * REPLAY_CHUNK + 7, 'c'));
        let trames = chunks(&data);
        assert_eq!(trames.len(), 4);
        assert_eq!(trames.concat(), data);
    }

    #[test]
    fn no_frame_exceeds_the_advertised_size() {
        // La borne vaut pour toutes les trames, pas seulement pour la
        // premiere : c est elle qui borne la taille d une trame de transport.
        let data = repete(3 * REPLAY_CHUNK + 11, 'x');
        for trame in chunks(&data) {
            assert!(
                unites_utf16(&trame) <= REPLAY_CHUNK,
                "trame trop large : {} unites",
                unites_utf16(&trame)
            );
        }
    }

    #[test]
    fn accented_char_at_the_boundary_neither_panics_nor_corrupts() {
        // Le piege du lot : un portage par index d'octet couperait l'accent en
        // deux ici, car il tient sur deux octets et commence a l'index 65 535.
        // En unites UTF-16, en revanche, il compte pour un, donc il finit la
        // premiere trame exactement.
        let data = format!("{}\u{e9}z", repete(REPLAY_CHUNK - 1, 'a'));
        let trames = chunks(&data);
        assert_eq!(trames.len(), 2);
        assert!(trames[0].ends_with('\u{e9}'), "l'accent reste entier");
        assert_eq!(trames.concat(), data);
    }

    #[test]
    fn emoji_at_the_boundary_is_backed_off_by_one_character_and_not_split() {
        // L'emoji pese deux unites UTF-16 : la coupure logique tombe dans son
        // couple de surrogates. La source livre alors deux pseudo surrogates
        // isoles, que son encodage transforme en U+FFFD. Le portage s'arrete au
        // caractere precedent : meme nombre de trames, aucun caractere de
        // remplacement, aucune corruption.
        let data = format!("{}\u{1f600}z", repete(REPLAY_CHUNK - 1, 'a'));
        let trames = chunks(&data);
        assert_eq!(trames.len(), 2);
        assert_eq!(trames[0], repete(REPLAY_CHUNK - 1, 'a'));
        assert_eq!(trames[1], "\u{1f600}z");
        assert_eq!(trames.concat(), data);
        for trame in &trames {
            assert!(!trame.contains('\u{fffd}'), "aucun caractere de remplacement : {trame}");
        }
    }

    #[test]
    fn mixed_text_of_accents_and_emoji_rejoins_without_loss() {
        let modele = "a\u{e9}\u{1f600}z\u{e8}\u{1f1eb}\u{1f1f7}";
        let data = repete(REPLAY_CHUNK, 'a') + modele;
        let trames = chunks(&data);
        assert_eq!(trames.concat(), data);
        assert_eq!(trames.len(), 2);
        assert!(trames[0].ends_with('a'));
        assert_eq!(trames[1], modele);
    }

    // -- meta_frame --------------------------------------------------------

    #[test]
    fn control_frame_starts_with_the_zero_byte() {
        let cadre = meta_frame(42);
        assert_eq!(cadre[0], 0x00);
    }

    #[test]
    fn control_frame_contains_the_cursor_json() {
        let cadre = meta_frame(42);
        assert_eq!(&cadre[1..], br#"{"cursor":42}"#);
    }

    #[test]
    fn control_frame_json_holds_only_the_cursor_key() {
        // Premier piege du lot : la cle est `cursor`, en minuscules, et rien
        // d'autre ne doit apparaitre.
        let json = serde_json::to_string(&CadreControle::nouveau(7)).expect("serialisation");
        assert_eq!(json, r#"{"cursor":7}"#);

        let objet = serde_json::to_value(CadreControle::nouveau(7))
            .expect("serialisation")
            .as_object()
            .expect("le cadre de controle est un objet")
            .clone();
        let mut cles: Vec<&str> = objet.keys().map(|cle| cle.as_str()).collect();
        cles.sort_unstable();
        assert_eq!(cles, vec!["cursor"]);
    }

    #[test]
    fn zero_or_negative_cursor_is_encoded_verbatim_and_is_not_a_missing_cursor() {
        // Piege `?` contre `??` : zero et les valeurs negatives sont des
        // curseurs presents, pas des curseurs manquants.
        assert_eq!(&meta_frame(0)[1..], br#"{"cursor":0}"#);
        assert_eq!(&meta_frame(-1)[1..], br#"{"cursor":-1}"#);
        assert_eq!(&meta_frame(i64::MAX)[1..], br#"{"cursor":9223372036854775807}"#);
    }

    #[test]
    fn misspelled_cursor_key_is_rejected_on_read() {
        // Sans renommage explicite, la faute passerait la compilation et ne
        // serait vue qu'a l'echange avec le TypeScript.
        let correct: Result<CadreControle, _> = serde_json::from_str(r#"{"cursor":42}"#);
        assert_eq!(correct.expect("deserialisation"), CadreControle::nouveau(42));

        let errone: Result<CadreControle, _> = serde_json::from_str(r#"{"Cursor":42}"#);
        assert!(errone.is_err(), "la cle doit rester `cursor`");

        let absent: Result<CadreControle, _> = serde_json::from_str("{}");
        assert!(absent.is_err(), "un curseur absent n'est pas un curseur nul");
    }

    #[test]
    fn fractional_or_nan_cursor_is_rejected_where_the_source_would_write_null() {
        // Cout documente du passage de `number` a entier : la source ecrirait
        // `{"cursor":null}` pour un NaN, et `1.5` tel quel. Les deux sont
        // refuses ici plutot qu arrondis en silence.
        for source in [r#"{"cursor":1.5}"#, r#"{"cursor":null}"#, r#"{"cursor":1e400}"#] {
            assert!(
                serde_json::from_str::<CadreControle>(source).is_err(),
                "doit etre refuse : {source}"
            );
        }
    }

    // -- aller-retour des trames -------------------------------------------

    #[test]
    fn control_frame_reads_back_identical() {
        let cadre = meta_frame(4096);
        assert_eq!(
            Cadre::depuis_octets(&cadre).expect("le cadre produit par meta_frame se relit"),
            Cadre::controle(4096)
        );
    }

    #[test]
    fn data_frame_reads_back_identical() {
        let texte = "total 12\n\u{e9}t\u{e9}\u{1f600}\n";
        let cadre = Cadre::donnees(texte);
        assert_eq!(cadre.vers_octets(), texte.as_bytes());
        assert_eq!(
            Cadre::depuis_octets(cadre.vers_octets().as_slice()).expect("trame relue"),
            cadre
        );
    }

    #[test]
    fn empty_data_frame_reads_back_as_empty_data() {
        // La source ne produit jamais une trame vide, puisque `chunks("")` ne
        // rend aucun element. Le portage accepte l nevertheless plutot que de
        // le refuser.
        let cadre = Cadre::donnees("");
        assert!(cadre.vers_octets().is_empty());
        assert_eq!(Cadre::depuis_octets(&[]).expect("trame relue"), cadre);
    }

    #[test]
    fn invalid_binary_frame_is_rejected() {
        let resultat = Cadre::depuis_octets(&[0xff, 0xfe, 0xfd]);
        assert_eq!(resultat, Err(ErreurCadre { taille: 3 }));
    }

    #[test]
    fn leading_zero_followed_by_incomplete_json_falls_back_to_data() {
        // L'octet nul annonce un controle, mais une sortie de terminal peut
        // commencer par lui. Le repli evite de perdre la sortie.
        let cadre = Cadre::depuis_octets(&[0x00, b'{']).expect("repli sur les donnees");
        assert_eq!(cadre, Cadre::donnees("\u{0}{"));
    }

    #[test]
    fn data_frame_starting_with_a_nul_stays_a_data_frame() {
        // Tant que ce qui suit le nul n'est pas un JSON de controle, c'est une
        // sortie, pas un cadre de controle.
        let cadre = Cadre::depuis_octets(b"\x00\x1b[31mrouge").expect("trame relue");
        assert_eq!(cadre, Cadre::donnees("\u{0}\u{1b}[31mrouge"));
    }

    #[test]
    fn unknown_field_in_the_control_frame_is_ignored_as_in_javascript() {
        // `JSON.parse` puis `.cursor` ignore les cles en trop, et serde_json
        // aussi : les deux lectures restent compatibles.
        let cadre = Cadre::depuis_octets(b"\x00{\"cursor\":5,\"extra\":1}").expect("trame relue");
        assert_eq!(cadre, Cadre::controle(5));
    }

    // -- decode_input ------------------------------------------------------

    #[test]
    fn text_message_passes_through_unchanged() {
        assert_eq!(
            decode_input(MessageEntree::Texte("ready".to_string())),
            Some("ready".to_string())
        );
    }

    #[test]
    fn valid_utf8_bytes_are_decoded() {
        assert_eq!(
            decode_input(MessageEntree::Octets(b"hello".to_vec())),
            Some("hello".to_string())
        );
    }

    #[test]
    fn invalid_bytes_are_dropped_and_not_replaced() {
        // `fatal: true` : la source abandonne la frame, elle ne la remplace pas
        // par un U+FFFD comme le ferait un decodeur lenient.
        assert_eq!(decode_input(MessageEntree::Octets(vec![0xff, 0xfe, 0xfd])), None);
    }

    #[test]
    fn surrogate_encoded_as_utf8_is_dropped() {
        // CESU-8 : les trois octets d'un U+D800. Un decodeur lenient en
        // ferait un caractere de remplacement, un decodeur fatal refuse.
        assert_eq!(decode_input(MessageEntree::Octets(vec![0xed, 0xa0, 0x80])), None);
    }

    #[test]
    fn empty_message_is_present_and_not_absent() {
        // Le piege `?` contre `??` du lot. Les appelants de la source ecrivent
        // `decoded !== undefined` : un message vide est donc ecrit dans le
        // pseudo terminal. `Some("")` et non `None`, sur les deux chemins.
        assert_eq!(decode_input(MessageEntree::Texte(String::new())), Some(String::new()));
        assert_eq!(decode_input(MessageEntree::Octets(Vec::new())), Some(String::new()));
        assert!(decode_input(MessageEntree::Texte(String::new())).is_some());
    }

    #[test]
    fn leading_bom_is_stripped_like_the_source_decoder() {
        // `new TextDecoder("utf-8", { fatal: true })` a `ignoreBOM: false`, donc
        // il retire le BOM de sa sortie. `String::from_utf8` le garde, il faut
        // donc le retirer ici pour rester compatible.
        let octets = vec![0xef, 0xbb, 0xbf, b'a'];
        assert_eq!(decode_input(MessageEntree::Octets(octets)), Some("a".to_string()));
    }

    #[test]
    fn bom_alone_yields_an_empty_string_and_not_absence() {
        // La distinction reste visible apres le retrait du BOM : le message
        // est valide, donc il donne une chaine vide.
        let octets = vec![0xef, 0xbb, 0xbf];
        assert_eq!(decode_input(MessageEntree::Octets(octets)), Some(String::new()));
    }

    #[test]
    fn bom_not_at_the_start_is_not_stripped() {
        let octets = "a\u{feff}b".as_bytes().to_vec();
        assert_eq!(
            decode_input(MessageEntree::Octets(octets)),
            Some("a\u{feff}b".to_string())
        );
    }

    #[test]
    fn text_message_keeps_its_leading_bom() {
        // Le chemin `typeof message === "string"` de la source renvoie la
        // chaine sans passer par le decodeur : le BOM y reste.
        assert_eq!(
            decode_input(MessageEntree::Texte("\u{feff}a".to_string())),
            Some("\u{feff}a".to_string())
        );
    }

    #[test]
    fn multi_byte_bytes_are_decoded_without_corruption() {
        let texte = "h\u{e9}llo \u{1f600} \u{1f1eb}\u{1f1f7}";
        assert_eq!(
            decode_input(MessageEntree::Octets(texte.as_bytes().to_vec())),
            Some(texte.to_string())
        );
    }

    #[test]
    fn all_three_source_forms_are_built_by_the_from_impls() {
        // Les trois formes du parametre `message` : deux donnent la meme
        // variante, car la source convertit l'ArrayBuffer en Uint8Array.
        assert_eq!(
            MessageEntree::from("a"),
            MessageEntree::Texte("a".to_string())
        );
        assert_eq!(
            MessageEntree::from(vec![0x61]),
            MessageEntree::Octets(vec![0x61])
        );
        assert_eq!(
            MessageEntree::from(&[0x61][..]),
            MessageEntree::Octets(vec![0x61])
        );
    }
}
