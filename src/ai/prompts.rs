use crate::domain::models::format_cents;

/// Data required to build an insights prompt.
pub struct InsightsData<'a> {
    pub period: &'a str,
    pub income: i64,
    pub expense: i64,
    pub prev_income: i64,
    pub prev_expense: i64,
    pub expense_by_tag: &'a [(String, i64, f64)],
    pub budget_status: &'a [(String, i64, i64, f64)],
    pub monthly_trend: &'a [(String, i64, i64)],
    pub currency: &'a str,
    pub tsep: &'a str,
    pub dsep: &'a str,
}

/// Build a prompt for monthly financial insights.
pub fn build_insights_prompt(data: &InsightsData<'_>) -> String {
    let balance = data.income - data.expense;
    let fmt = |amount: i64| format_cents(amount, data.currency, data.tsep, data.dsep);

    let inc_delta = if data.prev_income > 0 {
        let pct = ((data.income - data.prev_income) as f64 / data.prev_income.unsigned_abs() as f64) * 100.0;
        format!("{:+.1}%", pct)
    } else {
        "--".to_string()
    };
    let exp_delta = if data.prev_expense > 0 {
        let pct = ((data.expense - data.prev_expense) as f64 / data.prev_expense.unsigned_abs() as f64) * 100.0;
        format!("{:+.1}%", pct)
    } else {
        "--".to_string()
    };

    let mut categories = String::new();
    for (name, amount, pct) in data.expense_by_tag {
        categories.push_str(&format!("  - {}: {} ({:.1}%)\n", name, fmt(*amount), pct));
    }

    let mut budgets = String::new();
    for (label, spent, limit, pct) in data.budget_status {
        budgets.push_str(&format!(
            "  - {}: {} / {} ({:.0}%)\n",
            label,
            fmt(*spent),
            fmt(*limit),
            pct
        ));
    }

    let mut trend = String::new();
    for (month, _inc, exp) in data.monthly_trend.iter().rev().take(3) {
        trend.push_str(&format!("  - {}: {}\n", month, fmt(*exp)));
    }

    let period = data.period;

    format!(
        r#"You are a personal finance analyst. Analyze spending data and provide 3-5 concise insights in Spanish. Each insight should be one sentence with specific numbers. Focus on: trends, anomalies, savings opportunities, budget compliance.

Data:
- Period: {period}
- Income: {income} | Expenses: {expense} | Balance: {balance}
- vs Previous Period: Income {inc_delta}, Expenses {exp_delta}
- Top categories:
{categories}- Budget status:
{budgets}- Recent expense trend:
{trend}
Respond with a JSON array of strings, each being one insight. Example:
["Insight one.", "Insight two.", "Insight three."]

IMPORTANT: Respond ONLY with the JSON array, no other text."#,
        income = fmt(data.income),
        expense = fmt(data.expense),
        balance = fmt(balance),
    )
}

/// Build a prompt for assigning a tag to a transaction based on its description.
pub fn build_tag_assignment_prompt(source: &str, amount: i64, tag_names: &[String]) -> String {
    let tags = tag_names.join(", ");
    format!(
        r#"You are a transaction categorizer. Given a transaction, reply with ONLY the exact tag name that best matches. No other text.

Available tags: {tags}
Transaction: "{source}" (amount: {amount})

Reply with exactly one tag name from the list above, nothing else."#,
    )
}

/// Data required to build an AI rules prompt.
pub struct AiRulesData<'a> {
    pub rules: &'a str,
    pub source: &'a str,
    pub amount: i64,
    pub kind: &'a str,
    pub date: &'a str,
    pub email_subject: &'a str,
    pub tag_names: &'a [String],
    pub currency: &'a str,
    pub tsep: &'a str,
    pub dsep: &'a str,
}

/// Build a prompt for AI rules-based tag assignment.
///
/// Uses the user's natural-language `rules_prompt` together with transaction
/// details so Ollama can assign a tag or decide to SKIP the transaction.
pub fn build_ai_rules_prompt(data: &AiRulesData<'_>) -> String {
    let tags = data.tag_names.join(", ");
    let formatted_amount = format_cents(data.amount, data.currency, data.tsep, data.dsep);
    let rules = data.rules;
    let source = data.source;
    let kind = data.kind;
    let date = data.date;
    let email_subject = data.email_subject;

    format!(
        r#"You are a transaction categorizer for a personal finance app.

USER RULES (follow these strictly):
{rules}

IMPORTANT: Only reply "SKIP" if a rule EXPLICITLY says to skip. If no rule matches, assign the most appropriate tag from the list below.

AVAILABLE TAGS: {tags}

TRANSACTION:
- Source: {source}
- Amount: {formatted_amount}
- Type: {kind}
- Date: {date}
- Email subject: {email_subject}

Reply with EXACTLY one tag name from the list, or "SKIP" if a rule explicitly says so.
No other text."#,
    )
}

/// Build a prompt for natural language query parsing.
pub fn build_search_prompt(
    query: &str,
    tag_names: &[String],
    date_range: (&str, &str),
    today: &str,
) -> String {
    let tags = tag_names.join(", ");

    format!(
        r#"You are a query parser for a personal finance app. Convert natural language questions into structured filters.

Available tags: {tags}
Data date range: {start} to {end}
Today: {today}

User query: "{query}"

Respond ONLY with a JSON object (no other text):
{{
  "search": "string or null",
  "kind": "income or expense or null",
  "tag": "tag name or null",
  "date_from": "YYYY-MM-DD or null",
  "date_to": "YYYY-MM-DD or null",
  "min_amount": null,
  "max_amount": null
}}"#,
        start = date_range.0,
        end = date_range.1,
    )
}
