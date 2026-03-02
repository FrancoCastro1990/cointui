use chrono::Local;

use crate::domain::models::TransactionKind;
use crate::email::imap_client::FetchedEmail;
use crate::error::Result;

use super::{extract_amount, extract_date, html_to_text, BankParser, ParsedTransaction};

/// Subjects that indicate a real transaction email (case-insensitive).
const TRANSACTION_SUBJECTS: &[&str] = &[
    "comprobante transferencia",
    "comprobante de pago",
    "pago deuda nacional",
    "compra aprobada",
    "cargo en cuenta",
    "abono en cuenta",
];

pub struct SantanderParser;

impl BankParser for SantanderParser {
    fn bank_name(&self) -> &str {
        "santander"
    }

    fn can_parse(&self, email: &FetchedEmail) -> bool {
        if !email.from.to_lowercase().contains("santander.cl") {
            return false;
        }
        // Only parse transaction-related emails, skip marketing.
        let subject_lower = email.subject.to_lowercase();
        TRANSACTION_SUBJECTS
            .iter()
            .any(|s| subject_lower.contains(s))
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

        let date = extract_date(&body).unwrap_or_else(|| Local::now().date_naive());

        let subject_lower = email.subject.to_lowercase();

        // Determine transaction type based on subject and body content.
        let (kind, source, is_transfer) = classify_santander(&subject_lower, &body);

        Ok(vec![ParsedTransaction {
            source,
            amount,
            kind,
            date,
            is_transfer,
            email_subject: email.subject.clone(),
            notes: Some("Auto: email sync (Santander)".to_string()),
        }])
    }
}

/// Classify a Santander email into kind, source, and transfer status.
fn classify_santander(subject: &str, body: &str) -> (TransactionKind, String, bool) {
    let body_lower = body.to_lowercase();

    // "Comprobante Transferencia de fondos"
    if subject.contains("comprobante transferencia") {
        // Incoming: "realizó una transferencia a tu cuenta"
        if body_lower.contains("transferencia a tu cuenta")
            || body_lower.contains("a tu cuenta")
        {
            let sender = extract_transfer_sender(body).unwrap_or("Transferencia recibida".into());
            return (TransactionKind::Income, sender, false);
        }
        // Outgoing: "realizaste una transferencia" or has "Datos de destino" with a different name
        let recipient =
            extract_transfer_recipient(body).unwrap_or("Transferencia enviada".into());
        return (TransactionKind::Expense, recipient, false);
    }

    // "Comprobante de Pago" (bill payment)
    if subject.contains("comprobante de pago") {
        let service = extract_service_name(body).unwrap_or("Pago de servicio".into());
        return (TransactionKind::Expense, service, false);
    }

    // "Pago Deuda Nacional TCR" (credit card payment)
    if subject.contains("pago deuda nacional") {
        let card = extract_card_name(body).unwrap_or("Pago tarjeta de crédito".into());
        return (TransactionKind::Expense, card, false);
    }

    // "Compra aprobada"
    if subject.contains("compra") {
        let merchant = extract_merchant(body).unwrap_or("Compra".into());
        return (TransactionKind::Expense, merchant, false);
    }

    // "Abono en cuenta"
    if subject.contains("abono") {
        return (TransactionKind::Income, "Abono en cuenta".into(), false);
    }

    // "Cargo en cuenta"
    if subject.contains("cargo") {
        return (TransactionKind::Expense, "Cargo en cuenta".into(), false);
    }

    // Default: expense
    (TransactionKind::Expense, "Santander".into(), false)
}

/// Extract sender name from incoming transfer.
/// Pattern: "nuestro cliente NOMBRE realizó"
fn extract_transfer_sender(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?i)(?:nuestro\s+cliente|cliente)\s+([A-ZÁÉÍÓÚÑ][A-ZÁÉÍÓÚÑ\s]+?)\s+(?:realiz|ha realiz)").ok()?;
    if let Some(caps) = re.captures(text) {
        let name = caps.get(1)?.as_str().trim().to_string();
        if !name.is_empty() && name.len() <= 80 {
            return Some(format!("TEF de {name}"));
        }
    }
    None
}

/// Extract recipient name from outgoing transfer.
/// Pattern: "Nombre\nRECIPIENT NAME" in "Datos de destino" section.
fn extract_transfer_recipient(text: &str) -> Option<String> {
    // Look for the recipient name after "Datos de destino" section
    let re =
        regex::Regex::new(r"(?i)(?:datos\s+de\s+destino|destinatario)[\s\S]*?(?:nombre|beneficiario)\s+([A-ZÁÉÍÓÚÑ][A-ZÁÉÍÓÚÑ\s]+?)(?:\s+RUT|\s+Banco|\s+N)")
            .ok()?;
    if let Some(caps) = re.captures(text) {
        let name = caps.get(1)?.as_str().trim().to_string();
        if !name.is_empty() && name.len() <= 80 {
            return Some(format!("TEF a {name}"));
        }
    }
    None
}

/// Extract service name from "Comprobante de Pago".
/// Pattern: "Servicio:\nWomPagofacil"
fn extract_service_name(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?i)servicio[:\s]+([^\n]+)").ok()?;
    if let Some(caps) = re.captures(text) {
        let service = caps.get(1)?.as_str().trim().to_string();
        if !service.is_empty() && service.len() <= 80 {
            return Some(format!("Pago {service}"));
        }
    }
    None
}

/// Extract card name from "Pago Deuda Nacional".
/// Pattern: "VISA PLATINUM LATAM" after "Tarjeta:"
fn extract_card_name(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?i)(?:tarjeta|destino)[:\s]+([A-ZÁÉÍÓÚÑ][A-ZÁÉÍÓÚÑ\s*]+)").ok()?;
    if let Some(caps) = re.captures(text) {
        let card = caps.get(1)?.as_str().trim().to_string();
        if !card.is_empty() && card.len() <= 80 && !card.contains("****") {
            return Some(format!("Pago TC {card}"));
        }
    }
    Some("Pago tarjeta de crédito".into())
}

/// Extract merchant from purchase notification.
fn extract_merchant(text: &str) -> Option<String> {
    let re =
        regex::Regex::new(r"(?i)comercio[:\s]+([A-ZÁÉÍÓÚÑ\w][\w\s&'./-]{2,60})").ok()?;
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
    fn skips_marketing_email() {
        let parser = SantanderParser;
        let email = make_email(
            "alertas@santander.cl",
            "Llegó el nuevo Samsung Galaxy S26",
            "<html><body><p>$3.000.000 OFF</p></body></html>",
        );
        assert!(!parser.can_parse(&email));
    }

    #[test]
    fn parses_incoming_transfer() {
        let html = r#"<html><body>
            <p>Comprobante Transferencia de fondos</p>
            <p>Estimado(a) FRANCO:</p>
            <p>Te informamos que, con fecha 15/01/2026, nuestro cliente KARINA ANDREA OLIVERO MARTINEZ realizó una transferencia a tu cuenta.</p>
            <p>Monto transferido $ 1.320</p>
        </body></html>"#;

        let parser = SantanderParser;
        let email = make_email("alertas@santander.cl", "Comprobante Transferencia de fondos", html);
        assert!(parser.can_parse(&email));

        let txs = parser.parse(&email).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].amount, 1320);
        assert_eq!(txs[0].kind, TransactionKind::Income);
        assert!(txs[0].source.contains("KARINA ANDREA OLIVERO MARTINEZ"));
    }

    #[test]
    fn parses_outgoing_transfer() {
        let html = r#"<html><body>
            <p>Comprobante Transferencia de fondos</p>
            <p>Estimado(a) FRANCO: realizaste una transferencia.</p>
            <p>Monto transferido $ 50.000</p>
            <p>Datos de destino</p>
            <p>Nombre JUAN PEREZ GONZALEZ</p>
            <p>RUT 12.345.678-9</p>
            <p>Banco Banco Estado</p>
            <p>Fecha: 20/01/2026</p>
        </body></html>"#;

        let parser = SantanderParser;
        let email = make_email("alertas@santander.cl", "Comprobante Transferencia de fondos", html);
        let txs = parser.parse(&email).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].kind, TransactionKind::Expense);
        assert!(txs[0].source.contains("JUAN PEREZ GONZALEZ"));
    }

    #[test]
    fn parses_bill_payment() {
        let html = r#"<html><body>
            <p>Comprobante de Pago</p>
            <p>El pago se ha realizado con éxito</p>
            <p>Con fecha 03-03-2024</p>
            <p>$15.995</p>
            <p>Servicio: WomPagofacil</p>
        </body></html>"#;

        let parser = SantanderParser;
        let email = make_email("alertas@santander.cl", "Comprobante de Pago", html);
        let txs = parser.parse(&email).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].amount, 15995);
        assert_eq!(txs[0].kind, TransactionKind::Expense);
        assert!(txs[0].source.contains("WomPagofacil"));
    }

    #[test]
    fn parses_credit_card_payment() {
        let html = r#"<html><body>
            <p>Pago Deuda Nacional de Tarjeta de Credito</p>
            <p>Tu pago de Tarjeta de Credito ha sido realizado con exito.</p>
            <p>Fecha 13/07/2024</p>
            <p>Monto del pago: $267.634</p>
            <p>Tarjeta: VISA PLATINUM LATAM</p>
        </body></html>"#;

        let parser = SantanderParser;
        let email = make_email("alertas@santander.cl", "Pago Deuda Nacional TCR", html);
        let txs = parser.parse(&email).unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].amount, 267634);
        assert_eq!(txs[0].kind, TransactionKind::Expense);
        assert!(txs[0].source.contains("Pago"));
    }

    #[test]
    fn skips_zero_amount() {
        let html = r#"<html><body>
            <p>Comprobante de Pago</p>
            <p>$0</p>
        </body></html>"#;

        let parser = SantanderParser;
        let email = make_email("alertas@santander.cl", "Comprobante de Pago", html);
        let txs = parser.parse(&email).unwrap();
        assert!(txs.is_empty());
    }

    #[test]
    fn does_not_parse_other_banks() {
        let email = make_email("info@other-bank.com", "Comprobante de Pago", "<html></html>");
        let parser = SantanderParser;
        assert!(!parser.can_parse(&email));
    }

    #[test]
    fn parses_date_with_dashes() {
        let html = r#"<html><body>
            <p>Comprobante de Pago</p>
            <p>Con fecha 03-03-2024</p>
            <p>$10.000</p>
            <p>Servicio: Test</p>
        </body></html>"#;

        let parser = SantanderParser;
        let email = make_email("alertas@santander.cl", "Comprobante de Pago", html);
        let txs = parser.parse(&email).unwrap();
        assert_eq!(txs[0].date.to_string(), "2024-03-03");
    }
}
