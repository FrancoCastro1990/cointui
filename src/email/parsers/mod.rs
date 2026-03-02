pub mod cmr_falabella;
pub mod santander;
pub mod scotiabank;

use chrono::NaiveDate;

use crate::domain::models::TransactionKind;
use crate::email::imap_client::FetchedEmail;
use crate::error::Result;

/// A transaction extracted from a bank notification email.
#[derive(Debug, Clone)]
pub struct ParsedTransaction {
    /// Merchant or description.
    pub source: String,
    /// Amount in whole currency units, always positive.
    pub amount: i64,
    pub kind: TransactionKind,
    pub date: NaiveDate,
    /// Whether this is an own-account transfer (should be skipped).
    pub is_transfer: bool,
    pub email_subject: String,
    pub notes: Option<String>,
}

/// Trait implemented by each bank parser.
pub trait BankParser {
    fn bank_name(&self) -> &str;
    fn can_parse(&self, email: &FetchedEmail) -> bool;
    fn parse(&self, email: &FetchedEmail) -> Result<Vec<ParsedTransaction>>;
}

/// Return all available bank parsers.
pub fn all_parsers() -> Vec<Box<dyn BankParser>> {
    vec![
        Box::new(santander::SantanderParser),
        Box::new(cmr_falabella::CmrFalabellaParser),
        Box::new(scotiabank::ScotiabankParser),
    ]
}

/// Try to parse an email using the first matching parser.
/// Returns `Ok(Some((bank_name, transactions)))` if parsed, `Ok(None)` if no parser matched.
pub fn parse_email(email: &FetchedEmail) -> Result<Option<(String, Vec<ParsedTransaction>)>> {
    for parser in all_parsers() {
        if parser.can_parse(email) {
            let transactions = parser.parse(email)?;
            return Ok(Some((parser.bank_name().to_string(), transactions)));
        }
    }
    Ok(None)
}

/// Extract text content from HTML using scraper, stripping all tags.
pub(crate) fn html_to_text(html: &str) -> String {
    let document = scraper::Html::parse_document(html);
    let mut text = String::new();
    for node in document.tree.values() {
        if let scraper::node::Node::Text(t) = node {
            text.push_str(&t.text);
            text.push(' ');
        }
    }
    text
}

/// Try to extract an amount from Chilean peso format: `$15.990` or `$ 15.990`.
/// Returns the amount as whole currency units (i64).
pub(crate) fn extract_amount(text: &str) -> Option<i64> {
    let re = regex::Regex::new(r"\$\s*([\d.]+)").ok()?;
    if let Some(caps) = re.captures(text) {
        let raw = caps.get(1)?.as_str().replace('.', "");
        return raw.parse::<i64>().ok();
    }
    None
}

/// Try to extract a date in DD/MM/YYYY or DD-MM-YYYY format.
pub(crate) fn extract_date(text: &str) -> Option<NaiveDate> {
    // Try DD/MM/YYYY first, then DD-MM-YYYY
    let re = regex::Regex::new(r"(\d{2})[/-](\d{2})[/-](\d{4})").ok()?;
    if let Some(caps) = re.captures(text) {
        let day: u32 = caps.get(1)?.as_str().parse().ok()?;
        let month: u32 = caps.get(2)?.as_str().parse().ok()?;
        let year: i32 = caps.get(3)?.as_str().parse().ok()?;
        return NaiveDate::from_ymd_opt(year, month, day);
    }
    None
}

/// Detect own-account transfers by keyword matching.
pub(crate) fn is_own_transfer(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("transferencia entre cuentas propias")
        || lower.contains("traspaso entre cuentas")
        || lower.contains("traspaso a cuenta propia")
        || lower.contains("transferencia propia")
}

/// Detect income transactions by keyword matching.
pub(crate) fn is_income_keyword(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("abono")
        || lower.contains("depósito")
        || lower.contains("deposito")
        || lower.contains("ingreso")
        || lower.contains("transferencia recibida")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_amount() {
        assert_eq!(extract_amount("$15.990"), Some(15990));
        assert_eq!(extract_amount("$ 1.200.000"), Some(1200000));
        assert_eq!(extract_amount("$500"), Some(500));
        assert_eq!(extract_amount("no amount here"), None);
    }

    #[test]
    fn test_extract_date_dmy() {
        let d = extract_date("15/01/2026").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        assert!(extract_date("no date").is_none());
    }

    #[test]
    fn test_extract_date_with_dashes() {
        let d = extract_date("03-03-2024").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2024, 3, 3).unwrap());
    }

    #[test]
    fn test_is_own_transfer() {
        assert!(is_own_transfer("Transferencia entre cuentas propias"));
        assert!(is_own_transfer("TRASPASO ENTRE CUENTAS"));
        assert!(!is_own_transfer("Compra en Supermercado"));
    }

    #[test]
    fn test_is_income_keyword() {
        assert!(is_income_keyword("Abono en cuenta"));
        assert!(is_income_keyword("Depósito recibido"));
        assert!(is_income_keyword("Transferencia recibida"));
        assert!(!is_income_keyword("Compra con tarjeta"));
    }

    #[test]
    fn test_html_to_text() {
        let html = "<html><body><p>Hello</p><b>World</b></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }
}
