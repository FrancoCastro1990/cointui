use std::path::PathBuf;

use serde::Serialize;

use crate::db::connection::Database;
use crate::db::tag_repo::TagRepo;
use crate::db::transaction_repo::TransactionRepo;
use crate::domain::models::TransactionKind;
use crate::error::{AppError, Result};

#[derive(Serialize)]
struct ExportTransaction {
    date: String,
    source: String,
    amount: String,
    kind: String,
    tag_name: String,
    notes: String,
}

pub fn run(path: PathBuf, db: &Database, format: Option<String>) -> Result<()> {
    let fmt = detect_format(&path, format)?;

    let tx_repo = TransactionRepo::new(db);
    let tag_repo = TagRepo::new(db);
    let transactions = tx_repo.get_all()?;
    let tags = tag_repo.get_all()?;

    let tag_name = |tag_id: i64| -> String {
        tags.iter()
            .find(|t| t.id == Some(tag_id))
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "Unknown".into())
    };

    let rows: Vec<ExportTransaction> = transactions
        .iter()
        .map(|tx| ExportTransaction {
            date: tx.date.format("%Y-%m-%d").to_string(),
            source: tx.source.clone(),
            amount: tx.amount.to_string(),
            kind: match tx.kind {
                TransactionKind::Income => "income".into(),
                TransactionKind::Expense => "expense".into(),
            },
            tag_name: tag_name(tx.tag_id),
            notes: tx.notes.clone().unwrap_or_default(),
        })
        .collect();

    match fmt {
        ExportFormat::Csv => write_csv(&path, &rows)?,
        ExportFormat::Json => write_json(&path, &rows)?,
    }

    println!(
        "Exported {} transactions to {} ({})",
        rows.len(),
        path.display(),
        fmt.label()
    );
    Ok(())
}

enum ExportFormat {
    Csv,
    Json,
}

impl ExportFormat {
    fn label(&self) -> &'static str {
        match self {
            ExportFormat::Csv => "CSV",
            ExportFormat::Json => "JSON",
        }
    }
}

fn detect_format(path: &std::path::Path, format: Option<String>) -> Result<ExportFormat> {
    if let Some(f) = format {
        return match f.to_lowercase().as_str() {
            "csv" => Ok(ExportFormat::Csv),
            "json" => Ok(ExportFormat::Json),
            other => Err(AppError::Validation(format!(
                "Unknown format: '{other}'. Use 'csv' or 'json'."
            ))),
        };
    }

    match path.extension().and_then(|e| e.to_str()) {
        Some("csv") => Ok(ExportFormat::Csv),
        Some("json") => Ok(ExportFormat::Json),
        _ => Err(AppError::Validation(
            "Cannot detect format from file extension. Use --format csv or --format json.".into(),
        )),
    }
}

fn write_csv(path: &PathBuf, rows: &[ExportTransaction]) -> Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record(["date", "source", "amount", "kind", "tag_name", "notes"])?;
    for row in rows {
        wtr.write_record([
            &row.date,
            &row.source,
            &row.amount,
            &row.kind,
            &row.tag_name,
            &row.notes,
        ])?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_json(path: &PathBuf, rows: &[ExportTransaction]) -> Result<()> {
    let json = serde_json::to_string_pretty(rows)
        .map_err(|e| AppError::Validation(format!("JSON serialization error: {e}")))?;
    std::fs::write(path, json)?;
    Ok(())
}
