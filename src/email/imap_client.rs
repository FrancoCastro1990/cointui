use std::net::TcpStream;

use imap::Session;
use native_tls::TlsStream;

use crate::config::GmailConfig;
use crate::error::{AppError, Result};

/// An email fetched from IMAP with parsed headers and body.
#[derive(Debug, Clone)]
pub struct FetchedEmail {
    pub message_id: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub body_html: Option<String>,
    pub body_text: Option<String>,
}

/// Bank sender patterns used to search INBOX.
const BANK_SENDERS: &[(&str, &str)] = &[
    ("santander", "santander.cl"),
    ("cmr_falabella", "falabella.com"),
    ("scotiabank", "scotiabank.cl"),
];

/// Connect to Gmail IMAP and return an authenticated session.
pub fn connect(config: &GmailConfig) -> Result<Session<TlsStream<TcpStream>>> {
    let password = config.resolve_password().ok_or_else(|| {
        AppError::EmailSync(
            "No Gmail password configured. Set COINTUI_GMAIL_PASSWORD env var or app_password in config.toml".into(),
        )
    })?;

    let tls = native_tls::TlsConnector::builder()
        .build()
        .map_err(|e| AppError::EmailSync(format!("TLS error: {e}")))?;

    let client = imap::connect(
        (config.imap_host.as_str(), config.imap_port),
        &config.imap_host,
        &tls,
    )
    .map_err(|e| AppError::EmailSync(format!("IMAP connection failed: {e}")))?;

    let session = client
        .login(&config.email, &password)
        .map_err(|e| AppError::EmailSync(format!("IMAP login failed: {}", e.0)))?;

    Ok(session)
}

/// Search INBOX for bank notification emails since the given date.
/// Returns `(bank_name, sequence_numbers)` pairs.
pub fn search_bank_emails(
    session: &mut Session<TlsStream<TcpStream>>,
    since_date: &str,
) -> Result<Vec<(String, Vec<u32>)>> {
    session
        .select("INBOX")
        .map_err(|e| AppError::EmailSync(format!("Failed to select INBOX: {e}")))?;

    let mut results = Vec::new();

    for &(bank_name, sender_domain) in BANK_SENDERS {
        let query = format!("FROM \"{}\" SINCE {}", sender_domain, since_date);
        match session.search(&query) {
            Ok(seq_nums) => {
                if !seq_nums.is_empty() {
                    let nums: Vec<u32> = seq_nums.into_iter().collect();
                    results.push((bank_name.to_string(), nums));
                }
            }
            Err(e) => {
                eprintln!("Warning: IMAP search failed for {bank_name}: {e}");
            }
        }
    }

    Ok(results)
}

/// Fetch and parse emails by sequence numbers.
pub fn fetch_emails(
    session: &mut Session<TlsStream<TcpStream>>,
    seq_nums: &[u32],
) -> Result<Vec<FetchedEmail>> {
    if seq_nums.is_empty() {
        return Ok(Vec::new());
    }

    let seq_set: String = seq_nums
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let messages = session
        .fetch(&seq_set, "RFC822")
        .map_err(|e| AppError::EmailSync(format!("IMAP fetch failed: {e}")))?;

    let mut emails = Vec::new();

    for message in messages.iter() {
        if let Some(body) = message.body() {
            match parse_raw_email(body) {
                Ok(email) => emails.push(email),
                Err(e) => {
                    eprintln!("Warning: Failed to parse email: {e}");
                }
            }
        }
    }

    Ok(emails)
}

/// Parse raw RFC822 email bytes into a `FetchedEmail`.
fn parse_raw_email(raw: &[u8]) -> Result<FetchedEmail> {
    let parsed = mailparse::parse_mail(raw)
        .map_err(|e| AppError::EmailSync(format!("Mail parse error: {e}")))?;

    let headers = &parsed.headers;

    let message_id = headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case("Message-ID"))
        .map(|h| h.get_value())
        .unwrap_or_default();

    let from = headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case("From"))
        .map(|h| h.get_value())
        .unwrap_or_default();

    let subject = headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case("Subject"))
        .map(|h| h.get_value())
        .unwrap_or_default();

    let date = headers
        .iter()
        .find(|h| h.get_key().eq_ignore_ascii_case("Date"))
        .map(|h| h.get_value())
        .unwrap_or_default();

    let (body_html, body_text) = extract_bodies(&parsed);

    // Generate a fallback message_id if header is missing.
    let message_id = if message_id.is_empty() {
        format!("generated-{}", hash_bytes(raw))
    } else {
        message_id
    };

    Ok(FetchedEmail {
        message_id,
        from,
        subject,
        date,
        body_html,
        body_text,
    })
}

/// Extract HTML and plain-text bodies from a parsed email.
fn extract_bodies(mail: &mailparse::ParsedMail<'_>) -> (Option<String>, Option<String>) {
    let mut html = None;
    let mut text = None;

    if mail.subparts.is_empty() {
        let content_type = mail.ctype.mimetype.to_lowercase();
        if let Ok(body) = mail.get_body() {
            if content_type.contains("text/html") {
                html = Some(body);
            } else if content_type.contains("text/plain") {
                text = Some(body);
            }
        }
    } else {
        for part in &mail.subparts {
            let (h, t) = extract_bodies(part);
            if html.is_none() {
                html = h;
            }
            if text.is_none() {
                text = t;
            }
        }
    }

    (html, text)
}

/// Simple hash for generating fallback message IDs.
fn hash_bytes(data: &[u8]) -> u64 {
    let mut hash: u64 = 5381;
    for &b in data.iter().take(1024) {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}
