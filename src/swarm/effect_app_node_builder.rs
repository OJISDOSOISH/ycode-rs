//! Portage de `packages/core/src/effect/app-node-builder.ts`.
//!
//! La source tient en 23 lignes, dont 6 lignes de code effectif :
//!
//! ```ts
//! export function build<A, E>(root, replacements = []) {
//!   let allReplacements = replacements
//!   if (LayerNode.hasUnbound(root, LocationServiceMap.node) && !hasReplacement(replacements, LocationServiceMap.node)) {
//!     const locationMap = buildLocationServiceMap(replacements)
//!     const locationMapNode = makeGlobalNode({ service: LocationServiceMap.Service, layer: locationMap, deps: [] })
//!     allReplacements = replacements.concat([[LocationServiceMap.node, locationMapNode]])
//!   }
//!   return LayerNode.compile(root, allReplacements)
//! }
//! ```
//!
//! # Ce que fait ce fichier
//!
//! Un seul decideur, mais un decideur dont la condition est la moitie du
//! fichier. Avant de compiler le graphe de couches, il regarde si la racine a
//! besoin de la carte des services par emplacement (`LocationServiceMap`). Si
//! oui, et si personne ne fournit deja ce service, il construit la carte,
//! l emballe dans un noeud global et l ajoute en **dernier** element de la liste
//! des remplacements. Sinon il ne construit rien du tout et transmet la liste
//! telle quelle.
//!
//! # Le graphe de couches est porte, dans ce fichier
//!
//! L en-tete d une version anterieure de ce fichier affirmait que le graphe
//! `LayerNode` n existait pas en Rust, alors que le fichier portait deja
//! `AppNode`, `NodeTag` et `LayerRef` : l en-tete etait faux, et il estait
//! devenu un excuse pour ne pas porter le reste. Les types dont la source a
//! besoin sont donc portes ici, parce que c est ce fichier qui en est
//! l utilisateur :
//!
//! - [`NodeKind`] : le champ `kind` du noeud, `"layer" | "unbound" | "group"` ;
//! - [`NodeTag`] : les balises de `app-node.ts`, `global` et `location` ;
//! - [`AppNode`] : le noeud, avec son nom, sa balise, ses dependances et son
//!   implementation ;
//! - [`LayerRef`] : le nom d une couche d Effect, seule trace de la valeur
//!   `Layer` qui n a pas d equivalent en Rust (voir plus bas) ;
//! - [`Replacement`] : le couple `[source, remplacement]`.
//!
//! `crate::swarm::location` s appuie deja sur ces types pour `Location.node` et
//! `Location.boundNode`. Le doublon de cle de service entre
//! `crate::swarm::location_service_map::SERVICE_KEY` et
//! [`LOCATION_SERVICE_MAP_SERVICE_KEY`] est signale dans le rapport : les deux
//! chaines sont identiques, et c est a l agent principal de decider laquelle
//! survit.
//!
//! Aucun `Mutex`, aucun `Arc`, aucun `Scope`, aucune fermeture de finaliseur :
//! rien de tout cela ne figure dans la source, et rien de tout cela n est
//! fabrique ici. Le fichier est du calcul et de la structure, point.
//!
//! # Ce qui n a pas de traduction, et pourquoi
//!
//! Une `Layer` d Effect est une **valeur construite et branchee**, pas une
//! donnee : elle n a pas d equivalent en Rust, et ce fichier n en simule
//! aucune. Concretement, trois choses de la source restent dehors :
//!
//! 1. `buildLocationServiceMap(replacements)` renvoie une `Layer`. La fonction
//!    est donc un **parametre** de [`build`] et de [`plan_replacements`], et son
//!    resultat est un simple nom ([`LayerRef`]) fourni par l appelant ;
//! 2. `LayerNode.compile(root, allReplacements)` renvoie, lui aussi, une
//!    `Layer`. Il est donc lui aussi injecte : [`build`] rend le nom de la
//!    couche compilee, comme la source rend la `Layer` compilee, et la
//!    decision elle-meme reste testable seule via [`plan_replacements`] ;
//! 3. `LayerNode.hasUnbound(root, node)` vit dans `layer-node.ts`, qui n est pas
//!    porte. Une approximation locale est fournie par [`has_unbound_named`].
//!    C est la seule divergence de comportement de ce portage, et elle est
//!    signalee deux fois, dans le code et dans le rapport.
//!
//! Le reste de `layer-node.ts` n est pas implemente non plus : `walk`, `hoist`,
//! le corps de `compile`, `replacementNode`, `replacementMapFrom`,
//! `rewriteReplacementDependencies` et `flatten` sont des fonctions d un autre
//! module TypeScript, que ce fichier n appelle pas, ou dont il n appelle que
//! le resultat. Aucun n est simule ici. En particulier :
//!
//! - `replacementNode` leve une exception si le nom du remplacement differe du
//!   nom de la source, et une autre si les balises different. Cette
//!   verification appartient a `layer-node.ts`, elle n est pas portee ici ;
//! - la table des balises (`{ location: ["global"], global: [] }` de
//!   `app-node.ts`) est une verification **de type** en TypeScript, jamais
//!   executee. Elle est notee ici, pas traduite en fonction ;
//! - `export * as AppNodeBuilder from "./app-node-builder"` est un reexport de
//!   l espace de noms du module sur lui-meme, donc du code mort. Il n a pas
//!   d equivalent et n est pas repris.
//!
//! # Deux details de lecture
//!
//! - La source compare des **identites d objet** (`node === source`), pas des
//!   chaines. Le portage compare des noms *et* des genres, ce qui est plus
//!   proche mais pas identique. Les consequences sont detaillees sur
//!   [`has_unbound_named`].
//! - `replacements.concat([...])` **preserve l ordre** et n ajoute qu un
//!   element. Il n y a ni tri, ni deduplication, ni copie defensive du tableau
//!   d origine. Un test le verifie.
//!
//! Aucun nom de champ JSON sur ce fichier : rien n est serialise, donc aucun
//! `#[serde(rename)]` n est necessaire et le piege des majuscules des noms
//! (`projectID`, `sessionID`) ne se pose pas.

use std::fmt;

/// Genre d un noeud du graphe, repris de `LayerNode.Node.kind`.
///
/// La source utilise une chaine (`"layer" | "unbound" | "group"`), donc un
/// `enum` est une traduction directe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// Un noeud qui fournit une implementation : `LayerNode.make`.
    Layer,
    /// Un noeud qui demande un service sans le fournir : `LayerNode.unbound`.
    Unbound,
    /// Un noeud qui ne fait que regrouper d autres noeuds : `LayerNode.group`.
    Group,
}

impl NodeKind {
    /// Renvoie la chaine utilisee par la source.
    pub fn as_str(self) -> &'static str {
        match self {
            NodeKind::Layer => "layer",
            NodeKind::Unbound => "unbound",
            NodeKind::Group => "group",
        }
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Balise d un noeud, definie par `app-node.ts`.
///
/// En TypeScript ce sont des chaines marquees (`Tag<"global">`,
/// `Tag<"location">`), donc deux chaines identiques mais de marques
/// differentes. Ici la distinction est portee par l `enum` lui-meme, ce qui
/// donne la meme garantie a la compilation.
///
/// La table qui accompagne ces balises dans la source,
/// `{ location: ["global"], global: [] }`, dit qu un noeud balise `location`
/// peut dependre d un noeud balise `global` ou `location`, et qu un noeud
/// balise `global` ne peut dependre que d un noeud balise `global`. Cette
/// verification est faite par le systeme de types de TypeScript et n est
/// executee **jamais** : elle n est donc pas reproduite ici.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeTag {
    /// Balise `global`, la seule utilisee par ce fichier.
    Global,
    /// Balise `location`, declaree en face de `global` par `app-node.ts`.
    Location,
}

impl NodeTag {
    /// Renvoie la chaine utilisee par la source.
    pub fn as_str(self) -> &'static str {
        match self {
            NodeTag::Global => "global",
            NodeTag::Location => "location",
        }
    }

    /// Balise dont la valeur est la chaine donnee, si elle existe.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "global" => Some(NodeTag::Global),
            "location" => Some(NodeTag::Location),
            _ => None,
        }
    }
}

impl fmt::Display for NodeTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Nom donne a une couche de la bibliotheque `effect`.
///
/// Ce n est **pas** une couche : c est une etiquette. Une `Layer` d Effect est
/// une valeur construite et branchee, sans equivalent en Rust, donc la
/// fabrication de la carte des services comme la compilation du graphe sont
/// injectees dans [`build`] sous forme de fonctions qui rendent ce nom. Le
/// graphe, lui-meme, est porte par [`AppNode`].
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerRef(String);

impl LayerRef {
    /// Nomme une couche construite par l appelant.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Renvoie le nom de la couche.
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LayerRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Un noeud du graphe de couches.
///
/// Correspond a `LayerNode.Node` de `layer-node.ts`, dont deux champs sont
/// volontairement absents ici :
///
/// - `service` : en TypeScript c est une `Context.Service.Any`, c est a dire
///   une cle. Or `LayerNode.make` recopie deja cette cle dans `name`
///   (`name: input.service !== undefined ? input.service.key : input.name`),
///   et aucun code de ce fichier ne lit `service` autrement. Le nom suffit donc.
/// - `implementation` : une `Layer.Any`, sans equivalent. Le noeud porte donc
///   un [`LayerRef`], c est a dire un nom, et non une couche.
///
/// Les trois constructeurs reproduisent `LayerNode.group`, `LayerNode.unbound`
/// et `LayerNode.make`, qui sont appeles par ce fichier ou par ses voisins
/// dans le portage.
///
/// L arborescence est **possedee** : un `AppNode` contient ses dependances par
/// valeur, donc il ne peut pas contenir de cycle. Le `walk` de `layer-node.ts`
/// qui leve `Cycle detected in layer tree` n a pas d equivalent necessaire ici.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppNode {
    kind: NodeKind,
    name: String,
    tag: Option<NodeTag>,
    dependencies: Vec<AppNode>,
    implementation: Option<LayerRef>,
}

impl AppNode {
    /// Cree un noeud de groupe, qui ne fait que regrouper ses dependances.
    ///
    /// Comme dans `LayerNode.group`, le nom vaut `"group"` et aucune balise
    /// n est posee.
    pub fn group(dependencies: Vec<AppNode>) -> Self {
        Self {
            kind: NodeKind::Group,
            name: "group".to_string(),
            tag: None,
            dependencies,
            implementation: None,
        }
    }

    /// Cree un noeud non lie : il demande un service sans le fournir.
    ///
    /// Comme dans `LayerNode.unbound`, le nom est la cle du service et la
    /// liste de dependances est vide.
    pub fn unbound(name: impl Into<String>, tag: NodeTag) -> Self {
        Self {
            kind: NodeKind::Unbound,
            name: name.into(),
            tag: Some(tag),
            dependencies: Vec::new(),
            implementation: None,
        }
    }

    /// Cree un noeud de couche qui fournit une implementation et ne depend de
    /// rien d autre. C est la forme utilisee par ce fichier pour emballer la
    /// carte des services de localisation.
    pub fn layer(name: impl Into<String>, tag: NodeTag, implementation: LayerRef) -> Self {
        Self {
            kind: NodeKind::Layer,
            name: name.into(),
            tag: Some(tag),
            dependencies: Vec::new(),
            implementation: Some(implementation),
        }
    }

    /// Cree un noeud de couche qui a des dependances, comme le permet
    /// `LayerNode.make`.
    pub fn layer_with_dependencies(
        name: impl Into<String>,
        tag: NodeTag,
        implementation: LayerRef,
        dependencies: Vec<AppNode>,
    ) -> Self {
        Self {
            kind: NodeKind::Layer,
            name: name.into(),
            tag: Some(tag),
            dependencies,
            implementation: Some(implementation),
        }
    }

    /// Renvoie le genre du noeud.
    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    /// Renvoie le nom du noeud, qui est la cle du service pour un noeud construit
    /// a partir d un service.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Renvoie la balise du noeud, ou `None` pour un noeud de groupe.
    pub fn tag(&self) -> Option<NodeTag> {
        self.tag
    }

    /// Renvoie les dependances directes du noeud, dans l ordre de la source.
    pub fn dependencies(&self) -> &[AppNode] {
        &self.dependencies
    }

    /// Renvoie le nom de l implementation fournie, ou `None` si le noeud n en
    /// fournit aucune.
    pub fn implementation(&self) -> Option<&LayerRef> {
        self.implementation.as_ref()
    }
}

/// Valeur de remplacement acceptee pour un noeud.
///
/// La source autorise `AnyNode | Layer.Any`, donc deux formes. La forme noeud
/// est portee telle quelle ; la forme `Layer.Any` n existe pas en Rust et se
/// resout en un nom ([`LayerRef`]). Ce fichier ne lit **jamais** cette valeur :
/// seules les sources des remplacements sont consultees, comme dans la source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplacementValue {
    /// Le service est fourni par un autre noeud du graphe.
    Node(AppNode),
    /// Le service est fourni par une couche Effect prise hors du graphe.
    RawLayer(LayerRef),
}

/// Un remplacement : le noeud qui est remplace, et ce qui le remplace.
///
/// Equivalent du couple `[source, replacement]` de la source. Le premier
/// element est **toujours** un noeud, jamais une couche : c est ce qui permet
/// a [`has_replacement`] de lire le premier element du couple sans se soucier
/// de la forme du second.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replacement {
    /// Le noeud remplace, dont le nom sert de cle.
    pub source: AppNode,
    /// Ce qui prend sa place.
    pub replacement: ReplacementValue,
}

impl Replacement {
    /// Cree un remplacement dont la source et le remplacement sont deux noeuds.
    pub fn new(source: AppNode, replacement: AppNode) -> Self {
        Self {
            source,
            replacement: ReplacementValue::Node(replacement),
        }
    }

    /// Cree un remplacement dont la source est un noeud et le remplacement une
    /// couche prise hors du graphe.
    pub fn with_raw_layer(source: AppNode, replacement: LayerRef) -> Self {
        Self {
            source,
            replacement: ReplacementValue::RawLayer(replacement),
        }
    }

    /// Renvoie le nom du noeud source.
    pub fn source_name(&self) -> &str {
        self.source.name()
    }
}

/// Cle du `Context.Service` `LocationServiceMap`, declaree dans
/// `packages/core/src/location-service-map.ts`.
///
/// C est aussi le nom du noeud non lie de ce service, donc le nom que
/// cherche [`has_replacement`]. La chaine est recopiee telle quelle, avec les
/// deux "@" et le point, parce que c est elle qui sert de cle de comparaison.
///
/// La meme chaine existe sous le nom `SERVICE_KEY` dans
/// `crate::swarm::location_service_map`. Les deux fichiers sont portes par des
/// agents differents et aucun n importe l autre ; la duplication est signalee
/// dans le rapport, elle n est pas resolue ici.
pub const LOCATION_SERVICE_MAP_SERVICE_KEY: &str = "@opencode/example/LocationServiceMap";

/// Le noeud non lie du service `LocationServiceMap`.
///
/// Equivalent de `LayerNode.unbound(Service, Node.tags.values.global)`, c est a
/// dire de la ligne 16 de `location-service-map.ts` : le nom est la cle du
/// service, la balise est `global`, et le service lui-meme n est pas declare
/// ici. Il appartient a `location-service-map.ts`.
pub fn location_service_map_node() -> AppNode {
    AppNode::unbound(LOCATION_SERVICE_MAP_SERVICE_KEY, NodeTag::Global)
}

/// Le noeud qui fournit la carte des services par emplacement.
///
/// Equivalent de `makeGlobalNode({ service, layer, deps: [] })` applique a la
/// carte construite : nom du service, balise `global`, implementation donnee,
/// aucune dependance.
pub fn location_service_map_implementation_node(implementation: LayerRef) -> AppNode {
    AppNode::layer(
        LOCATION_SERVICE_MAP_SERVICE_KEY,
        NodeTag::Global,
        implementation,
    )
}

/// Dit si la liste des remplacements contient deja une entree pour ce noeud.
///
/// Portage direct du `hasReplacement` de la source, qui ecrit
/// `replacements.some(([source]) => source.name === node.name)` :
///
/// - la comparaison porte sur le **nom de la source** du couple, jamais sur la
///   valeur de remplacement, meme si cette valeur est une couche brute ;
/// - une liste vide donne `false` ;
/// - la liste est parcourue dans l ordre, et l arret est immediat.
///
/// Le nom compare est la cle du service, donc deux noeuds distincts qui portent
/// le meme nom sont indiscernables ici. C est voulu : c est ce que fait la
/// source, qui compare des chaines.
pub fn has_replacement(replacements: &[Replacement], node: &AppNode) -> bool {
    let name = node.name();
    replacements
        .iter()
        .any(|remplacement| remplacement.source_name() == name)
}

/// Dit si l arbre `root` contient un noeud non lie portant ce nom, a n importe
/// quelle profondeur.
///
/// Approximation locale de `LayerNode.hasUnbound`, qui vit dans
/// `layer-node.ts` et qui n est pas porte. La source ecrit :
///
/// ```ts
/// export function hasUnbound(root, source) {
///   if (source.kind !== "unbound") throw new Error(`Cannot check non-unbound layer node: ${source.name}`)
///   return walk(root, (node, context) => {
///     if (node === source) return true
///     return node.dependencies.some(context.visit)
///   })
/// }
/// ```
///
/// Trois differences, a connaitre :
///
/// 1. le TypeScript compare des **identites d objet** (`node === source`), donc
///    deux noeuds distincts de meme nom ne sont pas le meme noeud. Ici la
///    comparaison porte sur le nom **et** sur le genre `unbound`, ce qui est
///    plus proche : un noeud de couche ou un groupe qui porterait le nom du
///    service ne compte pas. Il reste que deux noeuds non lies distincts de
///    meme nom restent indiscernables. Dans le code d origine tous les noeuds
///    non lies sont des constantes de module, ce qui rend les deux criteres
///    equivalents en pratique, mais pas par construction ;
/// 2. la source refuse de s appliquer a une source qui n est pas non liee, et
///    leve alors une exception. Ici le parametre est un nom, donc cette
///    verification n a pas lieu d etre ;
/// 3. la source interdit les cycles par `visiting`, et utilise un cache. Ici
///    l arbre est possede et donc sans cycle, et le nom recherche est unique
///    dans les cas reels : aucun cache n est necessaire.
///
/// La descente suit exactement la source : le noeud visite est compare
/// d abord, puis ses dependances directes sont visitees, dans l ordre, et
/// l arret est immediat.
pub fn has_unbound_named(root: &AppNode, name: &str) -> bool {
    if root.kind() == NodeKind::Unbound && root.name() == name {
        return true;
    }
    root.dependencies()
        .iter()
        .any(|dependency| has_unbound_named(dependency, name))
}

/// Ce que la decision de `build` produit, avant la compilation du graphe.
///
/// La source ne renvoie pas cela : elle renvoie la `Layer` compilee. Ce module
/// expose les remplacements a compiler, et la carte de services qu il a
/// eventuellement construite, pour que l appelant puisse la fournir a sa
/// compilation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildPlan {
    /// Les remplacements a transmettre a la compilation, dans l ordre de la
    /// source, eventuellement augmentes de la carte en dernier.
    pub replacements: Vec<Replacement>,
    /// Le nom de la carte des services construite pendant la decision, ou
    /// `None` si rien n a ete construit.
    pub location_service_map: Option<LayerRef>,
}

/// Decide des remplacements, la condition de la source etant deja evaluee.
///
/// C est le coeur du fichier. La condition de la source est
///
/// ```ts
/// LayerNode.hasUnbound(root, LocationServiceMap.node)
///   && !hasReplacement(replacements, LocationServiceMap.node)
/// ```
///
/// Elle est donnee ici sous la forme de l argument `racine_demande_la_carte`,
/// parce que le premier terme appartient a `layer-node.ts`. Comme dans la
/// source, la construction n a lieu que si les deux termes sont vrais, et
/// `make_location_service_map` n est **appelee** dans aucun autre cas.
///
/// L ajout se fait en fin de liste, comme le `concat` de la source, donc
/// l ordre des remplacements deja presents est preserve. Le tableau d origine
/// n est jamais modifie : la source non plus, `concat` renvoyant une nouvelle
/// liste.
pub fn plan_replacements(
    racine_demande_la_carte: bool,
    replacements: &[Replacement],
    make_location_service_map: impl FnOnce(&[Replacement]) -> LayerRef,
) -> BuildPlan {
    let carte_source = location_service_map_node();
    if !racine_demande_la_carte || has_replacement(replacements, &carte_source) {
        return BuildPlan {
            replacements: replacements.to_vec(),
            location_service_map: None,
        };
    }

    // L appelant construit la carte avec les remplacements d origine, et non
    // avec la liste complete : la source passe `replacements` a
    // `buildLocationServiceMap` avant le `concat`.
    let carte = make_location_service_map(replacements);
    let noeud = location_service_map_implementation_node(carte.clone());

    let mut tous = replacements.to_vec();
    tous.push(Replacement::new(carte_source, noeud));

    BuildPlan {
        replacements: tous,
        location_service_map: Some(carte),
    }
}

/// Point d entree, equivalent de `build`.
///
/// La condition est evaluee avec [`has_unbound_named`], qui approxime
/// `LayerNode.hasUnbound(root, LocationServiceMap.node)`.
///
/// Deux etapes de la source n ont pas de valeur de retour traduisible, donc
/// deux parametres :
///
/// - `make_location_service_map` remplace `buildLocationServiceMap`, qui
///   renvoie une `Layer` d Effect. Elle est appelee **exactement** une fois si
///   et seulement si la carte est necessaire, et jamais sinon ;
/// - `compile` remplace `LayerNode.compile`, qui renvoie lui aussi une
///   `Layer`. Elle recoit la racine et la liste **augmentee**, comme la source,
///   et son resultat est ce que `build` renvoie.
///
/// Le parametre `replacements` n a pas de valeur par defaut, la source
/// notwithstanding : en Rust la liste vide s ecrit `&[]`, et un nom de
/// parametre muet serait moins clair qu un appel explicite.
pub fn build(
    root: &AppNode,
    replacements: &[Replacement],
    make_location_service_map: impl FnOnce(&[Replacement]) -> LayerRef,
    compile: impl FnOnce(&AppNode, &[Replacement]) -> LayerRef,
) -> LayerRef {
    let plan = plan_replacements(
        has_unbound_named(root, LOCATION_SERVICE_MAP_SERVICE_KEY),
        replacements,
        make_location_service_map,
    );
    compile(root, &plan.replacements)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// Nom que la construction produit. Il encode le nombre de remplacements
    /// recus, ce qui permet de verifier que la carte est construite sur la
    /// liste d origine et pas sur la liste complete.
    fn nom_de_carte(replacements: &[Replacement]) -> String {
        format!("carte[{}]", replacements.len())
    }

    fn noeud(nom: &str) -> AppNode {
        AppNode::unbound(nom, NodeTag::Global)
    }

    /// Le meme decideur que [`build`], sans l etape de compilation, pour que les
    /// tests portent sur la decision et non sur la fabrication de la couche.
    fn plan_de_test(
        racine: &AppNode,
        remplacements: &[Replacement],
        appels: &Cell<usize>,
    ) -> BuildPlan {
        plan_replacements(
            has_unbound_named(racine, LOCATION_SERVICE_MAP_SERVICE_KEY),
            remplacements,
            |recus: &[Replacement]| {
                appels.set(appels.get() + 1);
                LayerRef::new(nom_de_carte(recus))
            },
        )
    }

    #[test]
    fn une_liste_de_remplacements_vide_ne_declenche_aucune_construction() {
        let appels = Cell::new(0usize);

        let plan = plan_de_test(&AppNode::group(vec![]), &[], &appels);

        // La racine ne demande rien, donc rien n est construit et rien n est ajoute.
        assert_eq!(appels.get(), 0);
        assert_eq!(plan.location_service_map, None);
        assert!(plan.replacements.is_empty());
    }

    #[test]
    fn une_carte_necessaire_et_absente_est_construite_et_ajoutee_en_dernier() {
        let appels = Cell::new(0usize);
        let racine = AppNode::group(vec![
            noeud("A"),
            AppNode::group(vec![noeud("B")]),
            noeud(LOCATION_SERVICE_MAP_SERVICE_KEY),
        ]);
        let remplacements = vec![Replacement::new(noeud("A"), noeud("A-bis"))];

        let plan = plan_de_test(&racine, &remplacements, &appels);

        assert_eq!(appels.get(), 1);
        let noms: Vec<&str> = plan.replacements.iter().map(|r| r.source_name()).collect();
        // L ordre d origine est preserve et la carte arrive en dernier.
        assert_eq!(noms, vec!["A", LOCATION_SERVICE_MAP_SERVICE_KEY]);
        assert_eq!(plan.replacements[0], remplacements[0]);
    }

    #[test]
    fn la_carte_construite_est_un_noeud_global_sans_dependance() {
        let racine = AppNode::group(vec![noeud(LOCATION_SERVICE_MAP_SERVICE_KEY)]);

        let plan = plan_de_test(&racine, &[], &Cell::new(0usize));

        let ajout = &plan.replacements[0];
        let valeur = match &ajout.replacement {
            ReplacementValue::Node(noeud) => noeud,
            autre => panic!("le remplacement doit etre un noeud, obtenu {:?}", autre),
        };
        assert_eq!(ajout.source, location_service_map_node());
        assert_eq!(valeur.kind(), NodeKind::Layer);
        assert_eq!(valeur.name(), "@opencode/example/LocationServiceMap");
        assert_eq!(valeur.tag(), Some(NodeTag::Global));
        assert!(valeur.dependencies().is_empty());
        assert_eq!(valeur.implementation(), Some(&LayerRef::new("carte[0]")));
    }

    #[test]
    fn une_carte_deja_fournie_ne_construit_rien_et_ne_rajoute_rien() {
        let appels = Cell::new(0usize);
        let carte = noeud(LOCATION_SERVICE_MAP_SERVICE_KEY);
        let remplacements = vec![
            Replacement::new(noeud("A"), noeud("A-bis")),
            Replacement::new(carte.clone(), noeud("fourni-par-ailleurs")),
        ];
        let racine = AppNode::group(vec![carte.clone()]);

        let plan = plan_de_test(&racine, &remplacements, &appels);

        assert_eq!(appels.get(), 0);
        assert_eq!(plan.location_service_map, None);
        // La liste est transmise telle quelle, ni completee ni reordonnee.
        assert_eq!(plan.replacements, remplacements);
    }

    #[test]
    fn la_forme_du_remplacement_existant_est_ignoree() {
        // La source ecrit `[source]`, donc seule la source est lue. Une
        // entree dont la source porte le bon nom compte, meme si la valeur est
        // une couche brute, et meme si elle ne ressemble pas a une carte.
        let appels = Cell::new(0usize);
        let remplacements = vec![Replacement::with_raw_layer(
            noeud(LOCATION_SERVICE_MAP_SERVICE_KEY),
            LayerRef::new("couche-hors-graphe"),
        )];
        let racine = AppNode::group(vec![noeud(LOCATION_SERVICE_MAP_SERVICE_KEY)]);

        let plan = plan_de_test(&racine, &remplacements, &appels);

        assert_eq!(appels.get(), 0);
        assert_eq!(plan.replacements.len(), 1);
        assert_eq!(plan.replacements[0], remplacements[0]);
    }

    #[test]
    fn un_nom_presque_egal_ne_compte_pas_comme_carte_fournie() {
        // Le nom est compare avec `===`, sans normalisation : une cle voisine
        // ne suffit pas.
        for faux in [
            "LocationServiceMap",
            "@opencode/example/locationServiceMap",
            "@opencode/example/LocationServiceMaps",
            "@opencode/example/LocationServiceMap ",
            "group",
        ] {
            assert!(
                !has_replacement(
                    &[Replacement::new(noeud(faux), noeud("peu-importe"))],
                    &location_service_map_node()
                ),
                "le nom {} ne devrait pas compter",
                faux
            );
        }
    }

    #[test]
    fn une_racine_sans_carte_ne_construit_rien_meme_avec_des_remplacements() {
        let appels = Cell::new(0usize);
        let remplacements = vec![Replacement::new(noeud("A"), noeud("A-bis"))];

        let plan = plan_de_test(
            &AppNode::group(vec![noeud("A")]),
            &remplacements,
            &appels,
        );

        assert_eq!(appels.get(), 0);
        assert_eq!(plan.location_service_map, None);
        assert_eq!(plan.replacements, remplacements);
    }

    #[test]
    fn une_carte_profonde_dans_un_groupe_est_detectee() {
        let racine = AppNode::group(vec![
            noeud("A"),
            AppNode::group(vec![AppNode::group(vec![noeud(
                LOCATION_SERVICE_MAP_SERVICE_KEY,
            )])]),
        ]);

        assert!(has_unbound_named(
            &racine,
            LOCATION_SERVICE_MAP_SERVICE_KEY
        ));
    }

    #[test]
    fn un_noeud_isole_avec_dependances_reste_detecte() {
        // La descente ne depend pas du genre du noeud visite.
        let feuille = AppNode::layer_with_dependencies(
            "Service",
            NodeTag::Location,
            LayerRef::new("impl"),
            vec![noeud(LOCATION_SERVICE_MAP_SERVICE_KEY)],
        );

        assert!(has_unbound_named(
            &AppNode::group(vec![feuille]),
            LOCATION_SERVICE_MAP_SERVICE_KEY
        ));
    }

    #[test]
    fn un_noeud_de_couche_du_meme_nom_ne_compte_pas() {
        // La source compare des identites : le noeud recherche est
        // `LocationServiceMap.node`, qui est non lie. Un noeud de couche qui
        // porte le meme nom est un autre objet, donc la source ne le voit pas.
        let layer = AppNode::layer(
            LOCATION_SERVICE_MAP_SERVICE_KEY,
            NodeTag::Global,
            LayerRef::new("autre"),
        );
        let racine = AppNode::group(vec![noeud("A"), layer]);

        assert!(!has_unbound_named(
            &racine,
            LOCATION_SERVICE_MAP_SERVICE_KEY
        ));
    }

    #[test]
    fn une_carte_demandee_plusieurs_fois_ne_produit_qu_un_ajout() {
        // Deux demandes du meme noeud non lie, a deux profondeurs. La
        // condition est un `&&`, pas un compteur : la carte est construite une
        // fois et ajoutee une fois.
        let carte = noeud(LOCATION_SERVICE_MAP_SERVICE_KEY);
        let racine = AppNode::group(vec![carte.clone(), AppNode::group(vec![carte])]);
        let appels = Cell::new(0usize);

        let plan = plan_de_test(&racine, &[], &appels);

        assert_eq!(appels.get(), 1);
        assert_eq!(plan.replacements.len(), 1);
        assert_eq!(
            plan.replacements[0].source_name(),
            LOCATION_SERVICE_MAP_SERVICE_KEY
        );
    }

    #[test]
    fn la_cle_du_service_est_celle_du_context_service() {
        // La comparaison de la source porte sur cette chaine. Une faute de
        // frappe ici casserait silencieusement la detection.
        assert_eq!(
            LOCATION_SERVICE_MAP_SERVICE_KEY,
            "@opencode/example/LocationServiceMap"
        );
        assert_eq!(
            location_service_map_node().name(),
            LOCATION_SERVICE_MAP_SERVICE_KEY
        );
        assert_eq!(location_service_map_node().kind(), NodeKind::Unbound);
        assert_eq!(location_service_map_node().tag(), Some(NodeTag::Global));
        assert!(location_service_map_node().dependencies().is_empty());
    }

    #[test]
    fn une_balise_inconnue_ne_se_confond_avec_aucune_balise_connue() {
        assert_eq!(NodeTag::from_name("global"), Some(NodeTag::Global));
        assert_eq!(NodeTag::from_name("location"), Some(NodeTag::Location));
        assert_eq!(NodeTag::from_name("Global"), None);
        assert_eq!(NodeTag::from_name(""), None);
    }

    #[test]
    fn un_noeud_de_groupe_ne_porte_ni_balise_ni_implementation() {
        let groupe = AppNode::group(vec![noeud("A")]);

        assert_eq!(groupe.kind(), NodeKind::Group);
        assert_eq!(groupe.name(), "group");
        assert_eq!(groupe.tag(), None);
        assert_eq!(groupe.implementation(), None);
        assert_eq!(groupe.dependencies().len(), 1);
    }

    #[test]
    fn la_construction_recoit_les_remplacements_d_origine() {
        // La source appelle `buildLocationServiceMap(replacements)` avant le
        // `concat`, donc la carte est construite sur la liste d origine.
        let appels = Cell::new(0usize);
        let remplacements = vec![Replacement::new(noeud("A"), noeud("A-bis"))];
        let racine = AppNode::group(vec![noeud(LOCATION_SERVICE_MAP_SERVICE_KEY)]);

        let plan = plan_de_test(&racine, &remplacements, &appels);

        assert_eq!(appels.get(), 1);
        assert_eq!(plan.location_service_map, Some(LayerRef::new("carte[1]")));
        assert_eq!(plan.replacements.len(), 2);
    }

    #[test]
    fn la_liste_d_origine_n_est_jamais_modifiee() {
        // `concat` renvoie une nouvelle liste : le tableau fourni par
        // l appelant doit etre intact apres la decision.
        let appels = Cell::new(0usize);
        let remplacements = vec![Replacement::new(noeud("A"), noeud("A-bis"))];
        let avant = remplacements.clone();
        let racine = AppNode::group(vec![noeud(LOCATION_SERVICE_MAP_SERVICE_KEY)]);

        let _ = plan_de_test(&racine, &remplacements, &appels);

        assert_eq!(remplacements, avant);
        assert_eq!(remplacements.len(), 1);
    }

    #[test]
    fn la_compilation_recoit_la_liste_augmentee_et_son_resultat_est_renvoye() {
        // La source termine par `return LayerNode.compile(root, allReplacements)`,
        // donc la compilation voit la liste augmentee, et c est son resultat
        // que `build` renvoie.
        let racine = AppNode::group(vec![noeud(LOCATION_SERVICE_MAP_SERVICE_KEY)]);
        let vus = Cell::new(0usize);
        let taille = Cell::new(0usize);
        let noms = Cell::new(String::new());

        let resultat = build(
            &racine,
            &[Replacement::new(noeud("A"), noeud("A-bis"))],
            |recus: &[Replacement]| LayerRef::new(nom_de_carte(recus)),
            |_racine: &AppNode, remplacements: &[Replacement]| {
                vus.set(vus.get() + 1);
                taille.set(remplacements.len());
                noms.set(
                    remplacements
                        .iter()
                        .map(|r| r.source_name().to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                );
                LayerRef::new("couche-application")
            },
        );

        assert_eq!(resultat, LayerRef::new("couche-application"));
        assert_eq!(vus.get(), 1);
        assert_eq!(taille.get(), 2);
        assert_eq!(
            noms.take(),
            "A,@opencode/example/LocationServiceMap",
            "la compilation doit voir la carte en dernier"
        );
    }

    #[test]
    fn la_compilation_a_lieu_meme_quand_rien_n_est_ajoute() {
        // La source compile dans tous les cas : l absence de carte n annule
        // pas la compilation, elle passe simplement la liste telle quelle.
        let racine = AppNode::group(vec![noeud("A")]);
        let taille = Cell::new(0usize);
        let appels_carte = Cell::new(0usize);

        let resultat = build(
            &racine,
            &[Replacement::new(noeud("A"), noeud("A-bis"))],
            |_recus: &[Replacement]| {
                appels_carte.set(appels_carte.get() + 1);
                LayerRef::new("jamais")
            },
            |_racine: &AppNode, remplacements: &[Replacement]| {
                taille.set(remplacements.len());
                LayerRef::new("couche-application")
            },
        );

        assert_eq!(appels_carte.get(), 0);
        assert_eq!(taille.get(), 1);
        assert_eq!(resultat, LayerRef::new("couche-application"));
    }

    #[test]
    fn le_plan_par_defaut_ne_contient_ni_remplacement_ni_carte() {
        let plan = BuildPlan::default();

        assert!(plan.replacements.is_empty());
        assert_eq!(plan.location_service_map, None);
    }
}
