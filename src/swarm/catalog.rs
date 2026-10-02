//! Portage de `packages/core/src/catalog.ts`.
//!
//! La source fait deux cent soixante lignes : un catalogue de fournisseurs et
//! de modeles construit sur `Effect`, avec un etat mutable (`State`), des
//! couches (`Layer`), des evenements et des politiques d acces. Aucun test
//! TypeScript.
//!
//! Sans `Effect` ni couches, seul le comportement pur est porte : la forme
//! des donnees, la disponibilite d un fournisseur, la projection d un modele
//! sur son fournisseur (fusion des en-tetes, corps et URL d API), la
//! normalisation `baseURL`, le tri par date de sortie, le choix du modele par
//! defaut et la selection du petit modele (`SMALL_MODEL_RE`, cout et age).
//!
//! Les acces reseau, les politiques et les evenements ne sont pas portes :
//! ils relevent de l executant, pas du contrat de donnees.

use std::collections::{HashMap, HashSet};

/// Les actions de politique connues : la seule est `provider.use`.
pub const POLICY_ACTIONS: [&str; 1] = ["provider.use"];

/// Le couple fournisseur + modele par defaut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultModel {
    /// Identifiant du fournisseur.
    pub provider_id: String,
    /// Identifiant du modele.
    pub model_id: String,
}

impl DefaultModel {
    /// Construit le couple par defaut.
    pub fn new(provider_id: String, model_id: String) -> Self {
        Self { provider_id, model_id }
    }
}

/// L API d un fournisseur ou d un modele, reduite aux champs fusionnes par
/// la source : `type`, `id`, `url` et `settings`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Api {
    /// `native` ou `aisdk` dans la source ; conserve en texte.
    pub kind: String,
    /// Identifiant d API.
    pub api_id: String,
    /// URL d API, si presente.
    pub url: Option<String>,
    /// Reglages d API.
    pub settings: HashMap<String, String>,
}

impl Api {
    /// Construit une API.
    pub fn new(kind: &str, api_id: &str, url: Option<String>, settings: HashMap<String, String>) -> Self {
        Self { kind: kind.to_string(), api_id: api_id.to_string(), url, settings }
    }
}

/// La requete d un fournisseur ou d un modele : en-tetes, corps et variante.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Requete {
    /// En-tetes HTTP.
    pub headers: HashMap<String, String>,
    /// Corps JSON reduit a des chaines, comme dans la source.
    pub body: HashMap<String, String>,
    /// Variante de requete, si presente.
    pub variant: Option<String>,
}

impl Requete {
    /// Construit une requete.
    pub fn new(headers: HashMap<String, String>, body: HashMap<String, String>, variant: Option<String>) -> Self {
        Self { headers, body, variant }
    }
}

/// Un fournisseur, reduit aux champs lus par le catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInfo {
    /// Identifiant du fournisseur.
    pub id: String,
    /// `disabled` de la source.
    pub disabled: bool,
    /// Integration rattachee, si presente.
    pub integration_id: Option<String>,
    /// API du fournisseur.
    pub api: Api,
    /// Requete du fournisseur.
    pub request: Requete,
}

impl ProviderInfo {
    /// Construit un fournisseur minimal.
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            disabled: false,
            integration_id: None,
            api: Api::new("aisdk", "openai", None, HashMap::new()),
            request: Requete::default(),
        }
    }
}

/// Un modele, reduit aux champs lus par le catalogue.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelInfo {
    /// Identifiant du modele.
    pub id: String,
    /// Fournisseur proprietaire.
    pub provider_id: String,
    /// `enabled` de la source.
    pub enabled: bool,
    /// `status` (`active`, ...).
    pub status: String,
    /// API du modele.
    pub api: Api,
    /// Requete du modele.
    pub request: Requete,
    /// Date de sortie en millisecondes depuis l epoch.
    pub released: i64,
    /// Cout d entree, si connu.
    pub cost_input: Option<f64>,
    /// Cout de sortie, si connu.
    pub cost_output: Option<f64>,
    /// Capacites d entree (`text`, ...).
    pub input: Vec<String>,
    /// Capacites de sortie.
    pub output: Vec<String>,
    /// Famille du modele, si connue.
    pub family: Option<String>,
    /// Nom d affichage.
    pub name: String,
}

impl ModelInfo {
    /// Construit un modele minimal actif.
    pub fn new(provider_id: &str, id: &str, released: i64) -> Self {
        Self {
            id: id.to_string(),
            provider_id: provider_id.to_string(),
            enabled: true,
            status: "active".to_string(),
            api: Api::new("aisdk", "openai", None, HashMap::new()),
            request: Requete::default(),
            released,
            cost_input: None,
            cost_output: None,
            input: vec!["text".to_string()],
            output: vec!["text".to_string()],
            family: None,
            name: id.to_string(),
        }
    }
}

/// Une entree du catalogue : un fournisseur et ses modeles.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProviderRecord {
    /// Le fournisseur.
    pub provider: ProviderInfo,
    /// Les modeles indexes par identifiant.
    pub models: HashMap<String, ModelInfo>,
}

/// Le catalogue en memoire : l image du `State` de la source.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CatalogData {
    /// Fournisseurs indexes par identifiant.
    pub providers: HashMap<String, ProviderRecord>,
    /// Modele par defaut, si defini.
    pub default_model: Option<DefaultModel>,
}

impl CatalogData {
    /// Cree un catalogue vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Liste les entrees, comme `draft.provider.list()`.
    pub fn lister(&self) -> Vec<&ProviderRecord> {
        self.providers.values().collect()
    }

    /// Rend le fournisseur demande, si present.
    pub fn fournisseur(&self, provider_id: &str) -> Option<&ProviderInfo> {
        self.providers.get(provider_id).map(|r| &r.provider)
    }

    /// Supprime un fournisseur et tous ses modeles.
    pub fn retirer_fournisseur(&mut self, provider_id: &str) {
        self.providers.remove(provider_id);
    }

    /// Rend le modele demande, si present.
    pub fn modele(&self, provider_id: &str, model_id: &str) -> Option<&ModelInfo> {
        self.providers.get(provider_id)?.models.get(model_id)
    }

    /// Supprime un modele.
    pub fn retirer_modele(&mut self, provider_id: &str, model_id: &str) {
        if let Some(record) = self.providers.get_mut(provider_id) {
            record.models.remove(model_id);
        }
    }

    /// Definit le modele par defaut, comme `draft.model.default.set()`.
    pub fn definir_defaut(&mut self, provider_id: &str, model_id: &str) {
        self.default_model = Some(DefaultModel::new(provider_id.to_string(), model_id.to_string()));
    }

    /// Tous les modeles projetes sur leur fournisseur, tries par date de
    /// sortie decroissante, comme `model.all()`.
    pub fn tous_les_modeles(&self) -> Vec<ModelInfo> {
        let mut liste: Vec<ModelInfo> = self
            .providers
            .values()
            .flat_map(|record| record.models.values().map(|m| projeter_modele(m, &record.provider)))
            .collect();
        liste.sort_by(|a, b| b.released.cmp(&a.released));
        liste
    }
}

/// Disponibilite d un fournisseur, comme `available()` dans la source :
/// un fournisseur desactive ne l est jamais ; sinon il faut soit une cle
/// d API en clair, soit des connexions d integration, soit l absence a la
/// fois d integration requise et d integration active.
pub fn fournisseur_disponible(
    disabled: bool,
    corps_a_une_cle_en_clair: bool,
    connexions_integration: usize,
    integration_requise: Option<&str>,
    integration_active_presente: bool,
) -> bool {
    if disabled {
        return false;
    }
    if corps_a_une_cle_en_clair {
        return true;
    }
    if connexions_integration > 0 {
        return true;
    }
    integration_requise.is_none() && !integration_active_presente
}

/// Deplace `baseURL` du corps vers `api.url`, comme `normalizeApi`.
///
/// Sans effet quand le corps ne contient pas de `baseURL` textuel.
pub fn normaliser_api_url(body: &mut HashMap<String, String>, api_url: &mut Option<String>) {
    if let Some(base) = body.remove("baseURL") {
        *api_url = Some(base);
    }
}

/// Projette un modele sur son fournisseur, comme `projectModel`.
///
/// Les en-tetes et corps sont fusionnes (le modele gagne), la variante vient
/// du modele, et l URL d API suit les trois cas de la source : modele natif
/// sans URL ni reglages (reprend l API du fournisseur avec l id du modele),
/// modele et fournisseur `aisdk` sans URL de modele (reprend l URL et fusionne
/// les reglages), `aisdk` des deux cotes avec URL (fusionne les reglages).
pub fn projeter_modele(model: &ModelInfo, provider: &ProviderInfo) -> ModelInfo {
    let mut projete = model.clone();
    let api = if model.api.kind == "native" && model.api.url.is_none() && model.api.settings.is_empty() {
        Api {
            kind: provider.api.kind.clone(),
            api_id: model.api.api_id.clone(),
            url: provider.api.url.clone(),
            settings: provider.api.settings.clone(),
        }
    } else if model.api.kind == "aisdk"
        && provider.api.kind == "aisdk"
        && model.api.url.is_none()
    {
        let mut settings = provider.api.settings.clone();
        settings.extend(model.api.settings.clone());
        Api {
            kind: model.api.kind.clone(),
            api_id: model.api.api_id.clone(),
            url: provider.api.url.clone(),
            settings,
        }
    } else if model.api.kind == "aisdk" && provider.api.kind == "aisdk" {
        let mut settings = provider.api.settings.clone();
        settings.extend(model.api.settings.clone());
        Api {
            kind: model.api.kind.clone(),
            api_id: model.api.api_id.clone(),
            url: model.api.url.clone(),
            settings,
        }
    } else {
        model.api.clone()
    };
    let mut headers = provider.request.headers.clone();
    headers.extend(model.request.headers.clone());
    let mut body = provider.request.body.clone();
    body.extend(model.request.body.clone());
    projete.api = api;
    projete.request = Requete::new(headers, body, model.request.variant.clone());
    projete.provider_id = provider.id.clone();
    projete.id = model.id.clone();
    projete
}

/// Vrai si le texte contient un des mots de `SMALL_MODEL_RE` en mot entier :
/// `nano`, `flash`, `lite`, `mini`, `haiku`, `small` ou `fast`.
///
/// La source utilise `/\b(nano|flash|lite|mini|haiku|small|fast)\b/` ; sans
/// crate d expressions regulieres, la frontiere de mot est portee a la main :
/// decoupage sur tout ce qui n est pas alphanumerique, comparaison en
/// minuscules.
pub fn est_petit_modele(texte: &str) -> bool {
    const MOTS: [&str; 7] = ["nano", "flash", "lite", "mini", "haiku", "small", "fast"];
    let minuscules = texte.to_lowercase();
    let mut mot = String::new();
    let mut trouve = false;
    for c in minuscules.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_alphanumeric() {
            mot.push(c);
        } else if !mot.is_empty() {
            if MOTS.contains(&mot.as_str()) {
                trouve = true;
                break;
            }
            mot.clear();
        }
    }
    trouve
}

/// Filtre les fournisseurs soumis a une politique `deny`, comme `finalize`.
pub fn appliquer_politique(catalogue: &mut CatalogData, refuses: &HashSet<String>) {
    catalogue.providers.retain(|id, _| !refuses.contains(id));
}

/// Choisit le modele par defaut : le couple enregistre s il est disponible et
/// active, sinon le modele disponible le plus recent, comme `model.default()`.
pub fn modele_par_defaut(
    catalogue: &CatalogData,
    disponibles: &HashSet<String>,
    modeles: &[ModelInfo],
) -> Option<ModelInfo> {
    if let Some(defaut) = &catalogue.default_model {
        if disponibles.contains(&defaut.provider_id) {
            if let Some(m) = modeles.iter().find(|m| m.provider_id == defaut.provider_id && m.id == defaut.model_id) {
                if m.enabled {
                    return Some(m.clone());
                }
            }
        }
    }
    let mut tries = modeles.to_vec();
    tries.sort_by(|a, b| b.released.cmp(&a.released));
    tries.into_iter().next()
}

/// Choisit le petit modele d un fournisseur, comme `model.small()`.
///
/// Candidats : modeles actifs du fournisseur, actives, avec entree et sortie
/// texte, cout strictement positif et age de dix-huit mois au plus. Le score
/// est `(cout / cout_max) * 0.8 + (age / age_max) * 0.2`, le plus petit gagne ;
/// les modeles dont le nom contient un mot petit sont preferes quand il y en a.
pub fn petit_modele(record: &ProviderRecord, now_ms: i64) -> Option<ModelInfo> {
    if record.provider.id == "azure" || record.provider.id == "azure-cognitive-services" {
        return None;
    }
    if record.provider.id == "opencode" {
        if let Some(m) = record.models.get("gpt-5-nano") {
            if m.enabled && m.status == "active" {
                return Some(projeter_modele(m, &record.provider));
            }
        }
    }
    let mut candidats: Vec<(&ModelInfo, f64, f64)> = Vec::new();
    for m in record.models.values() {
        if m.provider_id != record.provider.id || !m.enabled || m.status != "active" {
            continue;
        }
        if !m.input.iter().any(|c| c.starts_with("text")) {
            continue;
        }
        if !m.output.iter().any(|c| c.starts_with("text")) {
            continue;
        }
        let cout = match (m.cost_input, m.cost_output) {
            (Some(i), Some(o)) => i + o,
            _ => 999.0,
        };
        let age_mois = (now_ms - m.released) as f64 / (1000.0 * 60.0 * 60.0 * 24.0 * 30.0);
        if cout <= 0.0 || age_mois > 18.0 {
            continue;
        }
        candidats.push((m, cout, age_mois));
    }
    if candidats.is_empty() {
        return None;
    }
    let avec_mot: Vec<(&ModelInfo, f64, f64)> = candidats
        .iter()
        .filter(|(m, _, _)| {
            let texte = format!("{} {} {}", m.id, m.family.clone().unwrap_or_default(), m.name);
            est_petit_modele(&texte)
        })
        .map(|(m, c, a)| (*m, *c, *a))
        .collect();
    let choisis = if avec_mot.is_empty() { candidats } else { avec_mot };
    let cout_max = choisis.iter().map(|(_, c, _)| *c).fold(0.01f64, f64::max);
    let age_max = choisis.iter().map(|(_, _, a)| *a).fold(0.01f64, f64::max);
    let mut tries = choisis;
    tries.sort_by(|(_, c1, a1), (_, c2, a2)| {
        let s1 = (c1 / cout_max) * 0.8 + (a1 / age_max) * 0.2;
        let s2 = (c2 / cout_max) * 0.8 + (a2 / age_max) * 0.2;
        s1.partial_cmp(&s2).unwrap_or(std::cmp::Ordering::Equal)
    });
    tries.into_iter().next().map(|(m, _, _)| projeter_modele(m, &record.provider))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fournisseur(id: &str) -> ProviderInfo {
        ProviderInfo::new(id)
    }

    fn modele(provider: &str, id: &str, released: i64) -> ModelInfo {
        ModelInfo::new(provider, id, released)
    }

    #[test]
    fn la_seule_action_de_politique_connue_est_provider_use() {
        assert_eq!(POLICY_ACTIONS, ["provider.use"]);
    }

    #[test]
    fn un_fournisseur_desactive_n_est_jamais_disponible() {
        assert!(!fournisseur_disponible(true, true, 5, None, false));
        assert!(!fournisseur_disponible(true, false, 0, None, false));
    }

    #[test]
    fn une_cle_en_clair_rend_disponible_meme_sans_integration() {
        assert!(fournisseur_disponible(false, true, 0, Some("x"), true));
        assert!(fournisseur_disponible(false, true, 0, None, false));
    }

    #[test]
    fn des_connexions_d_integration_rendent_disponible() {
        assert!(fournisseur_disponible(false, false, 2, Some("x"), false));
        assert!(!fournisseur_disponible(false, false, 0, Some("x"), false));
    }

    #[test]
    fn sans_cle_ni_connexion_seule_l_absence_d_integration_compte() {
        assert!(fournisseur_disponible(false, false, 0, None, false));
        assert!(!fournisseur_disponible(false, false, 0, None, true));
        assert!(!fournisseur_disponible(false, false, 0, Some("x"), false));
    }

    #[test]
    fn la_normalisation_deplace_baseurl_vers_l_url_d_api() {
        let mut body = HashMap::from([("baseURL".to_string(), "https://ex".to_string())]);
        let mut url: Option<String> = None;
        normaliser_api_url(&mut body, &mut url);
        assert_eq!(url.as_deref(), Some("https://ex"));
        assert!(!body.contains_key("baseURL"));
    }

    #[test]
    fn la_normalisation_ne_fait_rien_sans_baseurl() {
        let mut body = HashMap::from([("apiKey".to_string(), "k".to_string())]);
        let mut url = Some("https://gardee".to_string());
        normaliser_api_url(&mut body, &mut url);
        assert_eq!(url.as_deref(), Some("https://gardee"));
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn la_projection_fusionne_en_tetes_corps_et_variante_du_modele() {
        let mut p = fournisseur("p");
        p.request.headers.insert("h".to_string(), "f".to_string());
        p.request.body.insert("b".to_string(), "f".to_string());
        let mut m = modele("p", "m", 0);
        m.request.headers.insert("h".to_string(), "m".to_string());
        m.request.variant = Some("v".to_string());
        let projete = projeter_modele(&m, &p);
        assert_eq!(projete.request.headers.get("h").map(String::as_str), Some("m"));
        assert_eq!(projete.request.body.get("b").map(String::as_str), Some("f"));
        assert_eq!(projete.request.variant.as_deref(), Some("v"));
        assert_eq!(projete.provider_id, "p");
        assert_eq!(projete.id, "m");
    }

    #[test]
    fn un_modele_natif_sans_url_reprend_l_api_du_fournisseur() {
        let mut p = fournisseur("p");
        p.api.url = Some("https://f".to_string());
        let mut m = modele("p", "m", 0);
        m.api.kind = "native".to_string();
        m.api.url = None;
        let projete = projeter_modele(&m, &p);
        assert_eq!(projete.api.url.as_deref(), Some("https://f"));
        assert_eq!(projete.api.api_id, "openai");
    }

    #[test]
    fn le_detecteur_de_petit_modele_voit_les_mots_entiers() {
        assert!(est_petit_modele("gpt-5-nano"));
        assert!(est_petit_modele("Flash-Large"));
        assert!(est_petit_modele("my mini model"));
        assert!(est_petit_modele("HAIKU 3"));
        assert!(!est_petit_modele("grand-modele"));
        assert!(!est_petit_modele(""));
    }

    #[test]
    fn le_detecteur_ne_confond_pas_un_prefixe_avec_un_mot() {
        assert!(!est_petit_modele("minimalisme"));
        assert!(!est_petit_modele("nanometre"));
    }

    #[test]
    fn le_catalogue_liste_ajoute_et_retire_sans_reordonner_les_modeles() {
        let mut catalogue = CatalogData::new();
        catalogue.providers.insert(
            "p".to_string(),
            ProviderRecord { provider: fournisseur("p"), models: HashMap::from([("m".to_string(), modele("p", "m", 1))]) },
        );
        assert_eq!(catalogue.lister().len(), 1);
        assert!(catalogue.fournisseur("p").is_some());
        assert!(catalogue.modele("p", "m").is_some());
        catalogue.retirer_modele("p", "m");
        assert!(catalogue.modele("p", "m").is_none());
        catalogue.retirer_fournisseur("p");
        assert!(catalogue.fournisseur("p").is_none());
    }

    #[test]
    fn tous_les_modeles_sont_tries_par_date_decroissante() {
        let mut catalogue = CatalogData::new();
        catalogue.providers.insert(
            "p".to_string(),
            ProviderRecord {
                provider: fournisseur("p"),
                models: HashMap::from([
                    ("vieux".to_string(), modele("p", "vieux", 10)),
                    ("recent".to_string(), modele("p", "recent", 30)),
                    ("milieu".to_string(), modele("p", "milieu", 20)),
                ]),
            },
        );
        let ids: Vec<String> = catalogue.tous_les_modeles().iter().map(|m| m.id.clone()).collect();
        assert_eq!(ids, vec!["recent".to_string(), "milieu".to_string(), "vieux".to_string()]);
    }

    #[test]
    fn la_politique_deny_retire_les_fournisseurs_refuses() {
        let mut catalogue = CatalogData::new();
        catalogue.providers.insert("ok".to_string(), ProviderRecord { provider: fournisseur("ok"), models: HashMap::new() });
        catalogue.providers.insert("ko".to_string(), ProviderRecord { provider: fournisseur("ko"), models: HashMap::new() });
        appliquer_politique(&mut catalogue, &HashSet::from(["ko".to_string()]));
        assert!(catalogue.fournisseur("ok").is_some());
        assert!(catalogue.fournisseur("ko").is_none());
    }

    #[test]
    fn le_modele_par_defaut_prefere_le_couple_enregistre_quand_disponible() {
        let mut catalogue = CatalogData::new();
        catalogue.definir_defaut("p", "m");
        let modeles = vec![modele("p", "m", 5), modele("p", "autre", 99)];
        let dispo = HashSet::from(["p".to_string()]);
        let choisi = modele_par_defaut(&catalogue, &dispo, &modeles).unwrap();
        assert_eq!(choisi.id, "m");
    }

    #[test]
    fn sans_defaut_valide_le_plus_recent_gagne() {
        let catalogue = CatalogData::new();
        let modeles = vec![modele("p", "vieux", 5), modele("p", "recent", 50)];
        let dispo = HashSet::from(["p".to_string()]);
        let choisi = modele_par_defaut(&catalogue, &dispo, &modeles).unwrap();
        assert_eq!(choisi.id, "recent");
    }

    #[test]
    fn azure_n_a_jamais_de_petit_modele() {
        let record = ProviderRecord { provider: fournisseur("azure"), models: HashMap::from([("m".to_string(), modele("azure", "m", 0))]) };
        assert!(petit_modele(&record, 0).is_none());
        let record = ProviderRecord { provider: fournisseur("azure-cognitive-services"), models: HashMap::new() };
        assert!(petit_modele(&record, 0).is_none());
    }

    #[test]
    fn opencode_rend_gpt_5_nano_quand_actif() {
        let mut models = HashMap::new();
        models.insert("gpt-5-nano".to_string(), modele("opencode", "gpt-5-nano", 100));
        models.insert("gros".to_string(), modele("opencode", "gros", 200));
        let record = ProviderRecord { provider: fournisseur("opencode"), models };
        let choisi = petit_modele(&record, 200).unwrap();
        assert_eq!(choisi.id, "gpt-5-nano");
    }

    #[test]
    fn le_petit_modele_prefere_le_mot_petit_puis_le_moins_cher() {
        let mut record = ProviderRecord { provider: fournisseur("p"), models: HashMap::new() };
        let mut cher_mini = modele("p", "cher-mini", 100);
        cher_mini.cost_input = Some(5.0);
        cher_mini.cost_output = Some(5.0);
        let mut pas_cher = modele("p", "gros", 100);
        pas_cher.cost_input = Some(0.5);
        pas_cher.cost_output = Some(0.5);
        record.models.insert("cher-mini".to_string(), cher_mini);
        record.models.insert("gros".to_string(), pas_cher);
        let choisi = petit_modele(&record, 200).unwrap();
        assert_eq!(choisi.id, "cher-mini");
    }

    #[test]
    fn un_modele_gratuit_ou_trop_vieux_n_est_jamais_petit() {
        let mut record = ProviderRecord { provider: fournisseur("p"), models: HashMap::new() };
        let mut gratuit = modele("p", "mini-gratuit", 100);
        gratuit.cost_input = Some(0.0);
        gratuit.cost_output = Some(0.0);
        record.models.insert("mini-gratuit".to_string(), gratuit);
        assert!(petit_modele(&record, 200).is_none());
    }

    #[test]
    fn definir_le_defaut_ecrase_le_precedent() {
        let mut catalogue = CatalogData::new();
        catalogue.definir_defaut("p", "a");
        catalogue.definir_defaut("p", "b");
        assert_eq!(catalogue.default_model.unwrap().model_id, "b");
    }
}
