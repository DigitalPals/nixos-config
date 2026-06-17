//! System update command implementation
//!
//! This module handles the full NixOS system update process:
//! - Flake input updates
//! - System rebuild
//! - Package comparison
//! - CLI tool updates (Claude Code, Codex)
//! - Browser profile status check

pub mod flake;
pub mod history;
mod nvidia;
mod packages;
mod release;
mod tools;

use anyhow::Result;
use regex::Regex;
use std::io::{self, IsTerminal, Write};
use std::process::Stdio;
use std::sync::LazyLock;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::app::{
    state::{CommitInfo, FirmwareUpdateInfo},
    LocalChange, StashInfo, UpdateCoreStatus, UpdateDryRunStatus, UpdateOptions,
    UpdatePreflightReport, UpdateRemoteStatus, UpdateSummary,
};
use crate::commands::errors::{ErrorContext, ParsedError};
use crate::commands::executor::{
    command_exists, get_output, run_capture, run_command_cancellable_transformed,
    run_command_cancellable_transformed_with_timeout, run_command_cancellable_with_timeout,
    CommandResult,
};
use crate::commands::retry::{retry_with_backoff, RetryConfig};
use crate::commands::CommandMessage;

use flake::{get_flake_lock_hash, parse_flake_changes, save_flake_lock_backup};
use packages::{parse_package_changes, PackageCompareResult};
use tools::{check_browser_status, clean_version, get_npm_package_version};

use crate::constants::nixos_config_dir;

const STEP_PULL: &str = "update.pull";
const STEP_FLAKE: &str = "update.flake";
const STEP_REBUILD: &str = "update.rebuild";
const STEP_PACKAGES: &str = "update.packages";
const STEP_DESKTOP_SHELL: &str = "update.desktop-shell";
const STEP_PREFLIGHT_HEALTH: &str = "update.preflight.health";
const STEP_PREFLIGHT_REMOTE: &str = "update.preflight.remote";
const STEP_PREFLIGHT_DRYRUN: &str = "update.preflight.dryrun";
const STEP_CLAUDE: &str = "update.claude";
const STEP_CODEX: &str = "update.codex";
const STEP_BROWSER: &str = "update.browser";
const STEP_FIRMWARE: &str = "update.firmware";
/// Get the current NixOS system generation number
pub async fn get_current_generation() -> Option<u32> {
    let output = get_output("readlink", &["/nix/var/nix/profiles/system"])
        .await
        .ok()?;
    let gen_str = output.trim();
    // Format: system-N-link
    gen_str.split('-').nth(1)?.parse().ok()
}

/// Get the current system symlink target.
pub async fn get_current_system_path() -> Option<String> {
    get_output("readlink", &["/run/current-system"])
        .await
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Check for local uncommitted changes in the NixOS config directory.
/// Returns a list of changed files (empty if no changes).
pub fn check_local_changes() -> Vec<LocalChange> {
    let config_path = nixos_config_dir();

    // Check if this is a git repository
    if !config_path.join(".git").exists() {
        return Vec::new();
    }

    // Run git status --porcelain to get changed files
    let output = std::process::Command::new("git")
        .args([
            "-C",
            &config_path.to_string_lossy(),
            "status",
            "--porcelain",
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout
                .lines()
                .filter(|line| !line.is_empty())
                .map(|line| {
                    let path = if line.len() > 3 {
                        line[3..].to_string()
                    } else {
                        line.to_string()
                    };
                    let status = line.chars().take(2).collect::<String>();
                    LocalChange {
                        path,
                        tracked: status != "??",
                    }
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

/// A dirty flake.lock from a previous update is expected when the next update
/// will refresh all flake inputs again. Other changes still need user action.
pub fn can_regenerate_dirty_flake_lock(options: &UpdateOptions, changes: &[LocalChange]) -> bool {
    !options.rebuild_only
        && options.inputs.is_empty()
        && changes.len() == 1
        && changes[0].tracked
        && changes[0].path == "flake.lock"
}

/// Resolve the current branch upstream, if configured.
pub fn get_upstream_ref(config_path: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args([
            "-C",
            config_path,
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let upstream = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !upstream.is_empty() {
            return Some(upstream);
        }
    }
    None
}

/// Inspect local remote-tracking state without mutating git refs.
pub async fn inspect_remote_status(config_path: &str) -> UpdateRemoteStatus {
    let upstream =
        get_upstream_ref(config_path).unwrap_or_else(|| format!("origin/{}", get_default_branch()));

    let (success, stdout, stderr) = match run_capture(
        "git",
        &[
            "-C",
            config_path,
            "rev-list",
            "--left-right",
            "--count",
            &format!("HEAD...{}", upstream),
        ],
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            return UpdateRemoteStatus {
                upstream: Some(upstream),
                checked: true,
                error: Some(error.to_string()),
                ..UpdateRemoteStatus::default()
            };
        }
    };

    if !success {
        return UpdateRemoteStatus {
            upstream: Some(upstream),
            checked: true,
            error: Some(stderr.trim().to_string()),
            ..UpdateRemoteStatus::default()
        };
    }

    let counts: Vec<&str> = stdout.split_whitespace().collect();
    let ahead = counts
        .first()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let behind = counts
        .get(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    UpdateRemoteStatus {
        upstream: Some(upstream),
        ahead,
        behind,
        checked: true,
        error: None,
    }
}

/// Run a non-mutating system build check before update starts.
async fn run_preflight_dry_run(
    tx: &mpsc::Sender<CommandMessage>,
    cancel: CancellationToken,
    options: &UpdateOptions,
) -> Result<Option<UpdateDryRunStatus>> {
    if options.flake_only {
        return Ok(Some(UpdateDryRunStatus::Skipped(
            "Not needed in flake-input-only mode".to_string(),
        )));
    }

    let hostname = match get_output("hostname", &[]).await {
        Ok(name) if !name.trim().is_empty() => name,
        _ => {
            return Ok(Some(UpdateDryRunStatus::Failed(
                "Could not determine hostname".to_string(),
            )));
        }
    };

    let flake_dir = crate::constants::nixos_config_dir();
    let flake_path = flake_dir.to_string_lossy().to_string();
    let target = format!(
        "{}#nixosConfigurations.{}.config.system.build.toplevel",
        flake_path, hostname
    );

    let _ = tx
        .send(CommandMessage::StepDetail {
            step: STEP_PREFLIGHT_DRYRUN.to_string(),
            detail: format!("Building {}", hostname),
        })
        .await;
    out(tx, &format!("Checking target {}", hostname)).await;

    let result = run_command_cancellable_transformed(
        tx,
        "nix",
        &["build", "--no-link", &target],
        cancel,
        transform_nix_output,
    )
    .await?;

    Ok(match result {
        CommandResult::Cancelled => None,
        CommandResult::Completed(true) => Some(UpdateDryRunStatus::Passed),
        CommandResult::Completed(false) => Some(UpdateDryRunStatus::Failed(
            "Dry-run build failed. See output above for details.".to_string(),
        )),
    })
}

pub async fn start_preflight(
    tx: mpsc::Sender<CommandMessage>,
    cancel: CancellationToken,
    options: UpdateOptions,
    changes: Vec<LocalChange>,
    pending_resolution: Option<crate::app::LocalChangesResolution>,
) -> Result<()> {
    tokio::spawn(async move {
        if let Err(error) = run_preflight(&tx, cancel, &options, &changes, pending_resolution).await
        {
            tracing::error!("Update preflight failed: {}", error);
            let _ = tx
                .send(CommandMessage::UpdatePreflightFailed {
                    error: error.to_string(),
                })
                .await;
        }
    });
    Ok(())
}

async fn run_preflight(
    tx: &mpsc::Sender<CommandMessage>,
    cancel: CancellationToken,
    options: &UpdateOptions,
    changes: &[LocalChange],
    pending_resolution: Option<crate::app::LocalChangesResolution>,
) -> Result<()> {
    out(tx, "").await;
    out(tx, "Preparing update checks...").await;
    out(tx, &format!("Mode: {}", options.mode_label())).await;
    out(
        tx,
        &format!(
            "Local changes: {} tracked, {} untracked",
            changes.iter().filter(|change| change.tracked).count(),
            changes.iter().filter(|change| !change.tracked).count()
        ),
    )
    .await;

    let health =
        crate::commands::health::check_health(crate::commands::health::Operation::Update).await;
    if health.is_ok() {
        out(tx, "Required tools are available").await;
        tx.send(CommandMessage::StepComplete {
            step: STEP_PREFLIGHT_HEALTH.to_string(),
        })
        .await?;
    } else {
        out(tx, "Required tools are missing; review will explain").await;
        for line in health.error_message().lines() {
            out(tx, line).await;
        }
        tx.send(CommandMessage::StepWarning {
            step: STEP_PREFLIGHT_HEALTH.to_string(),
            detail: "Required tools missing".to_string(),
        })
        .await?;
    }

    let config_path = crate::constants::nixos_config_dir();
    let config_str = config_path.to_string_lossy().to_string();
    let remote = if config_path.join(".git").exists() {
        inspect_remote_status(&config_str).await
    } else {
        UpdateRemoteStatus {
            checked: false,
            error: Some("Not a git repository".to_string()),
            ..UpdateRemoteStatus::default()
        }
    };

    match &remote.error {
        Some(error) => {
            out(tx, &format!("Remote status unavailable: {}", error)).await;
            tx.send(CommandMessage::StepWarning {
                step: STEP_PREFLIGHT_REMOTE.to_string(),
                detail: "Remote status unavailable".to_string(),
            })
            .await?;
        }
        None if remote.checked => {
            out(
                tx,
                &format!(
                    "Remote status: {} ahead, {} behind on {}",
                    remote.ahead,
                    remote.behind,
                    remote.upstream.as_deref().unwrap_or("upstream")
                ),
            )
            .await;
            if remote.ahead > 0 || remote.behind > 0 {
                tx.send(CommandMessage::StepWarning {
                    step: STEP_PREFLIGHT_REMOTE.to_string(),
                    detail: "Repository differs from upstream".to_string(),
                })
                .await?;
            } else {
                tx.send(CommandMessage::StepComplete {
                    step: STEP_PREFLIGHT_REMOTE.to_string(),
                })
                .await?;
            }
        }
        None => {
            out(tx, "Remote status not available").await;
            tx.send(CommandMessage::StepSkipped {
                step: STEP_PREFLIGHT_REMOTE.to_string(),
                reason: Some("Remote status was not checked".to_string()),
            })
            .await?;
        }
    }

    let dry_run = if !health.is_ok() {
        out(
            tx,
            "Skipping dry-run build because required tools are missing",
        )
        .await;
        tx.send(CommandMessage::StepSkipped {
            step: STEP_PREFLIGHT_DRYRUN.to_string(),
            reason: Some("Required tools are missing".to_string()),
        })
        .await?;
        UpdateDryRunStatus::Skipped("Required tools missing".to_string())
    } else if matches!(
        pending_resolution,
        Some(crate::app::LocalChangesResolution::Overwrite)
    ) {
        out(
            tx,
            "Skipping dry-run build until local changes are resolved",
        )
        .await;
        tx.send(CommandMessage::StepSkipped {
            step: STEP_PREFLIGHT_DRYRUN.to_string(),
            reason: Some("Local changes will be discarded before updating".to_string()),
        })
        .await?;
        UpdateDryRunStatus::Skipped(
            "Dry run deferred because local changes will be discarded".to_string(),
        )
    } else {
        match run_preflight_dry_run(tx, cancel, options).await? {
            None => {
                out(tx, "Update preflight cancelled").await;
                tx.send(CommandMessage::Cancelled).await?;
                return Ok(());
            }
            Some(UpdateDryRunStatus::Passed) => {
                out(tx, "Dry-run build passed").await;
                tx.send(CommandMessage::StepComplete {
                    step: STEP_PREFLIGHT_DRYRUN.to_string(),
                })
                .await?;
                UpdateDryRunStatus::Passed
            }
            Some(UpdateDryRunStatus::Skipped(reason)) => {
                out(tx, &format!("Dry-run build skipped: {}", reason)).await;
                tx.send(CommandMessage::StepSkipped {
                    step: STEP_PREFLIGHT_DRYRUN.to_string(),
                    reason: Some(reason.clone()),
                })
                .await?;
                UpdateDryRunStatus::Skipped(reason)
            }
            Some(UpdateDryRunStatus::Failed(reason)) => {
                out(tx, &format!("Dry-run build failed: {}", reason)).await;
                tx.send(CommandMessage::StepWarning {
                    step: STEP_PREFLIGHT_DRYRUN.to_string(),
                    detail: "Dry-run build failed".to_string(),
                })
                .await?;
                UpdateDryRunStatus::Failed(reason)
            }
        }
    };

    let report = UpdatePreflightReport {
        missing_required_tools: health
            .missing_required
            .iter()
            .map(|tool| format!("{} ({})", tool.name, tool.install_hint))
            .collect(),
        missing_optional_tools: health
            .missing_optional
            .iter()
            .map(|tool| format!("{} ({})", tool.name, tool.install_hint))
            .collect(),
        tracked_count: changes.iter().filter(|change| change.tracked).count(),
        untracked_count: changes.iter().filter(|change| !change.tracked).count(),
        pending_resolution,
        remote,
        dry_run,
    };

    tx.send(CommandMessage::UpdatePreflightReady { report })
        .await?;
    Ok(())
}

pub async fn find_stash_reference(config_path: &str, message: &str) -> Option<StashInfo> {
    let (success, stdout, _) = run_capture(
        "git",
        &["-C", config_path, "stash", "list", "--format=%gd:%s"],
    )
    .await
    .ok()?;

    if !success {
        return None;
    }

    stdout.lines().find_map(|line| {
        let (reference, stash_message) = line.split_once(':')?;
        if stash_message.contains(message) {
            Some(StashInfo {
                reference: reference.to_string(),
                message: stash_message.to_string(),
            })
        } else {
            None
        }
    })
}

/// Get the default branch name (main or master) for the remote
pub fn get_default_branch() -> String {
    let config_path = nixos_config_dir();

    // Try to get the default branch from remote HEAD
    let output = std::process::Command::new("git")
        .args([
            "-C",
            &config_path.to_string_lossy(),
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
        ])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Format: refs/remotes/origin/main
            if let Some(branch) = stdout.trim().strip_prefix("refs/remotes/origin/") {
                return branch.to_string();
            }
        }
    }

    // Fall back to checking if origin/main exists
    let check_main = std::process::Command::new("git")
        .args([
            "-C",
            &config_path.to_string_lossy(),
            "rev-parse",
            "--verify",
            "origin/main",
        ])
        .output();

    if let Ok(output) = check_main {
        if output.status.success() {
            return "main".to_string();
        }
    }

    // Default to master
    "master".to_string()
}

/// Regex to extract "message" from JSON error responses
static JSON_MESSAGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""message"\s*:\s*"([^"]+)""#).unwrap());

/// Transform nix command output to remove noise and extract useful info from errors
/// Returns None to skip the line, Some(line) to include it (possibly transformed)
fn transform_nix_output(line: &str) -> Option<String> {
    let trimmed = line.trim();

    // Keep intentional empty lines
    if trimmed.is_empty() {
        return Some(line.to_string());
    }

    // Skip "is dirty" warnings
    if line.contains("is dirty") {
        return None;
    }

    // Skip HTML content (error pages from GitHub)
    if trimmed.starts_with("<!DOCTYPE")
        || trimmed.starts_with("<html")
        || trimmed.starts_with("<head")
        || trimmed.starts_with("<body")
        || trimmed.starts_with("<style")
        || trimmed.starts_with("<div")
        || trimmed.starts_with("<title")
        || trimmed.starts_with("<meta")
        || trimmed.starts_with("<link")
        || trimmed.starts_with("<p>")
        || trimmed.starts_with("<ul")
        || trimmed.starts_with("<li")
        || trimmed.starts_with("<a ")
        || trimmed.starts_with("<img")
        || trimmed.starts_with("</")
        || trimmed.starts_with("<!--")
        || trimmed.starts_with("-->")
        || trimmed == "{"
        || trimmed == "}"
        || trimmed == "("
        || trimmed == ")"
    {
        return None;
    }

    // Skip CSS content
    if trimmed.contains("background-color:")
        || trimmed.contains("font-family:")
        || trimmed.contains("text-align:")
        || trimmed.contains("margin:")
        || trimmed.contains("padding:")
        || (trimmed.starts_with(".") && trimmed.contains("{"))
        || (trimmed.starts_with("@media") && trimmed.contains("{"))
    {
        return None;
    }

    // Skip base64 data (long strings without spaces, typically image data)
    if trimmed.len() > 100 && !trimmed.contains(' ') && !trimmed.contains(':') {
        return None;
    }

    // Skip lines that are just closing HTML tags or whitespace with special chars
    if trimmed.starts_with("*/") || trimmed.ends_with("*/") {
        return None;
    }

    // Extract message from JSON error responses
    // e.g., {"message":"API rate limit exceeded..."} -> "  API: API rate limit exceeded..."
    if trimmed.starts_with("{\"") && trimmed.contains("\"message\"") {
        if let Some(caps) = JSON_MESSAGE_RE.captures(trimmed) {
            if let Some(msg) = caps.get(1) {
                // Truncate long messages and clean them up
                let message = msg.as_str();
                let clean_msg = if message.len() > 80 {
                    format!("{}...", &message[..77])
                } else {
                    message.to_string()
                };
                return Some(format!("       → {}", clean_msg));
            }
        }
        // Skip JSON lines we can't parse nicely
        return None;
    }

    // Keep the line as-is
    Some(line.to_string())
}

fn transform_internal_nix_message(line: &str) -> Option<String> {
    let payload = line.trim_start().strip_prefix("@nix ")?;
    let value = serde_json::from_str::<serde_json::Value>(payload).ok()?;
    if value.get("action").and_then(serde_json::Value::as_str) != Some("msg") {
        return None;
    }
    let msg = value.get("msg").and_then(serde_json::Value::as_str)?;
    transform_nix_output(msg)
}

async fn run_capture_with_retry(
    tx: &mpsc::Sender<CommandMessage>,
    step: &str,
    label: &str,
    cmd: &str,
    args: &[&str],
) -> Result<(bool, String, String)> {
    let config = RetryConfig::default();
    retry_with_backoff(tx, step, &config, || async {
        let (success, stdout, stderr) = run_capture(cmd, args).await?;
        if success {
            Ok((success, stdout, stderr))
        } else {
            let detail = stderr.trim();
            if detail.is_empty() {
                anyhow::bail!("{} failed", label);
            } else {
                anyhow::bail!("{} failed: {}", label, detail);
            }
        }
    })
    .await
}

async fn run_flake_update_with_retry(
    tx: &mpsc::Sender<CommandMessage>,
    cancel: CancellationToken,
    flake_args: &[String],
) -> Result<CommandResult> {
    let config = RetryConfig::default();
    let result = retry_with_backoff(tx, STEP_FLAKE, &config, || {
        let args = flake_args.to_vec();
        let tx = tx.clone();
        let cancel = cancel.clone();

        async move {
            let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            let tx_detail = tx.clone();
            let result = run_command_cancellable_transformed(
                &tx,
                "nix",
                &arg_refs,
                cancel,
                move |line: &str| {
                    let trimmed = line.trim();
                    if let Some(rest) = trimmed
                        .strip_prefix("updating input '")
                        .or_else(|| trimmed.strip_prefix("• Updated input '"))
                    {
                        if let Some(name) = rest.split('\'').next() {
                            let detail_msg = format!("Updating {}", name);
                            let tx = tx_detail.clone();
                            tokio::spawn(async move {
                                let _ = tx
                                    .send(CommandMessage::StepDetail {
                                        step: STEP_FLAKE.to_string(),
                                        detail: detail_msg,
                                    })
                                    .await;
                            });
                        }
                    }
                    transform_nix_output(line)
                },
            )
            .await?;

            match result {
                CommandResult::Completed(true) | CommandResult::Cancelled => Ok(result),
                CommandResult::Completed(false) => anyhow::bail!("nix flake update failed"),
            }
        }
    })
    .await;

    match result {
        Ok(result) => Ok(result),
        Err(error) => {
            tracing::warn!("flake update retries exhausted: {}", error);
            Ok(CommandResult::Completed(false))
        }
    }
}

/// Start the update process
pub async fn start_update(
    tx: mpsc::Sender<CommandMessage>,
    cancel: CancellationToken,
    options: crate::app::UpdateOptions,
) -> Result<()> {
    tokio::spawn(async move {
        if let Err(e) = run_update(&tx, cancel, &options).await {
            tracing::error!("Update failed: {}", e);
            let _ = tx
                .send(CommandMessage::StepFailed {
                    step: "Update".to_string(),
                    error: ParsedError::from_stderr(
                        &e.to_string(),
                        ErrorContext {
                            operation: "Update".to_string(),
                        },
                    ),
                })
                .await;
            let _ = tx.send(CommandMessage::Done { success: false }).await;
        }
    });
    Ok(())
}

async fn run_update(
    tx: &mpsc::Sender<CommandMessage>,
    cancel: CancellationToken,
    options: &crate::app::UpdateOptions,
) -> Result<()> {
    let mut summary = UpdateSummary::default();
    summary.system_before = get_current_system_path().await;

    let flake_dir = crate::constants::nixos_config_dir();
    let hostname = match get_output("hostname", &[]).await {
        Ok(h) if !h.is_empty() => h,
        _ => {
            tracing::warn!("Could not get hostname, using 'localhost'");
            "localhost".to_string()
        }
    };

    // Print header
    out(tx, "").await;
    out(tx, "==============================================").await;
    out(tx, "  NixOS System Update").await;
    out(tx, "==============================================").await;
    out(tx, "").await;

    let flake_path = flake_dir.to_str().unwrap_or(".");

    let skip_flake = options.rebuild_only;
    let skip_rebuild = options.flake_only;
    let mut core_mutated = false;
    // Set if the flake update was reverted (e.g. NVIDIA driver not ready for a
    // new kernel); carries the reason and forces the rebuild to be skipped.
    let mut nvidia_revert: Option<String> = None;

    if !skip_flake {
        match pull_config_updates(tx, flake_path).await {
            Ok(config_commits) => {
                if !config_commits.is_empty() {
                    core_mutated = true;
                }
                summary.config_commits = config_commits;
            }
            Err(error) => {
                tracing::warn!("Failed to pull configuration updates: {}", error);
                summary.core_status = UpdateCoreStatus::Partial;
                summary.partial_state =
                    Some("Configuration sync failed before the system was rebuilt.".to_string());
                finalize_update(tx, &summary).await?;
                tx.send(CommandMessage::Done { success: false }).await?;
                return Ok(());
            }
        }
    }

    let flake_nix_path = flake_dir.join("flake.nix");
    let flake_nix_before = tokio::fs::read_to_string(&flake_nix_path).await.ok();
    let lock_before = get_flake_lock_hash(&flake_dir).await;
    let lock_backup = save_flake_lock_backup(&flake_dir).await;

    let needs_rebuild = if !skip_flake {
        out(tx, "").await;
        out(tx, "══════════════════════════════════════════════").await;
        out(tx, "  Updating Flake Inputs").await;
        out(tx, "══════════════════════════════════════════════").await;
        out(tx, "").await;

        let release_result =
            release::update_release_tracked_inputs(&flake_dir, &options.inputs).await;
        let release_updates = match release_result {
            Ok(updates) => updates,
            Err(error) => {
                out(tx, "  ✗ Release input check failed").await;
                summary.core_status = UpdateCoreStatus::Partial;
                summary.partial_state = Some(
                    "Release-tracked flake input check failed before the system switch completed."
                        .to_string(),
                );
                let parsed_error = ParsedError::from_stderr(
                    &error.to_string(),
                    ErrorContext {
                        operation: "Release input update".to_string(),
                    },
                );
                tx.send(CommandMessage::StepFailed {
                    step: STEP_FLAKE.to_string(),
                    error: parsed_error,
                })
                .await?;
                if let Some(backup_path) = lock_backup.as_ref() {
                    let _ = tokio::fs::remove_file(backup_path).await;
                }
                finalize_update(tx, &summary).await?;
                tx.send(CommandMessage::Done { success: false }).await?;
                return Ok(());
            }
        };

        for update in &release_updates {
            out(
                tx,
                &format!(
                    "  ✓ {} release tag {} → {}",
                    update.name, update.old_tag, update.new_tag
                ),
            )
            .await;
        }

        // Build flake update args
        let mut flake_args: Vec<String> = vec!["flake".into(), "update".into()];
        for input in &options.inputs {
            flake_args.push(input.clone());
        }
        flake_args.push("--flake".into());
        flake_args.push(flake_path.to_string());

        let result = run_flake_update_with_retry(tx, cancel.clone(), &flake_args).await?;

        out(tx, "").await;
        match result {
            CommandResult::Cancelled => {
                out(tx, "  ⊘ Flake update cancelled").await;
                restore_flake_nix_if_needed(
                    &flake_nix_path,
                    flake_nix_before.as_deref(),
                    !release_updates.is_empty(),
                )
                .await;
                summary.core_status = UpdateCoreStatus::Cancelled;
                summary.partial_state = Some(
                    "Flake inputs may have changed before the update was cancelled.".to_string(),
                );
                if let Some(backup_path) = lock_backup.as_ref() {
                    let _ = tokio::fs::remove_file(backup_path).await;
                }
                finalize_update(tx, &summary).await?;
                tx.send(CommandMessage::Cancelled).await?;
                return Ok(());
            }
            CommandResult::Completed(false) => {
                out(tx, "  ✗ Flake update failed").await;
                restore_flake_nix_if_needed(
                    &flake_nix_path,
                    flake_nix_before.as_deref(),
                    !release_updates.is_empty(),
                )
                .await;
                summary.core_status = UpdateCoreStatus::Partial;
                summary.partial_state = Some(
                    "Flake input mutation failed before the system switch completed.".to_string(),
                );
                let error = ParsedError::from_stderr(
                    "Flake update failed - see output above for details",
                    ErrorContext {
                        operation: "Flake update".to_string(),
                    },
                );
                tx.send(CommandMessage::StepFailed {
                    step: STEP_FLAKE.to_string(),
                    error,
                })
                .await?;
                if let Some(backup_path) = lock_backup.as_ref() {
                    let _ = tokio::fs::remove_file(backup_path).await;
                }
                finalize_update(tx, &summary).await?;
                tx.send(CommandMessage::Done { success: false }).await?;
                return Ok(());
            }
            CommandResult::Completed(true) => {}
        }
        out(tx, "  ✓ Flake inputs updated").await;
        tx.send(CommandMessage::StepComplete {
            step: STEP_FLAKE.to_string(),
        })
        .await?;

        // Check if flake.lock changed
        let lock_after = get_flake_lock_hash(&flake_dir).await;
        let changed = lock_before != lock_after;

        if changed {
            core_mutated = true;
            if let Some(ref backup_path) = lock_backup {
                summary.flake_changes = parse_flake_changes(&flake_dir, backup_path)
                    .await
                    .unwrap_or_default();
            }

            // NVIDIA driver pre-flight: when a kernel bump lands before the
            // packaged driver supports it, build the configured driver against
            // the new kernel and revert the lock if it won't compile, rather
            // than discovering the failure partway through a full rebuild.
            match nvidia::check_nvidia_compatibility(
                tx,
                &flake_dir,
                &hostname,
                &summary.flake_changes,
                options.skip_nvidia_check,
            )
            .await
            {
                Ok(Some(reason)) => {
                    out(tx, "").await;
                    out(tx, &format!("  ✗ {}", reason)).await;
                    let restored = match lock_backup.as_ref() {
                        Some(backup_path) => {
                            nvidia::restore_flake_lock(&flake_dir, backup_path).await
                        }
                        None => false,
                    };
                    restore_flake_nix_if_needed(
                        &flake_nix_path,
                        flake_nix_before.as_deref(),
                        !release_updates.is_empty(),
                    )
                    .await;
                    if restored {
                        out(tx, "  ↩ Restored flake.lock to the previous inputs").await;
                    } else {
                        out(tx, "  ! Could not restore flake.lock automatically").await;
                    }
                    // Inputs were reverted, so don't report them as updated.
                    summary.flake_changes.clear();
                    nvidia_revert = Some(reason);
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!("NVIDIA compatibility check errored: {}", error);
                    out(
                        tx,
                        "  ! NVIDIA compatibility check could not run; continuing",
                    )
                    .await;
                }
            }

            if nvidia_revert.is_none() && !summary.flake_changes.is_empty() {
                tx.send(CommandMessage::UpdateFlakePreview {
                    changes: summary.flake_changes.clone(),
                })
                .await?;
            }
        }
        if let Some(backup_path) = lock_backup.as_ref() {
            let _ = tokio::fs::remove_file(backup_path).await;
        }
        changed && nvidia_revert.is_none()
    } else {
        true
    };

    if needs_rebuild && !skip_rebuild {
        out(tx, "").await;
        out(tx, "══════════════════════════════════════════════").await;
        out(tx, "  Rebuilding System").await;
        out(tx, "══════════════════════════════════════════════").await;
        out(tx, "").await;

        let config_name = hostname.clone();
        let flake_ref = format!("{}#{}", flake_path, config_name);

        // Capture current generation before rebuild for potential rollback
        let pre_rebuild_gen = get_current_generation().await;

        // Send detail for rebuild phase
        let _ = tx
            .send(CommandMessage::StepDetail {
                step: STEP_REBUILD.to_string(),
                detail: format!("nixos-rebuild switch --flake .#{}", config_name),
            })
            .await;

        // Use configurable timeout for rebuild (default 30 minutes, override with FORGE_REBUILD_TIMEOUT)
        let timeout = crate::constants::rebuild_timeout_secs();
        let result = if options.is_modern() {
            use std::sync::{Arc, Mutex};

            let _ = tx
                .send(CommandMessage::NixProgress(
                    crate::app::NixProgressEvent::Section {
                        title: "Rebuilding NixOS system".to_string(),
                    },
                ))
                .await;

            let parser = Arc::new(Mutex::new(NixProgressParser::default()));
            let tx_progress = tx.clone();
            run_command_cancellable_transformed_with_timeout(
                tx,
                "sudo",
                &[
                    "nixos-rebuild",
                    "--log-format",
                    "internal-json",
                    "switch",
                    "--flake",
                    &flake_ref,
                ],
                Some(timeout),
                cancel.clone(),
                move |line: &str| {
                    let events = parser
                        .lock()
                        .map(|mut parser| parser.parse_line(line))
                        .unwrap_or_default();

                    if !events.is_empty() {
                        let tx = tx_progress.clone();
                        tokio::spawn(async move {
                            for event in events {
                                let _ = tx.send(CommandMessage::NixProgress(event)).await;
                            }
                        });
                    }

                    if line.trim_start().starts_with("@nix ") {
                        transform_internal_nix_message(line)
                    } else {
                        transform_nix_output(line)
                    }
                },
            )
            .await?
        } else {
            run_command_cancellable_with_timeout(
                tx,
                "sudo",
                &["nixos-rebuild", "switch", "--flake", &flake_ref],
                Some(timeout),
                cancel.clone(),
            )
            .await?
        };

        out(tx, "").await;
        match result {
            CommandResult::Cancelled => {
                out(tx, "  ⊘ System rebuild cancelled").await;
                summary.core_status = UpdateCoreStatus::Cancelled;
                summary.partial_state = Some(
                    "The system switch was cancelled after update changes were prepared."
                        .to_string(),
                );
                summary.system_after = get_current_system_path().await;
                finalize_update(tx, &summary).await?;
                tx.send(CommandMessage::Cancelled).await?;
                return Ok(());
            }
            CommandResult::Completed(true) => {
                out(tx, "  ✓ System rebuilt successfully").await;
                summary.system_after = get_current_system_path().await;
                core_mutated = true;
                tx.send(CommandMessage::StepComplete {
                    step: STEP_REBUILD.to_string(),
                })
                .await?;
            }
            CommandResult::Completed(false) => {
                out(tx, "  ✗ System rebuild failed").await;
                summary.rebuild_failed = true;
                summary.core_status = UpdateCoreStatus::Partial;
                summary.partial_state = Some(
                    "Configuration or inputs changed, but the system switch did not complete."
                        .to_string(),
                );
                summary.system_after = get_current_system_path().await;
                let error = ParsedError::from_stderr(
                    "System rebuild failed - see output above for details",
                    ErrorContext {
                        operation: "System rebuild".to_string(),
                    },
                );
                tx.send(CommandMessage::StepFailed {
                    step: STEP_REBUILD.to_string(),
                    error,
                })
                .await?;

                // Offer rollback if we captured the pre-rebuild generation
                if let Some(gen) = pre_rebuild_gen {
                    tx.send(CommandMessage::RollbackAvailable { generation: gen })
                        .await?;
                }
            }
        }
    } else if let Some(reason) = nvidia_revert.as_ref() {
        out(tx, "").await;
        out(
            tx,
            "  ⊘ Skipping rebuild: NVIDIA driver not ready for the new kernel",
        )
        .await;
        summary.rebuild_skipped = true;
        summary.system_after = summary.system_before.clone();
        summary.core_status = UpdateCoreStatus::Partial;
        summary.partial_state = Some(format!(
            "Kernel update reverted ({}); the system stayed on the previous inputs.",
            reason
        ));
        summary.follow_up_warnings.push(format!(
            "NVIDIA driver not ready for the new kernel ({reason}); kernel update reverted. \
             Retry later, or run with --skip-nvidia-check to override."
        ));
        tx.send(CommandMessage::StepSkipped {
            step: STEP_REBUILD.to_string(),
            reason: Some(format!("NVIDIA driver not ready: {reason}")),
        })
        .await?;
    } else if !skip_rebuild {
        out(tx, "").await;
        out(tx, "  - Skipping rebuild (no changes)").await;
        summary.rebuild_skipped = true;
        summary.system_after = summary.system_before.clone();
        tx.send(CommandMessage::StepSkipped {
            step: STEP_REBUILD.to_string(),
            reason: Some("flake.lock did not change".to_string()),
        })
        .await?;
    }

    if !skip_rebuild {
        out(tx, "").await;
        out(tx, "  Comparing packages...").await;
        if summary.system_after.is_none() {
            summary.system_after = get_current_system_path().await;
        }
        let pkg_result = parse_package_changes(
            summary.system_before.as_deref(),
            summary.system_after.as_deref(),
            tx,
        )
        .await
        .unwrap_or_else(|_| PackageCompareResult::default());
        summary.package_changes = pkg_result.changes;
        summary.packages_added = pkg_result.added;
        summary.packages_removed = pkg_result.removed;
        summary.closure_summary = pkg_result.closure_summary;

        if summary.package_changes.is_empty() {
            out(tx, "  - No package version changes").await;
        } else {
            out(
                tx,
                &format!("  ✓ {} packages updated", summary.package_changes.len()),
            )
            .await;
        }
        tx.send(CommandMessage::StepComplete {
            step: STEP_PACKAGES.to_string(),
        })
        .await?;

        check_desktop_shell_launchers(tx, &mut summary).await?;
    }

    if summary.core_status == UpdateCoreStatus::Pending {
        summary.core_status = if summary.rebuild_failed {
            UpdateCoreStatus::Partial
        } else if skip_rebuild {
            if summary.flake_changes.is_empty() && !core_mutated {
                UpdateCoreStatus::UpToDate
            } else {
                UpdateCoreStatus::Success
            }
        } else if summary.rebuild_skipped {
            if summary.flake_changes.is_empty() && !core_mutated {
                UpdateCoreStatus::UpToDate
            } else {
                UpdateCoreStatus::Success
            }
        } else {
            UpdateCoreStatus::Success
        };
    }

    if matches!(
        summary.core_status,
        UpdateCoreStatus::Success | UpdateCoreStatus::UpToDate
    ) && !summary.rebuild_skipped
    {
        summary.reboot_reasons = detect_reboot_reasons(&summary.package_changes).await;
    }

    if !skip_flake
        && !skip_rebuild
        && matches!(
            summary.core_status,
            UpdateCoreStatus::Success | UpdateCoreStatus::UpToDate
        )
    {
        update_claude_code(tx, &mut summary).await?;
        update_codex_cli(tx, &mut summary).await?;
        check_app_profiles(tx, &mut summary).await?;
        check_firmware_updates(tx, &mut summary, options).await?;
    }

    finalize_update(tx, &summary).await?;
    tx.send(CommandMessage::Done {
        success: matches!(
            summary.core_status,
            UpdateCoreStatus::Success | UpdateCoreStatus::UpToDate
        ),
    })
    .await?;

    Ok(())
}

async fn update_claude_code(
    tx: &mpsc::Sender<CommandMessage>,
    summary: &mut UpdateSummary,
) -> Result<()> {
    let claude_path = crate::constants::claude_cli_path();

    if claude_path.exists() {
        let claude_cmd = claude_path.to_str().unwrap_or("claude");
        summary.claude_old = get_output(claude_cmd, &["--version"])
            .await
            .ok()
            .map(|v| clean_version(&v));

        let (success, _stdout, _stderr) = match run_capture_with_retry(
            tx,
            STEP_CLAUDE,
            "Claude Code update",
            claude_cmd,
            &["update"],
        )
        .await
        {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!("Claude Code update retries exhausted: {}", error);
                (false, String::new(), error.to_string())
            }
        };

        if success {
            out(tx, "  ✓ Updating Claude Code").await;
            tx.send(CommandMessage::StepComplete {
                step: STEP_CLAUDE.to_string(),
            })
            .await?;
        } else {
            let warning = "Claude Code update failed".to_string();
            out(
                tx,
                "  ! Claude Code update failed (system update still succeeded)",
            )
            .await;
            tx.send(CommandMessage::StepWarning {
                step: STEP_CLAUDE.to_string(),
                detail: warning.clone(),
            })
            .await?;
            summary.follow_up_warnings.push(warning);
        }

        summary.claude_new = get_output(claude_cmd, &["--version"])
            .await
            .ok()
            .map(|v| clean_version(&v));
    } else {
        out(tx, "  - Claude Code not installed").await;
        tx.send(CommandMessage::StepSkipped {
            step: STEP_CLAUDE.to_string(),
            reason: Some(format!("{} was not found", claude_path.display())),
        })
        .await?;
    }

    Ok(())
}

async fn check_desktop_shell_launchers(
    tx: &mpsc::Sender<CommandMessage>,
    summary: &mut UpdateSummary,
) -> Result<()> {
    out(tx, "  Checking desktop shell launchers...").await;

    let mut actions = Vec::new();
    let mut warnings = Vec::new();
    let timestamp = timestamp_suffix();

    if let Some(home) = home_dir() {
        cleanup_stale_local_bin(&home, "lumen", &timestamp, &mut actions, &mut warnings).await;
        cleanup_stale_local_bin(
            &home,
            "lumen-settings",
            &timestamp,
            &mut actions,
            &mut warnings,
        )
        .await;

        if cleanup_stale_lumen_systemd_override(&home, &timestamp, &mut actions, &mut warnings)
            .await
        {
            if let Err(error) = run_capture("systemctl", &["--user", "daemon-reload"]).await {
                warnings.push(format!("systemd user daemon-reload failed: {error}"));
            }

            match run_capture("systemctl", &["--user", "restart", "lumen.service"]).await {
                Ok((true, _, _)) => actions.push("restarted lumen.service".to_string()),
                Ok((false, _, stderr)) => warnings.push(format!(
                    "could not restart lumen.service after cleanup: {}",
                    stderr.trim()
                )),
                Err(error) => warnings.push(format!(
                    "could not restart lumen.service after cleanup: {error}"
                )),
            }
        }

        cleanup_direct_profile_entry(&home, "wayle", &mut actions, &mut warnings).await;
        cleanup_direct_profile_entry(&home, "lumen", &mut actions, &mut warnings).await;
    } else {
        warnings.push("HOME is not set; could not check user launch shadows".to_string());
    }

    if warnings.is_empty() {
        if actions.is_empty() {
            out(tx, "  ✓ Desktop shell launchers clean").await;
            summary.desktop_shell_status = "clean".to_string();
        } else {
            out(
                tx,
                &format!(
                    "  ✓ Removed {} stale desktop shell shadow(s)",
                    actions.len()
                ),
            )
            .await;
            summary.desktop_shell_status = format!("{} stale shadow(s) removed", actions.len());
            for action in &actions {
                out(tx, &format!("    {}", action)).await;
            }
        }
        tx.send(CommandMessage::StepComplete {
            step: STEP_DESKTOP_SHELL.to_string(),
        })
        .await?;
    } else {
        out(tx, "  ! Desktop shell launcher cleanup needs attention").await;
        summary.desktop_shell_status = "needs attention".to_string();
        for warning in &warnings {
            out(tx, &format!("    {}", warning)).await;
            summary.follow_up_warnings.push(warning.clone());
        }
        tx.send(CommandMessage::StepWarning {
            step: STEP_DESKTOP_SHELL.to_string(),
            detail: warnings.join("; "),
        })
        .await?;
    }

    Ok(())
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn timestamp_suffix() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

fn is_stale_lumen_target(target: &std::path::Path) -> bool {
    let target = target.to_string_lossy();
    target.contains("/Code/Lumen/target/release/")
        || target.contains("/Code/Wayle/target/release/")
        || target.ends_with("/target/release/lumen")
        || target.ends_with("/target/release/lumen-settings")
        || target.ends_with("/target/release/wayle")
        || target.ends_with("/target/release/wayle-settings")
}

async fn cleanup_stale_local_bin(
    home: &std::path::Path,
    name: &str,
    timestamp: &str,
    actions: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let path = home.join(".local/bin").join(name);
    let Ok(metadata) = tokio::fs::symlink_metadata(&path).await else {
        return;
    };
    if !metadata.file_type().is_symlink() {
        return;
    }

    let target = match tokio::fs::read_link(&path).await {
        Ok(target) => target,
        Err(error) => {
            warnings.push(format!("could not inspect {}: {error}", path.display()));
            return;
        }
    };
    if !is_stale_lumen_target(&target) {
        return;
    }

    let backup_dir = home
        .join(".local")
        .join(format!("bin.disabled-{timestamp}"));
    if let Err(error) = tokio::fs::create_dir_all(&backup_dir).await {
        warnings.push(format!(
            "could not create backup directory {}: {error}",
            backup_dir.display()
        ));
        return;
    }
    let backup_path = backup_dir.join(name);
    match tokio::fs::rename(&path, &backup_path).await {
        Ok(()) => actions.push(format!(
            "moved {} -> {}",
            path.display(),
            backup_path.display()
        )),
        Err(error) => warnings.push(format!(
            "could not move stale {} to {}: {error}",
            path.display(),
            backup_path.display()
        )),
    }
}

async fn cleanup_stale_lumen_systemd_override(
    home: &std::path::Path,
    timestamp: &str,
    actions: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> bool {
    let dropin_dir = home.join(".config/systemd/user/lumen.service.d");
    let override_path = dropin_dir.join("override.conf");
    let content = match tokio::fs::read_to_string(&override_path).await {
        Ok(content) => content,
        Err(_) => return false,
    };
    if !content.contains("/Code/Lumen/target/release/")
        && !content.contains("/Code/Wayle/target/release/")
        && !content.contains("target/release/lumen")
        && !content.contains("target/release/wayle")
    {
        return false;
    }

    let disabled_dir = home
        .join(".config/systemd/user")
        .join(format!("lumen.service.d.disabled-{timestamp}"));
    if let Err(error) = tokio::fs::create_dir_all(&disabled_dir).await {
        warnings.push(format!(
            "could not create systemd override backup {}: {error}",
            disabled_dir.display()
        ));
        return false;
    }
    let backup_path = disabled_dir.join("override.conf");
    match tokio::fs::rename(&override_path, &backup_path).await {
        Ok(()) => {
            let _ = tokio::fs::remove_dir(&dropin_dir).await;
            actions.push(format!(
                "disabled stale lumen.service override at {}",
                backup_path.display()
            ));
            true
        }
        Err(error) => {
            warnings.push(format!(
                "could not disable stale lumen.service override {}: {error}",
                override_path.display()
            ));
            false
        }
    }
}

async fn cleanup_direct_profile_entry(
    home: &std::path::Path,
    name: &str,
    actions: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let profile = home.join(".local/state/nix/profiles/profile");
    if !profile.exists() {
        return;
    }
    let profile_arg = profile.to_string_lossy().to_string();
    let Ok((true, stdout, _)) = run_capture(
        "nix",
        &["profile", "list", "--json", "--profile", &profile_arg],
    )
    .await
    else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&stdout) else {
        return;
    };
    let Some(elements) = value.get("elements").and_then(serde_json::Value::as_object) else {
        return;
    };
    if !elements.contains_key(name) {
        return;
    }

    match run_capture(
        "nix",
        &["profile", "remove", name, "--profile", &profile_arg],
    )
    .await
    {
        Ok((true, _, _)) => actions.push(format!("removed direct nix profile entry {name}")),
        Ok((false, _, stderr)) => warnings.push(format!(
            "could not remove direct nix profile entry {name}: {}",
            stderr.trim()
        )),
        Err(error) => warnings.push(format!(
            "could not remove direct nix profile entry {name}: {error}"
        )),
    }
}

async fn update_codex_cli(
    tx: &mpsc::Sender<CommandMessage>,
    summary: &mut UpdateSummary,
) -> Result<()> {
    let codex_path = crate::constants::codex_cli_path();

    if codex_path.exists() {
        summary.codex_old = get_npm_package_version("@openai/codex").await;

        let (success, _stdout, _stderr) = match run_capture_with_retry(
            tx,
            STEP_CODEX,
            "Codex CLI update",
            "npm",
            &["update", "-g", "@openai/codex"],
        )
        .await
        {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!("Codex CLI update retries exhausted: {}", error);
                (false, String::new(), error.to_string())
            }
        };

        if success {
            out(tx, "  ✓ Updating Codex CLI").await;
            tx.send(CommandMessage::StepComplete {
                step: STEP_CODEX.to_string(),
            })
            .await?;
        } else {
            let warning = "Codex CLI update failed".to_string();
            out(
                tx,
                "  ! Codex CLI update failed (system update still succeeded)",
            )
            .await;
            tx.send(CommandMessage::StepWarning {
                step: STEP_CODEX.to_string(),
                detail: warning.clone(),
            })
            .await?;
            summary.follow_up_warnings.push(warning);
        }

        summary.codex_new = get_npm_package_version("@openai/codex").await;
    } else {
        out(tx, "  - Codex CLI not installed").await;
        tx.send(CommandMessage::StepSkipped {
            step: STEP_CODEX.to_string(),
            reason: Some(format!("{} was not found", codex_path.display())),
        })
        .await?;
    }

    Ok(())
}

async fn check_app_profiles(
    tx: &mpsc::Sender<CommandMessage>,
    summary: &mut UpdateSummary,
) -> Result<()> {
    if command_exists("app-restore").await {
        let config_path = crate::constants::app_backup_config_path();

        if config_path.exists() {
            summary.browser_status = check_browser_status()
                .await
                .unwrap_or_else(|_| "unknown".to_string());
            if summary.browser_status == "up to date" {
                out(tx, "  ✓ Browser profiles up to date").await;
            } else {
                out(
                    tx,
                    &format!("  ! Browser profiles status: {}", summary.browser_status),
                )
                .await;
                summary.follow_up_warnings.push(format!(
                    "Browser profiles need attention: {}",
                    summary.browser_status
                ));
                tx.send(CommandMessage::StepWarning {
                    step: STEP_BROWSER.to_string(),
                    detail: summary.browser_status.clone(),
                })
                .await?;
                return Ok(());
            }
        } else {
            summary.browser_status = "not configured".to_string();
            out(tx, "  - Browser profiles not configured").await;
            tx.send(CommandMessage::StepSkipped {
                step: STEP_BROWSER.to_string(),
                reason: Some(format!("{} does not exist", config_path.display())),
            })
            .await?;
            return Ok(());
        }

        tx.send(CommandMessage::StepComplete {
            step: STEP_BROWSER.to_string(),
        })
        .await?;
    } else {
        summary.browser_status = "not configured".to_string();
        out(tx, "  - App backup not configured").await;
        tx.send(CommandMessage::StepSkipped {
            step: STEP_BROWSER.to_string(),
            reason: Some("app-restore command was not found".to_string()),
        })
        .await?;
    }

    Ok(())
}

async fn check_firmware_updates(
    tx: &mpsc::Sender<CommandMessage>,
    summary: &mut UpdateSummary,
    options: &UpdateOptions,
) -> Result<()> {
    // Check if fwupdmgr exists
    if !command_exists("fwupdmgr").await {
        out(tx, "  - fwupd not installed").await;
        tx.send(CommandMessage::StepSkipped {
            step: STEP_FIRMWARE.to_string(),
            reason: Some("fwupdmgr command was not found".to_string()),
        })
        .await?;
        return Ok(());
    }

    // Refresh metadata from LVFS
    out(tx, "  Refreshing firmware metadata...").await;
    let _ = run_capture_with_retry(
        tx,
        STEP_FIRMWARE,
        "Firmware metadata refresh",
        "fwupdmgr",
        &["refresh"],
    )
    .await;

    // Check for available updates (exit code 2 = no updates)
    let output = Command::new("fwupdmgr")
        .args(["get-updates", "--json"])
        .output()
        .await?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let status_code = output.status.code().unwrap_or(-1);

    if status_code == 2 {
        out(tx, "  ✓ Firmware is up to date").await;
        summary.firmware_status = "up to date".to_string();
    } else if output.status.success() {
        summary.firmware_updates = parse_firmware_updates_json(&stdout);
        let count = summary
            .firmware_updates
            .len()
            .max(count_firmware_updates(&stdout));
        out(tx, &format!("  ! {} firmware update(s) available", count)).await;
        summary.firmware_status = format!("{} update(s) available", count);

        if options.is_modern()
            && io::stdin().is_terminal()
            && io::stdout().is_terminal()
            && prompt_apply_firmware_updates(&summary.firmware_updates).await?
        {
            out(tx, "  Applying firmware updates...").await;
            let success = Command::new("fwupdmgr")
                .args(["update", "--assume-yes"])
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .await?
                .success();

            if success {
                summary.firmware_status = format!("{} update(s) applied", count);
                out(tx, "  ✓ Firmware updates applied").await;
                tx.send(CommandMessage::StepComplete {
                    step: STEP_FIRMWARE.to_string(),
                })
                .await?;
                return Ok(());
            }

            let warning = "Firmware update failed".to_string();
            summary.firmware_status = "update failed".to_string();
            summary.follow_up_warnings.push(warning.clone());
            out(tx, "  ! Firmware update failed").await;
            tx.send(CommandMessage::StepWarning {
                step: STEP_FIRMWARE.to_string(),
                detail: warning,
            })
            .await?;
            return Ok(());
        }

        out(tx, "    Run 'fwupdmgr update' to apply").await;
        summary
            .follow_up_warnings
            .push(format!("{} firmware update(s) available", count));
        tx.send(CommandMessage::StepWarning {
            step: STEP_FIRMWARE.to_string(),
            detail: summary.firmware_status.clone(),
        })
        .await?;
        return Ok(());
    } else {
        let warning = if stderr.trim().is_empty() {
            "Firmware check failed".to_string()
        } else {
            format!("Firmware check failed: {}", stderr.trim())
        };
        out(tx, "  ! Firmware check failed").await;
        tx.send(CommandMessage::StepWarning {
            step: STEP_FIRMWARE.to_string(),
            detail: warning.clone(),
        })
        .await?;
        summary.firmware_status = "check failed".to_string();
        summary.follow_up_warnings.push(warning);
        return Ok(());
    }

    tx.send(CommandMessage::StepComplete {
        step: STEP_FIRMWARE.to_string(),
    })
    .await?;
    Ok(())
}

async fn prompt_apply_firmware_updates(updates: &[FirmwareUpdateInfo]) -> Result<bool> {
    let updates = updates.to_vec();
    tokio::task::spawn_blocking(move || -> Result<bool> {
        println!();
        println!("❯ Firmware updates");
        if updates.is_empty() {
            println!("  ├ ! firmware updates are available");
        } else {
            for update in &updates {
                println!("  ├ ! {}", firmware_update_label(update));
            }
        }
        print!("  └ ? Apply firmware updates now? [y/N] ");
        io::stdout().flush()?;

        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        println!();

        Ok(matches!(
            answer.trim().to_ascii_lowercase().as_str(),
            "y" | "yes"
        ))
    })
    .await?
}

fn firmware_update_label(update: &FirmwareUpdateInfo) -> String {
    let version = match (
        update.current_version.as_deref(),
        update.new_version.as_deref(),
    ) {
        (Some(current), Some(new)) if current != new => format!("{current} → {new}"),
        (_, Some(new)) => new.to_string(),
        _ => "version unknown".to_string(),
    };

    match update.release.as_deref() {
        Some(release) if release != update.device => {
            format!("{} — {} ({})", update.device, release, version)
        }
        _ => format!("{} ({})", update.device, version),
    }
}

fn parse_firmware_updates_json(output: &str) -> Vec<FirmwareUpdateInfo> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(output) else {
        return Vec::new();
    };
    let Some(devices) = value.get("Devices").and_then(|devices| devices.as_array()) else {
        return Vec::new();
    };

    devices
        .iter()
        .filter_map(|device| {
            let releases = device.get("Releases")?.as_array()?;
            let release = releases.first()?;
            Some(FirmwareUpdateInfo {
                device: json_string(device, "Name").unwrap_or_else(|| "Unknown device".to_string()),
                current_version: json_string(device, "Version"),
                new_version: json_string(release, "Version"),
                release: json_string(release, "Name"),
            })
        })
        .collect()
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn count_firmware_updates(output: &str) -> usize {
    // Count device headers (lines ending with ":" that aren't indented)
    output
        .lines()
        .filter(|l| !l.starts_with(' ') && l.trim().ends_with(':'))
        .count()
        .max(1) // At least 1 if we got here
}

async fn detect_reboot_reasons(package_changes: &[(String, String, String)]) -> Vec<String> {
    let mut reasons = Vec::new();

    if let (Ok(booted), Ok(current)) = (
        get_output("readlink", &["/run/booted-system/kernel"]).await,
        get_output("readlink", &["/run/current-system/kernel"]).await,
    ) {
        if booted.trim() != current.trim() {
            reasons.push("Kernel updated".to_string());
        }
    }

    let mut bootloader_changed = false;
    let mut firmware_changed = false;

    for (pkg, _, _) in package_changes {
        let name = pkg.to_lowercase();
        if name.contains("limine") || name.contains("grub") || name.contains("refind") {
            bootloader_changed = true;
        }
        if name.contains("linux-firmware") || name.contains("firmware") || name == "fwupd" {
            firmware_changed = true;
        }
    }

    if bootloader_changed {
        reasons.push("Bootloader updated".to_string());
    }
    if firmware_changed {
        reasons.push("Firmware updated".to_string());
    }

    reasons
}

async fn finalize_update(tx: &mpsc::Sender<CommandMessage>, summary: &UpdateSummary) -> Result<()> {
    output_summary(tx, summary).await?;

    if let Err(error) = history::save_record(
        summary,
        matches!(
            summary.core_status,
            UpdateCoreStatus::Success | UpdateCoreStatus::UpToDate
        ),
    ) {
        tracing::warn!("Failed to save update history: {}", error);
    }

    tx.send(CommandMessage::UpdateSummaryData {
        summary: summary.clone(),
    })
    .await?;

    Ok(())
}

async fn restore_flake_nix_if_needed(
    flake_nix_path: &std::path::Path,
    backup_content: Option<&str>,
    should_restore: bool,
) {
    if !should_restore {
        return;
    }

    let Some(content) = backup_content else {
        tracing::warn!(
            "Could not restore {} because no backup content was captured",
            flake_nix_path.display()
        );
        return;
    };

    if let Err(error) = tokio::fs::write(flake_nix_path, content).await {
        tracing::warn!("Failed to restore {}: {}", flake_nix_path.display(), error);
    }
}

async fn output_summary(tx: &mpsc::Sender<CommandMessage>, summary: &UpdateSummary) -> Result<()> {
    out(tx, "").await;
    out(tx, "╔══════════════════════════════════════════════╗").await;
    out(tx, "║            Update Summary                    ║").await;
    out(tx, "╚══════════════════════════════════════════════╝").await;

    // Flake changes with commit messages
    if !summary.flake_changes.is_empty() {
        out(tx, "").await;
        out(tx, "  Flake inputs updated:").await;
        for change in &summary.flake_changes {
            out(tx, "").await;
            if change.total_commits > 0 {
                out(
                    tx,
                    &format!(
                        "  {} ({} commit{}):",
                        change.name,
                        change.total_commits,
                        if change.total_commits == 1 { "" } else { "s" }
                    ),
                )
                .await;

                // Show commit messages
                for commit in &change.commits {
                    out(tx, &format!("    {} {}", commit.hash, commit.message)).await;
                }

                // If there are more commits than shown, add a link
                if change.total_commits > change.commits.len() {
                    if let Some(ref url) = change.compare_url {
                        out(
                            tx,
                            &format!(
                                "    ... and {} more → {}",
                                change.total_commits - change.commits.len(),
                                url
                            ),
                        )
                        .await;
                    }
                }
            } else {
                // No commits fetched (API failed or other issue)
                out(
                    tx,
                    &format!(
                        "  {}: {} → {}",
                        change.name,
                        &change.old_rev[..7.min(change.old_rev.len())],
                        &change.new_rev[..7.min(change.new_rev.len())]
                    ),
                )
                .await;
                if let Some(ref url) = change.compare_url {
                    out(tx, &format!("    → {}", url)).await;
                }
            }
        }
    }

    // CLI tool updates
    let claude_updated = summary.claude_old.is_some()
        && summary.claude_new.is_some()
        && summary.claude_old != summary.claude_new;
    let codex_updated = summary.codex_old.is_some()
        && summary.codex_new.is_some()
        && summary.codex_old != summary.codex_new;

    if claude_updated || codex_updated {
        out(tx, "").await;
        out(tx, "  CLI tools updated:").await;
        if claude_updated {
            out(
                tx,
                &format!(
                    "    Claude Code: {} → {}",
                    summary.claude_old.as_deref().unwrap_or(""),
                    summary.claude_new.as_deref().unwrap_or("")
                ),
            )
            .await;
        }
        if codex_updated {
            out(
                tx,
                &format!(
                    "    Codex CLI: {} → {}",
                    summary.codex_old.as_deref().unwrap_or(""),
                    summary.codex_new.as_deref().unwrap_or("")
                ),
            )
            .await;
        }
    }

    // Package changes and closure summary
    if !summary.package_changes.is_empty() {
        out(tx, "").await;
        out(tx, "  Packages changed:").await;
        for (pkg, old, new) in &summary.package_changes {
            out(tx, &format!("    {}: {} → {}", pkg, old, new)).await;
        }
    }

    // Added packages
    if !summary.packages_added.is_empty() {
        out(tx, "").await;
        out(
            tx,
            &format!("  Packages added ({}):", summary.packages_added.len()),
        )
        .await;
        for (pkg, ver) in &summary.packages_added {
            if ver.is_empty() {
                out(tx, &format!("    + {}", pkg)).await;
            } else {
                out(tx, &format!("    + {} {}", pkg, ver)).await;
            }
        }
    }

    // Removed packages
    if !summary.packages_removed.is_empty() {
        out(tx, "").await;
        out(
            tx,
            &format!("  Packages removed ({}):", summary.packages_removed.len()),
        )
        .await;
        for (pkg, ver) in &summary.packages_removed {
            if ver.is_empty() {
                out(tx, &format!("    - {}", pkg)).await;
            } else {
                out(tx, &format!("    - {} {}", pkg, ver)).await;
            }
        }
    }

    // Show closure summary (especially useful when no version changes)
    if let Some(ref closure) = summary.closure_summary {
        out(tx, "").await;
        out(tx, &format!("  Closure: {}", closure)).await;
    }

    // Status section
    out(tx, "").await;
    out(tx, "  ─────────────────────────────────────────").await;
    out(tx, "").await;

    let system_status = match summary.core_status {
        UpdateCoreStatus::Pending => "Pending".to_string(),
        UpdateCoreStatus::Success => "Updated successfully".to_string(),
        UpdateCoreStatus::UpToDate => "Already up to date".to_string(),
        UpdateCoreStatus::Partial => "Partially updated".to_string(),
        UpdateCoreStatus::Cancelled => "Cancelled".to_string(),
    };
    out(tx, &format!("  System:      {}", system_status)).await;

    if let Some(partial_state) = &summary.partial_state {
        out(tx, &format!("  State:       {}", partial_state)).await;
    }

    if let (Some(before), Some(after)) = (&summary.system_before, &summary.system_after) {
        if before != after {
            out(tx, "  Snapshot:    before/after system changed").await;
        } else {
            out(tx, "  Snapshot:    before/after system unchanged").await;
        }
    }

    // Show versions that weren't updated
    if summary.claude_old.is_some() && !claude_updated {
        out(
            tx,
            &format!(
                "  Claude Code: {}",
                summary.claude_new.as_deref().unwrap_or("")
            ),
        )
        .await;
    }
    if summary.codex_old.is_some() && !codex_updated {
        out(
            tx,
            &format!(
                "  Codex CLI:   {}",
                summary.codex_new.as_deref().unwrap_or("")
            ),
        )
        .await;
    }

    // Browser status
    if !summary.browser_status.is_empty() {
        out(tx, &format!("  Browser:     {}", summary.browser_status)).await;
    }

    // Desktop shell launcher status
    if !summary.desktop_shell_status.is_empty() {
        out(
            tx,
            &format!("  Desktop:     {}", summary.desktop_shell_status),
        )
        .await;
    }

    // Firmware status
    if !summary.firmware_status.is_empty() {
        out(tx, &format!("  Firmware:    {}", summary.firmware_status)).await;
    }

    if !summary.follow_up_warnings.is_empty() {
        out(tx, "").await;
        out(tx, "  Follow-up warnings:").await;
        for warning in &summary.follow_up_warnings {
            out(tx, &format!("    ! {}", warning)).await;
        }
    }

    out(tx, "").await;
    out(tx, "══════════════════════════════════════════════").await;

    Ok(())
}

/// Pull configuration updates from remote repository
async fn pull_config_updates(
    tx: &mpsc::Sender<CommandMessage>,
    config_path: &str,
) -> Result<Vec<CommitInfo>> {
    // Check if this is a git repository
    let git_dir = std::path::Path::new(config_path).join(".git");
    if !git_dir.exists() {
        out(tx, "  - Not a git repository, skipping pull").await;
        tx.send(CommandMessage::StepSkipped {
            step: STEP_PULL.to_string(),
            reason: Some(format!("{} is not a git repository", config_path)),
        })
        .await?;
        return Ok(Vec::new());
    }

    let upstream =
        get_upstream_ref(config_path).unwrap_or_else(|| format!("origin/{}", get_default_branch()));
    let fetch_remote = upstream.split('/').next().unwrap_or("origin").to_string();

    // Fetch from the configured upstream remote
    let fetch_args = ["-C", config_path, "fetch", fetch_remote.as_str()];
    let fetch_result = run_capture_with_retry(tx, STEP_PULL, "Git fetch", "git", &fetch_args).await;
    let fetch_ok = fetch_result.is_ok();

    if !fetch_ok {
        out(tx, "  - Unable to fetch from remote").await;
        tx.send(CommandMessage::StepSkipped {
            step: STEP_PULL.to_string(),
            reason: Some(format!("git fetch {} failed", fetch_remote)),
        })
        .await?;
        return Ok(Vec::new());
    }

    // Check if flake.lock has local modifications
    let (diff_ok, diff_output, _) = run_capture(
        "git",
        &["-C", config_path, "diff", "--name-only", "flake.lock"],
    )
    .await?;
    let flake_lock_modified = diff_ok && !diff_output.trim().is_empty();

    let (count_ok, count_str, _) = run_capture(
        "git",
        &[
            "-C",
            config_path,
            "rev-list",
            &format!("HEAD..{}", upstream),
            "--count",
        ],
    )
    .await?;
    let count = if count_ok {
        count_str.trim().parse().unwrap_or(0)
    } else {
        0
    };

    if count == 0 {
        out(tx, "  - No configuration updates to pull").await;
        tx.send(CommandMessage::StepSkipped {
            step: STEP_PULL.to_string(),
            reason: Some(format!("{} has no commits ahead of HEAD", upstream)),
        })
        .await?;
        return Ok(Vec::new());
    }

    let (log_ok, log_output, _) = run_capture(
        "git",
        &[
            "-C",
            config_path,
            "log",
            &format!("HEAD..{}", upstream),
            "--pretty=format:%h%x00%s",
        ],
    )
    .await?;
    let commits = if log_ok {
        log_output
            .lines()
            .filter_map(|line| {
                let (hash, message) = line.split_once('\0')?;
                Some(CommitInfo {
                    hash: hash.to_string(),
                    message: message.to_string(),
                })
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    // Reset flake.lock to remote version if it has local modifications
    // This is safe because nix flake update will regenerate it anyway
    if flake_lock_modified {
        out(tx, "  - Resetting flake.lock to remote version").await;
        let (reset_ok, _, _) = run_capture(
            "git",
            &["-C", config_path, "checkout", &upstream, "--", "flake.lock"],
        )
        .await?;

        if !reset_ok {
            out(tx, "  ✗ Failed to reset flake.lock").await;
            // Non-fatal, continue and let pull fail naturally if needed
        }
    }

    // Pull the updates
    let pull_result = run_capture_with_retry(
        tx,
        STEP_PULL,
        "Git pull",
        "git",
        &["-C", config_path, "pull", "--ff-only"],
    )
    .await;

    if pull_result.is_ok() {
        out(tx, &format!("  ✓ Pulled {} commit(s)", count)).await;
        for commit in &commits {
            out(tx, &format!("    {} {}", commit.hash, commit.message)).await;
        }
        tx.send(CommandMessage::StepComplete {
            step: STEP_PULL.to_string(),
        })
        .await?;
        return Ok(commits);
    } else {
        out(tx, "  ✗ Failed to pull configuration updates").await;
        let stderr = pull_result
            .err()
            .map(|error| error.to_string())
            .unwrap_or_else(|| "git pull failed".to_string());
        let error = ParsedError::from_stderr(
            &stderr,
            ErrorContext {
                operation: "Git pull".to_string(),
            },
        );
        tx.send(CommandMessage::StepFailed {
            step: STEP_PULL.to_string(),
            error,
        })
        .await?;
        anyhow::bail!("git pull failed");
    }
}

/// Helper to send stdout message
pub(crate) async fn out(tx: &mpsc::Sender<CommandMessage>, msg: &str) {
    let _ = tx.send(CommandMessage::Stdout(msg.to_string())).await;
}

#[derive(Debug, Default)]
struct NixProgressParser {
    activities: std::collections::HashMap<u64, NixActivity>,
    download_parents: std::collections::HashMap<u64, u64>,
}

#[derive(Debug)]
struct NixActivity {
    name: String,
    started_at: std::time::Instant,
}

impl NixProgressParser {
    fn parse_line(&mut self, line: &str) -> Vec<crate::app::NixProgressEvent> {
        let Some(payload) = line.trim().strip_prefix("@nix ") else {
            return parse_nix_plain_progress_line(line);
        };

        let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
            return Vec::new();
        };

        let action = value
            .get("action")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let id = value.get("id").and_then(serde_json::Value::as_u64);

        match action {
            "start" => self.parse_start(id, &value),
            "result" => self.parse_result(id, &value),
            "stop" => self.parse_stop(id),
            "msg" => value
                .get("msg")
                .and_then(serde_json::Value::as_str)
                .map(parse_nix_plain_progress_line)
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn parse_start(
        &mut self,
        id: Option<u64>,
        value: &serde_json::Value,
    ) -> Vec<crate::app::NixProgressEvent> {
        let Some(id) = id else {
            return Vec::new();
        };
        let event_type = value
            .get("type")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        if event_type == 100 {
            if let Some(path) = value
                .get("fields")
                .and_then(serde_json::Value::as_array)
                .and_then(|fields| fields.first())
                .and_then(serde_json::Value::as_str)
            {
                let name = display_name_from_store_path(path);
                self.activities.insert(
                    id,
                    NixActivity {
                        name: name.clone(),
                        started_at: std::time::Instant::now(),
                    },
                );
                return vec![crate::app::NixProgressEvent::Download {
                    name,
                    transferred: 0,
                    total: None,
                    speed_bps: None,
                    eta_secs: None,
                }];
            }
        }

        if event_type == 101 {
            if let Some(parent) = value.get("parent").and_then(serde_json::Value::as_u64) {
                self.download_parents.insert(id, parent);
            }
        }

        let text = value
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let events = parse_nix_plain_progress_line(text);
        if let Some(crate::app::NixProgressEvent::Building { name }) = events.first() {
            self.activities.insert(
                id,
                NixActivity {
                    name: name.clone(),
                    started_at: std::time::Instant::now(),
                },
            );
        }
        events
    }

    fn parse_result(
        &mut self,
        id: Option<u64>,
        value: &serde_json::Value,
    ) -> Vec<crate::app::NixProgressEvent> {
        let Some(id) = id else {
            return Vec::new();
        };
        let Some(parent_id) = self.download_parents.get(&id).copied() else {
            return Vec::new();
        };
        let Some(activity) = self.activities.get(&parent_id) else {
            return Vec::new();
        };

        let Some(fields) = value.get("fields").and_then(serde_json::Value::as_array) else {
            return Vec::new();
        };
        let transferred = fields
            .first()
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let total = fields
            .get(1)
            .and_then(serde_json::Value::as_u64)
            .filter(|total| *total > 0);

        let elapsed = activity.started_at.elapsed().as_secs_f64();
        let speed_bps = (elapsed > 0.2 && transferred > 0).then_some(transferred as f64 / elapsed);
        let eta_secs = match (total, speed_bps) {
            (Some(total), Some(speed)) if speed > 0.0 && total > transferred => {
                Some(((total - transferred) as f64 / speed).ceil() as u64)
            }
            _ => None,
        };

        vec![crate::app::NixProgressEvent::Download {
            name: activity.name.clone(),
            transferred,
            total,
            speed_bps,
            eta_secs,
        }]
    }

    fn parse_stop(&mut self, id: Option<u64>) -> Vec<crate::app::NixProgressEvent> {
        let Some(id) = id else {
            return Vec::new();
        };

        if let Some(parent_id) = self.download_parents.remove(&id) {
            if let Some(activity) = self.activities.get(&parent_id) {
                return vec![crate::app::NixProgressEvent::Complete {
                    name: activity.name.clone(),
                }];
            }
        }

        if let Some(activity) = self.activities.remove(&id) {
            return vec![crate::app::NixProgressEvent::Complete {
                name: activity.name,
            }];
        }

        Vec::new()
    }
}

fn parse_nix_plain_progress_line(line: &str) -> Vec<crate::app::NixProgressEvent> {
    let lower = line.to_ascii_lowercase();
    if lower.contains("activating the configuration")
        || lower.contains("activating new configuration")
        || lower.contains("activating new system")
    {
        return vec![crate::app::NixProgressEvent::Activating];
    }

    if let Some(path) = quoted_store_path_after(line, "building ") {
        return vec![crate::app::NixProgressEvent::Building {
            name: display_name_from_store_path(path),
        }];
    }

    if let Some(path) = quoted_store_path_after(line, "error: builder for ") {
        return vec![crate::app::NixProgressEvent::Failed {
            name: display_name_from_store_path(path),
        }];
    }

    Vec::new()
}

fn quoted_store_path_after<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let lower = line.to_ascii_lowercase();
    let pos = lower.find(prefix)?;
    let rest = &line[pos + prefix.len()..];
    let start = rest.find("/nix/store/")?;
    let rest = &rest[start..];
    let end = rest
        .find('\'')
        .or_else(|| rest.find('"'))
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

fn display_name_from_store_path(path: &str) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    if base.len() > 33 && base.as_bytes().get(32) == Some(&b'-') {
        base[33..].to_string()
    } else {
        base.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_download_progress_with_total() {
        let mut parser = NixProgressParser::default();
        let start = r#"@nix {"action":"start","fields":["/nix/store/3pcn0admdqrrd0k405y9hyad4579wgb3-hello-2.12.3","https://cache.nixos.org","local"],"id":10,"parent":9,"type":100}"#;
        let download = r#"@nix {"action":"start","fields":["https://cache/nar.xz"],"id":11,"parent":10,"type":101}"#;
        let progress = r#"@nix {"action":"result","fields":[50,100,0,0],"id":11,"type":105}"#;

        parser.parse_line(start);
        parser.parse_line(download);
        let events = parser.parse_line(progress);

        match &events[0] {
            crate::app::NixProgressEvent::Download {
                name,
                transferred,
                total,
                ..
            } => {
                assert_eq!(name, "hello-2.12.3");
                assert_eq!(*transferred, 50);
                assert_eq!(*total, Some(100));
            }
            _ => panic!("expected download event"),
        }
    }

    #[test]
    fn omits_unknown_total() {
        let mut parser = NixProgressParser::default();
        parser.parse_line(r#"@nix {"action":"start","fields":["/nix/store/3pcn0admdqrrd0k405y9hyad4579wgb3-hello-2.12.3"],"id":10,"type":100}"#);
        parser.parse_line(r#"@nix {"action":"start","id":11,"parent":10,"type":101}"#);
        let events = parser.parse_line(r#"@nix {"action":"result","fields":[50,0,0,0],"id":11}"#);

        match &events[0] {
            crate::app::NixProgressEvent::Download { total, .. } => assert_eq!(*total, None),
            _ => panic!("expected download event"),
        }
    }

    #[test]
    fn parses_building_line() {
        let mut parser = NixProgressParser::default();
        let events = parser
            .parse_line("building '/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-linux-7.0.2.drv'");

        match &events[0] {
            crate::app::NixProgressEvent::Building { name } => {
                assert_eq!(name, "linux-7.0.2.drv")
            }
            _ => panic!("expected building event"),
        }
    }

    #[test]
    fn completes_internal_json_build_activity() {
        let mut parser = NixProgressParser::default();
        let start = r#"@nix {"action":"start","id":10,"level":5,"parent":0,"text":"building '/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-linux-7.0.2.drv'","type":0}"#;
        let stop = r#"@nix {"action":"stop","id":10}"#;

        let start_events = parser.parse_line(start);
        let stop_events = parser.parse_line(stop);

        match &start_events[0] {
            crate::app::NixProgressEvent::Building { name } => {
                assert_eq!(name, "linux-7.0.2.drv")
            }
            _ => panic!("expected building event"),
        }

        match &stop_events[0] {
            crate::app::NixProgressEvent::Complete { name } => {
                assert_eq!(name, "linux-7.0.2.drv")
            }
            _ => panic!("expected complete event"),
        }
    }
}
