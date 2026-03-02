use crate::config::AppConfig;
use crate::db::connection::Database;
use crate::email::sync;
use crate::error::Result;

pub fn run(db: &Database, config: &AppConfig) -> Result<()> {
    println!("Syncing Gmail emails...");

    let result = sync::sync_emails(db, config)?;

    println!("Email sync complete:");
    println!("  Emails found:    {}", result.emails_found);
    println!("  Imported:        {}", result.imported);
    println!("  Skipped (dup):   {}", result.skipped_duplicate);
    println!("  Skipped (xfer):  {}", result.skipped_transfer);
    println!("  Skipped (rule):  {}", result.skipped_rule);
    println!("  Skipped (error): {}", result.skipped_parse_error);

    Ok(())
}
