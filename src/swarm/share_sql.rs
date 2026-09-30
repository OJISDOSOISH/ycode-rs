//! Rust port of `opencode/packages/core/src/share/sql.ts`.
//!
//! ## What the source actually contains
//!
//! The file is thirteen lines and **twelve of them carry no information**: an
//! `import` of `sqliteTable` and `text`, then a spread of `Timestamps`. The
//! only real content is the table below.
//!
//! ## The part worth reading: `primaryKey` AND `references`
//!
//! `session_id` carries `.primaryKey()` AND `.references(..., { onDelete:
//! "cascade" })`, but **not** `.notNull()`. That is exactly the shape another
//! agent in this batch verified produces the same DDL as
//! `.notNull().primaryKey()`: the `drizzle-kit` snapshot records
//! `"notNull": false` in both cases, because the Drizzle model holds `notNull`
//! and `primaryKey` as **two independent flags**, not one.
//!
//! Consequence, and that is the whole lesson of this file: never derive
//! `not_null` from the fact that a column is a primary key. Both flags are
//! here, separately, each with its own value.
//!
//! ## What this file is not
//!
//! There is no SQL layer, no migration, no dependency: `Cargo.toml` declares
//! no SQL crate, and a project decision is to wire SQL up later. This module
//! therefore carries only the STRUCTURE of the table and the invariants the
//! source declares, as pure functions testable on a slice of data.

use serde::{Deserialize, Serialize};

/// What the source writes on a column, with nothing inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnFlags {
    /// What the source writes explicitly through `.notNull()`.
    ///
    /// NEVER derive this flag from the column being a primary key: in the
    /// Drizzle model the two are independent.
    pub not_null: bool,
    /// What the source writes explicitly through `.primaryKey()`.
    pub primary_key: bool,
}

impl ColumnFlags {
    /// An ordinary column: neither `notNull` nor `primaryKey`.
    pub const PLAIN: Self = Self { not_null: false, primary_key: false };
}

/// Action referenced when the parent row is deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReferenceAction {
    /// The row disappears with its parent.
    Cascade,
}

/// A foreign key reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignKey {
    /// Target table, named as the source names it.
    pub table: &'static str,
    /// Target column.
    pub column: &'static str,
    /// Behaviour on parent deletion.
    pub on_delete: ReferenceAction,
}

/// A column of the `session_share` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    /// Column name, in the exact casing of the source.
    pub name: &'static str,
    /// What the source writes, with no interpretation.
    pub flags: ColumnFlags,
    /// Reference, or `None` when the column declares none.
    pub reference: Option<ForeignKey>,
}

/// Table name, as passed to `sqliteTable`.
pub const TABLE: &str = "session_share";

/// The columns, in source declaration order.
pub const COLUMNS: &[Column] = &[
    Column {
        name: "session_id",
        flags: ColumnFlags { not_null: false, primary_key: true },
        reference: Some(ForeignKey {
            table: "session",
            column: "id",
            on_delete: ReferenceAction::Cascade,
        }),
    },
    Column { name: "id", flags: ColumnFlags { not_null: true, primary_key: false }, reference: None },
    Column { name: "secret", flags: ColumnFlags { not_null: true, primary_key: false }, reference: None },
    Column { name: "url", flags: ColumnFlags { not_null: true, primary_key: false }, reference: None },
];

/// The two `Timestamps` columns, added by the spread.
///
/// The source spreads `...Timestamps`, which adds `time_created` and
/// `time_updated` with no null constraint. The detail is therefore the same as
/// for the primary key: nothing is assumed, it is read.
pub const TIMESTAMPS: &[&str] = &["time_created", "time_updated"];

/// Every column, timestamps included, in source order.
pub fn all_columns() -> Vec<&'static str> {
    let mut all: Vec<&'static str> = COLUMNS.iter().map(|c| c.name).collect();
    all.extend_from_slice(TIMESTAMPS);
    all
}

/// The primary key column, or `None` when the table has none.
///
/// A function rather than an index, because the primary key can move if the
/// source changes. Searching instead of hardcoding a name avoids leaving a
/// stale index behind on a later edit.
pub fn primary_key() -> Option<&'static Column> {
    COLUMNS.iter().find(|c| c.flags.primary_key)
}

/// The columns the source declares mandatory.
///
/// `session_id` is NOT among them, and that is the point of the file: the
/// source writes `.primaryKey()` without `.notNull()`, so it never declared it
/// mandatory. An agent that derived nullability from the primary key would
/// tighten the contract and reject rows the source accepts.
pub fn mandatory_columns() -> Vec<&'static str> {
    COLUMNS.iter().filter(|c| c.flags.not_null).map(|c| c.name).collect()
}

/// The declared references, in source order.
pub fn foreign_keys() -> Vec<(&'static str, ForeignKey)> {
    COLUMNS.iter().filter_map(|c| c.reference.map(|r| (c.name, r))).collect()
}

/// A row of the table, as an associative slice.
///
/// The source declares no function, so there is no business logic to port: the
/// only thing verifiable is the structure, which the functions above do.
pub type Row = std::collections::BTreeMap<&'static str, &'static str>;

/// Checks that a row satisfies the declared NOT NULL constraints.
///
/// The only check derivable from the source, since the rest of the SQL
/// behaviour is not ported. The name says exactly what it verifies: the
/// declared constraints, not every possible constraint.
pub fn satisfies_mandatory(row: &Row) -> bool {
    mandatory_columns().iter().all(|name| row.get(name).is_some_and(|v| !v.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_has_the_right_name_and_columns() {
        // Order matters: the source declares `session_id` first, and that order
        // is what places the timestamps after the spread.
        assert_eq!(TABLE, "session_share");
        assert_eq!(all_columns(), vec![
            "session_id", "id", "secret", "url", "time_created", "time_updated"
        ]);
    }

    // -----------------------------------------------------------------------
    // The central trap: the two flags are independent
    // -----------------------------------------------------------------------

    #[test]
    fn a_primary_key_is_not_necessarily_mandatory() {
        // THE POINT OF THIS FILE. `session_id` carries `.primaryKey()` WITHOUT
        // `.notNull()`. An agent that derived nullability from the primary key
        // would fail this test, and the contract would carry a constraint the
        // source does not impose.
        let session_id = COLUMNS.iter().find(|c| c.name == "session_id").unwrap();
        assert!(session_id.flags.primary_key, "the column is a primary key");
        assert!(!session_id.flags.not_null, "the source never declared it mandatory");
    }

    #[test]
    fn the_primary_key_is_the_only_one_and_is_found_by_search() {
        let pk = primary_key().expect("the table has a primary key");
        assert_eq!(pk.name, "session_id");
        // A single column carries the flag, otherwise the table is invalid.
        assert_eq!(COLUMNS.iter().filter(|c| c.flags.primary_key).count(), 1);
    }

    #[test]
    fn the_mandatory_columns_are_the_ones_the_source_declares() {
        // `id`, `secret` and `url` carry `.notNull()`. `session_id` does not.
        assert_eq!(mandatory_columns(), vec!["id", "secret", "url"]);
    }

    // -----------------------------------------------------------------------
    // The foreign key
    // -----------------------------------------------------------------------

    #[test]
    fn the_only_reference_is_a_cascade_to_session() {
        let refs = foreign_keys();
        assert_eq!(refs.len(), 1, "the source declares a single reference");
        assert_eq!(refs[0].0, "session_id");
        assert_eq!(refs[0].1.table, "session");
        assert_eq!(refs[0].1.column, "id");
        assert_eq!(refs[0].1.on_delete, ReferenceAction::Cascade);
    }

    #[test]
    fn the_other_columns_declare_no_reference() {
        for name in ["id", "secret", "url"] {
            let c = COLUMNS.iter().find(|c| c.name == name).unwrap();
            assert!(c.reference.is_none(), "{name} declares no reference");
        }
    }

    // -----------------------------------------------------------------------
    // The timestamps
    // -----------------------------------------------------------------------

    #[test]
    fn timestamps_come_after_the_explicit_columns() {
        // `...Timestamps` is spread LAST, so the timestamps sit at positions 4
        // and 5, not 0 and 1.
        let all = all_columns();
        let i_created = all.iter().position(|c| *c == "time_created").unwrap();
        let i_updated = all.iter().position(|c| *c == "time_updated").unwrap();
        assert!(i_created > 3 && i_updated > 3, "timestamps are appended at the end");
    }

    #[test]
    fn timestamps_are_absent_from_the_explicit_column_list() {
        // The two lists are kept separate: `COLUMNS` holds only what the source
        // names field by field.
        assert_eq!(COLUMNS.len(), 4);
        for name in TIMESTAMPS {
            assert!(!COLUMNS.iter().any(|c| c.name == *name), "{name} must not be duplicated");
        }
    }

    // -----------------------------------------------------------------------
    // The only behaviour check derivable from the source
    // -----------------------------------------------------------------------

    #[test]
    fn a_complete_row_satisfies_the_mandatory_columns() {
        let mut row = Row::new();
        for c in COLUMNS.iter().filter(|c| c.flags.not_null) {
            row.insert(c.name, "value");
        }
        // `session_id` is not mandatory, it is left out on purpose.
        assert!(satisfies_mandatory(&row));
    }

    #[test]
    fn a_missing_mandatory_column_is_rejected() {
        let mut row = Row::new();
        for c in COLUMNS.iter().filter(|c| c.flags.not_null) {
            row.insert(c.name, "value");
        }
        row.remove("secret");
        assert!(!satisfies_mandatory(&row));
    }

    #[test]
    fn an_empty_mandatory_value_is_rejected() {
        // An empty string differs from an absent value: the column exists but
        // holds nothing. Same reasoning as the `?` trap.
        let mut row = Row::new();
        for c in COLUMNS.iter().filter(|c| c.flags.not_null) {
            row.insert(c.name, "value");
        }
        row.insert("url", "");
        assert!(!satisfies_mandatory(&row));
    }
}
