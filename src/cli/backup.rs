use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use crate::config::AppConfig;
use crate::db::connection::Database;
use crate::error::{AppError, Result};

pub fn run_backup(path: Option<PathBuf>, db: &Database, config: &AppConfig) -> Result<()> {
    let dest = match path {
        Some(p) => p,
        None => {
            let db_path = config.effective_db_path()?;
            let backups_dir = db_path
                .parent()
                .ok_or_else(|| AppError::Config("Cannot determine backups directory.".into()))?
                .join("backups");
            let timestamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S");
            backups_dir.join(format!("cointui-{timestamp}.db"))
        }
    };

    db.backup_to(&dest)?;
    println!("Backup created: {}", dest.display());
    Ok(())
}

pub fn run_restore(source: PathBuf, config: &AppConfig) -> Result<()> {
    if !source.exists() {
        return Err(AppError::NotFound(format!(
            "Backup file: {}",
            source.display()
        )));
    }

    // Verify the source is a valid SQLite DB by trying to open it.
    let _check = Database::new(&source)?;

    let dest = config.effective_db_path()?;
    println!(
        "This will replace your current database at:\n  {}\nwith the backup at:\n  {}",
        dest.display(),
        source.display()
    );
    print!("Continue? [y/N] ");
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    if !answer.trim().eq_ignore_ascii_case("y") {
        println!("Restore cancelled.");
        return Ok(());
    }

    std::fs::copy(&source, &dest)?;
    println!("Database restored from: {}", source.display());
    Ok(())
}
