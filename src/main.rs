use std::time::Duration;

use clap::Parser;

use cointui::app::App;
use cointui::config::AppConfig;
use cointui::db::connection::Database;
use cointui::db::tag_repo::TagRepo;
use cointui::error::Result;
use cointui::event::{AppEvent, EventHandler};
use cointui::ui;

/// CoinTUI - Terminal-based personal finance manager
#[derive(Parser, Debug)]
#[command(
    name = "cointui",
    version,
    about,
    long_about = "CoinTUI - Terminal-based personal finance manager\n\n\
        Manage your personal finances from the terminal. Launch without flags\n\
        to start the interactive TUI, or use flags for quick CLI operations.",
    after_long_help = "\
EXAMPLES:
    Add an expense (minimal):
        cointui --add \"Coffee\" --amount 3500

    Add an income with all options:
        cointui --add \"Salary\" --amount 1500000 --kind income --tag Salary --date 2026-01-15

    Import transactions from CSV:
        cointui --import transactions.csv

    Export to JSON:
        cointui --export data.json
        cointui --export data.txt --format json

    Backup and restore:
        cointui --backup
        cointui --backup my-backup.db
        cointui --restore my-backup.db

    Manage tags:
        cointui --tags
        cointui --add-tag \"Food\"
        cointui --rename-tag \"Food:Groceries\"
        cointui --delete-tag \"Groceries\"

FORMATS:
    Export: CSV (.csv) or JSON (.json), detected from extension or --format
    Import: CSV with headers; interactive column mapping prompts on run
    Dates:  YYYY-MM-DD (e.g., 2026-01-15)
    Amount: Whole currency units as number (e.g., 3500 for $3.500)"
)]
struct Cli {
    /// Path to the config file (default: ~/.config/cointui/config.toml)
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,

    // -- Transaction --

    /// Add a transaction (provide the source/description)
    #[arg(long, help_heading = "Transaction",
        long_help = "Add a transaction with the given source/description.\n\
            Requires --amount. Optional: --kind, --tag, --date, --notes.\n\
            Defaults: kind=expense, tag=first available, date=today.")]
    add: Option<String>,

    /// Amount for --add (required with --add)
    #[arg(long, help_heading = "Transaction",
        long_help = "Amount in whole currency units (e.g., 3500 for $3.500).\n\
            Required when using --add.")]
    amount: Option<f64>,

    /// Transaction kind for --add: "income" or "expense" (default: expense)
    #[arg(long, help_heading = "Transaction",
        long_help = "Transaction kind: \"income\" or \"expense\".\n\
            Defaults to \"expense\" if omitted.")]
    kind: Option<String>,

    /// Tag name for --add (default: first available tag)
    #[arg(long, help_heading = "Transaction")]
    tag: Option<String>,

    /// Date for --add in YYYY-MM-DD format (default: today)
    #[arg(long, help_heading = "Transaction",
        long_help = "Date in YYYY-MM-DD format (e.g., 2026-01-15).\n\
            Defaults to today if omitted.")]
    date: Option<String>,

    /// Notes for --add
    #[arg(long, help_heading = "Transaction")]
    notes: Option<String>,

    // -- Import / Export --

    /// Import transactions from a CSV file
    #[arg(long, help_heading = "Import / Export",
        long_help = "Import transactions from a CSV file.\n\
            The CSV must have headers. An interactive prompt will guide\n\
            you through mapping columns to transaction fields.")]
    import: Option<std::path::PathBuf>,

    /// Export transactions to a file (CSV or JSON, detected from extension)
    #[arg(long, help_heading = "Import / Export",
        long_help = "Export transactions to a file.\n\
            Format is auto-detected from extension (.csv or .json).\n\
            Use --format to override.")]
    export: Option<std::path::PathBuf>,

    /// Export format override (csv or json)
    #[arg(long, help_heading = "Import / Export")]
    format: Option<String>,

    // -- Backup --

    /// Create a database backup (optional path; defaults to timestamped file)
    #[arg(long, help_heading = "Backup",
        long_help = "Create a database backup.\n\
            If no path is given, saves a timestamped file to\n\
            ~/.local/share/cointui/backups/.")]
    backup: Option<Option<std::path::PathBuf>>,

    /// Restore database from a backup file
    #[arg(long, help_heading = "Backup")]
    restore: Option<std::path::PathBuf>,

    // -- Tags --

    /// List all tags
    #[arg(long, help_heading = "Tags")]
    tags: bool,

    /// Add a new tag
    #[arg(long, help_heading = "Tags")]
    add_tag: Option<String>,

    /// Rename a tag ("OldName:NewName")
    #[arg(long, help_heading = "Tags",
        long_help = "Rename a tag using the format \"OldName:NewName\".\n\
            Example: --rename-tag \"Food:Groceries\"")]
    rename_tag: Option<String>,

    /// Delete a tag (must have no transactions or recurring entries)
    #[arg(long, help_heading = "Tags",
        long_help = "Delete a tag by name.\n\
            Fails if any transactions or recurring entries reference the tag.\n\
            Reassign them first or delete them before removing the tag.")]
    delete_tag: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load configuration.
    let config = match cli.config {
        Some(ref path) => AppConfig::load_from(path)?,
        None => AppConfig::load()?,
    };

    // Resolve and open the database.
    let db_path = config.effective_db_path()?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let db = Database::new(&db_path)?;

    // Seed default tags on a fresh database.
    let tag_repo = TagRepo::new(&db);
    tag_repo.seed_defaults(&["Other".to_string(), "Salary".to_string()])?;

    // Handle CLI subcommands before launching TUI.
    if let Some(path) = cli.import {
        return cointui::cli::import::run(path, &db);
    }
    if let Some(path) = cli.export {
        return cointui::cli::export::run(path, &db, cli.format);
    }
    if let Some(path) = cli.backup {
        return cointui::cli::backup::run_backup(path, &db, &config);
    }
    if let Some(path) = cli.restore {
        return cointui::cli::backup::run_restore(path, &config);
    }
    if cli.tags {
        return cointui::cli::tags::run_list(&db);
    }
    if let Some(name) = cli.add_tag {
        return cointui::cli::tags::run_add(&name, &db);
    }
    if let Some(spec) = cli.rename_tag {
        return cointui::cli::tags::run_rename(&spec, &db);
    }
    if let Some(name) = cli.delete_tag {
        return cointui::cli::tags::run_delete(&name, &db);
    }
    if let Some(source) = cli.add {
        let args = cointui::cli::add::AddArgs {
            amount: cli.amount,
            kind: cli.kind,
            tag: cli.tag,
            date: cli.date,
            notes: cli.notes,
        };
        return cointui::cli::add::run(source, args, &db, &config);
    }

    let db_path_display = db_path.display().to_string();

    // Create the application state.
    let mut app = App::new(db, config, db_path_display)?;

    // Process any due recurring entries on startup.
    if let Err(e) = app.process_recurring() {
        app.set_status(e.user_message());
    }

    // Install a panic hook that restores the terminal before printing the panic.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        ratatui::restore();
        original_hook(panic_info);
    }));

    // Initialise the terminal.
    let mut terminal = ratatui::init();

    // Event handler with 250ms tick rate.
    let events = EventHandler::new(Duration::from_millis(250));

    // Main event loop.
    loop {
        // Draw the UI.
        terminal
            .draw(|frame| {
                ui::draw(frame, &mut app);
            })
            .map_err(cointui::error::AppError::Io)?;

        // Poll for the next event.
        match events.next()? {
            AppEvent::Key(key) => {
                app.handle_key(key);
            }
            AppEvent::Tick => {
                app.tick_status();
            }
            AppEvent::Resize(_, _) => {
                // Terminal handles resize automatically via ratatui.
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore the terminal.
    ratatui::restore();

    Ok(())
}
