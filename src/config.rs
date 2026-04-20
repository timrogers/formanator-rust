//! Persistent configuration stored at `~/.formanatorrc.json`.
//!
//! This matches the file format used by the original Node.js implementation, so
//! the two clients can share the same login state.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const CONFIG_FILENAME: &str = ".formanatorrc.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub email: Option<String>,
}

fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine your home directory")?;
    Ok(home.join(CONFIG_FILENAME))
}

/// Read the saved config from disk, returning `None` if the file does not exist.
pub fn read_config() -> Result<Option<Config>> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file at {}", path.display()))?;
    let parsed: Config = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse config file at {}", path.display()))?;
    Ok(Some(parsed))
}

/// Return the saved access token, if any.
pub fn get_access_token() -> Result<Option<String>> {
    Ok(read_config()?.map(|c| c.access_token))
}

/// Resolve an access token from an explicit CLI/env value, falling back to the
/// saved config file.
pub fn resolve_access_token(explicit: Option<&str>) -> Result<String> {
    if let Some(token) = explicit {
        return Ok(token.to_string());
    }
    match get_access_token()? {
        Some(t) if !t.is_empty() => Ok(t),
        _ => anyhow::bail!("You aren't logged in to Forma. Please run `formanator login` first."),
    }
}

/// Persist the given config to disk.
pub fn store_config(config: &Config) -> Result<()> {
    let path = config_path()?;
    let serialised = serde_json::to_string(config)?;
    fs::write(&path, serialised)
        .with_context(|| format!("Failed to write config file at {}", path.display()))?;
    Ok(())
}
