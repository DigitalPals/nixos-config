//! Browser profile management commands

use anyhow::Result;
use tokio::sync::mpsc;

use super::executor::{run_capture, run_command};
use super::CommandMessage;

/// Start browser backup
pub async fn start_backup(tx: mpsc::Sender<CommandMessage>, force: bool) -> Result<()> {
    tokio::spawn(async move {
        if let Err(e) = run_backup(&tx, force).await {
            tracing::error!("Backup failed: {}", e);
            let _ = tx
                .send(CommandMessage::StepFailed {
                    step: "Backup".to_string(),
                    error: e.to_string(),
                })
                .await;
            let _ = tx.send(CommandMessage::Done { success: false }).await;
        }
    });
    Ok(())
}

/// Start browser restore
pub async fn start_restore(tx: mpsc::Sender<CommandMessage>, force: bool) -> Result<()> {
    tokio::spawn(async move {
        if let Err(e) = run_restore(&tx, force).await {
            tracing::error!("Restore failed: {}", e);
            let _ = tx
                .send(CommandMessage::StepFailed {
                    step: "Restore".to_string(),
                    error: e.to_string(),
                })
                .await;
            let _ = tx.send(CommandMessage::Done { success: false }).await;
        }
    });
    Ok(())
}

/// Start browser status check
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

/// Helper to send stdout message
async fn out(tx: &mpsc::Sender<CommandMessage>, msg: &str) {
    let _ = tx.send(CommandMessage::Stdout(msg.to_string())).await;
}

async fn run_backup(tx: &mpsc::Sender<CommandMessage>, force: bool) -> Result<()> {
    out(tx, "").await;
    out(tx, "==============================================").await;
    out(tx, "  Browser Profile Backup").await;
    out(tx, "==============================================").await;
    out(tx, "").await;

    let mut args = vec!["--push"];
    if force {
        args.push("--force");
    }

    let success = run_command(tx, "browser-backup", &args).await?;

    if success {
        out(tx, "").await;
        out(tx, "  ✓ Browser profiles backed up successfully").await;
    } else {
        out(tx, "").await;
        out(tx, "  ✗ Backup failed").await;
    }

    out(tx, "").await;
    out(tx, "==============================================").await;

    tx.send(CommandMessage::Done { success }).await?;
    Ok(())
}

async fn run_restore(tx: &mpsc::Sender<CommandMessage>, force: bool) -> Result<()> {
    out(tx, "").await;
    out(tx, "==============================================").await;
    out(tx, "  Browser Profile Restore").await;
    out(tx, "==============================================").await;
    out(tx, "").await;

    let mut args = vec!["--pull"];
    if force {
        args.push("--force");
    }

    let success = run_command(tx, "browser-restore", &args).await?;

    if success {
        out(tx, "").await;
        out(tx, "  ✓ Browser profiles restored successfully").await;
    } else {
        out(tx, "").await;
        out(tx, "  ✗ Restore failed").await;
    }

    out(tx, "").await;
    out(tx, "==============================================").await;

    tx.send(CommandMessage::Done { success }).await?;
    Ok(())
}

async fn run_status(tx: &mpsc::Sender<CommandMessage>) -> Result<()> {
    out(tx, "").await;
    out(tx, "==============================================").await;
    out(tx, "  Browser Profile Status").await;
    out(tx, "==============================================").await;
    out(tx, "").await;

    let local_repo = dirs::home_dir()
        .map(|h| h.join(".local/share/browser-backup"))
        .unwrap_or_default();

    if !local_repo.join(".git").exists() {
        out(tx, "  Local repository not found.").await;
        out(tx, "  Run 'forge browser restore' to clone.").await;
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
        out(tx, "  ⚠ No remote 'origin' configured").await;
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
        out(tx, "  ⚠ Unable to reach remote; showing local status only").await;
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
            out(tx, "  ⚠ Remote branch not found (origin/main or origin/master)").await;
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
        out(tx, "  ✓ Browser profiles are up to date").await;
    } else {
        out(tx, "").await;
        out(tx, "  ⚠ Remote has newer profiles").await;
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
        out(tx, "  Run 'forge browser restore' to update").await;
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
