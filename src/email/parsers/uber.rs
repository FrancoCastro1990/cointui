use chrono::Local;
use regex::Regex;

use crate::domain::models::TransactionKind;
use crate::email::imap_client::FetchedEmail;
use crate::error::Result;

use super::{extract_amount, extract_date, html_to_text, parse_header_date, BankParser, ParsedTransaction};

pub struct UberParser;

impl BankParser for UberParser {
    fn bank_name(&self) -> &str {
        "uber"
    }

    fn can_parse(&self, email: &FetchedEmail) -> bool {
        email.from.to_lowercase().contains("uber.com")
    }

    fn dedup_by_content(&self) -> bool {
        true
    }

    fn parse(&self, email: &FetchedEmail) -> Result<Vec<ParsedTransaction>> {
        let body = match &email.body_html {
            Some(html) => html_to_text(html),
            None => match &email.body_text {
                Some(t) => t.clone(),
                None => return Ok(Vec::new()),
            },
        };

        let amount = extract_total(&body)
            .or_else(|| extract_amount(&body))
            .unwrap_or(0);

        if amount <= 0 {
            return Ok(Vec::new());
        }

        let date = extract_date(&body)
            .or_else(|| parse_header_date(&email.date))
            .unwrap_or_else(|| Local::now().date_naive());

        let body_lower = body.to_lowercase();
        let is_eats = body_lower.contains("uber eats")
            || body_lower.contains("restaurant")
            || body_lower.contains("pedido")
            || body_lower.contains("delivery fee")
            || body_lower.contains("tarifa de entrega");

        let source = if is_eats {
            "Uber Eats".to_string()
        } else {
            "Uber".to_string()
        };

        Ok(vec![ParsedTransaction {
            source,
            amount,
            kind: TransactionKind::Expense,
            date,
            is_transfer: false,
            email_subject: email.subject.clone(),
            notes: Some("Auto: email sync (Uber)".to_string()),
        }])
    }
}

/// Extract the TOTAL amount specifically, not subtotals.
fn extract_total(text: &str) -> Option<i64> {
    let re = Regex::new(r"(?i)total\s*\$\s*([\d.]+)").ok()?;
    // Take the last match — "Total" comes after "Subtotal" in receipts.
    let mut last: Option<i64> = None;
    for caps in re.captures_iter(text) {
        if let Some(m) = caps.get(1) {
            let raw = m.as_str().replace('.', "");
            if let Ok(val) = raw.parse::<i64>()
                && val > 0
            {
                last = Some(val);
            }
        }
    }
    last
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
    fn can_parse_uber_email() {
        let parser = UberParser;
        let email = make_email("noreply@uber.com", "Tu recibo de Uber", "<html></html>");
        assert!(parser.can_parse(&email));
    }

    #[test]
    fn cannot_parse_non_uber() {
        let parser = UberParser;
        let email = make_email("info@other.com", "Some email", "<html></html>");
        assert!(!parser.can_parse(&email));
    }

    #[test]
    fn parses_uber_ride() {
        let html = r#"<html><body>
            <p>Gracias por viajar con Uber</p>
            <p>Fecha: 15/01/2026</p>
            <p>Subtotal $3.500</p>
            <p>Total $4.200</p>
        </body></html>"#;

        let parser = UberParser;
        let email = make_email("noreply@uber.com", "Tu recibo de Uber", html);
        let txs = parser.parse(&email).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].amount, 4200);
        assert_eq!(txs[0].source, "Uber");
        assert_eq!(txs[0].kind, TransactionKind::Expense);
    }

    #[test]
    fn parses_uber_eats() {
        let html = r#"<html><body>
            <p>Tu pedido de Uber Eats</p>
            <p>Restaurant: Sushi Place</p>
            <p>Delivery fee $1.500</p>
            <p>Total $12.990</p>
        </body></html>"#;

        let parser = UberParser;
        let email = make_email("noreply@uber.com", "Tu pedido de Uber Eats", html);
        let txs = parser.parse(&email).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].amount, 12990);
        assert_eq!(txs[0].source, "Uber Eats");
    }

    #[test]
    fn skips_zero_amount() {
        let html = "<html><body><p>Uber promo $0</p></body></html>";
        let parser = UberParser;
        let email = make_email("noreply@uber.com", "Promo", html);
        let txs = parser.parse(&email).unwrap();
        assert!(txs.is_empty());
    }
}
