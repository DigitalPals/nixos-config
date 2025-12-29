//! Configuration file support for forge-notify
//!
//! Loads configuration from TOML file at ~/.config/forge/notify.toml
//! Falls back to defaults if the file doesn't exist or can't be parsed.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::constants;

/// Configuration file path relative to home
const CONFIG_SUBDIR: &str = ".config/forge";
const CONFIG_FILE: &str = "notify.toml";

/// Forge-notify configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotifyConfig {
    /// Timeout settings
    pub timeouts: TimeoutConfig,

    /// Notification settings
    pub notification: NotificationConfig,

    /// Which flake inputs to check for updates
    pub inputs: InputConfig,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            timeouts: TimeoutConfig::default(),
            notification: NotificationConfig::default(),
            inputs: InputConfig::default(),
        }
    }
}

/// Timeout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeoutConfig {
    /// Timeout for git fetch operations (seconds)
    pub git_fetch_secs: u64,

    /// Timeout for flake update checks (seconds)
    pub flake_check_secs: u64,

    /// Timeout for HTTP client requests (seconds)
    pub http_client_secs: u64,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            git_fetch_secs: constants::GIT_FETCH_TIMEOUT_SECS,
            flake_check_secs: constants::FLAKE_CHECK_TIMEOUT_SECS,
            http_client_secs: constants::HTTP_CLIENT_TIMEOUT_SECS,
        }
    }
}

/// Notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    /// Duration for desktop notification display (milliseconds)
    pub timeout_ms: i32,

    /// Notification urgency: "low", "normal", or "critical"
    pub urgency: String,

    /// Icon name for notifications
    pub icon: String,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            timeout_ms: constants::NOTIFICATION_TIMEOUT_MS,
            urgency: "normal".to_string(),
            icon: "software-update-available".to_string(),
        }
    }
}

/// Input configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InputConfig {
    /// Flake inputs to check for updates
    /// Default: ["nixpkgs"]
    pub priority_inputs: Vec<String>,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            priority_inputs: constants::PRIORITY_INPUTS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

impl NotifyConfig {
    /// Load configuration from file, or return defaults if not found
    pub fn load() -> Self {
        let path = config_file_path();

        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => config,
                Err(e) => {
                    tracing::warn!("Failed to parse config file: {}, using defaults", e);
                    Self::default()
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read config file: {}, using defaults", e);
                Self::default()
            }
        }
    }

    /// Save configuration to file
    #[allow(dead_code)]
    pub fn save(&self) -> anyhow::Result<()> {
        let path = config_file_path();

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Generate example configuration content
    #[allow(dead_code)]
    pub fn example_toml() -> String {
        let config = Self::default();
        toml::to_string_pretty(&config).unwrap_or_default()
    }
}

/// Get the configuration file path
fn config_file_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(CONFIG_SUBDIR).join(CONFIG_FILE))
        .unwrap_or_else(|| PathBuf::from("/tmp/forge-notify.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = NotifyConfig::default();
        assert_eq!(config.timeouts.git_fetch_secs, 10);
        assert_eq!(config.timeouts.flake_check_secs, 15);
        assert_eq!(config.timeouts.http_client_secs, 10);
        assert_eq!(config.notification.timeout_ms, 10000);
        assert!(!config.inputs.priority_inputs.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let config = NotifyConfig::default();
        let toml_str = toml::to_string(&config).unwrap();
        let parsed: NotifyConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.timeouts.git_fetch_secs, config.timeouts.git_fetch_secs);
    }

    #[test]
    fn test_partial_config_parsing() {
        let toml_str = r#"
[timeouts]
git_fetch_secs = 20
"#;
        let config: NotifyConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.timeouts.git_fetch_secs, 20);
        // Other values should be defaults
        assert_eq!(config.timeouts.flake_check_secs, 15);
        assert_eq!(config.notification.timeout_ms, 10000);
    }

    #[test]
    fn test_example_toml_is_valid() {
        let example = NotifyConfig::example_toml();
        assert!(!example.is_empty());
        let _parsed: NotifyConfig = toml::from_str(&example).unwrap();
    }
}
