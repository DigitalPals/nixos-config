//! App profile management commands (browsers, Termius, etc.)

use anyhow::Result;
use tokio::sync::mpsc;

use super::errors::{ErrorContext, ParsedError};
use super::executor::{run_capture, run_command};
use super::CommandMessage;

/// Start app backup
pub async fn start_backup(tx: mpsc::Sender<CommandMessage>, force: bool) -> Result<()> {
    tokio::spawn(async move {
        if let Err(e) = run_backup(&tx, force).await {
            tracing::error!("Backup failed: {}", e);
            let _ = tx
                .send(CommandMessage::StepFailed {
                    step: "Backup".to_string(),
                    error: ParsedError::from_stderr(
                        &e.to_string(),
                        ErrorContext {
                            operation: "App backup".to_string(),
                        },
                    ),
                })
                .await;
            let _ = tx.send(CommandMessage::Done { success: false }).await;
        }
    });
    Ok(())
}

/// Start app restore
pub async fn start_restore(tx: mpsc::Sender<CommandMessage>, force: bool) -> Result<()> {
    tokio::spawn(async move {
        if let Err(e) = run_restore(&tx, force).await {
            tracing::error!("Restore failed: {}", e);
            let _ = tx
                .send(CommandMessage::StepFailed {
                    step: "Restore".to_string(),
                    error: ParsedError::from_stderr(
                        &e.to_string(),
                        ErrorContext {
                            operation: "App restore".to_string(),
                        },
                    ),
                })
                .await;
            let _ = tx.send(CommandMessage::Done { success: false }).await;
        }
    });
    Ok(())
}

/// Start app status check
pub async fn start_status(tx: mpsc::Sender<CommandMessage>) -> Result<()> {
    tokio::spawn(async move {
        if let Err(e) = run_status(&tx).await {
            tracing::error!("Status check failed: {}", e);
            let _ = tx.send(CommandMessage::Stderr(e.to_string())).await;
        }
        let _ = tx.send(CommandMessage::Done { success: true }).await;
    });
    Ok(())
}

/// Start parallel background checks for all update types (non-blocking, silent on failure)
pub async fn start_quick_update_check(tx: mpsc::Sender<CommandMessage>) -> Result<()> {
    tokio::spawn(async move {
        // Run both checks in parallel
        let (nixos_result, apps_result) = tokio::join!(
            check_nixos_config_updates(),
            check_app_updates_available(),
        );

        let commits = nixos_result.unwrap_or_default();
        let nixos_config = !commits.is_empty();
        let app_profiles = apps_result.unwrap_or(false);

        // Always send message to clear startup_check_running flag
        let _ = tx
            .send(CommandMessage::UpdatesAvailable {
                nixos_config,
                app_profiles,
                commits,
            })
            .await;
    });
    Ok(())
}

/// Check for nixos-config updates and return pending commits (hash, message)
async fn check_nixos_config_updates() -> Result<Vec<(String, String)>> {
    let config_dir = crate::constants::nixos_config_dir();

    // If no git repo, no updates to check
    if !config_dir.join(".git").exists() {
        return Ok(vec![]);
    }

    // Fetch from remote (with timeout for startup check)
    let fetch_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        run_capture(
            "git",
            &["-C", config_dir.to_str().unwrap_or("."), "fetch", "origin"],
        ),
    )
    .await;

    match fetch_result {
        Ok(Ok((true, _, _))) => {}
        _ => return Ok(vec![]), // Timeout or fetch failed - fail silently
    }

    // Get list of commits on origin/main not in HEAD
    let (ok, log_output, _) = run_capture(
        "git",
        &[
            "-C",
            config_dir.to_str().unwrap_or("."),
            "log",
            "HEAD..origin/main",
            "--pretty=format:%h|%s",
        ],
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

/// Check if remote has newer app profiles (quick, non-blocking)
async fn check_app_updates_available() -> Result<bool> {
    let local_repo = crate::constants::app_backup_data_dir();

    if local_repo.as_os_str().is_empty() {
        return Ok(false);
    }

    // If no local repo, no updates available (user needs to run restore first)
    if !local_repo.join(".git").exists() {
        return Ok(false);
    }

    // Check if remote is configured
    let (remote_ok, _, _) = run_capture(
        "git",
        &[
            "-C",
            local_repo.to_str().unwrap_or("."),
            "remote",
            "get-url",
            "origin",
        ],
    )
    .await?;

    if !remote_ok {
        return Ok(false);
    }

    // Fetch from remote (with short timeout for startup check)
    let fetch_result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        run_capture(
            "git",
            &["-C", local_repo.to_str().unwrap_or("."), "fetch", "origin"],
        ),
    )
    .await;

    let (fetch_ok, _, _) = match fetch_result {
        Ok(Ok(result)) => result,
        _ => return Ok(false), // Timeout or error - fail silently
    };

    if !fetch_ok {
        return Ok(false);
    }

    // Count commits on origin that aren't in local HEAD
    // Try origin/main first, then origin/master
    let (ok, count_str, _) = run_capture(
        "git",
        &[
            "-C",
            local_repo.to_str().unwrap_or("."),
            "rev-list",
            "HEAD..origin/main",
            "--count",
        ],
    )
    .await?;

    let count: usize = if ok {
        count_str.trim().parse().unwrap_or(0)
    } else {
        // Try origin/master as fallback
        let (master_ok, master_count, _) = run_capture(
            "git",
            &[
                "-C",
                local_repo.to_str().unwrap_or("."),
                "rev-list",
                "HEAD..origin/master",
                "--count",
            ],
        )
        .await?;

        if !master_ok {
            return Ok(false);
        }
        master_count.trim().parse().unwrap_or(0)
    };

    // Updates available if there are commits on origin we don't have
    Ok(count > 0)
}

/// Helper to send stdout message
async fn out(tx: &mpsc::Sender<CommandMessage>, msg: &str) {
    let _ = tx.send(CommandMessage::Stdout(msg.to_string())).await;
}

async fn run_backup(tx: &mpsc::Sender<CommandMessage>, force: bool) -> Result<()> {
    out(tx, "").await;
    out(tx, "==============================================").await;
    out(tx, "  App Profile Backup").await;
    out(tx, "==============================================").await;
    out(tx, "").await;

    let mut args = vec!["--push"];
    if force {
        args.push("--force");
    }

    let success = run_command(tx, "app-backup", &args).await?;

    if success {
        out(tx, "").await;
        out(tx, "  App profiles backed up successfully").await;
    } else {
        out(tx, "").await;
        out(tx, "  Backup failed").await;
    }

    out(tx, "").await;
    out(tx, "==============================================").await;

    tx.send(CommandMessage::Done { success }).await?;
    Ok(())
}

async fn run_restore(tx: &mpsc::Sender<CommandMessage>, force: bool) -> Result<()> {
    out(tx, "").await;
    out(tx, "==============================================").await;
    out(tx, "  App Profile Restore").await;
    out(tx, "==============================================").await;
    out(tx, "").await;

    let mut args = vec!["--pull"];
    if force {
        args.push("--force");
    }

    let success = run_command(tx, "app-restore", &args).await?;

    if success {
        out(tx, "").await;
        out(tx, "  App profiles restored successfully").await;
    } else {
        out(tx, "").await;
        out(tx, "  Restore failed").await;
    }

    out(tx, "").await;
    out(tx, "==============================================").await;

    tx.send(CommandMessage::Done { success }).await?;
    Ok(())
}

async fn run_status(tx: &mpsc::Sender<CommandMessage>) -> Result<()> {
    out(tx, "").await;
    out(tx, "==============================================").await;
    out(tx, "  App Profile Status").await;
    out(tx, "==============================================").await;
    out(tx, "").await;

    let local_repo = crate::constants::app_backup_data_dir();

    if !local_repo.join(".git").exists() {
        out(tx, "  Local repository not found.").await;
        out(tx, "  Run 'forge apps restore' to clone.").await;
        out(tx, "").await;
        out(tx, "==============================================").await;
        return Ok(());
    }

    // Check if remote is configured
    let (remote_ok, _, _) = run_capture(
        "git",
        &[
            "-C",
            local_repo.to_str().unwrap_or("."),
            "remote",
            "get-url",
            "origin",
        ],
    )
    .await?;

    if !remote_ok {
        out(tx, "  No remote 'origin' configured").await;
        out(tx, "").await;
        list_local_files(tx, &local_repo).await;
        out(tx, "==============================================").await;
        return Ok(());
    }

    // Fetch from remote (quietly)
    out(tx, "  Checking for updates...").await;
    let (fetch_ok, _, _) = run_capture(
        "git",
        &["-C", local_repo.to_str().unwrap_or("."), "fetch", "origin"],
    )
    .await?;

    if !fetch_ok {
        out(tx, "  Unable to reach remote; showing local status only").await;
        out(tx, "").await;
        list_local_files(tx, &local_repo).await;
        out(tx, "==============================================").await;
        return Ok(());
    }

    // Compare heads
    let (_, local_head, _) = run_capture(
        "git",
        &["-C", local_repo.to_str().unwrap_or("."), "rev-parse", "HEAD"],
    )
    .await?;

    let (remote_ok, remote_head, _) = run_capture(
        "git",
        &[
            "-C",
            local_repo.to_str().unwrap_or("."),
            "rev-parse",
            "origin/main",
        ],
    )
    .await?;

    if !remote_ok {
        // Try origin/master
        let (master_ok, master_head, _) = run_capture(
            "git",
            &[
                "-C",
                local_repo.to_str().unwrap_or("."),
                "rev-parse",
                "origin/master",
            ],
        )
        .await?;

        if !master_ok {
            out(tx, "  Remote branch not found (origin/main or origin/master)").await;
            out(tx, "").await;
            list_local_files(tx, &local_repo).await;
            out(tx, "==============================================").await;
            return Ok(());
        }

        // Use master head
        check_and_show_status(tx, &local_repo, &local_head, &master_head).await?;
    } else {
        check_and_show_status(tx, &local_repo, &local_head, &remote_head).await?;
    }

    out(tx, "").await;
    list_local_files(tx, &local_repo).await;
    out(tx, "==============================================").await;

    Ok(())
}

async fn check_and_show_status(
    tx: &mpsc::Sender<CommandMessage>,
    local_repo: &std::path::Path,
    local_head: &str,
    remote_head: &str,
) -> Result<()> {
    if local_head.trim() == remote_head.trim() {
        out(tx, "").await;
        out(tx, "  App profiles are up to date").await;
    } else {
        out(tx, "").await;
        out(tx, "  Remote has newer profiles").await;
        out(tx, "").await;
        out(tx, "  Remote commits:").await;

        // Get commit log
        let (_, commits, _) = run_capture(
            "git",
            &[
                "-C",
                local_repo.to_str().unwrap_or("."),
                "log",
                "--oneline",
                &format!("{}..{}", local_head.trim(), remote_head.trim()),
            ],
        )
        .await?;

        for line in commits.lines() {
            out(tx, &format!("    {}", line)).await;
        }

        out(tx, "").await;
        out(tx, "  Run 'forge apps restore' to update").await;
    }
    Ok(())
}

async fn list_local_files(tx: &mpsc::Sender<CommandMessage>, local_repo: &std::path::Path) {
    out(tx, "  Local files:").await;

    // Use find to list .age files
    let (success, file_list, _) = run_capture(
        "find",
        &[
            local_repo.to_str().unwrap_or("."),
            "-maxdepth",
            "1",
            "-name",
            "*.age",
            "-exec",
            "ls",
            "-lh",
            "{}",
            ";",
        ],
    )
    .await
    .unwrap_or((false, String::new(), String::new()));

    if success && !file_list.trim().is_empty() {
        for line in file_list.lines() {
            // Extract just filename and size from ls output
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 9 {
                let size = parts[4];
                let filename = parts.last().unwrap_or(&"");
                // Get just the filename without path
                let name = std::path::Path::new(filename)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(filename);
                out(tx, &format!("    {} ({})", name, size)).await;
            }
        }
    } else {
        out(tx, "    (no backup files)").await;
    }
    out(tx, "").await;
}
