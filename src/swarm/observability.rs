//! Portage de `packages/core/src/observability.ts`.
//!
//! ## Ce que la source contient
//!
//! Vingt-quatre lignes, dont la premiere est
//! `export * as Observability from "./observability"`, c est a dire un reexport
//! de l espace de noms du module sur LUI-MEME. C est du code mort, comme la
//! derniere ligne de `config/command.ts` ou de `util/wildcard.ts` : il n a pas
//! d equivalent Rust et il n est donc pas retranscrit.
//!
//! Le reste du fichier est du CABLAGE de couches Effect, sans aucune donnee :
//!
//! ```ts
//! export const layer = Layer.unwrap(Effect.gen(function* () {
//!   const logs = Logger.layer([...Logging.loggers(), ...Otlp.loggers()], { mergeWithExisting: false }).pipe(
//!     Layer.provide(NodeFileSystem.layer),
//!     Layer.provide(OtlpSerialization.layerJson),
//!     Layer.provide(FetchHttpClient.layer),
//!     Layer.orDie,
//!     Layer.merge(Layer.succeed(References.MinimumLogLevel, Logging.minimumLogLevel())),
//!   )
//!   return Layer.merge(logs, yield* Effect.promise(Otlp.tracingLayer))
//! }))
//!
//! export const node = LayerNode.make({ name: "observability", layer, deps: [] })
//! ```
//!
//! ## Les deux exports tiennent dans un seul descripteur
//!
//! La source exporte deux choses : `layer`, la couche construite par l effet, et
//! `node`, le descripteur qui l embarque avec un nom et une liste de
//! dependances. Les deux disent la MEME chose a deux niveaux de detail, donc le
//! portage n a pas deux types : [`Node`] decrit la couche, et [`layer`] comme
//! [`node`] sont deux points d entree vers le meme calcul, l un avec des
//! options explicites, l autre avec les variables du processus.
//!
//! ## Ce qui est reporte, et comment
//!
//! Le graphe de couches d Effect (le `Scope`, l ordre de fermeture, les
//! `Reference`) n existe pas en Rust et n est pas simule, comme dans
//! `effect_memo_map.rs`. Ce qui EST reporte, c est la composition elle-meme,
//! vue comme une DONNEE : `Node` decrit ce que la couche installe, et les
//! fonctions qui le remplissent lisent les trois variables d environnement qui
//! le decident. Un test peut donc verifier l effet de `OPENCODE_PRINT_LOGS`,
//! `OTEL_EXPORTER_OTLP_ENDPOINT` et `OPENCODE_LOG_LEVEL`, ce qui est la seule
//! partie observable de ce fichier.
//!
//! ## Le detail qui casse tout si on l ignore
//!
//! Les deux listes de journaliers ne sont pas filtrees de la meme facon :
//!
//! - `Otlp.loggers()` teste `if (!endpoint) return []`, un test de VERACITE :
//!   une chaine VIDE y est traitee comme absente ;
//! - `Logging.loggers()` teste `process.env.OPENCODE_PRINT_LOGS === "1"`, une
//!   egalite stricte : une chaine d espaces n y est pas egale a "1".
//!
//! Traduire les deux par le meme `Option::is_none` (ou par un simple `.map()`)
//! ferait survivre une chaine vide la ou la source la jette.
//! Chaque test de ce fichier existe d ailleurs sur un de ces deux cas.
//!
//! ## Les trois nuances qu il faut savoir avant de relire
//!
//! 1. `Layer.unwrap(Effect.gen(...))` et `yield* Effect.promise(...)` ne sont
//!    pas simules, conformement a la consigne : le `Scope` d Effect, l ordre de
//!    fermeture des couches et la resolution asynchrone de `tracingLayer`
//!    n existent pas en Rust. Ce qui reste du `Effect.promise` est son issue,
//!    c est a dire le fait que la couche de tracage existe ou non : c est le
//!    booleen `tracingLayer` de [`Node`].
//!
//! 2. `value in levels` en JavaScript parcourt la CHAINE DE PROTOTYPES, pas
//!    seulement les cles de l objet litteral. `OPENCODE_LOG_LEVEL=toString`
//!    passerait donc le test et rendrait `Object.prototype.toString`, c est a
//!    dire une fonction, installee comme niveau minimum. Ce portage ne
//!    reproduit pas cette fuite : la table est une table, et un nom absent
//!    donne `Info`. C est la seule divergence VOLONNAIRE du fichier. Elle est
//!    assumee parce que dans la source ce cas produit une valeur INVALIDE :
//!    aucun des quatre niveaux ne porte un nom qui soit aussi membre de
//!    `Object.prototype`, donc le cas ne peut pas survenir pour une
//!    configuration reelle.
//!
//! 3. Les trois variables d environnement ne sont pas lues au meme moment dans
//!    la source. `otlp.ts` fait `const endpoint = Flag.OTEL_EXPORTER_OTLP_ENDPOINT`
//!    au chargement du module : le point de collecte est FIGE a l import.
//!    `Logging.loggers()` et `Logging.minimumLogLevel()` relisent `process.env`
//!    a chaque appel, donc au moment de la construction de la couche.
//!    [`Options::from_env`] lit les trois au meme instant, ce qui suppose que
//!    rien ne modifie l environnement entre l import et la construction.
//!
//! ## Ce qui n est PAS ici, et pourquoi
//!
//! - Le contenu des trois journaliers : le formateur, le chemin du fichier et
//!   l identifiant d execution sont dans `observability/logging.ts`. L
//!   identifiant, lui, est deja porte par `observability_shared.rs` (`run_id`)
//!   et n est donc pas recalcule ici.
//! - Le payload OTLP, les entetes et les attributs de ressource sont dans
//!   `observability/otlp.ts`. Seul le fait que la couche en ajoute un
//!   lorsqu un point de collecte existe est reporte, car c est la decision
//!   prise par CE fichier.
//! - La table des niveaux (`DEBUG`/`INFO`/`WARN`/`ERROR`) vient de
//!   `logging.ts`, mais la couche l installe : elle est donc reproduite ici
//!   pour que le noeud soit complet. C est le point de deduplication a
//!   surveiller quand `logging.ts` sera porte.

use std::env;

use serde::{Deserialize, Serialize};

/// Nom donne a `LayerNode.make`. C est aussi le nom de la couche dans
/// l arborescence de l application.
pub const NODE_NAME: &str = "observability";

/// Le champ `kind` que `LayerNode.make` ecrit dans tout noeud construit par
/// sa fonction d entree : `"layer"`.
///
/// Il est exporte comme constante et non porte comme champ de [`Node`] : voir la
/// doc de [`Node`] pour pourquoi les champs de l interface ne sont pas tous
/// presents.
pub const NODE_KIND: &str = "layer";

/// Journaliers d origine : `Logging.loggers()`.
///
/// Comparaison stricte a `"1"`, pas un test de veracite.
pub const ENV_PRINT_LOGS: &str = "OPENCODE_PRINT_LOGS";

/// Niveau minimum, lu par `Logging.minimumLogLevel()`.
pub const ENV_LOG_LEVEL: &str = "OPENCODE_LOG_LEVEL";

/// Point de collecte OTLP, lu par `Flag.OTEL_EXPORTER_OTLP_ENDPOINT`.
///
/// Le drapeau est un simple `process.env`, sans valeur par defaut : une
/// variable absente et une variable vide desactivent donc toutes deux l OTLP.
pub const ENV_OTLP_ENDPOINT: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";

/// Suffixe ajoute au point de collecte pour obtenir l URL des journaux.
pub const OTLP_LOGS_PATH: &str = "/v1/logs";

/// Suffixe ajoute au point de collecte pour obtenir l URL des traces.
pub const OTLP_TRACES_PATH: &str = "/v1/traces";

/// Un journalier installe par la couche.
///
/// Les noms de variante sont des etiquettes cote Rust : la source ne nomme que
/// `fileLogger`, `stderrLogger` et `OtlpLogger.make`, dont le contenu n est pas
/// porte ici.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Journal {
    /// `Logger.toFile` sur le fichier de journal de l installation.
    File,
    /// Ecriture directe sur `process.stderr`.
    Stderr,
    /// `OtlpLogger.make`, avec l URL complete des journaux.
    Otlp { url: String },
}

impl Journal {
    /// `true` si le journalier est celui de l OTLP.
    pub fn est_otlp(&self) -> bool {
        matches!(self, Journal::Otlp { .. })
    }
}

/// Un service fourni a la couche de journalisation.
///
/// L ordre de la liste produite par [`SERVICES`] est celui du `pipe` de la
/// source : le premier est le plus proche de la couche, le dernier le plus
/// exterieur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Service {
    /// `NodeFileSystem.layer`.
    NodeFileSystem,
    /// `OtlpSerialization.layerJson`.
    OtlpSerializationJson,
    /// `FetchHttpClient.layer`.
    FetchHttpClient,
}

/// Les trois services fournis, dans l ordre du `pipe`.
pub const SERVICES: [Service; 3] = [
    Service::NodeFileSystem,
    Service::OtlpSerializationJson,
    Service::FetchHttpClient,
];

/// Niveau de journalisation installe par la reference
/// `References.MinimumLogLevel`.
///
/// Les variantes reprennent les chaines de la source, qui ne sont pas les
/// noms de la variable d environnement : `WARN` devient `Warn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// La chaine attendue par le type `LogLevel` d Effect.
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Debug => "Debug",
            LogLevel::Info => "Info",
            LogLevel::Warn => "Warn",
            LogLevel::Error => "Error",
        }
    }
}

/// Les options de `Logger.layer`.
///
/// La seule option de la source est `mergeWithExisting: false`, et son nom est
/// camelCase : le `serde(rename)` ci-dessous est donc explicite, jamais deduit
/// d un `rename_all` sur le conteneur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoggerOptions {
    /// `false` : la liste REMPLACE le jeu de journaliers deja present au lieu
    /// de s y ajouter.
    #[serde(rename = "mergeWithExisting")]
    pub merge_with_existing: bool,
}

impl Default for LoggerOptions {
    fn default() -> Self {
        Self {
            merge_with_existing: false,
        }
    }
}

/// Les trois variables d'environnement dont depend la composition.
///
/// Elles sont regroupees dans une structure pour que les tests n aient pas a
/// toucher au processus : `Options::from_env` fait le pont.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// `OPENCODE_PRINT_LOGS`, comparee a `"1"`.
    pub print_logs: Option<String>,
    /// `OPENCODE_LOG_LEVEL`, mise en majuscules avant d etre lue.
    pub log_level: Option<String>,
    /// `OTEL_EXPORTER_OTLP_ENDPOINT`, testee par veracite.
    pub otlp_endpoint: Option<String>,
}

impl Options {
    /// Les options decrites explicitement.
    pub fn new(
        print_logs: Option<String>,
        log_level: Option<String>,
        otlp_endpoint: Option<String>,
    ) -> Self {
        Self {
            print_logs,
            log_level,
            otlp_endpoint,
        }
    }

    /// L environnement du processus courant.
    pub fn from_env() -> Self {
        Self {
            print_logs: env::var(ENV_PRINT_LOGS).ok(),
            log_level: env::var(ENV_LOG_LEVEL).ok(),
            otlp_endpoint: env::var(ENV_OTLP_ENDPOINT).ok(),
        }
    }

    /// `true` si la journalisation OTLP est activee.
    ///
    /// C est le test de veracite de la source : une chaine vide est absente,
    /// une chaine d espaces ne l est PAS.
    pub fn otlp_actif(&self) -> bool {
        match &self.otlp_endpoint {
            Some(point) => !point.is_empty(),
            None => false,
        }
    }

    /// Les journaliers de `Logging.loggers()` puis ceux de `Otlp.loggers()`,
    /// dans cet ordre, comme la concatenation de la source.
    pub fn loggers(&self) -> Vec<Journal> {
        let mut liste = Vec::new();
        // `loggers()` rend toujours le journal fichier, eventuellement suivi du
        // journal sur erreur standard.
        liste.push(Journal::File);
        if self.print_logs.as_deref() == Some("1") {
            liste.push(Journal::Stderr);
        }
        // `Otlp.loggers()` rend une liste vide ou une liste d un element.
        if self.otlp_actif() {
            let point = self
                .otlp_endpoint
                .as_deref()
                .unwrap_or_default()
                .to_owned();
            liste.push(Journal::Otlp {
                url: format!("{point}{OTLP_LOGS_PATH}"),
            });
        }
        liste
    }

    /// Le niveau minimum installe par la couche.
    pub fn log_level(&self) -> LogLevel {
        minimum_log_level(self.log_level.as_deref())
    }
}

/// La table de `Logging.minimumLogLevel()`.
///
/// La source fait `value && value in levels ? levels[value] : levels.INFO` :
/// le test est un test de veracite applique a la valeur deja mise en
/// majuscules. Une valeur absente, une chaine vide, des espaces autour du nom
/// ou un niveau inconnu donnent donc tous `Info`. Aucun espacement n est retire.
pub fn minimum_log_level(valeur: Option<&str>) -> LogLevel {
    match valeur {
        Some(valeur) if !valeur.is_empty() => match valeur.to_uppercase().as_str() {
            "DEBUG" => LogLevel::Debug,
            "WARN" => LogLevel::Warn,
            "ERROR" => LogLevel::Error,
            "INFO" => LogLevel::Info,
            _ => LogLevel::Info,
        },
        _ => LogLevel::Info,
    }
}

/// Le noeud de couches `observability`.
///
/// L interface `Node` de `effect/layer-node.ts` declare exactement six champs :
/// `kind`, `name`, `service?`, `implementation?`, `dependencies`, `tag?`. Deux
/// seulement sont portes ici, `name` et `dependencies`, parce que ce sont les
/// deux seuls qui soient des DONNEES : `service` et `tag` valent `undefined`
/// pour cet appel (la source passe `name`, pas `service`), `implementation`
/// vaut la couche, dont le graphe n est pas simule, et `kind` vaut `"layer"`
/// pour tout noeud produit par `make`, donc est une constante plutot qu une
/// donnee, voir [`NODE_KIND`].
///
/// Les autres champs de ce struct (`loggers`, `options`, `services`,
/// `minimum_log_level`, `tracing_layer`) ne viennent donc PAS de l interface :
/// ils decrivent ce que la couche installe, ce que la source laisse dans
/// l objet opaque `Layer.Any`. Ils n ont aucun nom contractuel, et c est
/// deliberement : c est la composition reportee comme donnee.
///
/// Les champs sont serialises pour pouvoir fixer leurs noms dans un test ; le
/// noeud ne voyage pas sur le reseau, c est un descripteur interne.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// `"observability"`, la seule chose que `LayerNode.make` retient du nom.
    pub name: String,
    /// `deps: []` : aucune dependance. La couche ne depend d aucun autre
    /// noeud de l arborescence, elle fournit elle-meme ses trois services.
    pub dependencies: Vec<String>,
    /// Les journaliers, dans l ordre de la source.
    pub loggers: Vec<Journal>,
    /// Les options de `Logger.layer`.
    pub options: LoggerOptions,
    /// Les trois services fournis, dans l ordre du `pipe`.
    pub services: Vec<Service>,
    /// `Layer.orDie` : une erreur de construction tue le programme au lieu de
    /// faire echouer la couche.
    #[serde(rename = "orDie")]
    pub or_die: bool,
    /// La valeur installee dans `References.MinimumLogLevel`.
    #[serde(rename = "minimumLogLevel")]
    pub minimum_log_level: LogLevel,
    /// `true` si `Otlp.tracingLayer` installe la couche OpenTelemetry, `false`
    /// si elle vaut `Layer.empty`.
    #[serde(rename = "tracingLayer")]
    pub tracing_layer: bool,
}

impl Node {
    /// Le noeud decrit par les options passees.
    pub fn from_options(options: &Options) -> Self {
        Self {
            name: NODE_NAME.to_owned(),
            dependencies: Vec::new(),
            loggers: options.loggers(),
            options: LoggerOptions::default(),
            services: SERVICES.to_vec(),
            or_die: true,
            minimum_log_level: options.log_level(),
            tracing_layer: options.otlp_actif(),
        }
    }

    /// `true` si un journalier OTLP est installe.
    pub fn a_journalier_otlp(&self) -> bool {
        self.loggers
            .iter()
            .any(|journal| matches!(journal, Journal::Otlp { .. }))
    }

    /// L URL des traces, si la couche de tracage est installee.
    ///
    /// `tracingLayer` construit `${endpoint}/v1/traces` avec le MEME point de
    /// collecte que le journalier OTLP, qui lui construit `${endpoint}/v1/logs`.
    /// Comme le descripteur ne stocke que le journalier, l URL des traces est
    /// obtenue en retirant le suffixe des journaux de celle du journalier.
    /// `strip_suffix` est ancre a la FIN de la chaine, donc un point de
    /// collecte qui contiendrait lui-meme `/v1/logs` donne bien
    /// `${endpoint}/v1/traces`, comme la source.
    pub fn url_traces(&self) -> Option<String> {
        let point = self
            .loggers
            .iter()
            .find_map(|journal| match journal {
                Journal::Otlp { url } => Some(url),
                _ => None,
            })?;
        let base = point.strip_suffix(OTLP_LOGS_PATH)?;
        Some(format!("{base}{OTLP_TRACES_PATH}"))
    }
}

/// `export const layer = Layer.unwrap(Effect.gen(...))`, avec des options
/// explicites.
pub fn layer(options: &Options) -> Node {
    Node::from_options(options)
}

/// `export const node = LayerNode.make({ name: "observability", layer, deps: [] })`,
/// lu dans l environnement du processus.
pub fn node() -> Node {
    layer(&Options::from_env())
}

#[cfg(test)]
mod tests {
    use super::{
        layer, minimum_log_level, node, Journal, LogLevel, LoggerOptions, Node, Options, Service,
        ENV_LOG_LEVEL, ENV_OTLP_ENDPOINT, ENV_PRINT_LOGS, NODE_KIND, NODE_NAME, OTLP_LOGS_PATH,
        OTLP_TRACES_PATH, SERVICES,
    };
    use std::collections::BTreeSet;

    /// Le journal fichier est toujours present : c est le seul journalier
    /// present dans un environnement vide.
    const JOURNAL: Journal = Journal::File;

    fn options() -> Options {
        Options::default()
    }

    fn sans_environnement() -> Node {
        Node::from_options(&Options::default())
    }

    // --- le noeud ---------------------------------------------------------

    #[test]
    fn un_noeud_porte_le_nom_de_la_source() {
        assert_eq!(sans_environnement().name, NODE_NAME);
    }

    #[test]
    fn un_noeud_construit_par_make_est_de_leurre_layer() {
        // `LayerNode.make` ecrit `kind: "layer"` dans tout noeud qu il produit,
        // quel que soit l arbre. Cette valeur n est pas un champ de `Node`
        // ici parce qu elle ne varie pas, mais elle fait partie de ce que la
        // source produit.
        assert_eq!(NODE_KIND, "layer");
    }

    #[test]
    fn un_noeud_sans_dependance_a_une_liste_vide() {
        assert!(sans_environnement().dependencies.is_empty());
    }

    #[test]
    fn un_noeud_lit_l_environnement_du_processus() {
        // `node` n est teste que sur ce qu il lit reellement : le processus
        // n a pas forcement ces variables, donc le seul invariant solide est
        // que le nom, lui, ne depend pas de l environnement.
        assert_eq!(node().name, "observability");
    }

    #[test]
    fn les_deux_exports_de_la_source_donnent_le_meme_noeud() {
        // `layer` et `node` sont deux exports distincts de la source, mais ils
        // decrivent la meme couche : seule l origine des options change.
        let options = Options::new(
            Some("1".to_owned()),
            Some("debug".to_owned()),
            Some("http://h".to_owned()),
        );
        assert_eq!(layer(&options), Node::from_options(&options));
        let par_layer = layer(&options);
        assert_eq!(par_layer.loggers.len(), 3);
        assert!(par_layer.tracing_layer);
        assert_eq!(par_layer.minimum_log_level, LogLevel::Debug);
    }

    #[test]
    fn une_erreur_de_construction_tue_le_programme() {
        // `Layer.orDie` est dans le pipe : c est le comportement du noeud.
        assert!(sans_environnement().or_die);
    }

    #[test]
    fn les_trois_services_sont_fournis_dans_l_ordre_du_pipe() {
        let attendus: BTreeSet<Service> = SERVICES.iter().copied().collect();
        let vus: BTreeSet<Service> = sans_environnement().services.iter().copied().collect();
        assert_eq!(vus, attendus);
        assert_eq!(
            sans_environnement().services,
            vec![
                Service::NodeFileSystem,
                Service::OtlpSerializationJson,
                Service::FetchHttpClient
            ]
        );
    }

    // --- les noms de champs, le piege de la mission -----------------------

    #[test]
    fn l_option_de_fusion_des_journaliers_porte_le_nom_de_la_source() {
        // `mergeWithExisting`, pas `merge_with_existing`, pas `mergeexisting`.
        let json = serde_json::to_string(&LoggerOptions::default()).unwrap();
        assert_eq!(json, r#"{"mergeWithExisting":false}"#);
    }

    #[test]
    fn un_noeud_serialise_les_noms_de_champs_de_la_source() {
        let json = serde_json::to_value(sans_environnement()).unwrap();
        let cles: BTreeSet<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let attendus: BTreeSet<&str> = [
            "name",
            "dependencies",
            "loggers",
            "options",
            "services",
            "orDie",
            "minimumLogLevel",
            "tracingLayer",
        ]
        .into_iter()
        .collect();
        assert_eq!(cles, attendus);
        assert_eq!(
            json["name"], "observability",
            "le nom de la couche ne doit jamais disparaitre"
        );
        assert_eq!(
            json["orDie"], true,
            "orDie doit rester camelCase, et non or_die"
        );
        assert_eq!(json["minimumLogLevel"], "Info");
    }

    #[test]
    fn un_noeud_serialise_comme_un_environnement_vide() {
        let json = serde_json::to_string(&sans_environnement()).unwrap();
        assert_eq!(
            json,
            concat!(
                r#"{"name":"observability","dependencies":[],"loggers":["File"],"#,
                r#""options":{"mergeWithExisting":false},"#,
                r#""services":["NodeFileSystem","OtlpSerializationJson","FetchHttpClient"],"#,
                r#""orDie":true,"minimumLogLevel":"Info","tracingLayer":false}"#
            )
        );
    }

    #[test]
    fn un_noeud_serialise_puis_deserialise_redonne_le_meme_noeud() {
        let original = Node::from_options(&Options::new(
            Some("1".to_owned()),
            Some("warn".to_owned()),
            Some("http://localhost:4318".to_owned()),
        ));
        let json = serde_json::to_string(&original).unwrap();
        let relu: Node = serde_json::from_str(&json).unwrap();
        assert_eq!(original, relu);
        assert!(json.contains(r#""minimumLogLevel":"Warn""#));
        assert!(json.contains(r#""url":"http://localhost:4318/v1/logs""#));
    }

    // --- les journaliers --------------------------------------------------

    #[test]
    fn sans_environnement_on_n_obtient_que_le_journal_fichier() {
        let liste = options().loggers();
        assert_eq!(liste, vec![JOURNAL]);
    }

    #[test]
    fn une_ariable_imprimante_a_1_ajoute_le_journal_sur_erreur() {
        let liste = Options::new(Some("1".to_owned()), None, None).loggers();
        assert_eq!(liste.len(), 2);
        assert_eq!(liste[1], Journal::Stderr);
    }

    #[test]
    fn une_ariable_imprimante_valide_autrement_n_ajoute_rien() {
        // Egalite stricte : ni "true", ni "0", ni " 1" ne valent "1".
        for valeur in ["true", "0", " 1", "1 ", "01", "1\n", "oui"] {
            let liste = Options::new(Some(valeur.to_owned()), None, None).loggers();
            assert_eq!(
                liste,
                vec![JOURNAL],
                "la valeur {valeur:?} ne devrait pas activer le journal sur erreur"
            );
        }
    }

    #[test]
    fn sans_point_de_collecte_il_n_y_a_pas_de_journalier_otlp() {
        assert!(!options().otlp_actif());
        assert!(!options()
            .loggers()
            .iter()
            .any(|journal| journal.est_otlp()));
    }

    #[test]
    fn un_point_de_collecte_vide_desactive_le_journalier_otlp() {
        // `if (!endpoint) return []` est un test de veracite : la chaine vide
        // est absente. Un simple `.map()` sur un `Option` l aurait laissee
        // survivee et aurait produit l URL "/v1/logs".
        let opt = Options::new(None, None, Some(String::new()));
        assert!(!opt.otlp_actif());
        assert_eq!(opt.loggers(), vec![JOURNAL]);
    }

    #[test]
    fn un_point_de_collecte_donne_un_chemin_de_journal_otlp() {
        let liste = Options::new(None, None, Some("http://localhost:4318".to_owned())).loggers();
        assert_eq!(liste.len(), 2);
        assert_eq!(
            liste[1],
            Journal::Otlp {
                url: format!("http://localhost:4318{OTLP_LOGS_PATH}")
            }
        );
    }

    #[test]
    fn un_point_de_collecte_termine_par_une_barre_garde_la_barre_en_double() {
        // `${endpoint}/v1/logs` ne normalise rien : pas de fusion de barre.
        let liste = Options::new(None, None, Some("http://h:4318/".to_owned())).loggers();
        assert_eq!(
            liste[1],
            Journal::Otlp {
                url: format!("http://h:4318/{OTLP_LOGS_PATH}")
            }
        );
    }

    #[test]
    fn un_point_de_collecte_d_espaces_est_actif_comme_en_javascript() {
        // Une chaine d espaces est veridique en JavaScript : elle reste un point
        // de collecte. On ne la nettoie pas.
        let opt = Options::new(None, None, Some("  ".to_owned()));
        assert!(opt.otlp_actif());
        assert_eq!(opt.loggers().len(), 2);
    }

    #[test]
    fn les_journaliers_du_fichier_et_de_l_erreur_viennent_avant_otlp() {
        // Ordre de la source : `[...Logging.loggers(), ...Otlp.loggers()]`.
        let liste = Options::new(
            Some("1".to_owned()),
            None,
            Some("http://h".to_owned()),
        )
        .loggers();
        assert_eq!(liste.len(), 3);
        assert_eq!(liste[0], JOURNAL);
        assert_eq!(liste[1], Journal::Stderr);
        assert!(liste[2].est_otlp());
    }

    // --- le niveau minimum ------------------------------------------------

    #[test]
    fn un_niveau_absent_donne_info() {
        assert_eq!(minimum_log_level(None), LogLevel::Info);
    }

    #[test]
    fn un_niveau_vide_donne_info() {
        // `value &&` : la chaine vide est falsy, donc la table n est pas
        // consultee et le defaut s applique.
        assert_eq!(minimum_log_level(Some("")), LogLevel::Info);
    }

    #[test]
    fn un_niveau_connu_est_reconnu_independant_de_la_casse() {
        assert_eq!(minimum_log_level(Some("debug")), LogLevel::Debug);
        assert_eq!(minimum_log_level(Some("DEBUG")), LogLevel::Debug);
        assert_eq!(minimum_log_level(Some("Warn")), LogLevel::Warn);
        assert_eq!(minimum_log_level(Some("error")), LogLevel::Error);
        assert_eq!(minimum_log_level(Some("Info")), LogLevel::Info);
    }

    #[test]
    fn un_niveau_inconnu_ou_entoure_d_espaces_donne_info() {
        // La table ne contient que quatre cles, sans nettoyage des espaces.
        for valeur in ["trace", " debug ", "debug\n", "notset", "verbose", "erreur"] {
            assert_eq!(
                minimum_log_level(Some(valeur)),
                LogLevel::Info,
                "la valeur {valeur:?} ne devrait pas etre reconnue"
            );
        }
    }

    #[test]
    fn un_nom_du_prototype_javascript_donne_info_et_non_une_fonction() {
        // `value in levels` remonte la chaine de prototypes en JavaScript : la
        // source installerait une FONCTION comme niveau minimum. Ce portage
        // garde une table ferme, divergence assumee et documentee en tete de
        // fichier.
        for valeur in ["toString", "constructor", "valueOf", "hasOwnProperty"] {
            assert_eq!(
                minimum_log_level(Some(valeur)),
                LogLevel::Info,
                "le nom {valeur:?} ne devrait pas traverser la table"
            );
        }
    }

    #[test]
    fn le_niveau_installe_suit_la_variable_du_noeud() {
        let n = Node::from_options(&Options::new(None, Some("debug".to_owned()), None));
        assert_eq!(n.minimum_log_level, LogLevel::Debug);
        assert_eq!(n.minimum_log_level.as_str(), "Debug");
        assert_eq!(
            sans_environnement().minimum_log_level,
            LogLevel::Info,
            "sans variable, le noeud installe Info"
        );
    }

    #[test]
    fn l_option_de_fusion_est_desactivee_par_defaut() {
        assert!(!LoggerOptions::default().merge_with_existing);
        assert!(!sans_environnement().options.merge_with_existing);
    }

    // --- la couche de tracage ---------------------------------------------

    #[test]
    fn la_couche_de_tracage_est_absente_sans_point_de_collecte() {
        assert!(!sans_environnement().tracing_layer);
        assert!(!sans_environnement().a_journalier_otlp());
        assert_eq!(sans_environnement().url_traces(), None);
    }

    #[test]
    fn la_couche_de_tracage_est_installee_avec_un_point_de_collecte() {
        let n = Node::from_options(&Options::new(None, None, Some("http://h:4318".to_owned())));
        assert!(n.tracing_layer);
        assert!(n.a_journalier_otlp());
        assert_eq!(
            n.url_traces(),
            Some(format!("http://h:4318{OTLP_TRACES_PATH}"))
        );
    }

    #[test]
    fn un_point_de_collecte_qui_contient_le_chemin_des_journaux_garde_la_bonne_url_de_traces() {
        // Le suffixe est retire a la FIN de la chaine, pas recherche partout :
        // `${endpoint}/v1/traces` reste exact, point de collecte compris.
        let n = Node::from_options(&Options::new(
            None,
            None,
            Some("http://h/v1/logs".to_owned()),
        ));
        assert_eq!(
            n.loggers[1],
            Journal::Otlp {
                url: format!("http://h/v1/logs{OTLP_LOGS_PATH}")
            }
        );
        assert_eq!(
            n.url_traces(),
            Some(format!("http://h/v1/logs{OTLP_TRACES_PATH}"))
        );
    }

    #[test]
    fn un_point_de_collecte_vide_laisse_la_tracage_desactivee() {
        let n = Node::from_options(&Options::new(None, None, Some(String::new())));
        assert!(!n.tracing_layer);
        assert!(!n.a_journalier_otlp());
    }

    // --- les noms des variables d'environnement --------------------------

    #[test]
    fn les_variables_lues_sont_celles_de_la_source() {
        assert_eq!(ENV_PRINT_LOGS, "OPENCODE_PRINT_LOGS");
        assert_eq!(ENV_LOG_LEVEL, "OPENCODE_LOG_LEVEL");
        assert_eq!(ENV_OTLP_ENDPOINT, "OTEL_EXPORTER_OTLP_ENDPOINT");
    }
}
