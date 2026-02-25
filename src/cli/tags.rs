use crate::db::connection::Database;
use crate::db::recurring_repo::RecurringRepo;
use crate::db::tag_repo::TagRepo;
use crate::db::transaction_repo::TransactionRepo;
use crate::domain::models::Tag;
use crate::error::{AppError, Result};

/// List all tags.
pub fn run_list(db: &Database) -> Result<()> {
    let repo = TagRepo::new(db);
    let tags = repo.get_all()?;

    if tags.is_empty() {
        println!("No tags found.");
        return Ok(());
    }

    println!("{:<6} Name", "ID");
    println!("{}", "-".repeat(30));
    for tag in &tags {
        println!("{:<6} {}", tag.id.unwrap_or(0), tag.name);
    }
    println!("\n{} tag(s) total.", tags.len());
    Ok(())
}

/// Create a new tag.
pub fn run_add(name: &str, db: &Database) -> Result<()> {
    let repo = TagRepo::new(db);

    if let Some(_existing) = repo.find_by_name(name)? {
        return Err(AppError::Validation(format!(
            "Tag '{name}' already exists."
        )));
    }

    let tag = Tag {
        id: None,
        name: name.to_string(),
        parent_id: None,
        icon: None,
    };
    let id = repo.create(&tag)?;
    println!("Tag created: [{id}] {name}");
    Ok(())
}

/// Rename a tag. `spec` must be "OldName:NewName".
pub fn run_rename(spec: &str, db: &Database) -> Result<()> {
    let parts: Vec<&str> = spec.splitn(2, ':').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(AppError::Validation(
            "Expected format: \"OldName:NewName\"".into(),
        ));
    }

    let old_name = parts[0].trim();
    let new_name = parts[1].trim();

    let repo = TagRepo::new(db);
    let tag = repo.find_by_name(old_name)?.ok_or_else(|| {
        AppError::Validation(format!("Tag '{old_name}' not found."))
    })?;

    if repo.find_by_name(new_name)?.is_some() {
        return Err(AppError::Validation(format!(
            "Tag '{new_name}' already exists."
        )));
    }

    let mut updated = tag;
    updated.name = new_name.to_string();
    repo.update(&updated)?;
    println!("Tag renamed: '{old_name}' -> '{new_name}'");
    Ok(())
}

/// Delete a tag. Blocks if any transactions or recurring entries reference it.
pub fn run_delete(name: &str, db: &Database) -> Result<()> {
    let tag_repo = TagRepo::new(db);
    let tag = tag_repo.find_by_name(name)?.ok_or_else(|| {
        AppError::Validation(format!("Tag '{name}' not found."))
    })?;
    let tag_id = tag.id.unwrap();

    let tx_repo = TransactionRepo::new(db);
    let txs = tx_repo.get_by_tag(tag_id)?;

    let rec_repo = RecurringRepo::new(db);
    let recs = rec_repo.get_by_tag(tag_id)?;

    if !txs.is_empty() || !recs.is_empty() {
        println!("Cannot delete tag '{name}': it is still in use.");
        if !txs.is_empty() {
            println!("  {} transaction(s) reference this tag.", txs.len());
        }
        if !recs.is_empty() {
            println!("  {} recurring entry(ies) reference this tag.", recs.len());
        }
        println!("Reassign or delete those records first.");
        return Err(AppError::Validation(format!(
            "Tag '{name}' is still in use by {} transaction(s) and {} recurring entry(ies).",
            txs.len(),
            recs.len()
        )));
    }

    tag_repo.delete(tag_id)?;
    println!("Tag deleted: '{name}'");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tag_repo::TagRepo;
    use crate::db::transaction_repo::TransactionRepo;
    use crate::domain::models::{Transaction, TransactionKind};
    use chrono::NaiveDate;

    fn setup() -> Database {
        let db = Database::in_memory().unwrap();
        let tag_repo = TagRepo::new(&db);
        tag_repo
            .seed_defaults(&["Other".to_string(), "Salary".to_string()])
            .unwrap();
        db
    }

    #[test]
    fn add_and_list() {
        let db = setup();
        run_add("Food", &db).unwrap();

        let tags = TagRepo::new(&db).get_all().unwrap();
        assert!(tags.iter().any(|t| t.name == "Food"));

        // run_list should not error
        run_list(&db).unwrap();
    }

    #[test]
    fn add_duplicate_errors() {
        let db = setup();
        run_add("Food", &db).unwrap();
        let result = run_add("Food", &db);
        assert!(result.is_err());
    }

    #[test]
    fn rename_success() {
        let db = setup();
        run_add("OldTag", &db).unwrap();
        run_rename("OldTag:NewTag", &db).unwrap();

        let tags = TagRepo::new(&db).get_all().unwrap();
        assert!(tags.iter().any(|t| t.name == "NewTag"));
        assert!(!tags.iter().any(|t| t.name == "OldTag"));
    }

    #[test]
    fn rename_bad_format() {
        let db = setup();
        let result = run_rename("NoColonHere", &db);
        assert!(result.is_err());
    }

    #[test]
    fn rename_not_found() {
        let db = setup();
        let result = run_rename("Ghost:NewName", &db);
        assert!(result.is_err());
    }

    #[test]
    fn rename_to_existing_errors() {
        let db = setup();
        let result = run_rename("Other:Salary", &db);
        assert!(result.is_err());
    }

    #[test]
    fn delete_clean_tag() {
        let db = setup();
        run_add("Temp", &db).unwrap();
        run_delete("Temp", &db).unwrap();

        let tags = TagRepo::new(&db).get_all().unwrap();
        assert!(!tags.iter().any(|t| t.name == "Temp"));
    }

    #[test]
    fn delete_with_transactions_blocked() {
        let db = setup();
        let tag = TagRepo::new(&db).find_by_name("Other").unwrap().unwrap();
        let tag_id = tag.id.unwrap();

        TransactionRepo::new(&db)
            .create(&Transaction {
                id: None,
                source: "Test".into(),
                amount: 1000,
                kind: TransactionKind::Expense,
                tag_id,
                date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                notes: None,
                created_at: None,
                updated_at: None,
            })
            .unwrap();

        let result = run_delete("Other", &db);
        assert!(result.is_err());
    }

    #[test]
    fn delete_not_found() {
        let db = setup();
        let result = run_delete("Ghost", &db);
        assert!(result.is_err());
    }

    #[test]
    fn list_empty() {
        let db = Database::in_memory().unwrap();
        run_list(&db).unwrap();
    }
}
