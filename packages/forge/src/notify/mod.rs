//! Background update checker for Forge
//!
//! Checks for:
//! - NixOS config repo updates (changes from other machines)
//! - App profile updates (private-settings repo)
//! - Flake input updates (nixpkgs, home-manager, etc.)

pub mod config;
pub mod constants;
pub mod flake;
pub mod paths;
pub mod state;

use anyhow::Result;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use constants::git_fetch_timeout;
use paths::{app_backup_data_dir, nixos_config_dir};

/// Status of all update checks
#[derive(Debug, Default)]
pub struct UpdateStatus {
    /// Commits available in nixos-config repo (hash, message)
    pub config_updates: Vec<(String, String)>,
    /// Whether app profiles have updates
    pub app_updates: bool,
    /// Flake inputs that have updates available
    pub flake_updates: Vec<String>,
}

impl UpdateStatus {
    /// Returns true if any updates are available
    pub fn has_updates(&self) -> bool {
        !self.config_updates.is_empty() || self.app_updates || !self.flake_updates.is_empty()
    }

    /// Build a notification summary message
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();

        if !self.config_updates.is_empty() {
            let count = self.config_updates.len();
            lines.push(format!(
                "- {} config commit{} available",
                count,
                if count == 1 { "" } else { "s" }
            ));
        }

        if self.app_updates {
            lines.push("- App profiles updated".to_string());
        }

        if !self.flake_updates.is_empty() {
            let names = self.flake_updates.join(", ");
            lines.push(format!("- Flake inputs: {}", names));
        }

        lines.join("\n")
    }
}

/// Run all update checks concurrently
pub async fn check_all_updates() -> Result<UpdateStatus> {
    let (config_result, apps_result, flake_result) = tokio::join!(
        check_nixos_config_updates(),
        check_app_updates(),
        flake::check_flake_updates(),
    );

    Ok(UpdateStatus {
        config_updates: config_result.unwrap_or_default(),
        app_updates: apps_result.unwrap_or(false),
        flake_updates: flake_result.unwrap_or_default(),
    })
}

/// Check for nixos-config repo updates
async fn check_nixos_config_updates() -> Result<Vec<(String, String)>> {
    let config_dir = nixos_config_dir();

    // If no git repo, no updates to check
    if !config_dir.join(".git").exists() {
        return Ok(vec![]);
    }

    // Fetch from remote with timeout
    let fetch_result = tokio::time::timeout(
        git_fetch_timeout(),
        run_git_command(&config_dir, &["fetch", "origin"]),
    )
    .await;

    match fetch_result {
        Ok(Ok(true)) => {}
        _ => return Ok(vec![]), // Timeout or fetch failed
    }

    // Get list of commits on origin/main not in HEAD
    let (ok, log_output) = run_git_output(
        &config_dir,
        &["log", "HEAD..origin/main", "--pretty=format:%h|%s"],
    )
    .await?;

    if !ok || log_output.trim().is_empty() {
        return Ok(vec![]);
    }

    // Parse output into (hash, message) pairs
    let commits: Vec<(String, String)> = log_output
        .lines()
        .map(|line| {
            let parts: Vec<&str> = line.splitn(2, '|').collect();
            (
                parts.first().unwrap_or(&"").to_string(),
                parts.get(1).unwrap_or(&"").to_string(),
            )
        })
        .collect();

    Ok(commits)
}

/// Check for app profile updates
async fn check_app_updates() -> Result<bool> {
    let local_repo = app_backup_data_dir();

    if local_repo.as_os_str().is_empty() || !local_repo.join(".git").exists() {
        return Ok(false);
    }

    // Check if remote is configured
    let (remote_ok, _) = run_git_output(&local_repo, &["remote", "get-url", "origin"]).await?;
    if !remote_ok {
        return Ok(false);
    }

    // Fetch from remote with timeout
    let fetch_result = tokio::time::timeout(
        git_fetch_timeout(),
        run_git_command(&local_repo, &["fetch", "origin"]),
    )
    .await;

    match fetch_result {
        Ok(Ok(true)) => {}
        _ => return Ok(false),
    }

    // Count commits on origin that aren't in local HEAD
    let (ok, count_str) =
        run_git_output(&local_repo, &["rev-list", "HEAD..origin/main", "--count"]).await?;

    let count: usize = if ok {
        count_str.trim().parse().unwrap_or(0)
    } else {
        // Try origin/master as fallback
        let (master_ok, master_count) =
            run_git_output(&local_repo, &["rev-list", "HEAD..origin/master", "--count"]).await?;
        if !master_ok {
            return Ok(false);
        }
        master_count.trim().parse().unwrap_or(0)
    };

    Ok(count > 0)
}

/// Run a git command and return success status
async fn run_git_command(dir: &Path, args: &[&str]) -> Result<bool> {
    let mut cmd_args = vec!["-C", dir.to_str().unwrap_or(".")];
    cmd_args.extend(args);

    let status = Command::new("git")
        .args(&cmd_args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;

    Ok(status.success())
}

/// Run a git command and capture output
async fn run_git_output(dir: &Path, args: &[&str]) -> Result<(bool, String)> {
    let mut cmd_args = vec!["-C", dir.to_str().unwrap_or(".")];
    cmd_args.extend(args);

    let output = Command::new("git")
        .args(&cmd_args)
        .output()
        .await?;

    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
    ))
}

