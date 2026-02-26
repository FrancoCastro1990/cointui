use std::path::PathBuf;

use chrono::{Datelike, Local, NaiveDate};

use crate::config::AppConfig;
use crate::db::budget_repo::BudgetRepo;
use crate::db::connection::Database;
use crate::db::tag_repo::TagRepo;
use crate::db::transaction_repo::TransactionRepo;
use crate::domain::models::{format_cents, BudgetPeriod, Tag};
use crate::error::{AppError, Result};

/// Report data gathered from the database.
struct ReportData {
    title: String,
    income: i64,
    expense: i64,
    balance: i64,
    prev_income: Option<i64>,
    prev_expense: Option<i64>,
    expense_by_tag: Vec<(String, i64, f64)>,
    budget_status: Vec<(String, i64, i64, f64)>,
    monthly_breakdown: Option<Vec<(String, i64, i64)>>,
    tx_count: usize,
}

/// Parse a `--report` argument like "monthly", "monthly 2026-01", "yearly", "yearly 2025",
/// "compare 2026-01 2026-02".
pub fn run(args: &[String], output: Option<PathBuf>, db: &Database, config: &AppConfig) -> Result<()> {
    if args.is_empty() {
        return Err(AppError::Validation(
            "Usage: --report <monthly|yearly|compare> [args...]".into(),
        ));
    }

    let subcommand = args[0].to_lowercase();
    let data = match subcommand.as_str() {
        "monthly" => {
            let month_str = args.get(1).map(|s| s.as_str());
            gather_monthly(month_str, db, config)?
        }
        "yearly" => {
            let year_str = args.get(1).map(|s| s.as_str());
            gather_yearly(year_str, db, config)?
        }
        "compare" => {
            let a = args.get(1).ok_or_else(|| {
                AppError::Validation("compare requires two periods: --report compare YYYY-MM YYYY-MM".into())
            })?;
            let b = args.get(2).ok_or_else(|| {
                AppError::Validation("compare requires two periods: --report compare YYYY-MM YYYY-MM".into())
            })?;
            gather_compare(a, b, db, config)?
        }
        other => {
            return Err(AppError::Validation(format!(
                "Unknown report type: '{other}'. Use 'monthly', 'yearly', or 'compare'."
            )));
        }
    };

    match output {
        Some(path) => {
            let md = render_markdown(&data, config);
            std::fs::write(&path, md)?;
            println!("Report written to {}", path.display());
        }
        None => {
            render_terminal(&data, config);
        }
    }

    Ok(())
}

fn parse_month(s: &str) -> Result<(i32, u32)> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return Err(AppError::Validation(format!(
            "Invalid month format: '{s}'. Expected YYYY-MM."
        )));
    }
    let year: i32 = parts[0].parse().map_err(|_| {
        AppError::Validation(format!("Invalid year in '{s}'."))
    })?;
    let month: u32 = parts[1].parse().map_err(|_| {
        AppError::Validation(format!("Invalid month in '{s}'."))
    })?;
    if !(1..=12).contains(&month) {
        return Err(AppError::Validation(format!("Month out of range in '{s}'.")));
    }
    Ok((year, month))
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

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn tag_lookup(tags: &[Tag]) -> impl Fn(i64) -> String + '_ {
    move |tag_id: i64| {
        tags.iter()
            .find(|t| t.id == Some(tag_id))
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }
}

fn gather_monthly(month_str: Option<&str>, db: &Database, _config: &AppConfig) -> Result<ReportData> {
    let today = Local::now().date_naive();
    let (year, month) = match month_str {
        Some(s) => parse_month(s)?,
        None => (today.year(), today.month()),
    };

    let (start, end) = month_bounds(year, month);
    let (py, pm) = prev_month(year, month);
    let (prev_start, prev_end) = month_bounds(py, pm);

    let tx_repo = TransactionRepo::new(db);
    let tag_repo = TagRepo::new(db);
    let budget_repo = BudgetRepo::new(db);
    let tags = tag_repo.get_all()?;
    let tag_name = tag_lookup(&tags);

    let (income, expense) = tx_repo.get_totals_for_period(&fmt_date(start), &fmt_date(end))?;
    let (prev_income, prev_expense) = tx_repo.get_totals_for_period(&fmt_date(prev_start), &fmt_date(prev_end))?;

    let expense_by_tag_raw = tx_repo.get_expense_by_tag_for_period(&fmt_date(start), &fmt_date(end))?;
    let expense_by_tag: Vec<(String, i64, f64)> = expense_by_tag_raw
        .iter()
        .map(|(tid, amt)| {
            let pct = if expense > 0 { *amt as f64 / expense as f64 * 100.0 } else { 0.0 };
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
            let pct = if b.amount > 0 { spent as f64 / b.amount as f64 * 100.0 } else { 0.0 };
            (label, spent, b.amount, pct)
        })
        .collect();

    let filter = crate::db::transaction_repo::TransactionFilter {
        date_from: Some(start),
        date_to: Some(end - chrono::Duration::days(1)),
        ..Default::default()
    };
    let tx_count = tx_repo.get_filtered(&filter)?.len();

    let month_name = start.format("%B %Y").to_string();

    Ok(ReportData {
        title: format!("CoinTUI Monthly Report: {}", month_name),
        income,
        expense,
        balance: income - expense,
        prev_income: Some(prev_income),
        prev_expense: Some(prev_expense),
        expense_by_tag,
        budget_status,
        monthly_breakdown: None,
        tx_count,
    })
}

fn gather_yearly(year_str: Option<&str>, db: &Database, _config: &AppConfig) -> Result<ReportData> {
    let today = Local::now().date_naive();
    let year: i32 = match year_str {
        Some(s) => s.parse().map_err(|_| {
            AppError::Validation(format!("Invalid year: '{s}'."))
        })?,
        None => today.year(),
    };

    let start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap();
    let prev_start = NaiveDate::from_ymd_opt(year - 1, 1, 1).unwrap();
    let prev_end = start;

    let tx_repo = TransactionRepo::new(db);
    let tag_repo = TagRepo::new(db);
    let budget_repo = BudgetRepo::new(db);
    let tags = tag_repo.get_all()?;
    let tag_name = tag_lookup(&tags);

    let (income, expense) = tx_repo.get_totals_for_period(&fmt_date(start), &fmt_date(end))?;
    let (prev_income, prev_expense) = tx_repo.get_totals_for_period(&fmt_date(prev_start), &fmt_date(prev_end))?;

    let expense_by_tag_raw = tx_repo.get_expense_by_tag_for_period(&fmt_date(start), &fmt_date(end))?;
    let expense_by_tag: Vec<(String, i64, f64)> = expense_by_tag_raw
        .iter()
        .map(|(tid, amt)| {
            let pct = if expense > 0 { *amt as f64 / expense as f64 * 100.0 } else { 0.0 };
            (tag_name(*tid), *amt, pct)
        })
        .collect();

    let budgets = budget_repo.get_active()?;
    let budget_status: Vec<(String, i64, i64, f64)> = budgets
        .iter()
        .filter(|b| b.period == BudgetPeriod::Yearly)
        .map(|b| {
            let spent = budget_repo.get_spent_for_budget(b).unwrap_or(0);
            let label = match b.tag_id {
                Some(tid) => format!("{} (yearly)", tag_name(tid)),
                None => "Global (yearly)".to_string(),
            };
            let pct = if b.amount > 0 { spent as f64 / b.amount as f64 * 100.0 } else { 0.0 };
            (label, spent, b.amount, pct)
        })
        .collect();

    // Monthly breakdown for the year
    let monthly = tx_repo.get_monthly_totals(12)?;
    let monthly_breakdown: Vec<(String, i64, i64)> = monthly
        .into_iter()
        .filter(|(m, _, _)| m.starts_with(&year.to_string()))
        .collect();

    let filter = crate::db::transaction_repo::TransactionFilter {
        date_from: Some(start),
        date_to: Some(end - chrono::Duration::days(1)),
        ..Default::default()
    };
    let tx_count = tx_repo.get_filtered(&filter)?.len();

    Ok(ReportData {
        title: format!("CoinTUI Yearly Report: {}", year),
        income,
        expense,
        balance: income - expense,
        prev_income: Some(prev_income),
        prev_expense: Some(prev_expense),
        expense_by_tag,
        budget_status,
        monthly_breakdown: Some(monthly_breakdown),
        tx_count,
    })
}

fn gather_compare(period_a: &str, period_b: &str, db: &Database, config: &AppConfig) -> Result<ReportData> {
    let data_a = gather_monthly(Some(period_a), db, config)?;
    let data_b = gather_monthly(Some(period_b), db, config)?;

    let expense_by_tag = data_b.expense_by_tag;
    let budget_status = data_b.budget_status;

    Ok(ReportData {
        title: format!("CoinTUI Comparison: {} vs {}", period_a, period_b),
        income: data_b.income,
        expense: data_b.expense,
        balance: data_b.balance,
        prev_income: Some(data_a.income),
        prev_expense: Some(data_a.expense),
        expense_by_tag,
        budget_status,
        monthly_breakdown: None,
        tx_count: data_a.tx_count + data_b.tx_count,
    })
}

// ---------------------------------------------------------------------------
// Terminal rendering
// ---------------------------------------------------------------------------

fn render_terminal(data: &ReportData, config: &AppConfig) {
    let c = &config.currency;
    let t = &config.thousands_separator;
    let d = &config.decimal_separator;

    println!();
    println!("  {}", data.title);
    println!("  {}", "=".repeat(data.title.len()));
    println!();

    // Summary
    println!("  Summary");
    println!("  -------");
    print_row("Income", data.income, data.prev_income, c, t, d);
    print_row("Expenses", data.expense, data.prev_expense, c, t, d);
    let prev_balance = match (data.prev_income, data.prev_expense) {
        (Some(pi), Some(pe)) => Some(pi - pe),
        _ => None,
    };
    print_row("Balance", data.balance, prev_balance, c, t, d);
    println!("  Transactions: {}", data.tx_count);
    println!();

    // Spending by category
    if !data.expense_by_tag.is_empty() {
        println!("  Spending by Category");
        println!("  --------------------");
        for (name, amount, pct) in &data.expense_by_tag {
            println!(
                "  {:<16} {:>16}  {:>5.1}%",
                name,
                format_cents(*amount, c, t, d),
                pct
            );
        }
        println!();
    }

    // Budget status
    if !data.budget_status.is_empty() {
        println!("  Budget Status");
        println!("  -------------");
        for (label, spent, limit, pct) in &data.budget_status {
            let indicator = if *pct >= 100.0 {
                "OVER"
            } else if *pct >= 80.0 {
                "WARN"
            } else {
                "OK"
            };
            println!(
                "  {:<24} {} / {} ({:.0}%) [{}]",
                label,
                format_cents(*spent, c, t, d),
                format_cents(*limit, c, t, d),
                pct,
                indicator
            );
        }
        println!();
    }

    // Monthly breakdown (yearly reports)
    if let Some(ref months) = data.monthly_breakdown
        && !months.is_empty() {
            println!("  Monthly Breakdown");
            println!("  -----------------");
            println!("  {:<10} {:>14} {:>14} {:>14}", "Month", "Income", "Expense", "Net");
            for (month, inc, exp) in months {
                let net = inc - exp;
                println!(
                    "  {:<10} {:>14} {:>14} {:>14}",
                    month,
                    format_cents(*inc, c, t, d),
                    format_cents(*exp, c, t, d),
                    format_cents(net, c, t, d),
                );
            }
            println!();
        }

    let today = Local::now().date_naive();
    println!("  Generated by CoinTUI on {}", today.format("%Y-%m-%d"));
    println!();
}

fn print_row(label: &str, current: i64, previous: Option<i64>, c: &str, t: &str, d: &str) {
    let formatted = format_cents(current, c, t, d);
    let delta = match previous {
        Some(prev) if prev != 0 => {
            let pct = ((current - prev) as f64 / prev.unsigned_abs() as f64) * 100.0;
            if pct >= 0.0 {
                format!("+{:.1}%", pct)
            } else {
                format!("{:.1}%", pct)
            }
        }
        _ => "--".to_string(),
    };
    println!("  {:<12} {:>16}  {}", label, formatted, delta);
}

// ---------------------------------------------------------------------------
// Markdown rendering
// ---------------------------------------------------------------------------

fn render_markdown(data: &ReportData, config: &AppConfig) -> String {
    let c = &config.currency;
    let t = &config.thousands_separator;
    let d = &config.decimal_separator;
    let mut md = String::new();

    md.push_str(&format!("# {}\n\n", data.title));

    // Summary table
    md.push_str("## Summary\n\n");
    md.push_str("| Metric | Amount | vs Previous |\n");
    md.push_str("|--------|--------|-------------|\n");

    let fmt_delta = |current: i64, previous: Option<i64>| -> String {
        match previous {
            Some(prev) if prev != 0 => {
                let pct = ((current - prev) as f64 / prev.unsigned_abs() as f64) * 100.0;
                if pct >= 0.0 {
                    format!("+{:.1}%", pct)
                } else {
                    format!("{:.1}%", pct)
                }
            }
            _ => "--".to_string(),
        }
    };

    md.push_str(&format!(
        "| Income | {} | {} |\n",
        format_cents(data.income, c, t, d),
        fmt_delta(data.income, data.prev_income)
    ));
    md.push_str(&format!(
        "| Expenses | {} | {} |\n",
        format_cents(data.expense, c, t, d),
        fmt_delta(data.expense, data.prev_expense)
    ));
    let prev_balance = match (data.prev_income, data.prev_expense) {
        (Some(pi), Some(pe)) => Some(pi - pe),
        _ => None,
    };
    md.push_str(&format!(
        "| Balance | {} | {} |\n",
        format_cents(data.balance, c, t, d),
        fmt_delta(data.balance, prev_balance)
    ));
    md.push_str(&format!("| Transactions | {} | |\n\n", data.tx_count));

    // Spending by category
    if !data.expense_by_tag.is_empty() {
        md.push_str("## Spending by Category\n\n");
        md.push_str("| Category | Amount | % of Total |\n");
        md.push_str("|----------|--------|------------|\n");
        for (name, amount, pct) in &data.expense_by_tag {
            md.push_str(&format!(
                "| {} | {} | {:.1}% |\n",
                name,
                format_cents(*amount, c, t, d),
                pct
            ));
        }
        md.push('\n');
    }

    // Budget status
    if !data.budget_status.is_empty() {
        md.push_str("## Budget Status\n\n");
        md.push_str("| Budget | Spent | Limit | Usage |\n");
        md.push_str("|--------|-------|-------|-------|\n");
        for (label, spent, limit, pct) in &data.budget_status {
            md.push_str(&format!(
                "| {} | {} | {} | {:.1}% |\n",
                label,
                format_cents(*spent, c, t, d),
                format_cents(*limit, c, t, d),
                pct
            ));
        }
        md.push('\n');
    }

    // Monthly breakdown
    if let Some(ref months) = data.monthly_breakdown
        && !months.is_empty() {
            md.push_str("## Monthly Breakdown\n\n");
            md.push_str("| Month | Income | Expense | Net |\n");
            md.push_str("|-------|--------|---------|-----|\n");
            for (month, inc, exp) in months {
                let net = inc - exp;
                md.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    month,
                    format_cents(*inc, c, t, d),
                    format_cents(*exp, c, t, d),
                    format_cents(net, c, t, d),
                ));
            }
            md.push('\n');
        }

    md.push_str("---\n");
    let today = Local::now().date_naive();
    md.push_str(&format!(
        "*Generated by CoinTUI on {}*\n",
        today.format("%Y-%m-%d")
    ));

    md
}
