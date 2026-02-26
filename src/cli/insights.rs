use chrono::{Datelike, Local, NaiveDate};

use crate::ai::ollama::OllamaClient;
use crate::ai::prompts;
use crate::config::AppConfig;
use crate::db::budget_repo::BudgetRepo;
use crate::db::connection::Database;
use crate::db::tag_repo::TagRepo;
use crate::db::transaction_repo::TransactionRepo;
use crate::domain::models::format_cents;
use crate::error::{AppError, Result};

/// Parse the period argument: `None` = current month, `YYYY` = full year, `YYYY-MM` = single month.
fn parse_period(s: Option<&str>) -> Result<(NaiveDate, NaiveDate, NaiveDate, NaiveDate, String)> {
    let today = Local::now().date_naive();

    match s {
        None => {
            // Current month
            let (start, end) = month_bounds(today.year(), today.month());
            let (py, pm) = prev_month(today.year(), today.month());
            let (prev_start, prev_end) = month_bounds(py, pm);
            let label = start.format("%B %Y").to_string();
            Ok((start, end, prev_start, prev_end, label))
        }
        Some(arg) => {
            if arg.contains('-') {
                // YYYY-MM
                let parts: Vec<&str> = arg.split('-').collect();
                if parts.len() != 2 {
                    return Err(AppError::Validation(format!(
                        "Invalid format: '{arg}'. Expected YYYY-MM or YYYY."
                    )));
                }
                let y: i32 = parts[0]
                    .parse()
                    .map_err(|_| AppError::Validation(format!("Invalid year in '{arg}'.")))?;
                let m: u32 = parts[1]
                    .parse()
                    .map_err(|_| AppError::Validation(format!("Invalid month in '{arg}'.")))?;
                if !(1..=12).contains(&m) {
                    return Err(AppError::Validation(format!("Month out of range in '{arg}'.")));
                }
                let (start, end) = month_bounds(y, m);
                let (py, pm) = prev_month(y, m);
                let (prev_start, prev_end) = month_bounds(py, pm);
                let label = start.format("%B %Y").to_string();
                Ok((start, end, prev_start, prev_end, label))
            } else {
                // YYYY (full year)
                let y: i32 = arg
                    .parse()
                    .map_err(|_| AppError::Validation(format!("Invalid year: '{arg}'.")))?;
                let start = NaiveDate::from_ymd_opt(y, 1, 1).unwrap();
                let end = NaiveDate::from_ymd_opt(y + 1, 1, 1).unwrap();
                let prev_start = NaiveDate::from_ymd_opt(y - 1, 1, 1).unwrap();
                let prev_end = start;
                let label = y.to_string();
                Ok((start, end, prev_start, prev_end, label))
            }
        }
    }
}

fn month_bounds(year: i32, month: u32) -> (NaiveDate, NaiveDate) {
    let start = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let end = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };
    (start, end)
}

fn prev_month(year: i32, month: u32) -> (i32, u32) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

pub fn run(period_str: Option<&str>, db: &Database, config: &AppConfig) -> Result<()> {
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

    let (start, end, prev_start, prev_end, period_label) = parse_period(period_str)?;
    let fmt = |d: NaiveDate| d.format("%Y-%m-%d").to_string();

    let tx_repo = TransactionRepo::new(db);
    let tag_repo = TagRepo::new(db);
    let budget_repo = BudgetRepo::new(db);
    let tags = tag_repo.get_all()?;

    let tag_name = |tid: i64| -> String {
        tags.iter()
            .find(|t| t.id == Some(tid))
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    };

    let (income, expense) = tx_repo.get_totals_for_period(&fmt(start), &fmt(end))?;
    let (prev_income, prev_expense) =
        tx_repo.get_totals_for_period(&fmt(prev_start), &fmt(prev_end))?;

    let expense_by_tag_raw = tx_repo.get_expense_by_tag_for_period(&fmt(start), &fmt(end))?;
    let expense_by_tag: Vec<(String, i64, f64)> = expense_by_tag_raw
        .iter()
        .map(|(tid, amt)| {
            let pct = if expense > 0 {
                *amt as f64 / expense as f64 * 100.0
            } else {
                0.0
            };
            (tag_name(*tid), *amt, pct)
        })
        .collect();

    let budgets = budget_repo.get_active()?;
    let budget_status: Vec<(String, i64, i64, f64)> = budgets
        .iter()
        .map(|b| {
            let spent = budget_repo.get_spent_for_budget(b).unwrap_or(0);
            let label = match b.tag_id {
                Some(tid) => format!("{} ({})", tag_name(tid), b.period),
                None => format!("Global ({})", b.period),
            };
            let pct = if b.amount > 0 {
                spent as f64 / b.amount as f64 * 100.0
            } else {
                0.0
            };
            (label, spent, b.amount, pct)
        })
        .collect();

    let monthly_totals = tx_repo.get_monthly_totals(12)?;

    let c = &config.currency;
    let t = &config.thousands_separator;
    let d = &config.decimal_separator;

    let prompt = prompts::build_insights_prompt(&prompts::InsightsData {
        period: &period_label,
        income,
        expense,
        prev_income,
        prev_expense,
        expense_by_tag: &expense_by_tag,
        budget_status: &budget_status,
        monthly_trend: &monthly_totals,
        currency: c,
        tsep: t,
        dsep: d,
    });

    println!();
    println!("  Generating AI insights for {}...", period_label);
    println!();

    let response = client.generate(&prompt)?;

    // Try to parse as JSON array
    let insights: Vec<String> = serde_json::from_str(&response).unwrap_or_else(|_| {
        // Fallback: try to extract JSON from within the response
        if let Some(start_idx) = response.find('[') {
            if let Some(end_idx) = response.rfind(']') {
                let json_str = &response[start_idx..=end_idx];
                serde_json::from_str(json_str).unwrap_or_else(|_| vec![response.clone()])
            } else {
                vec![response.clone()]
            }
        } else {
            vec![response]
        }
    });

    println!("  AI Insights: {}", period_label);
    println!("  {}", "=".repeat(40));
    println!();
    for (i, insight) in insights.iter().enumerate() {
        println!("  {}. {}", i + 1, insight);
        println!();
    }

    println!(
        "  Summary: {} income, {} expenses",
        format_cents(income, c, t, d),
        format_cents(expense, c, t, d),
    );
    println!();

    Ok(())
}
