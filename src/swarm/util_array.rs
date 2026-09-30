//! Portage Rust de `opencode/packages/core/src/util/array.ts`.
//!
//! La source tient en cinq lignes, mais la fonction qu elle contient est
//! utilisee partout ou une regle recente doit primer sur une regle plus
//! ancienne : permissions, politique, journal de session. Une erreur d ordre
//! ici ne se voit pas dans le code, elle se manifeste par un refus qui
//! s applique alors qu il ne devrait pas.
//!
//! Le contrat est celui de `Array.prototype.findLast` : on remonte la liste
//! depuis la fin et on renvoie le premier element trouve, c est a dire le
//! **dernier** element de la liste qui satisfait le predicat, et `None` si
//! aucun ne le fait.
//!
//! Deux choix meritent une justification :
//!
//! 1. `None` remplace `undefined`, sans enveloppe ni type de retour special.
//! 2. Le predicat est un `FnMut` et non un `Fn`. Le predicat du TS etait pur,
//!    donc tout `Fn` convient aussi, mais `FnMut` accepte en plus les
//!    fermetures qui capturent un compteur ou un etat modifie. On prend le
//!    choix le plus permissif : cela n exclut aucun appelant.
//!
//! Piege a ne pas reproduire : la fonction renvoie **l element**, jamais sa
//! veracite. En JavaScript `0`, `""` et `false` sont falsy ; une
//! implementation qui filtrerait sur la veracite au lieu de renvoyer la valeur
//! trouverait le bon indice mais perdrait la valeur. Le test dedie
//! `un_element_falsy_peut_etre_trouve` verrouille ce point.

/// Renvoie le dernier element de `items` qui satisfait `predicate`.
///
/// Le predicat est appele avec l element, son index, et la liste complete,
/// exactement comme le predicat de `findLast` en TypeScript.
///
/// # Exemples
///
/// ```
/// use ycode::swarm::util_array::find_last;
///
/// let items = vec!["deny", "allow", "ask"];
/// // Seul "ask" correspond, il est le dernier.
/// let trouve = find_last(&items, |item, _index, _liste| *item == "ask");
/// assert_eq!(trouve, Some(&"ask"));
/// ```
///
/// Renvoie `None` si aucun element ne correspond, y compris quand la liste est
/// vide.
pub fn find_last<'a, T, F>(items: &'a [T], mut predicate: F) -> Option<&'a T>
where
    F: FnMut(&T, usize, &'a [T]) -> bool,
{
    // Le parcours part de `items.len()` et non de `items.len() - 1` : sur une
    // liste vide la soustraction deborderait, puisque `usize` est non signe.
    // En JavaScript `0 - 1` vaut `-1` et la boucle ne s execute jamais.
    for index in (0..items.len()).rev() {
        if predicate(&items[index], index, items) {
            return Some(&items[index]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_liste_vide_ne_donne_rien() {
        let items: Vec<i32> = Vec::new();
        assert_eq!(find_last(&items, |item, _index, _liste| *item == 0), None);
    }

    #[test]
    fn un_seul_element_correspondant_est_trouve() {
        let items = vec![42];
        assert_eq!(find_last(&items, |item, _index, _liste| *item == 42), Some(&42));
    }

    #[test]
    fn un_seul_element_non_correspondant_ne_donne_rien() {
        let items = vec![42];
        assert_eq!(find_last(&items, |item, _index, _liste| *item == 7), None);
    }

    #[test]
    fn aucune_correspondance_donne_rien() {
        let items = vec![1, 2, 3, 4, 5];
        assert_eq!(find_last(&items, |item, _index, _liste| *item > 100), None);
    }

    #[test]
    fn la_derniere_occurrence_gagne() {
        // Trois elements correspondent, c est le dernier qui est renvoye.
        let items = vec![1, 2, 3, 4, 5];
        assert_eq!(find_last(&items, |item, _index, _liste| *item % 2 == 1), Some(&5));
    }

    #[test]
    fn l_ordre_inverse_donne_un_autre_resultat() {
        // Meme predicat, deux tris : seule la derniere position gagne, jamais
        // la premiere. C est la difference qui fait tout l interet de la
        // fonction.
        let items = vec![1, 2, 3, 4, 5];
        assert_eq!(find_last(&items, |item, _index, _liste| *item % 2 == 0), Some(&4));
    }

    #[test]
    fn la_derniere_occurrence_est_choisie_meme_si_des_elements_suivants_ne_correspondent_pas() {
        // Le parcours remonte : il s arrete sur 20 et n atteint jamais 10.
        let items = vec![10, 20, 99, 98];
        assert_eq!(find_last(&items, |item, _index, _liste| *item % 10 == 0), Some(&20));
    }

    #[test]
    fn un_element_falsy_peut_etre_trouve() {
        // En JavaScript `0`, `""` et `false` sont falsy mais parfaitement
        // valables comme resultat. La fonction renvoie la valeur, pas sa
        // veracite : `Some(&0)` et non `None`.
        let nombres = vec![0, 1];
        assert_eq!(find_last(&nombres, |item, _index, _liste| *item == 0), Some(&0));

        let chaines = vec!["a", ""];
        assert_eq!(find_last(&chaines, |item, _index, _liste| item.is_empty()), Some(&""));

        let booleens = vec![true, false];
        assert_eq!(find_last(&booleens, |item, _index, _liste| !*item), Some(&false));
    }

    #[test]
    fn le_predicat_parcourt_les_index_en_sens_decroissant() {
        // Prouve que l on remonte vraiment, et non qu on filtre puis qu on
        // prend le dernier du resultat : l ordre de visite est identique a
        // celui du TS.
        let items = vec!["a", "b", "c"];
        let mut vus: Vec<usize> = Vec::new();
        find_last(&items, |_item, index, _liste| {
            vus.push(index);
            false
        });
        assert_eq!(vus, vec![2, 1, 0]);
    }

    #[test]
    fn le_predicat_recoit_la_liste_entiere_a_chaque_appel() {
        // Le troisieme argument du predicat du TS est la liste complete. Il
        // reste accessible ici, ce qui permet d ecrire un predicat qui depend
        // du contexte et pas seulement de l element.
        let items = vec![1, 2, 3];
        let mut longueurs: Vec<usize> = Vec::new();
        find_last(&items, |_item, _index, liste| {
            longueurs.push(liste.len());
            false
        });
        assert_eq!(longueurs, vec![3, 3, 3]);
    }

    #[test]
    fn la_recherche_sarrete_des_le_resultat_trouve() {
        // Au-dela du premier succes en remontant, le predicat n est plus
        // appele : le court circuit du TS est conserve.
        let items = vec![1, 2, 3, 4];
        let mut appels = 0;
        let resultat = find_last(&items, |item, _index, _liste| {
            appels += 1;
            *item % 2 == 0
        });
        assert_eq!(resultat, Some(&4));
        assert_eq!(appels, 1, "les elements du dessus ne doivent pas etre testes");
    }

    #[test]
    fn le_resultat_designe_un_element_de_la_liste_d_origine() {
        // La fonction renvoie un emprunt sur la liste, pas une copie : les
        // appelants qui fournissent un `Vec` obtiennent la valeur originale.
        let items = vec!["a".to_string(), "b".to_string()];
        let trouve = find_last(&items, |item, _index, _liste| item == "b");
        assert_eq!(trouve, Some(&"b".to_string()));
        assert_eq!(items[1], "b", "la liste d origine reste inchangee");
    }
}
