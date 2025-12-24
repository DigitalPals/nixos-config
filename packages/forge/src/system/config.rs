//! Configuration file parsing

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// Browser backup configuration
#[derive(Debug, Clone, Default)]
pub struct BrowserBackupConfig {
    pub repo: String,
    pub age_recipient: String,
    pub age_key_1password: Option<String>,
    pub age_key_path: Option<String>,
    pub local_repo_path: String,
    pub backup_retention: u32,
}

/// Load browser backup configuration from file
pub fn load_browser_config(path: &Path) -> Result<BrowserBackupConfig> {
    let content = std::fs::read_to_string(path)?;
    let mut config = BrowserBackupConfig::default();
    let mut vars = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            vars.insert(key.to_string(), value.to_string());
        }
    }

    config.repo = vars
        .get("BROWSER_BACKUP_REPO")
        .cloned()
        .unwrap_or_default();
    config.age_recipient = vars.get("AGE_RECIPIENT").cloned().unwrap_or_default();
    config.age_key_1password = vars.get("AGE_KEY_1PASSWORD").cloned();
    config.age_key_path = vars.get("AGE_KEY_PATH").cloned();
    config.local_repo_path = vars
        .get("LOCAL_REPO_PATH")
        .cloned()
        .unwrap_or_else(|| "~/.local/share/browser-backup".to_string());
    config.backup_retention = vars
        .get("BACKUP_RETENTION")
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);

    Ok(config)
}

/// Expand ~ in paths to home directory
pub fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}{}", home.display(), &path[1..]);
        }
    }
    path.to_string()
}
