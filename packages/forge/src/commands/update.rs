//! System update command implementation

use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::mpsc;

use super::executor::{command_exists, get_output, run_capture};
use super::CommandMessage;
use crate::app::UpdateSummary;

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

    // Find the flake directory - prefer /etc/nixos symlink, fall back to ~/nixos-config
    let flake_dir = if std::path::Path::new("/etc/nixos/flake.nix").exists() {
        PathBuf::from("/etc/nixos")
    } else {
        dirs::home_dir()
            .map(|h| h.join("nixos-config"))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    };

    let log_path = dirs::home_dir()
        .map(|h| h.join("update.log"))
        .unwrap_or_else(|| PathBuf::from("/tmp/update.log"));
    summary.log_path = log_path.display().to_string();

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

    // Step 1: Flake update (run quietly, just show result)
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
        // Parse flake changes
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

    // Step 3: Compare packages (always run - compare current to previous generation)
    out(tx, "").await;
    out(tx, "  Comparing packages...").await;
    summary.package_changes = parse_package_changes_from_history(tx).await.unwrap_or_default();

    if summary.package_changes.is_empty() {
        out(tx, "  - No package version changes").await;
    } else {
        out(tx, &format!("  ✓ {} packages updated", summary.package_changes.len())).await;
    }
    tx.send(CommandMessage::StepComplete {
        step: "Packages".to_string(),
    })
    .await?;

    // Step 4: Update Claude Code
    let claude_path = dirs::home_dir()
        .map(|h| h.join(".local/bin/claude"))
        .unwrap_or_default();

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

    // Step 4: Update Codex CLI
    let codex_path = dirs::home_dir()
        .map(|h| h.join(".npm-global/bin/codex"))
        .unwrap_or_default();

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

    // Step 5: Check app profiles
    if command_exists("app-restore").await {
        let config_path = dirs::home_dir()
            .map(|h| h.join(".config/app-backup/config"))
            .unwrap_or_default();

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

    // Output summary
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
    out(tx, &format!("  Log: {}", summary.log_path)).await;
    out(tx, "").await;
    out(tx, "==============================================").await;

    tx.send(CommandMessage::Done {
        success: !summary.rebuild_failed,
    })
    .await?;

    Ok(())
}

/// Helper to send stdout message
async fn out(tx: &mpsc::Sender<CommandMessage>, msg: &str) {
    let _ = tx.send(CommandMessage::Stdout(msg.to_string())).await;
}

/// Clean version strings (remove duplicate labels)
fn clean_version(v: &str) -> String {
    v.lines()
        .next()
        .unwrap_or("")
        .replace(" (Claude Code)", "")
        .replace(" (Codex)", "")
        .trim()
        .to_string()
}

async fn get_flake_lock_hash(dir: &std::path::Path) -> Option<String> {
    let lock_path = dir.join("flake.lock");
    if lock_path.exists() {
        let (_, stdout, _) = run_capture("sha256sum", &[lock_path.to_str()?])
            .await
            .ok()?;
        Some(stdout.split_whitespace().next()?.to_string())
    } else {
        None
    }
}

async fn parse_flake_changes(dir: &std::path::Path) -> Result<Vec<(String, String, String)>> {
    // TODO: Implement flake.lock change parsing
    // This would require:
    // 1. Saving the old flake.lock before update (e.g., to /tmp/flake.lock.old)
    // 2. Parsing both old and new JSON structures with serde_json
    // 3. Comparing node revisions for each input in the "nodes" object
    // 4. Returning (input_name, old_rev[0..7], new_rev[0..7]) tuples
    //
    // For now, the UI shows "Flake inputs updated:" but no details.
    let lock_path = dir.join("flake.lock");
    if !lock_path.exists() {
        return Ok(Vec::new());
    }

    tracing::debug!("parse_flake_changes: not yet implemented, returning empty");
    Ok(Vec::new())
}

/// Compare current system generation to previous generation using nvd
async fn parse_package_changes_from_history(
    tx: &mpsc::Sender<CommandMessage>,
) -> Result<Vec<(String, String, String)>> {
    // Get current generation number from /nix/var/nix/profiles/system
    let current_gen = match get_output("readlink", &["/nix/var/nix/profiles/system"]).await {
        Ok(s) => s.trim().to_string(),
        Err(_) => {
            out(tx, "    Could not read current generation").await;
            return Ok(Vec::new());
        }
    };

    // Extract generation number (format: system-N-link -> we want N)
    // The symlink points to system-N-link
    let gen_num: u32 = current_gen
        .split('-')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if gen_num <= 1 {
        out(tx, "    No previous generation to compare").await;
        return Ok(Vec::new());
    }

    let prev_gen = gen_num - 1;
    let current_path = format!("/nix/var/nix/profiles/system-{}-link", gen_num);
    let prev_path = format!("/nix/var/nix/profiles/system-{}-link", prev_gen);

    // Check if previous generation exists
    if !std::path::Path::new(&prev_path).exists() {
        out(tx, "    Previous generation not found").await;
        return Ok(Vec::new());
    }

    out(tx, &format!("    Comparing generation {} → {}", prev_gen, gen_num)).await;

    // Run nvd diff
    let (success, stdout, _stderr) = run_capture("nvd", &["diff", &prev_path, &current_path]).await?;

    if !success {
        out(tx, "    nvd diff failed").await;
        return Ok(Vec::new());
    }

    // Parse nvd output - only show updates (version changes)
    // Format: "[U.]  #015  firefox    146.0 -> 146.0.1"
    let mut changes = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();

        // Only process updates [U.] or [U*]
        if !line.starts_with("[U") {
            continue;
        }

        // Find the arrow to extract version info
        if let Some(arrow_pos) = line.find(" -> ") {
            // Skip the "[U.]  #NNN  " prefix to get package name
            if let Some(hash_pos) = line.find('#') {
                let after_hash = &line[hash_pos..];
                // Skip "#NNN " to get to package name and version
                if let Some(space_pos) = after_hash.find(char::is_whitespace) {
                    let rest = after_hash[space_pos..].trim();

                    // Split at arrow
                    let before_arrow = &rest[..rest.find(" -> ").unwrap_or(rest.len())];
                    let after_arrow = &line[arrow_pos + 4..];

                    // Package name is the first token
                    let parts: Vec<&str> = before_arrow.split_whitespace().collect();
                    if parts.is_empty() {
                        continue;
                    }
                    let pkg_name = parts[0];

                    // Old version is the last token before arrow (may have comma)
                    let old_ver = if parts.len() > 1 {
                        parts.last().unwrap_or(&"").trim_end_matches(',')
                    } else {
                        continue; // No version info
                    };

                    // New version is the first token after arrow
                    let new_ver = after_arrow
                        .split(|c: char| c == ',' || c.is_whitespace())
                        .next()
                        .unwrap_or("")
                        .trim();

                    if !pkg_name.is_empty() && !old_ver.is_empty() && !new_ver.is_empty() {
                        // Output to screen
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

    Ok(changes)
}

#[allow(dead_code)]
async fn parse_package_changes(
    old_system: Option<&str>,
    tx: &mpsc::Sender<CommandMessage>,
) -> Result<Vec<(String, String, String)>> {
    let old_path = match old_system {
        Some(p) if !p.is_empty() => p,
        _ => {
            tracing::debug!("parse_package_changes: no old system path provided");
            return Ok(Vec::new());
        }
    };

    // Get new system path
    let new_system = match get_output("readlink", &["/run/current-system"]).await {
        Ok(s) => s.trim().to_string(),
        Err(_) => {
            tracing::debug!("parse_package_changes: could not read new system path");
            return Ok(Vec::new());
        }
    };

    // Skip if paths are the same (no actual change)
    if old_path == new_system {
        tracing::debug!("parse_package_changes: system paths unchanged");
        return Ok(Vec::new());
    }

    // Run nvd diff
    let (success, stdout, _stderr) = run_capture("nvd", &["diff", old_path, &new_system]).await?;

    if !success {
        out(tx, "    nvd diff failed").await;
        tracing::debug!("parse_package_changes: nvd diff failed");
        return Ok(Vec::new());
    }

    // Parse nvd output - only show updates (version changes)
    // Format: "[U.]  #015  firefox    146.0 -> 146.0.1"
    let mut changes = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();

        // Only process updates [U.] or [U*]
        if !line.starts_with("[U") {
            continue;
        }

        // Find the arrow to extract version info
        if let Some(arrow_pos) = line.find(" -> ") {
            // Skip the "[U.]  #NNN  " prefix to get package name
            if let Some(hash_pos) = line.find('#') {
                let after_hash = &line[hash_pos..];
                // Skip "#NNN " to get to package name and version
                if let Some(space_pos) = after_hash.find(char::is_whitespace) {
                    let rest = after_hash[space_pos..].trim();

                    // Split at arrow
                    let before_arrow = &rest[..rest.find(" -> ").unwrap_or(rest.len())];
                    let after_arrow = &line[arrow_pos + 4..];

                    // Package name is the first token
                    let parts: Vec<&str> = before_arrow.split_whitespace().collect();
                    if parts.is_empty() {
                        continue;
                    }
                    let pkg_name = parts[0];

                    // Old version is the last token before arrow (may have comma)
                    let old_ver = if parts.len() > 1 {
                        parts.last().unwrap_or(&"").trim_end_matches(',')
                    } else {
                        continue; // No version info
                    };

                    // New version is the first token after arrow
                    let new_ver = after_arrow
                        .split(|c: char| c == ',' || c.is_whitespace())
                        .next()
                        .unwrap_or("")
                        .trim();

                    if !pkg_name.is_empty() && !old_ver.is_empty() && !new_ver.is_empty() {
                        // Output to screen
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

    Ok(changes)
}

async fn get_npm_package_version(package: &str) -> Option<String> {
    let (success, stdout, _) = run_capture("npm", &["list", "-g", "--depth=0", package])
        .await
        .ok()?;

    if !success {
        tracing::debug!("npm list failed for package: {}", package);
        return None;
    }

    // Parse version from output like "@openai/codex@1.0.0" or "codex@1.0.0"
    // Handle both scoped (@scope/package) and unscoped packages
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.contains(package) {
            // Split from the right to handle scoped packages correctly
            // "@openai/codex@1.2.3" -> split at last '@' gives "1.2.3"
            if let Some(at_pos) = trimmed.rfind('@') {
                let before = &trimmed[..at_pos];
                // Make sure this '@' is for the version, not the scope
                // For "@openai/codex@1.2.3", before="@openai/codex", after="1.2.3"
                if before.contains(package) || before.ends_with(package) {
                    let version = trimmed[at_pos + 1..].trim();
                    if !version.is_empty() {
                        return Some(version.to_string());
                    }
                }
            }
        }
    }

    tracing::debug!("Could not parse version for package: {}", package);
    None
}

async fn check_browser_status() -> Result<String> {
    // Check both new and legacy paths
    let local_repo = dirs::home_dir()
        .map(|h| {
            let new_path = h.join(".local/share/app-backup");
            if new_path.join(".git").exists() {
                new_path
            } else {
                h.join(".local/share/browser-backup")
            }
        })
        .unwrap_or_default();

    if !local_repo.join(".git").exists() {
        return Ok("not synced".to_string());
    }

    // Fetch and compare
    let _ = run_capture(
        "git",
        &["-C", local_repo.to_str().unwrap_or("."), "fetch", "origin"],
    )
    .await;

    let (_, local_head, _) = run_capture(
        "git",
        &["-C", local_repo.to_str().unwrap_or("."), "rev-parse", "HEAD"],
    )
    .await?;

    let (_, remote_head, _) = run_capture(
        "git",
        &[
            "-C",
            local_repo.to_str().unwrap_or("."),
            "rev-parse",
            "origin/main",
        ],
    )
    .await
    .unwrap_or((false, String::new(), String::new()));

    if local_head.trim() == remote_head.trim() {
        Ok("up to date".to_string())
    } else {
        Ok("updates available".to_string())
    }
}
