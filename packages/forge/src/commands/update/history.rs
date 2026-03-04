//! Update history persistence and display
//!
//! Stores update summaries to ~/.local/state/forge/update-history.json
//! and provides CLI display for viewing past updates.

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::app::UpdateSummary;

/// Maximum number of history records to keep
const MAX_HISTORY_RECORDS: usize = 100;

/// A persisted record of an update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRecord {
    /// Unique ID (ISO timestamp)
    pub id: String,
    /// When the update occurred
    pub timestamp: DateTime<Utc>,
    /// Whether the update succeeded
    pub success: bool,
    /// The hostname where the update was run
    pub hostname: String,
    /// The update summary data
    pub summary: UpdateSummaryPersist,
}

/// Serializable version of UpdateSummary for persistence
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateSummaryPersist {
    /// Flake input changes (name, total_commits)
    pub flake_changes: Vec<FlakeChangePersist>,
    /// Package changes (name, old_version, new_version)
    pub package_changes: Vec<(String, String, String)>,
    /// Packages added (name, version)
    #[serde(default)]
    pub packages_added: Vec<(String, String)>,
    /// Packages removed (name, version)
    #[serde(default)]
    pub packages_removed: Vec<(String, String)>,
    /// Claude Code version change (old, new)
    pub claude_version: Option<(String, String)>,
    /// Codex CLI version change (old, new)
    pub codex_version: Option<(String, String)>,
    /// Reasons for reboot recommendation
    pub reboot_reasons: Vec<String>,
    /// Whether the rebuild failed
    pub rebuild_failed: bool,
    /// Whether rebuild was skipped (no changes)
    pub rebuild_skipped: bool,
    /// Browser profile status
    pub browser_status: String,
    /// Firmware status
    pub firmware_status: String,
}

/// Simplified flake change info for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakeChangePersist {
    pub name: String,
    pub total_commits: usize,
    pub compare_url: Option<String>,
}

/// Get the path to the history file
pub fn history_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".local/state/forge/update-history.json")
}

/// Convert UpdateSummary to persistable form
impl From<&UpdateSummary> for UpdateSummaryPersist {
    fn from(summary: &UpdateSummary) -> Self {
        let claude_version = match (&summary.claude_old, &summary.claude_new) {
            (Some(old), Some(new)) if old != new => Some((old.clone(), new.clone())),
            _ => None,
        };

        let codex_version = match (&summary.codex_old, &summary.codex_new) {
            (Some(old), Some(new)) if old != new => Some((old.clone(), new.clone())),
            _ => None,
        };

        UpdateSummaryPersist {
            flake_changes: summary
                .flake_changes
                .iter()
                .map(|c| FlakeChangePersist {
                    name: c.name.clone(),
                    total_commits: c.total_commits,
                    compare_url: c.compare_url.clone(),
                })
                .collect(),
            package_changes: summary.package_changes.clone(),
            packages_added: summary.packages_added.clone(),
            packages_removed: summary.packages_removed.clone(),
            claude_version,
            codex_version,
            reboot_reasons: summary.reboot_reasons.clone(),
            rebuild_failed: summary.rebuild_failed,
            rebuild_skipped: summary.rebuild_skipped,
            browser_status: summary.browser_status.clone(),
            firmware_status: summary.firmware_status.clone(),
        }
    }
}

/// Load history from disk
pub fn load_history() -> Result<Vec<UpdateRecord>> {
    let path = history_path();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&path)?;
    let records: Vec<UpdateRecord> = serde_json::from_str(&content)?;
    Ok(records)
}

/// Save a new record to history
pub fn save_record(summary: &UpdateSummary, success: bool) -> Result<()> {
    let path = history_path();

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Load existing records
    let mut records = load_history().unwrap_or_default();

    // Get hostname
    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()))
        .unwrap_or_else(|_| "unknown".to_string());

    // Create new record
    let now = Utc::now();
    let record = UpdateRecord {
        id: now.to_rfc3339(),
        timestamp: now,
        success,
        hostname,
        summary: UpdateSummaryPersist::from(summary),
    };

    // Add new record at the beginning
    records.insert(0, record);

    // Prune old records
    records.truncate(MAX_HISTORY_RECORDS);

    // Write back to disk
    let json = serde_json::to_string_pretty(&records)?;
    std::fs::write(&path, json)?;

    Ok(())
}

/// Print history to stdout (for CLI command)
pub fn print_history(details: Option<&str>) -> Result<()> {
    let records = load_history()?;

    if records.is_empty() {
        println!("No update history found.");
        println!();
        println!("Run 'forge update' to perform your first update.");
        return Ok(());
    }

    if let Some(id) = details {
        // Show details for specific record
        let record = records.iter().find(|r| r.id.starts_with(id));
        match record {
            Some(r) => print_record_details(r),
            None => {
                println!("No record found matching '{}'", id);
                println!();
                println!("Use 'forge update history' to see available records.");
            }
        }
    } else {
        // Show summary list
        print_history_list(&records);
    }

    Ok(())
}

/// Print the history list in a nice format
fn print_history_list(records: &[UpdateRecord]) {
    println!();
    println!("  Update History");
    println!("  ─────────────────────────────────────────────────────────────────");

    for record in records.iter().take(20) {
        let local_time: DateTime<Local> = record.timestamp.into();
        let date_str = local_time.format("%Y-%m-%d %H:%M").to_string();

        let status = if record.success { "✓" } else { "✗" };

        // Build summary text
        let mut parts = Vec::new();

        // Flake changes
        let total_commits: usize = record
            .summary
            .flake_changes
            .iter()
            .map(|c| c.total_commits)
            .sum();
        if total_commits > 0 {
            let input_count = record.summary.flake_changes.len();
            parts.push(format!("+{} commits ({})", total_commits, input_count));
        }

        // Package changes
        if !record.summary.package_changes.is_empty() {
            parts.push(format!("{} packages", record.summary.package_changes.len()));
        }

        // Rebuild status
        if record.summary.rebuild_failed {
            parts.push("rebuild failed".to_string());
        } else if record.summary.rebuild_skipped {
            parts.push("no changes".to_string());
        }

        let summary_text = if parts.is_empty() {
            "no changes".to_string()
        } else {
            parts.join(", ")
        };

        // Truncate summary if too long
        let max_len = 40;
        let summary_display = if summary_text.len() > max_len {
            format!("{}...", &summary_text[..max_len - 3])
        } else {
            summary_text
        };

        println!("  {}  {}  {}", date_str, status, summary_display);
    }

    if records.len() > 20 {
        println!("  ... and {} more", records.len() - 20);
    }

    println!("  ─────────────────────────────────────────────────────────────────");
    println!();
    println!("  Run 'forge update history --details <timestamp>' for full details.");
    println!();
}

/// Print detailed view of a single record
fn print_record_details(record: &UpdateRecord) {
    let local_time: DateTime<Local> = record.timestamp.into();

    println!();
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                     Update Details                               ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("  Date:     {}", local_time.format("%Y-%m-%d %H:%M:%S"));
    println!("  Host:     {}", record.hostname);
    println!(
        "  Status:   {}",
        if record.success { "Success" } else { "Failed" }
    );
    println!();

    // Flake changes
    if !record.summary.flake_changes.is_empty() {
        println!("  Flake Inputs:");
        for change in &record.summary.flake_changes {
            println!("    {:20} +{} commits", change.name, change.total_commits);
            if let Some(ref url) = change.compare_url {
                println!("                         → {}", url);
            }
        }
        println!();
    }

    // Package changes
    if !record.summary.package_changes.is_empty() {
        println!(
            "  Packages ({} changed):",
            record.summary.package_changes.len()
        );
        for (name, old, new) in &record.summary.package_changes {
            println!("    {:30} {} → {}", name, old, new);
        }
        println!();
    }

    // CLI tools
    if record.summary.claude_version.is_some() || record.summary.codex_version.is_some() {
        println!("  CLI Tools:");
        if let Some((old, new)) = &record.summary.claude_version {
            println!("    Claude Code:  {} → {}", old, new);
        }
        if let Some((old, new)) = &record.summary.codex_version {
            println!("    Codex CLI:    {} → {}", old, new);
        }
        println!();
    }

    // Status
    if record.summary.rebuild_failed {
        println!("  ⚠ Rebuild failed");
    } else if record.summary.rebuild_skipped {
        println!("  - Rebuild skipped (no changes)");
    }

    if !record.summary.reboot_reasons.is_empty() {
        println!("  ⚠ Reboot was recommended:");
        for reason in &record.summary.reboot_reasons {
            println!("    - {}", reason);
        }
    }

    println!();
}

/// Get a short summary string for display in the modal
pub fn format_short_summary(summary: &UpdateSummary) -> String {
    let mut parts = Vec::new();

    // Flake changes
    let total_commits: usize = summary.flake_changes.iter().map(|c| c.total_commits).sum();
    if total_commits > 0 {
        parts.push(format!("+{} commits", total_commits));
    }

    // Package changes
    if !summary.package_changes.is_empty() {
        parts.push(format!("{} packages", summary.package_changes.len()));
    }

    if parts.is_empty() {
        "No changes".to_string()
    } else {
        parts.join(", ")
    }
}
