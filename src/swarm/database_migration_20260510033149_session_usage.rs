//! Portage de `packages/core/src/database/migration/20260510033149_session_usage.ts`.
//!
//! La source ajoute six colonnes d'usage a la table `session`, toutes non
//! nulles avec defaut zero, puis recalcule leur contenu depuis la table
//! `message` pour les lignes existantes :
//!
//! ```sql
//! ALTER TABLE `session` ADD `cost` real DEFAULT 0 NOT NULL;
//! ALTER TABLE `session` ADD `tokens_input` integer DEFAULT 0 NOT NULL;
//! ALTER TABLE `session` ADD `tokens_output` integer DEFAULT 0 NOT NULL;
//! ALTER TABLE `session` ADD `tokens_reasoning` integer DEFAULT 0 NOT NULL;
//! ALTER TABLE `session` ADD `tokens_cache_read` integer DEFAULT 0 NOT NULL;
//! ALTER TABLE `session` ADD `tokens_cache_write` integer DEFAULT 0 NOT NULL;
//! UPDATE session SET cost = ..., tokens_input = ..., ... ;
//! ```
//!
//! Le `UPDATE` somme, par session, les champs `cost` et `tokens.*` des messages
//! de role `assistant`, avec `coalesce(..., 0)` a chaque niveau : un champ
//! absent vaut zero, et une session sans message assistant vaut zero partout.
//! Comme `Cargo.toml` ne declare aucun pilote SQL, ce fichier ne touche a
//! aucune base ; il porte l'identifiant, les instructions, et l'agregation du
//! `UPDATE` sous forme d'une fonction pure testable sans base.

/// Identifiant de la migration, tel que declare dans la source.
pub const MIGRATION_ID: &str = "20260510033149_session_usage";

/// Nom de la table visee.
pub const TABLE_SESSION: &str = "session";

/// Les six colonnes ajoutees, dans l'ordre des `ALTER TABLE`.
pub const COLONNES_AJOUTEES: [&str; 6] = [
    "cost",
    "tokens_input",
    "tokens_output",
    "tokens_reasoning",
    "tokens_cache_read",
    "tokens_cache_write",
];

/// Les six `ALTER TABLE`, dans l'ordre de la source.
pub const ALTER_STATEMENTS: [&str; 6] = [
    "ALTER TABLE `session` ADD `cost` real DEFAULT 0 NOT NULL;",
    "ALTER TABLE `session` ADD `tokens_input` integer DEFAULT 0 NOT NULL;",
    "ALTER TABLE `session` ADD `tokens_output` integer DEFAULT 0 NOT NULL;",
    "ALTER TABLE `session` ADD `tokens_reasoning` integer DEFAULT 0 NOT NULL;",
    "ALTER TABLE `session` ADD `tokens_cache_read` integer DEFAULT 0 NOT NULL;",
    "ALTER TABLE `session` ADD `tokens_cache_write` integer DEFAULT 0 NOT NULL;",
];

/// Le `UPDATE` de remplissage, tel qu'execute par `up`.
///
/// La chaine reprend la requete de la source : sommes par session des champs
/// des messages `assistant`, avec `coalesce(..., 0)` a chaque niveau.
pub const BACKFILL_SQL: &str = "UPDATE session SET cost = coalesce((SELECT sum(coalesce(json_extract(message.data, '$.cost'), 0)) FROM message WHERE message.session_id = session.id AND json_extract(message.data, '$.role') = 'assistant'), 0), tokens_input = coalesce((SELECT sum(coalesce(json_extract(message.data, '$.tokens.input'), 0)) FROM message WHERE message.session_id = session.id AND json_extract(message.data, '$.role') = 'assistant'), 0), tokens_output = coalesce((SELECT sum(coalesce(json_extract(message.data, '$.tokens.output'), 0)) FROM message WHERE message.session_id = session.id AND json_extract(message.data, '$.role') = 'assistant'), 0), tokens_reasoning = coalesce((SELECT sum(coalesce(json_extract(message.data, '$.tokens.reasoning'), 0)) FROM message WHERE message.session_id = session.id AND json_extract(message.data, '$.role') = 'assistant'), 0), tokens_cache_read = coalesce((SELECT sum(coalesce(json_extract(message.data, '$.tokens.cache.read'), 0)) FROM message WHERE message.session_id = session.id AND json_extract(message.data, '$.role') = 'assistant'), 0), tokens_cache_write = coalesce((SELECT sum(coalesce(json_extract(message.data, '$.tokens.cache.write'), 0)) FROM message WHERE message.session_id = session.id AND json_extract(message.data, '$.role') = 'assistant'), 0)";

/// L'identifiant de la migration.
pub fn id() -> &'static str {
    MIGRATION_ID
}

/// Le nombre d'instructions : six `ALTER` puis un `UPDATE`.
pub fn nombre_instructions() -> usize {
    ALTER_STATEMENTS.len() + 1
}

/// Un message tel que le `UPDATE` le lit : seuls le role et les compteurs
/// comptent, et un compteur absent vaut zero par `coalesce`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MessageUsage {
    /// Role du message (`assistant` est le seul pris en compte).
    pub role: String,
    /// `json_extract(data, '$.cost')`, absent vaut zero.
    pub cost: f64,
    /// `json_extract(data, '$.tokens.input')`.
    pub tokens_input: i64,
    /// `json_extract(data, '$.tokens.output')`.
    pub tokens_output: i64,
    /// `json_extract(data, '$.tokens.reasoning')`.
    pub tokens_reasoning: i64,
    /// `json_extract(data, '$.tokens.cache.read')`.
    pub tokens_cache_read: i64,
    /// `json_extract(data, '$.tokens.cache.write')`.
    pub tokens_cache_write: i64,
}

/// La ligne d'usage que le `UPDATE` ecrit sur `session`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionUsage {
    /// Somme des couts des messages `assistant`, zero sans message.
    pub cost: f64,
    /// Somme des jetons d'entree.
    pub tokens_input: i64,
    /// Somme des jetons de sortie.
    pub tokens_output: i64,
    /// Somme des jetons de raisonnement.
    pub tokens_reasoning: i64,
    /// Somme des lectures de cache.
    pub tokens_cache_read: i64,
    /// Somme des ecritures de cache.
    pub tokens_cache_write: i64,
}

/// L'agregation du `UPDATE` : somme des compteurs des seuls messages de role
/// `assistant`.
///
/// Une tranche vide rend des zeros, comme le `coalesce(..., 0)` externe de la
/// requete sur une session sans message assistant.
pub fn agrege_usage(messages: &[MessageUsage]) -> SessionUsage {
    let mut total = SessionUsage::default();
    for message in messages {
        if message.role != "assistant" {
            continue;
        }
        total.cost += message.cost;
        total.tokens_input += message.tokens_input;
        total.tokens_output += message.tokens_output;
        total.tokens_reasoning += message.tokens_reasoning;
        total.tokens_cache_read += message.tokens_cache_read;
        total.tokens_cache_write += message.tokens_cache_write;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message_assistant(cost: f64, input: i64, output: i64) -> MessageUsage {
        MessageUsage {
            role: "assistant".to_string(),
            cost,
            tokens_input: input,
            tokens_output: output,
            tokens_reasoning: 0,
            tokens_cache_read: 0,
            tokens_cache_write: 0,
        }
    }

    #[test]
    fn l_identifiant_reprend_exactement_celui_de_la_source() {
        assert_eq!(id(), "20260510033149_session_usage");
        assert_eq!(MIGRATION_ID, "20260510033149_session_usage");
    }

    #[test]
    fn les_six_alter_ajoutent_des_colonnes_non_nulles_avec_defaut_zero() {
        assert_eq!(COLONNES_AJOUTEES.len(), 6);
        assert_eq!(ALTER_STATEMENTS.len(), 6);
        assert_eq!(nombre_instructions(), 7);
        for instruction in ALTER_STATEMENTS {
            assert!(instruction.contains("`session`"), "hors table : {instruction}");
            assert!(instruction.contains("DEFAULT 0"), "sans defaut : {instruction}");
            assert!(instruction.contains("NOT NULL"), "nullable : {instruction}");
        }
    }

    #[test]
    fn la_colonne_de_cout_est_reelle_et_les_compteurs_sont_entiers() {
        assert!(ALTER_STATEMENTS[0].contains("`cost` real"));
        for instruction in &ALTER_STATEMENTS[1..] {
            assert!(instruction.contains("integer"), "compteur non entier : {instruction}");
        }
        assert_eq!(
            COLONNES_AJOUTEES,
            [
                "cost",
                "tokens_input",
                "tokens_output",
                "tokens_reasoning",
                "tokens_cache_read",
                "tokens_cache_write"
            ]
        );
    }

    #[test]
    fn le_remplissage_ne_lit_que_le_role_assistant() {
        assert!(BACKFILL_SQL.contains("'assistant'"));
        assert!(BACKFILL_SQL.contains("message.session_id = session.id"));
        assert!(BACKFILL_SQL.contains("coalesce"));
    }

    #[test]
    fn le_remplissage_couvre_les_six_colonnes() {
        for colonne in ["cost", "tokens_input", "tokens_output", "tokens_reasoning"] {
            assert!(BACKFILL_SQL.contains(colonne), "colonne absente : {colonne}");
        }
        assert!(BACKFILL_SQL.contains("tokens_cache_read"));
        assert!(BACKFILL_SQL.contains("tokens_cache_write"));
    }

    #[test]
    fn une_session_sans_message_vaut_zero_partout_comme_coalesce() {
        let usage = agrege_usage(&[]);
        assert_eq!(usage, SessionUsage::default());
        assert_eq!(usage.cost, 0.0);
        assert_eq!(usage.tokens_input, 0);
    }

    #[test]
    fn les_messages_non_assistant_sont_ignores() {
        let messages = vec![
            MessageUsage { role: "user".to_string(), cost: 10.0, tokens_input: 100, ..Default::default() },
            message_assistant(1.5, 10, 20),
        ];
        let usage = agrege_usage(&messages);
        assert_eq!(usage.cost, 1.5);
        assert_eq!(usage.tokens_input, 10);
        assert_eq!(usage.tokens_output, 20);
    }

    #[test]
    fn les_compteurs_des_messages_assistant_sont_sommes() {
        let messages = vec![
            message_assistant(1.0, 10, 20),
            message_assistant(2.5, 30, 40),
        ];
        let usage = agrege_usage(&messages);
        assert_eq!(usage.cost, 3.5);
        assert_eq!(usage.tokens_input, 40);
        assert_eq!(usage.tokens_output, 60);
    }

    #[test]
    fn les_champs_de_cache_et_de_raisonnement_sont_sommes_separement() {
        let messages = vec![MessageUsage {
            role: "assistant".to_string(),
            cost: 0.0,
            tokens_input: 0,
            tokens_output: 0,
            tokens_reasoning: 7,
            tokens_cache_read: 3,
            tokens_cache_write: 5,
        }];
        let usage = agrege_usage(&messages);
        assert_eq!(usage.tokens_reasoning, 7);
        assert_eq!(usage.tokens_cache_read, 3);
        assert_eq!(usage.tokens_cache_write, 5);
    }
}
