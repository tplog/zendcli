use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ZendConfig {
    #[serde(default)]
    pub subdomain: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub api_token: String,
}

fn config_dir() -> PathBuf {
    dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".zendcli")
}

fn config_file() -> PathBuf {
    config_dir().join("config.json")
}

/// Load config from file, merging with env vars (env takes precedence).
pub fn load_config() -> ZendConfig {
    let mut config = ZendConfig::default();

    let path = config_file();
    if path.exists() {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(file_config) = serde_json::from_str::<ZendConfig>(&data) {
                config = file_config;
            }
        }
    }

    // Environment variables override file config
    if let Ok(val) = std::env::var("ZENDESK_SUBDOMAIN") {
        if !val.is_empty() {
            config.subdomain = val;
        }
    }
    if let Ok(val) = std::env::var("ZENDESK_EMAIL") {
        if !val.is_empty() {
            config.email = val;
        }
    }
    if let Ok(val) = std::env::var("ZENDESK_API_TOKEN") {
        if !val.is_empty() {
            config.api_token = val;
        }
    }

    config
}

/// Save config to ~/.zendcli/config.json with secure permissions.
pub fn save_config(config: &ZendConfig) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("Failed to set dir permissions: {e}"))?;

    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    let path = config_file();
    fs::write(&path, json).map_err(|e| format!("Failed to write config: {e}"))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("Failed to set file permissions: {e}"))?;

    Ok(())
}

/// Load and validate config. Prints to stderr and exits if not configured (matches TS behavior).
pub fn get_config() -> Result<ZendConfig, String> {
    let config = load_config();
    if config.subdomain.is_empty() || config.email.is_empty() || config.api_token.is_empty() {
        eprintln!("Not configured. Run: zend configure or set ZENDESK_SUBDOMAIN, ZENDESK_EMAIL, ZENDESK_API_TOKEN");
        std::process::exit(1);
    }
    Ok(config)
}
