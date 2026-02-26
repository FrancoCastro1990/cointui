use chrono::NaiveDate;

use crate::db::connection::Database;
use crate::domain::models::{RecurringEntry, RecurringInterval, TransactionKind};
use crate::error::{AppError, Result};

/// Repository for [`RecurringEntry`] CRUD operations.
pub struct RecurringRepo<'a> {
    db: &'a Database,
}

impl<'a> RecurringRepo<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Insert a new recurring entry and return its generated id.
    pub fn create(&self, entry: &RecurringEntry) -> Result<i64> {
        self.db.conn().execute(
            "INSERT INTO recurring_entries
                (source, amount, kind, tag_id, interval, start_date, day_of_month, month, last_inserted_date, active)
             VALUES (?1, ?2, ?3, ?4, ?5, '2000-01-01', ?6, ?7, ?8, ?9)",
            rusqlite::params![
                entry.source,
                entry.amount,
                entry.kind.to_string(),
                entry.tag_id,
                entry.interval.to_string(),
                entry.day_of_month.map(|d| d as i64),
                entry.month.map(|m| m as i64),
                entry.last_inserted_date.map(|d| d.to_string()),
                entry.active as i32,
            ],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    /// Fetch a single recurring entry by id, or return `NotFound`.
    pub fn get_by_id(&self, id: i64) -> Result<RecurringEntry> {
        self.db
            .conn()
            .query_row(
                "SELECT id, source, amount, kind, tag_id, interval,
                        day_of_month, month, last_inserted_date, active
                 FROM recurring_entries WHERE id = ?1",
                rusqlite::params![id],
                row_to_recurring,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::NotFound(format!("Recurring entry with id {id}"))
                }
                other => AppError::Database(other),
            })
    }

    /// Return every recurring entry ordered by id.
    pub fn get_all(&self) -> Result<Vec<RecurringEntry>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, source, amount, kind, tag_id, interval,
                    day_of_month, month, last_inserted_date, active
             FROM recurring_entries ORDER BY id",
        )?;
        let entries = stmt
            .query_map([], row_to_recurring)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    /// Return only active recurring entries.
    pub fn get_active(&self) -> Result<Vec<RecurringEntry>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, source, amount, kind, tag_id, interval,
                    day_of_month, month, last_inserted_date, active
             FROM recurring_entries WHERE active = 1 ORDER BY id",
        )?;
        let entries = stmt
            .query_map([], row_to_recurring)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    /// Update an existing recurring entry. The entry's `id` must be `Some`.
    pub fn update(&self, entry: &RecurringEntry) -> Result<()> {
        let id = entry.id.ok_or_else(|| {
            AppError::Validation("Cannot update a recurring entry without an id.".into())
        })?;
        let affected = self.db.conn().execute(
            "UPDATE recurring_entries
             SET source = ?1, amount = ?2, kind = ?3, tag_id = ?4,
                 interval = ?5, day_of_month = ?6, month = ?7, last_inserted_date = ?8, active = ?9
             WHERE id = ?10",
            rusqlite::params![
                entry.source,
                entry.amount,
                entry.kind.to_string(),
                entry.tag_id,
                entry.interval.to_string(),
                entry.day_of_month.map(|d| d as i64),
                entry.month.map(|m| m as i64),
                entry.last_inserted_date.map(|d| d.to_string()),
                entry.active as i32,
                id,
            ],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(format!("Recurring entry with id {id}")));
        }
        Ok(())
    }

    /// Delete a recurring entry by id.
    pub fn delete(&self, id: i64) -> Result<()> {
        let affected = self.db.conn().execute(
            "DELETE FROM recurring_entries WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(format!("Recurring entry with id {id}")));
        }
        Ok(())
    }

    /// Record that the recurring entry was last materialised on `date`.
    pub fn update_last_inserted(&self, id: i64, date: NaiveDate) -> Result<()> {
        let affected = self.db.conn().execute(
            "UPDATE recurring_entries SET last_inserted_date = ?1 WHERE id = ?2",
            rusqlite::params![date.to_string(), id],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(format!("Recurring entry with id {id}")));
        }
        Ok(())
    }

    /// Return all recurring entries for a specific tag.
    pub fn get_by_tag(&self, tag_id: i64) -> Result<Vec<RecurringEntry>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, source, amount, kind, tag_id, interval,
                    day_of_month, month, last_inserted_date, active
             FROM recurring_entries WHERE tag_id = ?1 ORDER BY id",
        )?;
        let entries = stmt
            .query_map(rusqlite::params![tag_id], row_to_recurring)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    /// Reassign all recurring entries from one tag to another.
    /// Returns the number of rows updated.
    pub fn reassign_tag(&self, old_tag_id: i64, new_tag_id: i64) -> Result<usize> {
        let affected = self.db.conn().execute(
            "UPDATE recurring_entries SET tag_id = ?1 WHERE tag_id = ?2",
            rusqlite::params![new_tag_id, old_tag_id],
        )?;
        Ok(affected)
    }

    /// Toggle the `active` flag on a recurring entry.
    pub fn toggle_active(&self, id: i64) -> Result<()> {
        let affected = self.db.conn().execute(
            "UPDATE recurring_entries SET active = 1 - active WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(format!("Recurring entry with id {id}")));
        }
        Ok(())
    }
}

/// Map a row from the `recurring_entries` table to a [`RecurringEntry`].
///
/// Column order: id(0), source(1), amount(2), kind(3), tag_id(4), interval(5),
///               day_of_month(6), month(7), last_inserted_date(8), active(9)
fn row_to_recurring(row: &rusqlite::Row<'_>) -> rusqlite::Result<RecurringEntry> {
    let kind_str: String = row.get(3)?;
    let kind: TransactionKind = kind_str
        .parse()
        .map_err(|e: AppError| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e)))?;

    let interval_str: String = row.get(5)?;
    let interval: RecurringInterval = interval_str
        .parse()
        .map_err(|e: AppError| rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e)))?;

    let day_of_month: Option<i64> = row.get(6)?;
    let month: Option<i64> = row.get(7)?;

    let last_inserted_str: Option<String> = row.get(8)?;
    let last_inserted_date = last_inserted_str
        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

    let active_int: i32 = row.get(9)?;

    Ok(RecurringEntry {
        id: row.get(0)?,
        source: row.get(1)?,
        amount: row.get(2)?,
        kind,
        tag_id: row.get(4)?,
        interval,
        day_of_month: day_of_month.map(|d| d as u32),
        month: month.map(|m| m as u32),
        last_inserted_date,
        active: active_int != 0,
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
                name: "Servicios".into(),
                parent_id: None,
                icon: None,
            })
            .unwrap();
        db
    }

    fn sample_entry() -> RecurringEntry {
        RecurringEntry {
            id: None,
            source: "Netflix".into(),
            amount: 1_500,
            kind: TransactionKind::Expense,
            tag_id: 1,
            interval: RecurringInterval::Monthly,
            day_of_month: Some(1),
            month: None,
            last_inserted_date: None,
            active: true,
        }
    }

    #[test]
    fn create_and_get() {
        let db = setup();
        let repo = RecurringRepo::new(&db);

        let id = repo.create(&sample_entry()).unwrap();
        let fetched = repo.get_by_id(id).unwrap();
        assert_eq!(fetched.source, "Netflix");
        assert_eq!(fetched.amount, 1_500);
        assert_eq!(fetched.interval, RecurringInterval::Monthly);
        assert!(fetched.active);
        assert!(fetched.last_inserted_date.is_none());
    }

    #[test]
    fn get_active() {
        let db = setup();
        let repo = RecurringRepo::new(&db);

        let mut e1 = sample_entry();
        e1.active = true;
        let mut e2 = sample_entry();
        e2.source = "Spotify".into();
        e2.active = false;

        repo.create(&e1).unwrap();
        repo.create(&e2).unwrap();

        let active = repo.get_active().unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].source, "Netflix");
    }

    #[test]
    fn update_and_delete() {
        let db = setup();
        let repo = RecurringRepo::new(&db);

        let id = repo.create(&sample_entry()).unwrap();
        let mut entry = repo.get_by_id(id).unwrap();
        entry.amount = 2_000;
        entry.interval = RecurringInterval::Yearly;
        repo.update(&entry).unwrap();

        let updated = repo.get_by_id(id).unwrap();
        assert_eq!(updated.amount, 2_000);
        assert_eq!(updated.interval, RecurringInterval::Yearly);

        repo.delete(id).unwrap();
        assert!(repo.get_by_id(id).is_err());
    }

    #[test]
    fn update_last_inserted() {
        let db = setup();
        let repo = RecurringRepo::new(&db);

        let id = repo.create(&sample_entry()).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
        repo.update_last_inserted(id, date).unwrap();

        let entry = repo.get_by_id(id).unwrap();
        assert_eq!(entry.last_inserted_date, Some(date));
    }

    #[test]
    fn toggle_active() {
        let db = setup();
        let repo = RecurringRepo::new(&db);

        let id = repo.create(&sample_entry()).unwrap();
        assert!(repo.get_by_id(id).unwrap().active);

        repo.toggle_active(id).unwrap();
        assert!(!repo.get_by_id(id).unwrap().active);

        repo.toggle_active(id).unwrap();
        assert!(repo.get_by_id(id).unwrap().active);
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

        let repo = RecurringRepo::new(&db);
        repo.create(&sample_entry()).unwrap(); // tag_id = 1

        let mut e2 = sample_entry();
        e2.tag_id = 2;
        repo.create(&e2).unwrap();

        let results = repo.get_by_tag(1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "Netflix");
    }

    #[test]
    fn reassign_tag() {
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

        let repo = RecurringRepo::new(&db);
        repo.create(&sample_entry()).unwrap(); // tag_id = 1
        repo.create(&sample_entry()).unwrap(); // tag_id = 1

        let count = repo.reassign_tag(1, 2).unwrap();
        assert_eq!(count, 2);

        let tag1 = repo.get_by_tag(1).unwrap();
        assert_eq!(tag1.len(), 0);
        let tag2 = repo.get_by_tag(2).unwrap();
        assert_eq!(tag2.len(), 2);
    }

    #[test]
    fn not_found_errors() {
        let db = setup();
        let repo = RecurringRepo::new(&db);

        assert!(repo.get_by_id(999).is_err());
        assert!(repo.delete(999).is_err());
        assert!(repo.toggle_active(999).is_err());
        assert!(repo
            .update_last_inserted(999, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
            .is_err());
    }
}
