//! Package comparison utilities using nvd

use anyhow::Result;
use tokio::sync::mpsc;

use super::out;
use crate::commands::executor::run_capture;
use crate::commands::CommandMessage;

/// Result of package comparison containing version changes and closure summary
pub struct PackageCompareResult {
    pub changes: Vec<(String, String, String)>,
    pub added: Vec<(String, String)>,
    pub removed: Vec<(String, String)>,
    pub closure_summary: Option<String>,
}

impl Default for PackageCompareResult {
    fn default() -> Self {
        Self {
            changes: Vec::new(),
            added: Vec::new(),
            removed: Vec::new(),
            closure_summary: None,
        }
    }
}

/// Compare two specific system paths using nvd
pub async fn parse_package_changes(
    old_system: Option<&str>,
    new_system: Option<&str>,
    tx: &mpsc::Sender<CommandMessage>,
) -> Result<PackageCompareResult> {
    let old_path = match old_system {
        Some(path) if !path.is_empty() => path,
        _ => {
            tracing::debug!("parse_package_changes: no old system path provided");
            return Ok(PackageCompareResult::default());
        }
    };
    let new_path = match new_system {
        Some(path) if !path.is_empty() => path,
        _ => {
            tracing::debug!("parse_package_changes: no new system path provided");
            return Ok(PackageCompareResult::default());
        }
    };

    if old_path == new_path {
        out(tx, "    System path unchanged").await;
        return Ok(PackageCompareResult::default());
    }

    out(tx, "    Comparing explicit before/after system paths").await;

    let (success, stdout, _stderr) = run_capture("nvd", &["diff", old_path, new_path]).await?;

    if !success {
        out(tx, "    nvd diff failed").await;
        tracing::debug!("parse_package_changes: nvd diff failed");
        return Ok(PackageCompareResult::default());
    }

    parse_nvd_output(&stdout, tx).await
}

/// Parse nvd diff output into package changes and closure summary
async fn parse_nvd_output(
    stdout: &str,
    tx: &mpsc::Sender<CommandMessage>,
) -> Result<PackageCompareResult> {
    // Parse nvd output - extract version changes, added/removed packages, and closure summary
    // Update format: "[U.]  #015  firefox    146.0 -> 146.0.1"
    // Added format:  "[A.]  #001  package-name  1.0"
    // Removed format: "[R.]  #001  package-name  1.0"
    // Closure format: "Closure size: 2478 -> 2478 (8 paths added, 8 paths removed, delta +0, disk usage -2.8KiB)."
    let mut changes = Vec::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut closure_summary = None;

    for line in stdout.lines() {
        let line = line.trim();

        // Capture closure size summary
        if line.starts_with("Closure size:") {
            let summary = line.strip_prefix("Closure size:").unwrap_or(line).trim();
            closure_summary = Some(summary.trim_end_matches('.').to_string());
            continue;
        }

        // Extract package name and version from "[X.]  #NNN  name  version" format
        let parse_pkg_line = |line: &str| -> Option<(String, String)> {
            let hash_pos = line.find('#')?;
            let after_hash = &line[hash_pos..];
            let space_pos = after_hash.find(char::is_whitespace)?;
            let rest = after_hash[space_pos..].trim();
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.is_empty() {
                return None;
            }
            let name = parts[0].to_string();
            let version = parts
                .get(1)
                .map(|v| v.trim_end_matches(',').to_string())
                .unwrap_or_default();
            Some((name, version))
        };

        // Added packages [A.] or [A*]
        if line.starts_with("[A") {
            if let Some((name, version)) = parse_pkg_line(line) {
                added.push((name, version));
            }
            continue;
        }

        // Removed packages [R.] or [R*]
        if line.starts_with("[R") {
            if let Some((name, version)) = parse_pkg_line(line) {
                removed.push((name, version));
            }
            continue;
        }

        // Updated packages [U.] or [U*]
        if !line.starts_with("[U") {
            continue;
        }

        if let Some(arrow_pos) = line.find(" -> ") {
            if let Some(hash_pos) = line.find('#') {
                let after_hash = &line[hash_pos..];
                if let Some(space_pos) = after_hash.find(char::is_whitespace) {
                    let rest = after_hash[space_pos..].trim();
                    let before_arrow = &rest[..rest.find(" -> ").unwrap_or(rest.len())];
                    let after_arrow = &line[arrow_pos + 4..];

                    let parts: Vec<&str> = before_arrow.split_whitespace().collect();
                    if parts.is_empty() {
                        continue;
                    }
                    let pkg_name = parts[0];
                    let old_ver = if parts.len() > 1 {
                        parts[1].trim_end_matches(',')
                    } else {
                        continue;
                    };
                    let new_ver = after_arrow
                        .split(|c: char| c == ',' || c.is_whitespace())
                        .next()
                        .unwrap_or("")
                        .trim();

                    if !pkg_name.is_empty() && !old_ver.is_empty() && !new_ver.is_empty() {
                        out(tx, &format!("    {}: {} → {}", pkg_name, old_ver, new_ver)).await;
                        changes.push((
                            pkg_name.to_string(),
                            old_ver.to_string(),
                            new_ver.to_string(),
                        ));
                    }
                }
            }
        }
    }

    Ok(PackageCompareResult {
        changes,
        added,
        removed,
        closure_summary,
    })
}
