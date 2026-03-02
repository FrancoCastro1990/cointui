use chrono::Local;
use regex::Regex;

use crate::domain::models::TransactionKind;
use crate::email::imap_client::FetchedEmail;
use crate::error::Result;

use super::{extract_amount, extract_date, html_to_text, parse_header_date, BankParser, ParsedTransaction};

pub struct PedidosYaParser;

impl BankParser for PedidosYaParser {
    fn bank_name(&self) -> &str {
        "pedidosya"
    }

    fn can_parse(&self, email: &FetchedEmail) -> bool {
        email.from.to_lowercase().contains("pedidosya.com")
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

        Ok(vec![ParsedTransaction {
            source: "PedidosYa".to_string(),
            amount,
            kind: TransactionKind::Expense,
            date,
            is_transfer: false,
            email_subject: email.subject.clone(),
            notes: Some("Auto: email sync (PedidosYa)".to_string()),
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
    fn can_parse_pedidosya_email() {
        let parser = PedidosYaParser;
        let email = make_email(
            "confirmacion@pedidosya.com",
            "Tu pedido está confirmado",
            "<html></html>",
        );
        assert!(parser.can_parse(&email));
    }

    #[test]
    fn cannot_parse_non_pedidosya() {
        let parser = PedidosYaParser;
        let email = make_email("info@other.com", "Some email", "<html></html>");
        assert!(!parser.can_parse(&email));
    }

    #[test]
    fn parses_total() {
        let html = r#"<html><body>
            <p>Tu pedido ha sido confirmado</p>
            <p>Subtotal $8.990</p>
            <p>Envío $1.500</p>
            <p>Total $10.490</p>
            <p>Fecha: 20/01/2026</p>
        </body></html>"#;

        let parser = PedidosYaParser;
        let email = make_email(
            "confirmacion@pedidosya.com",
            "Confirmación de pedido",
            html,
        );
        let txs = parser.parse(&email).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].amount, 10490);
        assert_eq!(txs[0].source, "PedidosYa");
        assert_eq!(txs[0].kind, TransactionKind::Expense);
    }

    #[test]
    fn skips_no_amount() {
        let html = "<html><body><p>Gracias por tu pedido</p></body></html>";
        let parser = PedidosYaParser;
        let email = make_email("confirmacion@pedidosya.com", "Pedido", html);
        let txs = parser.parse(&email).unwrap();
        assert!(txs.is_empty());
    }
}
