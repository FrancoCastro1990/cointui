use chrono::Local;

use crate::domain::models::TransactionKind;
use crate::email::imap_client::FetchedEmail;
use crate::error::Result;

use super::{
    extract_amount, extract_date, html_to_text, is_income_keyword, is_own_transfer,
    parse_header_date, BankParser, ParsedTransaction,
};

/// Subjects that indicate a real transaction email.
const TRANSACTION_SUBJECTS: &[&str] = &[
    "compra",
    "pago",
    "cargo",
    "abono",
    "transferencia",
    "comprobante",
    "avance",
];

pub struct CmrFalabellaParser;

impl BankParser for CmrFalabellaParser {
    fn bank_name(&self) -> &str {
        "cmr_falabella"
    }

    fn can_parse(&self, email: &FetchedEmail) -> bool {
        let from_lower = email.from.to_lowercase();
        if !from_lower.contains("falabella.com") && !from_lower.contains("cmrfalabella.com") {
            return false;
        }
        let subject_lower = email.subject.to_lowercase();
        TRANSACTION_SUBJECTS.iter().any(|s| subject_lower.contains(s))
    }

    fn parse(&self, email: &FetchedEmail) -> Result<Vec<ParsedTransaction>> {
        let body = match &email.body_html {
            Some(html) => html_to_text(html),
            None => match &email.body_text {
                Some(t) => t.clone(),
                None => return Ok(Vec::new()),
            },
        };

        let amount = match extract_amount(&body) {
            Some(a) if a > 0 => a,
            _ => return Ok(Vec::new()),
        };

        let date = extract_date(&body)
            .or_else(|| parse_header_date(&email.date))
            .unwrap_or_else(|| Local::now().date_naive());

        let is_transfer = is_own_transfer(&body);

        let kind = if is_income_keyword(&body) {
            TransactionKind::Income
        } else {
            TransactionKind::Expense
        };

        let source = extract_merchant(&body)
            .unwrap_or_else(|| "CMR Falabella".to_string());

        Ok(vec![ParsedTransaction {
            source,
            amount,
            kind,
            date,
            is_transfer,
            email_subject: email.subject.clone(),
            notes: Some("Auto: email sync (CMR Falabella)".to_string()),
        }])
    }
}

fn extract_merchant(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?i)comercio[:\s]+([A-ZÁÉÍÓÚÑ\w][\w\s&'./-]{2,60})").ok()?;
    if let Some(caps) = re.captures(text) {
        let merchant = caps.get(1)?.as_str().trim().to_string();
        if !merchant.is_empty() {
            return Some(merchant);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_email(from: &str, subject: &str, html: &str) -> FetchedEmail {
        FetchedEmail {
            message_id: "test@msg".into(),
            from: from.into(),
            subject: subject.into(),
            date: "Mon, 15 Jan 2026 10:00:00 -0300".into(),
            body_html: Some(html.into()),
            body_text: None,
        }
    }

    #[test]
    fn parses_cmr_purchase() {
        let html = r#"<html><body>
            <p>Compra aprobada</p>
            <p>Comercio: FALABELLA RETAIL</p>
            <p>Monto: $29.990</p>
            <p>Fecha: 20/01/2026</p>
        </body></html>"#;

        let parser = CmrFalabellaParser;
        let email = make_email("alertas@cmrfalabella.com", "Compra aprobada", html);
        assert!(parser.can_parse(&email));

        let txs = parser.parse(&email).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].amount, 29990);
        assert_eq!(txs[0].kind, TransactionKind::Expense);
    }

    #[test]
    fn skips_marketing_email() {
        let email = make_email("no-reply@falabella.com", "Grandes ofertas!", "<html>$1.000</html>");
        let parser = CmrFalabellaParser;
        assert!(!parser.can_parse(&email));
    }

    #[test]
    fn does_not_parse_other_banks() {
        let email = make_email("info@santander.cl", "Compra", "<html></html>");
        let parser = CmrFalabellaParser;
        assert!(!parser.can_parse(&email));
    }
}
