#![allow(dead_code)]

use std::path::PathBuf;

use color_eyre::{eyre::Context, Result};

use crate::config::AppConfig;
use crate::models::connection::{ConnectionConfig, ConnectionList};

pub fn load_connections() -> Result<Vec<ConnectionConfig>> {
    let path = connections_path()?;
    if path.exists() {
        let content = std::fs::read_to_string(&path).context("Failed to read connections file")?;
        let list: ConnectionList =
            toml::from_str(&content).context("Failed to parse connections file")?;
        Ok(list.connections)
    } else {
        Ok(Vec::new())
    }
}

pub fn save_connections(connections: &[ConnectionConfig]) -> Result<()> {
    let path = connections_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create storage directory")?;
    }
    let list = ConnectionList {
        connections: connections.to_vec(),
    };
    let content = toml::to_string_pretty(&list).context("Failed to serialize connections")?;
    std::fs::write(&path, content).context("Failed to write connections file")?;
    Ok(())
}

fn connections_path() -> Result<PathBuf> {
    Ok(AppConfig::config_dir()?.join("connections.toml"))
}
