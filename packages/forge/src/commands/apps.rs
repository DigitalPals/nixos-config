//! App profile management commands (browsers, Termius, etc.)

use anyhow::Result;
use tokio::sync::mpsc;

use super::executor::run_capture;
use super::runner::{spawn_with_error_handling, CommandRunner};
use super::CommandMessage;

/// Start app backup
pub async fn start_backup(tx: mpsc::Sender<CommandMessage>, force: bool) -> Result<()> {
    spawn_with_error_handling(tx, "App backup", "Backup", move |tx| async move {
        let runner = CommandRunner::new(&tx);
        let args: Vec<&str> = if force {
            vec!["--push", "--force"]
        } else {
            vec!["--push"]
        };
        runner
            .run_simple_operation(
                "App Profile Backup",
                "app-backup",
                &args,
                "App profiles backed up successfully",
                "Backup failed",
            )
            .await?;
        Ok(())
    })
}

/// Start app restore
pub async fn start_restore(tx: mpsc::Sender<CommandMessage>, force: bool) -> Result<()> {
    spawn_with_error_handling(tx, "App restore", "Restore", move |tx| async move {
        let runner = CommandRunner::new(&tx);
        let args: Vec<&str> = if force {
            vec!["--pull", "--force"]
        } else {
            vec!["--pull"]
        };
        runner
            .run_simple_operation(
                "App Profile Restore",
                "app-restore",
                &args,
                "App profiles restored successfully",
                "Restore failed",
            )
            .await?;
        Ok(())
    })
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

async fn run_status(tx: &mpsc::Sender<CommandMessage>) -> Result<()> {
    let runner = CommandRunner::new(tx);
    runner.header("App Profile Status").await;

    let local_repo = crate::constants::app_backup_data_dir();

    if !local_repo.join(".git").exists() {
        runner.out("  Local repository not found.").await;
        runner.out("  Run 'forge apps restore' to clone.").await;
        runner.footer().await;
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
        runner.out("  No remote 'origin' configured").await;
        runner.out("").await;
        list_local_files(&runner, &local_repo).await;
        runner.footer().await;
        return Ok(());
    }

    // Fetch from remote (quietly)
    runner.out("  Checking for updates...").await;
    let (fetch_ok, _, _) = run_capture(
        "git",
        &["-C", local_repo.to_str().unwrap_or("."), "fetch", "origin"],
    )
    .await?;

    if !fetch_ok {
        runner.out("  Unable to reach remote; showing local status only").await;
        runner.out("").await;
        list_local_files(&runner, &local_repo).await;
        runner.footer().await;
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
            runner.out("  Remote branch not found (origin/main or origin/master)").await;
            runner.out("").await;
            list_local_files(&runner, &local_repo).await;
            runner.footer().await;
            return Ok(());
        }

        // Use master head
        check_and_show_status(&runner, &local_repo, &local_head, &master_head).await?;
    } else {
        check_and_show_status(&runner, &local_repo, &local_head, &remote_head).await?;
    }

    runner.out("").await;
    list_local_files(&runner, &local_repo).await;
    runner.footer().await;

    Ok(())
}

async fn check_and_show_status(
    runner: &CommandRunner<'_>,
    local_repo: &std::path::Path,
    local_head: &str,
    remote_head: &str,
) -> Result<()> {
    if local_head.trim() == remote_head.trim() {
        runner.out("").await;
        runner.out("  App profiles are up to date").await;
    } else {
        runner.out("").await;
        runner.out("  Remote has newer profiles").await;
        runner.out("").await;
        runner.out("  Remote commits:").await;

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
            runner.out(&format!("    {}", line)).await;
        }

        runner.out("").await;
        runner.out("  Run 'forge apps restore' to update").await;
    }
    Ok(())
}

async fn list_local_files(runner: &CommandRunner<'_>, local_repo: &std::path::Path) {
    runner.out("  Local files:").await;

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
                runner.out(&format!("    {} ({})", name, size)).await;
            }
        }
    } else {
        runner.out("    (no backup files)").await;
    }
    runner.out("").await;
}
