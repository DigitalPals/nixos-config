//! Key management commands (Age and SSH keys)

use anyhow::Result;
use tokio::sync::mpsc;

use super::executor::run_command;
use super::CommandMessage;

/// Start key setup from 1Password
pub async fn start_setup(tx: mpsc::Sender<CommandMessage>) -> Result<()> {
    tokio::spawn(async move {
        if let Err(e) = run_setup(&tx).await {
            tracing::error!("Key setup failed: {}", e);
            let _ = tx
                .send(CommandMessage::StepFailed {
                    step: "Setup".to_string(),
                    error: e.to_string(),
                })
                .await;
            let _ = tx.send(CommandMessage::Done { success: false }).await;
        }
    });
    Ok(())
}

/// Start key backup
pub async fn start_backup(tx: mpsc::Sender<CommandMessage>) -> Result<()> {
    tokio::spawn(async move {
        if let Err(e) = run_backup(&tx).await {
            tracing::error!("Key backup failed: {}", e);
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

/// Start key restore
pub async fn start_restore(tx: mpsc::Sender<CommandMessage>, force: bool) -> Result<()> {
    tokio::spawn(async move {
        if let Err(e) = run_restore(&tx, force).await {
            tracing::error!("Key restore failed: {}", e);
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

/// Start key status check
pub async fn start_status(tx: mpsc::Sender<CommandMessage>) -> Result<()> {
    tokio::spawn(async move {
        if let Err(e) = run_status(&tx).await {
            tracing::error!("Key status check failed: {}", e);
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

async fn run_setup(tx: &mpsc::Sender<CommandMessage>) -> Result<()> {
    out(tx, "").await;
    out(tx, "==============================================").await;
    out(tx, "  Key Setup (from 1Password)").await;
    out(tx, "==============================================").await;
    out(tx, "").await;

    let success = run_command(tx, "keys-setup", &[]).await?;

    if success {
        out(tx, "").await;
        out(tx, "  Keys set up successfully").await;
    } else {
        out(tx, "").await;
        out(tx, "  Setup failed").await;
    }

    out(tx, "").await;
    out(tx, "==============================================").await;

    tx.send(CommandMessage::Done { success }).await?;
    Ok(())
}

async fn run_backup(tx: &mpsc::Sender<CommandMessage>) -> Result<()> {
    out(tx, "").await;
    out(tx, "==============================================").await;
    out(tx, "  Key Backup").await;
    out(tx, "==============================================").await;
    out(tx, "").await;

    let success = run_command(tx, "keys-backup", &["--push"]).await?;

    if success {
        out(tx, "").await;
        out(tx, "  Keys backed up successfully").await;
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
    out(tx, "  Key Restore").await;
    out(tx, "==============================================").await;
    out(tx, "").await;

    let mut args = vec!["--pull"];
    if force {
        args.push("--force");
    }

    let success = run_command(tx, "keys-restore", &args).await?;

    if success {
        out(tx, "").await;
        out(tx, "  Keys restored successfully").await;
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
    out(tx, "  Key Status").await;
    out(tx, "==============================================").await;
    out(tx, "").await;

    let success = run_command(tx, "keys-status", &[]).await?;

    out(tx, "").await;
    out(tx, "==============================================").await;

    tx.send(CommandMessage::Done { success }).await?;
    Ok(())
}
