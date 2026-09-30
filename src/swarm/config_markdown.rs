//! Portage de `packages/core/src/config/markdown.ts`.
//!
//! ## Ce que la source contient reellement
//!
//! Trente-six lignes, dont **une seule** est de la logique pure au sens fort :
//! `sanitize`. Les deux autres fonctions n sont que de l orchestration autour
//! de `gray-matter`.
//!
//! ```ts
//! export function parse(content: string) {
//!   try { return matter(content) } catch { return matter(sanitize(content)) }
//! }
//!
//! export function parseOption(content: string) {
//!   try { return parse(content) } catch { return undefined }
//! }
//!
//! export function sanitize(content: string) {
//!   const match = content.match(/^---\r?\n([\s\S]*?)\r?\n---/)
//!   if (!match) return content
//!   const frontmatter = match[1]
//!   const result = frontmatter.split(/\r?\n/).flatMap((line) => { /* ... */ })
//!   return content.replace(frontmatter, () => result.join("\n"))
//! }
//! ```
//!
//! La premiere ligne, `export * as ConfigMarkdown from "./markdown"`, est un
//! reexport du module sur lui-meme. En Rust le module joue deja ce role, la
//! ligne disparait du portage.
//!
//! ## Ce qui est porte, et ce qui ne l est pas
//!
//! `sanitize` est **portee en entier**, parce qu elle ne depend que de la
//! manipulation de chaines : la regexp de debut, le decoupage en lignes, les
//! six tests de tri, et le remplacement final. Chacune a son test.
//!
//! `parse` et `parse_option` appellent `matter`, c est a dire `gray-matter`,
//! qui n est **pas** une dependance de `Cargo.toml` (qui ne declare que
//! `serde`, `serde_json`, `tokio`, `reqwest`, `anyhow`, `thiserror`,
//! `async-trait` et `uuid`). Reecrire un lecteur de YAML serait exactement le
//! piege du fichier `util_glob` : un moteur qui **parait** faire la meme chose
//! et qui diverge des la premiere cle d une aille, sur le premier `>-`, sur le
//! premier `&ancrage`. Les deux fonctions sont donc declarees avec la bonne
//! signature, la **bonne forme de retour**, et un corps signale comme manquant.
//! Une valeur de retour vide aurait ete pire : `parse_option` rend `undefined`
//! en cas d echec, et une chaine vide se confondrait avec un frontmatter
//! legitimement vide.
//!
//! Le type [`Frontmatter`] est declare malgre tout, avec les deux seules
//! proprietes que les trois appelants du depot lisent : `.data` et `.content`
//! (`packages/core/src/skill.ts` ligne 84, `config/plugin/agent.ts` ligne 154,
//! `config/plugin/command.ts` ligne 71). Les autres champs de l objet
//! `gray-matter` (`orig`, `language`, `matter`, `stringify`, `isEmpty`) ne sont
//! lus par personne, donc ne sont pas declares.
//!
//! ## Les deux tests de nullite et de veracite, a ne jamais confondre
//!
//! Ce fichier est un bon exemple du piege general du projet, et il contient les
//! deux formes cote a cote :
//!
//! 1. `if (!match) return content` teste la **nullite** du resultat de
//!    `content.match(...)`, qui vaut `null` ou un tableau. Ce n est pas un test
//!    de veracite : en JavaScript un tableau, meme vide, est toujours vrai.
//!    Traduction : `Option::is_none`, et rien d autre. surtout **pas** un test
//!    d emptiness : une regexp qui correspond sur un groupe **vide** donne un
//!    tableau non nul, donc on continue. C est pourquoi `---\n\n---` est
//!    traite comme un frontmatter vide alors que `"x"` est rendu tel quel. Les
//!    deux cas se confondraient si on traduisait par `is_empty()`.
//!
//! 2. `if (value === "")` est une **egalite stricte** sur une chaine, apres
//!    `entry[2].trim()`. Ce n est ni un test de nullite ni un test de
//!    veracite. Il distingue `key:` (valeur vide, la cle devient `null` en
//!    YAML) de `key: 0` (valeur presente). Ne pas le remplacer par un
//!    `.is_empty()` sur la valeur **avant** trim, ni par un `.is_none()`.
//!
//! Les deux fonctions [`reecrire_ligne`] et [`frontmatter_de`] sont les seules
//! qui portent ces tests, et chacune le porte a un seul endroit.
//!
//! ## Le piege principal : le remplacement vise une chaine, pas la regexp
//!
//! `content.replace(frontmatter, () => ...)` cherche la **chaine**
//! `frontmatter` dans tout `content` et ne remplace que sa **premiere**
//! occurrence. Ce n est pas `content.replace(regex, ...)`, ou la position
//! serait heritee de la correspondance : rien n est ancre, et la recherche
//! repart de l index 0. C est `str::replacen` avec un motif `&str` et un
//! compte de 1, ni plus ni moins.
//!
//! Deux consequences, une observable et une non, et il faut savoir
//! laquelle :
//!
//! - **Observable** : si le corps du document repete exactement le texte du
//!   frontmatter, seule la premiere occurrence est reecrite. Le corps n est
//!   pas protege. Test dedie.
//! - **Non observable** : la position de la premiere occurrence ne peut pas
//!   diverger de la position reelle. Pour que la chaine soit trouvee avant
//!   l offset 4, il faudrait qu elle soit un prefixe de `---\n`, donc qu elle
//!   commence par `-`. Une telle premiere ligne ne peut pas etre reecrite
//!   (`[a-zA-Z_]` refuse `-`), donc le remplacement la laisse inchangee de toute
//!   facon. Le bug latent est donc bien present dans la source, mais
//!   inobservable ici : un portage ancre serait equivalent, et un portage ancre
//!   serait meme **plus** faithful sur une source corrigee. On transporte la
//!   mecanique exacte plutot que de pretendre corriger.
//!
//! Le second piege est `result.join("\n")` : meme sur un document CRLF, la zone
//! reecrite repasse en LF, alors que le reste du fichier garde ses CRLF. Le
//! melange de fins de ligne resultant est voulu.
//!
//! ## Le piege du jeu de caracteres de la cle
//!
//! `/^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(.*)$/` n autorise dans la cle que des
//! lettres ASCII, des chiffres et le tiret bas. **Pas de tiret, pas de point.**
//! `my-key: a: b` n est donc pas reecrit et reste du YAML invalide : apres
//! `sanitize`, `matter` echoue toujours, et `parse_option` rend `undefined`.
//! C est le comportement de la source. Un portage qui elargirait la cle a
//! `[A-Za-z0-9_.-]+` « reussirait » sur ces fichiers la ou la source echoue, et
//! surtout accepterait des frontmatters que le TypeScript refuse.
//!
//! ## Aucun test ne touche au systeme de fichiers
//!
//! `sanitize` est une fonction pure sur `&str` : tous les tests du module sont
//! instantanes, sans thread, sans attente, sans `sleep`, et sans disque. C est
//! la raison pour laquelle le portage complet de la logique est possible malgre
//! l absence de `gray-matter` : la seule partie testable du fichier n a
//! besoin d aucune dependance.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Le contenu d'un fichier de configuration ecrit en Markdown avec frontmatter.
///
/// Cette structure est le **resultat** de `matter(content)` : elle n est
/// produite par aucune fonction de ce fichier, `gray-matter` n etant pas une
/// dependance du projet. Elle est declaree pour fixer la forme que les
/// appelants doivent attendre, et pour que le jour ou le lecteur de YAML
/// arrive, la seule chose a ecrire soit le corps de [`parse`].
///
/// `data` est l objet YAML du frontmatter, `content` est le corps du document,
/// delimites une fois le `---` de fermeture retire.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    /// Les cles du frontmatter, converties en objet. `Map` se serialise en
    /// objet JSON, comme le fait l objet simple de `gray-matter`.
    pub data: Map<String, Value>,

    /// Le document prive de son frontmatter, devant la ligne d ouverture
    /// fermee. Les appelants y font `.trim()` avant de l utiliser comme corps
    /// de prompt ou de commande.
    pub content: String,
}

/// Ce qui peut empecher [`parse`] et [`parse_option`] de rendre un resultat.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarkdownError {
    /// Le module `gray-matter` n est pas une dependance de `Cargo.toml` : ni
    /// `parse` ni `parse_option` ne peuvent etre portees.
    #[error("le module gray-matter n est pas une dependance du projet : parse et parse_option ne sont pas portees")]
    BibliothequeGrayMatterAbsente,
}

/// Extrait le frontmatter d un document, comme `/^---\r?\n([\s\S]*?)\r?\n---/`.
///
/// Rend le **groupe 1**, c est a dire le texte situe entre la ligne d
/// ouverture et la premiere ligne de fermeture, delimitateur de fin de ligne
/// compris.
///
/// Deux points ou la regexp est plus permissive qu on ne le croit :
///
/// - elle est ancree au **debut** du document. `" ---\n..."` ne correspond
///   pas, et le contenu est rendu tel quel ;
/// - la correspondance est **paresseuse**, elle s arrete a la premiere
///   occurrence de `\n---` ou `\r\n---`. Un `---` au milieu d une valeur
///   ferme donc le frontmatter, et ce qui suit est du corps ;
/// - le `---` de fermeture n a pas besoin d etre seul sur sa ligne :
///   `---\nkey: a: b\n---corps` correspond, avec `key: a: b` comme frontmatter.
///
/// Le rendu `None` signifie « pas de frontmatter du tout ». Il ne faut pas le
/// confondre avec un frontmatter **vide** : `---\n\n---` rend `Some("")`, et
/// la source continue de traitement dans ce cas. Le test de nullite est donc
/// `is_none`, jamais `is_empty`.
fn frontmatter_de<'a>(contenu: &'a str) -> Option<&'a str> {
    let apres_ouverture = contenu.strip_prefix("---")?;
    let corps = match apres_ouverture.strip_prefix("\r\n") {
        Some(reste) => reste,
        None => apres_ouverture.strip_prefix('\n')?,
    };

    // `char_indices` donne les positions de frontieres de caractere, dans
    // l ordre : la premiere qui convient est donc bien la plus a gauche, ce
    // qui reproduit la quantification paresseuse de `[\s\S]*?`.
    for (index, _) in corps.char_indices() {
        let suite = &corps[index..];
        if suite.starts_with("\r\n---") || suite.starts_with("\n---") {
            return Some(&corps[..index]);
        }
    }
    None
}

/// Decoupe un frontmatter en lignes, comme `frontmatter.split(/\r?\n/)`.
///
/// Le retour chariot de fin de ligne est consomme, celui qui se trouve au milieu
/// d une ligne ne l est pas : `"a\r\nb"` rend `["a", "b"]`, `"a\r\rb"` rend
/// `["a\r\rb"]`, et un saut de ligne final rend une derniere ligne vide.
fn diviser_lignes(frontmatter: &str) -> Vec<&str> {
    frontmatter
        .split('\n')
        .map(|ligne| ligne.strip_suffix('\r').unwrap_or(ligne))
        .collect()
}

/// Dit si un texte contient un terminateur de ligne que le `.` d une regexp
/// JavaScript ne franchit pas.
///
/// En JavaScript, `.` ne correspond ni a `\n`, ni a `\r`, ni a `U+2028`, ni a
/// `U+2029`. Consequence directe : la regexp de la source est ancree par un
/// `$` qui n existe qu en fin de chaine, donc une valeur contenant l un de ces
/// caracteres ne correspond **pas du tout**. En Rust, un simple slicing
/// matcherait et reecrirait la ligne ; il faut donc refuser explicitement.
///
/// `\n` est absent de la liste : les lignes ont deja ete decoupees par
/// [`diviser_lignes`], aucune n en peut contenir.
fn contient_terminateur(texte: &str) -> bool {
    texte
        .chars()
        .any(|c| c == '\r' || c == '\u{2028}' || c == '\u{2029}')
}

/// Reecrit une ligne de frontmatter qui contient un deux points non cite.
///
/// Rend `None` quand la ligne doit etre conservee telle quelle, et
/// `Some((ligne_de_remplacement, continuation_indentee))` sinon.
///
/// Les six tests sont dans l ordre de la source, et chacun peut rendre la ligne
/// intacte :
///
/// 1. commentaire (`line.trim().startsWith("#")`) ;
/// 2. ligne vide apres trim ;
/// 3. ligne indented : c est la suite d une valeur de bloc ou d une cle
///    imbriquee, jamais une cle de premier niveau ;
/// 4. la regexp de cle `^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*(.*)$` ne correspond
///    pas — cle avec tiret, avec point, ou commencee par un chiffre ;
/// 5. la valeur, **apres trim**, est vide, vaut exactement `>` ou exactement
///    `|`, ou commence par un guillemet simple ou double ;
/// 6. la valeur ne contient aucun deux points : il n y a rien a corriger.
///
/// Le test 5 est une egalite stricte, pas un prefixe : `>-` et `|-` ne sont
/// **pas** couverts, et une valeur `>- a: b` est donc reecrite. C est le
/// comportement de la source.
fn reecrire_ligne(ligne: &str) -> Option<(String, String)> {
    // 1. et 2. commentaire ou ligne vide, meme si elle est indentede
    let sans_bord = ligne.trim();
    if sans_bord.starts_with('#') || sans_bord.is_empty() {
        return None;
    }

    // 3. `/^\s+/` : toute ligne indentede est laissee intacte
    if ligne.starts_with(char::is_whitespace) {
        return None;
    }

    let octets = ligne.as_bytes();
    if octets.is_empty() {
        return None;
    }

    // 4. le jeu de caracteres de la cle : ASCIIMajuscule, minuscule,
    // chiffre et tiret bas, premier caractere lettre ou tiret bas
    if !(octets[0].is_ascii_alphabetic() || octets[0] == b'_') {
        return None;
    }
    let mut curseur = 1;
    while curseur < octets.len()
        && (octets[curseur].is_ascii_alphanumeric() || octets[curseur] == b'_')
    {
        curseur += 1;
    }
    let cle = &ligne[..curseur];

    // Les deux `\s*` autour du deux points, puis la capture `(.*)`.
    let apres_cle = ligne[curseur..].trim_start();
    let apres_virgule = match apres_cle.strip_prefix(':') {
        Some(reste) => reste,
        None => return None,
    };
    let capture = apres_virgule.trim_start();

    // Le `.` de la regexp ne franchit pas `\r`, `U+2028` ni `U+2029`, et le
    // `$` ancre la fin de la ligne : une capture qui en contient un empeche
    // toute correspondance.
    if contient_terminateur(capture) {
        return None;
    }

    // `entry[2].trim()`, le trim de la source
    let valeur = capture.trim_end();

    // 5. les cinq cas de valeurs qu on ne touche pas
    if valeur.is_empty() || valeur == ">" || valeur == "|" {
        return None;
    }
    if valeur.starts_with('"') || valeur.starts_with('\'') {
        return None;
    }

    // 6. sans deux point dans la valeur, il n y a rien a assainir
    if !valeur.contains(':') {
        return None;
    }

    // La cle est reecrite sans les espaces qui la separaient du deux points,
    // et la valeur est reindentée de deux espaces, exactement comme le
    // template `${entry[1]}: |-` puis `  ${value}`.
    Some((format!("{}: |-", cle), format!("  {}", valeur)))
}

/// Rend un document Markdown lisible par `gray-matter`, en repliant les
/// valeurs de frontmatter qui contiennent un deux points non cite.
///
/// C est le repli des agents de programmation qui acceptent
/// `command: npx tsc --noEmit` la ou YAML exigerait des guillemets. La
/// transformation est reversible en intention : la valeur est placee dans un
/// bloc litteral `|-`, dont le YAML rendra exactement les memes caracteres.
///
/// Le document est rendu **identique** si et seulement si aucune ligne de
/// frontmatter n avait besoin d etre reecrite. Le reste du document, closing
/// delimiter et corps compris, n est jamais touche : seul le texte du
/// frontmatter est recherche et remplace.
///
/// La fonction est pure, sans effet de bord, et ne lit rien sur le disque.
pub fn sanitize(content: &str) -> String {
    // `!match` est un test de nullite : le groupe vide est un groupe valide.
    let frontmatter = match frontmatter_de(content) {
        Some(texte) => texte,
        None => return content.to_string(),
    };

    let mut lignes: Vec<String> = Vec::new();
    for ligne in diviser_lignes(frontmatter) {
        match reecrire_ligne(ligne) {
            Some((remplacement, continuation)) => {
                lignes.push(remplacement);
                lignes.push(continuation);
            }
            None => lignes.push(ligne.to_string()),
        }
    }

    // `String.prototype.replace(chaine, fonction)` : premiere occurrence
    // seulement, et aucune interpretation de `$` dans le remplacement, ce que
    // donne aussi `str::replacen` avec un motif `&str`.
    content.replacen(frontmatter, &lignes.join("\n"), 1)
}

/// Lit un document Markdown et rend son frontmatter.
///
/// **Non portee.** La source appelle `matter(content)`, puis, en cas d echec,
/// `matter(sanitize(content))` : c est ce second essai, avec le document
/// assaini, qui permet aux agents tiers d ecrire des YAML invalides. Le module
/// `gray-matter` n est pas une dependance de `Cargo.toml`, donc les deux
/// appels sont absents.
///
/// La forme de retour est exacte : la source **leve**, elle ne rend jamais
/// `undefined`. Une version qui rendrait `Option` ici serait un changement de
/// contrat, et c est [`parse_option`] qui porte l absorption en `undefined`.
///
/// Le point d accroche est complet : `sanitize` est portee, donc le jour ou
/// `gray-matter` arrive, il ne reste que le `try`/`catch` et l appel a
/// `matter` a ecrire.
pub fn parse(content: &str) -> Result<Frontmatter, MarkdownError> {
    let _ = content;
    Err(MarkdownError::BibliothequeGrayMatterAbsente)
}

/// Comme [`parse`], mais rend `None` au lieu de lever.
///
/// **Non portee**, pour la meme raison et avec le meme code d erreur.
///
/// C est la forme qu utilisent les trois appelants du depot, tous sous la forme
/// `if (!markdown) return`. Le `None` rend ici n est donc **pas** une chaine
/// vide ni un frontmatter vide : `matter("")` ne leve pas, il rend un objet
/// dont `data` est vide. Traduire l echec par une chaine vide confondrait les
/// deux et ferait passer un fichier sans frontmatter pour un fichier casse.
pub fn parse_option(content: &str) -> Option<Frontmatter> {
    let _ = content;
    None
}

#[cfg(test)]
mod tests {
    use super::{
        frontmatter_de, parse, parse_option, sanitize, Frontmatter, MarkdownError,
    };

    // ---------------------------------------------------------------- forme
    // ---------------------------------------------------------------- initiale

    #[test]
    fn un_contenu_sans_frontmatter_est_rendu_identique() {
        // `content.match(...)` rend `null`, et `!null` est vrai : la fonction
        // rend le contenu tel quel, sans jamais le normaliser.
        for contenu in [
            "",
            "x",
            "# titre\n\ndu texte",
            "--\nkey: a: b\n--",
            "----\nkey: a: b\n----",
            "texte\n---\nkey: a: b\n---",
        ] {
            assert_eq!(sanitize(contenu), contenu, "ne devrait pas etre touche : {:?}", contenu);
        }
    }

    #[test]
    fn le_frontmatter_doit_commencer_le_document() {
        // L ancre `^` est en debut de chaine, pas en debut de ligne : une
        // ligne d espace avant l ouverture casse la correspondance.
        let contenu = " ---\nkey: a: b\n---\ncorps";
        assert_eq!(sanitize(contenu), contenu);

        let contenu = "\n---\nkey: a: b\n---\ncorps";
        assert_eq!(sanitize(contenu), contenu);
    }

    #[test]
    fn un_frontmatter_vide_est_different_de_l_absence_de_frontmatter() {
        // Le point le plus facile a perdre : `!match` teste la nullite du
        // tableau, pas son emptiness. Un groupe vide donne quand meme un
        // tableau, donc la source continue. Rendre `is_empty()` a la place
        // ferait disparaitre le traitement de ce cas.
        assert_eq!(sanitize("---\n\n---\ncorps"), "---\n\n---\ncorps");

        // Et le frontmatter vide n absorbe pas une ligne invalide qui
        // suivrait : il n y en a aucune, le document est inchange.
        let avec_vide = "---\n\n---";
        assert_eq!(frontmatter_de(avec_vide), Some(""));
        let sans_frontmatter = "x";
        assert_eq!(frontmatter_de(sans_frontmatter), None);
    }

    // ---------------------------------------------------------------- reecriture
    // ---------------------------------------------------------------- de la valeur

    #[test]
    fn une_valeur_non_citee_avec_un_deux_points_devient_un_bloc_litteral() {
        assert_eq!(
            sanitize("---\nkey: a: b: c\n---\ncorps"),
            "---\nkey: |-\n  a: b: c\n---\ncorps"
        );
        // Le cas reel : une commande shell, que YAML refuse sans guillemets.
        assert_eq!(
            sanitize("---\ncommand: npx tsc --noEmit --strict: true\n---\n"),
            "---\ncommand: |-\n  npx tsc --noEmit --strict: true\n---\n"
        );
    }

    #[test]
    fn la_cle_est_reecrite_sans_les_espaces_qui_allaient_vers_le_deux_points() {
        // `${entry[1]}` est le groupe de capture, donc la cle sans ses espaces.
        assert_eq!(
            sanitize("---\nkey   : a: b\n---"),
            "---\nkey: |-\n  a: b\n---"
        );
        assert_eq!(
            sanitize("---\nkey:     a: b\n---"),
            "---\nkey: |-\n  a: b\n---"
        );
    }

    #[test]
    fn une_valeur_sans_deux_points_n_est_pas_reecrite() {
        // Le sixieme test : pas de deux point, rien a assainir.
        for ligne in ["key: plain", "key: 42", "key: true", "key: a - b"] {
            let contenu = format!("---\n{}\n---\ncorps", ligne);
            assert_eq!(sanitize(&contenu), contenu, "{} ne doit pas bouger", ligne);
        }
    }

    #[test]
    fn une_valeur_vide_est_laissee_telle_quelle() {
        // `value === ""` est une egalite stricte sur la chaine apres trim.
        // `key:` reste `key:`, donc la cle vaut `null` en YAML : c'est une
        // configuration deliberee, pas une ligne cassee.
        for contenu in ["---\nkey:\n---", "---\nkey:   \n---", "---\nkey:\t\n---"] {
            assert_eq!(sanitize(contenu), contenu);
        }
        // Et ce n'est pas un test de nullite non plus : `0` est une valeur.
        assert_eq!(sanitize("---\nkey: 0\n---"), "---\nkey: 0\n---");
    }

    #[test]
    fn les_marqueurs_de_bloc_existants_ne_sont_pas_touches() {
        // `>` et `|` exacts sont exclus, parce que la source teste une
        // egalite et non un prefixe.
        for contenu in ["---\na: >\nb: |\n---", "---\na: >\n---"] {
            assert_eq!(sanitize(contenu), contenu);
        }
        // Mais `>-` et `|-` ne le sont pas. Une valeur qui commence par un
        // de ces marqueurs **et** contient un deux points est donc reecrite,
        // et son contenu passe en bloc litteral.
        assert_eq!(
            sanitize("---\nc: >- a: b\n---"),
            "---\nc: |-\n  >- a: b\n---"
        );
        assert_eq!(
            sanitize("---\nd: |- x: y\n---"),
            "---\nd: |-\n  |- x: y\n---"
        );
    }

    #[test]
    fn les_valeurs_citees_ne_sont_pas_reecrites() {
        // Guillemets simples ou doubles : deja du YAML valide, et surtout
        // un guillemet de debut n'est pas traite comme un prefixe a
        // reecrire. `'` comme `"`.
        for ligne in ["a: \"x: y\"", "b: 'x: y'", "c: \"a: b\"", "d: 'plain'"] {
            let contenu = format!("---\n{}\n---\ncorps", ligne);
            assert_eq!(sanitize(&contenu), contenu, "{} ne doit pas bouger", ligne);
        }
    }

    // ---------------------------------------------------------------- jeu de caracteres
    // ---------------------------------------------------------------- de la cle

    #[test]
    fn une_cle_avec_un_tiret_n_est_pas_reecrite() {
        // `[a-zA-Z_][a-zA-Z0-9_]*` n autorise pas le tiret. La ligne reste du
        // YAML invalide, `matter` echoue apres `sanitize`, et
        // `parse_option` rend `undefined`. Elargir la cle « reussirait » la
        // ou la source echoue.
        let contenu = "---\nmy-key: a: b\n---\ncorps";
        assert_eq!(sanitize(contenu), contenu);
    }

    #[test]
    fn une_cle_avec_un_point_n_est_pas_reecrite() {
        let contenu = "---\nmy.key: a: b\n---\ncorps";
        assert_eq!(sanitize(contenu), contenu);
    }

    #[test]
    fn une_cle_commencee_par_un_chiffre_n_est_pas_reecrite() {
        let contenu = "---\n1key: a: b\n---\ncorps";
        assert_eq!(sanitize(contenu), contenu);
    }

    #[test]
    fn les_majuscules_des_cles_sont_conservees_telles_quelles() {
        // Le nom de champ ne passe pas par une convention de casse : la cle
        // du YAML est reprise telle quelle, et c'est elle qui fait foi a
        // l'echange avec le TypeScript. `projectID` reste `projectID`.
        let obtenu = sanitize("---\nprojectID: a: b\nsessionID: c\n---\ncorps");
        assert_eq!(
            obtenu,
            "---\nprojectID: |-\n  a: b\nsessionID: c\n---\ncorps"
        );
        assert!(obtenu.contains("projectID: |-"), "la cle doit rester en majuscules");
        assert!(!obtenu.contains("projectId"), "aucune normalisation en camelCase");
        assert!(!obtenu.contains("project_id"), "aucune normalisation en snake_case");
        assert!(!obtenu.contains("sessionId"), "aucune normalisation en camelCase");
    }

    #[test]
    fn la_forme_snake_case_n_est_jamais_normalisee_en_majuscules() {
        // Le symetrique du test precedent, cote lecture : une cle deja en
        // snake_case reste en snake_case, et une cle en camelCase reste en
        // camelCase. Aucune des deux n'est « corrigee » vers l'autre.
        let obtenu = sanitize("---\nproject_id: a: b\nprojectId: c: d\n---\n");
        assert_eq!(
            obtenu,
            "---\nproject_id: |-\n  a: b\nprojectId: |-\n  c: d\n---\n"
        );
        assert!(obtenu.contains("project_id: |-"));
        assert!(obtenu.contains("projectId: |-"));
        assert!(!obtenu.contains("projectID"), "la casse n'est jamais unifiee");
    }

    // ---------------------------------------------------------------- lignes
    // ---------------------------------------------------------------- non concernees

    #[test]
    fn les_commentaires_ne_sont_pas_reecrits() {
        // `line.trim().startsWith("#")` : le test porte sur la ligne trimmee,
        // donc un commentaire indentede est protege deux fois.
        for ligne in ["# key: a: b", "   # key: a: b", "#a: b", "  #"] {
            let contenu = format!("---\n{}\n---", ligne);
            assert_eq!(sanitize(&contenu), contenu, "{} ne doit pas bouger", ligne);
        }
    }

    #[test]
    fn les_lignes_indentees_ne_sont_pas_reecrites() {
        // `/^\s+/` : une ligne indentede est la suite d'une valeur de bloc
        // ou une cle imbriquee, jamais une cle de premier niveau. C'est ce
        // qui rend la reecriture idempotente.
        let obtenu = sanitize("---\nnested:\n  a: b\n  c: d\n---\ncorps");
        assert_eq!(obtenu, "---\nnested:\n  a: b\n  c: d\n---\ncorps");
    }

    #[test]
    fn les_lignes_vides_sont_conservees_avec_leurs_espaces() {
        // La ligne est rendue par reference, jamais reassemblee : une ligne
        // de trois espaces reste une ligne de trois espaces.
        let obtenu = sanitize("---\nkey: a: b\n   \nautre: c: d\n---\ncorps");
        assert_eq!(obtenu, "---\nkey: |-\n  a: b\n   \nautre: |-\n  c: d\n---\ncorps");
    }

    #[test]
    fn un_retour_chariot_iso_dans_une_ligne_empeche_la_correspondance() {
        // `.` ne franchit pas `\r` en JavaScript, et `$` est ancre en fin de
        // chaine : la regexp ne correspond pas, la ligne reste intacte. Un
        // slicing Rust matcherait et reecrirait, ce qui serait faux.
        let obtenu = sanitize("---\nkey: a\rb: c\n---\ncorps");
        assert_eq!(obtenu, "---\nkey: a\rb: c\n---\ncorps");
    }

    // ---------------------------------------------------------------- regexp
    // ---------------------------------------------------------------- de selection

    #[test]
    fn le_delimiteur_de_fermeture_peut_etre_suivi_de_texte() {
        // `\n---` n'est pas `\n---$` : le reste de la ligne apres les trois
        // tirets est du corps de document.
        let obtenu = sanitize("---\nkey: a: b\n---corps");
        assert_eq!(obtenu, "---\nkey: |-\n  a: b\n---corps");
    }

    #[test]
    fn la_correspondance_est_paresseuse() {
        // Le premier `\n---` gagne, meme s'il est au milieu d'une valeur. Le
        // frontmatter s'arrete la, et le vrai closing delimiter plus loin
        // devient du corps.
        let obtenu = sanitize("---\nkey: a: b\nx: ---\n---\nfin");
        assert_eq!(obtenu, "---\nkey: |-\n  a: b\nx: ---\n---\nfin");
    }

    #[test]
    fn le_frontmatter_extrait_ignore_la_fermeture_crlf() {
        // Le `\r` appartient au separateur `\r?\n---`, pas au groupe. Sans ca,
        // la derniere valeur se terminerait par un retour chariot.
        assert_eq!(frontmatter_de("---\r\nkey: a\r\n---\r\ncorps"), Some("key: a"));
        assert_eq!(frontmatter_de("---\nkey: a\n---\ncorps"), Some("key: a"));
        assert_eq!(frontmatter_de("---\nkey: a\n---corps"), Some("key: a"));
    }

    // ---------------------------------------------------------------- remplacement
    // ---------------------------------------------------------------- final

    #[test]
    fn seul_le_premier_bloc_est_remplace() {
        // `String.replace(chaine, ...)` remplace la premiere occurrence de la
        // chaine, ou qu'elle soit. Le corps du document n'est pas protege par
        // une ancre, donc une repetition exacte du frontmatter dans le corps
        // reste intacte.
        let obtenu = sanitize("---\nkey: a: b\n---\nkey: a: b");
        assert_eq!(obtenu, "---\nkey: |-\n  a: b\n---\nkey: a: b");
    }

    #[test]
    fn le_corps_du_document_est_laisse_intact() {
        // Seul le texte du frontmatter est cherche et remplace : ni le
        // closing delimiter, ni le corps, ni le nombre de lignes ne bougent.
        let obtenu = sanitize("---\nkey: a: b\n---\n# Titre\n\nkey: a: b\nautre: c: d\n");
        assert_eq!(
            obtenu,
            "---\nkey: |-\n  a: b\n---\n# Titre\n\nkey: a: b\nautre: c: d\n"
        );
    }

    #[test]
    fn le_crlf_du_document_n_est_reecrit_que_dans_la_zone_assainie() {
        // `result.join("\n")` repasse en LF, alors que le reste du fichier
        // garde ses CRLF. Le melange de fins de ligne est voulu, il vient de
        // la source.
        let obtenu = sanitize("---\r\nkey: a: b\r\n---\r\ncorps\r\n");
        assert_eq!(obtenu, "---\r\nkey: |-\n  a: b\r\n---\r\ncorps\r\n");
    }

    #[test]
    fn assainir_deux_fois_ne_change_plus_rien() {
        // Le resultat est un frontmatter valide : `|-` n'est pas dans la liste
        // des valeurs exclues mais ne contient aucun deux points, et la
        // continuation de deux espaces est indentede donc protegee. C'est ce
        // qui rend le repli de `parse` convergent.
        let une = "---\nkey: a: b\nautre: c\n---\ncorps";
        let deux = sanitize(une);
        assert_eq!(deux, "---\nkey: |-\n  a: b\nautre: c\n---\ncorps");
        assert_eq!(sanitize(&deux), deux);
    }

    // ---------------------------------------------------------------- fonctions
    // ---------------------------------------------------------------- non portees

    #[test]
    fn parse_echoue_tant_que_gray_matter_est_absent() {
        // La source leve, elle ne rend jamais `undefined` : c'est bien un
        // `Result` et non un `Option`.
        assert_eq!(parse("---\nkey: a\n---\ncorps"), Err(MarkdownError::BibliothequeGrayMatterAbsente));
        assert_eq!(parse(""), Err(MarkdownError::BibliothequeGrayMatterAbsente));
    }

    #[test]
    fn parse_option_rend_none_tant_que_gray_matter_est_absent() {
        // Les trois appelants du depot font `if (!markdown) return`, donc
        // c'est bien cette forme qui est attendue.
        assert_eq!(parse_option("---\nkey: a\n---\ncorps"), None);
        assert_eq!(parse_option(""), None);
    }

    #[test]
    fn un_frontmatter_declare_serialise_en_objet_vide() {
        // Le type de retour est fixe des maintenant, y compris sa forme JSON,
        // afin que les appelants aient la forme attendue le jour ou
        // `gray-matter` arrive.
        let vide = Frontmatter::default();
        let json = serde_json::to_value(&vide).unwrap();
        assert_eq!(json, serde_json::json!({ "data": {}, "content": "" }));

        let relu: Frontmatter = serde_json::from_value(json).unwrap();
        assert_eq!(relu, vide);
    }
}
