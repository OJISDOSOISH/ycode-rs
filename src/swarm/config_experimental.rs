//! Portage Rust de `opencode/packages/core/src/config/experimental.ts`.
//!
//! La source tient en dix-huit lignes et ne contient aucun calcul : deux
//! declarations de schema, plus un schema d'action. Elle decrit la section
//! `experimental` du fichier de configuration, celle qui porte les regles de
//! politique (`allow` / `deny`) appliquees aux ressources d'un projet.
//!
//! ```ts
//! export const PolicyAction = Schema.Union([Catalog.PolicyActions])
//!
//! export class Policy extends Schema.Class<Policy>("ConfigV2.Experimental.Policy")({
//!   ...PolicyV2.Info.fields,
//!   action: PolicyAction,
//! }) {}
//!
//! export class Experimental extends Schema.Class<Experimental>("ConfigV2.Experimental")({
//!   policies: Policy.pipe(Schema.Array, Schema.optional),
//! }) {}
//! ```
//!
//! La premiere ligne de la source, `export * as ConfigExperimental from
//! "./experimental"`, est un reexport de l'espace de noms du module sur
//! LUI-MEME. C'est du code mort, il n'y a donc rien a en porter : le module
//! Rust joue deja ce role.
//!
//! ## Ce qui vient d'ailleurs, et ce qui n'a pas de traduction
//!
//! - `Catalog.PolicyActions` est declare dans `packages/core/src/catalog.ts`,
//!   ligne 20 : `Schema.Literals(["provider.use"])`. Ce fichier n'est pas porte
//!   par ce lot, et son reste (graphe de couches `Layer`, `State` transformable,
//!   `Effect.gen`) n'a pas de traduction Rust ici : rien n'est simule.
//! - `PolicyV2.Info.fields` vient de `packages/core/src/policy.ts`, ligne 11 :
//!   `{ action: Schema.String, effect: Effect, resource: Schema.String }`. Le
//!   spread `{ ...Info.fields, action: PolicyAction }` **recopie** ces trois
//!   champs puis **ecrase** `action`. L'ordre des cles reste donc
//!   `action, effect, resource`, et `action` devient un litteral la ou il
//!   etait un `String` libre. C'est le seul changement de comportement entre
//!   `Policy.Info` et `Policy` experimental.
//! - `crate::policy` (le portage de `policy.ts`, deja present dans
//!   `src/policy.rs`) definit deja l'enum `Effect` avec les memes deux
//!   variantes et le meme format JSON. On le **redeclare** ici plutot que de
//!   faire `pub use crate::policy::Effect` : le lot swarm garde des fichiers
//!   autonomes, et un agent a deja du arbitrer le meme doublon sur
//!   `tool_tools.rs`. Les deux definitions DOIVENT rester identiques, sinon
//!   `policy.load(...)` et la lecture de la configuration se contredisent.
//! - Les deux identifiants de schema, `"ConfigV2.Experimental.Policy"` et
//!   `"ConfigV2.Experimental"`, ne produisent aucun champ : ils ne servent
//!   qu'a nommer le schema dans les messages d'erreur d'Effect. On les garde
//!   en constantes, sans champ.
//! - `packages/core/src/v1/config/config.ts:185` ecrit
//!   `Schema.optional(Schema.mutable(Schema.Array(ConfigExperimental.Policy)))`.
//!   Le `Schema.mutable` n'a pas de traduction : en TypeScript la
//!   configuration decodee est un objet que l'on modifie sur place avant de
//!   le renvoyer, alors qu'en Rust un `Vec<Policy>` est deja possede et
//!   mutable par son proprietaire. Seule la forme `Vec` reste.
//!
//! ## Le piege `?` contre `??` : il est ici, mais chez les conSommateurs
//!
//! Ce fichier-ci ne contient ni ternaire `?` ni coalescent `??` : il n'y a rien
//! a distinguer a l'interieur. En revanche les deux operateurs se
//! combinent chez les conSommateurs, et la distinction se voit dans les types
//! choisis ici :
//!
//! - `config.ts:210` : `config.info.experimental?.policies ?? []`. Les deux
//!   operateurs testent la NULLITE. Une liste explicitement vide est un `[]`
//!   qui SURVIT au `??`, donc `Some(vec![])` doit rester `Some(vec![])` et ne
//!   peut pas etre confondu avec `None`.
//! - `v1/config/migrate.ts:69` :
//!   `info.experimental?.policies && { policies: info.experimental.policies }`.
//!   La la, `&&` teste la VERACITE, et c'est different : en JavaScript un
//!   tableau vide est **truthy**. Donc `None` doit disparaitre (le resultat de
//!   l'expression vaut alors `undefined`) et `Some(vec![])` doit produire
//!   `{ "policies": [] }`. Un portage qui testerait `policies.is_empty()` pour
//!   savoir si le champ est present se tromperait sur ce cas-la, et perdrait
//!   une information au moment de la migration v1 vers v2.
//!
//! C'est pourquoi `policies` est un `Option<Vec<Policy>>` et non un `Vec` avec
//! une valeur par defaut vide : l'absence et la presence vide sont deux etats
//! distincts ici. Aucun `unwrap_or_default`, aucun `is_empty()`.
//!
//! Meme remarque pour `resource`, qui est un `Schema.String` : la chaine vide
//! est une ressource **valide** et doit survivre telle quelle jusqu'a
//! `Wildcard.match`. Le type retenu est `String`, pas `Option<String>`, il n'y
//! donc rien a filtrer et rien a perdre.
//!
//! ## Les noms de champs : aucun piege, mais des `rename` explicites
//!
//! `action`, `effect`, `resource`, `policies` sont des mots uniques en
//! minuscules : zero `camelCase`, zero suffixe `ID`. Contrairement aux
//! fichiers de la meme vague (`projectID`, `sessionID`), il n'y a donc aucune
//! majuscule a faithfully transporter. Les `#[serde(rename = "...")]` sont
//! nevertheless poses un par un, en garde-fou : ils ecrivent ce que le nom du
//! champ Rust dit deja, donc un renommage futur cassera la compilation plutot
//! que l'echange avec le TypeScript. Un test dedie verifie les noms.
//!
//! ## Le point NON verifie : `null` dans `policies`
//!
//! Le test `null_dans_policies_est_lu_comme_une_absence_par_serde_et_la_parite_effect_reste_non_verifiee`
//! verrouille `{"policies": null}` comme une absence. Ce qui est **certain**,
//! c'est le comportement de Serde : `null` se decode en `None` pour un
//! `Option`, sans qu'aucune option ne le demande. Ce qui ne l'est **pas**,
//! c'est la parite avec Effect.
//!
//! L'hypothese a trancher est la suivante : `Schema.optional` n'etant pas
//! appele avec `{ exact: true }`, il traiterait `null` comme `undefined`. Elle
//! **n'a pas pu etre verifiee** : `exact` n'apparait avec `Schema.optional`
//! nulle part dans le depot opencode (tous les `exact: true` trouves sont des
//! selecteurs Playwright), et la recherche ci-dessous n'a rien tranche.
//!
//! - `node_modules` est absent du depot opencode, et il n'existe aucun
//!   `node_modules` ailleurs sur la machine : aucun `Schema.decodeUnknown`
//!   ne peut etre execute pour observer le comportement reel.
//! - `patches/effect@4.0.0-beta.83.patch` ne touche que
//!   `dist/unstable/httpapi/HttpApiSchema.ts`, pas `PropertySignature` : il ne
//!   change donc rien de notre cote.
//! - Aucun test du depot opencode ne decode un `null` contre un champ
//!   declare `Schema.optional` : il n'y a aucun resultat observe a recopier.
//!
//! Deux indices **indirects et de sens opposes** ont ete trouves, et c'est
//! precisement ce qui interdit d'arbitrer :
//!
//! - `packages/opencode/src/server/routes/instance/httpapi/public.ts` lignes
//!   92 et 460 : "Effect's `Schema.optional` emits `anyOf: [T, {type:"null"}]`"
//!   et "Strip `{type:"null"}` arms that Effect's `Schema.optional` adds".
//!   Cela decrit la GENERATION de JSON Schema, ou l'absence est representee
//!   par une branche `null`. Ce n'est pas le decodeur, et ca ne dit rien de ce
//!   qu'il accepte reellement.
//! - `packages/llm/src/protocols/shared.ts` ligne 25 definit
//!   `optionalNull = Schema.optional(Schema.NullOr(schema))`, employe partout
//!   ou un fournisseur envoie reellement `null`. Les auteurs n'auraient rien a
//!   ajouter si `optional` acceptait deja `null`.
//!
//! Consequence a connaitre : si Effect refuse `null` sur une propriete
//! optionnelle non exacte, alors `{"policies": null}` **est** une erreur de
//! decodage cote TypeScript, la ou le derive Serde l'accepte ici. Sur du JSON
//! ecrit par le TypeScript la difference est invisible ; elle ne se voit que
//! sur une saisie manuelle. Le test reste en place parce qu'il ne certifie
//! qu'une verite qui, elle, ne depend pas d Effect : ce que fait Serde. Le
//! jour ou la parite est etablie, le correctif tient en un `deserialize_with`
//! qui refuse `null` sur le champ, et rien d'autre ne change.
//!
//! L'hypothese se propage au niveau superieur : `v1/config/config.ts` ligne 185
//! declare `Schema.optional(Schema.mutable(Schema.Array(Policy)))`, donc
//! `{"policies": null}` y pose exactement la meme question. Ce fichier ne
//! porte que le niveau interne.

use serde::{Deserialize, Serialize};

/// Identifiant de schema de `Policy` dans la source.
pub const SCHEMA_IDENTIFIER_POLICY: &str = "ConfigV2.Experimental.Policy";

/// Identifiant de schema de `Experimental` dans la source.
pub const SCHEMA_IDENTIFIER_EXPERIMENTAL: &str = "ConfigV2.Experimental";

/// Les actions qu'une regle de politique peut viser.
///
/// Equivalent de `PolicyAction = Schema.Union([Catalog.PolicyActions])`, donc
/// d'un `Schema.Literals(["provider.use"])` defini dans `catalog.ts`.
/// `Schema.Union` ne contenant qu'un seul membre, l'union se reduit a ce
/// membre : une variante suffit.
///
/// Le commentaire de la source est explicite sur l'intention : ajouter une
/// action a cette union la rend valide dans une configuration ecrite a la
/// main, tout en gardant `Policy` generique. Concretement, ajouter une
/// variante **ici** est ce qui autorise une action de plus dans les fichiers
/// de configuration ; sans variante, serde refuse l'action a la lecture.
///
/// Une variante unique est un `enum` et non un `String` : `Schema.Literals`
/// REFUSE de decoder autre chose que `"provider.use"`, alors qu'un `String`
/// accepterait n'importe quoi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyAction {
    /// Autorise ou refuse l'usage d'un fournisseur.
    #[serde(rename = "provider.use")]
    ProviderUse,
}

impl PolicyAction {
    /// La chaine telle qu'elle est ecrite dans le fichier de configuration.
    pub const fn as_str(self) -> &'static str {
        match self {
            PolicyAction::ProviderUse => "provider.use",
        }
    }
}

/// L'effet d'une regle : autoriser ou refuser.
///
/// Reprise de `Schema.Literals(["allow", "deny"])` dans `policy.ts`, ligne 8,
/// qui arrive ici par le spread `...PolicyV2.Info.fields`. Les chaines sont
/// celles de l'origine, en minuscules, sans tiret ni majuscule.
///
/// ATTENTION : `src/policy.rs` definit deja cet enum. Les deux copies doivent
/// rester identiques, celle-ci n'est pas un reexport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    /// La regle s'applique : l'action est autorisee.
    Allow,
    /// La regle s'applique : l'action est refusee.
    Deny,
}

/// Une regle de politique ecrite dans la configuration.
///
/// Les trois champs viennent de `Policy.Info`, dont `action` est ici
/// remplace par le litteral `PolicyAction` : une regle de configuration ne
/// peut donc viser qu'une action reellement declaree par un domaine, alors
/// que `policy.ts` accepte n'importe quelle chaine et s'appuie sur
/// `Wildcard.match`.
///
/// `resource` est un motif a jokers (`*` et `?`), pas une chemin. Il est
/// conserve verbatim, sans normalisation ni validation. La chaine vide est
/// acceptee par le schema et reste acceptee ici.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    /// Action visee par la regle.
    #[serde(rename = "action")]
    pub action: PolicyAction,

    /// Effet applicable : autoriser ou refuser.
    #[serde(rename = "effect")]
    pub effect: Effect,

    /// Motif de la ressource visee.
    #[serde(rename = "resource")]
    pub resource: String,
}

impl Policy {
    /// Construit une regle a partir de ses trois champs.
    pub fn new(action: PolicyAction, effect: Effect, resource: impl Into<String>) -> Self {
        Self { action, effect, resource: resource.into() }
    }
}

/// La section `experimental` du fichier de configuration.
///
/// Un seul champ, `policies`, qui est la liste des regles de politique. Il
/// est optionnel, et ABSENT n'est pas la meme chose que PRESENT ET VIDE : voir
/// la section sur le piege `?` contre `??` dans l'en-tete du module.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Experimental {
    /// Regles de politique, dans l'ordre ou elles sont ecrites.
    ///
    /// L'ordre est significatif : `policy.ts:38` utilise `findLast`, donc
    /// c'est la derniere regle qui correspond qui gagne. Un `BTreeSet` ou un
    /// tri quelconque casserait cette semantique.
    ///
    /// Cote decodage, `null` se lit comme une absence : c'est le comportement
    /// de `Option` sous Serde, et il n'a rien a voir avec une hypothese sur
    /// Effect. La parite avec `Schema.optional` sans `exact: true`, qui
    /// traiterait `null` comme `undefined`, **n'est pas verifiee** ; voir la
    /// section "Le point NON verifie" du module.
    #[serde(rename = "policies", skip_serializing_if = "std::option::Option::is_none")]
    pub policies: Option<Vec<Policy>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regle() -> Policy {
        Policy::new(PolicyAction::ProviderUse, Effect::Allow, "*")
    }

    #[test]
    fn les_noms_des_champs_restent_ceux_du_typescript() {
        // Verification explicite demanded par la mission : on serialise et on
        // compare les noms, un par un.
        let json = serde_json::to_string(&regle()).unwrap();
        assert_eq!(
            json,
            "{\"action\":\"provider.use\",\"effect\":\"allow\",\"resource\":\"*\"}"
        );
        // L'ordre des cles suit l'ordre du spread `{ ...Info.fields, action }`.
        let valeur = serde_json::to_value(&regle()).unwrap();
        let cles: Vec<&String> = valeur.as_object().unwrap().keys().collect();
        assert_eq!(cles, vec!["action", "effect", "resource"]);
        // Aucune variante camelCase ni suffixe ID ne doit apparaitre.
        assert!(!json.contains("Action"));
        assert!(!json.contains("ID"));
        assert!(!json.contains("Id"));
    }

    #[test]
    fn le_nom_du_champ_de_liste_est_policies() {
        let experimental = Experimental { policies: Some(vec![regle()]) };
        let json = serde_json::to_string(&experimental).unwrap();
        assert!(json.starts_with("{\"policies\":["), "json inattendu : {}", json);
        assert!(!json.contains("Policies"), "json inattendu : {}", json);
    }

    #[test]
    fn un_champ_manquant_est_refuse_a_la_lecture() {
        // Les trois champs sont obligatoires, aucun n'a de valeur par defaut.
        assert!(serde_json::from_str::<Policy>("{}").is_err());
        assert!(serde_json::from_str::<Policy>("{\"action\":\"provider.use\"}").is_err());
        assert!(serde_json::from_str::<Policy>(
            "{\"action\":\"provider.use\",\"effect\":\"allow\"}"
        )
        .is_err());
        assert!(serde_json::from_str::<Policy>(
            "{\"effect\":\"deny\",\"resource\":\"*\"}"
        )
        .is_err());
    }

    #[test]
    fn une_action_hors_union_est_refusee() {
        // `Schema.Literals` est un litteral : la casse compte et rien d'autre
        // n'est accepte. Un `String` passerait tous ces cas.
        assert!(serde_json::from_str::<Policy>(
            "{\"action\":\"provider.read\",\"effect\":\"allow\",\"resource\":\"*\"}"
        )
        .is_err());
        assert!(serde_json::from_str::<Policy>(
            "{\"action\":\"Provider.use\",\"effect\":\"allow\",\"resource\":\"*\"}"
        )
        .is_err());
        assert!(serde_json::from_str::<Policy>(
            "{\"action\":\"\",\"effect\":\"allow\",\"resource\":\"*\"}"
        )
        .is_err());
    }

    #[test]
    fn un_effet_hors_union_est_refuse_et_les_deux_chaines_sont_acceptees() {
        for effet in ["\"Allow\"", "\"DENY\"", "\"deny \"", "\"\"" ] {
            let json = format!(
                "{{\"action\":\"provider.use\",\"effect\":{},\"resource\":\"*\"}}",
                effet
            );
            assert!(
                serde_json::from_str::<Policy>(&json).is_err(),
                "effet accepte a tort : {}",
                effet
            );
        }
        for effet in ["allow", "deny"] {
            let json = format!(
                "{{\"action\":\"provider.use\",\"effect\":\"{},\"resource\":\"*\"}}",
                effet
            );
            assert_eq!(
                serde_json::from_str::<Policy>(&json).unwrap().effect,
                Effect::Allow,
                ""
            );
            // le second tour doit donner Deny : on verifie la chaine exacte
            assert!(json.contains(effet));
        }
    }

    #[test]
    fn une_liste_absente_ne_serialise_rien() {
        // `Schema.optional` : la cle disparait, elle ne devient pas `null`.
        let experimental = Experimental { policies: None };
        assert_eq!(serde_json::to_string(&experimental).unwrap(), "{}");
        let relu: Experimental = serde_json::from_str("{}").unwrap();
        assert_eq!(relu.policies, None);
    }

    #[test]
    fn une_liste_vide_survit_et_ne_se_confond_pas_avec_l_absence() {
        // Point central du piege `?` contre `??`. `[] ?? []` vaut `[]` en
        // JavaScript, et `[] && {...}` vaut `{...}` car un tableau vide est
        // truthy. `Some(vec![])` doit donc rester `Some(vec![])` d'un bout a
        // l'autre, et aucun `is_empty()` ne doit la transformer en `None`.
        let experimental: Experimental = serde_json::from_str("{\"policies\":[]}").unwrap();
        assert_eq!(experimental.policies, Some(Vec::new()));
        assert!(experimental.policies.is_some());
        assert_ne!(experimental.policies, None);
        assert_eq!(
            serde_json::to_string(&experimental).unwrap(),
            "{\"policies\":[]}"
        );
    }

    #[test]
    fn une_liste_non_vide_conserve_l_ordre_des_regles() {
        // L'ordre decide du resultat : `policy.ts:38` prend la derniere regle
        // qui correspond. Aucune table triee ne doit etre introduite ici.
        let experimental: Experimental = serde_json::from_str(
            "{\"policies\":[\
             {\"action\":\"provider.use\",\"effect\":\"deny\",\"resource\":\"*\"},\
             {\"action\":\"provider.use\",\"effect\":\"allow\",\"resource\":\"anthropic/*\"}\
             ]}",
        )
        .unwrap();
        let policies = experimental.policies.unwrap();
        assert_eq!(policies.len(), 2);
        assert_eq!(policies[0].effect, Effect::Deny);
        assert_eq!(policies[1].effect, Effect::Allow);
        assert_eq!(policies[1].resource, "anthropic/*");
    }

    #[test]
    fn une_seule_regle_apres_une_liste_vide_ne_perd_pas_sa_place() {
        let experimental = Experimental { policies: Some(vec![regle()]) };
        let relu: Experimental =
            serde_json::from_str(&serde_json::to_string(&experimental).unwrap()).unwrap();
        assert_eq!(relu, experimental);
        assert_eq!(relu.policies.unwrap().len(), 1);
    }

    #[test]
    fn une_regle_se_sert_seule_comme_dans_la_configuration_v1() {
        // `v1/config/config.ts:185` place `Policy` directement dans un tableau,
        // sans passer par `Experimental`. Le type doit donc se lire et
        // s'ecrire seul.
        let policies: Vec<Policy> = serde_json::from_str(
            "[{\"action\":\"provider.use\",\"effect\":\"allow\",\"resource\":\"*\"}]",
        )
        .unwrap();
        assert_eq!(policies, vec![regle()]);
        assert_eq!(
            serde_json::to_string(&policies).unwrap(),
            "[{\"action\":\"provider.use\",\"effect\":\"allow\",\"resource\":\"*\"}]"
        );
    }

    #[test]
    fn une_ressource_vide_est_une_ressource_valide_et_survit() {
        // `resource` est un `Schema.String` : la chaine vide est acceptee.
        // Elle ne doit jamais etre confondue avec une absence.
        let policy: Policy =
            serde_json::from_str("{\"action\":\"provider.use\",\"effect\":\"deny\",\"resource\":\"\"}")
                .unwrap();
        assert_eq!(policy.resource, "");
        assert_eq!(
            serde_json::to_string(&policy).unwrap(),
            "{\"action\":\"provider.use\",\"effect\":\"deny\",\"resource\":\"\"}"
        );
    }

    #[test]
    fn un_motif_a_jokers_est_conserve_sans_normalisation() {
        // Aucune validation ni transformation du motif : il est transmis tel
        // quel a `Wildcard.match`.
        for motif in ["*", "provider/*", "a?b", "**", "  "] {
            let policy = Policy::new(PolicyAction::ProviderUse, Effect::Deny, motif);
            let relue: Policy =
                serde_json::from_str(&serde_json::to_string(&policy).unwrap()).unwrap();
            assert_eq!(relue.resource, motif, "motif altere : {}", motif);
        }
    }

    #[test]
    fn une_propriete_inconnue_est_ignoree_comme_en_typescript() {
        // `Schema.Class` ne refuse pas les champs en trop : pas de
        // `deny_unknown_fields` ici non plus.
        let policy: Policy = serde_json::from_str(
            "{\"action\":\"provider.use\",\"effect\":\"allow\",\"resource\":\"*\",\"inconnu\":1}",
        )
        .unwrap();
        assert_eq!(policy, regle());
        let experimental: Experimental =
            serde_json::from_str("{\"policies\":[],\"experimental\":true}").unwrap();
        assert_eq!(experimental.policies, Some(Vec::new()));
    }

    #[test]
    fn une_policies_qui_n_est_pas_une_liste_est_refusee() {
        assert!(serde_json::from_str::<Experimental>("{\"policies\":{}}").is_err());
        assert!(serde_json::from_str::<Experimental>("{\"policies\":\"*\"}").is_err());
        assert!(serde_json::from_str::<Experimental>("{\"policies\":[1]}").is_err());
        // Une regle dont un champ manque fait echouer toute la liste.
        assert!(serde_json::from_str::<Experimental>(
            "{\"policies\":[{\"action\":\"provider.use\"}]}"
        )
        .is_err());
    }

    #[test]
    fn null_dans_policies_est_lu_comme_une_absence_par_serde_et_la_parite_effect_reste_non_verifiee() {
        // Ce que ce test prouve, c'est le comportement de Serde : `null` se
        // decode en `None` pour un `Option`, sans qu'aucune option ne le
        // demande. Ce qu'il ne prouve PAS, c'est la parite avec
        // `Schema.optional` sans `exact: true`, qui traiterait `null` comme
        // `undefined` : cette hypothese n'a pas pu etre verifiee dans le depot
        // (node_modules absent, aucun test du depot qui decode un `null` contre
        // un `Schema.optional`, et deux indices indirects de sens opposes).
        // Si Effect refuse `null` ici, `{"policies": null}` est une erreur cote
        // TS et une absence ici : c'est le seul ecart possible de ce fichier,
        // et il est assume. Voir "Le point NON verifie" du module.
        let experimental: Experimental = serde_json::from_str("{\"policies\":null}").unwrap();

        assert_eq!(experimental.policies, None);
        assert!(experimental.policies.is_none());
        // Une absence se reecrit sans la cle, jamais avec `null`.
        assert_eq!(serde_json::to_string(&experimental).unwrap(), "{}");
    }

    #[test]
    fn null_au_niveau_du_champ_est_une_absence_mais_null_dans_la_liste_est_refuse() {
        // L'asymetrie que le derive introduit, et le lieu exact ou une
        // correction devra etre appliquee : `null` a la place du champ donne
        // `None`, `null` dans le tableau ne donne pas `Vec<Policy>`.
        let absence: Experimental = serde_json::from_str("{\"policies\":null}").unwrap();
        assert_eq!(absence.policies, None);

        let liste: Result<Experimental, _> = serde_json::from_str("{\"policies\":[null]}");
        assert!(liste.is_err(), "un null dans la liste ne peut pas devenir une regle");
    }

    #[test]
    fn la_valeur_par_defaut_ne_contient_aucune_regle() {
        // `new Experimental({})` donne `policies === undefined` en
        // TypeScript, donc `None` ici et surtout pas `Some(vec![])`.
        let experimental = Experimental::default();
        assert!(experimental.policies.is_none());
        assert_eq!(serde_json::to_string(&experimental).unwrap(), "{}");
    }

    #[test]
    fn la_chaine_de_l_action_est_celle_du_fichier_de_configuration() {
        assert_eq!(PolicyAction::ProviderUse.as_str(), "provider.use");
        assert_eq!(
            serde_json::to_string(&PolicyAction::ProviderUse).unwrap(),
            "\"provider.use\""
        );
        assert_eq!(
            serde_json::from_str::<PolicyAction>("\"provider.use\"").unwrap(),
            PolicyAction::ProviderUse
        );
        // Les identifiants de schema ne sont pas des champs.
        assert_eq!(SCHEMA_IDENTIFIER_POLICY, "ConfigV2.Experimental.Policy");
        assert_eq!(SCHEMA_IDENTIFIER_EXPERIMENTAL, "ConfigV2.Experimental");
    }
}
