//! NVIDIA driver compatibility checking for kernel updates
//!
//! This module checks if the NVIDIA driver can build against a new kernel
//! before proceeding with system rebuild. If incompatible, the flake.lock
//! is restored and the kernel update is skipped.

use anyhow::Result;
use std::path::Path;
use tokio::sync::mpsc;

use crate::commands::executor::run_capture;
use crate::commands::update::flake::FlakeInputChange;
use crate::commands::CommandMessage;
use crate::system::hardware::{detect_gpu, GpuVendor};

use super::out;

/// Check if current system has NVIDIA GPU
pub fn is_nvidia_system() -> bool {
    match detect_gpu() {
        Ok(gpu_info) => matches!(
            gpu_info.vendor,
            GpuVendor::NVIDIA | GpuVendor::HybridNvidiaAmd
        ),
        Err(_) => false,
    }
}

/// Check if kernel-related packages changed in flake inputs
pub fn kernel_changed(changes: &[FlakeInputChange]) -> bool {
    changes.iter().any(|change| {
        let name_lower = change.name.to_lowercase();
        // nixpkgs contains kernel packages, also check for explicit kernel inputs
        name_lower == "nixpkgs"
            || name_lower.contains("kernel")
            || name_lower.contains("linux")
    })
}

/// Main compatibility check - returns Some(reason) if incompatible
///
/// This performs a dry-run build of the NVIDIA driver package to check
/// if it can build against the new kernel version.
pub async fn check_nvidia_compatibility(
    tx: &mpsc::Sender<CommandMessage>,
    flake_dir: &Path,
    hostname: &str,
    changes: &[FlakeInputChange],
    skip_check: bool,
) -> Result<Option<String>> {
    // Skip if --skip-nvidia-check was passed
    if skip_check {
        return Ok(None);
    }

    // Skip if not an NVIDIA system
    if !is_nvidia_system() {
        return Ok(None);
    }

    // Skip if kernel didn't change
    if !kernel_changed(changes) {
        return Ok(None);
    }

    out(tx, "").await;
    out(tx, "  Checking NVIDIA driver compatibility...").await;

    // Build the attribute path for NVIDIA packages
    let flake_path = flake_dir.to_str().unwrap_or(".");
    let attr_path = format!(
        "{}#nixosConfigurations.{}.config.boot.kernelPackages.nvidiaPackages.stable",
        flake_path, hostname
    );

    // Run nix build --dry-run to check if it can evaluate/build
    let (success, _stdout, stderr) = run_capture(
        "nix",
        &["build", &attr_path, "--dry-run", "--no-link"],
    )
    .await?;

    if success {
        out(tx, "  ✓ NVIDIA driver compatible with new kernel").await;
        Ok(None)
    } else {
        // Parse the error to extract a meaningful reason
        let reason = parse_nvidia_error(&stderr);
        Ok(Some(reason))
    }
}

/// Parse NVIDIA build error to extract a user-friendly reason
fn parse_nvidia_error(stderr: &str) -> String {
    let stderr_lower = stderr.to_lowercase();

    // Check for common NVIDIA incompatibility errors
    if stderr_lower.contains("nvidia") && stderr_lower.contains("kernel") {
        if stderr_lower.contains("version") {
            return "NVIDIA driver does not support new kernel version".to_string();
        }
        if stderr_lower.contains("hash mismatch") || stderr_lower.contains("sha256") {
            return "NVIDIA driver source hash mismatch (driver not yet updated for new kernel)"
                .to_string();
        }
    }

    if stderr_lower.contains("build failed") {
        return "NVIDIA driver build check failed".to_string();
    }

    if stderr_lower.contains("attribute") && stderr_lower.contains("missing") {
        return "NVIDIA driver package not available for new kernel".to_string();
    }

    // Default message
    "NVIDIA driver compatibility check failed".to_string()
}

/// Restore flake.lock from backup
pub async fn restore_flake_lock(flake_dir: &Path) -> bool {
    let lock_path = flake_dir.join("flake.lock");
    let backup_path = Path::new("/tmp/forge-flake.lock.old");

    if !backup_path.exists() {
        return false;
    }

    let (success, _, _) = match run_capture(
        "cp",
        &[
            backup_path.to_str().unwrap_or("/tmp/forge-flake.lock.old"),
            lock_path.to_str().unwrap_or("flake.lock"),
        ],
    )
    .await
    {
        Ok(result) => result,
        Err(_) => return false,
    };

    success
}

/// Clean up the flake.lock backup file
pub async fn cleanup_backup() {
    let backup_path = Path::new("/tmp/forge-flake.lock.old");
    if backup_path.exists() {
        let _ = tokio::fs::remove_file(backup_path).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nvidia_error_kernel_version() {
        let stderr = "error: NVIDIA driver 555.58.02 does not support kernel version 6.19";
        let result = parse_nvidia_error(stderr);
        assert!(result.contains("kernel version"));
    }

    #[test]
    fn test_parse_nvidia_error_hash_mismatch() {
        let stderr = "error: hash mismatch for NVIDIA sha256-abc123...";
        let result = parse_nvidia_error(stderr);
        assert!(result.contains("hash mismatch") || result.contains("not yet updated"));
    }

    #[test]
    fn test_parse_nvidia_error_build_failed() {
        let stderr = "error: build failed with exit code 1";
        let result = parse_nvidia_error(stderr);
        assert!(result.contains("build") && result.contains("failed"));
    }

    #[test]
    fn test_parse_nvidia_error_default() {
        let stderr = "some other error";
        let result = parse_nvidia_error(stderr);
        assert!(result.contains("compatibility check failed"));
    }

    #[test]
    fn test_kernel_changed_with_nixpkgs() {
        let changes = vec![FlakeInputChange {
            name: "nixpkgs".to_string(),
            owner: "NixOS".to_string(),
            repo: "nixpkgs".to_string(),
            old_rev: "abc123".to_string(),
            new_rev: "def456".to_string(),
            commits: vec![],
            total_commits: 1,
            compare_url: None,
        }];
        assert!(kernel_changed(&changes));
    }

    #[test]
    fn test_kernel_changed_with_other_input() {
        let changes = vec![FlakeInputChange {
            name: "home-manager".to_string(),
            owner: "nix-community".to_string(),
            repo: "home-manager".to_string(),
            old_rev: "abc123".to_string(),
            new_rev: "def456".to_string(),
            commits: vec![],
            total_commits: 1,
            compare_url: None,
        }];
        assert!(!kernel_changed(&changes));
    }
}
