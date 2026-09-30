//! Point d'entree `swarm` de `opencode/packages/core/src/session/runner/max-steps.ts`.
//!
//! La source fait seize lignes et ne contient **qu'un seul** declaration : le
//! prompt `MAX_STEPS_PROMPT`. Aucune fonction, aucun type, aucun comportement.
//!
//! ## Ce fichier est un reexport, et c'est un choix
//!
//! Ce texte **existe deja** dans ce depot, en un seul endroit, portage :
//! [`crate::core::session::max_steps::MAX_STEPS_PROMPT`]. Le recopier ici
//! creerait une deuxieme constante du meme nom, portant un texte que rien ne
//! relie a l'autre. C'est exactement la faute que ce lot de portage a deja
//! commise huit fois sur `SdkEvent` : en Rust, deux definitions du meme contrat
//! dans deux modules different ne produisent **aucune erreur**. Le crate
//! compile, les tests passent, et les deux copies divergent en silence des que
//! l'une d'elles est retouchee.
//!
//! Le reexport rend cette divergence impossible : les deux chemins designent la
//! meme allocation, et le test `ptr::eq` le verifie. Corriger le prompt quelque
//! part dans le code ne peut donc pas en laisser une version perimee de cote.
//!
//! Le prefixe de chemin ne correspond pas au fichier source, et c'est voulu :
//! le Swarm consomme ce prompt, il n'en est pas le proprietaire. La source
//! TypeScript vit sous `session/runner/`, le seul porteur du contrat est sous
//! `core/session/`, et ce fichier ne fait que le rendre atteignable depuis la
//! face `swarm`.
//!
//! ## Ce qui n'est pas traduit ici, et pourquoi
//!
//! Rien. Il n'y a rien d'autre a traduire. Le ternaire `?` et le coalescent
//! `??` n'apparaissent nulle part dans cette source, et il n'y a aucun nom de
//! champ a preserver : la source est un unique litteral de chaine, sans
//! objet, sans cle, sans interface. Le piege de la casse des majuscules n'a
//! donc aucune prise ici.

/// Prompt envoye au modele quand il a epuise son budget de tours.
///
/// Origine : `session/runner/max-steps.ts`, unique declaration du fichier.
/// Definition : [`crate::core::session::max_steps::MAX_STEPS_PROMPT`].
///
/// Un agent sans ce prompt continue d'appeler des outils apres epuisement de
/// ses tours : la session se termine sans conclusion, et l'utilisateur ignore
/// ou elle en est.
pub use crate::core::session::max_steps::MAX_STEPS_PROMPT;

#[cfg(test)]
mod tests {
    use super::MAX_STEPS_PROMPT;

    #[test]
    fn le_reexport_designe_la_meme_constante_et_pas_une_copie() {
        // Le test qui justifie tout ce fichier. Une copie locale passerait les
        // tests de contenu, mais pas celui-ci : c'est la seule garantie
        // contre la divergence silencieuse decrite dans la documentation de
        // module.
        assert!(std::ptr::eq(
            MAX_STEPS_PROMPT,
            crate::core::session::max_steps::MAX_STEPS_PROMPT,
        ));
    }

    #[test]
    fn le_prompt_est_une_chaine_statique_partagee() {
        // Le contrat est `&str` constant, pas `String`, et pas une fonction.
        // La fonction de test ne compile que si l'element reexporte est bien
        // une chaine de duree de vie statique.
        fn attend_une_chaine_statique(_: &'static str) {}
        attend_une_chaine_statique(MAX_STEPS_PROMPT);
    }

    #[test]
    fn les_deux_bornes_du_prompt_sont_ceux_de_la_source() {
        // La source commence par `CRITICAL - MAXIMUM STEPS REACHED` et se
        // termine par `Respond with text ONLY.`. Une reecriture qui retoucherait
        // une seule de ces deux bornes changerait le sens de l'instruction.
        assert!(MAX_STEPS_PROMPT.starts_with("CRITICAL - MAXIMUM STEPS REACHED"));
        assert!(MAX_STEPS_PROMPT.ends_with("Respond with text ONLY."));
    }

    #[test]
    fn le_prompt_exige_les_quatre_elements_du_bilan() {
        // Le bloc `Response must include:` de la source. Ce sont les quatre
        // livrables que le modele doit produire, et non pas la formulation
        // complete du prompt.
        for exigence in [
            "Statement that maximum steps for this agent have been reached",
            "Summary of what has been accomplished so far",
            "List of any remaining tasks that were not completed",
            "Recommendations for what should be done next",
        ] {
            assert!(
                MAX_STEPS_PROMPT.contains(exigence),
                "exigence absente du prompt : {}",
                exigence
            );
        }
    }
}
