//! Portage Rust de `opencode/packages/core/src/util/binary.ts`.
//!
//! Le nom du fichier est trompeur : il n y a aucun fichier binaire ici, aucune
//! lecture disque, aucun octet. C est une recherche **binaire** sur un tableau
//! trie, exposee sous le nom d un espace de noms `Binary`.
//!
//! La source tient en quarante et une lignes et contient deux fonctions. Elle
//! ne contient aucun schema, aucune donnee serialisee, aucun ternaire `?` et
//! aucun `??`. Le piege des noms de champs s applique donc de facon triviale :
//! les deux seules cles exposees sont `found` et `index`, en minuscules
//! simples. Elles sont portees telles quelles, et un test verrouille leurs
//! noms exacts pour que personne ne les renomme plus tard par reflexe.
//!
//! ## Ce que la source garantit, et qu il faut conserver
//!
//! 1. **La liste doit etre triee.** Ni `search` ni `insert` ne verifient le
//!    tri. Sur une liste non triee, `search` peut declarer absent un element
//!    qui est present. C est le contrat de la source, on ne l ameliore pas :
//!    une amelioration silencieuse changerait le comportement de tous les
//!    appelants.
//!
//! 2. **Quand l element est absent, `index` est le point d insertion.** Les
//!    deux fonctions partagent la meme convention, ce qui permet d ecrire
//!    `list.splice(result.index, 0, item)` comme le fait `packages/app` :
//!    `search` donne l indice, `insert` l utilise.
//!
//! 3. **`index` peut valoir la longueur de la liste.** C est la valeur de fin
//!    du parcours quand la cle cherchee est superieure a toutes les cles
//!    presentes. En JavaScript `array[array.length]` vaut `undefined` sans
//!    erreur ; en Rust `liste[liste.len()]` **panique**. Il faut donc passer par
//!    `Vec::insert` ou `Vec::remove`, qui acceptent la position de fin, et
//!    jamais par un indexage direct apres un `found: false`.
//!
//! ## Deux choix de portage
//!
//! - Le comparateur est un `FnMut` comme dans `util_array.rs`. Le TS impose
//!   une fonction pure, donc `Fn` suffirait, mais `FnMut` accepte en plus les
//!   fermetures qui comptent leurs appels, ce qui permet de tester que la
//!   recherche reste bien logarithmique.
//!
//! - Le comparateur renvoie `S: AsRef<str>` et non `String`, ce qui accepte un
//!   comparateur qui produit une chaine posee comme un comparateur qui produit
//!   un emprunt, sans changer la signature. Reserve : un comparateur qui
//!   renvoie un emprunt de son argument impose au compilateur d unifier le type
//!   de sortie avec la duree de vie de l argument, ce qui n est pas verifie
//!   ici, aucune compilation n etant autorisee. Tous les comparateurs ecrits
//!   dans ce fichier renvoient donc un type possede.
//!
//! ## Divergence de sens a connaitre
//!
//! La source compare avec `<` sur des chaines JavaScript, donc par unites
//! UTF-16. Rust compare des chaines UTF-8, donc par octets. Les deux
//! concordent exactement sur de l ASCII, ce qui couvre tous les identifiants
//! du projet (hexadecimal et base62). Ils divergent des que la cle contient
//! un caractere hors ASCII, par exemple certains emoji, dont l ordre UTF-16
//! n est pas l ordre de points de code.

use serde::{Deserialize, Serialize};

/// Resultat d une recherche dichotomique.
///
/// Reproduit l objet litteral `{ found: boolean; index: number }` de la
/// source. Les deux noms de champs sont donnes explicitement, meme s ils ne
/// comportent aucune majuscule : c est la preuve formelle que l echange avec
/// le TypeScript n a pas derive, et le test
/// `un_nom_de_champ_different_est_refuse` verrouille le contrat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    /// `true` si la cle cherchee est presente dans la liste.
    pub found: bool,
    /// Position de la cle trouvee, sinon point d insertion du `search` et du
    /// `insert` de la source. Peut valoir la longueur de la liste.
    pub index: usize,
}

impl SearchResult {
    /// Resultat d une reussite : la cle est presente a cette position.
    pub fn found(index: usize) -> Self {
        SearchResult { found: true, index }
    }

    /// Resultat d un echec : la cle est absente, et `index` indique ou elle
    /// devrait etre inseree pour que la liste reste triee.
    pub fn not_found(index: usize) -> Self {
        SearchResult { found: false, index }
    }
}

/// Recherche dichotomique d une cle dans une liste triee.
///
/// Renvoie `SearchResult`. En cas de succes, `found` vaut `true` et `index`
/// est la position trouvee. En cas d echec, `found` vaut `false` et `index`
/// est le point d insertion, compris entre `0` et `array.len()` inclus.
///
/// Le comparateur est appele au plus `log2(longueur) + 1` fois, donc la
/// dichotomie ne peut pas degenerer en un parcours lineaire.
///
/// # Exemples
///
/// ```
/// use ycode::swarm::util_binary::{search, SearchResult};
///
/// let cles = vec!["a".to_string(), "c".to_string(), "e".to_string()];
///
/// let trouve = search(&cles, "c", |cle| cle.clone());
/// assert_eq!(trouve, SearchResult::found(1));
///
/// let absent = search(&cles, "d", |cle| cle.clone());
/// assert_eq!(absent, SearchResult::not_found(2));
/// ```
pub fn search<T, F, S>(array: &[T], id: &str, mut compare: F) -> SearchResult
where
    F: FnMut(&T) -> S,
    S: AsRef<str>,
{
    // La source demarre a `array.length - 1`, ce qui vaut `-1` sur une liste
    // vide en JavaScript. Avec un entier non signe, la soustraction deborderait
    // et paniquerait, donc les deux bornes sont des `isize`. Elles ne sont
    // negatives que par la borne droite, jamais la gauche.
    let mut left: isize = 0;
    let mut right: isize = array.len() as isize - 1;

    // D ans la boucle, `left >= 0` et `right >= left >= 0`, donc la somme
    // `left + right` n est jamais negative et la division entiere de Rust vers
    // zero coincide avec le `Math.floor` de la source. La meme remarque vaut
    // pour `insert`, ou la borne droite vaut `array.len()`.
    while left <= right {
        let mid = (left + right) / 2;
        // `mid` est dans `[left, right]`, donc dans `[0, len - 1]`.
        let mid_id = compare(&array[mid as usize]);
        // Le type est force en `&str` plutot que laisse a l inference : `S`
        // peut implanter plusieurs `AsRef`, et le choix doit etre visible.
        let mid_key: &str = mid_id.as_ref();

        if mid_key == id {
            return SearchResult::found(mid as usize);
        } else if mid_key < id {
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }

    // Echec : `left` est la premiere position dont la cle n est pas inferieure
    // a `id`, donc exactement le point d insertion du `insert`.
    SearchResult::not_found(left as usize)
}

/// Insere un element dans une liste triee, a sa place, en place.
///
/// La source renvoie le tableau qu elle vient de modifier ; en Rust le tableau
/// est deja emprunte en ecriture, donc rien n est renvoye. La convention de
/// point d insertion est identique a celle de [`search`].
///
/// Point de vigilance : l insertion se fait **avant** un element de cle
/// egale, alors que `Vec::insert_sorted` de la bibliotheque standard insere
/// apres. Sur une liste contenant des cles en double, l ordre des elements
/// change, meme si l ordre des cles ne change pas.
///
/// # Exemples
///
/// ```
/// use ycode::swarm::util_binary::insert;
///
/// let mut cles = vec!["a".to_string(), "c".to_string()];
/// insert(&mut cles, "b".to_string(), |cle| cle.clone());
/// assert_eq!(cles, vec!["a", "b", "c"]);
/// ```
pub fn insert<T, F, S>(array: &mut Vec<T>, item: T, mut compare: F)
where
    F: FnMut(&T) -> S,
    S: AsRef<str>,
{
    let id = compare(&item);
    let id_key: &str = id.as_ref();
    let mut left: isize = 0;
    // Ici la borne droite est `array.len()` et non `len() - 1`, et la boucle
    // teste `left < right` : c est ce qui garantit `mid < right`, donc
    // `mid` est toujours un indice valide meme quand la liste est vide.
    let mut right: isize = array.len() as isize;

    while left < right {
        let mid = (left + right) / 2;
        let mid_id = compare(&array[mid as usize]);
        let mid_key: &str = mid_id.as_ref();

        // Consequence : l insertion se place avant toute cle egale.
        if mid_key < id_key {
            left = mid + 1;
        } else {
            right = mid;
        }
    }

    array.insert(left as usize, item);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Element de test dont la cle est stockee a part, ce qui permet de
    /// distinguer deux elements de meme cle.
    #[derive(Debug, Clone, PartialEq)]
    struct Entree {
        cle: String,
        valeur: u32,
    }

    impl Entree {
        fn nouvelle(cle: &str, valeur: u32) -> Self {
            Entree { cle: cle.to_string(), valeur }
        }
    }

    /// Comparateur de test : renvoie une chaine posee. Tous les comparateurs de
    /// ce fichier renvoient un type possede, volontairement, pour ne dependre
    /// d aucune inference de region. Voir la note du module sur `AsRef<str>`.
    fn par_cle(entree: &Entree) -> String {
        entree.cle.clone()
    }

    /// La source compare des chaines, pas des nombres : `sortParts` et
    /// `messageKey` produisent des cles composees. Ce comparateur imite le
    /// second, `time.created + id`, pour prouver que la cle n est pas forcee
    /// a etre un identifiant.
    fn cle_par_date_puis_id(message: &Entree) -> String {
        format!("{:020}{}", message.valeur, message.cle)
    }

    #[test]
    fn une_liste_vide_donne_une_recherche_non_trouvee_a_zero() {
        // Le piege de portage : la source demarre a `length - 1`, ce qui vaut
        // moins un. Une soustraction sur entier non signe paniquerait ici.
        let liste: Vec<Entree> = Vec::new();
        let resultat = search(&liste, "a", par_cle);
        assert_eq!(resultat, SearchResult::not_found(0));
    }

    #[test]
    fn une_liste_vide_recoit_son_premier_element_a_la_position_zero() {
        let mut liste: Vec<Entree> = Vec::new();
        insert(&mut liste, Entree::nouvelle("a", 1), par_cle);
        assert_eq!(liste, vec![Entree::nouvelle("a", 1)]);
    }

    #[test]
    fn un_seul_element_present_est_trouve_a_zero() {
        let liste = vec![Entree::nouvelle("a", 1)];
        assert_eq!(search(&liste, "a", par_cle), SearchResult::found(0));
    }

    #[test]
    fn un_seul_element_absent_donne_une_position_selon_la_cle_cherchee() {
        let liste = vec![Entree::nouvelle("b", 1)];
        // Cle inferieure : rien avant elle.
        assert_eq!(search(&liste, "a", par_cle), SearchResult::not_found(0));
        // Cle superieure : rien apres, la position vaut la longueur de la liste.
        assert_eq!(search(&liste, "c", par_cle), SearchResult::not_found(1));
    }

    #[test]
    fn la_premiere_derniere_et_du_milieu_sont_trouvees() {
        let liste = vec![
            Entree::nouvelle("a", 1),
            Entree::nouvelle("c", 2),
            Entree::nouvelle("e", 3),
            Entree::nouvelle("g", 4),
        ];
        assert_eq!(search(&liste, "a", par_cle), SearchResult::found(0));
        assert_eq!(search(&liste, "e", par_cle), SearchResult::found(2));
        assert_eq!(search(&liste, "g", par_cle), SearchResult::found(3));
    }

    #[test]
    fn une_cle_absente_au_milieu_donne_le_point_d_insertion() {
        let liste = vec![
            Entree::nouvelle("a", 1),
            Entree::nouvelle("c", 2),
            Entree::nouvelle("e", 3),
            Entree::nouvelle("g", 4),
        ];
        // "d" se placerait entre "c" et "e".
        assert_eq!(search(&liste, "d", par_cle), SearchResult::not_found(2));
    }

    #[test]
    fn une_cle_avant_toute_la_liste_donne_zero_et_une_cle_apres_donne_la_longueur() {
        let liste = vec![Entree::nouvelle("a", 1), Entree::nouvelle("b", 2)];
        assert_eq!(search(&liste, "A", par_cle), SearchResult::not_found(0));
        // Position de fin : egale a la longueur. En JavaScript indexer ainsi
        // donnerait `undefined`, en Rust cela paniquerait, voir la note du
        // module.
        assert_eq!(search(&liste, "z", par_cle), SearchResult::not_found(2));
    }

    #[test]
    fn la_recherche_reste_logarithmique() {
        // Huit elements : au plus quatre comparaisons. Un parcours lineaire en
        // ferait huit, et un portage degrade serait passe inapercu.
        let liste: Vec<Entree> = ["a", "b", "c", "d", "e", "f", "g", "h"]
            .iter()
            .enumerate()
            .map(|(index, cle)| Entree::nouvelle(cle, index as u32))
            .collect();

        let mut appels = 0usize;
        let mut compteur = |entree: &Entree| {
            appels += 1;
            entree.cle.clone()
        };
        let resultat = search(&liste, "h", &mut compteur);

        assert_eq!(resultat, SearchResult::found(7));
        assert!(appels <= 4, "la recherche a depasse la borne de comparaisons");
    }

    #[test]
    fn l_insertion_reste_logarithmique() {
        let liste: Vec<Entree> = ["a", "b", "c", "d", "e", "f", "g", "h"]
            .iter()
            .enumerate()
            .map(|(index, cle)| Entree::nouvelle(cle, index as u32))
            .collect();

        let mut appels = 0usize;
        let mut compteur = |entree: &Entree| {
            appels += 1;
            entree.cle.clone()
        };
        let mut copie = liste.clone();
        insert(&mut copie, Entree::nouvelle("h", 99), &mut compteur);

        // Une comparaison de plus que `search`, celle de l element insere.
        // Un parcours lineaire en ferait neuf.
        assert!(
            appels <= 5,
            "l insertion a fait plus de comparaisons qu une dichotomie"
        );
        // "h" existe deja dans la liste, donc la regle d insertion avant cle
        // egale place le nouvel element devant l ancien.
        assert_eq!(copie[7].valeur, 99);
        assert_eq!(copie[8].valeur, 7);
    }

    #[test]
    fn une_liste_en_ordre_inverse_peut_declarer_absent_un_element_present() {
        // Contrat de la source, volontairement conserve : la fonction ne trie
        // pas. Sur une liste decroissante, la dichotomie parcourt toujours la
        // moitie gauche, qui contient les plus grandes cles, donc "a" qui se
        // trouve a la derniere position n est jamais atteint.
        let liste = vec![
            Entree::nouvelle("c", 3),
            Entree::nouvelle("b", 2),
            Entree::nouvelle("a", 1),
        ];
        assert_eq!(liste[2].cle, "a", "l element est bien present");
        assert_eq!(search(&liste, "a", par_cle), SearchResult::not_found(0));
    }

    #[test]
    fn l_insertion_dans_une_liste_non_triee_ne_repare_rien() {
        // Preuve complementaire du contrat : `insert` place l element sans
        // trier le reste, exactement comme la source.
        let mut liste = vec![Entree::nouvelle("c", 3), Entree::nouvelle("a", 1)];
        insert(&mut liste, Entree::nouvelle("b", 2), par_cle);
        assert_eq!(
            liste.iter().map(|e| e.cle.as_str()).collect::<Vec<_>>(),
            vec!["c", "a", "b"]
        );
    }

    #[test]
    fn l_insertion_avant_une_cle_egale_et_non_apres() {
        // C est la divergence la plus facile a rater : `Vec::insert_sorted` de
        // la bibliotheque standard insere apres une cle egale, la source
        // insere avant. L element inserve garde son identite, donc c est sa
        // position qui prouve le comportement.
        let mut liste = vec![Entree::nouvelle("a", 1), Entree::nouvelle("b", 2)];
        insert(&mut liste, Entree::nouvelle("b", 99), par_cle);
        assert_eq!(liste[1].valeur, 99, "le nouveau element est passe devant l ancien");
        assert_eq!(liste[2].valeur, 2);
    }

    #[test]
    fn l_insertion_au_debut_au_milieu_et_a_la_fin_garde_la_liste_triee() {
        let mut liste: Vec<Entree> = Vec::new();
        insert(&mut liste, Entree::nouvelle("m", 13), par_cle);
        insert(&mut liste, Entree::nouvelle("z", 26), par_cle);
        insert(&mut liste, Entree::nouvelle("a", 1), par_cle);
        insert(&mut liste, Entree::nouvelle("f", 6), par_cle);

        assert_eq!(
            liste.iter().map(|e| e.cle.as_str()).collect::<Vec<_>>(),
            vec!["a", "f", "m", "z"]
        );
    }

    #[test]
    fn le_point_d_insertion_de_search_et_celui_de_insert_sont_le_meme() {
        // Les deux fonctions du fichier partagent cette convention, et les
        // appelants de `packages/app` s en servent : ils indexent le resultat
        // de `search` pour inserer. Ce test verrouille l invariant.
        let base = vec![
            Entree::nouvelle("a", 1),
            Entree::nouvelle("d", 4),
            Entree::nouvelle("g", 7),
        ];

        for absent in ["A", "b", "e", "h"] {
            let attendu = search(&base, absent, par_cle);
            assert!(!attendu.found);

            let mut copie = base.clone();
            insert(&mut copie, Entree::nouvelle(absent, 0), par_cle);
            assert_eq!(copie[attendu.index].cle, absent);
        }
    }

    #[test]
    fn une_cle_vide_est_traitee_comme_une_cle_et_non_comme_une_absence() {
        // La source teste l egalite stricte et la comparaison, jamais la
        // veracite. Donc la chaine vide se recherche normalement, et une
        // reponse `false` ne peut pas provenir d une cle vide posee quelque part.
        let liste = vec![Entree::nouvelle("", 0), Entree::nouvelle("b", 2)];
        assert_eq!(search(&liste, "", par_cle), SearchResult::found(0));
        assert_eq!(search(&liste, "a", par_cle), SearchResult::not_found(1));

        let vide: Vec<Entree> = Vec::new();
        assert_eq!(search(&vide, "", par_cle), SearchResult::not_found(0));
    }

    #[test]
    fn la_cle_dune_liste_peut_etre_composee() {
        // `messageKey` de `packages/app` renvoie `time.created + id`. Les deux
        // elements ci-dessous ont des identifiants dans l ordre inverse de
        // leur date, donc le comparateur de la source les ordonne par date,
        // ce que `par_cle` ne ferait pas.
        let liste = vec![Entree::nouvelle("bruno", 1), Entree::nouvelle("alfa", 2)];

        // "bruno" est a la date 1 et "alfa" a la date 2, donc "bruno" vient en
        // premier meme si "bruno" est lexicographiquement plus grand que
        // "alfa". L element cherche est le second de la liste.
        assert_eq!(
            search(&liste, &cle_par_date_puis_id(&liste[1]), cle_par_date_puis_id),
            SearchResult::found(1)
        );

        // Une date posterieure se place a la fin, une date anterieure au debut.
        let mut triee = liste.clone();
        insert(&mut triee, Entree::nouvelle("zulu", 3), cle_par_date_puis_id);
        insert(&mut triee, Entree::nouvelle("omega", 0), cle_par_date_puis_id);
        assert_eq!(
            triee.iter().map(|e| e.cle.as_str()).collect::<Vec<_>>(),
            vec!["omega", "bruno", "alfa", "zulu"]
        );
    }

    #[test]
    fn les_noms_de_champs_serialises_sont_found_et_index() {
        // Le piege numero un du portage. Les deux cles sont en minuscules
        // simples dans la source ; ce test interdit de les renommer par
        // reflexe, ce qui casserait l echange avec le TypeScript sans aucune
        // erreur de compilation.
        let trouve = SearchResult::found(2);
        let json = serde_json::to_value(trouve).expect("la serialisation ne peut pas echouer");
        let objet = json.as_object().expect("le resultat est un objet JSON");

        let mut cles: Vec<&str> = objet.keys().map(|cle| cle.as_str()).collect();
        cles.sort_unstable();
        assert_eq!(cles, vec!["found", "index"]);

        assert_eq!(serde_json::to_string(&trouve).unwrap(), r#"{"found":true,"index":2}"#);

        let absent = SearchResult::not_found(0);
        assert_eq!(
            serde_json::to_string(&absent).unwrap(),
            r#"{"found":false,"index":0}"#
        );
    }

    #[test]
    fn le_json_aller_retour_se_deserialise() {
        let source = r#"{"found":true,"index":4}"#;
        let lu: SearchResult = serde_json::from_str(source).expect("le JSON produit par la source doit se relire");
        assert_eq!(lu, SearchResult::found(4));

        let reecrit = serde_json::to_string(&lu).unwrap();
        assert_eq!(reecrit, source);
    }

    #[test]
    fn un_nom_de_champ_different_est_refuse() {
        // Sans renommage explicite, une faute de frappe passerait la
        // compilation et ne serait vue qu a l echange. Ici la deserialization
        // echoue, donc le nom `found` est bien verifie.
        let errone = r#"{"isFound":true,"index":4}"#;
        let resultat = serde_json::from_str::<SearchResult>(errone);
        assert!(resultat.is_err(), "une cle inconnue a la place de found doit etre refusee");
    }
}
