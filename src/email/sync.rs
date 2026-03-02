use chrono::Local;

use crate::ai::ollama::OllamaClient;
use crate::ai::prompts::{build_ai_rules_prompt, build_tag_assignment_prompt, AiRulesData};
use crate::config::AppConfig;
use crate::db::connection::Database;
use crate::db::email_repo::EmailRepo;
use crate::db::tag_repo::TagRepo;
use crate::db::transaction_repo::TransactionRepo;
use crate::domain::models::Transaction;
use crate::email::imap_client::FetchedEmail;
use crate::email::parsers::{self, ParsedTransaction};
use crate::error::{AppError, Result};

/// Summary of a sync operation.
#[derive(Debug, Default)]
pub struct SyncResult {
    pub emails_found: usize,
    pub imported: usize,
    pub skipped_duplicate: usize,
    pub skipped_transfer: usize,
    pub skipped_parse_error: usize,
    pub skipped_rule: usize,
}

impl std::fmt::Display for SyncResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Emails found: {}, Imported: {}, Duplicates: {}, Transfers: {}, Rules: {}, Parse errors: {}",
            self.emails_found,
            self.imported,
            self.skipped_duplicate,
            self.skipped_transfer,
            self.skipped_rule,
            self.skipped_parse_error,
        )
    }
}

/// Result of tag assignment: either a tag ID or a decision to skip.
enum TagAssignment {
    Assigned(i64),
    Skip,
}

/// Per-account sync result with error isolation.
pub struct AccountSyncResult {
    pub email: String,
    pub result: std::result::Result<SyncResult, AppError>,
}

/// Full sync across all configured Gmail accounts.
/// Each account is processed independently; one failure does not block others.
pub fn sync_all_accounts(db: &Database, config: &AppConfig) -> Result<Vec<AccountSyncResult>> {
    if !config.gmail.enabled {
        return Err(AppError::EmailSync(
            "Gmail sync is not enabled. Set [gmail] enabled = true in config.toml".into(),
        ));
    }

    if config.gmail.accounts.is_empty() {
        return Err(AppError::EmailSync(
            "No Gmail accounts configured. Add [[gmail.accounts]] entries in config.toml".into(),
        ));
    }

    let since = Local::now()
        .date_naive()
        .checked_sub_signed(chrono::Duration::days(config.gmail.lookback_days as i64))
        .unwrap_or(Local::now().date_naive());
    let since_str = since.format("%d-%b-%Y").to_string();

    let mut results = Vec::new();

    for account in &config.gmail.accounts {
        let account_result = sync_single_account(db, config, account, &since_str);
        results.push(AccountSyncResult {
            email: account.email.clone(),
            result: account_result,
        });
    }

    Ok(results)
}

/// Sync a single Gmail account.
fn sync_single_account(
    db: &Database,
    config: &AppConfig,
    account: &crate::config::GmailAccount,
    since_str: &str,
) -> std::result::Result<SyncResult, AppError> {
    let mut session = super::imap_client::connect(
        &config.gmail.imap_host,
        config.gmail.imap_port,
        &account.email,
        &account.app_password,
    )?;

    let bank_results = super::imap_client::search_bank_emails(&mut session, since_str)?;

    let mut all_emails: Vec<(String, FetchedEmail)> = Vec::new();
    for (bank_name, seq_nums) in &bank_results {
        let emails = super::imap_client::fetch_emails(&mut session, seq_nums)?;
        for email in emails {
            all_emails.push((bank_name.clone(), email));
        }
    }

    let _ = session.logout();

    process_fetched_emails(db, config, &all_emails, &account.email)
}

/// Process already-fetched emails. This is the testable core that doesn't require IMAP.
pub fn process_fetched_emails(
    db: &Database,
    config: &AppConfig,
    emails: &[(String, FetchedEmail)],
    account_email: &str,
) -> Result<SyncResult> {
    let email_repo = EmailRepo::new(db);
    let tx_repo = TransactionRepo::new(db);
    let tag_repo = TagRepo::new(db);

    let tags = tag_repo.get_all()?;
    let tag_names: Vec<String> = tags.iter().map(|t| t.name.clone()).collect();

    let ollama = if !config.gmail.rules_prompt.is_empty() || config.gmail.ai_tag_fallback {
        OllamaClient::from_config(&config.ai)
    } else {
        None
    };

    let mut result = SyncResult {
        emails_found: emails.len(),
        ..Default::default()
    };

    for (_bank_hint, email) in emails {
        // Dedup check.
        if email_repo.is_processed(&email.message_id)? {
            result.skipped_duplicate += 1;
            continue;
        }

        // Parse the email.
        let parsed = match parsers::parse_email(email) {
            Ok(Some(pr)) => Some(pr),
            Ok(None) => None,
            Err(_) => {
                result.skipped_parse_error += 1;
                email_repo.record(
                    &email.message_id,
                    _bank_hint,
                    Some(&email.subject),
                    Some(&email.date),
                    "skipped_error",
                    None,
                    account_email,
                )?;
                continue;
            }
        };

        let parse_result = match parsed {
            Some(pr) if !pr.transactions.is_empty() => pr,
            _ => {
                result.skipped_parse_error += 1;
                email_repo.record(
                    &email.message_id,
                    _bank_hint,
                    Some(&email.subject),
                    Some(&email.date),
                    "skipped_error",
                    None,
                    account_email,
                )?;
                continue;
            }
        };

        let bank = parse_result.bank_name;
        let dedup = parse_result.dedup_by_content;

        for parsed_tx in &parse_result.transactions {
            // Skip own-account transfers.
            if parsed_tx.is_transfer {
                result.skipped_transfer += 1;
                email_repo.record(
                    &email.message_id,
                    &bank,
                    Some(&email.subject),
                    Some(&email.date),
                    "skipped_transfer",
                    None,
                    account_email,
                )?;
                continue;
            }

            // Assign tag (or skip via AI rules).
            let tag_id = match assign_tag(parsed_tx, config, &tags, &tag_names, &ollama) {
                TagAssignment::Skip => {
                    result.skipped_rule += 1;
                    email_repo.record(
                        &email.message_id,
                        &bank,
                        Some(&email.subject),
                        Some(&email.date),
                        "skipped_rule",
                        None,
                        account_email,
                    )?;
                    continue;
                }
                TagAssignment::Assigned(id) => id,
            };

            // Content-based dedup: skip if same source + amount + date already exists.
            // Only applies to parsers that opt in (e.g. Uber sends duplicate emails per trip).
            if dedup && tx_repo.exists_by_content(&parsed_tx.source, parsed_tx.amount, &parsed_tx.date)? {
                result.skipped_duplicate += 1;
                email_repo.record(
                    &email.message_id,
                    &bank,
                    Some(&email.subject),
                    Some(&email.date),
                    "skipped_duplicate",
                    None,
                    account_email,
                )?;
                continue;
            }

            // Create transaction.
            let tx = Transaction {
                id: None,
                source: parsed_tx.source.clone(),
                amount: parsed_tx.amount,
                kind: parsed_tx.kind,
                tag_id,
                date: parsed_tx.date,
                notes: parsed_tx.notes.clone(),
                created_at: None,
                updated_at: None,
            };

            match tx_repo.create(&tx) {
                Ok(tx_id) => {
                    email_repo.record(
                        &email.message_id,
                        &bank,
                        Some(&email.subject),
                        Some(&email.date),
                        "imported",
                        Some(tx_id),
                        account_email,
                    )?;
                    result.imported += 1;
                }
                Err(e) => {
                    eprintln!("Warning: Failed to create transaction: {e}");
                    result.skipped_parse_error += 1;
                    email_repo.record(
                        &email.message_id,
                        &bank,
                        Some(&email.subject),
                        Some(&email.date),
                        "skipped_error",
                        None,
                        account_email,
                    )?;
                }
            }
        }
    }

    Ok(result)
}

/// Parse an AI response into a `TagAssignment`.
///
/// Returns `Some(Skip)` for "SKIP", `Some(Assigned(id))` if the response
/// matches a known tag, or `None` if the response is unrecognizable.
fn parse_ai_response(
    response: &str,
    tags: &[crate::domain::models::Tag],
) -> Option<TagAssignment> {
    let cleaned = response.trim().trim_matches('"');
    if cleaned.eq_ignore_ascii_case("SKIP") {
        return Some(TagAssignment::Skip);
    }
    tags.iter()
        .find(|t| t.name.eq_ignore_ascii_case(cleaned))
        .and_then(|t| t.id)
        .map(TagAssignment::Assigned)
}

/// Assign a tag to a parsed transaction using AI rules, keyword rules, AI fallback, or default.
fn assign_tag(
    tx: &ParsedTransaction,
    config: &AppConfig,
    tags: &[crate::domain::models::Tag],
    tag_names: &[String],
    ollama: &Option<OllamaClient>,
) -> TagAssignment {
    // 1. AI rules prompt (highest priority when configured).
    if !config.gmail.rules_prompt.is_empty()
        && let Some(client) = ollama
    {
        let prompt = build_ai_rules_prompt(&AiRulesData {
            rules: &config.gmail.rules_prompt,
            source: &tx.source,
            amount: tx.amount,
            kind: &tx.kind.to_string(),
            date: &tx.date.to_string(),
            email_subject: &tx.email_subject,
            tag_names,
            currency: &config.currency,
            tsep: &config.thousands_separator,
            dsep: &config.decimal_separator,
        });
        if let Ok(response) = client.generate(&prompt)
            && let Some(assignment) = parse_ai_response(&response, tags)
        {
            return assignment;
        }
    }

    // 2. Keyword rule-based matching.
    let combined = format!("{} {}", tx.source, tx.email_subject).to_lowercase();
    for rule in &config.gmail.tag_rules {
        if combined.contains(&rule.keyword.to_lowercase())
            && let Some(tag) = tags.iter().find(|t| t.name.eq_ignore_ascii_case(&rule.tag))
            && let Some(id) = tag.id
        {
            return TagAssignment::Assigned(id);
        }
    }

    // 3. Basic AI fallback (no rules_prompt, just source + amount).
    if config.gmail.ai_tag_fallback
        && let Some(client) = ollama
    {
        let prompt = build_tag_assignment_prompt(&tx.source, tx.amount, tag_names);
        if let Ok(response) = client.generate(&prompt)
            && let Some(TagAssignment::Assigned(id)) = parse_ai_response(&response, tags)
        {
            return TagAssignment::Assigned(id);
        }
    }

    // 4. Default: first available tag.
    let id = tags.first().and_then(|t| t.id).unwrap_or(1);
    TagAssignment::Assigned(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Database, AppConfig) {
        let db = Database::in_memory().unwrap();
        let tag_repo = TagRepo::new(&db);
        tag_repo.seed_defaults(&["Other".into(), "Food".into(), "Transport".into()]).unwrap();
        let config = AppConfig::default();
        (db, config)
    }

    #[test]
    fn process_empty_list() {
        let (db, config) = setup();
        let result = process_fetched_emails(&db, &config, &[], "").unwrap();
        assert_eq!(result.emails_found, 0);
        assert_eq!(result.imported, 0);
    }

    #[test]
    fn process_valid_email() {
        let (db, config) = setup();
        let email = FetchedEmail {
            message_id: "test-001@gmail.com".into(),
            from: "alertas@santander.cl".into(),
            subject: "Compra aprobada".into(),
            date: "Mon, 15 Jan 2026 10:00:00 -0300".into(),
            body_html: Some(r#"<html><body>
                <p>Compra en SUPERMERCADO</p>
                <p>Monto: $15.990</p>
                <p>Fecha: 15/01/2026</p>
            </body></html>"#.into()),
            body_text: None,
        };

        let emails = vec![("santander".to_string(), email)];
        let result = process_fetched_emails(&db, &config, &emails, "").unwrap();
        assert_eq!(result.emails_found, 1);
        assert_eq!(result.imported, 1);
    }

    #[test]
    fn dedup_skips_duplicate() {
        let (db, config) = setup();
        let email = FetchedEmail {
            message_id: "dup-001@gmail.com".into(),
            from: "alertas@santander.cl".into(),
            subject: "Compra aprobada".into(),
            date: "Mon, 15 Jan 2026 10:00:00 -0300".into(),
            body_html: Some(r#"<html><body>
                <p>Compra en TIENDA</p>
                <p>Monto: $5.000</p>
                <p>Fecha: 15/01/2026</p>
            </body></html>"#.into()),
            body_text: None,
        };

        let emails = vec![("santander".to_string(), email.clone())];
        let r1 = process_fetched_emails(&db, &config, &emails, "").unwrap();
        assert_eq!(r1.imported, 1);

        let r2 = process_fetched_emails(&db, &config, &emails, "").unwrap();
        assert_eq!(r2.imported, 0);
        assert_eq!(r2.skipped_duplicate, 1);
    }

    #[test]
    fn skips_own_transfer() {
        let (db, config) = setup();
        // Use Scotiabank parser which uses is_own_transfer() detection.
        let email = FetchedEmail {
            message_id: "transfer-001@gmail.com".into(),
            from: "alertas@scotiabank.cl".into(),
            subject: "Aviso de Transferencia".into(),
            date: "Mon, 15 Jan 2026 10:00:00 -0300".into(),
            body_html: Some(r#"<html><body>
                <p>Transferencia entre cuentas propias</p>
                <p>Monto: $100.000</p>
                <p>Fecha: 15/01/2026</p>
            </body></html>"#.into()),
            body_text: None,
        };

        let emails = vec![("scotiabank".to_string(), email)];
        let result = process_fetched_emails(&db, &config, &emails, "").unwrap();
        assert_eq!(result.skipped_transfer, 1);
        assert_eq!(result.imported, 0);
    }

    #[test]
    fn tag_rule_matching() {
        let (db, mut config) = setup();
        config.gmail.tag_rules = vec![
            crate::config::TagRule {
                keyword: "supermercado".into(),
                tag: "Food".into(),
            },
        ];

        let email = FetchedEmail {
            message_id: "rule-001@gmail.com".into(),
            from: "alertas@santander.cl".into(),
            subject: "Compra aprobada".into(),
            date: "Mon, 15 Jan 2026 10:00:00 -0300".into(),
            body_html: Some(r#"<html><body>
                <p>Compra aprobada</p>
                <p>Comercio: SUPERMERCADO LIDER</p>
                <p>Monto: $15.990</p>
                <p>Fecha: 15/01/2026</p>
            </body></html>"#.into()),
            body_text: None,
        };

        let emails = vec![("santander".to_string(), email)];
        let result = process_fetched_emails(&db, &config, &emails, "").unwrap();
        assert_eq!(result.imported, 1);

        // Verify the transaction was assigned the Food tag.
        let tx_repo = TransactionRepo::new(&db);
        let txs = tx_repo.get_all().unwrap();
        assert_eq!(txs.len(), 1);
        let tag_repo = TagRepo::new(&db);
        let food_tag = tag_repo.find_by_name("Food").unwrap().unwrap();
        assert_eq!(txs[0].tag_id, food_tag.id.unwrap());
    }

    fn make_tag(id: i64, name: &str) -> crate::domain::models::Tag {
        crate::domain::models::Tag {
            id: Some(id),
            name: name.into(),
            parent_id: None,
            icon: None,
        }
    }

    #[test]
    fn parse_ai_response_skip() {
        let tags = vec![make_tag(1, "Food")];
        let result = parse_ai_response("SKIP", &tags);
        assert!(matches!(result, Some(TagAssignment::Skip)));
        // Case insensitive
        let result2 = parse_ai_response("skip", &tags);
        assert!(matches!(result2, Some(TagAssignment::Skip)));
        // With quotes
        let result3 = parse_ai_response("\"SKIP\"", &tags);
        assert!(matches!(result3, Some(TagAssignment::Skip)));
    }

    #[test]
    fn parse_ai_response_valid_tag() {
        let tags = vec![make_tag(1, "Food"), make_tag(2, "Transport")];
        let result = parse_ai_response("Food", &tags);
        assert!(matches!(result, Some(TagAssignment::Assigned(1))));
        // Case insensitive
        let result2 = parse_ai_response("  transport  ", &tags);
        assert!(matches!(result2, Some(TagAssignment::Assigned(2))));
    }

    #[test]
    fn parse_ai_response_invalid() {
        let tags = vec![make_tag(1, "Food")];
        let result = parse_ai_response("Unknown tag that doesn't exist", &tags);
        assert!(result.is_none());
    }

    #[test]
    fn build_ai_rules_prompt_contains_data() {
        use crate::ai::prompts::{build_ai_rules_prompt, AiRulesData};

        let rules = "- Compras en supermercados → tag \"Food\"";
        let tag_names = vec!["Food".into(), "Other".into()];
        let prompt = build_ai_rules_prompt(&AiRulesData {
            rules,
            source: "SUPERMERCADO LIDER",
            amount: 15990,
            kind: "expense",
            date: "2026-01-15",
            email_subject: "Compra aprobada",
            tag_names: &tag_names,
            currency: "$",
            tsep: ".",
            dsep: ",",
        });
        assert!(prompt.contains(rules));
        assert!(prompt.contains("SUPERMERCADO LIDER"));
        assert!(prompt.contains("$ 15.990"));
        assert!(prompt.contains("expense"));
        assert!(prompt.contains("2026-01-15"));
        assert!(prompt.contains("Compra aprobada"));
        assert!(prompt.contains("Food, Other"));
        assert!(prompt.contains("SKIP"));
    }
}
