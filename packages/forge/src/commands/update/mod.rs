//! System update command implementation
//!
//! This module handles the full NixOS system update process:
//! - Flake input updates
//! - System rebuild
//! - Package comparison
//! - CLI tool updates (Claude Code, Codex)
//! - Browser profile status check

mod flake;
mod packages;
mod tools;

use anyhow::Result;
use tokio::sync::mpsc;

use crate::app::UpdateSummary;
use crate::commands::executor::{command_exists, get_output, run_capture};
use crate::commands::CommandMessage;

use flake::{get_flake_lock_hash, parse_flake_changes};
use packages::parse_package_changes_from_history;
use tools::{check_browser_status, clean_version, get_npm_package_version};

/// Start the update process
pub async fn start_update(tx: mpsc::Sender<CommandMessage>) -> Result<()> {
    tokio::spawn(async move {
        if let Err(e) = run_update(&tx).await {
            tracing::error!("Update failed: {}", e);
            let _ = tx
                .send(CommandMessage::StepFailed {
                    step: "Update".to_string(),
                    error: e.to_string(),
                })
                .await;
            let _ = tx.send(CommandMessage::Done { success: false }).await;
        }
    });
    Ok(())
}

async fn run_update(tx: &mpsc::Sender<CommandMessage>) -> Result<()> {
    let mut summary = UpdateSummary::default();

    // Find the flake directory
    let flake_dir = crate::constants::nixos_config_dir();

    // Get hostname
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

    // Save flake.lock hash before update
    let lock_before = get_flake_lock_hash(&flake_dir).await;
    let flake_path = flake_dir.to_str().unwrap_or(".");

    // Step 1: Flake update
    let (success, _stdout, _stderr) =
        run_capture("nix", &["flake", "update", flake_path]).await?;

    if !success {
        out(tx, "  ✗ Updating flake inputs").await;
        tx.send(CommandMessage::StepFailed {
            step: "flake".to_string(),
            error: "Flake update failed".to_string(),
        })
        .await?;
        tx.send(CommandMessage::Done { success: false }).await?;
        return Ok(());
    }
    out(tx, "  ✓ Updating flake inputs").await;
    tx.send(CommandMessage::StepComplete {
        step: "flake".to_string(),
    })
    .await?;

    // Check if flake.lock changed
    let lock_after = get_flake_lock_hash(&flake_dir).await;
    let needs_rebuild = lock_before != lock_after;

    if needs_rebuild {
        summary.flake_changes = parse_flake_changes(&flake_dir).await.unwrap_or_default();
    }

    // Step 2: Rebuild (only if needed)
    if needs_rebuild {
        let config_name = hostname.clone();
        let flake_ref = format!("{}#{}", flake_path, config_name);
        let (success, _stdout, _stderr) =
            run_capture("sudo", &["nixos-rebuild", "switch", "--flake", &flake_ref]).await?;

        if success {
            out(tx, "  ✓ Rebuilding system").await;
            tx.send(CommandMessage::StepComplete {
                step: "Rebuild".to_string(),
            })
            .await?;
        } else {
            out(tx, "  ✗ Rebuilding system").await;
            summary.rebuild_failed = true;
            tx.send(CommandMessage::StepFailed {
                step: "Rebuild".to_string(),
                error: "Rebuild failed".to_string(),
            })
            .await?;
        }
    } else {
        out(tx, "  - Skipping rebuild (no changes)").await;
        summary.rebuild_skipped = true;
        tx.send(CommandMessage::StepSkipped {
            step: "Rebuild".to_string(),
        })
        .await?;
    }

    // Step 3: Compare packages
    out(tx, "").await;
    out(tx, "  Comparing packages...").await;
    summary.package_changes = parse_package_changes_from_history(tx).await.unwrap_or_default();

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
        step: "Packages".to_string(),
    })
    .await?;

    // Step 4: Update Claude Code
    update_claude_code(tx, &mut summary).await?;

    // Step 5: Update Codex CLI
    update_codex_cli(tx, &mut summary).await?;

    // Step 6: Check app profiles
    check_app_profiles(tx, &mut summary).await?;

    // Output summary
    output_summary(tx, &summary).await?;

    tx.send(CommandMessage::Done {
        success: !summary.rebuild_failed,
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

        let (success, _stdout, _stderr) = run_capture(claude_cmd, &["update"]).await?;

        if success {
            out(tx, "  ✓ Updating Claude Code").await;
        } else {
            out(tx, "  ✗ Updating Claude Code").await;
        }

        summary.claude_new = get_output(claude_cmd, &["--version"])
            .await
            .ok()
            .map(|v| clean_version(&v));

        tx.send(CommandMessage::StepComplete {
            step: "Claude".to_string(),
        })
        .await?;
    } else {
        out(tx, "  - Claude Code not installed").await;
        tx.send(CommandMessage::StepSkipped {
            step: "Claude".to_string(),
        })
        .await?;
    }

    Ok(())
}

async fn update_codex_cli(
    tx: &mpsc::Sender<CommandMessage>,
    summary: &mut UpdateSummary,
) -> Result<()> {
    let codex_path = crate::constants::codex_cli_path();

    if codex_path.exists() {
        summary.codex_old = get_npm_package_version("@openai/codex").await;

        let (success, _stdout, _stderr) =
            run_capture("npm", &["update", "-g", "@openai/codex"]).await?;

        if success {
            out(tx, "  ✓ Updating Codex CLI").await;
        } else {
            out(tx, "  ✗ Updating Codex CLI").await;
        }

        summary.codex_new = get_npm_package_version("@openai/codex").await;

        tx.send(CommandMessage::StepComplete {
            step: "Codex".to_string(),
        })
        .await?;
    } else {
        out(tx, "  - Codex CLI not installed").await;
        tx.send(CommandMessage::StepSkipped {
            step: "Codex".to_string(),
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
            summary.browser_status =
                check_browser_status().await.unwrap_or_else(|_| "unknown".to_string());
            out(tx, "  ✓ Browser profiles up to date").await;
        } else {
            summary.browser_status = "not configured".to_string();
            out(tx, "  - Browser profiles not configured").await;
        }

        tx.send(CommandMessage::StepComplete {
            step: "browser".to_string(),
        })
        .await?;
    } else {
        summary.browser_status = "not configured".to_string();
        out(tx, "  - App backup not configured").await;
        tx.send(CommandMessage::StepSkipped {
            step: "browser".to_string(),
        })
        .await?;
    }

    Ok(())
}

async fn output_summary(tx: &mpsc::Sender<CommandMessage>, summary: &UpdateSummary) -> Result<()> {
    out(tx, "").await;
    out(tx, "==============================================").await;
    out(tx, "  Update Summary").await;
    out(tx, "==============================================").await;

    // Flake changes
    if !summary.flake_changes.is_empty() {
        out(tx, "").await;
        out(tx, "  Flake inputs updated:").await;
        for (input, old, new) in &summary.flake_changes {
            out(tx, &format!("    {}: {} → {}", input, old, new)).await;
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

    // Package changes
    if !summary.package_changes.is_empty() {
        out(tx, "").await;
        out(tx, "  Packages changed:").await;
        for (pkg, old, new) in &summary.package_changes {
            out(tx, &format!("    {}: {} → {}", pkg, old, new)).await;
        }
    }

    // Status section
    out(tx, "").await;
    out(tx, "  ─────────────────────────────────────────").await;
    out(tx, "").await;

    // System status
    if summary.rebuild_failed {
        out(tx, "  System:      Rebuild failed").await;
    } else if summary.rebuild_skipped {
        out(tx, "  System:      Already up to date").await;
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

    out(tx, "").await;
    out(tx, "==============================================").await;

    Ok(())
}

/// Helper to send stdout message
pub(crate) async fn out(tx: &mpsc::Sender<CommandMessage>, msg: &str) {
    let _ = tx.send(CommandMessage::Stdout(msg.to_string())).await;
}
