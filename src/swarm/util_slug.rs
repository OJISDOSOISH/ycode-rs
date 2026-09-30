//! Portage de `packages/core/src/util/slug.ts`.
//!
//! ## Ce que fait vraiment cette source
//!
//! Un `namespace` de 74 lignes qui expose **une seule** fonction,
//! `Slug.create()`, et deux tables de mots. Elle ne normalise rien : elle tire
//! au hasard un adjectif, tire au hasard un nom, et les colle avec un tiret.
//!
//! ```ts
//! export function create() {
//!   return [
//!     ADJECTIVES[Math.floor(Math.random() * ADJECTIVES.length)],
//!     NOUNS[Math.floor(Math.random() * NOUNS.length)],
//!   ].join("-")
//! }
//! ```
//!
//! Il n'y a **aucune entree** a normaliser : le module ne recoit aucun
//! parametre. Les accents, la casse, la ponctuation, les espaces multiples, les
//! chaines vides, les caracteres non ASCII et la longueur maximale, qui
//! Parent a demandes de couvrir exhaustivement, **n'existent pas ici** : ils
//! appartiennent a une autre fonction, `slugify`, qui vit dans deux fichiers
//! qui ne sont pas dans mon lot :
//!
//! - `packages/opencode/src/worktree/index.ts:91`
//! - `packages/opencode/src/server/routes/instance/httpapi/handlers/project-copy.ts:76`
//!
//! ```ts
//! function slugify(input: string) {
//!   return input.trim().toLowerCase()
//!     .replace(/[^a-z0-9]+/g, "-").replace(/^-+/, "").replace(/-+$/, "")
//! }
//! ```
//!
//! Elle n'est pas portee ici, et personne dans le swarm ne la porte : c'est une
//! fonction locale, invisible pour le reste du depot, alors que
//! `Slug.create()` sert de repli a `project-copy.ts:27`, `:29`, `:59` et `:68`.
//! Si quelqu'un cherche un "slugify" dans le portage Rust, il n'est pas ici.
//!
//! Ce qui reste a porter est donc court, mais pas vide : le comportement
//! observable tient en quatre proprietes, et ce sont elles que les tests
//! ci-dessous couvrent.
//!
//! ## Propriete 1 : les deux listes n'ont pas la meme taille
//!
//! 29 adjectifs et 31 noms. `ADJECTIVES[i]` et `NOUNS[i]` sont donc indexes par
//! deux bornes differentes, et un index tire avec la mauvaise longueur se
//! voit : il rendrait les deux derniers noms, `wizard` et `wolf`,
//! inatteignables. Un test dedie le verifie.
//!
//! ## Propriete 2 : le format est un invariant fort
//!
//! Les 60 mots sont en minuscules ASCII, sans espace, sans tiret et sans
//! accent. Le slug est donc toujours exactement `mot-tiret-mot`, de 9 a 16
//! octets, toujours en ASCII, jamais vide, avec exactement un tiret. C'est la
//! seule lecture possible de "pas d'accent, pas de non ASCII, longueur
//! maximale" pour cette source : ces trois proprietes sont des garanties de
//! sortie, pas des cas d'entree.
//!
//! ## Propriete 3 : le hasard
//!
//! `Math.floor(Math.random() * n)` est uniforme sur `[0, n)`. Le modulo naive
//! `u64 % n` ne l'est pas quand `n` ne divise pas 2 puissance 64, ce qui est le
//! cas de 29 et de 31. Un tirage par rejet est donc utilise, ce qui rend le
//! modulo exact. C'est la seule divergence de comportement possible, et elle
//! est invisible pour un appelant qui ne demande qu'un nom.
//!
//! ## Ecart assume : la source du hasard
//!
//! Le TypeScript tire dans le PRNG du moteur V8. `Cargo.toml` ne declare ni
//! `rand` ni `getrandom`, et ce fichier n'a pas le droit de le modifier :
//! SplitMix64 est donc utilise, amorce avec les cles aleatoires de `RandomState`
//! de la bibliotheque standard, qui elle-meme vient de l'OS. La loi de
//! distribution est la meme, la suite de valeurs ne l'est evidemment pas.
//! Meme Choix que `util_identifier.rs`, pour que le lot soit homogene.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::{Mutex, MutexGuard};

/// Les 29 adjectifs, dans l'ordre exact du tableau `ADJECTIVES`.
///
/// Tables priveses dans la source, donc privees ici aussi. Les tests y ont
/// acces par `use super::*`.
const ADJECTIVES: [&str; 29] = [
    "brave",
    "calm",
    "clever",
    "cosmic",
    "crisp",
    "curious",
    "eager",
    "gentle",
    "glowing",
    "happy",
    "hidden",
    "jolly",
    "kind",
    "lucky",
    "mighty",
    "misty",
    "neon",
    "nimble",
    "playful",
    "proud",
    "quick",
    "quiet",
    "shiny",
    "silent",
    "stellar",
    "sunny",
    "swift",
    "tidy",
    "witty",
];

/// Les 31 noms, dans l'ordre exact du tableau `NOUNS`.
///
/// Deux de plus que les adjectifs : voir la propriete 1 dans le documentation
/// du module.
const NOUNS: [&str; 31] = [
    "cabin",
    "cactus",
    "canyon",
    "circuit",
    "comet",
    "eagle",
    "engine",
    "falcon",
    "forest",
    "garden",
    "harbor",
    "island",
    "knight",
    "lagoon",
    "meadow",
    "moon",
    "mountain",
    "nebula",
    "orchid",
    "otter",
    "panda",
    "pixel",
    "planet",
    "river",
    "rocket",
    "sailor",
    "squid",
    "star",
    "tiger",
    "wizard",
    "wolf",
];

/// Le germe du generateur. Zero signifie "pas encore amorce".
static ETAT: Mutex<u64> = Mutex::new(0);

/// Le verrou peut etre empoisonne si un autre thread a panique a l'interieur.
///
/// Comme dans `util_identifier.rs`, on recupere l'etat quand meme : l'origine
/// n'a aucune action a reussir, il n'y a donc rien a proteger : une entree
/// corrompue par un panic reste un slug, pas une donnee durable.
fn verrouiller() -> MutexGuard<'static, u64> {
    match ETAT.lock() {
        Ok(garde) => garde,
        Err(empoisonne) => empoisonne.into_inner(),
    }
}

/// SplitMix64. `wrapping` partout parce que l'arithmetique tourne en anneau
/// modulo 2 puissance 64, y compris en debug ou un depassement paniquerait.
fn suivant(etat: &mut u64) -> u64 {
    *etat = etat.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *etat;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Une graine non nulle, tiree des cles de `RandomState`.
///
/// Le zero est reserve comme marqueur "pas encore amorce" dans [`verrouiller`]
/// et dans [`creer`], donc on l'ecarte explicitement plutot que de croire qu'il
/// ne sortira jamais.
fn amorcer() -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(0x1234_5678_9ABC_DEF0);
    let graine = hasher.finish();
    if graine == 0 {
        0x9E37_79B9_7F4A_7C15
    } else {
        graine
    }
}

/// Un index uniforme dans `[0, longueur)`.
///
/// Le seuil de rejet vaut `2^64 mod longueur`, calcule comme `(-n) mod n` en
/// arithmetique qui deborde. On ne garde que les tirages au-dessus de ce
/// seuil, ce qui laisse exactement un multiple de `n` valeurs possibles et
/// supprime donc le biais du modulo. Pour `n == 1` le seuil vaut zero et
/// l'index rendu est toujours zero, comme `Math.floor(r * 1)`.
///
/// Une longueur nulle rendrait 0 : les deux listes du module ne sont jamais
/// vides, la branche n'est donc jamais prise, mais elle evite une boucle
/// infinie si le portage evolue.
fn tirer_index(etat: &mut u64, longueur: usize) -> usize {
    let n = longueur as u64;
    if n == 0 {
        return 0;
    }
    let seuil = n.wrapping_neg() % n;
    loop {
        let tirage = suivant(etat);
        if tirage >= seuil {
            return (tirage % n) as usize;
        }
    }
}

/// Tire un mot dans `liste`.
///
/// Le retour est `&'static str` et non `&str` parce que toutes les tables du
/// module sont des constantes statiques.
///
/// Une liste vide rend la chaine vide, ce qui reproduit ce que fait
/// `Array.prototype.join` en JavaScript : `join` convertit un `undefined` en
/// chaine vide, donc une liste vide donnerait `"-"` de l'autre cote, et non une
/// panique d'indexation comme le ferait `[0]` en Rust.
fn piocher(etat: &mut u64, liste: &[&'static str]) -> &'static str {
    let index = tirer_index(etat, liste.len());
    liste.get(index).copied().unwrap_or("")
}

/// Le collage du TypeScript : `[adjectif, nom].join("-")`.
fn assembler(adjectif: &str, nom: &str) -> String {
    format!("{}-{}", adjectif, nom)
}

/// Le corps de `Slug.create()`, isole de l'etat global pour etre rejouable.
fn composer(etat: &mut u64) -> String {
    let adjectif = piocher(etat, &ADJECTIVES);
    let nom = piocher(etat, &NOUNS);
    assembler(adjectif, nom)
}

/// Cree un nom aleatoire de la forme `adjectif-nom`.
///
/// Equivaut a `Slug.create()`.
pub fn creer() -> String {
    let mut etat = verrouiller();
    if *etat == 0 {
        *etat = amorcer();
    }
    composer(&mut *etat)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un index dans les bornes, pour n'importe quelle longueur de liste.
    fn borne(etat: &mut u64, longueur: usize) -> bool {
        (0..500).all(|_| tirer_index(etat, longueur) < longueur)
    }

    #[test]
    fn un_slug_a_toujours_la_forme_adjectif_tiret_nom() {
        for _ in 0..2000 {
            let slug = creer();
            let morceaux: Vec<&str> = slug.split('-').collect();
            assert_eq!(morceaux.len(), 2, "pas un seul tiret : {}", slug);
            assert!(!morceaux[0].is_empty(), "adjectif vide : {}", slug);
            assert!(!morceaux[1].is_empty(), "nom vide : {}", slug);
            assert!(
                ADJECTIVES.contains(&morceaux[0]),
                "adjectif inconnu : {}",
                slug
            );
            assert!(NOUNS.contains(&morceaux[1]), "nom inconnu : {}", slug);
        }
    }

    #[test]
    fn un_slug_ne_contient_jamais_de_caractere_hors_ascii() {
        // Les accents, le non ASCII et la casse ne sont pas des entrees
        // traitables ici : ce sont des impossibilites de sortie. Aucun des
        // 60 mots n'a d'accent, donc le slug ne peut pas en avoir.
        for _ in 0..2000 {
            let slug = creer();
            for octet in slug.bytes() {
                assert!(octet < 128, "octet non ASCII dans {}", slug);
            }
            // Le slug n'est compose que de deux mots en minuscules ASCII
            // separes par un seul tiret : ni espace, ni ponctuation, ni
            // accent, ni caractere reserve. Les assertions suivantes sont
            // toutes des consequences de l'invariant du format.
            assert!(
                slug.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "caractere interdit dans {}",
                slug
            );
            assert!(!slug.contains(' '), "espace parasite : {}", slug);
            assert!(!slug.starts_with('-'), "tiret en tete : {}", slug);
            assert!(!slug.ends_with('-'), "tiret en queue : {}", slug);
        }
    }

    #[test]
    fn la_longueur_d_un_slug_va_de_neuf_a_seize_octets_sans_troncature() {
        // Bornes issues des tables, pas des souvenirs. Le mot le plus court
        // d'un cote et le plus long de l'autre : 4 + 1 + 4 pour le plus court,
        // 7 + 1 + 8 pour le plus long. Aucun mot n'est coupe, la source
        // n'applique aucune troncature.
        //
        // "brave" fait cinq lettres et non quatre : c'est "calm", "kind",
        // "neon" et "tidy" qui font quatre. Utiliser "brave" ici ferait
        //echouer le test alors que le code est correct.
        assert_eq!(assembler("calm", "moon").len(), 9);
        assert_eq!(assembler("stellar", "mountain").len(), 16);
        for _ in 0..2000 {
            let slug = creer();
            assert!(slug.len() >= 9, "trop court : {}", slug);
            assert!(slug.len() <= 16, "trop long : {}", slug);
        }
    }

    /// Les mots d'une longueur donnee, dans l'ordre de la table.
    ///
    /// Sert a verifier les bornes de longueur sans dependre du
    /// deregement d'egalite de `min_by_key` et `max_by_key`, qui rendent le
    /// premier element en cas d'egalite pour le premier et le dernier pour
    /// le second. Un test qui choisirait un mot parmi d'autres de meme
    /// longueur serait faux pour une raison qui n'a rien a voir avec le code.
    fn mots_de_longueur(table: &[&str], longueur: usize) -> Vec<&str> {
        table.iter().filter(|m| m.len() == longueur).copied().collect()
    }

    #[test]
    fn les_bornes_de_longueur_sont_bien_celles_des_tables() {
        // Les mots les plus courts et les plus longs, nommes explicitement.
        assert_eq!(mots_de_longueur(&ADJECTIVES, 4), ["calm", "kind", "neon", "tidy"]);
        assert_eq!(
            mots_de_longueur(&ADJECTIVES, 7),
            ["curious", "glowing", "playful", "stellar"]
        );
        assert_eq!(mots_de_longueur(&NOUNS, 4), ["moon", "star", "wolf"]);
        assert_eq!(mots_de_longueur(&NOUNS, 8), ["mountain"]);
        // Et aucun mot ne sort de ces bornes, sinon le test sur la longueur
        // d'un slug pourrait laisser passer un mot trop long sans le voir.
        assert!(ADJECTIVES.iter().all(|m| (4..=7).contains(&m.len())));
        assert!(NOUNS.iter().all(|m| (4..=8).contains(&m.len())));
    }

    #[test]
    fn les_deux_extremes_de_longueur_sont_bien_assembles() {
        // Verification directe des deux valeurs, sans dependre du hasard.
        assert_eq!(assembler("calm", "moon"), "calm-moon");
        assert_eq!(assembler("stellar", "mountain"), "stellar-mountain");
    }

    #[test]
    fn un_slug_n_est_jamais_la_chaine_vide() {
        for _ in 0..500 {
            assert!(!creer().is_empty());
        }
    }

    #[test]
    fn le_mot_avant_le_tiret_n_est_jamais_un_nom() {
        // Les deux tables sont disjointes, donc l'erreur "un seul index tire
        // pour les deux moities" se voit : le premier mot serait un nom.
        for _ in 0..2000 {
            let slug = creer();
            let premier = slug.split('-').next().unwrap();
            assert!(ADJECTIVES.contains(&premier), "nom en tete : {}", slug);
            assert!(!NOUNS.contains(&premier), "nom en tete : {}", slug);
            let second = slug.rsplit('-').next().unwrap();
            assert!(NOUNS.contains(&second), "adjectif en queue : {}", slug);
            assert!(!ADJECTIVES.contains(&second), "adjectif en queue : {}", slug);
        }
    }

    #[test]
    fn les_deux_derniers_noms_de_la_plus_longue_liste_apparaissent() {
        // 31 noms contre 29 adjectifs : si l'index du nom etait tire avec la
        // longueur de la liste des adjectifs, wizard et wolf seraient
        // impossibles a atteindre. C'est le test qui attrape cette erreur.
        let mut vus = 0;
        for _ in 0..20000 {
            let slug = creer();
            if slug.ends_with("-wizard") || slug.ends_with("-wolf") {
                vus += 1;
            }
        }
        assert!(vus > 0, "jamais vus, la borne de la liste des noms est fausse");
    }

    #[test]
    fn les_tables_ne_contiennent_ni_vide_ni_doublon_ni_accident() {
        for table in [&ADJECTIVES[..], &NOUNS[..]] {
            let mut vus: Vec<&str> = Vec::new();
            for mot in table {
                assert!(!mot.is_empty(), "mot vide");
                assert!(!vus.contains(mot), "doublon : {}", mot);
                assert!(mot.is_ascii(), "mot non ASCII : {}", mot);
                assert!(mot.chars().all(|c| c.is_ascii_lowercase()));
                assert!(!mot.contains('-'), "tiret dans un mot : {}", mot);
                vus.push(mot);
            }
        }
        // Aucun mot commun aux deux tables, sinon le test de separation
        // ci-dessus ne prouverait rien.
        for adjectif in ADJECTIVES.iter() {
            assert!(!NOUNS.contains(adjectif), "mot dans les deux tables");
        }
        assert_eq!(ADJECTIVES.len(), 29);
        assert_eq!(NOUNS.len(), 31);
    }

    #[test]
    fn un_meme_amorce_rejouee_donne_le_meme_slug() {
        let mut a = 42u64;
        let mut b = 42u64;
        let premier = composer(&mut a);
        let second = composer(&mut b);
        assert_eq!(premier, second);
    }

    #[test]
    fn un_tirage_par_rejet_reste_dans_les_bornes_de_la_liste() {
        let mut etat = 1u64;
        for longueur in 1..=40usize {
            assert!(borne(&mut etat, longueur), "hors bornes pour {}", longueur);
        }
        // Une liste d'un seul element rend toujours cet element.
        for _ in 0..100 {
            assert_eq!(piocher(&mut etat, &["seul"]), "seul");
        }
    }

    #[test]
    fn une_liste_vide_donne_un_mot_vide_comme_en_javascript() {
        // `[][0]` vaut undefined en JavaScript et `join` le rend en chaine
        // vide : une liste vide donnerait donc "-", pas une panique. La version
        // Rust renvoie "" et l'assemblage produit le meme tiret nu.
        let mut etat = 7u64;
        assert_eq!(piocher(&mut etat, &[]), "");
        assert_eq!(assembler("", ""), "-");
        assert_eq!(assembler("", "moon"), "-moon");
    }

    #[test]
    fn le_mot_tire_vient_de_la_bonne_table() {
        let mut etat = 2024u64;
        for _ in 0..1000 {
            let adjectif = piocher(&mut etat, &ADJECTIVES);
            let nom = piocher(&mut etat, &NOUNS);
            assert!(ADJECTIVES.contains(&adjectif));
            assert!(NOUNS.contains(&nom));
        }
    }

    #[test]
    fn plusieurs_appels_produisent_des_slugs_varies() {
        // 29 x 31 soit 899 combinaisons. Sur 64 tirages, en exiger 8 distincts
        // laisse une probabilite d'echec infime. Exiger en revanche 64 slugues
        // tous differents serait instable : avec 899 combinaisons, une
        // collision sur 64 tirages est un hasard courant.
        let mut vus: Vec<String> = Vec::new();
        for _ in 0..64 {
            let slug = creer();
            if !vus.contains(&slug) {
                vus.push(slug);
            }
        }
        assert!(vus.len() >= 8, "que {} slugues distincts", vus.len());
    }
}
