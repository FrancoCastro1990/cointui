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
#[command(name = "cointui", version, about)]
struct Cli {
    /// Path to the config file (default: ~/.config/cointui/config.toml)
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,
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
    tag_repo.seed_defaults(&config.default_tags)?;

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
            .map_err(|e| cointui::error::AppError::Io(e))?;

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
