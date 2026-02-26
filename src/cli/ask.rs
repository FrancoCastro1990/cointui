use chrono::{Local, NaiveDate};

use crate::ai::ollama::OllamaClient;
use crate::ai::prompts;
use crate::config::AppConfig;
use crate::db::connection::Database;
use crate::db::tag_repo::TagRepo;
use crate::db::transaction_repo::{TransactionFilter, TransactionRepo};
use crate::domain::models::{format_cents, TransactionKind};
use crate::error::{AppError, Result};

pub fn run(query: &str, db: &Database, config: &AppConfig) -> Result<()> {
    let client = OllamaClient::from_config(&config.ai).ok_or_else(|| {
        AppError::Validation(
            "AI not available. Enable [ai] in config.toml and set enabled = true.".into(),
        )
    })?;

    if !client.is_available() {
        return Err(AppError::Validation(
            "Ollama is not running. Start it with: ollama serve".into(),
        ));
    }

    let tag_repo = TagRepo::new(db);
    let tx_repo = TransactionRepo::new(db);
    let tags = tag_repo.get_all()?;
    let tag_names: Vec<String> = tags.iter().map(|t| t.name.clone()).collect();

    // Get date range of existing data
    let all_txs = tx_repo.get_all()?;
    let (date_from, date_to) = if all_txs.is_empty() {
        let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
        (today.clone(), today)
    } else {
        let dates: Vec<&NaiveDate> = all_txs.iter().map(|t| &t.date).collect();
        let min = dates.iter().min().unwrap().format("%Y-%m-%d").to_string();
        let max = dates.iter().max().unwrap().format("%Y-%m-%d").to_string();
        (min, max)
    };

    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();

    let prompt = prompts::build_search_prompt(query, &tag_names, (&date_from, &date_to), &today);

    println!();
    println!("  Parsing query: \"{}\"", query);
    println!();

    let response = client.generate(&prompt)?;

    // Parse the JSON response
    let parsed: serde_json::Value = extract_json(&response)?;

    let mut filter = TransactionFilter::default();

    if let Some(s) = parsed.get("search").and_then(|v| v.as_str())
        && !s.is_empty() {
            filter.search = Some(s.to_string());
        }

    if let Some(kind_str) = parsed.get("kind").and_then(|v| v.as_str()) {
        filter.kind = match kind_str.to_lowercase().as_str() {
            "income" => Some(TransactionKind::Income),
            "expense" => Some(TransactionKind::Expense),
            _ => None,
        };
    }

    if let Some(tag_name) = parsed.get("tag").and_then(|v| v.as_str())
        && let Some(tag) = tags.iter().find(|t| t.name.eq_ignore_ascii_case(tag_name)) {
            filter.tag_id = tag.id;
        }

    if let Some(df) = parsed.get("date_from").and_then(|v| v.as_str())
        && let Ok(d) = NaiveDate::parse_from_str(df, "%Y-%m-%d") {
            filter.date_from = Some(d);
        }

    if let Some(dt) = parsed.get("date_to").and_then(|v| v.as_str())
        && let Ok(d) = NaiveDate::parse_from_str(dt, "%Y-%m-%d") {
            filter.date_to = Some(d);
        }

    if let Some(min) = parsed.get("min_amount").and_then(|v| v.as_i64()) {
        filter.min_amount = Some(min);
    }

    if let Some(max) = parsed.get("max_amount").and_then(|v| v.as_i64()) {
        filter.max_amount = Some(max);
    }

    // Show parsed filter
    println!("  Applied filters:");
    if let Some(ref s) = filter.search {
        println!("    Search: {}", s);
    }
    if let Some(k) = filter.kind {
        println!("    Kind: {}", k);
    }
    if let Some(tid) = filter.tag_id {
        let name = tags
            .iter()
            .find(|t| t.id == Some(tid))
            .map(|t| t.name.as_str())
            .unwrap_or("Unknown");
        println!("    Tag: {}", name);
    }
    if let Some(d) = filter.date_from {
        println!("    From: {}", d);
    }
    if let Some(d) = filter.date_to {
        println!("    To: {}", d);
    }
    println!();

    let results = tx_repo.get_filtered(&filter)?;

    let c = &config.currency;
    let t = &config.thousands_separator;
    let d = &config.decimal_separator;

    if results.is_empty() {
        println!("  No transactions found matching your query.");
    } else {
        println!(
            "  {:<12} {:<20} {:>14} {:<10} {:<12}",
            "Date", "Source", "Amount", "Kind", "Tag"
        );
        println!("  {}", "-".repeat(70));

        let mut total_income: i64 = 0;
        let mut total_expense: i64 = 0;

        for tx in &results {
            let tag_name = tags
                .iter()
                .find(|tg| tg.id == Some(tx.tag_id))
                .map(|tg| tg.name.as_str())
                .unwrap_or("Unknown");

            println!(
                "  {:<12} {:<20} {:>14} {:<10} {:<12}",
                tx.date.format("%Y-%m-%d"),
                truncate(&tx.source, 18),
                format_cents(tx.amount, c, t, d),
                tx.kind,
                tag_name,
            );

            match tx.kind {
                TransactionKind::Income => total_income += tx.amount,
                TransactionKind::Expense => total_expense += tx.amount,
            }
        }

        println!("  {}", "-".repeat(70));
        println!(
            "  {} transactions | Income: {} | Expenses: {} | Net: {}",
            results.len(),
            format_cents(total_income, c, t, d),
            format_cents(total_expense, c, t, d),
            format_cents(total_income - total_expense, c, t, d),
        );
    }

    println!();
    Ok(())
}

fn extract_json(response: &str) -> Result<serde_json::Value> {
    // Try parsing the entire response first
    if let Ok(v) = serde_json::from_str(response) {
        return Ok(v);
    }

    // Try to find JSON object within the response
    if let Some(start) = response.find('{')
        && let Some(end) = response.rfind('}') {
            let json_str = &response[start..=end];
            if let Ok(v) = serde_json::from_str(json_str) {
                return Ok(v);
            }
        }

    Err(AppError::Validation(format!(
        "Could not parse AI response as JSON: {}",
        truncate(response, 100)
    )))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
