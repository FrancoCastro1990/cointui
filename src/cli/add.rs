use chrono::{Local, NaiveDate};

use crate::config::AppConfig;
use crate::db::connection::Database;
use crate::db::tag_repo::TagRepo;
use crate::db::transaction_repo::TransactionRepo;
use crate::domain::models::{format_centavos, Transaction, TransactionKind};
use crate::error::{AppError, Result};

/// Optional arguments for `--add`.
pub struct AddArgs {
    pub amount: Option<f64>,
    pub kind: Option<String>,
    pub tag: Option<String>,
    pub date: Option<String>,
    pub notes: Option<String>,
}

/// Core logic: create a transaction given fully-resolved parameters.
/// Returns the new transaction id.
pub fn create_transaction(
    source: String,
    amount_centavos: i64,
    kind: TransactionKind,
    tag_id: i64,
    date: NaiveDate,
    notes: Option<String>,
    db: &Database,
) -> Result<i64> {
    let tx = Transaction {
        id: None,
        source,
        amount: amount_centavos,
        kind,
        tag_id,
        date,
        notes,
        created_at: None,
        updated_at: None,
    };
    TransactionRepo::new(db).create(&tx)
}

/// CLI entry point for `--add`.
pub fn run(
    source: String,
    args: AddArgs,
    db: &Database,
    config: &AppConfig,
) -> Result<()> {
    // 1. Validate and convert amount.
    let amount_f64 = args.amount.ok_or_else(|| {
        AppError::Validation("--amount is required when using --add.".into())
    })?;
    let amount_centavos = (amount_f64 * 100.0).round() as i64;

    // 2. Parse kind (default: expense).
    let kind = match args.kind {
        Some(s) => s.parse::<TransactionKind>()?,
        None => TransactionKind::Expense,
    };

    // 3. Resolve tag.
    let tag_repo = TagRepo::new(db);
    let resolved_tag = if let Some(ref name) = args.tag {
        tag_repo.find_by_name(name)?.ok_or_else(|| {
            AppError::Validation(format!("Tag '{name}' not found."))
        })?
    } else {
        // Try "Otros", then fall back to first available tag.
        if let Some(t) = tag_repo.find_by_name("Otros")? {
            t
        } else {
            let all = tag_repo.get_all()?;
            all.into_iter().next().ok_or_else(|| {
                AppError::Validation("No tags exist. Run the TUI first to seed defaults.".into())
            })?
        }
    };
    let tag_id = resolved_tag.id.unwrap();
    let tag_name = &resolved_tag.name;

    // 4. Parse date (default: today).
    let date = match args.date {
        Some(s) => NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(|_| {
            AppError::Validation(format!(
                "Invalid date '{s}'. Expected format: YYYY-MM-DD."
            ))
        })?,
        None => Local::now().date_naive(),
    };

    // 5. Create the transaction.
    create_transaction(source.clone(), amount_centavos, kind, tag_id, date, args.notes, db)?;

    // 6. Print confirmation.
    let sign = match kind {
        TransactionKind::Income => "+",
        TransactionKind::Expense => "-",
    };
    let amount_display = format_centavos(amount_centavos, &config.currency, &config.thousands_separator, &config.decimal_separator);
    println!(
        "Transaction added: {source} {sign}{amount_display} [{tag_name}] ({date})"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tag_repo::TagRepo;
    use crate::db::transaction_repo::TransactionRepo;
    use crate::domain::models::Tag;

    fn setup() -> Database {
        let db = Database::in_memory().unwrap();
        let tag_repo = TagRepo::new(&db);
        tag_repo
            .create(&Tag {
                id: None,
                name: "Otros".into(),
                parent_id: None,
                icon: None,
            })
            .unwrap();
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

    #[test]
    fn add_with_all_fields() {
        let db = setup();
        let config = AppConfig::default();

        run(
            "Supermercado".into(),
            AddArgs {
                amount: Some(50.00),
                kind: Some("expense".into()),
                tag: Some("Comida".into()),
                date: Some("2026-02-25".into()),
                notes: Some("weekly groceries".into()),
            },
            &db,
            &config,
        )
        .unwrap();

        let txs = TransactionRepo::new(&db).get_all().unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].source, "Supermercado");
        assert_eq!(txs[0].amount, 5000);
        assert_eq!(txs[0].kind, TransactionKind::Expense);
        assert_eq!(txs[0].tag_id, 2); // Comida
        assert_eq!(
            txs[0].date,
            NaiveDate::from_ymd_opt(2026, 2, 25).unwrap()
        );
        assert_eq!(txs[0].notes, Some("weekly groceries".into()));
    }

    #[test]
    fn add_with_defaults() {
        let db = setup();
        let config = AppConfig::default();

        run(
            "Coffee".into(),
            AddArgs {
                amount: Some(3.50),
                kind: None,
                tag: None,
                date: None,
                notes: None,
            },
            &db,
            &config,
        )
        .unwrap();

        let txs = TransactionRepo::new(&db).get_all().unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].kind, TransactionKind::Expense);
        assert_eq!(txs[0].amount, 350);
        assert_eq!(txs[0].tag_id, 1); // Otros (default)
        assert_eq!(txs[0].date, Local::now().date_naive());
    }

    #[test]
    fn missing_amount_errors() {
        let db = setup();
        let config = AppConfig::default();

        let result = run(
            "Test".into(),
            AddArgs {
                amount: None,
                kind: None,
                tag: None,
                date: None,
                notes: None,
            },
            &db,
            &config,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn invalid_tag_errors() {
        let db = setup();
        let config = AppConfig::default();

        let result = run(
            "Test".into(),
            AddArgs {
                amount: Some(10.0),
                kind: None,
                tag: Some("NonExistentTag".into()),
                date: None,
                notes: None,
            },
            &db,
            &config,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
