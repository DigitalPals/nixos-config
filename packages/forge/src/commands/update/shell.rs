//! Shell restart logic for Quickshell-based desktop shells
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

/// Types of Quickshell-based desktop shells
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShellType {
    Noctalia,
    Illogical,
    Caelestia,
}

impl ShellType {
    /// Display name for the shell
    pub fn name(&self) -> &'static str {
        match self {
            ShellType::Noctalia => "Noctalia",
            ShellType::Illogical => "Illogical Impulse",
            ShellType::Caelestia => "Caelestia",
        }
    }

    /// Command to restart this shell
    pub fn restart_command(&self) -> (&'static str, Vec<&'static str>) {
        match self {
            ShellType::Noctalia => ("noctalia-shell", vec![]),
            ShellType::Illogical => ("quickshell", vec!["-c", "~/.config/quickshell/ii"]),
            ShellType::Caelestia => ("caelestia-shell", vec![]),
        }
    }

    /// Path to check for expected store path (symlink target)
    pub fn config_symlink_path(&self) -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        match self {
            ShellType::Noctalia => Some(PathBuf::from(format!(
                "{}/.config/quickshell/noctalia-shell",
                home
            ))),
            ShellType::Illogical => Some(PathBuf::from(format!(
                "{}/.config/quickshell/ii",
                home
            ))),
            ShellType::Caelestia => Some(PathBuf::from(format!(
                "{}/.config/quickshell/caelestia-shell",
                home
            ))),
        }
    }
}

/// Information about a running Quickshell process
#[derive(Debug)]
pub struct RunningShellInfo {
    pub shell_type: ShellType,
    pub running_path: String,
    pub pid: u32,
}

/// Detect which Quickshell-based shell is running and get its store path
pub async fn get_running_quickshell_info() -> Option<RunningShellInfo> {
    // Get all quickshell processes with full command line
    let output = get_output("pgrep", &["-a", "quickshell"]).await.ok()?;

    if output.is_empty() {
        return None;
    }

    // Parse the first quickshell process (there should typically be only one)
    for line in output.lines() {
        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        if parts.len() < 2 {
            continue;
        }

        let pid: u32 = parts[0].parse().ok()?;
        let cmd = parts[1];

        // Detect shell type and extract path from command line
        if let Some(info) = parse_quickshell_command(pid, cmd) {
            return Some(info);
        }
    }

    None
}

/// Parse a quickshell command line to determine shell type and path
fn parse_quickshell_command(pid: u32, cmd: &str) -> Option<RunningShellInfo> {
    // Noctalia: quickshell -p /nix/store/.../noctalia-shell/share/noctalia-shell
    if cmd.contains("/noctalia-shell") {
        if let Some(path) = extract_path_arg(cmd, "-p") {
            return Some(RunningShellInfo {
                shell_type: ShellType::Noctalia,
                running_path: path,
                pid,
            });
        }
    }

    // Illogical: quickshell -c ~/.config/quickshell/ii
    // The -c points to a config dir, but we need to check the quickshell binary's store path
    if cmd.contains("quickshell/ii") || cmd.contains("-c") && cmd.contains("/ii") {
        // For illogical, the path comparison is different - check the quickshell binary itself
        if let Some(path) = extract_quickshell_binary_path(cmd) {
            return Some(RunningShellInfo {
                shell_type: ShellType::Illogical,
                running_path: path,
                pid,
            });
        }
    }

    // Caelestia: quickshell -p /nix/store/.../caelestia-shell/share/caelestia-shell
    if cmd.contains("/caelestia-shell") {
        if let Some(path) = extract_path_arg(cmd, "-p") {
            return Some(RunningShellInfo {
                shell_type: ShellType::Caelestia,
                running_path: path,
                pid,
            });
        }
    }

    None
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

/// Extract the quickshell binary path from the command
fn extract_quickshell_binary_path(cmd: &str) -> Option<String> {
    // The command starts with the binary path
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if let Some(binary) = parts.first() {
        if binary.contains("/nix/store/") {
            return Some(binary.to_string());
        }
    }
    None
}

/// Get the expected store path for a shell after rebuild
pub async fn get_expected_shell_path(shell: ShellType) -> Option<String> {
    let symlink_path = shell.config_symlink_path()?;

    // Read the symlink target
    match std::fs::read_link(&symlink_path) {
        Ok(target) => Some(target.to_string_lossy().to_string()),
        Err(_) => {
            // For illogical, the config dir might not be a symlink
            // In that case, check if the quickshell command exists and get its path
            if shell == ShellType::Illogical {
                get_output("which", &["quickshell"]).await.ok()
            } else {
                None
            }
        }
    }
}

/// Check if shell needs restart and restart if necessary
/// Returns Some(shell_name) if restarted, None if not needed
pub async fn restart_shell_if_needed(
    tx: &mpsc::Sender<CommandMessage>,
) -> Result<Option<String>> {
    // Get info about running quickshell
    let running_info = match get_running_quickshell_info().await {
        Some(info) => info,
        None => {
            tracing::debug!("No Quickshell process running, skipping restart check");
            return Ok(None);
        }
    };

    tracing::info!(
        "Found running {} shell (PID {}): {}",
        running_info.shell_type.name(),
        running_info.pid,
        running_info.running_path
    );

    // Get expected path after rebuild
    let expected_path = match get_expected_shell_path(running_info.shell_type).await {
        Some(path) => path,
        None => {
            tracing::warn!(
                "Could not determine expected path for {} shell",
                running_info.shell_type.name()
            );
            return Ok(None);
        }
    };

    tracing::info!("Expected shell path: {}", expected_path);

    // Compare paths - for Noctalia/Caelestia, compare the full -p path
    // For Illogical, compare the quickshell binary path
    let needs_restart = match running_info.shell_type {
        ShellType::Noctalia | ShellType::Caelestia => {
            // The running path should match the symlink target
            running_info.running_path != expected_path
        }
        ShellType::Illogical => {
            // Compare quickshell binary paths
            !running_info.running_path.contains(&expected_path)
                && !expected_path.contains(&running_info.running_path)
        }
    };

    if !needs_restart {
        tracing::info!("Shell paths match, no restart needed");
        return Ok(None);
    }

    // Restart the shell
    out(tx, "").await;
    out(
        tx,
        &format!(
            "  Restarting {} shell (store path changed)...",
            running_info.shell_type.name()
        ),
    )
    .await;

    // Kill existing quickshell processes
    let _ = run_capture("pkill", &["-x", "quickshell"]).await;

    // Wait a moment for the process to die
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Restart using hyprctl if available, otherwise direct launch
    let (cmd, args) = running_info.shell_type.restart_command();

    // Try hyprctl dispatch exec first (preferred for Wayland)
    let hyprctl_available = run_capture("which", &["hyprctl"]).await.map(|(ok, _, _)| ok).unwrap_or(false);

    if hyprctl_available {
        let exec_cmd = if args.is_empty() {
            cmd.to_string()
        } else {
            format!("{} {}", cmd, args.join(" "))
        };

        let _ = run_capture("hyprctl", &["dispatch", "exec", &exec_cmd]).await;
    } else {
        // Direct launch as fallback
        let mut launch_args: Vec<&str> = vec![cmd];
        launch_args.extend(args.iter());

        // Use nohup to detach the process
        let _ = run_capture("nohup", &launch_args).await;
    }

    // Wait for shell to start
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Verify restart succeeded
    let new_info = get_running_quickshell_info().await;
    if new_info.is_some() {
        tracing::info!("Shell restarted successfully");
        Ok(Some(running_info.shell_type.name().to_string()))
    } else {
        tracing::warn!("Shell may not have restarted properly");
        // Still report as restarted since we attempted it
        Ok(Some(running_info.shell_type.name().to_string()))
    }
}
