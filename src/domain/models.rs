use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::error::{AppError, Result};

// ---------------------------------------------------------------------------
// TransactionKind
// ---------------------------------------------------------------------------

/// Whether a transaction represents money coming in or going out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionKind {
    Income,
    Expense,
}

impl fmt::Display for TransactionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionKind::Income => write!(f, "income"),
            TransactionKind::Expense => write!(f, "expense"),
        }
    }
}

impl FromStr for TransactionKind {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "income" => Ok(TransactionKind::Income),
            "expense" => Ok(TransactionKind::Expense),
            other => Err(AppError::Validation(format!(
                "Invalid transaction kind: '{other}'. Expected 'income' or 'expense'."
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Transaction
// ---------------------------------------------------------------------------

/// A single financial transaction.
///
/// `amount` is stored as whole currency units (e.g. pesos, dollars).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Option<i64>,
    /// A human-readable label such as "Mercadona" or "Nómina febrero".
    pub source: String,
    /// Amount in whole currency units. Always positive; `kind` indicates direction.
    pub amount: i64,
    pub kind: TransactionKind,
    pub tag_id: i64,
    pub date: NaiveDate,
    pub notes: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

impl Transaction {
    pub fn amount_display(&self, currency: &str, thousands_sep: &str, decimal_sep: &str) -> String {
        format_cents(self.amount, currency, thousands_sep, decimal_sep)
    }

    /// Signed amount: positive for income, negative for expense.
    pub fn signed_amount(&self) -> i64 {
        match self.kind {
            TransactionKind::Income => self.amount,
            TransactionKind::Expense => -self.amount,
        }
    }
}

// ---------------------------------------------------------------------------
// BudgetPeriod
// ---------------------------------------------------------------------------

/// The time window over which a budget is evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetPeriod {
    Weekly,
    Monthly,
    Yearly,
}

impl fmt::Display for BudgetPeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BudgetPeriod::Weekly => write!(f, "weekly"),
            BudgetPeriod::Monthly => write!(f, "monthly"),
            BudgetPeriod::Yearly => write!(f, "yearly"),
        }
    }
}

impl FromStr for BudgetPeriod {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "weekly" => Ok(BudgetPeriod::Weekly),
            "monthly" => Ok(BudgetPeriod::Monthly),
            "yearly" => Ok(BudgetPeriod::Yearly),
            other => Err(AppError::Validation(format!(
                "Invalid budget period: '{other}'. Expected 'weekly', 'monthly', or 'yearly'."
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Budget
// ---------------------------------------------------------------------------

/// A spending limit for a given tag (or global when `tag_id` is `None`)
/// over a `BudgetPeriod`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub id: Option<i64>,
    /// `None` means the budget applies globally (all tags).
    pub tag_id: Option<i64>,
    /// Maximum amount in whole currency units.
    pub amount: i64,
    pub period: BudgetPeriod,
    pub active: bool,
}

impl Budget {
    pub fn amount_display(&self, currency: &str, thousands_sep: &str, decimal_sep: &str) -> String {
        format_cents(self.amount, currency, thousands_sep, decimal_sep)
    }
}

// ---------------------------------------------------------------------------
// Tag
// ---------------------------------------------------------------------------

/// A category tag. Tags form a single-level hierarchy via `parent_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: Option<i64>,
    pub name: String,
    pub parent_id: Option<i64>,
    pub icon: Option<String>,
}

impl Tag {
    /// Returns the tag name. When parent context is available the caller can
    /// prepend the parent name; this helper is a placeholder for that pattern.
    pub fn full_name(&self, parent_name: Option<&str>) -> String {
        match parent_name {
            Some(parent) => format!("{parent} > {}", self.name),
            None => self.name.clone(),
        }
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref icon) = self.icon {
            write!(f, "{} {}", icon, self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

// ---------------------------------------------------------------------------
// RecurringInterval
// ---------------------------------------------------------------------------

/// How often a recurring entry is automatically inserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecurringInterval {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl fmt::Display for RecurringInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecurringInterval::Daily => write!(f, "daily"),
            RecurringInterval::Weekly => write!(f, "weekly"),
            RecurringInterval::Monthly => write!(f, "monthly"),
            RecurringInterval::Yearly => write!(f, "yearly"),
        }
    }
}

impl FromStr for RecurringInterval {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "daily" => Ok(RecurringInterval::Daily),
            "weekly" => Ok(RecurringInterval::Weekly),
            "monthly" => Ok(RecurringInterval::Monthly),
            "yearly" => Ok(RecurringInterval::Yearly),
            other => Err(AppError::Validation(format!(
                "Invalid recurring interval: '{other}'. Expected 'daily', 'weekly', 'monthly', or 'yearly'."
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// RecurringEntry
// ---------------------------------------------------------------------------

/// A template for transactions that are inserted automatically on a schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringEntry {
    pub id: Option<i64>,
    pub source: String,
    /// Amount in whole currency units.
    pub amount: i64,
    pub kind: TransactionKind,
    pub tag_id: i64,
    pub interval: RecurringInterval,
    pub start_date: NaiveDate,
    pub last_inserted_date: Option<NaiveDate>,
    pub active: bool,
}

impl RecurringEntry {
    pub fn amount_display(&self, currency: &str, thousands_sep: &str, decimal_sep: &str) -> String {
        format_cents(self.amount, currency, thousands_sep, decimal_sep)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format an amount as a human-readable string with thousands separators,
/// e.g. `"$ 2.700.000"` (Chilean) or `"$ 2,700,000"` (US).
///
/// Amounts are stored as whole currency units (not cents/centavos).
pub fn format_cents(amount: i64, currency: &str, thousands_sep: &str, _decimal_sep: &str) -> String {
    let abs = amount.unsigned_abs();

    // Build with thousands separators.
    let formatted = {
        let raw = abs.to_string();
        let mut result = String::with_capacity(raw.len() + raw.len() / 3);
        for (i, ch) in raw.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 {
                result.push_str(thousands_sep);
            }
            result.push(ch);
        }
        result.chars().rev().collect::<String>()
    };

    let sign = if amount < 0 { "-" } else { "" };
    format!("{sign}{currency} {formatted}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_cents_basic() {
        assert_eq!(format_cents(2700000, "$", ".", ","), "$ 2.700.000");
        assert_eq!(format_cents(50, "$", ".", ","), "$ 50");
        assert_eq!(format_cents(0, "$", ".", ","), "$ 0");
        assert_eq!(format_cents(1000, "€", ".", ","), "€ 1.000");
    }

    #[test]
    fn format_cents_us() {
        assert_eq!(format_cents(123456, "$", ",", "."), "$ 123,456");
        assert_eq!(format_cents(0, "$", ",", "."), "$ 0");
    }

    #[test]
    fn format_cents_negative() {
        assert_eq!(format_cents(-2700000, "$", ".", ","), "-$ 2.700.000");
    }

    #[test]
    fn format_cents_large() {
        assert_eq!(format_cents(2700000, "$", ".", ","), "$ 2.700.000");
        assert_eq!(format_cents(1000000, "$", ",", "."), "$ 1,000,000");
    }

    #[test]
    fn transaction_kind_roundtrip() {
        for kind in [TransactionKind::Income, TransactionKind::Expense] {
            let s = kind.to_string();
            let parsed: TransactionKind = s.parse().unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn budget_period_roundtrip() {
        for period in [
            BudgetPeriod::Weekly,
            BudgetPeriod::Monthly,
            BudgetPeriod::Yearly,
        ] {
            let s = period.to_string();
            let parsed: BudgetPeriod = s.parse().unwrap();
            assert_eq!(parsed, period);
        }
    }

    #[test]
    fn recurring_interval_roundtrip() {
        for interval in [
            RecurringInterval::Daily,
            RecurringInterval::Weekly,
            RecurringInterval::Monthly,
            RecurringInterval::Yearly,
        ] {
            let s = interval.to_string();
            let parsed: RecurringInterval = s.parse().unwrap();
            assert_eq!(parsed, interval);
        }
    }

    #[test]
    fn tag_full_name_with_parent() {
        let tag = Tag {
            id: Some(1),
            name: "Pizza".into(),
            parent_id: Some(2),
            icon: None,
        };
        assert_eq!(tag.full_name(Some("Comida")), "Comida > Pizza");
        assert_eq!(tag.full_name(None), "Pizza");
    }

    #[test]
    fn transaction_signed_amount() {
        let mut tx = Transaction {
            id: None,
            source: "test".into(),
            amount: 1000,
            kind: TransactionKind::Income,
            tag_id: 1,
            date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            notes: None,
            created_at: None,
            updated_at: None,
        };
        assert_eq!(tx.signed_amount(), 1000);

        tx.kind = TransactionKind::Expense;
        assert_eq!(tx.signed_amount(), -1000);
    }
}
