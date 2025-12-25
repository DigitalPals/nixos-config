//! Configuration file parsing

#![allow(dead_code)]

use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// CPU metadata from host-info.json
#[derive(Debug, Clone, Deserialize)]
pub struct CpuMeta {
    pub vendor: String,
    pub model: String,
}

/// GPU metadata from host-info.json
#[derive(Debug, Clone, Deserialize)]
pub struct GpuMeta {
    pub vendor: String,
    pub model: Option<String>,
}

/// Host hardware metadata loaded from host-info.json
#[derive(Debug, Clone, Deserialize)]
pub struct HostMetadata {
    pub cpu: Option<CpuMeta>,
    pub gpu: Option<GpuMeta>,
    pub form_factor: Option<String>,
    pub ram: Option<String>,
}

/// Host configuration discovered from filesystem
#[derive(Debug, Clone)]
pub struct HostConfig {
    pub name: String,
    pub description: String,
    pub metadata: Option<HostMetadata>,
}

/// Load host metadata from host-info.json
fn load_host_metadata(host_path: &Path) -> Option<HostMetadata> {
    let metadata_path = host_path.join("host-info.json");
    if let Ok(content) = std::fs::read_to_string(&metadata_path) {
        serde_json::from_str(&content).ok()
    } else {
        None
    }
}

/// Discover available hosts from the hosts/ directory
/// Checks multiple locations in order: /tmp/nixos-config, ~/nixos-config, /etc/nixos
pub fn discover_hosts() -> Vec<HostConfig> {
    let locations = [
        "/tmp/nixos-config/hosts".to_string(),
        format!(
            "{}/nixos-config/hosts",
            std::env::var("HOME").unwrap_or_else(|_| "/root".to_string())
        ),
        "/etc/nixos/hosts".to_string(),
    ];

    // Find first existing hosts directory
    let hosts_dir = locations
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .cloned();

    let mut hosts = Vec::new();

    if let Some(hosts_path) = hosts_dir {
        if let Ok(entries) = std::fs::read_dir(&hosts_path) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let host_path = entry.path();
                    let default_nix = host_path.join("default.nix");

                    // Extract description from first comment line
                    let description = if let Ok(content) = std::fs::read_to_string(&default_nix) {
                        parse_host_description(&content)
                    } else {
                        "Host configuration".to_string()
                    };

                    // Load hardware metadata if available
                    let metadata = load_host_metadata(&host_path);

                    hosts.push(HostConfig {
                        name,
                        description,
                        metadata,
                    });
                }
            }
        }
    }

    // Sort alphabetically by name
    hosts.sort_by(|a, b| a.name.cmp(&b.name));
    hosts
}

/// Parse description from first line comment: "# hostname - Description"
fn parse_host_description(content: &str) -> String {
    if let Some(first_line) = content.lines().next() {
        if first_line.starts_with('#') {
            // Format: "# hostname - Description"
            if let Some(pos) = first_line.find(" - ") {
                return first_line[pos + 3..].trim().to_string();
            }
        }
    }
    "Host configuration".to_string()
}

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
