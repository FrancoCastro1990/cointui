use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

/// Application-level configuration persisted as TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Currency symbol displayed next to amounts (e.g. `"$"`, `"€"`).
    pub currency: String,
    /// Thousands separator for amount display (e.g. `"."` for Chilean, `","` for US).
    #[serde(default = "default_thousands_separator")]
    pub thousands_separator: String,
    /// Decimal separator for amount display (e.g. `","` for Chilean, `"."` for US).
    #[serde(default = "default_decimal_separator")]
    pub decimal_separator: String,
    /// Tags that are seeded into a fresh database.
    pub default_tags: Vec<String>,
    /// Override for the database file path. When `None` the default XDG data
    /// directory is used (`~/.local/share/cointui/cointui.db`).
    pub db_path: Option<PathBuf>,
}

fn default_thousands_separator() -> String {
    ".".into()
}

fn default_decimal_separator() -> String {
    ",".into()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            currency: "$".into(),
            thousands_separator: ".".into(),
            decimal_separator: ",".into(),
            default_tags: vec![
                "Comida".into(),
                "Transporte".into(),
                "Entretenimiento".into(),
                "Servicios".into(),
                "Salario".into(),
                "Salud".into(),
                "Educación".into(),
                "Otros".into(),
            ],
            db_path: None,
        }
    }
}

impl AppConfig {
    /// Resolve the effective database path.
    ///
    /// If `db_path` is set in the config, that value is returned. Otherwise we
    /// use the XDG data directory (`~/.local/share/cointui/cointui.db`).
    pub fn effective_db_path(&self) -> Result<PathBuf> {
        if let Some(ref p) = self.db_path {
            return Ok(p.clone());
        }

        let dirs = ProjectDirs::from("", "", "cointui").ok_or_else(|| {
            AppError::Config("Could not determine XDG data directory.".into())
        })?;

        let data_dir = dirs.data_dir();
        fs::create_dir_all(data_dir)?;
        Ok(data_dir.join("cointui.db"))
    }

    /// Load the configuration from the default XDG config path
    /// (`~/.config/cointui/config.toml`).
    ///
    /// If the file does not exist it is created with sensible defaults.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        Self::load_from(&path)
    }

    /// Load from an explicit path, creating a default file if it is missing.
    pub fn load_from(path: &Path) -> Result<Self> {
        if path.exists() {
            let contents = fs::read_to_string(path)?;
            let config: AppConfig =
                toml::from_str(&contents).map_err(|e| AppError::Config(e.to_string()))?;
            Ok(config)
        } else {
            let config = AppConfig::default();
            config.save_to(path)?;
            Ok(config)
        }
    }

    /// Persist the current config to the default XDG path.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        self.save_to(&path)
    }

    /// Persist the current config to an explicit path, creating parent
    /// directories as needed.
    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents =
            toml::to_string_pretty(self).map_err(|e| AppError::Config(e.to_string()))?;
        fs::write(path, contents)?;
        Ok(())
    }

    /// Returns the default config file path
    /// (`~/.config/cointui/config.toml`).
    fn config_path() -> Result<PathBuf> {
        let dirs = ProjectDirs::from("", "", "cointui").ok_or_else(|| {
            AppError::Config("Could not determine XDG config directory.".into())
        })?;
        let config_dir = dirs.config_dir();
        Ok(config_dir.join("config.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_config_has_expected_tags() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.currency, "$");
        assert!(cfg.default_tags.contains(&"Comida".to_string()));
        assert!(cfg.default_tags.contains(&"Otros".to_string()));
        assert_eq!(cfg.default_tags.len(), 8);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");

        let original = AppConfig::default();
        original.save_to(&path).unwrap();

        let loaded = AppConfig::load_from(&path).unwrap();
        assert_eq!(loaded.currency, original.currency);
        assert_eq!(loaded.default_tags, original.default_tags);
    }

    #[test]
    fn load_creates_default_when_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("subdir/config.toml");

        let loaded = AppConfig::load_from(&path).unwrap();
        assert_eq!(loaded.currency, "$");
        assert!(path.exists());
    }
}
