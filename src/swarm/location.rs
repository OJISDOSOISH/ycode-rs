//! Portage Rust de `opencode/packages/core/src/location.ts`.
//!
//! La source tient en **39 lignes**, et sa premiere ligne est un reexport de
//! l'espace de noms du module sur lui-meme, donc sans traduction. Ce qui reste
//! tient en quatre choses, dont une seule contient de la logique :
//!
//! ```ts
//! export { Info, Ref, response }
//!
//! export interface Interface extends Info {
//!   readonly vcs?: Project.Vcs
//! }
//!
//! export class Service extends Context.Service<Service, Interface>()("@opencode/Location") {}
//!
//! export const node = LayerNode.unbound(Service, tags.values.location)
//!
//! const layer = (ref: Ref) =>
//!   Layer.effect(Service, Effect.gen(function* () {
//!     const project = yield* Project.Service
//!     const resolved = yield* project.resolve(ref.directory)
//!     return Service.of({
//!       directory: ref.directory,
//!       workspaceID: ref.workspaceID,
//!       project: { id: resolved.id, directory: resolved.directory },
//!       vcs: resolved.vcs,
//!     })
//!   }))
//!
//! export const boundNode = (ref: Ref) =>
//!   makeLocationNode({ service: Service, layer: layer(ref), deps: [Project.node] })
//! ```
//!
//! # Les trois exports `Info`, `Ref`, `response`
//!
//! - `Ref` vient de `packages/schema/src/location.ts` et vaut
//!   `{ directory, workspaceID? }`. C'est exactement le struct `LocationRef`
//!   **deja porte** par `src/core/session/schema.rs`, qui le porte pour la
//!   session. On le **reexporte** sous le nom de la source, on ne le
//!   redeclare pas : deux definitions du meme contrat qui divergent en
//!   silence sont le defaut que la revue a deja attrape une fois.
//! - `Info` est `{ directory, workspaceID?, project: { id, directory } }`. C'est
//!   le seul contrat reellement publie par ce fichier, et il est porte ici.
//! - `response` est un constructeur de schema. En Rust un schema n'a pas de
//!   representation : il ne produit aucune donnee, seulement une forme. Le type
//!   [`Response`] **est** cette forme, et [`response`] le remplit.
//!
//! # Le piege des noms de champs : `workspaceID`
//!
//! `workspaceID` porte un **D** majuscule, dans les trois structs concernes
//! (`Ref`, `Info`, et par heritage `Interface`). Ecrit `workspaceId`, le code
//! compile, tous les tests de logique passent, et l'erreur n'apparait qu'a
//! l'echange avec le TypeScript. Le `#[serde(rename = "workspaceID")]` est donc
//! pose explicitement, et un test verifie le nom sur la sortie JSON.
//!
//! # Le piege `?` contre `??` : ce qu'il se passe reellement
//!
//! **Ce fichier ne contient ni ternaire ni coalescent.** Les quatre champs
//! construits par `layer` sont des recopies brutes : `directory: ref.directory`,
//! `workspaceID: ref.workspaceID`, `vcs: resolved.vcs`. Aucune veracite n'est
//! testee, donc aucune chaine vide ne disparait. Un `.filter(|w| !w.is_empty())`
//! ajoute ici serait une invention.
//!
//! Le seul point de decision qui reste est l'**encodage** du champ
//! facultatif, dans `packages/schema/src/schema.ts` :
//!
//! ```ts
//! export const optional = (schema) =>
//!   Schema.optionalKey(schema).pipe(
//!     Schema.decodeTo(Schema.optional(Schema.toType(schema)), {
//!       decode: SchemaGetter.passthrough({ strict: false }),
//!       encode: SchemaGetter.transformOptional(Option.filter((value) => value !== undefined)),
//!     }),
//!   )
//! ```
//!
//! Le filtre est `value !== undefined` : c'est un test de **nullite**, pas de
//! veracite. Un filtrage par veracite serait donc faux, et c'est ce que le
//! test `l_encodage_ne_supprime_jamais_une_chaine_vide` verrouille.
//!
//! ## ...mais la chaine vide est hors d'atteinte, et ce n'est pas anodin
//!
//! Les deux types du couple ne se comportent pas pareil, et la lecture
//! hative du schema donne une fausse confiance sur ce point.
//!
//! - `AbsolutePath` est `Schema.String.pipe(Schema.brand("AbsolutePath"))` :
//!   une **marque** et rien d'autre. Une chaine vide est une valeur tout a
//!   fait legitime, et `directory: ""` survit donc reellement.
//! - `WorkspaceID` est
//!   `Schema.String.check(Schema.isStartsWith("wrk")).pipe(Schema.brand(...))` :
//!   une **verification** en plus. Toute chaine qui ne commence pas par `wrk`
//!   est refusee a la decodage, et `""` en fait partie.
//!
//! Consequence : un `workspaceID` vide ne peut pas exister dans une source
//! bien typee, puisque `Ref` sort lui-meme d une decodage de schema. Le test
//! qui le construit verifie donc une etat **non representable** cote
//! TypeScript. Il n est pas faux -- il fixe le comportement de l encodage et
//! interdit d introduire un jour un `.filter()` par veracite -- mais il ne
//! faut pas le lire comme une regle de la source. Le prefixe est repris dans
//! [`WORKSPACE_ID_PREFIX`] pour que la divergence soit explicite plutot que
//! distribuee dans des tests.
//!
//! Une consequence cote implementation : la source construit un objet dont la
//! propriete vaut `undefined`, et c'est l'encodage qui retire ensuite la cle.
//! En Rust on represente des la construction l'absence par `None`, et
//! `skip_serializing_if` retire la cle au meme endroit. Les deux choix sont
//! indiscernables sur la sortie JSON.
//!
//! # Le piege inverse, celui qui ne se voit pas : la lecture
//!
//! Le piege d ecriture est regle par `#[serde(rename = ...)]`. Le piege de
//! **lecture** n est regle par rien du tout : `serde` ignore par defaut une
//! cle inconnue, et ce fichier n'emploie pas `deny_unknown_fields`, parce que
//! la source ne l'emploie pas non plus (`Schema.Struct` ignore les proprietes
//! en trop par defaut).
//!
//! Consequence concrete et verifiable : un payload ou la cle s'appelle
//! `workspaceId` au lieu de `workspaceID` est **accepte sans bruit**, donne
//! `workspace_id: None`, puis perd la valeur a la re-serialisation. Une faute
//! de casse se paie donc par une **disparition silencieuse**, pas par une
//! erreur. Les tests `une_cle_snake_case_est_ignoree_a_la_lecture_sur_info`
//! et `..._sur_interface` fixent ce comportement tel quel, pour que la revue
//! sache qu il est voulu et non subi.
//!
//! `deny_unknown_fields` ne pourrait de toute facon pas etre ajoute a
//! [`Interface`] : l attribut est incompatible avec `#[serde(flatten)]`, et le
//! `flatten` est indispensable (voir plus bas). Le durcissement serait donc
//! soit un ecart de fidelite sur [`Info`], soit impossible sur [`Interface`].
//!
//! # Deux repertoires, et ils ne sont pas le meme
//!
//! `layer` ecrit `directory: ref.directory` mais
//! `project: { id: resolved.id, directory: resolved.directory }`. Le premier
//! vient du **reference** fourni a `boundNode`, le second de la **resolution** du
//! service projet. Ils ne coincident pas toujours : `Project.resolve` renvoie
//! la racine du depot, alors que `ref.directory` peut etre un sous-dossier
//! d'un worktree. Confondre les deux ferait perdre le repertoire de travail.
//!
//! # Le graphe de couches, lui, est maintenant porte
//!
//! Une version anterieure de ce fichier affirmait que `LayerNode` n'existait
//! pas en Rust, et laissait `node` et `boundNode` reduits a des constantes.
//! C'etait vrai au moment ou le fichier a ete ecrit, et c'est faux
//! aujourd'hui : `crate::swarm::effect_app_node_builder` porte desormais
//! [`AppNode`], [`NodeTag`] et [`LayerRef`], avec exactement les
//! constructeurs dont ces deux exports ont besoin. Les deux fonctions sont
//! donc portees pour de vrai, sur ces types, sans rien inventer.
//!
//! Ce qui reste hors de portee, et uniquement cela : le `Layer.effect` et la
//! closure `layer(ref)`. Une `Layer` d Effect est une valeur construite et
//! branchee, sans equivalent en Rust. Le graphe est donc decrit par
//! [`bound_node`], et le **corps** de la couche -- l'unique logique du
//! fichier -- par [`bind`].
//!
//! # Ce qui n'est pas porte, et pourquoi
//!
//! - `Service.of({...})` construit la valeur du service a partir de la forme
//!   de l'interface. Comme `Interface` ne declare **aucune methode**, le
//!   [`trait Service`] n'en a aucune non plus : c'est un trait-marqueur, et
//!   [`Interface`] en est la seule implementation. C'est ce que dit la source,
//!   pas un raccourci.
//! - `Project.Service` et `Project.Resolved` sont declares dans
//!   `packages/core/src/project.ts`, qui n'est pas porte dans ce lot. Ils sont
//!   donc reproduits ici sous une forme **minimale** -- [`ProjectResolver`] et
//!   [`ProjectResolved`] -- pour que la signature reste exacte. `Project.node`
//!   n'est pas davantage disponible : c'est pourquoi [`bound_node`] le prend
//!   en parametre au lieu de le construire. Le jour ou `project.ts` sera
//!   porte, ces items doivent disparaitre au profit des vrais, et c'est le
//!   doublon qu'il faudra resoudre.
//!
//! # Note d integration
//!
//! Ce fichier attend `pub mod location;` dans `src/swarm/mod.rs`, qui est deja
//! en place. Il depend de `crate::swarm::project_schema` (pour `Vcs` et
//! `ProjectId`), de `crate::swarm::effect_app_node_builder` (pour [`AppNode`],
//! [`NodeTag`] et [`LayerRef`]) et de `crate::core::session::schema` (pour
//! `AbsolutePath`, `LocationRef` et `WorkspaceId`). Les trois sont deja `pub`.
//!
//! # Un `WorkspaceId`, et deux dans le depot
//!
//! Il existe desormais **deux** types nommes `WorkspaceId` dans l'arbre Rust :
//! l'alias `pub type WorkspaceId = String` de
//! `src/core/session/schema.rs`, et le nouveau type de
//! `src/swarm/core_workspace.rs` qui enveloppe une chaine et **valide son
//! prefixe**. Ce fichier utilise **l'alias**, et pour deux raisons
//! concretes. D'abord le champ `workspace_id` de `Ref` vient de `LocationRef`,
//! donc il est deja type avec l'alias : une `Info` qui prendrait le
//! nouveautype serait d'un autre type que le `Ref` qu'elle recoit, et la
//! recopie de la ligne 27 de la source (`workspaceID: ref.workspaceID`)
//! deviendrait impossible sans conversion inventee. Ensuite le nouveautype
//! impose le prefixe `wrk`, ce qui est une information que la source porte
//! dans le schema et pas dans ce fichier. C'est le meme raisonnement que pour
//! `LocationRef` : rester compatible avec ce qui est deja porte prime sur le
//! choix de type.

use serde::{Deserialize, Serialize};

use crate::core::session::schema::{AbsolutePath, WorkspaceId};
use crate::swarm::effect_app_node_builder::{AppNode, LayerRef, NodeTag};
use crate::swarm::project_schema::{ProjectId, Vcs};

/// Reference vers une localisation.
///
/// Reexport de `LocationRef`, qui est deja porte par
/// `src/core/session/schema.rs`.
///
/// Correspond a `Ref` de `packages/schema/src/location.ts` :
///
/// ```ts
/// export const Ref = Schema.Struct({
///   directory: AbsolutePath,
///   workspaceID: optional(WorkspaceID),
/// })
/// ```
///
/// Le type est **le meme** que celui de la session, a un renommage de module
/// pres : les deux sources declarent exactement `{ directory, workspaceID? }`,
/// et c'est confirme par le type genere du SDK,
/// `packages/sdk/js/src/v2/gen/types.gen.ts` ligne 3047 :
/// `export type LocationRef = { directory: string, workspaceID?: string }`.
/// Ce n'est donc pas une coincidence de forme, c'est le meme contrat. Aucune
/// copie, donc aucune possibilite de divergence.
pub use crate::core::session::schema::LocationRef as Ref;

/// Identifiant du service dans le graphe Effect d'origine.
///
/// La source ecrit la chaine au moment de la declaration du `Context.Service` :
/// `Context.Service<Service, Interface>()("@opencode/Location")`. Elle fait
/// partie de l'identite du service, donc elle est conservee telle quelle.
pub const SERVICE_TAG: &str = "@opencode/Location";

/// Nom du noeud de couches associe.
///
/// `LayerNode.unbound(service, tag)` et `LayerNode.make({ service, ... })`
/// positionnent tous les deux `name: input.service.key`. Le nom du noeud est donc
/// la cle du service, sans exception et sans possibilite de le choisir
/// autrement. Le test le verifie.
pub const NODE_NAME: &str = SERVICE_TAG;

/// Tag du noeud de localisation, lu dans `packages/core/src/effect/app-node.ts`.
///
/// Ce n'est pas une invention : c'est la seule chaine qui rende
/// `NodeTag::Location` conforme a ce que la source ecrit. Le test
/// `les_tags_de_ce_fichier_sont_ceux_du_porteur` le verifie mecaniquement.
pub const TAG_LOCATION: &str = "location";

/// Tag du noeud global, lu dans la meme source.
///
/// Meme verification, meme raison.
pub const TAG_GLOBAL: &str = "global";

/// Tags dont un noeud `location` a le droit de dependre.
///
/// La source ecrit `LayerNode.tags({ location: ["global"], global: [] })`.
///
/// Consequence concrete et verifiable : `boundNode` de ce fichier est un noeud
/// `location` qui depend de `Project.node`, or `project.ts` construit ce
/// dernier par `makeGlobalNode`. La dependance est donc **autorisee** par la
/// configuration des tags, ce qui n'aurait pas ete le cas d'un noeud `global`
/// dependant d'un noeud `location`.
pub const TAGS_LOCATION_DEPENDENCIES: [&str; 1] = [TAG_GLOBAL];

/// Tags dont un noeud `global` a le droit de dependre.
///
/// La liste est **vide** cote source : un noeud global ne peut en dependre
/// aucun. La longueur 0 est donc fidele, et non un oubli.
pub const TAGS_GLOBAL_DEPENDENCIES: [&str; 0] = [];

/// Cle du service projet dont ce fichier depend.
///
/// Lue dans `packages/core/src/project.ts` :
/// `Context.Service<Service, Interface>()("@opencode/ProjectV2")`. Elle n'est
/// pas un contrat publie en JSON, c'est l'identite du noeud dont `boundNode`
/// declare la dependance, et elle est le seul moyen de tracer ce rattachement
/// depuis le Rust.
pub const PROJECT_SERVICE_TAG: &str = "@opencode/ProjectV2";

/// Prefice impose a un identifiant d'espace de travail.
///
/// Lu dans `packages/schema/src/workspace-id.ts` :
/// `Schema.String.check(Schema.isStartsWith("wrk"))`.
///
/// La regle est reprise ici pour etre **visible**, pas pour etre appliquee :
/// le champ `workspace_id` est type avec l'alias `WorkspaceId = String`, qui
/// n'impose rien. La divergence est donc assumee et documentee plutot que
/// repartie dans des tests. Voir la section de l'en-tete sur le prefixe, et le
/// type nouveau [`crate::swarm::core_workspace::WorkspaceId`] qui, lui,
/// l'applique.
pub const WORKSPACE_ID_PREFIX: &str = "wrk";

/// Le projet auquel appartient une localisation.
///
/// Reprise de la structure anonyme
/// `Schema.Struct({ id: ProjectID, directory: AbsolutePath })` de
/// `packages/schema/src/location.ts`. La source ne lui donne pas de nom, d'ou le
/// nom pose ici.
///
/// Aucun nom de champ a renommer : `id` et `directory` sont en minuscules des
/// deux cotes. C'est le contraste avec `workspaceID`, qui est en majuscules,
/// qui rend la verification utile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// Identifiant du projet resolu.
    pub id: ProjectId,
    /// Repertoire racine du projet resolu.
    ///
    /// Attention : c'est le repertoire **du projet**, pas celui du lieu de
    /// travail. Voir l'en-tete, section "Deux repertoires, et ils ne sont pas
    /// le meme".
    pub directory: AbsolutePath,
}

/// Localisation publiee par l'API.
///
/// Correspond a `Info` de `packages/schema/src/location.ts`. L'ordre des
/// champs reprend celui de la source : il n'a aucune signification en JSON, mais
/// il rend la comparaison avec l'original lisible, et ca ne coute rien.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// Repertoire de travail.
    ///
    /// `AbsolutePath` est une marque sans verification : une chaine vide est
    /// donc une valeur legitime et survit a la construction comme a
    /// l'encodage.
    pub directory: AbsolutePath,
    /// Espace de travail parent. Absent si la localisation est a la racine.
    ///
    /// La majuscule est volontaire : `workspaceID`, pas `workspaceId`. C'est le
    /// genre de nom que TypeScript ecrit sans y penser et qu'aucun compilateur
    /// ne signale.
    #[serde(rename = "workspaceID", default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<WorkspaceId>,
    /// Projet auquel la localisation appartient.
    pub project: ProjectInfo,
}

/// Enveloppe de reponse de l'API pour une localisation.
///
/// Correspond a `Schema.Struct({ location: Info, data })`, construit par
/// `response` de `packages/schema/src/location.ts`.
///
/// Le type est generique sur la forme des donnees, comme le parametre `S` de la
/// source. Les deux noms de champs sont poses explicitement, ce qui est la
/// regle du projet, meme si `serde` les choisirait de la meme facon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response<T> {
    /// Localisation de l'appel.
    pub location: Info,
    /// Donnees specifiques a l'appel.
    pub data: T,
}

/// Enveloppe une localisation et des donnees dans la forme de `response`.
///
/// En TypeScript, `response(data)` ne renvoie **aucune donnee** : il renvoie un
/// schema, c'est-a-dire une description inerte. Rust n'a pas de schema, et le
/// type [`Response`] est deja cette description. Cette fonction est donc le seul
/// moyen de donner un sens executable a l'export, et elle produit la valeur que
/// la source ne fait que decrire.
///
/// L'ordre des parametres est `location` puis `data`, l'inverse de l'appel
/// TypeScript : en Rust le type generique se deduit du second argument, et
/// mettre la localisation en premier laisse l'appel se lire dans l'ordre des
/// champs du JSON.
pub fn response<T>(location: Info, data: T) -> Response<T> {
    Response { location, data }
}

/// Localisation et son gestionnaire de versions.
///
/// Correspond a `Interface` de `packages/core/src/location.ts` :
///
/// ```ts
/// export interface Interface extends Info {
///   readonly vcs?: Project.Vcs
/// }
/// ```
///
/// ## Pourquoi un `flatten` et non des champs repetes
///
/// `extends` est une **heritage structurel** en TypeScript : les membres de
/// `Info` sont sur `Interface` au meme niveau, et `location.project.directory`
/// comme `location.directory` sont lus de facon identique par les appelants
/// (`snapshot.ts` ligne 94, `system-context/builtins.ts` ligne 19,
/// `filesystem/watcher.ts` ligne 111). Dupliquer les quatre champs de `Info`
/// produirait un JSON identique mais deux declarations a maintenir, et
/// surtout perdrait le lien avec le struct reellement publie. `flatten` garde le
/// JSON plat, comme la source.
///
/// ## Pourquoi pas `Eq`
///
/// `Vcs` de `crate::swarm::project_schema` ne derive que `Debug, Clone,
/// PartialEq, Serialize, Deserialize` : **pas** `Eq`. Or `derive(Eq)` exige
/// `Eq` sur le type de **chaque** champ, et `vcs: Option<Vcs>` ne l'a pas.
/// Un `Eq` ici serait donc un refus de compilation, et non une simple
/// difference de comportement. Comme rien dans la source ne demande `Eq`, et
/// que `PartialEq` suffit a tous les usages, ce fichier s'en passe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Interface {
    /// Les champs de `Info`, au meme niveau.
    #[serde(flatten)]
    pub info: Info,
    /// Depot git du projet, quand le projet en a un.
    ///
    /// Copie brute de `resolved.vcs` : aucun filtrage. Une localisation sans
    /// depot git a bien `vcs: None` ici, et la cle disparait du JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs: Option<Vcs>,
}

impl Service for Interface {}

/// Service de localisation.
///
/// La source ecrit :
///
/// ```ts
/// export class Service extends Context.Service<Service, Interface>()("@opencode/Location")
/// ```
///
/// `Interface` ne declare **aucune methode**, uniquement des donnees. Le
/// `trait` n'en declare donc aucune non plus : il ne sert qu'a nommer le
/// service, et [`Interface`] en est l'unique implementation. Lui donner des
/// methodes serait inventer une API que la source n'a pas.
pub trait Service {}

/// Ce que le service projet renvoie pour un repertoire donne.
///
/// Reproduit sous forme minimale le `Resolved` de
/// `packages/core/src/project.ts` :
///
/// ```ts
/// export interface Resolved {
///   readonly previous?: ID
///   readonly id: ID
///   readonly directory: AbsolutePath
///   readonly vcs?: Vcs
/// }
/// ```
///
/// Seuls les trois champs que **ce** fichier consomme sont portes. `previous`
/// est omettre parce que `location.ts` ne le lit jamais, et le porter ici
/// exposerait un champ que rien ne remarque.
///
/// Aucun derive `Serialize` : `Resolved` est une interface nue en
/// TypeScript, pas un schema. Elle ne franchit aucune frontiere JSON, et lui
/// donner une representation serie serait inventer une publication.
///
/// Pas de `Eq` non plus, pour la meme raison que [`Interface`] : le champ
/// `vcs: Option<Vcs>` l'interdit.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectResolved {
    /// Identifiant du projet resolu.
    pub id: ProjectId,
    /// Repertoire racine du projet resolu.
    pub directory: AbsolutePath,
    /// Depot git du projet, quand il en a un.
    pub vcs: Option<Vcs>,
}

/// Ce que ce fichier attend du service projet.
///
/// Le `Project.Service` de `packages/core/src/project.ts` expose trois
/// methodes (`directories`, `resolve`, `commit`). Seule `resolve` est utilisee
/// ici, donc seule celle-ci est portee.
///
/// Le type d'erreur est associe, comme dans `src/core/session/execution.rs` :
/// la source ne nomme pas le canal d'erreur de `resolve` dans le code utilise
/// ici, et chaque implementation choisit le sien plutot que d'inventer un type.
pub trait ProjectResolver {
    /// Type d'erreur que `resolve` peut renvoyer.
    type Error;

    /// Resout le projet auquel appartient le repertoire donne.
    fn resolve(&self, directory: &AbsolutePath) -> Result<ProjectResolved, Self::Error>;
}

/// Construit la localisation d'un reference.
///
/// Equivalent du corps de `layer(ref)` dans la source, c'est-a-dire de la
/// totalite du travail de ce fichier.
///
/// ```ts
/// const project = yield* Project.Service
/// const resolved = yield* project.resolve(ref.directory)
/// return Service.of({
///   directory: ref.directory,
///   workspaceID: ref.workspaceID,
///   project: { id: resolved.id, directory: resolved.directory },
///   vcs: resolved.vcs,
/// })
/// ```
///
/// La resolution est le seul point qui peut echouer, et son erreur est
/// remontee telle quelle. Les quatre champs sont ensuite des recopies brutes :
/// `directory` vient du reference, `project` et `vcs` de la resolution. Aucun
/// test de veracite, donc une chaine vide presente survit, comme dans la source.
pub fn bind<P>(project: &P, reference: &Ref) -> Result<Interface, P::Error>
where
    P: ProjectResolver + ?Sized,
{
    let resolved = project.resolve(&reference.directory)?;
    Ok(Interface {
        info: Info {
            // Repertoire de travail : celui du reference.
            directory: reference.directory.clone(),
            // Copie brute, y compris pour une chaine vide.
            workspace_id: reference.workspace_id.clone(),
            // Repertoire du projet : celui de la resolution, qui est different.
            project: ProjectInfo { id: resolved.id, directory: resolved.directory },
        },
        // Copie brute elle aussi : `None` reste `None`.
        vcs: resolved.vcs,
    })
}

/// Le noeud non lie du service de localisation.
///
/// Equivalent direct de `LayerNode.unbound(Service, tags.values.location)` :
/// le nom est la cle du service et la balise est `location`, comme dans
/// `LayerNode.unbound`, et la liste de dependances est vide.
///
/// C'est le noeud que `effect/app-node-builder.ts` recherche pour savoir si un
/// emplacement doit recevoir une implementation (`hasUnbound`).
pub fn node() -> AppNode {
    AppNode::unbound(NODE_NAME, NodeTag::Location)
}

/// Le noeud de couche du service de localisation.
///
/// Equivalent de :
///
/// ```ts
/// export const boundNode = (ref: Ref) =>
///   makeLocationNode({ service: Service, layer: layer(ref), deps: [Project.node] })
/// ```
///
/// `makeLocationNode` est `tags.make("location")`, c'est-a-dire `LayerNode.make`
/// avec la balise imposee. Le nom vient donc de `service.key`, la balise est
/// `location`, et la seule dependance est le noeud du service projet.
///
/// Deux differences par rapport a l'original, et aucune n'est un choix :
///
/// - La `Layer` produite par `layer(ref)` n'a pas d equivalent en Rust, ou
///   meme dans la source du portage. Elle est donc designee par son nom,
///   [`LayerRef`], que l appelant fournit.
/// - `Project.node` est construit par `project.ts`, qui n est pas porte dans ce
///   lot. Le noeud est donc recu en parametre plutot que construit ici.
///
/// Consequence : la closure qui captait `ref` n a pas de representation, puisque
/// rien dans le noeud ne porte la valeur. C est [`bind`] qui joue ce role, et
/// les deux pieces se completent : l une decrit le graphe, l autre produit la
/// valeur que la couche aurait fournie.
pub fn bound_node(implementation: LayerRef, project_node: AppNode) -> AppNode {
    AppNode::layer_with_dependencies(
        NODE_NAME,
        NodeTag::Location,
        implementation,
        vec![project_node],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::project_schema::VcsType;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Service projet de test : renvoie toujours la meme resolution et compte
    /// les appels, pour verifier que `bind` interroge bien le service avec le
    /// repertoire du reference.
    #[derive(Debug)]
    struct FauxProjet {
        resolu: ProjectResolved,
        appeles: Rc<RefCell<Vec<AbsolutePath>>>,
        erreur: Option<String>,
    }

    impl FauxProjet {
        /// Resolution d'un projet git, sans erreur.
        fn git() -> Self {
            FauxProjet {
                resolu: ProjectResolved {
                    id: "pro_1".to_string(),
                    directory: "/depot".to_string(),
                    vcs: Some(Vcs::git("/depot/.git".to_string())),
                },
                appeles: Rc::new(RefCell::new(Vec::new())),
                erreur: None,
            }
        }

        /// Resolution d'un projet qui n'est dans aucun depot git.
        fn sans_git() -> Self {
            FauxProjet { vcs: None, ..FauxProjet::git() }
        }

        /// Resolution qui echoue.
        fn en_erreur(message: &str) -> Self {
            FauxProjet { erreur: Some(message.to_string()), ..FauxProjet::git() }
        }
    }

    impl ProjectResolver for FauxProjet {
        type Error = String;

        fn resolve(&self, directory: &AbsolutePath) -> Result<ProjectResolved, Self::Error> {
            self.appeles.borrow_mut().push(directory.clone());
            match &self.erreur {
                Some(message) => Err(message.clone()),
                None => Ok(self.resolu.clone()),
            }
        }
    }

    /// Reference de depart, dans l'etat le plus courant.
    fn reference() -> Ref {
        Ref {
            directory: "/depot/sous/dossier".to_string(),
            workspace_id: Some("wsp_1".to_string()),
        }
    }

    /// Info attendue pour le projet de test `git`.
    fn info_de_reference() -> Info {
        Info {
            directory: "/depot/sous/dossier".to_string(),
            workspace_id: Some("wsp_1".to_string()),
            project: ProjectInfo { id: "pro_1".to_string(), directory: "/depot".to_string() },
        }
    }

    #[test]
    fn la_localisation_publie_workspace_id_avec_la_majuscule() {
        // Le test le plus important du fichier. `workspaceId` passerait la
        // compilation ET tous les tests de logique : l'erreur n'apparaitrait
        // qu'a l'echange avec le TypeScript.
        let json = serde_json::to_value(info_de_reference()).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj["workspaceID"], "wsp_1", "la cle doit porter le D majuscule");
        assert!(!obj.contains_key("workspaceId"), "la forme minuscule ne doit pas apparaitre");
        // Les deux champs de `project` sont en minuscules des deux cotes.
        assert_eq!(json["project"]["id"], "pro_1");
        assert_eq!(json["project"]["directory"], "/depot");
    }

    #[test]
    fn une_cle_snake_case_est_ignoree_a_la_lecture_sur_info() {
        // Le piege de lecture, et le plus discret des deux. `serde` ignore par
        // defaut une cle inconnue, et la source aussi (`Schema.Struct` ignore
        // les proprietes en trop) : une faute de casse sur `workspaceID` ne
        // produit donc AUCUNE erreur, elle produit une **valeur perdue**.
        //
        // C'est pour cela que le test verifie l'absence de l'erreur, et pas la
        // presence de la valeur.
        let json = r#"{"directory":"/depot","workspaceId":"wsp_1",
            "project":{"id":"pro_1","directory":"/depot"}}"#;
        let info: Info = serde_json::from_str(json).expect("la forme snake_case n'est pas refusee");
        assert_eq!(
            info.workspace_id, None,
            "la valeur est perdue sans le moindre signal : c'est exactement le piege"
        );
        // Et elle ne ressort pas : le round-trip aggrave la perte au lieu de la
        // signaler.
        let json = serde_json::to_value(&info).unwrap();
        assert!(!json.as_object().unwrap().contains_key("workspaceID"));
    }

    #[test]
    fn une_cle_snake_case_est_ignoree_a_la_lecture_sur_interface() {
        // Meme piege sur la charge utile reellement publiee par le service.
        // Le `flatten` rend la surface d'erreur identique : `vcs` est extrait,
        // tout le reste part dans `Info`.
        let json = r#"{"directory":"/depot","workspaceId":"wsp_1",
            "project":{"id":"pro_1","directory":"/depot"}}"#;
        let local: Interface =
            serde_json::from_str(json).expect("la forme snake_case n'est pas refusee");
        assert_eq!(local.info.workspace_id, None);
        assert_eq!(local.info.directory, "/depot");
    }

    #[test]
    fn la_forme_correcte_se_relit_sans_perte() {
        // Le contrepoint du test precedent, et le seul qui compte en pratique :
        // avec la vraie casse, l'aller-retour ne perd rien.
        let depart = info_de_reference();
        let json = serde_json::to_string(&depart).unwrap();
        let relue: Info = serde_json::from_str(&json).unwrap();
        assert_eq!(relue, depart);
    }

    #[test]
    fn un_workspace_absent_supprime_la_cle_a_l_encodage() {
        // `optional` retire la cle quand la valeur vaut `undefined`. `None` est
        // la traduction exacte de cette absence.
        let info = Info { workspace_id: None, ..info_de_reference() };
        let json = serde_json::to_value(&info).unwrap();
        assert!(!json.as_object().unwrap().contains_key("workspaceID"));
        // Le reste du contrat reste intact.
        assert_eq!(json["directory"], "/depot/sous/dossier");
    }

    #[test]
    fn l_encodage_ne_supprime_jamais_une_chaine_vide() {
        // Le filtre d'encodage de `optional` est `value !== undefined` : un
        // test de **nullite**. Un filtrage par veracite supprimerait aussi la
        // chaine vide, et ce serait faux. Le test verrouille la distinction.
        //
        // Attention a la lecture : `workspaceID` est un `WorkspaceID`, qui
        // exige le prefixe `wrk` et refuse donc `""`. L'etat teste n'est pas
        // representable cote TypeScript. Ce test fixe le comportement de
        // l'encodage cote Rust, il n'etablit pas une regle de la source.
        let vide = Ref { directory: "/depot".to_string(), workspace_id: Some(String::new()) };
        let local = bind(&FauxProjet::git(), &vide).unwrap();
        let json = serde_json::to_value(&local).unwrap();
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("workspaceID"), "la cle doit rester presente : {obj:?}");
        assert_eq!(obj["workspaceID"], "");
    }

    #[test]
    fn le_prefixe_du_workspace_est_wrk_et_pas_controle_par_le_type_rust() {
        // `WorkspaceID` refuse tout ce qui ne commence pas par `wrk`. Le type
        // Rust, lui, est l'alias `String` : il n'applique rien. Cette divergence
        // est assumee, elle est donc ecrite ici pour qu'on ne la decouvre pas
        // plus tard en croyant que le compilateur protege quelque chose.
        assert_eq!(WORKSPACE_ID_PREFIX, "wrk");
        let faux: WorkspaceId = "pas-un-prefixe".to_string();
        assert!(!faux.starts_with(WORKSPACE_ID_PREFIX), "le type Rust laisse passer l'invalide");
        let vrai: WorkspaceId = format!("{WORKSPACE_ID_PREFIX}_1");
        assert!(vrai.starts_with(WORKSPACE_ID_PREFIX));
    }

    #[test]
    fn un_repertoire_vide_survit_a_la_construction_de_la_localisation() {
        // Meme raison que pour `workspaceID`, mais ici le resultat est
        // **atteignable** : `AbsolutePath` est une marque sans verification,
        // donc `""` est une valeur valide du schema. Le test est donc une
        // regle de la source, pas seulement une garde-fou.
        let project = FauxProjet::git();
        let vide = Ref { directory: String::new(), workspace_id: None };
        let local = bind(&project, &vide).unwrap();
        assert_eq!(local.info.directory, "");
        assert_eq!(serde_json::to_value(&local).unwrap()["directory"], "");
    }

    #[test]
    fn le_repertoire_de_travail_vient_du_reference_et_non_de_la_resolution() {
        // Les deux repertoires sont distincts par construction. Les confondre
        // ferait perdre le repertoire de travail, qui peut etre un sous-dossier
        // d'un worktree.
        let project = FauxProjet::git();
        let local = bind(&project, &reference()).unwrap();
        assert_eq!(local.info.directory, "/depot/sous/dossier");
        assert_eq!(local.info.project.directory, "/depot");
        // Et la resolution a bien ete demandee sur le repertoire du reference.
        assert_eq!(project.appeles.borrow().as_slice(), ["/depot/sous/dossier"]);
    }

    #[test]
    fn la_localisation_du_service_est_plate_comme_l_heritage_de_la_source() {
        // `Interface extends Info` donne des membres au meme niveau : aucun
        // sous-objet `info` ne doit apparaitre dans le JSON.
        let local = bind(&FauxProjet::git(), &reference()).unwrap();
        let json = serde_json::to_value(&local).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("info"), "le flatten doit eviter la sous-structure : {obj:?}");
        assert_eq!(obj["directory"], "/depot/sous/dossier");
        assert_eq!(obj["workspaceID"], "wsp_1");
        assert_eq!(obj["project"]["id"], "pro_1");
    }

    #[test]
    fn un_projet_git_publie_le_type_et_le_depot() {
        let local = bind(&FauxProjet::git(), &reference()).unwrap();
        let json = serde_json::to_value(&local).unwrap();
        assert_eq!(json["vcs"]["type"], "git");
        assert_eq!(json["vcs"]["store"], "/depot/.git");
    }

    #[test]
    fn un_projet_hors_git_ne_publie_pas_la_cle_vcs() {
        // `vcs: resolved.vcs` recopie `undefined` tel quel. Cote Rust c'est
        // `None`, et la cle disparait, ce qui est le meme resultat observable.
        let local = bind(&FauxProjet::sans_git(), &reference()).unwrap();
        assert_eq!(local.vcs, None);
        let json = serde_json::to_value(&local).unwrap();
        assert!(!json.as_object().unwrap().contains_key("vcs"));
    }

    #[test]
    fn le_type_vcs_reste_toujours_git() {
        // Le service projet ne peut produire qu'une variante. La conversion de
        // `vcs` en chaine doit donc toujours donner "git", comme le test
        // `location.vcs?.type === "git"` de la source.
        let local = bind(&FauxProjet::git(), &reference()).unwrap();
        let vcs = local.vcs.expect("le projet de test a un depot");
        assert_eq!(vcs.r#type, VcsType::Git);
        assert_eq!(serde_json::to_string(&vcs.r#type).unwrap(), "\"git\"");
    }

    #[test]
    fn une_erreur_de_resolution_est_remontee_telle_quelle() {
        // `yield* project.resolve(...)` propage l'erreur du canal, sans la
        // transformer ni l'envelopper.
        let projet = FauxProjet::en_erreur("depot illisible");
        assert_eq!(bind(&projet, &reference()).unwrap_err(), "depot illisible");
    }

    #[test]
    fn le_reference_sans_workspace_donne_une_localisation_sans_workspace() {
        let sans_workspace = Ref { directory: "/depot".to_string(), workspace_id: None };
        let local = bind(&FauxProjet::git(), &sans_workspace).unwrap();
        assert_eq!(local.info.workspace_id, None);
        let json = serde_json::to_value(&local).unwrap();
        assert!(!json.as_object().unwrap().contains_key("workspaceID"));
    }

    #[test]
    fn la_reponse_emballe_la_localisation_et_les_donnees() {
        // `Schema.Struct({ location: Info, data })` : deux cles, a ce niveau.
        let enveloppe = response(info_de_reference(), vec!["une", "deux"]);
        let json = serde_json::to_value(&enveloppe).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 2, "la reponse ne doit avoir que deux cles : {obj:?}");
        assert_eq!(json["location"]["workspaceID"], "wsp_1");
        assert_eq!(json["data"][1], "deux");
    }

    #[test]
    fn la_reponse_se_relit_depuis_le_json() {
        // Sens inverse : ce que le TypeScript produit doit se relire tel quel.
        let json = r#"{"location":{"directory":"/depot","workspaceID":"wsp_1",
            "project":{"id":"pro_1","directory":"/depot"}},"data":42}"#;
        let lue: Response<i64> = serde_json::from_str(json).unwrap();
        assert_eq!(lue.data, 42);
        assert_eq!(lue.location.workspace_id.as_deref(), Some("wsp_1"));
        assert_eq!(lue.location.project.directory, "/depot");
    }

    #[test]
    fn une_localisation_lue_depuis_le_json_reprend_le_gestionnaire_de_versions() {
        let json = r#"{"directory":"/depot/sous/dossier","workspaceID":"wsp_1",
            "project":{"id":"pro_1","directory":"/depot"},
            "vcs":{"type":"git","store":"/depot/.git"}}"#;
        let local: Interface = serde_json::from_str(json).unwrap();
        assert_eq!(local.info.directory, "/depot/sous/dossier");
        assert_eq!(local.vcs.expect("vcs present").store, "/depot/.git");
    }

    #[test]
    fn une_localisation_sans_vcs_se_relit_avec_une_absence() {
        // La cle absente donne `None`, et `vcs` n'est pas devenu une valeur
        // par defaut : `Schema.Literal("git")` n'a pas de defaut non plus.
        let json = r#"{"directory":"/depot","project":{"id":"pro_1","directory":"/depot"}}"#;
        let local: Interface = serde_json::from_str(json).unwrap();
        assert_eq!(local.vcs, None);
        assert_eq!(local.info.workspace_id, None);
    }

    #[test]
    fn le_nom_du_noeud_est_la_cle_du_service() {
        // `LayerNode.make` et `LayerNode.unbound` positionnent tous deux
        // `name: service.key`. Le nom n'est donc pas choisi librement.
        assert_eq!(SERVICE_TAG, "@opencode/Location");
        assert_eq!(NODE_NAME, SERVICE_TAG);
    }

    #[test]
    fn les_tags_de_ce_fichier_sont_ceux_du_porteur() {
        // `crate::swarm::effect_app_node_builder` porte deja `NodeTag`. Les
        // chaines de ce fichier et l'enum d ailleurs decrivent le meme contrat :
        // ce test les solidarise mecaniquement, pour qu'une evolution de l'un
        // ne passe pas inapercue de l'autre.
        assert_eq!(TAG_LOCATION, NodeTag::Location.as_str());
        assert_eq!(TAG_GLOBAL, NodeTag::Global.as_str());
    }

    #[test]
    fn le_noeud_non_lie_demande_le_service_sans_fournir_d_implementation() {
        // `LayerNode.unbound(Service, tags.values.location)`.
        let noeud = node();
        assert_eq!(noeud.name(), SERVICE_TAG);
        assert_eq!(noeud.tag(), Some(NodeTag::Location));
        assert!(noeud.implementation().is_none());
        assert!(noeud.dependencies().is_empty());
    }

    #[test]
    fn le_noeud_lie_depend_du_noeud_projet_et_rien_d_autre() {
        // `deps: [Project.node]` : une dependance, et une seule. Le nom de
        // cette dependance est la cle du service projet, que `project.ts` ecrit
        // ainsi.
        let projet = AppNode::unbound(PROJECT_SERVICE_TAG, NodeTag::Global);
        let noeud = bound_node(LayerRef::new("Location.bound"), projet);
        assert_eq!(noeud.name(), SERVICE_TAG);
        assert_eq!(noeud.tag(), Some(NodeTag::Location));
        assert_eq!(noeud.implementation().map(LayerRef::name), Some("Location.bound"));
        assert_eq!(noeud.dependencies().len(), 1);
        assert_eq!(noeud.dependencies()[0].name(), PROJECT_SERVICE_TAG);
    }

    #[test]
    fn le_noeud_de_localisation_a_le_droit_de_dependre_du_noeud_global() {
        // `location: ["global"]` autorise exactement cette dependance, et c'est
        // celle que `boundNode` declare avec `Project.node`, qui est lui-meme
        // un noeud `global` (`makeGlobalNode` dans `project.ts`).
        assert_eq!(TAGS_LOCATION_DEPENDENCIES, [NodeTag::Global.as_str()]);
        assert!(TAGS_LOCATION_DEPENDENCIES.contains(&PROJECT_SERVICE_TAG_HINT));
    }

    #[test]
    fn un_noeud_global_ne_depend_d_aucun_tag() {
        // `global: []` : liste vide, donc aucun tag autorise. Ce n'est pas un
        // oubli de la source, et une liste d'un element serait faux.
        assert_eq!(TAGS_GLOBAL_DEPENDENCIES.len(), 0);
        assert!(!TAGS_GLOBAL_DEPENDENCIES.contains(&TAG_LOCATION));
    }

    /// Rappel : `Project.node` est construit par `makeGlobalNode`, donc il
    /// porte le tag `global`, et non `location`.
    const PROJECT_SERVICE_TAG_HINT: &str = TAG_GLOBAL;
}
