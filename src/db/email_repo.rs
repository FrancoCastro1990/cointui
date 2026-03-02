use crate::db::connection::Database;
use crate::error::Result;

/// Repository for the `processed_emails` deduplication table.
pub struct EmailRepo<'a> {
    db: &'a Database,
}

impl<'a> EmailRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Check whether an email with the given Message-ID has already been processed.
    pub fn is_processed(&self, message_id: &str) -> Result<bool> {
        let count: i64 = self.db.conn().query_row(
            "SELECT COUNT(*) FROM processed_emails WHERE message_id = ?1",
            rusqlite::params![message_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Record a processed email and return its generated id.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        message_id: &str,
        bank: &str,
        subject: Option<&str>,
        email_date: Option<&str>,
        status: &str,
        transaction_id: Option<i64>,
        account_email: &str,
    ) -> Result<i64> {
        self.db.conn().execute(
            "INSERT INTO processed_emails (message_id, bank, subject, email_date, status, transaction_id, account_email)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![message_id, bank, subject, email_date, status, transaction_id, account_email],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    /// Return counts of processed emails by status: `(imported, skipped_transfer, skipped_error)`.
    pub fn get_counts(&self) -> Result<(i64, i64, i64)> {
        let imported: i64 = self.db.conn().query_row(
            "SELECT COUNT(*) FROM processed_emails WHERE status = 'imported'",
            [],
            |row| row.get(0),
        )?;
        let skipped_transfer: i64 = self.db.conn().query_row(
            "SELECT COUNT(*) FROM processed_emails WHERE status = 'skipped_transfer'",
            [],
            |row| row.get(0),
        )?;
        let skipped_error: i64 = self.db.conn().query_row(
            "SELECT COUNT(*) FROM processed_emails WHERE status = 'skipped_error'",
            [],
            |row| row.get(0),
        )?;
        Ok((imported, skipped_transfer, skipped_error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Database {
        Database::in_memory().unwrap()
    }

    #[test]
    fn is_processed_false_initially() {
        let db = setup();
        let repo = EmailRepo::new(&db);
        assert!(!repo.is_processed("msg-001@gmail.com").unwrap());
    }

    #[test]
    fn record_and_is_processed() {
        let db = setup();
        let repo = EmailRepo::new(&db);

        let id = repo
            .record("msg-001@gmail.com", "santander", Some("Compra"), Some("2026-01-15"), "imported", None, "")
            .unwrap();
        assert!(id > 0);
        assert!(repo.is_processed("msg-001@gmail.com").unwrap());
        assert!(!repo.is_processed("msg-002@gmail.com").unwrap());
    }

    #[test]
    fn get_counts() {
        let db = setup();
        let repo = EmailRepo::new(&db);

        repo.record("msg-001", "santander", None, None, "imported", None, "").unwrap();
        repo.record("msg-002", "santander", None, None, "imported", None, "").unwrap();
        repo.record("msg-003", "cmr", None, None, "skipped_transfer", None, "").unwrap();
        repo.record("msg-004", "scotiabank", None, None, "skipped_error", None, "").unwrap();

        let (imported, transfer, error) = repo.get_counts().unwrap();
        assert_eq!(imported, 2);
        assert_eq!(transfer, 1);
        assert_eq!(error, 1);
    }

    #[test]
    fn duplicate_message_id_rejected() {
        let db = setup();
        let repo = EmailRepo::new(&db);

        repo.record("msg-dup", "santander", None, None, "imported", None, "").unwrap();
        let result = repo.record("msg-dup", "santander", None, None, "imported", None, "");
        assert!(result.is_err());
    }
}
