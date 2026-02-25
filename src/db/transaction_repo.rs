use chrono::NaiveDate;

use crate::db::connection::Database;
use crate::domain::models::{Transaction, TransactionKind};
use crate::error::{AppError, Result};

/// Optional filters for querying transactions.
#[derive(Debug, Default, Clone)]
pub struct TransactionFilter {
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    pub kind: Option<TransactionKind>,
    pub tag_id: Option<i64>,
    /// Case-insensitive substring match against `source` and `notes`.
    pub search: Option<String>,
    /// Minimum amount in centavos (inclusive).
    pub min_amount: Option<i64>,
    /// Maximum amount in centavos (inclusive).
    pub max_amount: Option<i64>,
}

/// Repository for [`Transaction`] CRUD operations.
pub struct TransactionRepo<'a> {
    db: &'a Database,
}

impl<'a> TransactionRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Insert a new transaction and return its generated id.
    pub fn create(&self, tx: &Transaction) -> Result<i64> {
        self.db.conn().execute(
            "INSERT INTO transactions (source, amount, kind, tag_id, date, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                tx.source,
                tx.amount,
                tx.kind.to_string(),
                tx.tag_id,
                tx.date.to_string(),
                tx.notes,
            ],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    /// Fetch a single transaction by id, or return `NotFound`.
    pub fn get_by_id(&self, id: i64) -> Result<Transaction> {
        self.db
            .conn()
            .query_row(
                "SELECT id, source, amount, kind, tag_id, date, notes, created_at, updated_at
                 FROM transactions WHERE id = ?1",
                rusqlite::params![id],
                row_to_transaction,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::NotFound(format!("Transaction with id {id}"))
                }
                other => AppError::Database(other),
            })
    }

    /// Return every transaction ordered by date descending.
    pub fn get_all(&self) -> Result<Vec<Transaction>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, source, amount, kind, tag_id, date, notes, created_at, updated_at
             FROM transactions ORDER BY date DESC, id DESC",
        )?;
        let txs = stmt
            .query_map([], row_to_transaction)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(txs)
    }

    /// Return transactions matching the given filters.
    ///
    /// The WHERE clause is built dynamically; only non-`None` filters are
    /// applied.
    pub fn get_filtered(&self, filter: &TransactionFilter) -> Result<Vec<Transaction>> {
        let mut sql = String::from(
            "SELECT id, source, amount, kind, tag_id, date, notes, created_at, updated_at
             FROM transactions WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1u32;

        if let Some(ref date_from) = filter.date_from {
            sql.push_str(&format!(" AND date >= ?{idx}"));
            params.push(Box::new(date_from.to_string()));
            idx += 1;
        }
        if let Some(ref date_to) = filter.date_to {
            sql.push_str(&format!(" AND date <= ?{idx}"));
            params.push(Box::new(date_to.to_string()));
            idx += 1;
        }
        if let Some(kind) = filter.kind {
            sql.push_str(&format!(" AND kind = ?{idx}"));
            params.push(Box::new(kind.to_string()));
            idx += 1;
        }
        if let Some(tag_id) = filter.tag_id {
            sql.push_str(&format!(" AND tag_id = ?{idx}"));
            params.push(Box::new(tag_id));
            idx += 1;
        }
        if let Some(ref search) = filter.search {
            sql.push_str(&format!(
                " AND (source LIKE ?{idx} OR notes LIKE ?{idx})"
            ));
            params.push(Box::new(format!("%{search}%")));
            idx += 1;
        }
        if let Some(min) = filter.min_amount {
            sql.push_str(&format!(" AND amount >= ?{idx}"));
            params.push(Box::new(min));
            idx += 1;
        }
        if let Some(max) = filter.max_amount {
            sql.push_str(&format!(" AND amount <= ?{idx}"));
            params.push(Box::new(max));
            // idx += 1; // uncomment when adding more filters after this one
        }

        sql.push_str(" ORDER BY date DESC, id DESC");

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.db.conn().prepare(&sql)?;
        let txs = stmt
            .query_map(param_refs.as_slice(), row_to_transaction)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(txs)
    }

    /// Update an existing transaction. The transaction's `id` must be `Some`.
    pub fn update(&self, tx: &Transaction) -> Result<()> {
        let id = tx.id.ok_or_else(|| {
            AppError::Validation("Cannot update a transaction without an id.".into())
        })?;
        let affected = self.db.conn().execute(
            "UPDATE transactions
             SET source = ?1, amount = ?2, kind = ?3, tag_id = ?4,
                 date = ?5, notes = ?6, updated_at = datetime('now')
             WHERE id = ?7",
            rusqlite::params![
                tx.source,
                tx.amount,
                tx.kind.to_string(),
                tx.tag_id,
                tx.date.to_string(),
                tx.notes,
                id,
            ],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(format!("Transaction with id {id}")));
        }
        Ok(())
    }

    /// Delete a transaction by id.
    pub fn delete(&self, id: i64) -> Result<()> {
        let affected = self.db.conn().execute(
            "DELETE FROM transactions WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(format!("Transaction with id {id}")));
        }
        Ok(())
    }

    /// Return `(total_income, total_expense)` across all transactions, both in
    /// centavos.
    pub fn get_totals(&self) -> Result<(i64, i64)> {
        let income: i64 = self.db.conn().query_row(
            "SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE kind = 'income'",
            [],
            |row| row.get(0),
        )?;
        let expense: i64 = self.db.conn().query_row(
            "SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE kind = 'expense'",
            [],
            |row| row.get(0),
        )?;
        Ok((income, expense))
    }

    /// Return monthly totals for the last `months` months.
    ///
    /// Each element is `(YYYY-MM, total_income, total_expense)`.
    pub fn get_monthly_totals(&self, months: u32) -> Result<Vec<(String, i64, i64)>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT
                 strftime('%Y-%m', date) AS month,
                 COALESCE(SUM(CASE WHEN kind = 'income'  THEN amount ELSE 0 END), 0),
                 COALESCE(SUM(CASE WHEN kind = 'expense' THEN amount ELSE 0 END), 0)
             FROM transactions
             WHERE date >= date('now', '-' || ?1 || ' months')
             GROUP BY month
             ORDER BY month",
        )?;

        let rows = stmt
            .query_map(rusqlite::params![months], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    /// Return all transactions for a specific tag.
    pub fn get_by_tag(&self, tag_id: i64) -> Result<Vec<Transaction>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, source, amount, kind, tag_id, date, notes, created_at, updated_at
             FROM transactions WHERE tag_id = ?1 ORDER BY date DESC, id DESC",
        )?;
        let txs = stmt
            .query_map(rusqlite::params![tag_id], row_to_transaction)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(txs)
    }
}

/// Map a row from the `transactions` table to a [`Transaction`].
fn row_to_transaction(row: &rusqlite::Row<'_>) -> rusqlite::Result<Transaction> {
    let kind_str: String = row.get(3)?;
    let kind: TransactionKind = kind_str
        .parse()
        .map_err(|e: AppError| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e)))?;

    let date_str: String = row.get(5)?;
    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
    })?;

    let created_at: Option<String> = row.get(7)?;
    let updated_at: Option<String> = row.get(8)?;

    Ok(Transaction {
        id: row.get(0)?,
        source: row.get(1)?,
        amount: row.get(2)?,
        kind,
        tag_id: row.get(4)?,
        date,
        notes: row.get(6)?,
        created_at: created_at.and_then(|s| {
            chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()
        }),
        updated_at: updated_at.and_then(|s| {
            chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tag_repo::TagRepo;
    use crate::domain::models::Tag;

    fn setup() -> Database {
        let db = Database::in_memory().unwrap();
        let tag_repo = TagRepo::new(&db);
        tag_repo
            .create(&Tag {
                id: None,
                name: "Test".into(),
                parent_id: None,
                icon: None,
            })
            .unwrap();
        db
    }

    fn sample_tx(kind: TransactionKind) -> Transaction {
        Transaction {
            id: None,
            source: "Supermercado".into(),
            amount: 5000,
            kind,
            tag_id: 1,
            date: NaiveDate::from_ymd_opt(2026, 2, 15).unwrap(),
            notes: Some("weekly groceries".into()),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn create_and_get() {
        let db = setup();
        let repo = TransactionRepo::new(&db);

        let id = repo.create(&sample_tx(TransactionKind::Expense)).unwrap();
        let fetched = repo.get_by_id(id).unwrap();
        assert_eq!(fetched.source, "Supermercado");
        assert_eq!(fetched.amount, 5000);
        assert_eq!(fetched.kind, TransactionKind::Expense);
    }

    #[test]
    fn update_and_delete() {
        let db = setup();
        let repo = TransactionRepo::new(&db);

        let id = repo.create(&sample_tx(TransactionKind::Income)).unwrap();
        let mut tx = repo.get_by_id(id).unwrap();
        tx.source = "Nómina".into();
        tx.amount = 200_000;
        repo.update(&tx).unwrap();

        let updated = repo.get_by_id(id).unwrap();
        assert_eq!(updated.source, "Nómina");
        assert_eq!(updated.amount, 200_000);

        repo.delete(id).unwrap();
        assert!(repo.get_by_id(id).is_err());
    }

    #[test]
    fn get_totals() {
        let db = setup();
        let repo = TransactionRepo::new(&db);

        repo.create(&sample_tx(TransactionKind::Income)).unwrap();
        repo.create(&sample_tx(TransactionKind::Expense)).unwrap();
        repo.create(&sample_tx(TransactionKind::Expense)).unwrap();

        let (income, expense) = repo.get_totals().unwrap();
        assert_eq!(income, 5000);
        assert_eq!(expense, 10000);
    }

    #[test]
    fn get_filtered_by_kind() {
        let db = setup();
        let repo = TransactionRepo::new(&db);

        repo.create(&sample_tx(TransactionKind::Income)).unwrap();
        repo.create(&sample_tx(TransactionKind::Expense)).unwrap();

        let filter = TransactionFilter {
            kind: Some(TransactionKind::Income),
            ..Default::default()
        };
        let results = repo.get_filtered(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, TransactionKind::Income);
    }

    #[test]
    fn get_filtered_by_amount_range() {
        let db = setup();
        let repo = TransactionRepo::new(&db);

        let mut small = sample_tx(TransactionKind::Expense);
        small.amount = 100;
        let mut big = sample_tx(TransactionKind::Expense);
        big.amount = 50000;

        repo.create(&small).unwrap();
        repo.create(&big).unwrap();

        let filter = TransactionFilter {
            min_amount: Some(1000),
            ..Default::default()
        };
        let results = repo.get_filtered(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].amount, 50000);
    }

    #[test]
    fn get_filtered_by_search() {
        let db = setup();
        let repo = TransactionRepo::new(&db);

        let mut tx1 = sample_tx(TransactionKind::Expense);
        tx1.source = "Uber ride".into();
        let mut tx2 = sample_tx(TransactionKind::Expense);
        tx2.source = "Supermercado".into();

        repo.create(&tx1).unwrap();
        repo.create(&tx2).unwrap();

        let filter = TransactionFilter {
            search: Some("uber".into()),
            ..Default::default()
        };
        let results = repo.get_filtered(&filter).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "Uber ride");
    }

    #[test]
    fn get_by_tag() {
        let db = setup();
        let tag_repo = TagRepo::new(&db);
        tag_repo
            .create(&Tag {
                id: None,
                name: "Other".into(),
                parent_id: None,
                icon: None,
            })
            .unwrap();

        let repo = TransactionRepo::new(&db);
        repo.create(&sample_tx(TransactionKind::Expense)).unwrap(); // tag_id = 1

        let mut tx2 = sample_tx(TransactionKind::Expense);
        tx2.tag_id = 2;
        repo.create(&tx2).unwrap();

        let results = repo.get_by_tag(1).unwrap();
        assert_eq!(results.len(), 1);
    }
}
