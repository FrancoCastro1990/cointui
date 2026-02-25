use crate::db::connection::Database;
use crate::domain::models::{Budget, BudgetPeriod};
use crate::error::{AppError, Result};

/// Repository for [`Budget`] CRUD operations.
pub struct BudgetRepo<'a> {
    db: &'a Database,
}

impl<'a> BudgetRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Insert a new budget and return its generated id.
    pub fn create(&self, budget: &Budget) -> Result<i64> {
        self.db.conn().execute(
            "INSERT INTO budgets (tag_id, amount, period, active) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                budget.tag_id,
                budget.amount,
                budget.period.to_string(),
                budget.active as i32,
            ],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    /// Fetch a single budget by id, or return `NotFound`.
    pub fn get_by_id(&self, id: i64) -> Result<Budget> {
        self.db
            .conn()
            .query_row(
                "SELECT id, tag_id, amount, period, active FROM budgets WHERE id = ?1",
                rusqlite::params![id],
                row_to_budget,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::NotFound(format!("Budget with id {id}"))
                }
                other => AppError::Database(other),
            })
    }

    /// Return every budget ordered by id.
    pub fn get_all(&self) -> Result<Vec<Budget>> {
        let mut stmt = self
            .db
            .conn()
            .prepare("SELECT id, tag_id, amount, period, active FROM budgets ORDER BY id")?;
        let budgets = stmt
            .query_map([], row_to_budget)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(budgets)
    }

    /// Return only active budgets.
    pub fn get_active(&self) -> Result<Vec<Budget>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, tag_id, amount, period, active FROM budgets WHERE active = 1 ORDER BY id",
        )?;
        let budgets = stmt
            .query_map([], row_to_budget)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(budgets)
    }

    /// Update an existing budget. The budget's `id` must be `Some`.
    pub fn update(&self, budget: &Budget) -> Result<()> {
        let id = budget.id.ok_or_else(|| {
            AppError::Validation("Cannot update a budget without an id.".into())
        })?;
        let affected = self.db.conn().execute(
            "UPDATE budgets SET tag_id = ?1, amount = ?2, period = ?3, active = ?4 WHERE id = ?5",
            rusqlite::params![
                budget.tag_id,
                budget.amount,
                budget.period.to_string(),
                budget.active as i32,
                id,
            ],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(format!("Budget with id {id}")));
        }
        Ok(())
    }

    /// Delete a budget by id.
    pub fn delete(&self, id: i64) -> Result<()> {
        let affected = self.db.conn().execute(
            "DELETE FROM budgets WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(format!("Budget with id {id}")));
        }
        Ok(())
    }

    /// Calculate how much has been spent in the current period for the given
    /// budget.
    ///
    /// For a tag-specific budget this sums expenses for that tag; for a global
    /// budget (`tag_id = None`) it sums all expenses.  The period start is
    /// computed from the current date.
    pub fn get_spent_for_budget(&self, budget: &Budget) -> Result<i64> {
        let period_start_expr = match budget.period {
            BudgetPeriod::Weekly => "date('now', 'weekday 0', '-6 days')",
            BudgetPeriod::Monthly => "date('now', 'start of month')",
            BudgetPeriod::Yearly => "date('now', 'start of year')",
        };

        let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match budget.tag_id {
            Some(tag_id) => (
                format!(
                    "SELECT COALESCE(SUM(amount), 0) FROM transactions
                     WHERE kind = 'expense' AND tag_id = ?1 AND date >= {period_start_expr}"
                ),
                vec![Box::new(tag_id)],
            ),
            None => (
                format!(
                    "SELECT COALESCE(SUM(amount), 0) FROM transactions
                     WHERE kind = 'expense' AND date >= {period_start_expr}"
                ),
                vec![],
            ),
        };

        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

        let spent: i64 = self
            .db
            .conn()
            .query_row(&sql, param_refs.as_slice(), |row| row.get(0))?;

        Ok(spent)
    }
}

/// Map a row from the `budgets` table to a [`Budget`].
fn row_to_budget(row: &rusqlite::Row<'_>) -> rusqlite::Result<Budget> {
    let period_str: String = row.get(3)?;
    let period: BudgetPeriod = period_str
        .parse()
        .map_err(|e: AppError| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e)))?;

    let active_int: i32 = row.get(4)?;

    Ok(Budget {
        id: row.get(0)?,
        tag_id: row.get(1)?,
        amount: row.get(2)?,
        period,
        active: active_int != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tag_repo::TagRepo;
    use crate::db::transaction_repo::TransactionRepo;
    use crate::domain::models::{Tag, Transaction, TransactionKind};

    fn setup() -> Database {
        let db = Database::in_memory().unwrap();
        let tag_repo = TagRepo::new(&db);
        tag_repo
            .create(&Tag {
                id: None,
                name: "Comida".into(),
                parent_id: None,
                icon: None,
            })
            .unwrap();
        db
    }

    fn sample_budget() -> Budget {
        Budget {
            id: None,
            tag_id: Some(1),
            amount: 50_000, // $500.00
            period: BudgetPeriod::Monthly,
            active: true,
        }
    }

    #[test]
    fn create_and_get() {
        let db = setup();
        let repo = BudgetRepo::new(&db);

        let id = repo.create(&sample_budget()).unwrap();
        let fetched = repo.get_by_id(id).unwrap();
        assert_eq!(fetched.amount, 50_000);
        assert_eq!(fetched.period, BudgetPeriod::Monthly);
        assert!(fetched.active);
    }

    #[test]
    fn get_active() {
        let db = setup();
        let repo = BudgetRepo::new(&db);

        let mut b1 = sample_budget();
        b1.active = true;
        let mut b2 = sample_budget();
        b2.active = false;
        b2.tag_id = None;

        repo.create(&b1).unwrap();
        repo.create(&b2).unwrap();

        let active = repo.get_active().unwrap();
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn update_and_delete() {
        let db = setup();
        let repo = BudgetRepo::new(&db);

        let id = repo.create(&sample_budget()).unwrap();
        let mut budget = repo.get_by_id(id).unwrap();
        budget.amount = 100_000;
        budget.active = false;
        repo.update(&budget).unwrap();

        let updated = repo.get_by_id(id).unwrap();
        assert_eq!(updated.amount, 100_000);
        assert!(!updated.active);

        repo.delete(id).unwrap();
        assert!(repo.get_by_id(id).is_err());
    }

    #[test]
    fn get_spent_for_budget() {
        let db = setup();
        let budget_repo = BudgetRepo::new(&db);
        let tx_repo = TransactionRepo::new(&db);

        let budget_id = budget_repo.create(&sample_budget()).unwrap();
        let budget = budget_repo.get_by_id(budget_id).unwrap();

        // Insert an expense dated today.
        let today = chrono::Local::now().date_naive();
        tx_repo
            .create(&Transaction {
                id: None,
                source: "Mercadona".into(),
                amount: 3_500,
                kind: TransactionKind::Expense,
                tag_id: 1,
                date: today,
                notes: None,
                created_at: None,
                updated_at: None,
            })
            .unwrap();

        let spent = budget_repo.get_spent_for_budget(&budget).unwrap();
        assert_eq!(spent, 3_500);
    }
}
