use chrono::Local;

use crate::domain::models::TransactionKind;
use crate::email::imap_client::FetchedEmail;
use crate::error::Result;

use super::{
    extract_amount, extract_date, html_to_text, is_income_keyword, is_own_transfer,
    BankParser, ParsedTransaction,
};

/// Subjects that indicate a real transaction email.
const TRANSACTION_SUBJECTS: &[&str] = &[
    "aviso de transferencia",
    "compra",
    "pago",
    "cargo",
    "abono",
    "comprobante",
];

pub struct ScotiabankParser;

impl BankParser for ScotiabankParser {
    fn bank_name(&self) -> &str {
        "scotiabank"
    }

    fn can_parse(&self, email: &FetchedEmail) -> bool {
        if !email.from.to_lowercase().contains("scotiabank.cl") {
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
            .unwrap_or_else(|| Local::now().date_naive());

        let is_transfer = is_own_transfer(&body);

        let kind = if is_income_keyword(&body) {
            TransactionKind::Income
        } else {
            TransactionKind::Expense
        };

        let source = extract_merchant(&body)
            .unwrap_or_else(|| "Scotiabank".to_string());

        Ok(vec![ParsedTransaction {
            source,
            amount,
            kind,
            date,
            is_transfer,
            email_subject: email.subject.clone(),
            notes: Some("Auto: email sync (Scotiabank)".to_string()),
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
    fn parses_scotiabank_purchase() {
        let html = r#"<html><body>
            <p>Compra aprobada</p>
            <p>Comercio: FARMACIA CRUZ VERDE</p>
            <p>Monto: $8.500</p>
            <p>Fecha: 25/01/2026</p>
        </body></html>"#;

        let parser = ScotiabankParser;
        let email = make_email("alertas@scotiabank.cl", "Compra aprobada", html);
        assert!(parser.can_parse(&email));

        let txs = parser.parse(&email).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].amount, 8500);
        assert_eq!(txs[0].kind, TransactionKind::Expense);
    }

    #[test]
    fn skips_marketing_email() {
        let email = make_email("promo@scotiabank.cl", "Ofertas de verano", "<html>$10.000</html>");
        let parser = ScotiabankParser;
        assert!(!parser.can_parse(&email));
    }

    #[test]
    fn does_not_parse_other_banks() {
        let email = make_email("info@santander.cl", "Aviso de Transferencia", "<html></html>");
        let parser = ScotiabankParser;
        assert!(!parser.can_parse(&email));
    }
}
