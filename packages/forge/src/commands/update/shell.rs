//! Shell restart logic for Noctalia Desktop Shell
//!
//! After a NixOS rebuild, Quickshell may continue running with an old store path
//! while the shell commands point to a new path. This module detects when a restart
//! is needed and handles it automatically.

use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::commands::executor::{get_output, run_capture};
use crate::commands::CommandMessage;

use super::out;

/// Information about a running Quickshell process
#[derive(Debug)]
pub struct RunningShellInfo {
    pub running_path: String,
    pub pid: u32,
}

/// Detect ALL running Quickshell-based shells and get their store paths
pub async fn get_all_running_quickshell_info() -> Vec<RunningShellInfo> {
    // Get all quickshell processes with full command line
    let output = match get_output("pgrep", &["-a", "quickshell"]).await {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    if output.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();

    // Parse ALL quickshell processes (there may be duplicates after updates)
    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() < 2 {
            continue;
        }

        let pid: u32 = match parts[0].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let cmd = parts[1];

        // Noctalia: quickshell -p /nix/store/.../noctalia-shell/share/noctalia-shell
        if cmd.contains("/noctalia-shell") {
            if let Some(path) = extract_path_arg(cmd, "-p") {
                results.push(RunningShellInfo {
                    running_path: path,
                    pid,
                });
            }
        }
    }

    results
}

/// Extract a path argument from a command line (e.g., -p /path/to/something)
fn extract_path_arg(cmd: &str, flag: &str) -> Option<String> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == flag && i + 1 < parts.len() {
            return Some(parts[i + 1].to_string());
        }
    }
    None
}

/// Get the expected store path for Noctalia shell after rebuild
pub async fn get_expected_shell_path() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let symlink_path = PathBuf::from(format!(
        "{}/.config/quickshell/noctalia-shell",
        home
    ));

    // Read the symlink target
    match std::fs::read_link(&symlink_path) {
        Ok(target) => Some(target.to_string_lossy().to_string()),
        Err(_) => None,
    }
}

/// Check if shell needs restart and restart if necessary
/// Returns Some("Noctalia") if restarted/cleaned up, None if not needed
pub async fn restart_shell_if_needed(
    tx: &mpsc::Sender<CommandMessage>,
) -> Result<Option<String>> {
    // Get info about ALL running quickshell processes
    let running_shells = get_all_running_quickshell_info().await;

    if running_shells.is_empty() {
        tracing::debug!("No Quickshell process running, skipping restart check");
        return Ok(None);
    }

    tracing::info!("Found {} running Quickshell process(es)", running_shells.len());
    for info in &running_shells {
        tracing::info!(
            "  PID {}: Noctalia at {}",
            info.pid,
            info.running_path
        );
    }

    // Get expected path after rebuild
    let expected_path = match get_expected_shell_path().await {
        Some(path) => path,
        None => {
            tracing::warn!("Could not determine expected path for Noctalia shell");
            return Ok(None);
        }
    };

    tracing::info!("Expected shell path: {}", expected_path);

    // Categorize processes: correct path vs wrong path
    let mut correct_pids: Vec<u32> = Vec::new();
    let mut wrong_pids: Vec<u32> = Vec::new();

    for info in &running_shells {
        if info.running_path == expected_path {
            correct_pids.push(info.pid);
        } else {
            wrong_pids.push(info.pid);
        }
    }

    tracing::info!(
        "Correct path: {} process(es), wrong path: {} process(es)",
        correct_pids.len(),
        wrong_pids.len()
    );

    // If there are no wrong processes, nothing to do
    if wrong_pids.is_empty() {
        tracing::info!("All shells have correct path, no cleanup needed");
        return Ok(None);
    }

    // Kill only the processes with wrong paths
    out(tx, "").await;
    if correct_pids.is_empty() {
        out(
            tx,
            "  Restarting Noctalia shell (store path changed)...",
        )
        .await;
    } else {
        out(
            tx,
            &format!(
                "  Cleaning up {} stale Noctalia shell process(es)...",
                wrong_pids.len()
            ),
        )
        .await;
    }

    // Kill processes with wrong paths
    for pid in &wrong_pids {
        tracing::info!("Killing stale quickshell PID {}", pid);
        let _ = run_capture("kill", &[&pid.to_string()]).await;
    }

    // Wait a moment for processes to die
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Only start a new shell if there wasn't already a correct one running
    if correct_pids.is_empty() {
        // Try hyprctl dispatch exec first (preferred for Wayland)
        let hyprctl_available = run_capture("which", &["hyprctl"])
            .await
            .map(|(ok, _, _)| ok)
            .unwrap_or(false);

        if hyprctl_available {
            let _ = run_capture("hyprctl", &["dispatch", "exec", "noctalia-shell"]).await;
        } else {
            // Direct launch as fallback
            let _ = run_capture("nohup", &["noctalia-shell"]).await;
        }

        // Wait for shell to start
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Verify restart succeeded
        let new_shells = get_all_running_quickshell_info().await;
        if !new_shells.is_empty() {
            tracing::info!("Shell restarted successfully");
        } else {
            tracing::warn!("Shell may not have restarted properly");
        }

        Ok(Some("Noctalia".to_string()))
    } else {
        // A correct shell was already running, we just cleaned up stale ones
        tracing::info!("Kept existing correct shell, cleaned up {} stale process(es)", wrong_pids.len());
        Ok(Some("Noctalia (cleanup)".to_string()))
    }
}
