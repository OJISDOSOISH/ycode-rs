//! Portage Rust de `opencode/packages/core/src/session/info.ts`.
//!
//! Ce fichier ne fait qu'une chose : traduire une **ligne de la table `session`**
//! en un objet [`Info`]. Il ne definit aucun contrat ; le contrat vit dans
//! [`super::schema`], et on l'importe.
//!
//! ## Le piege central : deux operateurs, deux regles
//!
//! L'original melange deux operateurs qui ne se comportent **pas** de la meme
//! facon, et les confondre est la principale source d'incompatibilite de ce
//! portage :
//!
//! - Le **ternaire** `row.parent_id ? X : undefined` teste la *veracite*. En
//!   JavaScript la chaine vide `""` est **falsy**. Une chaine vide devient donc
//!   `undefined`, c'est-a-dire `None` en Rust. Un `Option<String>` traduit
//!   naivement garderait `Some("")` : divergence silencieuse, invisible a la
//!   compilation, visible seulement a l'echange avec le TypeScript.
//!
//! - Le **coalescent** `row.model.variant ?? "default"` teste la *nullite*. Seuls
//!   `null` et `undefined` declenchent le defaut. Une chaine vide **survit** et
//!   ne devient PAS `"default"`.
//!
//! Autrement dit, `""` doit disparaitre a un endroit et survivre a l'autre. C'est
//! ce que reproduisent [`truthy`] et [`present_or`], et c'est ce que les tests
//! verifient explicitement.
//!
//! Le meme piege existe sur les entiers : `row.time_archived ? ... : undefined`
//! evalue `0` comme falsy. Une horodatage nul doit donc disparaitre, alors qu'un
//! simple `is_some()` la conserverait.

use serde::{Deserialize, Serialize};

use super::schema::{
    CacheTokens, DEFAULT_VARIANT, Info, LocationRef, ModelRef, ProjectId, SessionId, Time, Tokens,
};

// ---------------------------------------------------------------------------
// Veracite JavaScript
// ---------------------------------------------------------------------------

/// Reproduit la veracite JavaScript sur une chaine.
///
/// `""` est falsy en JavaScript, donc le ternaire `x ? ... : undefined` du
/// TypeScript d'origine le transforme en `undefined`. Traduire par un simple
/// `.map()` conserverait `Some("")` et produirait un JSON different.
fn truthy(value: &Option<String>) -> Option<&str> {
    match value {
        Some(s) if !s.is_empty() => Some(s.as_str()),
        _ => None,
    }
}

/// Reproduit la veracite JavaScript sur un entier.
///
/// Seule exception par rapport au vide : `0` est lui aussi falsy. Une colonne
/// d'horodatage contenant `0` doit donc disparaitre, et non etre conservee.
fn truthy_millis(value: i64) -> Option<i64> {
    if value == 0 {
        None
    } else {
        Some(value)
    }
}

/// Reproduit l'operateur `??` : seule la nullite declenche la valeur par defaut.
///
/// Contrairement a [`truthy`], une chaine vide est **conservee**. Confondre les
/// deux est l'erreur la plus facile a commettre dans ce fichier.
fn present_or(value: Option<&str>, default: &str) -> String {
    value.unwrap_or(default).to_string()
}

// ---------------------------------------------------------------------------
// La ligne de la table
// ---------------------------------------------------------------------------

/// Une ligne de la table `session`, telle que SQLite la renvoie.
///
/// Les noms de champs sont ceux des **colonnes SQL** (`project_id`,
/// `tokens_cache_read`...) et non ceux de l'API (`projectID`). Les deux jeux de
/// noms coexistent desormais dans deux fichiers distincts : ici les colonnes,
/// dans [`super::schema`] le contrat. C'est le decoupage du au numro 2 de la
/// negociation, et c'est ce qui evite de les confondre.
///
/// Le SQL proprement dit, la creation de table, les index, les migrations,
/// n'est volontairement pas porte. Il n'existe aucun crate SQL dans
/// `Cargo.toml`, et la regle du brief est explicite : le SQL sera branche plus
/// tard, et la logique metier se teste sur une tranche de donnees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRow {
    /// Identifiant de session.
    pub id: SessionId,
    /// Identifiant du projet parent, obligatoire en base.
    pub project_id: ProjectId,
    /// Titre affiche.
    pub title: String,
    /// Repertoire de travail, obligatoire en base.
    pub directory: String,
    /// Cout cumule. `real` en SQL, `Schema.Finite` en TS : c'est un flottant.
    pub cost: f64,
    /// Compteurs de jetons, stockes a plat puis regroupes par [`from_row`].
    pub tokens_input: f64,
    /// Voir [`SessionRow::tokens_input`].
    pub tokens_output: f64,
    /// Voir [`SessionRow::tokens_input`].
    pub tokens_reasoning: f64,
    /// Voir [`SessionRow::tokens_input`].
    pub tokens_cache_read: f64,
    /// Voir [`SessionRow::tokens_input`].
    pub tokens_cache_write: f64,
    /// Creation, en millisecondes depuis l'epoch.
    pub time_created: i64,
    /// Mise a jour, en millisecondes depuis l'epoch.
    pub time_updated: i64,
    /// Session parente. Colonne nullable, et peut contenir la chaine vide.
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Espace de travail. Colonne nullable, et peut contenir la chaine vide.
    #[serde(default)]
    pub workspace_id: Option<String>,
    /// Agent utilise. Colonne nullable, et peut contenir la chaine vide.
    #[serde(default)]
    pub agent: Option<String>,
    /// Sous-chemin relatif. Colonne nullable, et peut contenir la chaine vide.
    #[serde(default)]
    pub path: Option<String>,
    /// Modele utilise. Colonne JSON nullable.
    #[serde(default)]
    pub model: Option<ModelRef>,
    /// Point de retour arriere. Colonne JSON nullable.
    #[serde(default)]
    pub revert: Option<super::schema::RevertState>,
    /// Archivage. Colonne nullable ; `0` doit disparaitre.
    #[serde(default)]
    pub time_archived: Option<i64>,
}

/// Traduit une ligne de la table `session` en objet publiable.
///
/// Equivaut ligne pour ligne a `fromRow` de l'original, ternaires et coalescent
/// compris.
pub fn from_row(row: &SessionRow) -> Info {
    Info {
        id: row.id.clone(),
        // `row.parent_id ? ... : undefined` : la chaine vide disparait.
        parent_id: truthy(&row.parent_id).map(str::to_string),
        project_id: row.project_id.clone(),
        // Meme regle que pour le parent.
        agent: truthy(&row.agent).map(str::to_string),
        // `row.model ? {...} : undefined` : la colonne est un objet ou null, donc
        // ici seule la nullite compte. Le `variant` interne, lui, passe par un
        // coalescent, et c'est le seul endroit du fichier ou une chaine vide
        // doit survivre.
        model: row.model.as_ref().map(|model| ModelRef {
            id: model.id.clone(),
            provider_id: model.provider_id.clone(),
            variant: Some(present_or(model.variant.as_deref(), DEFAULT_VARIANT)),
        }),
        cost: row.cost,
        title: row.title.clone(),
        // Les cinq colonnes sont plates en base et imbriquees dans le contrat.
        tokens: Tokens {
            input: row.tokens_input,
            output: row.tokens_output,
            reasoning: row.tokens_reasoning,
            cache: CacheTokens { read: row.tokens_cache_read, write: row.tokens_cache_write },
        },
        location: LocationRef {
            directory: row.directory.clone(),
            workspace_id: truthy(&row.workspace_id).map(str::to_string),
        },
        subpath: truthy(&row.path).map(str::to_string),
        revert: row.revert.clone(),
        time: Time {
            created: row.time_created,
            updated: row.time_updated,
            archived: row.time_archived.and_then(truthy_millis),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ligne minimale : uniquement les colonnes `NOT NULL`.
    fn row_minimale() -> SessionRow {
        SessionRow {
            id: "ses_1".to_string(),
            project_id: "pro_1".to_string(),
            title: "Sans titre".to_string(),
            directory: "C:\\projet".to_string(),
            cost: 0.0,
            tokens_input: 0.0,
            tokens_output: 0.0,
            tokens_reasoning: 0.0,
            tokens_cache_read: 0.0,
            tokens_cache_write: 0.0,
            time_created: 1_700_000_000_000,
            time_updated: 1_700_000_001_000,
            parent_id: None,
            workspace_id: None,
            agent: None,
            path: None,
            model: None,
            revert: None,
            time_archived: None,
        }
    }

    #[test]
    fn une_ligne_minimale_ne_produit_que_les_champs_obligatoires() {
        let info = from_row(&row_minimale());
        assert_eq!(info.id, "ses_1");
        assert_eq!(info.title, "Sans titre");
        assert_eq!(info.location.directory, "C:\\projet");
        assert_eq!(info.time.archived, None);
    }

    #[test]
    fn une_seule_ligne_donne_un_objet_unique() {
        // Cas limite du brief : un seul element, pas de liste.
        let info = from_row(&row_minimale());
        assert_eq!(info.tokens, Tokens::default());
        assert!(info.parent_id.is_none());
    }

    // -----------------------------------------------------------------------
    // Le piege : veracite (`?`) contre nullite (`??`)
    // -----------------------------------------------------------------------

    #[test]
    fn la_chaine_vide_disparait_sur_les_ternaires() {
        // `row.parent_id ? ... : undefined` : "" est falsy, donc absent.
        let mut row = row_minimale();
        row.parent_id = Some(String::new());
        row.agent = Some(String::new());
        row.path = Some(String::new());
        row.workspace_id = Some(String::new());

        let info = from_row(&row);
        assert_eq!(info.parent_id, None);
        assert_eq!(info.agent, None);
        assert_eq!(info.subpath, None);
        assert_eq!(info.location.workspace_id, None);
    }

    #[test]
    fn la_chaine_vide_survit_sur_le_coalescent_du_variant() {
        // Contre-exemple volontaire : `row.model.variant ?? "default"` teste la
        // nullite, pas la veracite. "" n'est pas nullish, donc il reste "".
        let mut row = row_minimale();
        row.model = Some(ModelRef {
            id: "gpt".to_string(),
            provider_id: "openai".to_string(),
            variant: Some(String::new()),
        });
        let info = from_row(&row);
        assert_eq!(info.model.unwrap().variant, Some(String::new()));
    }

    #[test]
    fn un_variant_absent_devient_default() {
        let mut row = row_minimale();
        row.model = Some(ModelRef {
            id: "gpt".to_string(),
            provider_id: "openai".to_string(),
            variant: None,
        });
        let info = from_row(&row);
        assert_eq!(info.model.unwrap().variant, Some(DEFAULT_VARIANT.to_string()));
    }

    #[test]
    fn un_horodatage_nul_disparait_aussi() {
        // `row.time_archived ? ... : undefined` : 0 est falsy en JavaScript.
        let mut row = row_minimale();
        row.time_archived = Some(0);
        assert_eq!(from_row(&row).time.archived, None);

        row.time_archived = Some(1_700_000_002_000);
        assert_eq!(from_row(&row).time.archived, Some(1_700_000_002_000));
    }

    #[test]
    fn la_veracite_et_la_nullite_donnent_des_reponses_opposees() {
        // Les deux tests ci-dessus isoles pourraient passer si l'un des deux etait
        // faux. Ici on les met face a face sur la meme fonction, pour qu'une
        // inversion des deux soit visible.
        let mut row = row_minimale();
        row.parent_id = Some(String::new());
        row.model = Some(ModelRef {
            id: "gpt".to_string(),
            provider_id: "openai".to_string(),
            variant: Some(String::new()),
        });
        let info = from_row(&row);
        assert_eq!(info.parent_id, None, "ternaire : la vide disparait");
        assert_eq!(info.model.unwrap().variant, Some(String::new()), "coalescent : la vide reste");
    }

    // -----------------------------------------------------------------------
    // Regroupement des compteurs
    // -----------------------------------------------------------------------

    #[test]
    fn les_compteurs_plate_de_bases_sont_regroupes() {
        let mut row = row_minimale();
        row.tokens_input = 10.0;
        row.tokens_output = 20.0;
        row.tokens_reasoning = 30.0;
        row.tokens_cache_read = 40.0;
        row.tokens_cache_write = 50.0;
        let json = serde_json::to_value(from_row(&row)).unwrap();

        assert_eq!(json["tokens"]["input"], 10.0);
        assert_eq!(json["tokens"]["output"], 20.0);
        assert_eq!(json["tokens"]["reasoning"], 30.0);
        assert_eq!(json["tokens"]["cache"]["read"], 40.0);
        assert_eq!(json["tokens"]["cache"]["write"], 50.0);
    }

    #[test]
    fn les_jetons_sont_des_flottants_et_non_des_entiers() {
        // `Schema.Finite` cote TS : un compteur fractionnaire doit survivre.
        let mut row = row_minimale();
        row.cost = 0.5;
        row.tokens_input = 10.25;
        let info = from_row(&row);
        assert_eq!(info.cost, 0.5);
        assert_eq!(info.tokens.input, 10.25);
    }

    // -----------------------------------------------------------------------
    // Aller-retour complet
    // -----------------------------------------------------------------------

    #[test]
    fn une_ligne_deserialisee_repasse_par_from_row() {
        // Ce que SQLite donne, ce que l'API renvoie.
        let json = r#"{
            "id":"ses_9","project_id":"pro_2","title":"T","directory":"/tmp",
            "cost":1.5,"tokens_input":1,"tokens_output":2,"tokens_reasoning":3,
            "tokens_cache_read":4,"tokens_cache_write":5,
            "time_created":100,"time_updated":200,"time_archived":300
        }"#;
        let row: SessionRow = serde_json::from_str(json).unwrap();
        let info = from_row(&row);
        assert_eq!(info.time.created, 100);
        assert_eq!(info.time.updated, 200);
        assert_eq!(info.time.archived, Some(300));
        assert_eq!(info.tokens.cache.write, 5.0);
    }

    #[test]
    fn deux_lignes_donneent_deux_objets_independants() {
        // Aucune donnee ne doit etre partagee entre deux conversions.
        let mut autre = row_minimale();
        autre.id = "ses_2".to_string();
        autre.parent_id = Some("ses_1".to_string());

        let premiere = from_row(&row_minimale());
        let seconde = from_row(&autre);
        assert_eq!(premiere.id, "ses_1");
        assert_eq!(seconde.parent_id.as_deref(), Some("ses_1"));
        assert!(premiere.parent_id.is_none());
    }
}
