//! Portage Rust de `core/src/plugin/models-dev.ts`.
//!
//! Le plugin TS branche le contenu de `models.dev` sur deux sorties :
//! les integrations (methode d'authentification) et le catalogue
//! (fournisseurs + modeles + modeles experimentaux par mode).
//!
//! # D'ou vient le contrat de donnees
//!
//! `ModelsDev` (`core/src/models-dev.ts`) est le service qui fetch
//! `https://models.dev/api.json`. La source de ce module n'etait pas
//! disponible localement ; le contrat a donc ete releve sur la reponse
//! reelle de l'API, qui est la seule source de verite que le plugin
//! consomme. Champs releves :
//!
//! - fournisseur : `id`, `name`, `env[]`, `npm?`, `api?`, `models{}` ;
//! - modele : `id`, `name`, `family?`, `tool_call`, `modalities?{input,output}`,
//!   `release_date`, `limit{context,input,output}`, `cost?`, `status?`,
//!   `provider?{npm,api}`, `experimental?{modes{}}` ;
//! - cout : `input`, `output`, `cache_read?`, `cache_write?`, `tiers?[]`
//!   (chaque palier porte `tier{type,size}`), `context_over_200k?`.
//!
//! # Le piege des casse : ici, tout est deja en snake_case
//!
//! `api.json` est porte par les modeles dev : `cache_read`, `cache_write`,
//! `tool_call`, `release_date`, `context_over_200k` sont **deja** en
//! snake_case dans le JSON TypeScript. Aucun champ camelCase n'existe dans
//! ce contrat, donc aucun `rename` camelCase n'est pose. Le seul renommage
//! du fichier est `tier_type` -> `"type"` dans le palier, herite de
//! [`crate::core::model::CostTier`] (reutilise, jamais redeclare — meme
//! regle que dans `plugin_variant.rs` : les types de contrat sont
//! importes, pas copies). Un test verifie qu'un `cacheRead` camelCase est
//! rejete a la lecture : si un jour l'API passait en camelCase, le port
//! doit casser visiblement, pas silencieusement.
//!
//! # Le piege `?` : verite, pas nullite
//!
//! `model.provider?.npm ? ...` est un test de **verite** sur une chaine :
//! un `npm` present mais vide (`""`) bascule sur l'API `native`. C'est le
//! seul endroit du fichier ou `?` et `??` divergent, et il est testé.
//!
//! # `??` : nullite, partout ailleurs
//!
//! `input?.input ?? 0`, `model.status ?? "active"`,
//! `input.name ?? model.name` : le repli ne fuse que sur l'absence. Les
//! `Option::unwrap_or` du port reproduisent exactement cela.
//!
//! # `Date.parse` et millisecondes
//!
//! `released` renvoie `Date.parse(date)` : des **millisecondes** depuis
//! l'epoch (et non des secondes, contrairement au commentaire de
//! [`crate::core::model::ModelTime`]). Le port garde les millisecondes.
//! `Date.parse` accepte plusieurs formats ; `api.json` n'en emit qu'un
//! (ISO-8601, `YYYY-MM-DD` suivi eventuellement d'une heure et d'un
//! fuseau). Le port n'analyse que cet ISO-8601 : tout le reste vaut `NaN`
//! en TS, donc `0` ici. Algorithme des jours civils : Hinnant.
//!
//! # Ordre d'insertion des cartes
//!
//! `Object.values(...)` en TS itere dans l'ordre d'insertion du JSON. Les
//! fournisseurs, les modeles et les modes experimentaux sont donc portes
//! en `Vec<(String, T)>` avec un deserializeur serde maison (`carte_ordonnee`)
//! qui conserve l'ordre du document — un `BTreeMap` trierait les ids et
//! changerait l'ordre des `update`, observable sur le catalogue.
//!
//! # Ce qui n'est pas porte
//!
//! Le moteur Effect (`Effect.fn`, `Stream.runForEach`, `forkScoped`) et les
//! services (`ModelsDev.Service`, `EventV2.Service`, `ctx.integration.reload`)
//! n'ont pas de sens en Rust autonome : les deux callbacks de transformation
//! sont portes en fonctions pures (`transformation_integrations`,
//! `transformation_catalogue`) et l'abonnement `Refreshed` est laisse au
//! coordinateur, comme dans les autres ports du repertoire.

use std::collections::BTreeMap;
use std::marker::PhantomData;

use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde::ser::Serialize;
use serde_json::Value;

use crate::core::model::{
    empty_info, Cost, CostCache, CostTier, ModelApi, ModelInfo, ModelStatus, AisdkApi, NativeApi,
};

/// Cle d'API distincte des modeles dev, telle qu'emandee par `define`.
pub const ID: &str = "models-dev";

// ---------------------------------------------------------------------
// Deserialisation de carte en ordre d'insertion
// ---------------------------------------------------------------------

/// Deserialise une carte JSON en `Vec<(String, T)>` dans l'ordre du
/// document, pour reproduire `Object.entries` / `Object.values` de TS.
fn carte_ordonnee<'de, D, T>(deserialiseur: D) -> Result<Vec<(String, T)>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Visiteur<T>(PhantomData<T>);

    impl<'de, T: Deserialize<'de>> Visitor<'de> for Visiteur<T> {
        type Value = Vec<(String, T)>;

        fn expecting(&self, formateur: &mut std::fmt::Formatter) -> std::fmt::Result {
            formateur.write_str("une carte JSON")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut acces: A) -> Result<Self::Value, A::Error> {
            let mut entrees = Vec::new();
            while let Some(paire) = acces.next_entry::<String, T>()? {
                entrees.push(paire);
            }
            Ok(entrees)
        }
    }

    deserialiseur.deserialize_map(Visiteur(PhantomData))
}

// ---------------------------------------------------------------------
// Types du contrat models.dev (donnees d'entree, jamais re-emises)
// ---------------------------------------------------------------------

/// Cout brut d'un modele tel qu'publie par `models.dev`.
///
/// En TS : `ModelsDev.Model["cost"]`. Les champs `input`/`output` et les
/// deux champs de cache sont optionnels dans le type TS (le `?? 0` de
/// `cost` le prouve), donc des `Option` ici.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CoutModelsDev {
    pub input: Option<f64>,
    pub output: Option<f64>,
    /// Deja en snake_case dans le JSON source — un `cacheRead` est refuse.
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
    /// Paliers tariferes par taille de contexte, dans l'ordre du document.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiers: Option<Vec<PalierCoutModelsDev>>,
    /// Surcharge tarifaire au-dela de 200k de contexte.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_over_200k: Option<CoutModelsDev>,
}

/// Palier tarifaire : un cout portant son propre `tier{type,size}`.
///
/// Le champ `tier` reutilise [`CostTier`] de `crate::core::model`, qui
/// porte deja le renommage `type` — declare une fois, herite partout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PalierCoutModelsDev {
    pub tier: CostTier,
    pub input: f64,
    pub output: f64,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
}

impl Default for PalierCoutModelsDev {
    fn default() -> Self {
        PalierCoutModelsDev {
            tier: CostTier { tier_type: "context".to_string(), size: 0 },
            input: 0.0,
            output: 0.0,
            cache_read: None,
            cache_write: None,
        }
    }
}

/// Limites de contexte d'un modele (`ModelsDev.Model["limit"]`).
///
/// `input` peut manquer ou valoir `null` dans `api.json`, d'ou l'`Option` ;
/// il traverse tel quel vers `ModelLimit.input` (lui aussi `Option`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitesModelsDev {
    pub context: i64,
    pub input: Option<i64>,
    pub output: i64,
}

/// Modalites d'entree / de sortie (`model.modalities?.input ?? []`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModalitesModelsDev {
    pub input: Vec<String>,
    pub output: Vec<String>,
}

/// Sous-objet `provider` d'un modele : le npm et l'URL d'API du
/// fournisseur, reprojete sur le modele.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FournisseurModeleModelsDev {
    pub npm: Option<String>,
    pub api: Option<String>,
}

/// Entree de `model.experimental.modes` : un cout de remplacement et une
/// requete fournisseur (`headers` / `body`) a fusionner dans le brouillon.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModeModelsDev {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<CoutModelsDev>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<RequeteFournisseurModelsDev>,
}

/// En-tetes et corps de requete d'un mode experimental.
///
/// Les cles de `body` sont opaques (ce sont des corps d'API libres) : la
/// casse y est celle de l'endpoint, pas celle du schema.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RequeteFournisseurModelsDev {
    pub headers: BTreeMap<String, String>,
    pub body: BTreeMap<String, Value>,
}

/// Bloc `experimental` d'un modele.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExperimentalModelsDev {
    /// Carte mode -> options, en ordre d'insertion du document.
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "carte_ordonnee_option"
    )]
    pub modes: Option<Vec<(String, ModeModelsDev)>>,
}

/// Modele d'un fournisseur `models.dev` (`ModelsDev.Model`).
///
/// Seuls les champs que `models-dev.ts` lit sont declares ; le reste
/// (`description`, `reasoning`, `open_weights`, ...) est ignore, comme le
/// laisse faire l'acces par propriete en TS.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ModeleModelsDev {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    pub tool_call: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<ModalitesModelsDev>,
    /// Date ISO-8601 ; `Date.parse` echoue sur tout le reste -> 0.
    pub release_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    pub limit: LimitesModelsDev,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<CoutModelsDev>,
    /// Absent ou `null` -> `"active"` (`model.status ?? "active"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ModelStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<FournisseurModeleModelsDev>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<ExperimentalModelsDev>,
}

/// Fournisseur `models.dev` (`ModelsDev.Provider`).
///
/// `env` vide court-circuite la transformation des integrations. `id` est
/// le champ du document : c'est lui (et non la cle de la carte) qui sert
/// de `providerID` / `integrationID` dans le TS.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FournisseurModelsDev {
    pub id: String,
    pub name: String,
    pub env: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<String>,
    /// Carte modele-id -> modele, en ordre d'insertion du document.
    #[serde(deserialize_with = "carte_ordonnee")]
    pub models: Vec<(String, ModeleModelsDev)>,
}

/// Donnees completes de `models.dev` : la carte fournisseur-id ->
/// fournisseur, en ordre d'insertion.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DonneesModelsDev(pub Vec<(String, FournisseurModelsDev)>);

/// Variante optionnelle de [`carte_ordonnee`] pour un champ `Option`.
fn carte_ordonnee_option<'de, D, T>(deserialiseur: D) -> Result<Option<Vec<(String, T)>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    carte_ordonnee(deserialiseur).map(Some)
}

// ---------------------------------------------------------------------
// Helpers portes du TS
// ---------------------------------------------------------------------

/// Port de `released(date) : number` (lignes 8-11).
///
/// `Date.parse` renvoie des millisecondes ; echec -> `NaN` -> `0`. Le
/// port n'analyse que l'ISO-8601 que `api.json` emit : `YYYY-MM-DD`,
/// eventuellement suivi de `T HH:MM[:SS[.f]]` et d'un fuseau `Z` ou
/// `+HH[:MM]`.
pub fn publier(date: &str) -> f64 {
    let octets = date.as_bytes();
    if octets.len() < 10
        || !octets[0..4].iter().all(u8::is_ascii_digit)
        || octets[4] != b'-'
        || !octets[5..7].iter().all(u8::is_ascii_digit)
        || octets[7] != b'-'
        || !octets[8..10].iter().all(u8::is_ascii_digit)
    {
        return 0.0;
    }
    let annee: i64 = date[0..4].parse().unwrap_or(0);
    let mois: u32 = date[5..7].parse().unwrap_or(0);
    let jour: u32 = date[8..10].parse().unwrap_or(0);
    if !(1..=12).contains(&mois) || !(1..=31).contains(&jour) {
        return 0.0;
    }

    let mut secondes = jours_civils_vers_epoch(annee, mois, jour) * 86_400;
    let mut millisecondes: f64 = 0.0;

    if octets.len() > 10 {
        // Separateur : 'T', 't' ou espace — tous acceptes par Date.parse.
        if !matches!(octets[10], b'T' | b't' | b' ') {
            return 0.0;
        }
        let reste = &date[11..];
        let r = reste.as_bytes();
        if r.len() < 5
            || !r[0..2].iter().all(u8::is_ascii_digit)
            || r[2] != b':'
            || !r[3..5].iter().all(u8::is_ascii_digit)
        {
            return 0.0;
        }
        let heure: i64 = reste[0..2].parse().unwrap_or(0);
        let minute: i64 = reste[3..5].parse().unwrap_or(0);
        let mut indice = 5;
        let mut seconde = 0i64;
        if r.len() >= 8 && r.get(5) == Some(&b':') {
            if !r[6..8].iter().all(u8::is_ascii_digit) {
                return 0.0;
            }
            seconde = reste[6..8].parse().unwrap_or(0);
            indice = 8;
            // Fraction de seconde, ignoree au-dela de la milliseconde.
            if r.get(indice) == Some(&b'.') {
                let debut = indice + 1;
                let mut fin = debut;
                while fin < r.len() && r[fin].is_ascii_digit() {
                    fin += 1;
                }
                if fin == debut {
                    return 0.0;
                }
                let chiffres = &reste[debut..fin.min(debut + 3)];
                millisecondes = format!("0.{chiffres}").parse::<f64>().unwrap_or(0.0);
                indice = fin;
            }
        }
        if !(0..24).contains(&heure) || !(0..60).contains(&minute) || !(0..60).contains(&seconde) {
            return 0.0;
        }
        secondes += heure * 3_600 + minute * 60 + seconde;

        // Fuseau : Z, ou +/-HH[:MM]. Sans fuseau, Date.parse traiterait la
        // chaine en heure locale — indeterministe, dependant de la machine
        // — donc le port choisit UTC, divergence assumee que api.json ne
        // produit jamais (dates nues ou horodatees en Z).
        match r.get(indice) {
            None => {}
            Some(b'Z') | Some(b'z') => {}
            Some(&signe @ (b'+' | b'-')) => {
                let reste_fuseau = &reste[indice + 1..];
                let f = reste_fuseau.as_bytes();
                let (heures, minutes) = match f.len() {
                    2 => match reste_fuseau[0..2].parse::<i64>() {
                        Ok(h) if f.iter().all(u8::is_ascii_digit) => (h, 0),
                        _ => return 0.0,
                    },
                    5 if f[2] == b':' => match (
                        reste_fuseau[0..2].parse::<i64>(),
                        reste_fuseau[3..5].parse::<i64>(),
                    ) {
                        (Ok(h), Ok(m)) if f[0..2].iter().all(u8::is_ascii_digit) && f[3..5].iter().all(u8::is_ascii_digit) => (h, m),
                        _ => return 0.0,
                    },
                    _ => return 0.0,
                };
                let decalage = heures * 3_600 + minutes * 60;
                if signe == b'+' {
                    secondes -= decalage;
                } else {
                    secondes += decalage;
                }
            }
            _ => return 0.0,
        }
    }

    (secondes as f64) * 1_000.0 + millisecondes
}

/// Nombre de jours entre le 1970-01-01 et la date civile donnee
/// (algorithme de Howard Hinnant, `days_from_civil`).
fn jours_civils_vers_epoch(annee: i64, mois: u32, jour: u32) -> i64 {
    let m = mois as i64;
    let y = annee - i64::from(m <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + jour as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Port de `cost(input)` (lignes 13-50) : normalise un cout `models.dev`
/// en `Vec<Cost>` du schema V2.
///
/// Ordre du vecteur : le cout par defaut d'abord (sans `tier`), puis les
/// paliers declares, puis le palier `context_over_200k` s'il existe, avec
/// `tier = {type: "context", size: 200_000}`. Chaque champ de cache absent
/// tombe a `0.0` (`?? 0`, nullite : un `0` present reste un `0`).
pub fn cout(entree: Option<&CoutModelsDev>) -> Vec<Cost> {
    let Some(entree) = entree else {
        return vec![cout_vide()];
    };
    let mut sortie = vec![Cost {
        tier: None,
        input: entree.input.unwrap_or(0.0),
        output: entree.output.unwrap_or(0.0),
        cache: CostCache {
            read: entree.cache_read.unwrap_or(0.0),
            write: entree.cache_write.unwrap_or(0.0),
        },
    }];
    for palier in entree.tiers.iter().flatten() {
        sortie.push(Cost {
            tier: Some(palier.tier.clone()),
            input: palier.input,
            output: palier.output,
            cache: CostCache {
                read: palier.cache_read.unwrap_or(0.0),
                write: palier.cache_write.unwrap_or(0.0),
            },
        });
    }
    if let Some(dela) = &entree.context_over_200k {
        sortie.push(Cost {
            tier: Some(CostTier { tier_type: "context".to_string(), size: 200_000 }),
            input: dela.input.unwrap_or(0.0),
            output: dela.output.unwrap_or(0.0),
            cache: CostCache {
                read: dela.cache_read.unwrap_or(0.0),
                write: dela.cache_write.unwrap_or(0.0),
            },
        });
    }
    sortie
}

/// Le `{ input: 0, output: 0, cache: { read: 0, write: 0 } }` du TS,
/// reutilise comme base de repli dans [`fusionner_cout`].
fn cout_vide() -> Cost {
    Cost {
        tier: None,
        input: 0.0,
        output: 0.0,
        cache: CostCache { read: 0.0, write: 0.0 },
    }
}

/// La cle de palier du TS (ligne 57) : `${tier?.type ?? "base"}:${tier?.size ?? 0}`.
fn cle_palier(cout: &Cost) -> String {
    match &cout.tier {
        Some(palier) => format!("{}:{}", palier.tier_type, palier.size),
        None => format!("base:0"),
    }
}

/// Port de `mergeCost` (lignes 52-70).
///
/// `merge(left, right)` du TS se lit ainsi sur un `Cost` complet :
/// `{...left, ...right}` fait gagner `right` sur `input`/`output`/`cache`
/// (toujours presents apres [`cout`]), puis `tier: right.tier ?? left.tier`
/// retombe sur le palier de gauche quand le droit n'en a pas — c'est le
/// cas du cout par defaut produit par `cout(...)`, qui porte `tier: None`.
/// Les paliers de base conservent leur position ; un palier de droite avec
/// la meme cle est fusionne sur place, un palier inconnu est ajoute a la
/// fin (`Map.set` / iteration d'insertion).
pub fn fusionner_cout(base: Vec<Cost>, remplacement: Option<&CoutModelsDev>) -> Vec<Cost> {
    let Some(remplacement) = remplacement else {
        return base;
    };
    let suivant = cout(Some(remplacement));
    let (defaut_suivant, paliers_suivants) = suivant
        .split_first()
        .expect("cout() emit toujours le cout par defaut en premier");

    let defaut_base = base
        .first()
        .cloned()
        .unwrap_or_else(cout_vide);
    let mut sortie = vec![fusionner(defaut_base, defaut_suivant.clone())];

    let mut paliers: Vec<Cost> = base.into_iter().skip(1).collect();
    for palier in paliers_suivants {
        let cle = cle_palier(palier);
        match paliers.iter().position(|existant| cle_palier(existant) == cle) {
            Some(indice) => {
                let gauche = std::mem::replace(&mut paliers[indice], cout_vide());
                paliers[indice] = fusionner(gauche, palier.clone());
            }
            None => paliers.push(palier.clone()),
        }
    }
    sortie.extend(paliers);
    sortie
}

/// Fusion champ a champ d'un cout de base et d'un cout de remplacement.
fn fusionner(gauche: Cost, droite: Cost) -> Cost {
    Cost {
        tier: droite.tier.or(gauche.tier),
        input: droite.input,
        output: droite.output,
        // {...left.cache, ...right.cache} : apres cout(), les deux caches
        // sont complets, donc celui de droite gagne integralement.
        cache: droite.cache,
    }
}

/// Port de `modeName` (lignes 72-74) : `` `${model.name} ${Mode}` ``
/// avec la premiere lettre du mode en majuscule.
///
/// `toUpperCase` est Unicode : le port utilise `char::to_uppercase`, qui
/// peut produire plusieurs caracteres (ex. `'ß'` -> `"SS"`, comme en TS),
/// puis reprend le reste du mode a partir du caractere suivant.
pub fn nom_mode(modele: &ModeleModelsDev, mode: &str) -> String {
    let majuscule = mode
        .chars()
        .next()
        .map(|premier| premier.to_uppercase().collect::<String>())
        .unwrap_or_default();
    format!("{} {}{}", modele.name, majuscule, &mode[premiere_longueur(mode)..])
}

/// Longueur en octets du premier caractere d'une chaine (0 si vide).
fn premiere_longueur(mode: &str) -> usize {
    mode.chars().next().map_or(0, char::len_utf8)
}

// ---------------------------------------------------------------------
// Transformation du catalogue
// ---------------------------------------------------------------------

/// Port du parametre `input` de `applyModel` (lignes 79-83).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntreeModele {
    /// `name?: string` — absent -> `model.name`.
    pub nom: Option<String>,
    /// `cost?: ModelV2Info["cost"]` — absent -> `cost(model.cost)`.
    pub cout: Option<Vec<Cost>>,
    /// `request?` — en-tetes et corps fusionnes dans `draft.request`.
    pub requete: Option<RequeteFournisseurModelsDev>,
}

/// Port de `applyModel` (lignes 76-117) : projette un modele `models.dev`
/// sur un brouillon `ModelV2Info`, champ par champ, dans l'ordre du TS.
pub fn appliquer_modele(
    brouillon: &mut ModelInfo,
    modele: &ModeleModelsDev,
    entree: EntreeModele,
) {
    brouillon.name = entree.nom.unwrap_or_else(|| modele.name.clone());
    brouillon.family = modele.family.clone();

    // `model.provider?.npm ? {aisdk} : {native}` — test de VERITE sur la
    // chaine : un `npm` = "" est present mais falsy, donc `native`.
    let npm = modele.provider.as_ref().and_then(|p| p.npm.as_deref());
    brouillon.api = match npm {
        Some(npm) if !npm.is_empty() => ModelApi::Aisdk(AisdkApi {
            id: modele.id.clone(),
            package: npm.to_string(),
            url: modele.provider.as_ref().and_then(|p| p.api.clone()),
            settings: None,
        }),
        _ => ModelApi::Native(NativeApi {
            id: modele.id.clone(),
            url: modele.provider.as_ref().and_then(|p| p.api.clone()),
            settings: BTreeMap::new(),
        }),
    };

    brouillon.capabilities = crate::core::model::Capabilities {
        tools: modele.tool_call,
        input: modele.modalities.as_ref().map_or(Vec::new(), |m| m.input.clone()),
        output: modele.modalities.as_ref().map_or(Vec::new(), |m| m.output.clone()),
    };
    brouillon.variants = Vec::new();
    brouillon.time.released = publier(&modele.release_date);
    brouillon.cost = entree
        .cout
        .unwrap_or_else(|| cout(modele.cost.as_ref()));
    brouillon.status = modele.status.clone().unwrap_or(ModelStatus::Active);
    brouillon.enabled = true;
    brouillon.limit = crate::core::model::ModelLimit {
        context: modele.limit.context,
        input: modele.limit.input,
        output: modele.limit.output,
    };
    if let Some(requete) = &entree.requete {
        brouillon.request.headers.extend(requete.headers.clone());
        brouillon.request.body.extend(requete.body.clone());
    }
}

/// Catalogue minimal cible de `ctx.catalog.transform` : un fournisseur
/// porte un nom, une API et ses modeles, dans l'ordre d'insertion.
#[derive(Debug, Clone, PartialEq)]
pub struct FournisseurCatalogue {
    pub id: String,
    pub name: String,
    pub api: ModelApi,
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalogue {
    pub fournisseurs: Vec<FournisseurCatalogue>,
}

impl Catalogue {
    /// Port de `catalog.provider.update` (lignes 147-160) : cree ou met a
    /// jour le nom et l'API d'un fournisseur.
    pub fn maj_fournisseur(&mut self, id: &str, nom: &str, npm: Option<&str>, api: Option<&str>) {
        let api_fournisseur = match npm {
            // Meme test de verite que dans appliquer_modele.
            Some(npm) if !npm.is_empty() => ModelApi::Aisdk(AisdkApi {
                id: id.to_string(),
                package: npm.to_string(),
                url: api.map(str::to_string),
                settings: None,
            }),
            _ => ModelApi::Native(NativeApi {
                id: id.to_string(),
                url: api.map(str::to_string),
                settings: BTreeMap::new(),
            }),
        };
        match self.fournisseurs.iter_mut().find(|f| f.id == id) {
            Some(fournisseur) => {
                fournisseur.name = nom.to_string();
                fournisseur.api = api_fournisseur;
            }
            None => self.fournisseurs.push(FournisseurCatalogue {
                id: id.to_string(),
                name: nom.to_string(),
                api: api_fournisseur,
                models: Vec::new(),
            }),
        }
    }

    /// Port de `catalog.model.update` : renvoie le brouillon du modele,
    /// en le creant depuis `Info.empty` s'il manque.
    pub fn maj_modele(&mut self, provider_id: &str, model_id: &str) -> &mut ModelInfo {
        let indice = match self
            .fournisseurs
            .iter()
            .position(|f| f.id == provider_id)
        {
            Some(indice) => indice,
            None => {
                self.fournisseurs.push(FournisseurCatalogue {
                    id: provider_id.to_string(),
                    name: provider_id.to_string(),
                    api: ModelApi::Native(NativeApi {
                        id: provider_id.to_string(),
                        url: None,
                        settings: BTreeMap::new(),
                    }),
                    models: Vec::new(),
                });
                self.fournisseurs.len() - 1
            }
        };
        let modeles = &mut self.fournisseurs[indice].models;
        let position = match modeles.iter().position(|m| m.id == model_id) {
            Some(position) => position,
            None => {
                modeles.push(empty_info(provider_id, model_id));
                modeles.len() - 1
            }
        };
        &mut modeles[position]
    }
}

/// Port du corps de `ctx.catalog.transform` (lignes 142-177).
///
/// Pour chaque fournisseur : `provider.update` (nom + API), puis pour
/// chaque modele : un modele de base (avec `cost` recalcule et reinjecte,
/// line 164) puis un modele par mode experimental, d'identifiant
/// `` `${model.id}-${mode}` `` et de cout passe par [`fusionner_cout`].
pub fn transformation_catalogue(donnees: &DonneesModelsDev, catalogue: &mut Catalogue) {
    for (_cle, item) in &donnees.0 {
        catalogue.maj_fournisseur(&item.id, &item.name, item.npm.as_deref(), item.api.as_deref());

        for (_cle_modele, modele) in &item.models {
            let cout_base = cout(modele.cost.as_ref());
            let brouillon = catalogue.maj_modele(&item.id, &modele.id);
            appliquer_modele(
                brouillon,
                modele,
                EntreeModele { nom: None, cout: Some(cout_base.clone()), requete: None },
            );

            if let Some(modes) = modele.experimental.as_ref().and_then(|e| e.modes.as_ref()) {
                for (mode, options) in modes {
                    let brouillon =
                        catalogue.maj_modele(&item.id, &format!("{}-{}", modele.id, mode));
                    appliquer_modele(
                        brouillon,
                        modele,
                        EntreeModele {
                            nom: Some(nom_mode(modele, mode)),
                            cout: Some(fusionner_cout(cout_base.clone(), options.cost.as_ref())),
                            requete: options.provider.clone(),
                        },
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------
// Transformation des integrations
// ---------------------------------------------------------------------

/// Etat final d'une integration apres la premiere transformation du TS
/// (lignes 124-140) : `update` du nom, puis `method.update` `{type:"key"}`
/// immediatement remplace par `{type:"env", names}` — seul l'etat final
/// est observable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MajIntegration {
    /// `item.id` — le champ du document, pas la cle de la carte.
    pub integration_id: String,
    pub name: String,
    pub env_names: Vec<String>,
}

/// Port du corps de `ctx.integration.transform`.
///
/// `if (item.env.length === 0) continue` : les fournisseurs sans variable
/// d'environnement sont sautes, les autres produisent une integration dont
/// la methode finale est `{type: "env", names: [...item.env]}`.
pub fn transformation_integrations(donnees: &DonneesModelsDev) -> Vec<MajIntegration> {
    donnees
        .0
        .iter()
        .filter(|(_cle, item)| !item.env.is_empty())
        .map(|(_cle, item)| MajIntegration {
            integration_id: item.id.clone(),
            name: item.name.clone(),
            env_names: item.env.clone(),
        })
        .collect()
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -------------------------------------------------- le contrat JSON

    #[test]
    fn les_cles_du_contrat_sont_deja_en_snake_case() {
        // `api.json` est en snake_case : cache_read, cache_write, tool_call,
        // release_date, context_over_200k. Le port lit ces cles et refuse
        // les variantes camelCase : si l'API change de casse, ca casse ici.
        let cout: CoutModelsDev = serde_json::from_str(
            r#"{"input":3,"output":15,"cache_read":0.3,"cache_write":3.75}"#,
        )
        .unwrap();
        assert_eq!(cout.cache_read, Some(0.3));
        assert_eq!(cout.cache_write, Some(3.75));

        let camel: Result<CoutModelsDev, _> =
            serde_json::from_str(r#"{"input":3,"output":15,"cacheRead":0.3}"#);
        assert!(camel.is_err(), "cacheRead camelCase doit etre refuse");
    }

    #[test]
    fn le_palier_renomme_type_et_round_trip() {
        // `tier` porte un champ discrimine `type`, porte par CostTier herite
        // de crate::core::model — declare la bas, jamais ici.
        let palier: PalierCoutModelsDev =
            serde_json::from_str(r#"{"tier":{"type":"context","size":32000},"input":5,"output":15}"#)
                .unwrap();
        assert_eq!(palier.tier.tier_type, "context");
        assert_eq!(palier.tier.size, 32_000);
        assert_eq!(
            serde_json::to_value(&palier.tier).unwrap(),
            json!({"type":"context","size":32000})
        );
    }

    #[test]
    fn les_cartes_conservent_l_ordre_d_insertion() {
        // Un BTreeMap trierait ; Object.values itere dans l'ordre du JSON.
        let donnees: DonneesModelsDev = serde_json::from_str(
            r#"{"zeta":{"id":"zeta","name":"Zeta","env":[],"models":{}},
                "alpha":{"id":"alpha","name":"Alpha","env":[],"models":{}}}"#,
        )
        .unwrap();
        let cles: Vec<&str> = donnees.0.iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(cles, ["zeta", "alpha"]);

        let modele: ModeleModelsDev = serde_json::from_str(
            r#"{"id":"m","name":"M","tool_call":false,"release_date":"2020-01-01",
                "limit":{"context":1,"output":1},
                "experimental":{"modes":{"zulu":{},"alpha":{}}}}"#,
        )
        .unwrap();
        let modes = modele.experimental.unwrap().modes.unwrap();
        let noms: Vec<&str> = modes.iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(noms, ["zulu", "alpha"]);
    }

    // ------------------------------------------------------ publier()

    #[test]
    fn publier_renvoie_des_millisecondes_comme_date_parse() {
        // Date.parse("2026-07-06") -> ms depuis l'epoch minuit UTC.
        assert_eq!(publier("1970-01-01"), 0.0);
        assert_eq!(publier("1970-01-02"), 86_400_000.0);
        assert_eq!(publier("2026-07-06"), 1_783_296_000_000.0);
    }

    #[test]
    fn publier_analyse_heure_et_fuseau() {
        // 2026-07-06T00:00:00Z est 86_400_000 ms apres le 2026-07-05.
        let z = publier("2026-07-06T00:00:00Z");
        assert_eq!(z, 1_783_296_000_000.0);
        // +02:00 enleve deux heures ; -02:00 en ajoute deux.
        assert_eq!(publier("2026-07-06T00:00:00+02:00"), 1_783_296_000_000.0 - 7_200_000.0);
        assert_eq!(publier("2026-07-06T00:00:00-02:00"), 1_783_296_000_000.0 + 7_200_000.0);
        assert_eq!(publier("2026-07-06T12:30:45.500Z"), z + 45_045_500.0);
    }

    #[test]
    fn publier_renvoie_zero_sur_une_date_invalide() {
        // Number.isFinite(NaN) est faux -> 0, pour toute chaine hors ISO.
        for invalide in ["", "pas-une-date", "2026-13-01", "2026-00-10", "2026-02-30x"] {
            assert_eq!(publier(invalide), 0.0, "{invalide} devrait valoir 0");
        }
        // 2026-02-30 est accepte comme un jour de plus (Date.parse tolerant)
        // : le port valide la forme, pas le calendrier, mais une heure
        // hors bornes est refusee comme NaN l'est.
        assert_eq!(publier("2026-07-06T25:00:00Z"), 0.0);
    }

    // ------------------------------------------------------ cout()

    #[test]
    fn cout_par_defaut_zero_sur_champs_absents() {
        // `input?.input ?? 0` : nullite, donc un 0 present reste 0.
        let cout = cout(Some(&serde_json::from_str(r#"{"input":3,"output":15}"#).unwrap()));
        assert_eq!(cout.len(), 1);
        assert_eq!(cout[0].input, 3.0);
        assert_eq!(cout[0].cache.read, 0.0);
        assert_eq!(cout[0].cache.write, 0.0);
        assert!(cout[0].tier.is_none());

        let nul = cout(None);
        assert_eq!(nul.len(), 1);
        assert_eq!(nul[0], cout_vide());
    }

    #[test]
    fn cout_porduit_les_paliers_puis_le_palier_200k() {
        let brut: CoutModelsDev = serde_json::from_str(
            r#"{"input":1,"output":2,"cache_read":0.1,"cache_write":0.2,
                "tiers":[{"tier":{"type":"context","size":32000},"input":5,"output":15}],
                "context_over_200k":{"input":4,"output":18}}"#,
        )
        .unwrap();
        let c = cout(Some(&brut));
        assert_eq!(c.len(), 3);
        assert_eq!(c[1].tier.as_ref().unwrap().size, 32_000);
        assert_eq!(c[1].cache.read, 0.0, "cache absent du palier -> 0");
        let dela = c[2].tier.as_ref().unwrap();
        assert_eq!(dela.tier_type, "context");
        assert_eq!(dela.size, 200_000);
        assert_eq!(c[2].input, 4.0);
    }

    // ----------------------------------------------- fusionner_cout()

    #[test]
    fn fusionner_sans_remplacement_rend_la_base() {
        let base = cout(None);
        assert_eq!(fusionner_cout(base.clone(), None), base);
    }

    #[test]
    fn fusionner_garde_le_palier_de_gauche_quand_le_defaut_n_en_a_pas() {
        // Le cout par defaut du remplacement porte tier: None, donc
        // `right.tier ?? left.tier` retombe sur le palier de la base.
        let base = vec![Cost {
            tier: Some(CostTier { tier_type: "priorite".to_string(), size: 1 }),
            input: 1.0,
            output: 2.0,
            cache: CostCache { read: 0.0, write: 0.0 },
        }];
        let remplacement: CoutModelsDev =
            serde_json::from_str(r#"{"input":8,"output":40}"#).unwrap();
        let fusion = fusionner_cout(base, Some(&remplacement));
        assert_eq!(fusion.len(), 1);
        assert_eq!(fusion[0].tier.as_ref().unwrap().tier_type, "priorite");
        assert_eq!(fusion[0].input, 8.0);
    }

    #[test]
    fn fusionner_garde_l_ordre_des_paliers_et_ajoute_les_nouveaux_a_la_fin() {
        let brut: CoutModelsDev = serde_json::from_str(
            r#"{"input":1,"output":2,
                "tiers":[{"tier":{"type":"context","size":32000},"input":5,"output":15}]}"#,
        )
        .unwrap();
        let base = cout(Some(&brut));
        let remplacement: CoutModelsDev = serde_json::from_str(
            r#"{"input":8,"output":40,
                "tiers":[
                    {"tier":{"type":"context","size":32000},"input":50,"output":150},
                    {"tier":{"type":"context","size":128000},"input":60,"output":160}
                ]}"#,
        )
        .unwrap();
        let fusion = fusionner_cout(base, Some(&remplacement));

        assert_eq!(fusion.len(), 3, "defaut + 32000 fusionne + 128000 ajoute");
        assert_eq!(cle_palier(&fusion[1]), "context:32000");
        assert_eq!(fusion[1].input, 50.0, "le palier de droite gagne");
        assert_eq!(cle_palier(&fusion[2]), "context:128000");
        assert_eq!(cle_palier(&fusion[0]), "base:0");
    }

    // ------------------------------------------------------ nom_mode()

    #[test]
    fn nom_mode_majuscule_la_premiere_lettre() {
        let modele = ModeleModelsDev { name: "Claude".to_string(), ..Default::default() };
        assert_eq!(nom_mode(&modele, "fast"), "Claude Fast");
        assert_eq!(nom_mode(&modele, "thinking"), "Claude Thinking");
        // Le reste du mode n'est pas touche.
        assert_eq!(nom_mode(&modele, "xLLM"), "Claude XLLM");
        // toUpperCase est Unicode : 'ß' -> "SS", comme en TS.
        assert_eq!(nom_mode(&modele, "ßeta"), "Claude SSeta");
        assert_eq!(nom_mode(&modele, ""), "Claude ");
    }

    // ----------------------------------------------- appliquer_modele()

    fn modele_exemple() -> ModeleModelsDev {
        serde_json::from_str(
            r#"{
                "id": "glm-5.2",
                "name": "GLM 5.2",
                "family": "glm",
                "tool_call": true,
                "modalities": {"input": ["text"], "output": ["text"]},
                "release_date": "2026-07-06",
                "limit": {"context": 262144, "input": 192000, "output": 65536},
                "cost": {"input": 3, "output": 15, "cache_read": 0.3, "cache_write": 3.75},
                "status": "beta",
                "provider": {"npm": "@ai-sdk/anthropic", "api": "https://api.example.com"}
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn appliquer_modele_produit_le_miroir_du_schema_v2() {
        let modele = modele_exemple();
        let mut brouillon = empty_info("anthropic", "glm-5.2");
        appliquer_modele(&mut brouillon, &modele, EntreeModele::default());

        assert_eq!(brouillon.name, "GLM 5.2");
        assert_eq!(brouillon.family.as_deref(), Some("glm"));
        match &brouillon.api {
            ModelApi::Aisdk(api) => {
                assert_eq!(api.id, "glm-5.2");
                assert_eq!(api.package, "@ai-sdk/anthropic");
                assert_eq!(api.url.as_deref(), Some("https://api.example.com"));
            }
            autre => panic!("attendait aisdk, eu {autre:?}"),
        }
        assert!(brouillon.capabilities.tools);
        assert_eq!(brouillon.capabilities.input, ["text"]);
        assert_eq!(brouillon.variants.len(), 0, "variants est vide dans le TS");
        assert_eq!(brouillon.time.released, 1_783_296_000_000.0);
        assert_eq!(brouillon.cost.len(), 1);
        assert_eq!(brouillon.cost[0].input, 3.0);
        assert!(matches!(brouillon.status, ModelStatus::Beta));
        assert!(brouillon.enabled);
        assert_eq!(brouillon.limit.context, 262_144);
        assert_eq!(brouillon.limit.input, Some(192_000));
        assert_eq!(brouillon.limit.output, 65_536);
    }

    #[test]
    fn un_npm_vide_bascule_sur_native_le_piege_de_verite() {
        // `model.provider?.npm ? ... : ...` : "" est present mais falsy.
        let mut modele = modele_exemple();
        modele.provider.as_mut().unwrap().npm = Some(String::new());
        let mut brouillon = empty_info("anthropic", "glm-5.2");
        appliquer_modele(&mut brouillon, &modele, EntreeModele::default());

        match &brouillon.api {
            ModelApi::Native(native) => {
                assert_eq!(native.id, "glm-5.2");
                assert_eq!(native.url.as_deref(), Some("https://api.example.com"));
                assert!(native.settings.is_empty());
            }
            autre => panic!("attendait native, eu {autre:?}"),
        }
        // Pas de provider du tout : meme branche, url absente.
        modele.provider = None;
        let mut brouillon = empty_info("anthropic", "glm-5.2");
        appliquer_modele(&mut brouillon, &modele, EntreeModele::default());
        match &brouillon.api {
            ModelApi::Native(native) => assert_eq!(native.url, None),
            autre => panic!("attendait native, eu {autre:?}"),
        }
    }

    #[test]
    fn statut_absent_vaut_active_et_cout_d_entree_est_reutilise() {
        let mut modele = modele_exemple();
        modele.status = None;
        modele.modalities = None;
        let mut brouillon = empty_info("anthropic", "glm-5.2");
        appliquer_modele(&mut brouillon, &modele, EntreeModele::default());

        assert!(matches!(brouillon.status, ModelStatus::Active), "`?? \"active\"`");
        assert!(brouillon.capabilities.input.is_empty(), "`?? []`");
        // input.cost ?? cost(model.cost) : ici input.cost fourni.
        let remise = vec![Cost {
            tier: None,
            input: 99.0,
            output: 99.0,
            cache: CostCache { read: 0.0, write: 0.0 },
        }];
        appliquer_modele(
            &mut brouillon,
            &modele,
            EntreeModele { cout: Some(remise), ..Default::default() },
        );
        assert_eq!(brouillon.cost[0].input, 99.0);
    }

    #[test]
    fn la_requete_d_entree_est_fusionnee_et_pas_remplacee() {
        // Object.assign(draft.request.headers, input.request?.headers ?? {}).
        let mut modele = modele_exemple();
        modele.limit.input = None;
        let mut brouillon = empty_info("anthropic", "glm-5.2");
        brouillon.request.headers.insert("X-Base".to_string(), "1".to_string());
        brouillon.request.body.insert("temperature".to_string(), json!(1));
        appliquer_modele(
            &mut brouillon,
            &modele,
            EntreeModele {
                requete: Some(RequeteFournisseurModelsDev {
                    headers: BTreeMap::from([("anthropic-beta".to_string(), "fast".to_string())]),
                    body: BTreeMap::from([("speed".to_string(), json!("fast"))]),
                }),
                ..Default::default()
            },
        );
        assert_eq!(brouillon.request.headers.get("X-Base").map(String::as_str), Some("1"));
        assert_eq!(brouillon.request.body.get("temperature"), Some(&json!(1)));
        assert_eq!(brouillon.request.body.get("speed"), Some(&json!("fast")));
        // limit.input null traverse tel quel (Option des deux cotes).
        assert_eq!(brouillon.limit.input, None);
    }

    #[test]
    fn le_brouillon_s_serialise_avec_les_cles_du_schema_v2() {
        let modele = modele_exemple();
        let mut brouillon = empty_info("anthropic", "glm-5.2");
        appliquer_modele(&mut brouillon, &modele, EntreeModele::default());
        let json = serde_json::to_value(&brouillon).unwrap();

        assert_eq!(json["providerID"], json!("anthropic"));
        assert_eq!(json["api"]["type"], json!("aisdk"));
        assert_eq!(json["api"]["package"], json!("@ai-sdk/anthropic"));
        assert_eq!(json["capabilities"]["tools"], json!(true));
        assert_eq!(json["time"]["released"], json!(1_783_296_000_000.0));
        assert_eq!(json["cost"][0]["cache"]["read"], json!(0.3));
        assert_eq!(json["status"], json!("beta"));
        assert_eq!(json["enabled"], json!(true));
        assert_eq!(json["limit"]["context"], json!(262144));
        // Aller-retour complet.
        let retour: ModelInfo = serde_json::from_value(json).unwrap();
        assert_eq!(retour, brouillon);
    }

    // ------------------------------------ transformation_catalogue()

    #[test]
    fn le_catalogue_recoit_fournisseurs_modeles_et_modes() {
        let donnees: DonneesModelsDev = serde_json::from_str(
            r#"{
                "anthropic": {
                    "id": "anthropic",
                    "name": "Anthropic",
                    "env": ["ANTHROPIC_API_KEY"],
                    "npm": "@ai-sdk/anthropic",
                    "api": "https://api.anthropic.com",
                    "models": {
                        "claude": {
                            "id": "claude",
                            "name": "Claude",
                            "tool_call": true,
                            "release_date": "2026-01-01",
                            "limit": {"context": 200000, "output": 64000},
                            "cost": {"input": 1, "output": 2},
                            "experimental": {
                                "modes": {
                                    "fast": {
                                        "cost": {"input": 8, "output": 40},
                                        "provider": {
                                            "headers": {"anthropic-beta": "fast-mode"},
                                            "body": {"speed": "fast"}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        let mut catalogue = Catalogue::default();
        transformation_catalogue(&donnees, &mut catalogue);

        assert_eq!(catalogue.fournisseurs.len(), 1);
        let fournisseur = &catalogue.fournisseurs[0];
        assert_eq!(fournisseur.id, "anthropic");
        assert_eq!(fournisseur.name, "Anthropic");
        match &fournisseur.api {
            ModelApi::Aisdk(api) => assert_eq!(api.package, "@ai-sdk/anthropic"),
            autre => panic!("attendait aisdk, eu {autre:?}"),
        }

        assert_eq!(fournisseur.models.len(), 2, "le modele de base + son mode");
        assert_eq!(fournisseur.models[0].id, "claude");
        assert_eq!(fournisseur.models[0].cost[0].input, 1.0);
        assert_eq!(fournisseur.models[1].id, "claude-fast");
        assert_eq!(fournisseur.models[1].name, "Claude Fast");
        assert_eq!(fournisseur.models[1].cost[0].input, 8.0);
        // mergeCost(base, {input:8}) : le palier de droite gagne le default.
        assert_eq!(fournisseur.models[1].cost[0].output, 40.0);
        assert_eq!(
            fournisseur.models[1].request.headers.get("anthropic-beta").map(String::as_str),
            Some("fast-mode")
        );
        assert_eq!(fournisseur.models[1].request.body.get("speed"), Some(&json!("fast")));
    }

    #[test]
    fn maj_modele_cree_un_brouillon_vide_quand_le_fournisseur_manque() {
        let mut catalogue = Catalogue::default();
        let brouillon = catalogue.maj_modele("autre", "m");
        assert!(matches!(brouillon.api, ModelApi::Native(_)));
        assert_eq!(brouillon.id, "m");
        assert_eq!(catalogue.fournisseurs.len(), 1);
    }

    // --------------------------------- transformation_integrations()

    #[test]
    fn les_fournisseurs_sans_env_sont_sautes() {
        let donnees: DonneesModelsDev = serde_json::from_str(
            r#"{
                "vide": {"id":"vide","name":"Vide","env":[],"models":{}},
                "plein": {"id":"plein","name":"Plein","env":["CLE_A","CLE_B"],"models":{}}
            }"#,
        )
        .unwrap();
        let majs = transformation_integrations(&donnees);
        assert_eq!(majs.len(), 1);
        assert_eq!(majs[0].integration_id, "plein");
        assert_eq!(majs[0].name, "Plein");
        assert_eq!(majs[0].env_names, ["CLE_A", "CLE_B"]);
    }

    #[test]
    fn l_id_de_integration_vient_du_champ_et_pas_de_la_cle() {
        // integrationID = item.id, jamais la cle de la carte.
        let donnees: DonneesModelsDev = serde_json::from_str(
            r#"{"cle":{"id":"champ","name":"N","env":["E"],"models":{}}}"#,
        )
        .unwrap();
        assert_eq!(transformation_integrations(&donnees)[0].integration_id, "champ");
    }
}
