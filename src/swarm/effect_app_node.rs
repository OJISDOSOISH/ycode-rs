//! Portage de `packages/core/src/effect/app-node.ts`.
//!
//! La source tient en 14 lignes, dont neuf lignes de code effectif, une ligne
//! de reexport mort, et le reste en blancs :
//!
//! ```ts
//! import { LayerNode } from "./layer-node"
//!
//! export const tags = LayerNode.tags({
//!   location: ["global"],
//!   global: [],
//! })
//!
//! export type GlobalNode<A, E = never> = LayerNode.Node<A, E, (typeof tags.values)["global"]>
//! export type LocationNode<A, E = never> = LayerNode.Node<A, E, (typeof tags.values)["location"]>
//!
//! export const makeGlobalNode = tags.make("global")
//! export const makeLocationNode = tags.make("location")
//!
//! export * as Node from "./app-node"
//! ```
//!
//! C est un fichier de **declaration de vocabulaire**, pas de logique : il ne
//! compile rien, ne cherche rien, ne compare rien. Il dit deux choses.
//!
//! 1. Il existe exactement deux balises de noeud, `location` et `global`, et
//!    la table `{ location: ["global"], global: [] }` dit lesquelles ont le
//!    droit de dependre de quelles.
//! 2. Il existe deux constructeurs de noeud, `makeGlobalNode` et
//!    `makeLocationNode`, qui sont `LayerNode.make` avec la balise deja
//!    imposee.
//!
//! # Ce que ce fichier porte
//!
//! - [`TAGS`] : la table de la source, objet litteral compris. Elle est une
//!   **valeur** cote TypeScript, pas une annotation, donc elle est traduite en
//!   donnee et pas en commentaire.
//! - [`allowed_dependencies`] : l acces executable a cette table, et
//!   [`tag_names`] : l equivalent de `Object.keys(config)`.
//! - [`NodeIdentity`] : l union `{ service } | { name }` de `MakeInput`, et le
//!   choix qui en decoule, `name: input.service !== undefined ? input.service.key
//!   : input.name`. C est le seul vrai calcul du fichier.
//! - [`NodeMaker`] : la fabrique `tags.make(name)`, et les deux constructeurs
//!   exportes, [`make_global_node`] et [`make_location_node`].
//!
//! # Le graphe `LayerNode` est deja porte : il est importe
//!
//! Deux versions anterieures de ce fichier ont ecrit dans leur en-tete que le
//! graphe `LayerNode` n existait pas en Rust, et ont laisse les deux
//! constructeurs reduits a des constantes. C etait une **fausse** excuse :
//! `crate::swarm::effect_app_node_builder` porte deja [`AppNode`], [`NodeTag`]
//! et [`LayerRef`], avec exactement les constructeurs dont ce fichier a
//! besoin, et `crate::swarm::location` les importe deja.
//!
//! Ce fichier les **importe** donc, et n en redeclare aucun. Aucun `Mutex`,
//! aucun `Arc`, aucun `Scope`, aucune closure de finaliseur : rien de tout cela
//! ne figure dans la source, et rien de tout cela n est fabrique ici. Le
//! fichier est du calcul et de la structure, point.
//!
//! Ce qu il apporte a [`crate::swarm::location`] en retour, c est la table des
//! balises vue du bon cote : `location.rs` redeclare `TAGS_LOCATION_DEPENDENCIES`
//! et `TAGS_GLOBAL_DEPENDENCIES` pour decrire la meme configuration, alors
//! que c est ici qu elle est deposee. Le doublon est signale dans le rapport,
//! il n est pas resolu ici : resoudre demanderait de modifier `location.rs`,
//! qui n est pas le fichier de cet agent.
//!
//! # Ce qui n a pas de traduction, et pourquoi
//!
//! 1. `layer: Implementation`, ou `Implementation extends Layer.Any`. Une
//!    `Layer` d Effect est une **valeur construite et branchee**, pas une
//!    donnee : elle n a pas d equivalent en Rust. Le champ existe donc dans
//!    [`MakeInput`], sous le nom de la source, mais il est type [`LayerRef`],
//!    c est a dire un nom fourni par l appelant. C est le meme choix que
//!    dans `location.rs` pour `layer(ref)`, et c est le seul.
//! 2. `CheckTags`, `CheckDependencies`, `CheckReplacementErrors`,
//!    `Output<Item>` et `Error<Item>` sont des **types**. Ils verifient des
//!    relations entre noeuds a la compilation TypeScript et ne sont **jamais**
//!    executes. Les reproduire en fonctions d execution serait inventer une
//!    verification que la source ne fait pas, et elle echouerait sur des
//!    programmes que TypeScript, lui, refuse a la compilation. Elles sont donc
//!    documentees et portees en tableaux de donnees ([`TAGS`]), pas en
//!    controle. Le test `un_noeud_global_peut_construire_une_dependance_de_tag_location`
//!    fixe cette divergence de facon explicite.
//! 3. `GlobalNode<A, E>` et `LocationNode<A, E>` sont des alias qui **plaquent
//!    la balise dans le type** du noeud. En Rust la balise voyage dans la
//!    **valeur** : [`AppNode`] porte deja `tag: Option<NodeTag>`, et un type
//!    fantome n ajouterait aucune capacite reelle au moment de l execution. Il
//!    n en est donc pas introduit, et le controle equivalent est la
//!    constructeur : [`NodeMaker`] ne peut pas produire de noeud dont la balise
//!    ne soit pas la sienne.
//! 4. `export * as Node from "./app-node"` est un reexport de l espace de noms
//!    du module sur lui-meme, donc du code mort : il n a pas d equivalent et n
//!    est pas repris.
//!
//! # Trois details de lecture
//!
//! - **L ordre des balises vient de l objet.** `LayerNode.tags` fait
//!   `Object.keys(config)`, donc `values` est construit dans l ordre
//!   d insertion de l objet litteral, `location` puis `global`. [`tag_names`]
//!   rend cet ordre explicite.
//! - **Un nom hors table ne serait pas refuse.** `tags.make` ecrit
//!   `tag: values[name]` sans verifier que `name` est une cle de la
//!   configuration : appele avec un nom inconnu, le noeud porterait
//!   `tag: undefined`. Le type `TagNames<Config>` l interdit a la compilation.
//!   En Rust, [`maker`] prend un [`NodeTag`], donc il n existe aucun nom hors
//!   table : le cas est **irrepresentable**, ce qui est plus fort que la
//!   source et non equivalent a elle.
//! - **`service` est teste contre `undefined`, pas contre une chaine vide.**
//!   Une `Context.Service` dont la cle vaut `""` est donc utilisee, et le nom
//!   du noeud vaut `""`. Aucun test de veracite n existe dans ce chemin, donc
//!   aucune chaine vide ne disparait. C est la meme famille de piege que
//!   `workspaceID` dans `location.rs`, et deux tests la verrouillent.
//!
//! # Aucun nom de champ JSON
//!
//! Rien n est serialise ni deserialise sur ce fichier : la table des balises
//! est un objet interne, et les noeuds ne franchissent aucune frontiere JSON.
//! Aucun `#[serde(rename)]` n est donc necessaire, et le piege des noms en
//! majuscules (`projectID`, `sessionID`, `workspaceID`) **ne se pose pas** ici.
//!
//! # Note d integration
//!
//! Ce fichier attend `pub mod effect_app_node;` dans `src/swarm/mod.rs`, qui
//! n est **pas** en place : la regle du lot interdit de modifier `mod.rs`.
//! Tant que cette ligne n est pas ajoutee, le fichier n est pas compile et ses
//! tests ne tournent pas. C est une action pour l agent principal, signalee
//! dans le rapport.

use crate::swarm::effect_app_node_builder::{AppNode, LayerRef, NodeTag};

/// Une entree de la configuration des balises.
///
/// Reproduit une cle de l objet passe a `LayerNode.tags` :
/// `{ location: ["global"], global: [] }`. Le champ `allowed` est la liste des
/// balises dont un noeud portant `tag` a le droit de dependre.
///
/// La liste est une **contrainte de type** cote TypeScript (`CheckTags`), donc
/// elle n est jamais verifiee a l execution. Elle est portee comme donnee,
/// parce que dans la source c est une donnee, et un test la solidarise avec
/// [`allowed_dependencies`] pour que les deux ne divergent pas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagConfig {
    /// La balise decrite.
    pub tag: NodeTag,
    /// Les balises autorisees parmi les dependances, dans l ordre de la
    /// source. La liste est vide pour `global`, et c est voulu.
    pub allowed: &'static [NodeTag],
}

/// La configuration des balises, exactement celle de la source.
///
/// L ordre des elements est l ordre des cles de l objet litteral, donc
/// `location` puis `global`, comme `Object.keys(config)` le restituerait.
pub const TAGS: [TagConfig; 2] = [
    TagConfig { tag: NodeTag::Location, allowed: &[NodeTag::Global] },
    TagConfig { tag: NodeTag::Global, allowed: &[] },
];

/// Les balises declarees, dans l ordre de l objet de la source.
///
/// Equivalent de `Object.keys(config)`, puisque c est ainsi que `LayerNode.tags`
/// construit `values`.
pub fn tag_names() -> Vec<&'static str> {
    TAGS.iter().map(|entree| entree.tag.as_str()).collect()
}

/// Les balises dont un noeud portant `tag` a le droit de dependre.
///
/// C est la lecture executable de [`TAGS`]. Comme la table, elle decrit une
/// regle que la source applique a la compilation et jamais a l execution.
pub fn allowed_dependencies(tag: NodeTag) -> &'static [NodeTag] {
    match tag {
        NodeTag::Location => &[NodeTag::Global],
        NodeTag::Global => &[],
    }
}

/// L identite d un noeud en construction.
///
/// Reprise de l union `NodeIdentity` de `layer-node.ts` :
///
/// ```ts
/// type NodeIdentity =
///   | { readonly service: Context.Service.Any; readonly name?: never }
///   | { readonly name: string; readonly service?: never }
/// ```
///
/// Une `Context.Service.Any` est une cle, donc une chaine : le service est
/// ici designe par cette cle, exactement comme `AppNode` l explique dans
/// `effect_app_node_builder` (le champ `service` y est un alias du nom).
pub enum NodeIdentity {
    /// Le noeud est construit depuis un service, et son nom est la cle de ce
    /// service : c est la branche de `LayerNode.make` qui ecrit
    /// `input.service.key`.
    Service(String),
    /// Le noeud est construit depuis un nom libre : c est la branche
    /// `input.name`.
    Name(String),
}

impl NodeIdentity {
    /// Le nom que `LayerNode.make` donnerait au noeud.
    ///
    /// Traduction de `input.service !== undefined ? input.service.key :
    /// input.name`. C est le seul calcul de la source qui ne soit pas une
    /// declaration.
    ///
    /// Le test porte sur la **presence** d un service, pas sur la veracite de
    /// sa cle : une cle vide est un service comme un autre, et donne un nom
    /// vide.
    pub fn resolve(&self) -> &str {
        match self {
            NodeIdentity::Service(cle) => cle,
            NodeIdentity::Name(nom) => nom,
        }
    }
}

/// L entree d un constructeur de noeud.
///
/// Reprise de `MakeInput`, **balise retiree** : la source ecrit
/// `DistributiveOmit<MakeInput<...>, "tag">`, donc la balise ne se choisit pas,
/// c est le constructeur qui l impose. C est la raison d etre de [`NodeMaker`].
///
/// Le champ `dependencies` est un `Vec`, alors que la source impose
/// `NodeList`, c est a dire `[] | [Item, ...Item[]]` : une liste vide, ou une
/// liste d au moins un element. Un `Vec` ne peut pas avoir de trou, donc il ne
/// peut pas violer cette contrainte : il n y a rien a porter de ce cote.
pub struct MakeInput {
    /// Service ou nom libre dont le noeud tire son nom.
    pub identity: NodeIdentity,
    /// L implementation fournie par le noeud.
    ///
    /// La source y met une `Layer` d Effect. Ici c est un [`LayerRef`], donc
    /// un nom, fourni par l appelant : c est le seul point du portage ou une
    /// valeur de la source n a pas d equivalent.
    pub implementation: LayerRef,
    /// Les noeuds dont depend le noeud construit, dans l ordre de la source.
    pub dependencies: Vec<AppNode>,
}

impl MakeInput {
    /// Entree dont le noeud tire son nom d un service, sans dependance.
    ///
    /// Le nom est la cle du service, et le nom de la source est alors interdit
    /// par le type.
    pub fn service(cle: impl Into<String>, implementation: LayerRef) -> Self {
        Self {
            identity: NodeIdentity::Service(cle.into()),
            implementation,
            dependencies: Vec::new(),
        }
    }

    /// Entree dont le noeud porte un nom libre, sans dependance.
    pub fn named(nom: impl Into<String>, implementation: LayerRef) -> Self {
        Self {
            identity: NodeIdentity::Name(nom.into()),
            implementation,
            dependencies: Vec::new(),
        }
    }

    /// Ajoute les dependances du noeud, dans l ordre donne.
    ///
    /// La source impose `deps: Items & CheckDependencies<Implementation,
    /// Items>` : la liste des services rendus par la couche doit etre couverte
    /// par les dependances. Comme la couche n est qu un nom ici, cette
    /// verification n a pas d objet, et elle n est pas simulee.
    pub fn with_dependencies(mut self, dependencies: Vec<AppNode>) -> Self {
        self.dependencies = dependencies;
        self
    }
}

/// La fabrique `tags.make(name)`, c est a dire un constructeur dont la balise
/// est fixee.
///
/// La source renvoie une fonction :
///
/// ```ts
/// make: ((name: TagNames<Config>) => (input) => make({ ...input, tag: values[name] }))
/// ```
///
/// Le seul calcul est `tag: values[name]`, donc porter la fonction revient a
/// porter un constructeur qui possede une balise. Elle est `Copy` : la
/// reutiliser ne consomme rien, contrairement a une closure qui aurait
/// consomme la balise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeMaker {
    tag: NodeTag,
}

impl NodeMaker {
    /// La balise que ce constructeur impose.
    pub fn tag(&self) -> NodeTag {
        self.tag
    }

    /// Construit le noeud, c est a dire `LayerNode.make({ ...input, tag })`.
    ///
    /// Le genre est toujours [`crate::swarm::effect_app_node_builder::NodeKind::Layer`] :
    /// la source passe par `make`, qui produit toujours un noeud de couche.
    /// Une dependance n existe donc pas dans cette entree, et le genre ne peut
    /// pas changer.
    pub fn make(&self, input: MakeInput) -> AppNode {
        AppNode::layer_with_dependencies(
            input.identity.resolve().to_string(),
            self.tag,
            input.implementation,
            input.dependencies,
        )
    }
}

/// La fabrique de balise `tag`.
///
/// Equivalant de `tags.make(name)`. Le nom est un [`NodeTag`] et non une
/// chaine : la table de la source ne contient que ces deux noms, donc un nom
/// hors table n est pas representable ici, alors qu il donnerait en
/// TypeScript un noeud sans balise.
pub fn maker(tag: NodeTag) -> NodeMaker {
    NodeMaker { tag }
}

/// Le constructeur de noeud global.
///
/// Export de la source : `export const makeGlobalNode = tags.make("global")`.
pub fn make_global_node(input: MakeInput) -> AppNode {
    maker(NodeTag::Global).make(input)
}

/// Le constructeur de noeud de localisation.
///
/// Export de la source : `export const makeLocationNode = tags.make("location")`.
pub fn make_location_node(input: MakeInput) -> AppNode {
    maker(NodeTag::Location).make(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::effect_app_node_builder::NodeKind;

    /// Nom d une implementation de test. La source met une `Layer` a cet
    /// endroit ; ici c est une etiquette.
    fn couche(nom: &str) -> LayerRef {
        LayerRef::new(nom)
    }

    /// Une dependance non liee, balisee comme on veut.
    fn dependance(nom: &str, tag: NodeTag) -> AppNode {
        AppNode::unbound(nom, tag)
    }

    /// L entree de reference : un service, aucune dependance.
    fn entree() -> MakeInput {
        MakeInput::service("@opencode/example/Service", couche("impl"))
    }

    #[test]
    fn la_table_de_tags_reproduit_celle_de_la_source() {
        // `{ location: ["global"], global: [] }`, element par element.
        assert_eq!(TAGS.len(), 2);
        assert_eq!(TAGS[0].tag, NodeTag::Location);
        assert_eq!(TAGS[0].allowed, [NodeTag::Global]);
        assert_eq!(TAGS[1].tag, NodeTag::Global);
        assert!(TAGS[1].allowed.is_empty());
    }

    #[test]
    fn les_noms_viennent_dans_l_ordre_declaration_de_l_objet() {
        // `Object.keys(config)` conserve l'ordre d insertion du litteral, donc
        // `location` avant `global`. L'ordre n'a aucune signification dans la
        // source, mais c'est ce qu'un `Object.keys` rendrait.
        assert_eq!(tag_names(), vec!["location", "global"]);
    }

    #[test]
    fn la_fonction_et_la_table_disent_la_meme_chose() {
        // `allowed_dependencies` et `TAGS` decrivent la meme configuration par
        // deux chemins. Ce test est ce qui empeche les deux de diverger.
        for entree in TAGS {
            assert_eq!(allowed_dependencies(entree.tag), entree.allowed);
        }
        // Et dans les deux sens : aucune balise declaree n'est oubliee.
        for tag in [NodeTag::Location, NodeTag::Global] {
            assert!(TAGS.iter().any(|entree| entree.tag == tag));
        }
    }

    #[test]
    fn un_noeud_global_ne_peut_dependre_d_aucun_tag() {
        // `global: []` : liste vide, donc aucun tag autorise. Une liste d un
        // element serait fausse, et c est le test qui le dit.
        let autorises = allowed_dependencies(NodeTag::Global);
        assert!(autorises.is_empty());
        assert!(!autorises.contains(&NodeTag::Location));
        assert!(!autorises.contains(&NodeTag::Global));
    }

    #[test]
    fn un_noeud_de_localisation_peut_dependre_du_global_seulement() {
        // `location: ["global"]` : exactement une balise autorisee.
        let autorises = allowed_dependencies(NodeTag::Location);
        assert_eq!(autorises.len(), 1);
        assert!(autorises.contains(&NodeTag::Global));
        assert!(!autorises.contains(&NodeTag::Location));
    }

    #[test]
    fn un_noeud_global_peut_construire_une_dependance_de_tag_location() {
        // Divergence assumee et fixee. TypeScript refuse ce programme la, via
        // `CheckTags`, donc la construction est impossible cote source. Elle est
        // possible ici parce que la verification de type n est pas traduite en
        // controle d execution. Le test existe pour que personne ne lise cette
        // construction comme validee : elle ne l est pas.
        let interdit = dependance("@opencode/example/Interdit", NodeTag::Location);
        let noeud = make_global_node(entree().with_dependencies(vec![interdit.clone()]));

        assert_eq!(noeud.tag(), Some(NodeTag::Global));
        assert_eq!(noeud.dependencies(), [interdit]);
        // La regle que la source applique, elle, reste ce que dit la table.
        assert!(!allowed_dependencies(NodeTag::Global).contains(&NodeTag::Location));
    }

    #[test]
    fn la_dependance_autorisee_par_la_table_est_construite_sans_reserve() {
        // Le contrepoint du test precedent, et le cas reel : `location` depend
        // de `global`. C est exactement ce que fait `boundNode` de
        // `location.rs` avec `Project.node`.
        let projet = dependance("@opencode/ProjectV2", NodeTag::Global);
        let noeud = make_location_node(entree().with_dependencies(vec![projet.clone()]));

        assert_eq!(noeud.tag(), Some(NodeTag::Location));
        assert_eq!(noeud.dependencies(), [projet]);
        assert!(allowed_dependencies(NodeTag::Location).contains(&NodeTag::Global));
    }

    #[test]
    fn le_nom_vient_de_la_cle_du_service() {
        // `name: input.service !== undefined ? input.service.key : input.name`.
        let noeud = make_global_node(MakeInput::service("@opencode/ProjectV2", couche("impl")));
        assert_eq!(noeud.name(), "@opencode/ProjectV2");
    }

    #[test]
    fn le_nom_libre_est_utilise_quand_l_entree_n_en_a_pas_de_service() {
        // La seconde branche du ternaire. Elle est atteinte des que
        // `input.service` vaut `undefined`, donc des que l identite est un nom.
        let noeud = make_global_node(MakeInput::named("Service", couche("impl")));
        assert_eq!(noeud.name(), "Service");
    }

    #[test]
    fn une_cle_vide_donne_un_nom_vide() {
        // Le ternaire teste `undefined`, pas la veracite : une cle `""` est un
        // service, et le nom du noeud vaut `""`. Un `.filter()` par veracite
        // serait une invention, exactement comme sur `workspaceID` dans
        // `location.rs`.
        let par_service = make_global_node(MakeInput::service("", couche("impl")));
        let par_nom = make_global_node(MakeInput::named("", couche("impl")));
        assert_eq!(par_service.name(), "");
        assert_eq!(par_nom.name(), "");
        assert_eq!(par_service, par_nom);
    }

    #[test]
    fn le_noeud_construit_est_un_noeud_de_couche_balise() {
        // `LayerNode.make` produit toujours `kind: "layer"`.
        for noeud in [make_global_node(entree()), make_location_node(entree())] {
            assert_eq!(noeud.kind(), NodeKind::Layer);
            assert!(noeud.implementation().is_some());
        }
    }

    #[test]
    fn les_deux_constructeurs_ne_different_que_par_la_balise() {
        // Meme entree, deux constructeurs : c est la seule difference que
        // `tags.make(name)` introduit, et c est exactement ce que
        // `DistributiveOmit<..., "tag">` dit de la source.
        let global = make_global_node(entree());
        let location = make_location_node(entree());

        assert_eq!(global.tag(), Some(NodeTag::Global));
        assert_eq!(location.tag(), Some(NodeTag::Location));
        // Tout le reste est identique, nom compris.
        assert_eq!(global.name(), location.name());
        assert_eq!(global.implementation(), location.implementation());
        assert_eq!(global.dependencies(), location.dependencies());
    }

    #[test]
    fn la_balise_vient_du_constructeur_et_non_de_l_entree() {
        // L'entree n'a aucun champ de balise : il n'y a pas ou la mettre. Les
        // deux constructeurs le prouvent en produisant deux balises differentes
        // a partir de la meme entree.
        // Meme entree reconstruite, car les constructeurs la consomment.
        assert_eq!(make_global_node(entree()).tag(), Some(NodeTag::Global));
        assert_eq!(make_location_node(entree()).tag(), Some(NodeTag::Location));
    }

    #[test]
    fn les_dependances_sont_conservees_dans_leur_ordre() {
        let a = dependance("A", NodeTag::Global);
        let b = dependance("B", NodeTag::Global);
        let c = dependance("C", NodeTag::Global);
        let noeud = make_global_node(entree().with_dependencies(vec![a.clone(), b.clone(), c.clone()]));

        assert_eq!(noeud.dependencies(), [a, b, c]);
    }

    #[test]
    fn une_entree_sans_dependance_donne_un_noeud_sans_dependance() {
        // `deps: []` est une `NodeList` valide, et le noeud reste de genre
        // `layer` : ce n'est pas un groupe.
        let noeud = make_global_node(entree());
        assert!(noeud.dependencies().is_empty());
        assert_eq!(noeud.kind(), NodeKind::Layer);
        assert_ne!(noeud.kind(), NodeKind::Group);
    }

    #[test]
    fn l_implementation_du_noeud_est_le_nom_fourni_par_l_appelant() {
        // La source y met une `Layer` d Effect, qui n'a pas d equivalent : le
        // nom vient donc de l'appelant, et il traverse tel quel.
        let noeud = make_global_node(MakeInput::service("@opencode/S", couche("S.couche")));
        assert_eq!(noeud.implementation().map(LayerRef::name), Some("S.couche"));
    }

    #[test]
    fn la_fabrique_ne_peut_produire_que_les_balises_de_la_table() {
        // `tag: values[name]` sans controle : en TypeScript, un nom hors table
        // donnerait `tag: undefined`. Le parametre etant un `NodeTag`, ce cas
        // est irrepresentable ici, et la table est donc une borne de l API.
        assert_eq!(maker(NodeTag::Global).tag(), NodeTag::Global);
        assert_eq!(maker(NodeTag::Location).tag(), NodeTag::Location);
        for tag in [NodeTag::Location, NodeTag::Global] {
            assert!(TAGS.iter().any(|entree| entree.tag == tag));
        }
    }

    #[test]
    fn les_balises_de_la_table_sont_celles_du_porteur_de_types() {
        // `NodeTag` est porte par `effect_app_node_builder` et importe ici. Ce
        // test solidarise mecaniquement les deux representations : une
        // nouvelle variante de l'enum, ou un nom change, casserait ici.
        for entree in TAGS {
            assert_eq!(NodeTag::from_name(entree.tag.as_str()), Some(entree.tag));
        }
        assert_eq!(NodeTag::from_name("Location"), None);
        assert_eq!(NodeTag::from_name(""), None);
    }

    #[test]
    fn la_fabrique_est_reutilisable_sans_effet_de_bord() {
        // Une closure JavaScript consomme sa balise au premier appel d'une
        // certaine facon ; une `NodeMaker` est `Copy`, donc elle rend toujours
        // le meme noeud.
        let fabrique = maker(NodeTag::Global);
        let premiere = fabrique.make(entree());
        let seconde = fabrique.make(entree());
        let troisieme = fabrique.make(entree());
        assert_eq!(premiere, seconde);
        assert_eq!(seconde, troisieme);
        // Le constructeur de la source, lui, n'est qu'une application partielle
        // de `make`, donc le meme resultat.
        assert_eq!(premiere, make_global_node(entree()));
    }

    #[test]
    fn les_constructeurs_produisent_le_meme_noeud_que_le_porteur_de_types() {
        // Ce fichier n'invente pas de forme de noeud : il appelle
        // `AppNode::layer_with_dependencies`, celui du porteur, avec les memes
        // arguments. Si les deux evoluaient, ce test le verrait.
        let attendu = AppNode::layer_with_dependencies(
            "@opencode/example/Service",
            NodeTag::Global,
            couche("impl"),
            vec![dependance("@opencode/ProjectV2", NodeTag::Global)],
        );
        let obtenu = make_global_node(
            entree().with_dependencies(vec![dependance("@opencode/ProjectV2", NodeTag::Global)]),
        );

        assert_eq!(obtenu, attendu);
    }
}
