use crate::config::AppConfig;
use crate::db::connection::Database;
use crate::email::sync;
use crate::error::Result;

pub fn run(db: &Database, config: &AppConfig) -> Result<()> {
    println!("Syncing Gmail emails...\n");

    let account_results = sync::sync_all_accounts(db, config)?;

    for ar in &account_results {
        println!("Account: {}", ar.email);
        match &ar.result {
            Ok(result) => {
                println!("  Emails found:    {}", result.emails_found);
                println!("  Imported:        {}", result.imported);
                println!("  Skipped (dup):   {}", result.skipped_duplicate);
                println!("  Skipped (xfer):  {}", result.skipped_transfer);
                println!("  Skipped (rule):  {}", result.skipped_rule);
                println!("  Skipped (error): {}", result.skipped_parse_error);
            }
            Err(e) => {
                println!("  ERROR: {}", e.user_message());
            }
        }
        println!();
    }

    Ok(())
}
