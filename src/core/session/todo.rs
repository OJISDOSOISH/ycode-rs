//! Portage Rust de `opencode/packages/schema/src/session-todo.ts` et de
//! `opencode/packages/core/src/session/todo.ts`.
//!
//! La liste de taches d'une session. Elle est stockee en base et lue par ordre
//! de `position`, qui est l'ordre d'insertion, pas un rang explicite.
//!
//! Choix de portage : `status` et `priority` sont des `String` libres dans le
//! TypeScript, sans contrainte de valeurs. On les garde en `String` plutot que
//! d'en faire des enums. Introduire un enum serait une invention : le TS accepte
//! "urgent" comme `priority`, et le portage doit accepter exactement ce que
//! l'original accepte. C'est le genre d'amelioration qu'on ne fait pas pendant un
//! portage, parce qu'elle change le comportement observable.

use serde::{Deserialize, Serialize};

/// Une tache de la session.
///
/// Le nom `Info` est celui du TS. On le garde plutot que de renommer en `Todo` :
/// le portage doit rester lisible face a l'original, et chercher `Info` dans le
/// TS doit mener au meme nom ici.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Info {
    /// Description breve de la tache.
    pub content: String,
    /// Etat courant : `pending`, `in_progress`, `completed` ou `cancelled`.
    ///
    /// Libre par construction, comme dans le TS. Aucune validation n'est
    /// appliquee ici : le modele peut ecrire n'importe quelle chaine.
    pub status: String,
    /// Priorite : `high`, `medium` ou `low`. Libre egalement.
    pub priority: String,
}

impl Info {
    pub fn new(content: impl Into<String>, status: impl Into<String>, priority: impl Into<String>) -> Self {
        Self { content: content.into(), status: status.into(), priority: priority.into() }
    }

    /// La tache est-elle terminee, sous quelque libelle que ce soit ?
    ///
    /// Utilitaire absent du TS, ajoute parce que la question sera posee partout
    /// ou l'on manipule des taches. La comparaison est insensible a la casse et
    /// tolere les espaces, parce que les valeurs viennent d'un modele.
    pub fn is_done(&self) -> bool {
        matches!(self.status.trim().to_ascii_lowercase().as_str(), "completed" | "cancelled" | "done")
    }
}

/// Evenement emis quand la liste de taches change.
///
/// Le TS definit cet evenement via `define({ type: "todo.updated", ... })` et
/// l'inventorie. Ici c'est une structure simple ; l'inventaire d'evenements est
/// une mecanique globale, portee ailleurs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatedEvent {
    #[serde(rename = "sessionID")]
    pub session_id: String,
    pub todos: Vec<Info>,
}

/// Ligne de base, avec sa position.
///
/// `position` n'existe pas dans `Info` : c'est un artefact de stockage, l'ordre
/// d'insertion. On le garde dans une structure distincte plutot que de le
/// melanger au domaine, sinon il finirait par etre expose par erreur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub content: String,
    pub status: String,
    pub priority: String,
    pub position: i64,
}

impl Row {
    /// Projette vers le domaine, en perdant la position.
    pub fn to_info(&self) -> Info {
        Info { content: self.content.clone(), status: self.status.clone(), priority: self.priority.clone() }
    }
}

/// Prepare l'insertion d'une liste de taches.
///
/// Equivalent de l'operation `update` du TS : suppression de toutes les lignes de
/// la session, puis reinsertion complete. C'est un remplacement integral, pas une
/// fusion, ce qui est plus simple et evite les etats intermediaires ou une tache
/// disparaitrait.
///
/// Le TS n'insere rien si la liste est vide, apres avoir tout supprime. On
/// renvoie une liste vide : l'appelant n'a rien a executer.
pub fn prepare_rows(session_id: &str, todos: &[Info]) -> Vec<(String, Info, i64)> {
    if todos.is_empty() {
        return Vec::new();
    }
    todos
        .iter()
        .enumerate()
        .map(|(i, todo)| (session_id.to_string(), todo.clone(), i as i64))
        .collect()
}

/// Lit la liste de taches, dans l'ordre.
///
/// Le TS fait un `ORDER BY position ASC` en base. Ici on suppose les lignes deja
/// triees, comme pour `history::load` : trier une fois en base coute une
/// operation de moins qu'un tri en memoire sur un resultat deja ordonne.
pub fn read(rows: &[Row]) -> Vec<Info> {
    rows.iter().map(Row::to_info).collect()
}

/// Compte les taches restantes.
///
/// Petit utilitaire absent du TS, mais necessaire a un tableau de bord de tache :
/// sans lui, chaque appelant recompte la liste.
pub fn remaining(todos: &[Info]) -> usize {
    todos.iter().filter(|t| !t.is_done()).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(content: &str, status: &str, priority: &str) -> Info {
        Info::new(content, status, priority)
    }

    #[test]
    fn les_champs_ont_les_bons_noms_json() {
        // Le TS utilise `content`, `status`, `priority` : deja en minuscules,
        // donc pas de rename necessaire. Ce test verrouille ce point.
        let v = serde_json::to_value(t("ecrire le test", "pending", "high")).unwrap();
        assert_eq!(v["content"], "ecrire le test");
        assert_eq!(v["status"], "pending");
        assert_eq!(v["priority"], "high");
    }

    #[test]
    fn l_evenement_serialise_session_id_en_camelcase() {
        // Meme piege que `sessionID` dans les messages : Serde emettrait
        // `session_id` sans le rename.
        let e = UpdatedEvent { session_id: "ses_1".into(), todos: vec![] };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["sessionID"], "ses_1");
        assert!(v.get("session_id").is_none());
    }

    #[test]
    fn les_positions_sont_attribuees_dans_l_ordre() {
        let todos = vec![t("a", "pending", "low"), t("b", "pending", "low"), t("c", "pending", "low")];
        let rows = prepare_rows("ses_1", &todos);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].2, 0);
        assert_eq!(rows[1].2, 1);
        assert_eq!(rows[2].2, 2);
    }

    #[test]
    fn une_liste_vide_ne_prepare_rien() {
        // Le TS supprime tout puis n insere rien. On renvoie une liste vide,
        // l appelant n execute aucune insertion.
        assert!(prepare_rows("ses_1", &[]).is_empty());
    }

    #[test]
    fn la_lecture_projette_et_enleve_la_position() {
        let rows = vec![
            Row { content: "a".into(), status: "pending".into(), priority: "low".into(), position: 0 },
            Row { content: "b".into(), status: "completed".into(), priority: "high".into(), position: 1 },
        ];
        let todos = read(&rows);
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[1].content, "b");
    }

    #[test]
    fn un_statut_libre_est_accepte() {
        // Le TS n impose aucune valeur. Un enum aurait refuse "urgent" ici.
        let tache = t("tache", "urgent", "highest");
        assert_eq!(tache.status, "urgent");
        assert!(!tache.is_done());
    }

    #[test]
    fn is_done_tolere_casse_et_espaces() {
        assert!(t("x", "completed", "low").is_done());
        assert!(t("x", "  COMPLETED ", "low").is_done());
        assert!(t("x", "cancelled", "low").is_done());
        assert!(!t("x", "in_progress", "low").is_done());
    }

    #[test]
    fn remaining_compte_les_taches_non_terminees() {
        let todos = vec![
            t("a", "pending", "low"),
            t("b", "completed", "low"),
            t("c", "in_progress", "high"),
        ];
        assert_eq!(remaining(&todos), 2);
    }
}
