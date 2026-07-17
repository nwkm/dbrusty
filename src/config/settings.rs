use std::path::PathBuf;

use color_eyre::{eyre::Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub theme: String,
    pub default_fetch_size: u32,
    pub autosave_interval: u64,
    pub log_level: String,
    pub editor: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            default_fetch_size: 1000,
            autosave_interval: 300,
            log_level: "info".to_string(),
            editor: String::new(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path).context("Failed to read config file")?;
            Ok(toml::from_str(&content).context("Failed to parse config file")?)
        } else {
            let config = AppConfig::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).context("Failed to create config directory")?;
            }
            let content = toml::to_string_pretty(&config).context("Failed to serialize config")?;
            std::fs::write(&path, content).context("Failed to write config file")?;
            tracing::info!("Created default config at {:?}", path);
            Ok(config)
        }
    }

    pub fn config_dir() -> Result<PathBuf> {
        let dir = ProjectDirs::from("", "", "dbrusty")
            .map(|d| d.config_dir().to_path_buf())
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                PathBuf::from(home).join(".config").join("dbrusty")
            });
        std::fs::create_dir_all(&dir).context("Failed to create config directory")?;
        Ok(dir)
    }

    #[allow(dead_code)]
    pub fn data_dir() -> Result<PathBuf> {
        let dir = ProjectDirs::from("", "", "dbrusty")
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join("dbrusty")
            });
        std::fs::create_dir_all(&dir).context("Failed to create data directory")?;
        Ok(dir)
    }

    fn path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }
}
