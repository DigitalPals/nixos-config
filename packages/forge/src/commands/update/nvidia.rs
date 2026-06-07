//! NVIDIA driver compatibility checking for kernel updates
//!
//! When a flake update bumps the kernel (usually via nixpkgs) the packaged
//! NVIDIA driver may not yet support it. This module builds the host's
//! configured NVIDIA driver against the new kernel *before* the full system
//! rebuild. If it fails to build, the flake.lock is restored so the rebuild is
//! skipped rather than switching into a system whose GPU driver won't build.

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
/// This builds the host's configured NVIDIA driver against the new kernel. A
/// kernel/driver mismatch fails at build time (the kernel module won't
/// compile), so a real build — not `--dry-run`, which only evaluates — is
/// required to catch it. The build is reused by the subsequent rebuild when it
/// succeeds, so the only extra cost is when the driver is genuinely broken.
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
    out(
        tx,
        "  Kernel changed - building NVIDIA driver against the new kernel...",
    )
    .await;

    // Build the host's actually-configured driver (respects open vs proprietary
    // module selection) so the kernel module is compiled against the new kernel.
    let flake_path = flake_dir.to_str().unwrap_or(".");
    let attr_path = format!(
        "{}#nixosConfigurations.{}.config.hardware.nvidia.package",
        flake_path, hostname
    );

    let (success, _stdout, stderr) =
        run_capture("nix", &["build", &attr_path, "--no-link"]).await?;

    if success {
        out(tx, "  ✓ NVIDIA driver builds against the new kernel").await;
        Ok(None)
    } else {
        // Parse the error to extract a meaningful reason
        let reason = parse_nvidia_error(&stderr);
        Ok(Some(reason))
    }
}

/// Parse NVIDIA build error to extract a user-friendly reason.
///
/// Checks run most-specific first. A fixed-output hash mismatch (the driver
/// tarball changed / isn't yet packaged for the new kernel) is reported even
/// without a "kernel" token, since those errors rarely contain one.
fn parse_nvidia_error(stderr: &str) -> String {
    let s = stderr.to_lowercase();

    if s.contains("hash mismatch") || s.contains("sha256-") {
        return "NVIDIA driver source hash mismatch (driver not yet updated for new kernel)"
            .to_string();
    }

    if s.contains("nvidia") && s.contains("kernel") && s.contains("version") {
        return "NVIDIA driver does not support new kernel version".to_string();
    }

    if s.contains("attribute") && s.contains("missing") {
        return "NVIDIA driver package not available for new kernel".to_string();
    }

    if s.contains("build failed") || s.contains("builder for") || s.contains("error: build") {
        return "NVIDIA driver failed to build against the new kernel".to_string();
    }

    // Default message
    "NVIDIA driver compatibility check failed".to_string()
}

/// Restore flake.lock from the pre-update backup created by the caller.
///
/// Returns `true` if the working-tree flake.lock was reverted to its
/// pre-update state, so the subsequent rebuild uses the previous (known-good)
/// inputs.
pub async fn restore_flake_lock(flake_dir: &Path, backup_path: &Path) -> bool {
    if !backup_path.exists() {
        return false;
    }

    let lock_path = flake_dir.join("flake.lock");
    tokio::fs::copy(backup_path, &lock_path).await.is_ok()
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
            new_last_modified: None,
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
            new_last_modified: None,
            commits: vec![],
            total_commits: 1,
            compare_url: None,
        }];
        assert!(!kernel_changed(&changes));
    }
}
