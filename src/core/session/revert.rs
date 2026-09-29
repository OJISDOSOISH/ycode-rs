//! Portage Rust de `opencode/packages/core/src/session/revert.ts`.
//!
//! Revenir en arriere dans une session doit restaurer les fichiers ecrits apres
//! le point de retour. Le point delicat n'est pas la restauration, mais **lequel**
//! des snapshots d'un fichier_f 使用 le premier, et non le dernier.
//!
//! Exemple : l'agent edite `a.ts` au tour 3, puis encore au tour 7. Pour revenir
//! au tour 2, il faut le snapshot d'avant le tour 3. Prendre le dernier snapshot
//! (avant le tour 7) laisserait le fichier dans l'etat du tour 3 : l'annulation
//! serait incomplete, sans que rien ne signale l'echec.
//!
//! C'est exactement ce que fait le `if (!files.has(file))` de l'original, et
//! c'est le genre de detail qui disparait silencieusement dans un portage.

use std::collections::BTreeMap;

/// Identifiant d'un instantane de fichier.
///
/// L'original utilise `Snapshot.ID`, une chaine opaque. On garde `String` plutot
/// qu'un type nomme, parce que rien ici n'a besoin de la valider.
pub type SnapshotId = String;

/// Association fichier a restaurer, et instantane depuis lequel le restaurer.
///
/// `BTreeMap` et non `HashMap` : l'ordre de restauration doit etre deterministe,
/// sinon deux executions produisent des systemes de fichiers differents et le
/// diff devient incomparable.
pub type RevertPlan = BTreeMap<String, SnapshotId>;

/// Ce qu'un message d'assistant apporte a la restauration.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Instantane de debut d'etape. `None` si l'etape n'a rien modifie.
    pub start: Option<SnapshotId>,
    /// Fichiers concernes par cet instantane.
    pub files: Vec<String>,
}

/// Erreur de domaine : le message de retour en arriere n'existe pas.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("message introuvable dans la session : {message_id}")]
pub struct MessageNotFound {
    pub session_id: String,
    pub message_id: String,
}

/// Construit le plan de restauration depuis les messages assistants posterieurs
/// au point de retour.
///
/// `boundary_seq` est le rang du message de retour. Seuls les messages
/// **assistants** le suivant sont pris en compte : un message utilisateur ne
/// touche pas de fichier, et l'inclure fausserait le premier-snapshot.
///
/// Les entrees sont supposees triees par rang croissant, comme le `ORDER BY seq`
/// de l'original. La fonction ne retrie pas : elle fait confiance a l'appelant,
/// qui a deja la base sous la main.
pub fn plan(
    session_id: &str,
    boundary_seq: i64,
    entries: &[(i64, Snapshot)],
) -> Result<RevertPlan, MessageNotFound> {
    let mut files: RevertPlan = BTreeMap::new();

    for (seq, snapshot) in entries {
        // Anterieur au point de retour : ce qui s'est passe avant ne se
        // restaure pas, on garde ce qui existe.
        if *seq <= boundary_seq {
            continue;
        }
        // Sans instantane de depart, l'etape n'a rien ecrit : rien a restaurer.
        let Some(start) = &snapshot.start else {
            continue;
        };

        for file in &snapshot.files {
            // Le PREMIER snapshot gagne. C'est le point central de ce module.
            files.entry(file.clone()).or_insert_with(|| start.clone());
        }
    }

    Ok(files)
}

/// Verifie qu'un message de retour existe, pour produire l'erreur de domaine.
pub fn check_boundary(session_id: &str, message_id: &str, entries: &[(i64, String)]) -> Result<(), MessageNotFound> {
    if entries.iter().any(|(_, id)| id == message_id) {
        Ok(())
    } else {
        Err(MessageNotFound { session_id: session_id.to_string(), message_id: message_id.to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(start: Option<&str>, files: &[&str]) -> Snapshot {
        Snapshot {
            start: start.map(String::from),
            files: files.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn rien_a_rRestaurer_sans_modification() {
        let entries = vec![(3, snap(Some("s1"), &[]))];
        assert_eq!(plan("ses_1", 2, &entries).unwrap().len(), 0);
    }

    #[test]
    fn un_fichier_touche_est_restaure() {
        let entries = vec![(3, snap(Some("s1"), &["a.rs"]))];
        let p = plan("ses_1", 2, &entries).unwrap();
        assert_eq!(p.get("a.rs"), Some(&"s1".to_string()));
    }

    #[test]
    fn le_premier_snapshot_gagne_pour_un_fichier_edite_plusieurs_fois() {
        // Le cas central : `a.rs` edite au tour 3 puis au tour 7.
        // Revenir au tour 2 doit restaurer l'etat AVANT le tour 3.
        let entries = vec![
            (3, snap(Some("avant_tour3"), &["a.rs"])),
            (7, snap(Some("avant_tour7"), &["a.rs"])),
        ];
        let p = plan("ses_1", 2, &entries).unwrap();
        assert_eq!(
            p.get("a.rs"),
            Some(&"avant_tour3".to_string()),
            "le snapshot du tour 3, pas celui du tour 7"
        );
    }

    #[test]
    fn les_messages_anterieurs_au_point_de_retour_sont_ignores() {
        let entries = vec![
            (1, snap(Some("avant_1"), &["a.rs"])),
            (5, snap(Some("avant_5"), &["a.rs"])),
        ];
        let p = plan("ses_1", 3, &entries).unwrap();
        // Seul le message 5 compte.
        assert_eq!(p.get("a.rs"), Some(&"avant_5".to_string()));
    }

    #[test]
    fn un_message_sans_instantane_n_ecrase_pas_un_snapshot_existant() {
        // Un etape qui n'a rien modifie ne doit pas faire disparaitre le
        // snapshot deja retenu pour ce fichier.
        let entries = vec![
            (3, snap(Some("avant_3"), &["a.rs"])),
            (5, snap(None, &["a.rs"])),
        ];
        let p = plan("ses_1", 2, &entries).unwrap();
        assert_eq!(p.get("a.rs"), Some(&"avant_3".to_string()));
    }

    #[test]
    fn plusieurs_fichiers_sont_restores_independantamment() {
        let entries = vec![
            (3, snap(Some("s3"), &["a.rs"])),
            (4, snap(Some("s4"), &["b.rs"])),
            (5, snap(Some("s5"), &["a.rs", "c.rs"])),
        ];
        let p = plan("ses_1", 2, &entries).unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!(p.get("a.rs"), Some(&"s3".to_string()));
        assert_eq!(p.get("b.rs"), Some(&"s4".to_string()));
        assert_eq!(p.get("c.rs"), Some(&"s5".to_string()));
    }

    #[test]
    fn le_plan_est_deterministe() {
        // Deux appels successifs doivent produire exactement le meme plan,
        // sinon la restauration n'est pas reproductible.
        let entries = vec![(3, snap(Some("s3"), &["z.rs", "a.rs", "m.rs"]))];
        let p1 = plan("ses_1", 2, &entries).unwrap();
        let p2 = plan("ses_1", 2, &entries).unwrap();
        assert_eq!(p1, p2);
        let cles: Vec<&str> = p1.keys().map(|s| s.as_str()).collect();
        assert_eq!(cles, vec!["a.rs", "m.rs", "z.rs"]);
    }

    #[test]
    fn un_message_de_retour_inconnu_est_signale() {
        let entries = vec![(1, "msg_1".to_string())];
        assert!(check_boundary("ses_1", "msg_1", &entries).is_ok());
        let err = check_boundary("ses_1", "msg_absent", &entries).unwrap_err();
        assert_eq!(err.message_id, "msg_absent");
    }
}
