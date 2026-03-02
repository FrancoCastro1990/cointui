/// All possible errors in the CoinTUI application.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Email sync: {0}")]
    EmailSync(String),
}

impl AppError {
    /// Returns a short, user-friendly message suitable for display in the TUI
    /// status bar.
    pub fn user_message(&self) -> String {
        match self {
            AppError::Database(_) => "A database error occurred. Check logs for details.".into(),
            AppError::Config(msg) => format!("Configuration problem: {msg}"),
            AppError::Io(_) => "An I/O error occurred. Check file permissions.".into(),
            AppError::Csv(_) => "Error processing CSV file. Check the file format.".into(),
            AppError::Validation(msg) => msg.clone(),
            AppError::NotFound(msg) => format!("Not found: {msg}"),
            AppError::EmailSync(msg) => format!("Email sync: {msg}"),
        }
    }
}

/// Convenience alias used throughout the application.
pub type Result<T> = std::result::Result<T, AppError>;
