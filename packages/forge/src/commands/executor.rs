//! Async command execution with output streaming

use anyhow::{Context, Result};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use super::CommandMessage;
use crate::constants::DEFAULT_COMMAND_TIMEOUT_SECS;

/// Execute a command and stream output to the channel
pub async fn run_command(
    tx: &mpsc::Sender<CommandMessage>,
    cmd: &str,
    args: &[&str],
) -> Result<bool> {
    run_command_with_timeout(tx, cmd, args, None).await
}

/// Execute a command with explicit timeout
pub async fn run_command_with_timeout(
    tx: &mpsc::Sender<CommandMessage>,
    cmd: &str,
    args: &[&str],
    timeout_secs: Option<u64>,
) -> Result<bool> {
    tracing::info!("Running command: {} {:?}", cmd, args);

    let mut child = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn command: {}", cmd))?;

    // Use proper error handling instead of .expect()
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout for command: {}", cmd))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr for command: {}", cmd))?;

    let tx_out = tx.clone();
    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            // Log channel send failures but don't propagate - channel may be closed
            if let Err(e) = tx_out.send(CommandMessage::Stdout(line)).await {
                tracing::warn!("Failed to send stdout to channel: {}", e);
                break;
            }
        }
    });

    let tx_err = tx.clone();
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if let Err(e) = tx_err.send(CommandMessage::Stderr(line)).await {
                tracing::warn!("Failed to send stderr to channel: {}", e);
                break;
            }
        }
    });

    // Apply timeout
    let timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_COMMAND_TIMEOUT_SECS));
    let status = tokio::time::timeout(timeout, child.wait())
        .await
        .with_context(|| format!("Command timed out after {}s: {}", timeout.as_secs(), cmd))?
        .with_context(|| format!("Failed to wait for command: {}", cmd))?;

    // Wait for output tasks with short timeout (they should complete quickly after process exits)
    match tokio::time::timeout(Duration::from_secs(5), stdout_task).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!("stdout reader task panicked: {}", e),
        Err(_) => tracing::warn!("stdout reader task timed out for command: {}", cmd),
    }
    match tokio::time::timeout(Duration::from_secs(5), stderr_task).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!("stderr reader task panicked: {}", e),
        Err(_) => tracing::warn!("stderr reader task timed out for command: {}", cmd),
    }

    let success = status.success();
    tracing::info!("Command completed with success={}", success);
    Ok(success)
}

/// Execute a command with sudo
#[allow(dead_code)]
pub async fn run_sudo(
    tx: &mpsc::Sender<CommandMessage>,
    cmd: &str,
    args: &[&str],
) -> Result<bool> {
    let mut sudo_args = vec![cmd];
    sudo_args.extend(args);
    run_command(tx, "sudo", &sudo_args).await
}

/// Execute a command and capture output (no streaming)
pub async fn run_capture(cmd: &str, args: &[&str]) -> Result<(bool, String, String)> {
    tracing::info!("Capturing command: {} {:?}", cmd, args);

    let output = Command::new(cmd)
        .args(args)
        .output()
        .await
        .with_context(|| format!("Failed to execute command: {}", cmd))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok((output.status.success(), stdout, stderr))
}

/// Check if a command exists
pub async fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Get output of a command as a string
pub async fn get_output(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .await
        .with_context(|| format!("Failed to get output from command: {}", cmd))?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
