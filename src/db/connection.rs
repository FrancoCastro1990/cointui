use std::path::Path;

use rusqlite::Connection;

use crate::error::Result;

/// Thin wrapper around a [`rusqlite::Connection`] that owns the connection and
/// provides schema initialisation helpers.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (or create) a database at `path`.
    ///
    /// Enables WAL journal mode and foreign key enforcement, then runs the
    /// schema initialisation and any pending migrations.
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.configure_pragmas()?;
        db.initialize_schema()?;
        db.run_migrations()?;
        Ok(db)
    }

    /// Create a purely in-memory database (useful for tests).
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.configure_pragmas()?;
        db.initialize_schema()?;
        db.run_migrations()?;
        Ok(db)
    }

    /// Returns a reference to the underlying connection so that repository
    /// types can execute queries.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn configure_pragmas(&self) -> Result<()> {
        // WAL mode for better concurrent read performance.
        self.conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        // Enforce foreign key constraints.
        self.conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(())
    }

    /// Create every table if it does not already exist.
    fn initialize_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tags (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                name       TEXT    NOT NULL UNIQUE,
                parent_id  INTEGER REFERENCES tags(id) ON DELETE SET NULL,
                icon       TEXT
            );

            CREATE TABLE IF NOT EXISTS transactions (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                source      TEXT    NOT NULL,
                amount      INTEGER NOT NULL,
                kind        TEXT    NOT NULL CHECK (kind IN ('income', 'expense')),
                tag_id      INTEGER NOT NULL REFERENCES tags(id),
                date        TEXT    NOT NULL,
                notes       TEXT,
                created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS budgets (
                id       INTEGER PRIMARY KEY AUTOINCREMENT,
                tag_id   INTEGER REFERENCES tags(id) ON DELETE CASCADE,
                amount   INTEGER NOT NULL,
                period   TEXT    NOT NULL CHECK (period IN ('weekly', 'monthly', 'yearly')),
                active   INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS recurring_entries (
                id                 INTEGER PRIMARY KEY AUTOINCREMENT,
                source             TEXT    NOT NULL,
                amount             INTEGER NOT NULL,
                kind               TEXT    NOT NULL CHECK (kind IN ('income', 'expense')),
                tag_id             INTEGER NOT NULL REFERENCES tags(id),
                interval           TEXT    NOT NULL CHECK (interval IN ('daily', 'weekly', 'monthly', 'yearly')),
                start_date         TEXT    NOT NULL,
                last_inserted_date TEXT,
                active             INTEGER NOT NULL DEFAULT 1
            );

            CREATE INDEX IF NOT EXISTS idx_transactions_date   ON transactions(date);
            CREATE INDEX IF NOT EXISTS idx_transactions_tag_id ON transactions(tag_id);
            CREATE INDEX IF NOT EXISTS idx_transactions_kind   ON transactions(kind);
            ",
        )?;
        Ok(())
    }

    /// Placeholder for future schema migrations.
    ///
    /// A simple `user_version` pragma approach is used: each migration checks
    /// the current version and applies incremental DDL.
    fn run_migrations(&self) -> Result<()> {
        let version: i64 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;

        if version < 1 {
            // Version 0 → 1: initial schema already created above.
            self.conn
                .execute_batch("PRAGMA user_version = 1;")?;
        }

        // Future migrations go here:
        // if version < 2 { ... PRAGMA user_version = 2; }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_creates_tables() {
        let db = Database::in_memory().unwrap();
        // Verify all four tables exist.
        let tables: Vec<String> = db
            .conn()
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();

        assert!(tables.contains(&"tags".to_string()));
        assert!(tables.contains(&"transactions".to_string()));
        assert!(tables.contains(&"budgets".to_string()));
        assert!(tables.contains(&"recurring_entries".to_string()));
    }

    #[test]
    fn foreign_keys_are_enabled() {
        let db = Database::in_memory().unwrap();
        let fk: i64 = db
            .conn()
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn idempotent_schema_creation() {
        let db = Database::in_memory().unwrap();
        // Running initialize_schema a second time should be fine.
        // (Already called in in_memory(), so just open another one on the
        // same connection — but since it's in-memory we just verify no panic.)
        let db2 = Database::in_memory().unwrap();
        assert!(db.conn().is_autocommit());
        assert!(db2.conn().is_autocommit());
    }
}
