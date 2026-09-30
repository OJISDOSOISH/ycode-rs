//! Portage Rust de `opencode/packages/core/src/plugin/provider/venice.ts`.
//!
//! ## Ce que dit la source
//!
//! Quinze lignes, et un seul contenu : le plugin `venice`. Il enregistre un
//! seul crochet sur `ctx.aisdk.sdk`, c'est-a-dire le crochet `sdk` de
//! l'interface `AISDKHooks` (`packages/plugin/src/v2/effect/aisdk.ts`). Ce
//! crochet est appele a chaque construction de SDK AI et fait trois choses :
//!
//! 1. si le paquet de l'evenement n'est pas `"venice-ai-sdk-provider"`, il
//!    rend la main sans rien changer du tout ;
//! 2. sinon il importe dynamiquement le paquet npm du meme nom ;
//! 3. et il remplit `evt.sdk` avec `mod.createVenice(evt.options)`.
//!
//! Ce n'est donc pas un objet de configuration litteral, c'est un crochet. Il
//! n'y a pas de donnee statique a traduire en struct : il n'y a que
//! l'evenement que le crochet recoit et modifie, plus les deux chaines
//! constantes qui commandent son comportement.
//!
//! ## Choix de portage
//!
//! - Le `return` de la ligne 9 est un retour nu : l'evenement n'est **pas**
//!   remis a zero. Un `sdk` deja present, pose par un crochet enregistre avant
//!   celui-ci, reste en place. `SdkOutcome::Ignore` dit precisement cela.
//! - La comparaison `evt.package !== PACKAGE` est stricte : la casse et les
//!   espaces comptent. Elle n'est traduite par aucun test de veracite, donc un
//!   nom de paquet vide est rejete au lieu d'etre accepte.
//! - L'`import()` dynamique d'un paquet npm n'a pas d'equivalent Rust : la
//!   fabrique `createVenice` est passee en parametre de `sdk_hook`. C'est le
//!   chargeur de plugins qui possede le paquet, donc c'est lui qui fournit la
//!   fonction. Ce fichier, lui, ne fait que le test du nom de paquet et l'appel.
//! - `evt.options` est un `Record<string, any>` libre : `BTreeMap<String,
//!   Value>`. On perd l'ordre d'insertion des cles, comme partout ailleurs dans
//!   ce portage, on gagne un ordre deterministe. Les **cles** des options ne
//!   sont pas touchees : elles viennent de la configuration de l'utilisateur,
//!   donc `apiKey` reste `apiKey` et n'est jamais renommee.
//! - `evt.model` est un `ModelV2Info` du SDK que ce plugin ne lit jamais. Il
//!   n'est donc pas traduit en struct : il reste une valeur JSON opaque.
//! - `sdk?: any` devient `Option<Value>` avec `skip_serializing_if`, pour ne
//!   pas ecrire une cle `sdk: null` que le TS ne produirait jamais.
//! - `options` et `model` sont `readonly` mais obligatoires en TS, donc
//!   obligatoires ici aussi : un evenement qui ne les porte pas est refuse au
//!   decodage plutot que complete par un dictionnaire vide.
//! - Les noms de champs sont deja identiques entre le TS et le Rust (`model`,
//!   `package`, `options`, `sdk`), donc aucun `#[serde(rename = ...)]`. Un test
//!   verifie quand meme les noms au JSON : c'est l'erreur la plus frequente de
//!   ce portage, et elle est invisible de l'interieur du code Rust.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Identifiant du plugin. En TS : `id: "venice"`.
///
/// Il sert a `PluginV2.ID.make(loaded.id)` dans `plugin/internal.ts`, donc il
/// n'est pas seulement decoratif : deux plugins ne peuvent pas le partager.
pub const PLUGIN_ID: &str = "venice";

/// Nom du paquet npm reconnu par ce plugin.
///
/// La source le compare avec `!==`, donc la comparaison est exacte, casse
/// comprise. Le nom du paquet est aussi celui du `import()` dynamique de la
/// ligne 10.
pub const PACKAGE: &str = "venice-ai-sdk-provider";

/// Nom de la fonction exportee par le paquet et appelee par le crochet.
///
/// En TS : `mod.createVenice(evt.options)`. Ce n'est pas une chaine comparee,
/// c'est le nom que le chargeur doit retrouver sur le module importe.
pub const FACTORY: &str = "createVenice";

/// Options d'un fournisseur : dictionnaire libre, cle en chaine, valeur libre.
///
/// En TS : `Record<string, any>`. Aucune contrainte sur les valeurs, donc
/// `Value` est la traduction exacte. L'ordre des cles devient deterministe.
pub type Options = BTreeMap<String, Value>;

/// Descripteur du plugin.
///
/// En TS, `define({ id, effect })` renvoie un objet dont `effect` est une
/// fonction. Une fonction n'a pas de representation en Rust : le crochet est
/// `sdk_hook` ci-dessous, et il reste dans le descripteur la seule donnee, a
/// savoir `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plugin {
    /// Nom du plugin, present dans l'objet JS comme champ `id`.
    pub id: String,
}

impl Plugin {
    /// Descripteur du plugin Venice, avec l'identifiant de la source.
    pub fn new() -> Self {
        Self { id: PLUGIN_ID.to_string() }
    }

    /// Le nom de paquet que ce plugin reconnait, pour ne pas le repeter.
    pub fn package(&self) -> &'static str {
        PACKAGE
    }
}

impl Default for Plugin {
    fn default() -> Self {
        Self::new()
    }
}

/// Evenement recu par le crochet `aisdk.sdk`.
///
/// En TS, l'evenement est `{ readonly model, readonly package, readonly
/// options, sdk? }`. C'est un objet que les crochets successifs se partagent et
/// modifient : `sdk` est le seul champ ecrit, et il est ecrit par affectation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkEvent {
    /// `readonly model: ModelV2Info`.
    ///
    /// Ce plugin ne le lit jamais. Il reste une valeur JSON opaque plutot que
    /// d'inventer un struct pour un type qui n'est pas dans ce fichier.
    pub model: Value,
    /// `readonly package: string`. Le champ que le crochet compare.
    pub package: String,
    /// `readonly options: Record<string, any>`. Les options du fournisseur.
    pub options: Options,
    /// `sdk?: any`. Absent tant qu'aucun crochet ne l'a rempli.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk: Option<Value>,
}

impl SdkEvent {
    /// Evenement sans SDK construit, avec des options donnees.
    pub fn new(package: impl Into<String>, options: Options) -> Self {
        Self { model: Value::Null, package: package.into(), options, sdk: None }
    }
}

/// Ce que le crochet a fait de l'evenement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkOutcome {
    /// La comparaison de paquet a echoue : le crochet a rendu la main, et
    /// l'evenement est reste strictement tel qu'il etait.
    Ignore,
    /// `evt.sdk = mod.createVenice(evt.options)` a ete fait.
    Created,
}

/// Vrai si le crochet s'interesse a ce nom de paquet.
///
/// La source teste `evt.package !== PACKAGE`, une inegalite stricte sur des
/// chaines. Aucun test de veracite n'intervient : une chaine vide est donc un
/// nom de paquet comme un autre, et elle est rejetee.
pub fn handles(package: &str) -> bool {
    package == PACKAGE
}

/// Le crochet du plugin, en version pure et testable.
///
/// `create` est la fabrique `createVenice` du paquet npm. Le TypeScript fait
/// un `import()` dynamique puis appelle la fonction exportee ; en Rust, c'est
/// le chargeur de plugins qui detient le paquet et qui fournit la fonction. Le
/// comportement observable du fichier, lui, est entier ici : le test du nom de
/// paquet, l'appel une seule fois, et l'ecriture de `sdk`.
pub fn sdk_hook<F>(event: &mut SdkEvent, create: F) -> SdkOutcome
where
    F: FnOnce(&Options) -> Value,
{
    if !handles(&event.package) {
        return SdkOutcome::Ignore;
    }
    let created = create(&event.options);
    event.sdk = Some(created);
    SdkOutcome::Created
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::Cell;

    /// Fabrique de test : compte ses appels et renvoie un marqueur lisible.
    fn marqueur(nb: &Cell<usize>) -> impl FnOnce(&Options) -> Value + '_ {
        move |_| {
            nb.set(nb.get() + 1);
            json!("sdk venice")
        }
    }

    fn options_paires() -> Options {
        let mut options = Options::new();
        options.insert("apiKey".to_string(), json!("secret"));
        options.insert("baseURL".to_string(), json!("https://api.venice.ai"));
        options
    }

    // ------------------------------------------------------------- cas nominal

    #[test]
    fn un_evenement_du_paquet_venice_remplit_le_champ_sdk() {
        let mut event = SdkEvent::new(PACKAGE, options_paires());
        let appels = Cell::new(0);

        let issue = sdk_hook(&mut event, marqueur(&appels));

        assert_eq!(issue, SdkOutcome::Created);
        assert_eq!(appels.get(), 1, "la fabrique doit etre appelee une seule fois");
        assert_eq!(event.sdk, Some(json!("sdk venice")));
    }

    #[test]
    fn un_evenement_d_un_autre_paquet_laisse_le_champ_sdk_absent() {
        let mut event = SdkEvent::new("@ai-sdk/openai", options_paires());
        let appels = Cell::new(0);

        let issue = sdk_hook(&mut event, marqueur(&appels));

        assert_eq!(issue, SdkOutcome::Ignore);
        assert_eq!(event.sdk, None, "rien ne doit etre ecrit sur un paquet etranger");
    }

    #[test]
    fn un_evenement_d_un_autre_paquet_appelle_pas_la_fabrique() {
        // Le `return` de la ligne 9 est avant le `import()`. Si la fabrique etait
        // appelee quand meme, le seul symptome visible serait un effet de bord :
        // voila pourquoi elle est comptee.
        let mut event = SdkEvent::new("@ai-sdk/openai", Options::new());
        let appels = Cell::new(0);

        sdk_hook(&mut event, marqueur(&appels));

        assert_eq!(appels.get(), 0, "la fabrique ne doit pas etre appelee");
    }

    // ------------------------------------------------- evenement deja pourvu

    #[test]
    fn un_sdk_deja_presente_est_remplace_par_la_fabrique_venice() {
        // Les crochets s'enchainent dans l'ordre d'enregistrement et se voient
        // les uns les autres : une affectation directe ecrase la valeur d'avant.
        let mut event = SdkEvent::new(PACKAGE, Options::new());
        event.sdk = Some(json!("sdk pose par un autre crochet"));
        let appels = Cell::new(0);

        let issue = sdk_hook(&mut event, marqueur(&appels));

        assert_eq!(issue, SdkOutcome::Created);
        assert_eq!(event.sdk, Some(json!("sdk venice")), "la valeur doit etre ecrasee");
    }

    #[test]
    fn un_sdk_deja_presente_survive_a_un_paquet_etranger() {
        // Piege du `return` de la ligne 9 : la source rend la main sans rien
        // remettre a zero. Un `sdk` pose par un crochet precedent reste la.
        let mut event = SdkEvent::new("@ai-sdk/openai", Options::new());
        event.sdk = Some(json!("sdk pose par un autre crochet"));
        let appels = Cell::new(0);

        let issue = sdk_hook(&mut event, marqueur(&appels));

        assert_eq!(issue, SdkOutcome::Ignore);
        assert_eq!(
            event.sdk,
            Some(json!("sdk pose par un autre crochet")),
            "le crochet ne doit ni ecrire ni effacer"
        );
    }

    // ------------------------------------------------------ comparaison stricte

    #[test]
    fn la_casse_du_nom_du_paquet_compte() {
        // `!==` sur des chaines JavaScript compare les octets. Un nom qui
        // differe d'une seule lettre n'est pas le bon paquet.
        assert!(!handles("Venice-AI-SDK-Provider"));
        assert!(!handles("VENICE-AI-SDK-PROVIDER"));
        assert!(handles(PACKAGE));
    }

    #[test]
    fn un_espace_en_fin_de_nom_de_paquet_rompt_la_correspondance() {
        assert!(!handles("venice-ai-sdk-provider "));
        assert!(!handles(" venice-ai-sdk-provider"));
    }

    #[test]
    fn un_nom_de_paquet_vide_est_rejete_comme_tout_autre() {
        // Aucun test de veracite ici : si la chaine vide etait acceptee, le
        // crochet construirait un SDK Venice pour un evenement sans paquet.
        let mut event = SdkEvent::new("", Options::new());
        let appels = Cell::new(0);

        let issue = sdk_hook(&mut event, marqueur(&appels));

        assert_eq!(issue, SdkOutcome::Ignore);
        assert_eq!(appels.get(), 0);
        assert_eq!(event.sdk, None);
    }

    #[test]
    fn des_options_vides_produisent_quand_meme_un_sdk() {
        // Le TS ne teste pas les options avant d'appeler la fabrique : un
        // dictionnaire vide est un argument valide, pas une raison de skipper.
        let mut event = SdkEvent::new(PACKAGE, Options::new());
        let appels = Cell::new(0);

        let issue = sdk_hook(&mut event, marqueur(&appels));

        assert_eq!(issue, SdkOutcome::Created);
        assert_eq!(appels.get(), 1);
        assert_eq!(event.sdk, Some(json!("sdk venice")));
    }

    // ------------------------------------------------- noms de champs et options

    #[test]
    fn la_fabrique_recoit_les_options_telles_quelles() {
        // Les cles d'options viennent de la configuration de l'utilisateur.
        // Aucune ne doit etre renommee, en particulier pas les camelCase.
        let mut event = SdkEvent::new(PACKAGE, options_paires());
        let vues = Cell::new(Options::new());

        sdk_hook(&mut event, |options: &Options| -> Value {
            vues.set(Options::clone(options));
            Value::Bool(true)
        });

        let recues = vues.into_inner();
        assert_eq!(recues.len(), 2);
        assert_eq!(recues.get("apiKey"), Some(&json!("secret")));
        assert_eq!(recues.get("baseURL"), Some(&json!("https://api.venice.ai")));
        assert!(recues.get("api_key").is_none(), "aucun renommage en snake_case");
        assert!(recues.get("ApiKey").is_none(), "aucun renommage de casse");
    }

    #[test]
    fn les_noms_de_champs_json_sont_ceux_du_typescript() {
        let event = SdkEvent::new(PACKAGE, options_paires());

        let json = serde_json::to_value(&event).unwrap();

        let objet = json.as_object().expect("l'evenement doit serialiser en objet");
        let mut cles: Vec<&str> = objet.keys().map(|cle| cle.as_str()).collect();
        cles.sort();
        assert_eq!(cles, vec!["model", "options", "package"]);

        assert_eq!(json["package"], json!("venice-ai-sdk-provider"));
        assert_eq!(json["options"]["apiKey"], json!("secret"));
        // Aucun nom en snake_case ne doit apparaitre a la place.
        assert!(json.get("sdk_").is_none());
        assert!(json.get("Package").is_none());
        assert!(json.get("OPTIONS").is_none());
    }

    #[test]
    fn un_sdk_absent_ne_serialise_pas_une_cle_nulle() {
        // `sdk?: any` : la cle disparait quand la valeur manque, elle ne doit
        // jamais devenir `null`, que le TS ne produit pas.
        let event = SdkEvent::new(PACKAGE, Options::new());

        let json = serde_json::to_value(&event).unwrap();

        assert!(json.get("sdk").is_none());
        assert_eq!(json.as_object().unwrap().len(), 3);
    }

    #[test]
    fn un_sdk_present_reapparait_sous_le_meme_nom() {
        let mut event = SdkEvent::new(PACKAGE, Options::new());
        sdk_hook(&mut event, |_: &Options| -> Value { json!({ "use": "venice" }) });

        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(json["sdk"], json!({ "use": "venice" }));
    }

    // ------------------------------------------------------- entrees invalides

    #[test]
    fn un_evenement_complet_se_relit_identique() {
        let json = json!({
            "model": { "id": "venice/meta/llama-3.3" },
            "package": "venice-ai-sdk-provider",
            "options": { "apiKey": "secret" },
            "sdk": { "use": "venice" }
        });

        let event: SdkEvent = serde_json::from_value(json.clone()).unwrap();

        assert_eq!(serde_json::to_value(&event).unwrap(), json);
    }

    #[test]
    fn un_evenement_sans_sdk_se_relit_avec_sdk_absent() {
        let json = json!({
            "model": { "id": "venice/meta/llama-3.3" },
            "package": "venice-ai-sdk-provider",
            "options": {}
        });

        let event: SdkEvent = serde_json::from_value(json).unwrap();

        assert_eq!(event.sdk, None);
    }

    #[test]
    fn un_evenement_sans_options_est_refuse() {
        // `readonly options: Record<string, any>` est obligatoire en TS. Le
        // decoder ne doit pas inventer un dictionnaire vide a sa place.
        let json = json!({
            "model": { "id": "venice/meta/llama-3.3" },
            "package": "venice-ai-sdk-provider"
        });

        assert!(serde_json::from_value::<SdkEvent>(json).is_err());
    }

    #[test]
    fn un_evenement_sans_model_est_refuse() {
        let json = json!({
            "package": "venice-ai-sdk-provider",
            "options": {}
        });

        assert!(serde_json::from_value::<SdkEvent>(json).is_err());
    }

    #[test]
    fn un_descripteur_de_plugin_porte_son_seul_identifiant() {
        let plugin = Plugin::new();

        assert_eq!(plugin.id, "venice");
        assert_eq!(plugin.package(), PACKAGE);
        // La fonction `effect` du TS n'a pas de representation en JSON.
        assert_eq!(serde_json::to_value(&plugin).unwrap(), json!({ "id": "venice" }));
    }

    #[test]
    fn les_constantes_de_la_source_sont_reprises_telles_quelles() {
        assert_eq!(PLUGIN_ID, "venice");
        assert_eq!(PACKAGE, "venice-ai-sdk-provider");
        assert_eq!(FACTORY, "createVenice");
    }
}
