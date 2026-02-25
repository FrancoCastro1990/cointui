use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use chrono::NaiveDate;

use crate::db::connection::Database;
use crate::db::tag_repo::TagRepo;
use crate::db::transaction_repo::TransactionRepo;
use crate::domain::models::{Transaction, TransactionKind};
use crate::error::{AppError, Result};

/// Mapping configuration for CSV columns.
struct ColumnMapping {
    date_col: usize,
    source_col: usize,
    amount_col: usize,
    notes_col: Option<usize>,
    date_format: String,
    negative_is_expense: bool,
}

pub fn run(path: PathBuf, db: &Database) -> Result<()> {
    if !path.exists() {
        return Err(AppError::NotFound(format!("CSV file: {}", path.display())));
    }

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(&path)?;

    let headers = reader.headers()?.clone();
    println!("CSV file: {}", path.display());
    println!("Columns found:");
    for (i, h) in headers.iter().enumerate() {
        println!("  [{i}] {h}");
    }
    println!();

    // Show first 3 rows as preview.
    let records: Vec<csv::StringRecord> = reader.records().filter_map(|r| r.ok()).collect();
    let preview_count = records.len().min(3);
    if preview_count > 0 {
        println!("Preview ({preview_count} rows):");
        for record in &records[..preview_count] {
            let fields: Vec<&str> = record.iter().collect();
            println!("  {}", fields.join(" | "));
        }
        println!();
    }

    let mapping = prompt_mapping(headers.len())?;

    // Get default tag for imports.
    let tag_repo = TagRepo::new(db);
    let tags = tag_repo.get_all()?;
    let default_tag_id = tags
        .iter()
        .find(|t| t.name == "Otros")
        .or_else(|| tags.first())
        .and_then(|t| t.id)
        .ok_or_else(|| {
            AppError::Validation("No tags in database. Run TUI first to seed tags.".into())
        })?;

    let tx_repo = TransactionRepo::new(db);
    let mut imported = 0u32;
    let mut skipped = 0u32;

    for (line_num, record) in records.iter().enumerate() {
        match parse_record(record, &mapping, default_tag_id) {
            Ok(tx) => {
                tx_repo.create(&tx)?;
                imported += 1;
            }
            Err(e) => {
                eprintln!("  Row {}: skipped - {e}", line_num + 2);
                skipped += 1;
            }
        }
    }

    println!();
    println!("Import complete: {imported} imported, {skipped} skipped.");
    Ok(())
}

fn prompt_mapping(num_cols: usize) -> Result<ColumnMapping> {
    let stdin = io::stdin();
    let mut lines = stdin.lock();

    let date_col = prompt_usize(&mut lines, "Date column index", num_cols)?;
    let source_col = prompt_usize(&mut lines, "Source/description column index", num_cols)?;
    let amount_col = prompt_usize(&mut lines, "Amount column index", num_cols)?;
    let notes_col = prompt_optional_usize(&mut lines, "Notes column index (blank to skip)", num_cols)?;

    print!("Date format (default: %Y-%m-%d): ");
    io::stdout().flush()?;
    let mut date_fmt = String::new();
    lines.read_line(&mut date_fmt)?;
    let date_format = date_fmt.trim().to_string();
    let date_format = if date_format.is_empty() {
        "%Y-%m-%d".to_string()
    } else {
        date_format
    };

    print!("Negative amounts mean expense? [Y/n]: ");
    io::stdout().flush()?;
    let mut neg_answer = String::new();
    lines.read_line(&mut neg_answer)?;
    let negative_is_expense = !neg_answer.trim().eq_ignore_ascii_case("n");

    Ok(ColumnMapping {
        date_col,
        source_col,
        amount_col,
        notes_col,
        date_format,
        negative_is_expense,
    })
}

fn prompt_usize(reader: &mut impl BufRead, label: &str, max: usize) -> Result<usize> {
    loop {
        print!("{label} [0-{}]: ", max - 1);
        io::stdout().flush()?;
        let mut input = String::new();
        reader.read_line(&mut input)?;
        if let Ok(v) = input.trim().parse::<usize>()
            && v < max {
                return Ok(v);
            }
        println!("  Invalid. Enter a number between 0 and {}.", max - 1);
    }
}

fn prompt_optional_usize(
    reader: &mut impl BufRead,
    label: &str,
    max: usize,
) -> Result<Option<usize>> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut input = String::new();
    reader.read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    match trimmed.parse::<usize>() {
        Ok(v) if v < max => Ok(Some(v)),
        _ => Ok(None),
    }
}

fn parse_record(
    record: &csv::StringRecord,
    mapping: &ColumnMapping,
    default_tag_id: i64,
) -> std::result::Result<Transaction, String> {
    let date_str = record
        .get(mapping.date_col)
        .ok_or("Missing date column")?
        .trim();
    let date = NaiveDate::parse_from_str(date_str, &mapping.date_format)
        .map_err(|e| format!("Bad date '{date_str}': {e}"))?;

    let source = record
        .get(mapping.source_col)
        .ok_or("Missing source column")?
        .trim()
        .to_string();
    if source.is_empty() {
        return Err("Empty source".into());
    }

    let amount_str = record
        .get(mapping.amount_col)
        .ok_or("Missing amount column")?
        .trim()
        .replace(',', "");
    let amount_f64: f64 = amount_str
        .parse()
        .map_err(|_| format!("Bad amount: '{amount_str}'"))?;

    let (kind, amount) = if mapping.negative_is_expense {
        if amount_f64 < 0.0 {
            (TransactionKind::Expense, (-amount_f64 * 100.0).round() as i64)
        } else {
            (TransactionKind::Income, (amount_f64 * 100.0).round() as i64)
        }
    } else {
        (
            TransactionKind::Expense,
            (amount_f64.abs() * 100.0).round() as i64,
        )
    };

    if amount == 0 {
        return Err("Zero amount".into());
    }

    let notes = mapping
        .notes_col
        .and_then(|col| record.get(col))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(Transaction {
        id: None,
        source,
        amount,
        kind,
        tag_id: default_tag_id,
        date,
        notes,
        created_at: None,
        updated_at: None,
    })
}
