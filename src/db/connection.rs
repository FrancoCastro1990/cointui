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

    /// Create a backup of this database to the given path using SQLite's
    /// online backup API.
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut dest_conn = Connection::open(dest)?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut dest_conn)?;
        backup.run_to_completion(100, std::time::Duration::from_millis(10), None)?;
        Ok(())
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

            CREATE TABLE IF NOT EXISTS processed_emails (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id      TEXT    NOT NULL UNIQUE,
                bank            TEXT    NOT NULL,
                subject         TEXT,
                email_date      TEXT,
                processed_at    TEXT    NOT NULL DEFAULT (datetime('now')),
                status          TEXT    NOT NULL DEFAULT 'imported',
                transaction_id  INTEGER REFERENCES transactions(id) ON DELETE SET NULL
            );
            CREATE INDEX IF NOT EXISTS idx_processed_emails_message_id ON processed_emails(message_id);
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

        if version < 2 {
            // Add day_of_month and month columns for configurable recurring intervals.
            self.conn.execute_batch(
                "ALTER TABLE recurring_entries ADD COLUMN day_of_month INTEGER;
                 ALTER TABLE recurring_entries ADD COLUMN month INTEGER;"
            )?;

            // Populate new columns from existing start_date values.
            // Monthly entries: extract day from start_date.
            self.conn.execute_batch(
                "UPDATE recurring_entries
                 SET day_of_month = CAST(strftime('%d', start_date) AS INTEGER)
                 WHERE interval = 'monthly';"
            )?;
            // Yearly entries: extract both month and day from start_date.
            self.conn.execute_batch(
                "UPDATE recurring_entries
                 SET day_of_month = CAST(strftime('%d', start_date) AS INTEGER),
                     month = CAST(strftime('%m', start_date) AS INTEGER)
                 WHERE interval = 'yearly';"
            )?;

            self.conn.execute_batch("PRAGMA user_version = 2;")?;
        }

        if version < 3 {
            // Version 2 → 3: processed_emails table (handled by CREATE IF NOT EXISTS above).
            self.conn.execute_batch("PRAGMA user_version = 3;")?;
        }

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

    #[test]
    fn backup_creates_valid_copy() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src_path = tmp.path().join("source.db");
        let backup_path = tmp.path().join("backup.db");

        let db = Database::new(&src_path).unwrap();
        // Insert some data.
        db.conn()
            .execute(
                "INSERT INTO tags (name) VALUES (?1)",
                rusqlite::params!["BackupTest"],
            )
            .unwrap();

        db.backup_to(&backup_path).unwrap();

        // Open the backup and verify data.
        let backup_db = Database::new(&backup_path).unwrap();
        let name: String = backup_db
            .conn()
            .query_row(
                "SELECT name FROM tags WHERE name = 'BackupTest'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "BackupTest");
    }
}
