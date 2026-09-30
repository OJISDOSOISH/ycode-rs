//! Portage de `packages/core/src/location-service-map.ts`.
//!
//! La source tient en dix-huit lignes et n a que trois exports :
//!
//! ```ts
//! export class Service extends Context.Service<
//!   Service,
//!   LayerMap.LayerMap<Location.Ref, LocationServices, LocationError>
//! >()("@opencode/example/LocationServiceMap") {
//!   static get(ref: Location.Ref) {
//!     return Layer.unwrap(Effect.map(Service, (locations) => locations.get(ref)))
//!   }
//! }
//!
//! export const node = LayerNode.unbound(Service, Node.tags.values.global)
//!
//! export * as LocationServiceMap from "./location-service-map"
//! ```
//!
//! Ce que dit cette source, en clair :
//!
//! - `Service` est une **etiquette de contexte**. Sa forme est une `LayerMap`
//!   parametre par trois arguments, et leur ordre compte : la **cle**, la
//!   **valeur**, puis l'**erreur**. Ici la cle est un `Location.Ref` (un
//!   repertoire de travail), la valeur est le paquet de services demarres pour ce
//!   repertoire, et l erreur est la reunion des erreurs de ce paquet.
//! - `Service.get(ref)` est une **fournisseuse paresseuse** : elle lit la carte
//!   dans l environnement, demande l entree correspondant a `ref`, et rend la
//!   `Layer` qui sera construite plus tard, a la premiere utilisation reelle.
//! - `node` est un **noeud non lie** (`unbound`) de `LayerNode`, marque avec le
//!   tag `global` de `effect/app-node.ts`. Il ne depend d aucun autre noeud.
//!
//! ## Ce qui n a pas de traduction, et qui n est pas simule
//!
//! Aucun de ces quatre morceaux n existe en Rust standard. Ils ne sont pas
//! reimplementes ici, seulement decrits :
//!
//! 1. **La `LayerMap` de la bibliotheque `effect`.** Elle memorise une `Layer`
//!    par cle, la construit a la premiere demande, la garde en vie tant que
//!    quelqu un l utilise, puis la libere apres un delai d inactivite. La
//!    source ne fixe pas ce delai : il est passe au moment de la construction,
//!    dans `location-services.ts:109` (`{ idleTimeToLive: "60 minutes" }`).
//!    Le paquet `effect` n est pas vendorise dans ce depot, donc son interface
//!    exacte n a pas pu etre relue ; seule la methode **reellement appelee par
//!    la source** est modelisee, a savoir `get`.
//! 2. **La construction paresseuse des couches.** `Layer.unwrap` transforme un
//!    `Effect<Layer<...>>` en `Layer<...>` sans rien construire. Ici `get_for`
//!    renvoie directement le resultat, donc l echec est rendu immediatement par
//!    `Result` au lieu d etre deferre a la construction. C est la seule
//!    difference de comportement introduite ici.
//! 3. **Le graphe `LayerNode`.** `unbound` ne veut pas dire *sans type*, il
//!    veut dire *connu du systeme de couches, mais sans implementation a cet
//!    endroit*. L effet de cet etat est visible deux lignes plus loin, dans
//!    `LayerNode.compile` (`effect/layer-node.ts:260`), qui leve
//!    `Unbound layer node: <nom>` si on compile un graphe qui en contient un
//!    encore. Cette verification, ainsi que `hasUnbound` (`:321`), vit dans
//!    `effect/layer-node.ts` et n est pas portee ici.
//! 4. **Le `export * as` de la derniere ligne.** Il ne se reexporte pas :
//!    `location-service-map.ts` n existe qu en un seul exemplaire dans
//!    `packages/core/src`, et un module Rust n a pas besoin de se declarer
//!    lui-meme.
//!
//! `LocationServices` et `LocationError` ne sont **pas** definis non plus. Ce
//! sont, dans `location-services.ts:81-82`, la sortie et l union des erreurs
//! des **trente-six** noeuds listes dans `LayerNode.group` (`:42-79`). Les
//! retranscrire ici reviendrait a dupliquer le travail d une trentaine d autres
//! agents. Ils sont donc des **types associes** du trait : l assembleur du
//! graphe, qui possede deja ces noeuds, choisit les types.
//!
//! Concretement, la seule chose qui produit reellement la carte est
//! `buildLocationServiceMap` (`location-services.ts:84-112`), qui appelle
//! `LayerMap.make` puis `LayerNode.hoist` puis `LayerNode.compile`. Ce fichier
//! la consomme, il ne la produit pas.
//!
//! ## Ce qui a change par rapport a la premiere version du fichier
//!
//! Trois corrections, dans l ordre d importance :
//!
//! - **`LocationRef` n est plus redeclare ici.** Il est **importe** de
//!   `crate::core::session::schema`, qui le definit deja avec le bon nom de
//!   champ. La version precedente en declarait une copie, avec ses propres
//!   constructeurs. Deux definitions d un meme contrat ne produisent aucune
//!   erreur de compilation : elles divergent en silence. C est exactement le
//!   piege que la relecture a deja attrape deux fois ailleurs (`sessionID` /
//!   `callID`).
//! - **`invalidate` a ete retire du trait.** La source portee n appelle que
//!   `get`. `invalidate` existe bien dans l interface `LayerMap` de la
//!   bibliotheque `effect`, mais elle appartient a l **implementation** de la
//!   carte, qui est construite dans `location-services.ts`, donc dans le fichier
//!   d un autre agent. Le garder obligait a concevoir une mutation derriere un
//!   `&self`, et donc a inventer une mecanique de concurrrence pour y
//!   repondre. Le trait ne contient plus que ce que la source utilise.
//! - **La carte de test est un `Vec`, pas une `BTreeMap`.** `LocationRef`
//!   importe ne derive que `Debug, Clone, PartialEq, Serialize, Deserialize` :
//!   ni `Eq`, ni `Ord`, ni `Hash`. Il ne peut donc pas servir de cle de table
//!   triee. C est une contrainte du type importe, pas un choix de gout, et elle
//!   est signalee plutot que masquee par une copie du struct.
//!
//! ## Sur les deux pieges classiques
//!
//! - **Noms de champs.** Le seul nom serde de ce fichier est porte par
//!   `LocationRef.workspace_id`, qui se serialise en `workspaceID`, avec D et I en
//!   majuscules. C est le type importe qui le porte, et deux tests le verifient
//!   quand meme, parce que ce sont les tests qui attraperont une regression si
//!   quelqu un redefinit le struct ailleurs.
//! - **`?` contre `??`.** Il ny en a **aucun** dans cette source : ni
//!   ternaire, ni coalescent. `optional(WorkspaceID)` vient du schema
//!   `packages/schema/src/location.ts:11`, ou c est un `Schema.optionalKey` :
//!   la cle peut etre absente du JSON. Cela correspond a `skip_serializing_if`,
//!   et une chaine vide **survit** a la serialisation, comme sous `??`.

/// La cle du service telle qu elle apparait dans la source, et telle qu elle
/// doit apparaitre dans les traces et les diagnostics. A verifier caractere par
/// caractere contre `"@opencode/example/LocationServiceMap"` :
/// c est ce `example` qui est suspect, il ne fait pas partie du nom du paquet
/// alors qu il est dans le repertoire `core`.
pub const SERVICE_KEY: &str = "@opencode/example/LocationServiceMap";

/// Le tag porte par `node`, qui vient de `Node.tags.values.global`, lui-meme
/// declare dans `effect/app-node.ts:3-6`.
pub const NODE_TAG: &str = "global";

/// `Location.Ref`, c est-a-dire `packages/schema/src/location.ts:9-12`.
///
/// Ce type n est **pas** declare ici. Il est importe de
/// `crate::core::session::schema`, qui porte deja
///
/// ```ts
/// export const Ref = Schema.Struct({
///   directory: AbsolutePath,
///   workspaceID: optional(WorkspaceID),
/// })
/// ```
///
/// Les deux sources declarent exactement la meme forme, donc il n y a rien a
/// retranscrire. `location.rs` avait pose le premier jalon en important le meme
/// type ; le suivre est la seule facon d eviter que deux `LocationRef` coexistent.
pub use crate::core::session::schema::LocationRef;

/// Reexport de [`LocationRef`] sous le nom de la source, `Location.Ref`.
///
/// Meme choix de forme que dans `location.rs:163` : la source parle de
/// `Location.Ref`, le contrat Rust porte `LocationRef`, et le reexport evite
/// d ecrire `LocationRef` partout ou l on devrait ecrire `Ref`.
pub use crate::core::session::schema::LocationRef as Ref;

/// La forme du service `Service` : la `LayerMap` elle-meme, c est-a-dire la
/// table qui associe un `LocationRef` au paquet de services de ce repertoire.
///
/// La source utilise `Context.Service`, dont la conversion demandee est un
/// `trait`. Les deux types de la table ne sont pas fixes : ils dependent des
/// trente-six noeuds de `location-services.ts`, d ou les types associes.
///
/// En TypeScript, `get` renvoie une `Layer` : la demande est enregistree, et le
/// paquet n est construit qu a la premiere utilisation reelle. En Rust il n y a
/// pas de construction paresseuse, donc `get` renvoie le paquet tout de suite.
/// C est la seule difference de comportement introduite ici.
///
/// Le trait ne prend **aucun** parametre generique. Les deux types de la table
/// sont les types associes `Services` et `Error`, choisis par l implementeur.
pub trait LocationServiceMapService {
    /// Le paquet de services d un repertoire, c est-a-dire
    /// `LocationServices = LayerNode.Output<typeof locationServices>`.
    /// Fourni par l assembleur du graphe, non defini dans ce fichier.
    type Services;

    /// Le canal d erreur, c est-a-dire
    /// `LocationError = LayerNode.Error<typeof locationServices>`.
    /// Fourni par l assembleur du graphe, non defini dans ce fichier.
    type Error;

    /// L instance correspondant a `key`, ou l erreur de construction.
    fn get(&self, key: &LocationRef) -> Result<Self::Services, Self::Error>;
}

/// Portage du `static get(ref)` de la classe `Service`.
///
/// La source ecrit :
///
/// ```ts
/// return Layer.unwrap(Effect.map(Service, (locations) => locations.get(ref)))
/// ```
///
/// Les trois morceaux se lisent ainsi : `Effect.map(Service, ...)` lit la
/// carte dans l environnement (utiliser `Service` comme valeur signifie *le
/// service courant*), l appelle sur `ref`, et `Layer.unwrap` transforme le
/// resultat en `Layer` sans rien construire.
///
/// En Rust il n y a ni environnement ni `Layer`. La lecture de la carte
/// devient donc un **argument** : la carte est passee en parametre au lieu
/// d etre lue dans un contexte ambiant. C est le meme graphe de dependances,
/// explicite plutot qu implicite.
///
/// La fonction ne fait donc presque rien. Elle existe quand meme, parce que le
/// nom `get` de la source est le nom que le reste du code appelle, et qu un
/// point de couture unique evite d avoir trois endroits ou le nom change.
pub fn get_for<S: LocationServiceMapService>(
    map: &S,
    key: &LocationRef,
) -> Result<S::Services, S::Error> {
    map.get(key)
}

/// Le genre de noeud de `LayerNode`, dans son sous-ensemble utilise ici.
/// Les trois valeurs possibles sont declarees dans `effect/layer-node.ts:23`,
/// mais seules celles qui servent ce fichier sont portees : `unbound`, parce
/// que cest ce que produit `LayerNode.unbound`. Les deux autres
/// (`layer`, `group`) appartiennent aux autres agents du swarm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// Un noeud connu du systeme de couches, sans implementation a cet endroit.
    Unbound,
}

/// Le noeud non lie exporte par la source.
///
/// `LayerNode.unbound` renvoie un objet de cette forme
/// (`effect/layer-node.ts:98-106`) :
///
/// ```ts
/// return { kind: "unbound", name: service.key, service, dependencies: [], tag }
/// ```
///
/// Le champ `service` est omis ici : en TypeScript il porte la cle de
/// contexte, qui est deja rendue par [`SERVICE_KEY`].
///
/// Ce noeud n est pas serialisable, et ne doit pas l etre : dans la source il
/// ne quitte jamais la memoire, il ne fait que participer au calcul du
/// graphe de `LayerNode`, dont rien n est echange avec le TypeScript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// Toujours [`NodeKind::Unbound`] pour ce fichier.
    pub kind: NodeKind,
    /// La cle du service, reprise de `service.key`.
    pub name: &'static str,
    /// Toujours vide : `unbound` ne declare aucune dependance.
    pub dependencies: Vec<Node>,
    /// Le tag du noeud, repris du second argument de `unbound`.
    pub tag: &'static str,
}

/// Portage de `export const node = LayerNode.unbound(Service, Node.tags.values.global)`.
///
/// La source utilise une constante. Ici `node` est une fonction parce que
/// [`Node`] contient un `Vec`, qui ne peut pas etre construit dans une
/// `const` stable. Le contenu est identique et est fige de toute facon : il ne
/// depend d aucun parametre d execution.
pub fn node() -> Node {
    Node {
        kind: NodeKind::Unbound,
        name: SERVICE_KEY,
        dependencies: Vec::new(),
        tag: NODE_TAG,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construit une cle de table.
    ///
    /// `LocationRef` est **importe**, donc ses champs sont publics, mais il
    /// n a pas de constructeur : il n en a jamais eu dans
    /// `core::session::schema`, et on ne lui en ajoute pas ici. Ajouter une
    /// methode associee a un type partage, depuis un fichier de swarm qui
    /// n est pas son proprietaire, exposerait chaque agent a definir la meme
    /// methode, et ferait echouer la compilation du premier d entre eux. Un
    /// constructeur local de test evite ce piege.
    fn ref_de(repertoire: &str, espace: Option<&str>) -> LocationRef {
        LocationRef {
            directory: repertoire.to_string(),
            workspace_id: espace.map(str::to_string),
        }
    }

    /// Une carte de test, qui imite le strict minimum de la `LayerMap`.
    ///
    /// Le stockage est un `Vec` et non une table, pour une raison imposee :
    /// `LocationRef` est importe, et le type importe ne derive ni `Ord` ni
    /// `Hash`. La recherche est donc lineaire. C est un defaut assume du test,
    /// pas une pretention a reproduire le cache de la `LayerMap`.
    struct FausseCarte {
        entrees: Vec<(LocationRef, String)>,
    }

    impl FausseCarte {
        /// Une carte sans aucune entree, c est-a-dire une `LayerMap` jamais
        /// interrogee.
        fn vide() -> Self {
            Self {
                entrees: Vec::new(),
            }
        }

        /// Une carte qui contient exactement une entree.
        fn avec_une_entree(cle: LocationRef, valeur: &str) -> Self {
            Self {
                entrees: vec![(cle, valeur.to_string())],
            }
        }

        /// Une carte qui contient plusieurs entrees, dans l ordre fourni.
        fn avec_entrees(paires: &[(&LocationRef, &str)]) -> Self {
            Self {
                entrees: paires
                    .iter()
                    .map(|(cle, valeur)| ((*cle).clone(), (*valeur).to_string()))
                    .collect(),
            }
        }
    }

    impl LocationServiceMapService for FausseCarte {
        type Services = String;
        type Error = String;

        fn get(&self, key: &LocationRef) -> Result<String, String> {
            self.entrees
                .iter()
                .find(|(cle, _)| cle == key)
                .map(|(_, valeur)| valeur.clone())
                .ok_or_else(|| format!("aucune entree pour {}", key.directory))
        }
    }

    #[test]
    fn la_cle_du_service_est_cellule_de_la_source() {
        assert_eq!(SERVICE_KEY, "@opencode/example/LocationServiceMap");
        assert_eq!(NODE_TAG, "global");
    }

    #[test]
    fn le_noeud_est_non_lie_sans_dependance_et_marque_global() {
        let n = node();

        assert_eq!(n.kind, NodeKind::Unbound);
        assert_eq!(n.name, "@opencode/example/LocationServiceMap");
        assert_eq!(n.tag, "global");
        assert!(n.dependencies.is_empty());
    }

    #[test]
    fn deux_appels_a_node_donnent_le_meme_noeud() {
        assert_eq!(node(), node());
    }

    /// Compteur de compilation : oblige le compilateur a accepter la meme valeur
    /// la fois comme `LocationRef` de `core::session::schema` et comme le
    /// reexport `Ref` de ce module. Le jour ou quelqu un redeclare un struct
    /// `LocationRef` ici, cette fonction cesse de compiler.
    fn accepte_les_deux_noms(_: &crate::core::session::schema::LocationRef, _: &Ref) {}

    #[test]
    fn la_cle_de_la_table_est_le_meme_type_que_celle_de_la_session() {
        let cle = ref_de("/proj/app", Some("wrk_01"));

        accepte_les_deux_noms(&cle, &cle);

        assert_eq!(cle, ref_de("/proj/app", Some("wrk_01")));
    }

    #[test]
    fn une_ref_avec_espace_de_travail_serialise_la_cle_en_majuscules() {
        let cle = ref_de("/proj/app", Some("wrk_01"));
        let json = serde_json::to_string(&cle).expect("serialisation");

        assert_eq!(json, r#"{"directory":"/proj/app","workspaceID":"wrk_01"}"#);
        assert!(
            json.contains("workspaceID"),
            "le D et le I doivent etre majuscules"
        );
        assert!(
            !json.contains("workspaceId"),
            "la forme camelCase lowercase est fausse"
        );
        assert!(
            !json.contains("workspace_id"),
            "le nom du champ Rust ne doit pas fuir"
        );
    }

    #[test]
    fn une_ref_sans_espace_de_travail_omet_la_cle() {
        let cle = ref_de("/proj/app", None);
        let json = serde_json::to_string(&cle).expect("serialisation");

        assert_eq!(json, r#"{"directory":"/proj/app"}"#);
        assert!(!json.contains("workspaceID"));
    }

    #[test]
    fn un_espace_de_travail_vide_survit_a_la_serialization() {
        // La source ne contient aucun ternaire et aucun coalescent : rien n y
        // teste la veracite. `optional` est un `optionalKey`, qui teste la
        // nullite, exactement comme `??` : la chaine vide est une valeur
        // presente, elle disparait donc pas.
        let cle = ref_de("/proj/app", Some(""));
        let json = serde_json::to_string(&cle).expect("serialisation");

        assert_eq!(json, r#"{"directory":"/proj/app","workspaceID":""}"#);

        let relu: LocationRef = serde_json::from_str(&json).expect("deserialisation");
        assert_eq!(relu.workspace_id, Some(String::new()));
    }

    #[test]
    fn une_cle_ecrite_en_mauvaise_casse_est_ignoreee_a_la_deserialisation() {
        // Contre-test du piege des noms de champs : un producteur qui ecrit
        // `workspaceId` au lieu de `workspaceID` ne declenche aucune erreur,
        // son champ est simplement perdu. C est exactement ce qui casse a
        // l echange, et c est invisible de l interieur du code Rust.
        let json = r#"{"directory":"/proj/app","workspaceId":"wrk_01"}"#;
        let relu: LocationRef = serde_json::from_str(json).expect("deserialisation");

        assert_eq!(relu.directory, "/proj/app");
        assert_eq!(relu.workspace_id, None);
    }

    #[test]
    fn la_cle_majuscule_est_acceptee_a_la_deserialisation() {
        let json = r#"{"directory":"/proj/app","workspaceID":"wrk_01"}"#;
        let relu: LocationRef = serde_json::from_str(json).expect("deserialisation");

        assert_eq!(relu, ref_de("/proj/app", Some("wrk_01")));
    }

    #[test]
    fn deux_refs_de_meme_repertoire_avec_des_espaces_differents_sont_distinctes() {
        let a = ref_de("/proj/app", Some("wrk_01"));
        let b = ref_de("/proj/app", Some("wrk_02"));
        let c = ref_de("/proj/app", None);

        let carte = FausseCarte::avec_entrees(&[
            (&c, "sans espace"),
            (&a, "premier"),
            (&b, "second"),
        ]);

        assert_eq!(carte.get(&a), Ok("premier".to_string()));
        assert_eq!(carte.get(&b), Ok("second".to_string()));
        assert_eq!(carte.get(&c), Ok("sans espace".to_string()));
    }

    #[test]
    fn une_cle_absente_reste_absente_apres_un_allers_retour_par_le_json() {
        let avec = ref_de("/proj/app", Some("wrk_01"));
        let sans = ref_de("/proj/app", None);

        let json = serde_json::to_string(&sans).expect("serialisation");
        let relu: LocationRef = serde_json::from_str(&json).expect("deserialisation");

        assert_ne!(avec, relu);
        assert_eq!(sans, relu);
    }

    #[test]
    fn get_for_renvoie_le_paquet_de_la_cle_demandee() {
        let ref_a = ref_de("/proj/a", Some("wrk_01"));
        let carte = FausseCarte::avec_une_entree(ref_a.clone(), "paquet-a");

        let obtenu = get_for(&carte, &ref_a);

        assert_eq!(obtenu, Ok("paquet-a".to_string()));
    }

    #[test]
    fn get_for_signale_une_cle_absente_au_lieu_de_l_inventer() {
        let carte = FausseCarte::avec_une_entree(ref_de("/proj/a", None), "paquet-a");

        let obtenu = get_for(&carte, &ref_de("/proj/absent", None));

        match obtenu {
            Ok(v) => panic!("aucune entree ne devait etre rendue, on a eu {}", v),
            Err(e) => assert!(e.contains("/proj/absent")),
        }
    }

    #[test]
    fn une_carte_vide_renvoie_toujours_une_erreur() {
        let carte = FausseCarte::vide();

        for repertoire in ["/proj/a", "/proj/b", "/"] {
            let obtenu = get_for(&carte, &ref_de(repertoire, None));
            assert!(obtenu.is_err(), "une carte vide ne peut rien rendre");
        }
    }

    #[test]
    fn une_carte_a_une_seule_entree_ne_rend_que_celle_la() {
        let seule = ref_de("/proj/seul", None);
        let carte = FausseCarte::avec_une_entree(seule.clone(), "le-seul");

        assert_eq!(get_for(&carte, &seule), Ok("le-seul".to_string()));
        assert!(get_for(&carte, &ref_de("/proj/autre", None)).is_err());
    }

    #[test]
    fn l_ordre_d_insertion_ne_change_pas_ce_que_rend_une_recherche() {
        // Meme table, deux ordres d insertion opposes. Une `LayerMap` ne se
        // comporte pas comme une liste : elle rend la meme valeur dans les deux
        // cas, ou elle echoue. La carte de test est un `Vec`, donc c est elle
        // qui risque de confondre l ordre avec l identite.
        let a = ref_de("/proj/a", Some("wrk_01"));
        let b = ref_de("/proj/b", Some("wrk_01"));

        let dans_lordre = FausseCarte::avec_entrees(&[(&a, "paquet-a"), (&b, "paquet-b")]);
        let dans_lordre_inverse = FausseCarte::avec_entrees(&[(&b, "paquet-b"), (&a, "paquet-a")]);

        assert_eq!(get_for(&dans_lordre, &a), get_for(&dans_lordre_inverse, &a));
        assert_eq!(get_for(&dans_lordre, &b), get_for(&dans_lordre_inverse, &b));
    }

    #[test]
    fn get_for_renvoie_la_meme_fois_qu_on_appelle_la_carte() {
        let cle = ref_de("/proj/a", None);
        let carte = FausseCarte::avec_une_entree(cle.clone(), "paquet-a");

        assert_eq!(get_for(&carte, &cle), get_for(&carte, &cle));
        assert_eq!(
            carte.entrees.len(),
            1,
            "une lecture ne doit rien ajouter a la carte"
        );
    }
}