//! Portage de `packages/core/src/util/wildcard.ts`.
//!
//! La source n'exporte qu'une fonction, `match(input, pattern)`, qui convertit
//! un motif a jokers (`*` et `?`) en expression reguliere puis teste l'entree.
//!
//! La premiere ligne de la source, `export * as Wildcard from "./wildcard"`,
//! est un reexport de l'espace de noms du module sur lui-meme, afin que les
//! appelants ecrivent `Wildcard.match(...)`. En Rust le module joue deja ce
//! role : il n'y a rien a porter la-dessus.
//!
//! On reproduit fidelement la *construction* de la regexp, pas un moteur de
//! regexp. La transformation ne peut produire qu'un langage trivial : des
//! litteraux, `.`, `.*`, et au plus un groupe optionnel `( .*)?` en fin de
//! motif. Un petit automate suffit donc, et evite d'ajouter une dependance
//! externe. L'automate memorise l'ensemble des positions atteignables, ce qui
//! evite l'explosion combinatoire du retour arriere des regexp.
//!
//! Point d'attention : `match` est un mot cle reserve de Rust, la fonction
//! porte donc le nom brut `r#match`. Les appelants ecrivent
//! `util_wildcard::r#match(entree, motif)`.

use std::collections::BTreeSet;

/// Sous Windows, la source ajoute le drapeau `i` (insensible a la casse) a la
/// regexp ; partout ailleurs elle n'ajoute que `s` (`.` traverse les retours a
/// la ligne). On fige ce choix a la compilation, comme le fait `process.platform`
/// cote TypeScript, mais via `r#match_with_case` pour pouvoir tester les deux
/// branches.
pub const CASE_INSENSITIVE: bool = cfg!(target_os = "windows");

/// Un element du motif, une fois la regexp construite.
enum Node {
    /// Caractere litteral, y compris un caractere precedement echappe.
    Literal(char),
    /// Le `.` : n'importe quel caractere, retour a la ligne compris.
    Any,
    /// Le `.*` : zero caractere ou plus.
    AnyStar,
    /// Un groupe `( ... )`, avec le booleen qui dit s'il est optionnel (`?`).
    Group(Vec<Node>, bool),
}

/// Rejoue les remplacements de la source, dans le meme ordre, et renvoie le
/// texte de la regexp avant l'ajout des ancres `^` et `$`.
///
/// L'ordre compte : l'echappement des metacaracteres passe avant la conversion
/// des jokers, et la detection de la fin ouverte ` .*` passe apres.
fn to_regex(pattern: &str) -> Vec<char> {
    // 1. toute contre-oblique du motif devient une barre oblique
    let text: Vec<char> = pattern.replace('\\', "/").chars().collect();

    // 2. echappement des metacaracteres de regexp. Le `-` et le `/` n'en font
    //    pas partie : ils n'ont aucun sens hors d'une classe de caracteres, or
    //    la source n'en produit jamais. La contre-oblique n'y est pas non plus,
    //    la source l'echappe aussi, mais l'etape 1 vient de la supprimer.
    let mut protected: Vec<char> = Vec::with_capacity(text.len());
    for c in text {
        if matches!(
            c,
            '.' | '+' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']'
        ) {
            protected.push('\\');
        }
        protected.push(c);
    }

    // 3. l'etoile devient `.*`
    let mut starred: Vec<char> = Vec::with_capacity(protected.len());
    for c in protected {
        if c == '*' {
            starred.push('.');
        }
        starred.push(c);
    }

    // 4. le point d interrogation disparait et devient un `.` : le motif n'a
    //    aucun moyen d'echapper un `?`, il perd donc son sens de joker
    //    optionnel, il devient un joker "n'importe quoi".
    let mut marked: Vec<char> = Vec::with_capacity(starred.len());
    for c in starred {
        if c == '?' {
            marked.push('.');
        } else {
            marked.push(c);
        }
    }

    // 5. une fin ` .*` devient un groupe optionnel `( .*)?` : l'espace puis
    //    n'importe quoi devient facultatif, le reste du motif reste obligatoire
    let n = marked.len();
    if n >= 3 && marked[n - 3] == ' ' && marked[n - 2] == '.' && marked[n - 1] == '*' {
        marked.truncate(n - 3);
        marked.extend(['(', ' ', '.', '*', ')', '?']);
    }

    marked
}

/// Decoupe la regexp construite en noeuds.
///
/// Le seul groupe possible est celui fabrique a l'etape 5 ; il est plat, donc
/// une simple recherche du `)` fermant suffit.
fn compile(source: &[char]) -> Vec<Node> {
    let mut nodes: Vec<Node> = Vec::new();
    let mut i = 0;
    while i < source.len() {
        match source[i] {
            '\\' if i + 1 < source.len() => {
                nodes.push(Node::Literal(source[i + 1]));
                i += 2;
            }
            '(' => {
                let start = i + 1;
                let mut end = start;
                while end < source.len() && source[end] != ')' {
                    end += 1;
                }
                if end == source.len() {
                    // groupe jamais ferme : la source aurait echappe cette
                    // parenthesis, ce cas n'arrive pas, on la traite en litteral
                    nodes.push(Node::Literal('('));
                    i += 1;
                } else {
                    let inner = compile(&source[start..end]);
                    let optional = end + 1 < source.len() && source[end + 1] == '?';
                    nodes.push(Node::Group(inner, optional));
                    i = if optional { end + 2 } else { end + 1 };
                }
            }
            '.' if i + 1 < source.len() && source[i + 1] == '*' => {
                nodes.push(Node::AnyStar);
                i += 2;
            }
            '.' => {
                nodes.push(Node::Any);
                i += 1;
            }
            c => {
                nodes.push(Node::Literal(c));
                i += 1;
            }
        }
    }
    nodes
}

/// Comparaison de deux caracteres, en tenant compte du drapeau `i`.
fn same(expected: char, found: char, ignore_case: bool) -> bool {
    if expected == found {
        return true;
    }
    // `to_lowercase` est plus proche du repli simple de JavaScript qu'un
    // `eq_ignore_ascii_case`, qui ignorerait les lettres accentuees
    ignore_case && expected.to_lowercase().eq(found.to_lowercase())
}

/// Ajoute dans `ends` toutes les positions de l'entree atteignables apres avoir
/// consomme `nodes` a partir de `pos`.
///
/// On garde un ensemble de positions plutot qu'un retour arriere avec un unique
/// resultat : l'ordre d'essai n'a plus d'importance et le nombre d'appels reste
/// borne meme avec plusieurs `.*` de suite.
fn walk(nodes: &[Node], input: &[char], pos: usize, ignore_case: bool, ends: &mut BTreeSet<usize>) {
    let (head, rest) = match nodes.split_first() {
        Some(pair) => pair,
        None => {
            ends.insert(pos);
            return;
        }
    };

    match head {
        Node::Literal(c) => {
            if pos < input.len() && same(*c, input[pos], ignore_case) {
                walk(rest, input, pos + 1, ignore_case, ends);
            }
        }
        Node::Any => {
            if pos < input.len() {
                walk(rest, input, pos + 1, ignore_case, ends);
            }
        }
        Node::AnyStar => {
            for next in pos..=input.len() {
                walk(rest, input, next, ignore_case, ends);
            }
        }
        Node::Group(inner, optional) => {
            let mut inner_ends = BTreeSet::new();
            walk(inner, input, pos, ignore_case, &mut inner_ends);
            for end in inner_ends {
                walk(rest, input, end, ignore_case, ends);
            }
            if *optional {
                walk(rest, input, pos, ignore_case, ends);
            }
        }
    }
}

/// La regexp de la source est toujours ancree, donc la correspondance est
/// exacte : l'entree doit etre entierement consommee.
fn matches_at_end(nodes: &[Node], input: &[char], ignore_case: bool) -> bool {
    let mut ends = BTreeSet::new();
    walk(nodes, input, 0, ignore_case, &mut ends);
    ends.contains(&input.len())
}

/// Variante exposing le drapeau d'insensibilite a la casse, pour pouvoir
/// tester le comportement des deux plateformes.
pub fn r#match_with_case(input: &str, pattern: &str, ignore_case: bool) -> bool {
    // l'entree est normalisee comme le motif : contre-oblique vers barre oblique
    let normalized: Vec<char> = input.replace('\\', "/").chars().collect();
    let nodes = compile(&to_regex(pattern));
    matches_at_end(&nodes, &normalized, ignore_case)
}

/// Dit si `input` correspond au motif a jokers `pattern`.
///
/// Le drapeau de casse suit la plateforme de compilation, exactement comme
/// `process.platform === "win32"` dans la source. Le drapeau `s`, lui, est
/// toujours actif : le `.` traverse les retours a la ligne sur toute
/// plateforme.
pub fn r#match(input: &str, pattern: &str) -> bool {
    r#match_with_case(input, pattern, CASE_INSENSITIVE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_motif_vide_ne_correspond_qu_a_une_chaine_vide() {
        assert!(r#match("", ""));
        assert!(!r#match("a", ""));
        assert!(r#match("", "*"));
        assert!(!r#match("", "?"));
    }

    #[test]
    fn une_etoile_seule_correspond_a_toute_la_chaine() {
        assert!(r#match("", "*"));
        assert!(r#match("a", "*"));
        assert!(r#match("a/b/c", "*"));
        assert!(r#match("ligne 1\nligne 2", "*"));
    }

    #[test]
    fn l_etoile_en_debut_au_milieu_et_a_la_fin_couvre_ce_qui_est_autour() {
        assert!(r#match("/foo/bar", "*/bar"));
        assert!(!r#match("/foo/baz", "*/bar"));
        assert!(r#match("foobar", "foo*"));
        assert!(r#match("foo", "foo*"));
        assert!(!r#match("afoo", "foo*"));
        assert!(r#match("barfoo", "*foo"));
        assert!(!r#match("foobar", "*foo"));
    }

    #[test]
    fn plusieurs_etoiles_ou_points_d_interrogation_se_comportent_indepamment() {
        // `**` devient `.*.*`, les deux etoiles sont sans effet l'une sur l'autre
        assert!(r#match("axxb", "a**b"));
        assert!(r#match("ab", "a**b"));
        // `?` disparait et devient `.` : il matche exactement un caractere
        assert!(r#match("axb", "a?b"));
        assert!(!r#match("ab", "a?b"));
        assert!(!r#match("axxb", "a?b"));
        assert!(r#match("abc", "???"));
        assert!(!r#match("ab", "???"));
    }

    #[test]
    fn les_caracteres_speciaux_du_motif_sont_des_litteraux() {
        // le `.` du motif est echappe, donc il matche un point et rien d'autre
        assert!(r#match("a.b", "a.b"));
        assert!(!r#match("axb", "a.b"));
        assert!(r#match("a+b", "a+b"));
        assert!(!r#match("aab", "a+b"));
        assert!(r#match("a|b", "a|b"));
        assert!(!r#match("a", "a|b"));
        assert!(r#match("(a)", "(a)"));
        assert!(r#match("[a]", "[a]"));
        assert!(r#match("^a$", "^a$"));
        assert!(r#match("{a}", "{a}"));
    }

    #[test]
    fn les_contre_obliques_sont_normalisees_en_barres_obliques_des_deux_cotes() {
        assert!(r#match("a\\b", "a/b"));
        assert!(r#match("a/b", "a\\b"));
        assert!(r#match("a\\b\\c", "a/*/c"));
        assert!(!r#match("a\\b", "a/b/c"));
    }

    #[test]
    fn une_fin_etoile_avec_espace_laisse_la_fin_du_motif_facultative() {
        // le groupe optionnel exige un espace quand il est pris
        assert!(r#match("foo", "foo *"));
        assert!(r#match("foo ", "foo *"));
        assert!(r#match("foo bar", "foo *"));
        assert!(!r#match("foobar", "foo *"));
        // le motif strict ne gagne rien de plus
        assert!(r#match("foo", "foo"));
        assert!(!r#match("foo bar", "foo"));
        // un motif reduit a ` .*` devient entierement optionnel
        assert!(r#match("", " *"));
        assert!(r#match(" abc", " *"));
        assert!(!r#match("abc", " *"));
    }

    #[test]
    fn un_espace_final_sans_etoile_ne_rend_rien_optional() {
        assert!(r#match("foo ", "foo "));
        assert!(!r#match("foo", "foo "));
        assert!(!r#match("foo  ", "foo "));
    }

    #[test]
    fn la_casse_ne_compte_pas_seulement_si_le_drapeau_est_active() {
        // sous Windows `r#match` est insensible a la casse, ailleurs non :
        // on teste donc la variante explicite
        assert!(r#match_with_case("Hello", "h*", true));
        assert!(r#match_with_case("HELLO", "hello*", true));
        assert!(!r#match_with_case("Hello", "h*", false));
        assert!(r#match_with_case("hello", "hello", false));
        // le motif sensible a la casse qui commence par la meme lettre matche
        assert!(r#match_with_case("hello", "h*", false));
    }

    #[test]
    fn un_motif_sans_joker_ne_matche_que_lui_meme() {
        assert!(r#match("edit", "edit"));
        assert!(!r#match("edit ", "edit"));
        assert!(!r#match("edit", "edit "));
        assert!(!r#match("edits", "edit"));
    }
}
