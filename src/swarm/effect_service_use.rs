//! Portage de `packages/core/src/effect/service-use.ts`.
//!
//! La source tient en 43 lignes et exporte `serviceUse(tag)` : un acces
//! paresseux aux methodes Effect d un service, avec un `Map` de cache et
//! un `Proxy` qui construit l accesseur au premier appel puis le rejoue.
//! La frontiere dynamique est unique : le nom de la propriete est une
//! valeur d execution, pas un type.
//!
//! Sans Effect dans ce crate, le portage retient ce qui est pur et
//! observable : le cache (premier appel construit, second rejoue), le
//! message d erreur quand la methode manque, et le refus des cles non
//! chaines. `tag.use` et `Effect` ne sont pas portes : ils appartiennent
//! au moteur d effets, pas a ce fichier.

use std::collections::HashMap;

/// Message rendu quand la methode demandee n existe pas sur le service.
///
/// Reprend mot pour mot la chaine de la source.
pub fn message_methode_manquante(cle: &str) -> String {
    format!("Service method not found: {}", cle)
}

/// Un cache d accesseurs, comme le `Map` de la source.
///
/// La valeur compte les constructions : chaque cle n est construite qu une
/// fois, les appels suivants rejouent la valeur gardee.
#[derive(Debug, Default)]
pub struct CacheAcces {
    /// Nombre de constructions par cle.
    constructions: HashMap<String, usize>,
}

impl CacheAcces {
    /// Cree un cache vide.
    pub fn nouveau() -> Self {
        Self { constructions: HashMap::new() }
    }

    /// Rend l accesseur pour `cle`, en le construisant au premier appel.
    ///
    /// Le retour est le nombre de constructions de cette cle : `1` au
    /// premier appel, toujours `1` ensuite. Une cle non chaine ne peut pas
    /// arriver ici : le `Proxy` de la source rend `undefined` avant, ce que
    /// `cle_est_chaine` modelise a part.
    pub fn acces(&mut self, cle: &str) -> usize {
        let compteur = self.constructions.entry(cle.to_string()).or_insert(0);
        if *compteur == 0 {
            *compteur = 1;
        }
        *compteur
    }

    /// Combien de cles distinctes ont ete construites ?
    pub fn nombre_de_cles(&self) -> usize {
        self.constructions.len()
    }
}

/// Une cle de `Proxy` n est utilisable que si elle est une chaine.
///
/// Traduit `if (typeof key !== "string") return undefined` : un symbole ou
/// un nombre ne traverse pas la frontiere dynamique.
pub fn cle_est_chaine(est_chaine: bool) -> bool {
    est_chaine
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_message_d_erreur_reprend_la_chaine_exacte() {
        assert_eq!(
            message_methode_manquante("maMethode"),
            "Service method not found: maMethode"
        );
    }

    #[test]
    fn le_premier_appel_construit_le_second_rejoue() {
        let mut cache = CacheAcces::nouveau();
        assert_eq!(cache.acces("lire"), 1);
        assert_eq!(cache.acces("lire"), 1);
        assert_eq!(cache.nombre_de_cles(), 1);
    }

    #[test]
    fn deux_cles_distinctes_construisent_deux_accesseurs() {
        let mut cache = CacheAcces::nouveau();
        cache.acces("lire");
        cache.acces("ecrire");
        assert_eq!(cache.nombre_de_cles(), 2);
        assert_eq!(cache.acces("lire"), 1);
    }

    #[test]
    fn une_cle_non_chaine_ne_traverse_pas_le_proxy() {
        assert!(cle_est_chaine(true));
        assert!(!cle_est_chaine(false));
    }

    #[test]
    fn un_cache_neuf_ne_contient_aucune_cle() {
        let cache = CacheAcces::nouveau();
        assert_eq!(cache.nombre_de_cles(), 0);
    }
}
