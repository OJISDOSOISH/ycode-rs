//! Portage de `packages/core/src/effect/app-node-platform.ts`.
//!
//! La source tient en dix-huit lignes et ne contient aucun calcul :
//!
//! ```ts
//! export const filesystem = makeGlobalNode({ service: FileSystem.FileSystem, layer: NodeFileSystem.layer, deps: [] })
//! export const path = makeGlobalNode({ service: Path.Path, layer: NodePath.layer, deps: [] })
//! export const httpClient = makeGlobalNode({ service: HttpClient.HttpClient, layer: FetchHttpClient.layer, deps: [] })
//! export const requestExecutor = makeGlobalNode({ service: RequestExecutor.Service, layer: RequestExecutor.layer, deps: [httpClient] })
//! export const llmClient = makeGlobalNode({ service: LLMClient.Service, layer: LLMClient.layer, deps: [requestExecutor] })
//! export * as LayerNodePlatform from "./app-node-platform"
//! ```
//!
//! Ces cinq lignes forment une **table de cablage** : elles disent, pour cinq
//! services, quelle implementation de couche fournir et quelles autres couches
//! doivent etre fournies avant elle. Aucun graphe n est construit ici. La
//! construction effective est faite plus loin, par `LayerNode.compile`, qui
//! transforme ces noeuds en une couche `Layer`. Le `memoMap` n entre pas non
//! plus dans ce fichier : il est fourni a la construction du runtime, dans
//! `effect/runtime.ts`.
//!
//! Une contrainte merite d etre notee. Les cinq noeuds portent tous l etiquette
//! `global`, et `LayerNode` refuse une dependance dont l etiquette ne fait pas
//! partie des etiquettes admises pour le noeud qui la declare. Or la
//! configuration de `app-node.ts` est `{ location: ["global"], global: [] }` :
//! la liste admise pour le groupe `global` est vide, donc une dependance
//! declaree n est acceptee que si elle porte elle aussi l etiquette `global`.
//! C est bien le cas des deux dependances declarees ici, `httpClient` et
//! `requestExecutor`. Cette regle est un controle de type, sans execution, elle
//! n est donc pas reproduite en code : elle est rappelee ici pour que la
//! contrainte reste visible.
//!
//! Ce qui est porte ici :
//!
//! 1. la liste des cinq services concernes, dans l ordre de la source, avec
//!    pour chacun le nom de la couche d implementation ;
//! 2. la liste des dependances declaree par chaque noeud, qui est la seule
//!    information reellement portee par ce fichier ;
//! 3. le nom de chaque noeud, qui est la cle de son service, et les operations
//!    simples que le code appelant fait sur ces noms : recherche par nom,
//!    dependances directes, dependants, racines, feuilles, ordre de
//!    construction calcule sur les seules dependances declarees.
//!
//! Ce qui n est **pas** porte, et qui n a pas d equivalent en Rust :
//!
//! - `Context.Tag` et `Context.Service` de la bibliotheque `effect` : un service
//!   y est une reference d objet vivante, pas une donnee. Ici chaque service est
//!   un simple identite, `ServiceId` ;
//! - `Layer` : `NodeFileSystem.layer`, `NodePath.layer`, `FetchHttpClient.layer`,
//!   `RequestExecutor.layer` et `LLMClient.layer` sont des valeurs opaques de la
//!   bibliotheque `effect`. Elles sont notees ici **par leur nom**, comme des
//!   chaines, et aucune implementation de systeme de fichiers, de chemin, de
//!   client HTTP ou de client LLM n est simulee ;
//! - le `Scope` d Effect et son compteur de references : le portage de
//!   `effect/memo-map.ts` le signale deja comme non traduisible ;
//! - la compilation du graphe (`LayerNode.compile`, `LayerNode.hoist`) : elle vit
//!   dans `layer-node.ts`, pas dans ce fichier. La fonction `build_order` de ce
//!   module est un **ordre declaratif** calcule sur la liste des dependances,
//!   et non une compilation de couches ;
//! - le type `Node` complet de `layer-node.ts` est ici restreint : seules les
//!   trois especes `Layer`, `Unbound` et `Group` sont enumerees, et seules les
//!   instances de type `PlatformNode` sont decrites, sans les types de sortie et
//!   d erreur par symbole.
//!
//! Derniere ligne de la source :
//!
//! ```ts
//! export * as LayerNodePlatform from "./app-node-platform"
//! ```
//!
//! Cet auto reexport est un renommage de namespace vers le module lui-meme. Il
//! ne cree rien et n est donc pas traduit : en Rust, le nom du module fournit
//! deja ce namespace.

use std::fmt;

/// Especes de noeud que la bibliotheque `effect` distingue, reprises de
/// `readonly kind: "layer" | "unbound" | "group"` dans `layer-node.ts`.
///
/// Seul `Layer` est construit par le module porte : les cinq noeuds de la
/// source sortent tous de `makeGlobalNode`, qui appelle `LayerNode.make`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    /// Noeud dont l implementation est une couche.
    Layer,
    /// Noeud dont l implementation doit etre fournie par l appelant.
    Unbound,
    /// Noeud qui ne sert qu a regrouper d autres noeuds.
    Group,
}

/// Etiquette des noeuds de ce module. La source la pose par
/// `tags = LayerNode.tags({ location: ["global"], global: [] })` suivi de
/// `makeGlobalNode = tags.make("global")`.
pub const GLOBAL_TAG: &str = "global";

/// Liste de dependances vide, partagee par les trois services qui ne dependent
/// d aucun autre. Les noeuds du module sont statiques, donc cette liste l est
/// aussi.
const AUCUNE_DEPENDANCE: &[ServiceId] = &[];

/// Les cinq services que ce module branche sur une implementation de couche.
///
/// L ordre de declaration est celui de la source. Il sert de tri
/// deterministe partout ou une liste de services doit etre rendue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ServiceId {
    /// `FileSystem.FileSystem`, realise par `NodeFileSystem.layer`.
    FileSystem,
    /// `Path.Path`, realise par `NodePath.layer`.
    Path,
    /// `HttpClient.HttpClient`, realise par `FetchHttpClient.layer`.
    HttpClient,
    /// `RequestExecutor.Service`, realise par `RequestExecutor.layer`.
    RequestExecutor,
    /// `LLMClient.Service`, realise par `LLMClient.layer`.
    LlmClient,
}

impl ServiceId {
    /// Les cinq services, dans l ordre de declaration de la source.
    pub const ALL: [ServiceId; 5] = [
        ServiceId::FileSystem,
        ServiceId::Path,
        ServiceId::HttpClient,
        ServiceId::RequestExecutor,
        ServiceId::LlmClient,
    ];

    /// La cle du service, cest a dire le nom du noeud correspondant.
    ///
    /// La source calcule ce nom avec `input.service.key` dans `LayerNode.make`.
    ///
    /// Les deux dernieres valeurs sont litteralement les chaines passees a
    /// `Context.Service` dans le paquet `llm`, verifiees dans la source :
    /// `class Service extends Context.Service<Service, Interface>()(
    /// "@opencode/LLM/RequestExecutor")` et `"@opencode/LLMClient"`.
    ///
    /// Les trois premieres sont deduites, pas verifiees : la bibliotheque
    /// `effect` nest pas installee dans le depot, et on ne peut donc pas lire
    /// la declaration de `FileSystem`, `Path` et `HttpClient`. On applique la
    /// regle usuelle de `effect`, ou une cle non explicitement fournie prend le
    /// nom du symbole declare, ce qui redonne exactement les trois noms
    /// importes par la source. Si une relecture peut ouvrir `node_modules`,
    /// ces trois chaines sont la premiere chose a verifier.
    pub const fn key(self) -> &'static str {
        match self {
            ServiceId::FileSystem => "FileSystem",
            ServiceId::Path => "Path",
            ServiceId::HttpClient => "HttpClient",
            ServiceId::RequestExecutor => "@opencode/LLM/RequestExecutor",
            ServiceId::LlmClient => "@opencode/LLMClient",
        }
    }

    /// Le service correspondant a une cle de service, ou `None` si la cle est
    /// inconnue. C est l operation inverse de `key`, et c est ainsi que le
    /// code appelant retrouve un noeud a partir de son nom.
    pub fn from_key(key: &str) -> Option<Self> {
        ServiceId::ALL.iter().copied().find(|service| service.key() == key)
    }

    /// Le nom de la couche qui realise le service. Ce n est pas une valeur de
    /// couche, seulement son nom : la couche elle-meme est un objet opaque de la
    /// bibliotheque `effect`, sans traduction ici.
    pub const fn implementation(self) -> &'static str {
        match self {
            ServiceId::FileSystem => "NodeFileSystem.layer",
            ServiceId::Path => "NodePath.layer",
            ServiceId::HttpClient => "FetchHttpClient.layer",
            ServiceId::RequestExecutor => "RequestExecutor.layer",
            ServiceId::LlmClient => "LLMClient.layer",
        }
    }

    /// Les services dont la couche doit etre fournie avant celle du service.
    ///
    /// Ce sont les `deps` de la source : vides pour `filesystem`, `path` et
    /// `httpClient`, `[httpClient]` pour `requestExecutor`, `[requestExecutor]`
    /// pour `llmClient`.
    pub const fn dependencies(self) -> &'static [ServiceId] {
        match self {
            ServiceId::FileSystem | ServiceId::Path | ServiceId::HttpClient => AUCUNE_DEPENDANCE,
            ServiceId::RequestExecutor => &[ServiceId::HttpClient],
            ServiceId::LlmClient => &[ServiceId::RequestExecutor],
        }
    }

    /// Le nom sous lequel la source exporte le noeud de ce service. Ces noms
    /// sont en `camelCase` dans la source ; ils ne sont utilises que comme
    /// etiquettes, aucune serialisation ne depend d eux.
    pub const fn export_name(self) -> &'static str {
        match self {
            ServiceId::FileSystem => "filesystem",
            ServiceId::Path => "path",
            ServiceId::HttpClient => "httpClient",
            ServiceId::RequestExecutor => "requestExecutor",
            ServiceId::LlmClient => "llmClient",
        }
    }
}

/// Un noeud de la table de cablage, equivalent du `Node` produit par
/// `makeGlobalNode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlatformNode {
    /// Toujours `NodeKind::Layer` pour les noeuds fabriques par ce module.
    pub kind: NodeKind,
    /// Le nom du noeud, egal a la cle de son service.
    pub name: &'static str,
    /// Le service fourni par le noeud.
    pub service: ServiceId,
    /// Le nom de la couche qui realise le service.
    pub implementation: &'static str,
    /// Les services a fournir avant celui-ci.
    pub dependencies: &'static [ServiceId],
    /// Toujours `GLOBAL_TAG` pour les noeuds fabriques par ce module.
    pub tag: &'static str,
}

impl PlatformNode {
    /// Construit un noeud de couche a partir de son service, en reprenant la
    /// cle du service comme nom et les dependances declarees pour ce service.
    pub const fn of(service: ServiceId) -> Self {
        Self {
            kind: NodeKind::Layer,
            name: service.key(),
            service,
            implementation: service.implementation(),
            dependencies: service.dependencies(),
            tag: GLOBAL_TAG,
        }
    }

    /// `true` si le noeud n a aucune dependance declaree.
    pub const fn is_root(&self) -> bool {
        self.dependencies.is_empty()
    }
}

/// `export const filesystem` : service `FileSystem`, sans dependance.
pub const FILESYSTEM: PlatformNode = PlatformNode::of(ServiceId::FileSystem);

/// `export const path` : service `Path`, sans dependance.
pub const PATH: PlatformNode = PlatformNode::of(ServiceId::Path);

/// `export const httpClient` : service `HttpClient`, sans dependance.
pub const HTTP_CLIENT: PlatformNode = PlatformNode::of(ServiceId::HttpClient);

/// `export const requestExecutor` : service `RequestExecutor`, qui depend de
/// `httpClient`.
pub const REQUEST_EXECUTOR: PlatformNode = PlatformNode::of(ServiceId::RequestExecutor);

/// `export const llmClient` : service `LLMClient`, qui depend de
/// `requestExecutor`.
pub const LLM_CLIENT: PlatformNode = PlatformNode::of(ServiceId::LlmClient);

/// Les cinq noeuds du module, dans l ordre de declaration de la source.
pub const ALL: [PlatformNode; 5] =
    [FILESYSTEM, PATH, HTTP_CLIENT, REQUEST_EXECUTOR, LLM_CLIENT];

/// Les cinq noeuds du module.
///
/// L auto reexport de la source, `export * as LayerNodePlatform`, ne produit
/// rien de plus : en Rust le module porte deja le nom de ses membres.
pub fn all() -> &'static [PlatformNode] {
    &ALL
}

/// Le noeud dont le nom est la cle donnee, ou `None` si aucun noeud ne porte
/// ce nom. Une cle vide ne correspond a aucun service.
pub fn find_by_name(name: &str) -> Option<PlatformNode> {
    ALL.iter().copied().find(|node| node.name == name)
}

/// Les services dont le noeud declare le service donne parmi ses dependances,
/// dans l ordre de declaration de la source.
pub fn dependents_of(service: ServiceId) -> Vec<ServiceId> {
    ServiceId::ALL
        .iter()
        .copied()
        .filter(|candidat| candidat.dependencies().contains(&service))
        .collect()
}

/// Les services sans dependance declaree, dans l ordre de declaration.
pub fn roots() -> Vec<ServiceId> {
    ServiceId::ALL
        .iter()
        .copied()
        .filter(|service| service.dependencies().is_empty())
        .collect()
}

/// Les services dont aucun autre service ne depend, dans l ordre de
/// declaration.
pub fn leaves() -> Vec<ServiceId> {
    ServiceId::ALL
        .iter()
        .copied()
        .filter(|service| {
            !ServiceId::ALL
                .iter()
                .any(|autre| autre.dependencies().contains(service))
        })
        .collect()
}

/// Erreur renvoyee quand la relation de dependance contient un cercle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cycle {
    /// Les services qui n ont pas pu etre places, parce que chacun depend,
    /// directement ou indirectement, d un autre de la liste.
    pub nodes: Vec<ServiceId>,
}

impl fmt::Display for Cycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let noms: Vec<&str> = self.nodes.iter().map(|service| service.key()).collect();
        write!(f, "cycle de dependance : {}", noms.join(" -> "))
    }
}

/// L ordre de construction des cinq noeuds du module, calcule sur les seules
/// dependances declarees.
///
/// Le resultat est deterministe et ne depend pas de l ordre dans lequel
/// l appelant presente les services : a chaque etape, on prend parmi les
/// services restants le premier, dans l ordre de declaration de la source, dont
/// toutes les dependances sont deja placees.
pub fn build_order() -> Result<Vec<ServiceId>, Cycle> {
    build_order_from(&ServiceId::ALL, |service| {
        service.dependencies().to_vec()
    })
}

/// Le meme calcul que `build_order`, sur un ensemble et une relation de
/// dependance choisis par l appelant.
///
/// Les candidats sont examines dans l ordre de declaration de `ServiceId`, et
/// non dans l ordre de `entree` : deux appels qui presentent le meme ensemble
/// dans des ordres differents rendent donc le meme resultat.
///
/// Une dependance qui ne figure pas dans `entree` est consideree comme deja
/// fournie : elle ne peut pas etre placee par cet ordre, et son absence ne
/// provoque donc pas d erreur. Une dependance qui se reference elle-meme, ou un
/// cercle entre services presents dans `entree`, donne `Err`.
pub fn build_order_from(
    entree: &[ServiceId],
    mut deps_of: impl FnMut(ServiceId) -> Vec<ServiceId>,
) -> Result<Vec<ServiceId>, Cycle> {
    let mut reste: Vec<ServiceId> = entree.to_vec();
    let mut ordre: Vec<ServiceId> = Vec::with_capacity(entree.len());

    while !reste.is_empty() {
        let suivant = ServiceId::ALL
            .iter()
            .copied()
            .filter(|candidat| reste.contains(candidat))
            .find(|candidat| {
                deps_of(*candidat)
                    .iter()
                    .all(|dependance| !reste.contains(&dependance))
            });

        match suivant {
            Some(noeud) => {
                reste.retain(|service| *service != noeud);
                ordre.push(noeud);
            }
            None => return Err(Cycle { nodes: reste }),
        }
    }

    Ok(ordre)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_noeud_sans_dependance_na_dependance_daucune() {
        assert!(FILESYSTEM.dependencies.is_empty());
        assert!(PATH.dependencies.is_empty());
        assert!(HTTP_CLIENT.dependencies.is_empty());
        assert!(FILESYSTEM.is_root());
        assert!(PATH.is_root());
        assert!(HTTP_CLIENT.is_root());
    }

    #[test]
    fn l_executeur_de_requetes_depend_du_client_http() {
        assert_eq!(REQUEST_EXECUTOR.dependencies, &[ServiceId::HttpClient]);
    }

    #[test]
    fn le_client_llm_depend_de_l_executeur_de_requetes() {
        assert_eq!(LLM_CLIENT.dependencies, &[ServiceId::RequestExecutor]);
    }

    #[test]
    fn le_nom_du_noeud_est_la_cle_de_son_service() {
        for node in all() {
            assert_eq!(node.name, node.service.key());
        }
        assert_eq!(FILESYSTEM.name, "FileSystem");
        assert_eq!(PATH.name, "Path");
        assert_eq!(HTTP_CLIENT.name, "HttpClient");
        assert_eq!(REQUEST_EXECUTOR.name, "@opencode/LLM/RequestExecutor");
        assert_eq!(LLM_CLIENT.name, "@opencode/LLMClient");
    }

    #[test]
    fn une_cle_inconnue_ne_donne_aucun_noeud() {
        assert!(find_by_name("HttpClients").is_none());
        assert!(find_by_name("filesystem").is_none());
        assert!(find_by_name("").is_none());
        assert!(ServiceId::from_key("").is_none());
    }

    #[test]
    fn une_cle_connue_redonne_le_noeud_du_service() {
        for node in all() {
            let retrouve = find_by_name(node.name).expect("le noeud doit etre retrouve");

            assert_eq!(retrouve, *node);
            assert_eq!(
                ServiceId::from_key(node.name).expect("service attendu"),
                node.service
            );
        }
    }

    #[test]
    fn les_cinq_noeuds_du_module_sont_cinq_services_distincts() {
        let vus: Vec<ServiceId> = all().iter().map(|node| node.service).collect();

        assert_eq!(vus.len(), 5);
        assert_eq!(vus, ServiceId::ALL.to_vec());

        for (index, service) in ServiceId::ALL.iter().enumerate() {
            let autres = &vus[..index];
            assert!(!autres.contains(service));
        }
    }

    #[test]
    fn tous_les_noeuds_sont_des_couches_marquees_global() {
        for node in all() {
            assert_eq!(node.kind, NodeKind::Layer);
            assert_eq!(node.tag, "global");
        }
    }

    #[test]
    fn chaque_service_a_le_nom_de_sa_couche_de_la_source() {
        assert_eq!(FILESYSTEM.implementation, "NodeFileSystem.layer");
        assert_eq!(PATH.implementation, "NodePath.layer");
        assert_eq!(HTTP_CLIENT.implementation, "FetchHttpClient.layer");
        assert_eq!(REQUEST_EXECUTOR.implementation, "RequestExecutor.layer");
        assert_eq!(LLM_CLIENT.implementation, "LLMClient.layer");
    }

    #[test]
    fn les_noms_exportes_sont_ceux_de_la_source() {
        let noms: Vec<&str> = ServiceId::ALL
            .iter()
            .map(|service| service.export_name())
            .collect();

        assert_eq!(
            noms,
            vec!["filesystem", "path", "httpClient", "requestExecutor", "llmClient"]
        );
    }

    #[test]
    fn un_ensemble_vide_donne_un_ordre_vide() {
        let ordre = build_order_from(&[], |_| Vec::new()).expect("aucun cycle possible");

        assert!(ordre.is_empty());
    }

    #[test]
    fn un_ensemble_reduit_a_un_noeud_donne_ce_noeud_seul() {
        let ordre = build_order_from(&[ServiceId::LlmClient], |service| {
            service.dependencies().to_vec()
        })
        .expect("un noeud unique ne peut pas former de cercle");

        assert_eq!(ordre, vec![ServiceId::LlmClient]);
    }

    #[test]
    fn une_dependance_externe_est_consideree_comme_deja_fournie() {
        let ordre = build_order_from(&[ServiceId::LlmClient], |_| Vec::new())
            .expect("aucune dependance a attendre");

        assert_eq!(ordre, vec![ServiceId::LlmClient]);
    }

    #[test]
    fn l_ordre_de_construction_place_une_dependance_avant_son_utilisateur() {
        let ordre = build_order().expect("la table de la source est sans cercle");

        let http = position(&ordre, ServiceId::HttpClient);
        let requetes = position(&ordre, ServiceId::RequestExecutor);
        let llm = position(&ordre, ServiceId::LlmClient);

        assert!(http < requetes);
        assert!(requetes < llm);
    }

    #[test]
    fn l_ordre_de_construction_suit_l_ordre_de_declaration_pour_les_etapes_libres() {
        let ordre = build_order().expect("la table de la source est sans cercle");

        // Les trois premiers services sont sans dependance : ils sont donc
        // places dans l ordre de declaration, avant la chaine
        // HttpClient -> RequestExecutor -> LlmClient.
        assert_eq!(
            ordre,
            vec![
                ServiceId::FileSystem,
                ServiceId::Path,
                ServiceId::HttpClient,
                ServiceId::RequestExecutor,
                ServiceId::LlmClient,
            ]
        );
    }

    #[test]
    fn l_ordre_de_construction_est_le_meme_qu_on_l_appelle_deux_fois() {
        assert_eq!(
            build_order().expect("ordre attendu"),
            build_order().expect("ordre attendu")
        );
    }

    #[test]
    fn l_ordre_ne_depend_pas_de_l_ordre_de_l_entree() {
        let direct = build_order_from(&ServiceId::ALL, |service| {
            service.dependencies().to_vec()
        })
        .expect("la table de la source est sans cercle");
        let inverse = build_order_from(
            &[
                ServiceId::LlmClient,
                ServiceId::RequestExecutor,
                ServiceId::HttpClient,
                ServiceId::Path,
                ServiceId::FileSystem,
            ],
            |service| service.dependencies().to_vec(),
        )
        .expect("la table de la source est sans cercle");

        assert_eq!(direct, inverse);
        assert_eq!(direct, build_order().expect("ordre attendu"));
    }

    #[test]
    fn un_cercle_de_dependances_est_signale_au_lieu_de_boucler() {
        let erreur = build_order_from(&[ServiceId::HttpClient, ServiceId::LlmClient], |service| {
            match service {
                ServiceId::HttpClient => vec![ServiceId::LlmClient],
                ServiceId::LlmClient => vec![ServiceId::HttpClient],
                _ => Vec::new(),
            }
        })
        .expect_err("un cercle doit etre signale");

        assert_eq!(
            erreur.nodes,
            vec![ServiceId::HttpClient, ServiceId::LlmClient]
        );
        assert_eq!(
            erreur.to_string(),
            "cycle de dependance : HttpClient -> @opencode/LLMClient"
        );
    }

    #[test]
    fn un_noeud_qui_depend_de_lui_meme_est_signe_comme_un_cercle() {
        let erreur = build_order_from(&[ServiceId::Path], |_| vec![ServiceId::Path])
            .expect_err("une auto dependance doit etre signalee");

        assert_eq!(erreur.nodes, vec![ServiceId::Path]);
    }

    #[test]
    fn les_racines_sont_les_services_sans_dependance() {
        assert_eq!(
            roots(),
            vec![ServiceId::FileSystem, ServiceId::Path, ServiceId::HttpClient]
        );
    }

    #[test]
    fn les_feuilles_sont_les_services_que_personne_n_utilise() {
        assert_eq!(
            leaves(),
            vec![ServiceId::FileSystem, ServiceId::Path, ServiceId::LlmClient]
        );
    }

    #[test]
    fn les_dependants_du_client_http_sont_l_executeur_de_requetes() {
        assert_eq!(dependents_of(ServiceId::HttpClient), vec![ServiceId::RequestExecutor]);
    }

    #[test]
    fn un_service_terminal_na_dependant_avec_personne() {
        assert!(dependents_of(ServiceId::LlmClient).is_empty());
    }

    #[test]
    fn la_dependance_et_le_dependant_sont_symetriques() {
        for service in ServiceId::ALL {
            for dependance in service.dependencies() {
                assert!(dependents_of(*dependance).contains(&service));
            }
        }
    }

    fn position(ordre: &[ServiceId], service: ServiceId) -> usize {
        ordre
            .iter()
            .position(|element| *element == service)
            .expect("le service doit figurer dans l ordre")
    }
}
