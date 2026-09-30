//! Portage de `packages/core/src/util/token.ts`.
//!
//! La source tient en cinq lignes, et il n'y a rien d'autre a porter :
//!
//! ```ts
//! export * as Token from "./token"
//!
//! const CHARS_PER_TOKEN = 4
//!
//! export const estimate = (input: string) => Math.max(0, Math.round(input.length / CHARS_PER_TOKEN))
//! ```
//!
//! **Aucune cryptographie ici.** Ni signature, ni hachage, ni comparaison de
//! secret, ni lecture d'un jeton d'authentification. Le mot "token" designe un
//! jeton de contexte, c'est-a-dire une unite de texte du modele, et non une
//! credencial. La seule operation du fichier est une division par quatre. Il
//! n'y a donc rien a affaiblir et rien a inventer.
//!
//! ## La ligne 1 : un reexport de soi-meme
//!
//! `export * as Token from "./token"` ouvre le module sous le nom `Token`,
//! ce qui explique pourquoi tous les appelants ecrivent `Token.estimate(...)`,
//! comme on le voit dans `session/compaction.ts`. Le chemin vise etant le
//! module lui-meme, il n'existe aucun autre fichier a reexporter, et Rust n'a
//! pas d'equivalent de cet alias de namespace : les appelants utilisent
//! directement `util_token::estimate`. Cette ligne n'apporte donc aucun
//! comportement et n'a pas de traduction.
//!
//! ## La seule subtilite : `input.length` compte en UTF-16
//!
//! En JavaScript, `.length` d'une chaine est le nombre de codes units UTF-16,
//! et **pas** le nombre d'octets. En Rust, `str::len()` donne des octets et
//! `str::chars().count()` donne des points de code Unicode : les deux se
//! trompent sur la longueur JS. Un emoji compte pour deux codes units UTF-16
//! alors qu'il ne fait qu'un point de code, et un caractere suivi d'un accent
//! combinant compte pour deux codes units. C'est `str::encode_utf16().count()`
//! qui redonne exactement le `.length` du TypeScript, et c'est donc ce qui est
//! utilise ici. Compter en octets ou en points de code surestimerait d'un
//! facteur deux a quatre la taille d'un texte accentue ou emoji, ce qui
//! fausserait toutes les comparaisons de `session/compaction.ts`.

/// Nombre de caracteres par jeton de contexte.
///
/// La source en fait une constante privee du module, non exportee ; elle
/// reste donc privee ici aussi.
const CHARS_PER_TOKEN: f64 = 4.0;

/// Estime le nombre de jetons de contexte occupes par `input`.
///
/// Equivalent de `Math.max(0, Math.round(input.length / 4))`.
///
/// La longueur est comptee en codes units UTF-16, comme `.length` en
/// JavaScript. `Math.round` arrondit a l'entier le plus proche en poussant vers
/// le haut les moities, ce qui donne le meme resultat que `f64::round` ici
/// puisque la longueur n'est jamais negative. Le `Math.max(0, ...)` est
/// reporte tel quel, bien qu'il soit defensif : une longueur est toujours nulle
/// ou positive, donc il ne change jamais la valeur retournee.
///
/// Le retour est un `i64` signe et non un `usize`, parce que la source rend un
/// `number` JavaScript signe et que les appelants font des soustractions qui
/// peuvent devenir negatives, comme dans `session/compaction.ts` qui compare
/// `Token.estimate(x) > context - summaryOutput`.
pub fn estimate(input: &str) -> i64 {
    let longueur_utf16 = input.encode_utf16().count();
    let jetons = (longueur_utf16 as f64 / CHARS_PER_TOKEN).round().max(0.0);
    jetons as i64
}

#[cfg(test)]
mod tests {
    use super::estimate;

    #[test]
    fn une_chaine_vide_donne_zero_jeton() {
        assert_eq!(estimate(""), 0);
    }

    #[test]
    fn un_seul_caractere_donne_zero_jeton() {
        assert_eq!(estimate("a"), 0);
    }

    #[test]
    fn une_chaine_de_quatre_caracteres_donne_exactement_un_jeton() {
        assert_eq!(estimate("abcd"), 1);
    }

    #[test]
    fn une_chaine_de_huit_caracteres_donne_exactement_deux_jetons() {
        assert_eq!(estimate("abcdefgh"), 2);
    }

    #[test]
    fn une_longueur_aux_moitie_dun_jeton_arrondit_au_jeton_suivant() {
        // 2 caracteres : 2 / 4 vaut exactement 0.5, qui monte a 1.
        assert_eq!(estimate("ab"), 1);
        // 6 caracteres : 6 / 4 vaut exactement 1.5, qui monte a 2.
        assert_eq!(estimate("abcdef"), 2);
        // 10 caracteres : 10 / 4 vaut exactement 2.5, qui monte a 3.
        assert_eq!(estimate("abcdefghij"), 3);
    }

    #[test]
    fn trois_caracteres_donnent_deja_un_jeton_entier() {
        // 3 / 4 vaut 0.75, qui monte a 1 : c'est le cas le plus trompeur.
        assert_eq!(estimate("abc"), 1);
    }

    #[test]
    fn un_caractere_accentue_compte_pour_un_seul_caractere() {
        // Quatre e accentues : 4 caracteres, donc 1 jeton. La meme chaine
        // occupe 8 octets en memoire et donnerait 2 jetons si on comptait les
        // octets.
        let texte = "\u{00e9}\u{00e9}\u{00e9}\u{00e9}";
        assert_eq!(texte.len(), 8);
        assert_eq!(estimate(texte), 1);
    }

    #[test]
    fn un_emoji_hors_du_plan_de_base_compte_pour_deux_caracteres() {
        // Chaque emoji fait un seul point de code mais deux codes units
        // UTF-16. Quatre emoji font donc 8 caracteres, soit 2 jetons, alors
        // qu'un compte par points de code n'en donnerait qu'un, et qu'un
        // compte par octets en donnerait quatre.
        let texte = "\u{1F600}\u{1F600}\u{1F600}\u{1F600}";
        assert_eq!(texte.chars().count(), 4);
        assert_eq!(estimate(texte), 2);
    }

    #[test]
    fn un_texte_melange_ne_compte_que_ses_caracteres_reels() {
        // "salut" + 3 e accentues + 2 emojis : 5 + 3 + 2 x 2 = 12 caracteres,
        // soit 3 jetons.
        let texte = "salut\u{00e9}\u{00e9}\u{00e9}\u{1F600}\u{1F600}";
        assert_eq!(estimate(texte), 3);
    }
}
