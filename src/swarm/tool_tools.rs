//! Portage de `packages/core/src/tool/tools.ts`.
//!
//! La source tient en treize lignes et ne contient ni implementation, ni etat,
//! ni catalogue reel : seulement une interface et une declaration de service.
//! Ce fichier ne fait donc que nommer le contrat, fidelement a sa source.
//!
//! Ce qui est traduit :
//! - `export interface Interface` devient le `trait` [`Service`] ;
//! - `export class Service extends Context.Service<Service, Interface>()(...)
//!   ("@opencode/v2/Tools")` devient le `trait` lui-meme, plus la constante
//!   [`SERVICE_TAG`] qui porte l'identifiant du service ;
//! - `Readonly<Record<string, Tool.AnyTool>>` devient un
//!   `&BTreeMap<String, AnyTool>` : deterministe, comme le veut la table de
//!   traduction du projet pour un `Record` en lecture seule ;
//! - le canal d'erreur `Tool.RegistrationError` devient [`RegistrationError`].
//!
//! ## Ce qui n'est pas traduit, et pourquoi
//!
//! - `Scope.Scope`, troisieme parametre de `Effect.Effect<void, ...>`, est une
//!   **exigence** de duree de vie : l'effet ne s'execute qu'entre l'ouverture
//!   et la fermeture d'une portee, et les enregistrements sont liberes a la
//!   fermeture. En Rust cette duree de vie appartient a celui qui detient le
//!   `&mut self` : l'appelant ouvre et ferme, il n'y a rien d'autre a
//!   representer.
//!
//! - `Tool.AnyTool` et `Tool.RegistrationError` sont declares dans
//!   `tool/tool.ts`, qui n'est pas porte dans ce lot. Ils sont donc reproduits
//!   ici, sous une forme minimale, pour que la signature du `trait` reste
//!   exacte. `AnyTool` est en TypeScript un objet fige et vide dont les types
//!   d'entree et de sortie n'existent qu'au niveau du systeme ; un struct vide
//!   en tient lieu. Le fichier `src/tool.rs` du projet est une autre conception,
//!   sans `AnyTool` ni `RegistrationError`, et n'a pas ete modifie.
//!
//! - `export * as Tools from "./tools"` (ligne 1) est un reexport de l'espace de
//!   noms du module sur lui-meme. En Rust, le module joue deja ce role : aucune
//!   ecriture supplementaire n'est necessaire.

use std::collections::BTreeMap;

/// Identifiant du service dans le graphe Effect d'origine.
///
/// La source ecrit la chaine au moment de la declaration du `Context.Service` :
/// `Context.Service<Service, Interface>()("@opencode/v2/Tools")`. Elle fait
/// partie de l'identite du service, donc elle est conservee telle quelle.
pub const SERVICE_TAG: &str = "@opencode/v2/Tools";

/// Tag de l'erreur d'enregistrement, declare dans `tool/tool.ts`.
///
/// On ne reproduit pas ici le `_tag` produit par `Schema.TaggedErrorClass` :
/// l'erreur ne franchit aucune frontiere JSON dans le code d'origine, elle vit
/// dans le canal d'erreur d'un `Effect`. La constante documente le tag sans
/// pretendre qu'il est serialise.
pub const REGISTRATION_ERROR_TAG: &str = "Tool.RegistrationError";

/// Une definition d'outil, vue comme une valeur opaque.
///
/// Cote TypeScript, `AnyTool` vaut `Definition<any, any>` : un objet fige et
/// vide, dont le type d'entree et le type de sortie n'existent que comme
/// parametres fantomes. Les deux sont effaces en Rust, puisque le type n'a
/// aucune valeur a l'execution : un struct vide represente exactement cela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct AnyTool;

/// Echec de l'enregistrement d'un outil.
///
/// Reprend les deux champs de `Tool.RegistrationError` : le `name` de l'outil
/// refuse et le `message` qui explique le refus. Les champs sont inchanges,
/// car ils ne portent aucune majuscule interne.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("enregistrement d'outil refuse : {name} ({message})")]
pub struct RegistrationError {
    /// Nom de loutil refuse.
    pub name: String,
    /// Explication du refus.
    pub message: String,
}

impl RegistrationError {
    /// Construit une erreur a partir du nom et du message.
    pub fn new(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self { name: name.into(), message: message.into() }
    }

    /// Tag de cette erreur dans le code d'origine.
    pub fn tag(&self) -> &'static str {
        REGISTRATION_ERROR_TAG
    }
}

/// Service d'enregistrement des outils.
///
/// La source le documente ainsi : "Narrow registration-only Location
/// capability". Le service n'expose donc qu'une seule operation, `register`,
/// et cette operation enregistre ; elle n'execute rien.
///
/// En TypeScript, l'effet rendu par `register` est vide, peut echouer avec
/// `Tool.RegistrationError` et exige une `Scope`. Ici il renvoie `()`, peut
/// echouer avec [`RegistrationError`], et la portee est celle du `&mut self`.
pub trait Service {
    /// Enregistre les outils nommes dans le catalogue donne.
    ///
    /// Le catalogue est en lecture seule et ses cles sont uniques : c'est le
    /// `Readonly<Record<string, Tool.AnyTool>>` de la source. Le `BTreeMap`
    /// impose un ordre de parcours alphabetique, la ou un objet JavaScript
    /// parcourrait ses cles dans l'ordre d'insertion.
    fn register(&mut self, tools: &BTreeMap<String, AnyTool>) -> Result<(), RegistrationError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    /// Catalogue de test : note les noms dans l'ordre d'arrivee, et peut etre
    /// configure pour refuser tous les enregistrements.
    struct Registre {
        noms: Rc<RefCell<Vec<String>>>,
        refus: bool,
    }

    impl Service for Registre {
        fn register(&mut self, tools: &BTreeMap<String, AnyTool>) -> Result<(), RegistrationError> {
            if self.refus {
                let nom = tools.keys().next().cloned().unwrap_or_default();
                return Err(RegistrationError::new(nom, "enregistrement refuse par le test"));
            }
            self.noms.borrow_mut().extend(tools.keys().cloned());
            Ok(())
        }
    }

    /// Construit un catalogue a partir d'une liste de noms, comme le ferait
    /// l'appelant TypeScript qui assemble son `Record` avant d'appeler
    /// `register`.
    fn catalogue(noms: &[&str]) -> BTreeMap<String, AnyTool> {
        noms.iter().map(|nom| (nom.to_string(), AnyTool)).collect()
    }

    /// Renvoie un catalogue de test, et un manche qui permet de lire ce qu'il a
    /// enregistre, meme apres un boxing en `dyn Service`.
    fn registre(refus: bool) -> (Registre, Rc<RefCell<Vec<String>>>) {
        let noms = Rc::new(RefCell::new(Vec::new()));
        (Registre { noms: Rc::clone(&noms), refus }, noms)
    }

    #[test]
    fn l_identifiant_du_service_est_exactement_celui_de_la_source() {
        assert_eq!(SERVICE_TAG, "@opencode/v2/Tools");
    }

    #[test]
    fn un_catalogue_vide_est_accepte_et_n_enregistre_aucun_outil() {
        let (mut service, noms) = registre(false);
        assert!(service.register(&catalogue(&[])).is_ok());
        assert!(noms.borrow().is_empty());
    }

    #[test]
    fn un_seul_outil_est_enregistre_sous_le_nom_donne() {
        let (mut service, noms) = registre(false);
        assert!(service.register(&catalogue(&["read"])).is_ok());
        assert_eq!(*noms.borrow(), vec!["read".to_string()]);
    }

    #[test]
    fn les_outils_sont_parcourus_par_ordre_alphabetique_et_non_d_insertion() {
        // Le catalogue est construit dans le desordre, comme un objet JavaScript
        // dont l'ordre des cles est celui de l'ecriture. Le BTreeMap impose
        // l'ordre alphabetique, donc l'enregistrement ne depend pas de
        // l'ordre de construction.
        let mut tools: BTreeMap<String, AnyTool> = BTreeMap::new();
        tools.insert("write".to_string(), AnyTool);
        tools.insert("read".to_string(), AnyTool);
        tools.insert("edit".to_string(), AnyTool);

        let (mut service, noms) = registre(false);
        assert!(service.register(&tools).is_ok());
        let attendus = vec![
            "edit".to_string(),
            "read".to_string(),
            "write".to_string(),
        ];
        assert_eq!(*noms.borrow(), attendus);
    }

    #[test]
    fn un_nom_vide_est_conserve_comme_une_cle_ordinaire() {
        // Le nom d'outil n'est filtre nulle part dans ce module : la chaine
        // vide est une cle comme une autre. Le nom est valide par ailleurs,
        // dans `Tool.validateName`, qui n'appartient pas a ce fichier.
        let (mut service, noms) = registre(false);
        assert!(service.register(&catalogue(&[""])).is_ok());
        assert_eq!(*noms.borrow(), vec![String::new()]);
    }

    #[test]
    fn deux_appels_successifs_cumulent_leurs_outils_sans_perte() {
        let (mut service, noms) = registre(false);
        assert!(service.register(&catalogue(&["read"])).is_ok());
        assert!(service.register(&catalogue(&["write", "edit"])).is_ok());
        let attendus = vec!["read".to_string(), "edit".to_string(), "write".to_string()];
        assert_eq!(*noms.borrow(), attendus);
    }

    #[test]
    fn un_catalogue_refuse_remonte_une_erreur_et_n_enregistre_rien() {
        let (mut service, noms) = registre(true);
        let resultat = service.register(&catalogue(&["read", "write"]));
        let erreur = match resultat {
            Ok(()) => panic!("l'enregistrement aurait du echouer"),
            Err(erreur) => erreur,
        };
        assert_eq!(erreur.name, "read");
        assert_eq!(erreur.message, "enregistrement refuse par le test");
        assert!(noms.borrow().is_empty());
    }

    #[test]
    fn l_erreur_d_enregistrement_conserve_son_nom_son_message_et_son_tag() {
        let erreur = RegistrationError::new("outil invalide", "Invalid tool name");
        assert_eq!(erreur.name, "outil invalide");
        assert_eq!(erreur.message, "Invalid tool name");
        assert_eq!(erreur.tag(), REGISTRATION_ERROR_TAG);
        assert_eq!(erreur.tag(), "Tool.RegistrationError");
    }

    #[test]
    fn le_service_est_utilisable_a_travers_un_pointeur_de_trait() {
        // Le service est partage comme un service Effect : on ne manipule que
        // le contrat, jamais l'implementation concrete.
        let (service, noms) = registre(false);
        let mut service: Box<dyn Service> = Box::new(service);
        assert!(service.register(&catalogue(&[])).is_ok());
        assert!(service.register(&catalogue(&["glob"])).is_ok());
        assert_eq!(*noms.borrow(), vec!["glob".to_string()]);
    }
}
