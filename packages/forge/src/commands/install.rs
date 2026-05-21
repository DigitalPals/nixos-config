//! Fresh NixOS installation command
//!
//! This module handles the complete NixOS installation process, broken down into steps:
//! 1. Network check
//! 2. Enable flakes
//! 3. Clone/prepare configuration repository
//! 4. Configure disk device
//! 5. Run disko (partition and format)
//! 6. Install NixOS
//! 7. Set user password

use anyhow::{Context, Result};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tokio::sync::mpsc;

use super::errors::{ErrorContext, ParsedError};
use super::executor::{
    run_capture, run_capture_with_timeout, run_command_sensitive_with_input,
    run_command_with_timeout,
};
use super::runner::CommandRunner;
use super::CommandMessage;
use crate::app::SwapMode;
use crate::constants::{
    self, INSTALL_MOUNT_POINT, INSTALL_SYMLINK_PATH, NIXOS_CONFIG_HOME_DIR, PRIMARY_USER_GID,
    PRIMARY_USER_UID,
};

// =============================================================================
// Install Constants
// =============================================================================

/// Path to the temporary LUKS password file (used by disko)
const LUKS_PASSWORD_FILE: &str = "/tmp/luks-password";

/// GitHub repository URL for the NixOS configuration
const REPO_URL: &str = "https://github.com/DigitalPals/nixos-config.git";

/// Nix config used during install (enable flakes + disable sandbox for disk ops)
const NIX_CONFIG_VALUE: &str = "experimental-features = nix-command flakes\nsandbox = false";

/// Generated local install overrides persisted onto the installed system
const LOCAL_OVERRIDE_FILE: &str = "local.nix";

/// Generated installer-only overrides used only during provisioning
const INSTALLER_OVERRIDE_FILE: &str = "installer.nix";

/// Generated optional hardware detection layer from the live installer
const DETECTED_HARDWARE_FILE: &str = "detected-hardware.nix";

#[derive(Debug, Clone)]
struct GeneratedInstallConfig {
    username: String,
    disk: String,
    hibernate_swap_size_gb: Option<u64>,
    luks_uuid: Option<String>,
    resume_offset: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TargetUser {
    name: String,
    uid: u32,
    gid: u32,
    home: PathBuf,
    shell: PathBuf,
}

// =============================================================================
// Security Helpers
// =============================================================================

/// Securely overwrite a file with zeros and 0xFF before removing it.
/// This prevents password data from lingering on disk.
fn shred_and_remove(path: &str) {
    use std::io::{Seek, Write};
    let size = match std::fs::metadata(path) {
        Ok(m) => m.len() as usize,
        Err(_) => {
            // File doesn't exist, nothing to shred
            return;
        }
    };
    if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(path) {
        // Overwrite with zeros
        let _ = file.write_all(&vec![0u8; size]);
        let _ = file.sync_all();
        // Seek back to start before second pass
        let _ = file.rewind();
        // Overwrite with 0xFF
        let _ = file.write_all(&vec![0xFFu8; size]);
        let _ = file.sync_all();
    }
    if let Err(e) = std::fs::remove_file(path) {
        tracing::warn!("Failed to remove file {}: {}", path, e);
    }
}

// =============================================================================
// Regex Patterns
// =============================================================================

/// Regex to read the persisted encrypted-root UUID from the generated local override.
static LOCAL_LUKS_UUID_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r#"boot\.initrd\.luks\.devices\."cryptroot"\.device = lib\.mkForce "/dev/disk/by-uuid/([^"]+)";"#,
    )
    .expect("local LUKS UUID regex pattern is statically validated")
});

/// Regex to match AMD GPU bus ID in PRIME configuration.
static AMD_BUS_ID_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"amdgpuBusId = "PCI:[^"]*""#)
        .expect("AMD bus ID regex pattern is statically validated")
});

/// Regex to detect LVM configuration in disko files.
static LVM_PV_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"type\s*=\s*"lvm_pv""#)
        .expect("LVM PV regex pattern is statically validated")
});

/// Regex to match NVIDIA GPU bus ID in PRIME configuration.
static NVIDIA_BUS_ID_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"nvidiaBusId = "PCI:[^"]*""#)
        .expect("NVIDIA bus ID regex pattern is statically validated")
});

/// Get total RAM size in GB (rounded up) from /proc/meminfo
fn get_ram_size_gb() -> u64 {
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u64>() {
                        // Convert KB to GB, rounding up
                        return (kb + 1024 * 1024 - 1) / (1024 * 1024);
                    }
                }
            }
        }
    }
    8 // Fallback to 8GB
}

/// Get the config directory path for the mounted system
fn get_config_dir(username: &str) -> String {
    format!(
        "{}/home/{}/{}",
        INSTALL_MOUNT_POINT, username, NIXOS_CONFIG_HOME_DIR
    )
}

/// Get the symlink target (path on the installed system, not /mnt)
fn get_symlink_target(username: &str) -> String {
    format!("/home/{}/{}", username, NIXOS_CONFIG_HOME_DIR)
}

fn target_config_dir(mount_root: &Path, username: &str) -> PathBuf {
    mount_root
        .join("home")
        .join(username)
        .join(NIXOS_CONFIG_HOME_DIR)
}

fn target_home_dir(mount_root: &Path, username: &str) -> PathBuf {
    mount_root.join("home").join(username)
}

fn target_xdg_config_dir(mount_root: &Path, username: &str) -> PathBuf {
    target_home_dir(mount_root, username).join(".config")
}

fn target_etc_nixos_path(mount_root: &Path) -> PathBuf {
    mount_root.join("etc").join("nixos")
}

fn installed_absolute_path_under_mount(mount_root: &Path, installed_path: &Path) -> PathBuf {
    if installed_path.is_absolute() {
        match installed_path.strip_prefix("/") {
            Ok(relative) => mount_root.join(relative),
            Err(_) => mount_root.join(installed_path),
        }
    } else {
        mount_root.join(installed_path)
    }
}

fn parse_passwd_user(line: &str, username: &str) -> Result<Option<TargetUser>> {
    let fields: Vec<&str> = line.split(':').collect();
    if fields.len() < 7 || fields[0] != username {
        return Ok(None);
    }

    let uid = fields[2]
        .parse::<u32>()
        .with_context(|| format!("Invalid UID for user '{}': {}", username, fields[2]))?;
    let gid = fields[3]
        .parse::<u32>()
        .with_context(|| format!("Invalid GID for user '{}': {}", username, fields[3]))?;

    Ok(Some(TargetUser {
        name: username.to_string(),
        uid,
        gid,
        home: PathBuf::from(fields[5]),
        shell: PathBuf::from(fields[6]),
    }))
}

fn read_target_user(mount_root: &Path, username: &str) -> Result<TargetUser> {
    let passwd_path = mount_root.join("etc/passwd");
    let passwd = std::fs::read_to_string(&passwd_path)
        .with_context(|| format!("Failed to read target passwd: {}", passwd_path.display()))?;

    for line in passwd.lines() {
        if let Some(user) = parse_passwd_user(line, username)? {
            return Ok(user);
        }
    }

    anyhow::bail!(
        "Installed user '{}' was not found in {}",
        username,
        passwd_path.display()
    )
}

fn mode_allows_user_dir_write(metadata: &std::fs::Metadata, uid: u32, gid: u32) -> bool {
    let mode = metadata.permissions().mode();
    if metadata.uid() == uid {
        mode & 0o300 == 0o300
    } else if metadata.gid() == gid {
        mode & 0o030 == 0o030
    } else {
        mode & 0o003 == 0o003
    }
}

fn verify_writable_dir_for_user(path: &Path, uid: u32, gid: u32) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path).with_context(|| {
        format!(
            "Missing expected user-writable directory: {}",
            path.display()
        )
    })?;

    if !metadata.is_dir() {
        anyhow::bail!("Expected a directory at {}", path.display());
    }

    if !mode_allows_user_dir_write(&metadata, uid, gid) {
        anyhow::bail!(
            "Directory is not writable by installed user {}:{}: {} (mode {:o}, owner {}:{})",
            uid,
            gid,
            path.display(),
            metadata.permissions().mode() & 0o777,
            metadata.uid(),
            metadata.gid()
        );
    }

    Ok(())
}

fn verify_tree_owned_by_user(root: &Path, uid: u32, gid: u32) -> Result<()> {
    let mut stack = vec![root.to_path_buf()];

    while let Some(path) = stack.pop() {
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("Failed to inspect {}", path.display()))?;

        if metadata.uid() != uid || metadata.gid() != gid {
            anyhow::bail!(
                "Unexpected ownership for {}: expected {}:{}, got {}:{}",
                path.display(),
                uid,
                gid,
                metadata.uid(),
                metadata.gid()
            );
        }

        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path)
                .with_context(|| format!("Failed to read directory {}", path.display()))?
            {
                let entry =
                    entry.with_context(|| format!("Failed to read entry in {}", path.display()))?;
                stack.push(entry.path());
            }
        }
    }

    Ok(())
}

fn verify_no_root_fish_artifacts(mount_root: &Path) -> Result<()> {
    for relative_path in ["config.fish", "completions", "conf.d", "functions"] {
        let path = mount_root.join(relative_path);
        if path.exists() {
            anyhow::bail!(
                "Fish/XDG sanity check failed: unexpected root-level {} exists",
                path.display()
            );
        }
    }

    Ok(())
}

fn verify_fish_shell_path(mount_root: &Path, shell: &Path) -> Result<()> {
    if shell.file_name().and_then(|name| name.to_str()) != Some("fish") {
        anyhow::bail!("Installed user shell is not fish: {}", shell.display());
    }

    if shell.starts_with("/nix/store") {
        let target_shell = installed_absolute_path_under_mount(mount_root, shell);
        if !target_shell.exists() {
            anyhow::bail!(
                "Configured fish shell does not exist in mounted target: {}",
                target_shell.display()
            );
        }
    }

    Ok(())
}

fn verify_home_sanity_at(mount_root: &Path, username: &str, uid: u32, gid: u32) -> Result<()> {
    let home_dir = target_home_dir(mount_root, username);
    verify_writable_dir_for_user(&home_dir, uid, gid)?;
    verify_tree_owned_by_user(&home_dir, uid, gid)?;

    for path in [
        target_xdg_config_dir(mount_root, username),
        target_xdg_config_dir(mount_root, username).join("fish"),
        home_dir.join(".local"),
        home_dir.join(".local/state"),
        home_dir.join(".cache"),
    ] {
        verify_writable_dir_for_user(&path, uid, gid)?;
    }

    let fish_config_dir = target_xdg_config_dir(mount_root, username).join("fish");
    if !fish_config_dir.starts_with(target_xdg_config_dir(mount_root, username)) {
        anyhow::bail!(
            "Fish config directory escaped XDG config home: {}",
            fish_config_dir.display()
        );
    }

    verify_no_root_fish_artifacts(mount_root)?;
    Ok(())
}

fn verify_installed_home_sanity_at(mount_root: &Path, username: &str) -> Result<TargetUser> {
    let user = read_target_user(mount_root, username)?;
    let expected_home = PathBuf::from(format!("/home/{username}"));

    if user.home != expected_home {
        anyhow::bail!(
            "Installed user '{}' has wrong home directory: expected {}, got {}",
            username,
            expected_home.display(),
            user.home.display()
        );
    }

    verify_fish_shell_path(mount_root, &user.shell)?;
    verify_home_sanity_at(mount_root, username, user.uid, user.gid)?;

    Ok(user)
}

fn verify_installed_sanity_at(
    mount_root: &Path,
    username: &str,
    hostname: &str,
) -> Result<TargetUser> {
    verify_installed_config_layout_at(mount_root, username, hostname)?;
    verify_installed_home_sanity_at(mount_root, username)
}

fn host_dir(temp_config: &Path, hostname: &str) -> PathBuf {
    temp_config.join(constants::HOSTS_SUBDIR).join(hostname)
}

fn local_override_path(temp_config: &Path, hostname: &str) -> PathBuf {
    host_dir(temp_config, hostname).join(LOCAL_OVERRIDE_FILE)
}

fn installer_override_path(temp_config: &Path, hostname: &str) -> PathBuf {
    host_dir(temp_config, hostname).join(INSTALLER_OVERRIDE_FILE)
}

fn detected_hardware_path(temp_config: &Path, hostname: &str) -> PathBuf {
    host_dir(temp_config, hostname).join(DETECTED_HARDWARE_FILE)
}

fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(path)
            .with_context(|| format!("Failed to remove generated file: {}", path.display()))?;
    }
    Ok(())
}

fn reset_generated_install_state(temp_config: &Path, hostname: &str) -> Result<()> {
    remove_if_exists(&local_override_path(temp_config, hostname))?;
    remove_if_exists(&installer_override_path(temp_config, hostname))?;
    remove_if_exists(&detected_hardware_path(temp_config, hostname))?;
    Ok(())
}

fn host_uses_lvm(temp_config: &Path, hostname: &str) -> Result<bool> {
    let disko_file = temp_config
        .join("modules")
        .join("disko")
        .join(format!("{hostname}.nix"));
    let disko_content = std::fs::read_to_string(&disko_file)
        .with_context(|| format!("Failed to read disko config: {}", disko_file.display()))?;
    Ok(LVM_PV_RE.is_match(&disko_content))
}

fn validate_config_tree(config_dir: &Path, hostname: &str) -> Result<()> {
    let required_paths = [
        PathBuf::from(constants::FLAKE_NIX),
        PathBuf::from("flake.lock"),
        PathBuf::from(constants::HOSTS_SUBDIR),
        PathBuf::from("home"),
        PathBuf::from("modules"),
        PathBuf::from("packages"),
        PathBuf::from("wallpapers"),
        PathBuf::from(constants::HOSTS_SUBDIR)
            .join(hostname)
            .join("default.nix"),
        PathBuf::from("modules")
            .join("disko")
            .join(format!("{hostname}.nix")),
    ];

    for relative_path in required_paths {
        let path = config_dir.join(&relative_path);
        if !path.exists() {
            anyhow::bail!(
                "NixOS configuration is incomplete: missing {} in {}",
                relative_path.display(),
                config_dir.display()
            );
        }
    }

    Ok(())
}

fn verify_installed_config_layout_at(
    mount_root: &Path,
    username: &str,
    hostname: &str,
) -> Result<()> {
    let config_dir = target_config_dir(mount_root, username);
    validate_config_tree(&config_dir, hostname).with_context(|| {
        format!(
            "Installed config verification failed at {}",
            config_dir.display()
        )
    })?;

    let home_flake = config_dir.join(constants::FLAKE_NIX);
    if !home_flake.is_file() {
        anyhow::bail!("Missing installed flake: {}", home_flake.display());
    }

    let etc_nixos = target_etc_nixos_path(mount_root);
    let etc_metadata = std::fs::symlink_metadata(&etc_nixos)
        .with_context(|| format!("Missing installed /etc/nixos at {}", etc_nixos.display()))?;

    if etc_metadata.file_type().is_symlink() {
        let actual_target = std::fs::read_link(&etc_nixos)
            .with_context(|| format!("Failed to read symlink {}", etc_nixos.display()))?;
        let expected_target = PathBuf::from(get_symlink_target(username));

        if actual_target != expected_target {
            anyhow::bail!(
                "Invalid /etc/nixos symlink target: expected {}, got {}",
                expected_target.display(),
                actual_target.display()
            );
        }

        let resolved_flake = installed_absolute_path_under_mount(mount_root, &actual_target)
            .join(constants::FLAKE_NIX);
        if !resolved_flake.is_file() {
            anyhow::bail!(
                "/etc/nixos symlink does not resolve to a flake in the mounted target: {} -> {}",
                etc_nixos.display(),
                resolved_flake.display()
            );
        }
    } else {
        let etc_flake = etc_nixos.join(constants::FLAKE_NIX);
        if !etc_flake.is_file() {
            anyhow::bail!(
                "Missing installed /etc/nixos flake: {}",
                etc_flake.display()
            );
        }
    }

    Ok(())
}

fn build_local_install_config(
    username: &str,
    disk: &str,
    swap_mode: &SwapMode,
    uses_lvm: bool,
    luks_uuid: Option<String>,
    resume_offset: Option<u64>,
) -> GeneratedInstallConfig {
    let hibernate_swap_size_gb = if *swap_mode == SwapMode::HibernateSupport && !uses_lvm {
        Some(get_ram_size_gb() + 2)
    } else {
        None
    };

    GeneratedInstallConfig {
        username: username.to_string(),
        disk: disk.to_string(),
        hibernate_swap_size_gb,
        luks_uuid,
        resume_offset,
    }
}

fn render_local_install_config(config: &GeneratedInstallConfig) -> String {
    let mut rendered = String::from(
        "# Auto-generated by Forge during installation.\n\
{ lib, ... }:\n\
\n\
{\n",
    );

    rendered.push_str(&format!(
        "  forge.installer.username = \"{}\";\n",
        config.username
    ));
    rendered.push_str(&format!(
        "  disko.devices.disk.main.device = lib.mkForce \"{}\";\n",
        config.disk
    ));

    if let Some(swap_size_gb) = config.hibernate_swap_size_gb {
        rendered.push_str(
            "\n  # Generated hibernate swap configuration\n\
  disko.devices.disk.main.content.partitions.luks.content.content.subvolumes.\"@swap\" = {\n\
    mountpoint = \"/swap\";\n\
    mountOptions = [ \"noatime\" ];\n\
    swap.swapfile = {\n",
        );
        rendered.push_str(&format!("      size = \"{}G\";\n", swap_size_gb));
        rendered.push_str(
            "      path = \"swapfile\";\n\
    };\n\
  };\n\
  zramSwap.enable = lib.mkForce false;\n",
        );
    }

    if let Some(uuid) = &config.luks_uuid {
        rendered.push_str(&format!(
            "\n  boot.initrd.luks.devices.\"cryptroot\".device = lib.mkForce \"/dev/disk/by-uuid/{}\";\n",
            uuid
        ));
    }

    if let Some(resume_offset) = config.resume_offset {
        rendered.push_str("\n  boot.resumeDevice = lib.mkForce \"/dev/mapper/cryptroot\";\n");
        rendered.push_str(&format!(
            "  boot.kernelParams = lib.mkAfter [ \"resume_offset={}\" ];\n",
            resume_offset
        ));
    }

    rendered.push_str("}\n");
    rendered
}

fn write_local_install_config(
    temp_config: &Path,
    hostname: &str,
    config: &GeneratedInstallConfig,
) -> Result<()> {
    let path = local_override_path(temp_config, hostname);
    std::fs::write(&path, render_local_install_config(config)).with_context(|| {
        format!(
            "Failed to write generated install profile: {}",
            path.display()
        )
    })
}

fn render_installer_override() -> &'static str {
    "# Auto-generated by Forge during installation. Do not persist this file.\n\
{ ... }:\n\
\n\
{\n\
  disko.devices.disk.main.content.partitions.luks.content.passwordFile = \"/tmp/luks-password\";\n\
}\n"
}

fn write_installer_override(temp_config: &Path, hostname: &str) -> Result<()> {
    let path = installer_override_path(temp_config, hostname);
    std::fs::write(&path, render_installer_override())
        .with_context(|| format!("Failed to write installer overrides: {}", path.display()))
}

fn detect_existing_luks_uuid(temp_config: &Path, hostname: &str) -> Result<Option<String>> {
    let path = local_override_path(temp_config, hostname);
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "Failed to read generated install profile: {}",
            path.display()
        )
    })?;
    Ok(LOCAL_LUKS_UUID_RE
        .captures(&content)
        .and_then(|caps| caps.get(1).map(|m| m.as_str().to_string())))
}

/// Start the installation process
pub async fn start_install(
    tx: mpsc::Sender<CommandMessage>,
    hostname: &str,
    disk: &str,
    username: &str,
    password: &str,
    swap_mode: SwapMode,
    refresh_hardware_config: bool,
) -> Result<()> {
    let hostname = hostname.to_string();
    let disk = disk.to_string();
    let username = username.to_string();
    let password = password.to_string();

    tokio::spawn(async move {
        if let Err(e) = run_install(
            &tx,
            &hostname,
            &disk,
            &username,
            &password,
            &swap_mode,
            refresh_hardware_config,
        )
        .await
        {
            let error_msg = format!("{:#}", e); // Full error chain with context
            tracing::error!("Installation failed: {}", error_msg);
            // Display error to user
            let _ = tx
                .send(CommandMessage::Stderr(format!(
                    "\n*** INSTALLATION ERROR ***"
                )))
                .await;
            let _ = tx.send(CommandMessage::Stderr(error_msg.clone())).await;
            let _ = tx.send(CommandMessage::Stderr("".to_string())).await;
            let _ = tx
                .send(CommandMessage::StepFailed {
                    step: "Install".to_string(),
                    error: ParsedError::from_stderr(
                        &error_msg,
                        ErrorContext {
                            operation: "Installation".to_string(),
                        },
                    ),
                })
                .await;
            let _ = tx.send(CommandMessage::Done { success: false }).await;
        }
    });
    Ok(())
}

/// Check GitHub connectivity using HTTPS rather than ICMP.
async fn check_github_connectivity() -> Result<bool> {
    let (success, _, _) = run_capture_with_timeout(
        "curl",
        &["-sfL", "--max-time", "10", "https://github.com"],
        Some(constants::NETWORK_CHECK_TIMEOUT_SECS),
    )
    .await?;

    Ok(success)
}

/// Clone the repository to /tmp/nixos-config for host discovery
/// This is called before the install wizard to populate the host list
pub async fn start_clone_repository(tx: mpsc::Sender<CommandMessage>) -> Result<()> {
    tokio::spawn(async move {
        let temp_config = constants::temp_config_dir();
        let temp_config_str = temp_config.to_string_lossy().to_string();

        // Check if already cloned
        let hosts_dir = temp_config.join(constants::HOSTS_SUBDIR);
        if hosts_dir.exists() {
            let _ = tx
                .send(CommandMessage::Stdout(
                    "Using existing configuration...".to_string(),
                ))
                .await;
            let _ = tx
                .send(CommandMessage::CloneComplete { success: true })
                .await;
            return;
        }

        let _ = tx
            .send(CommandMessage::Stdout(
                "Checking network connectivity...".to_string(),
            ))
            .await;

        // Check network
        let net_ok = match check_github_connectivity().await {
            Ok(result) => result,
            Err(_) => {
                let _ = tx
                    .send(CommandMessage::Stderr("Network check failed".to_string()))
                    .await;
                let _ = tx
                    .send(CommandMessage::CloneComplete { success: false })
                    .await;
                return;
            }
        };

        if !net_ok {
            let _ = tx
                .send(CommandMessage::Stderr(
                    "No internet connection. Please configure WiFi with nmtui.".to_string(),
                ))
                .await;
            let _ = tx
                .send(CommandMessage::CloneComplete { success: false })
                .await;
            return;
        }

        let _ = tx
            .send(CommandMessage::Stdout(
                "Cloning configuration repository...".to_string(),
            ))
            .await;

        // Enable flakes and disable sandbox for disk operations
        std::env::set_var("NIX_CONFIG", NIX_CONFIG_VALUE);

        // Remove any partial clone
        let _ = std::fs::remove_dir_all(&temp_config);

        // Clone repository (streaming output with 5-minute timeout)
        let clone_cmd = format!("git clone --depth 1 {} {}", REPO_URL, temp_config_str);
        let success = match run_command_with_timeout(
            &tx,
            "nix-shell",
            &["-p", "git", "--run", &clone_cmd],
            Some(constants::REPOSITORY_COMMAND_TIMEOUT_SECS),
        )
        .await
        {
            Ok(result) => result,
            Err(e) => {
                let _ = tx
                    .send(CommandMessage::Stderr(format!(
                        "Clone command failed: {}",
                        e
                    )))
                    .await;
                let _ = tx
                    .send(CommandMessage::CloneComplete { success: false })
                    .await;
                return;
            }
        };

        if success {
            let _ = tx
                .send(CommandMessage::Stdout(
                    "Repository cloned successfully".to_string(),
                ))
                .await;
        }

        let _ = tx.send(CommandMessage::CloneComplete { success }).await;
    });

    Ok(())
}

// =============================================================================
// Installation Steps
// =============================================================================

/// Step 1: Check network connectivity
async fn step_check_network(runner: &CommandRunner<'_>) -> Result<bool> {
    runner.out("Checking network connectivity...").await;

    if !check_github_connectivity().await? {
        runner
            .step_failed("network", "No network connection", "Network check")
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    runner.step_complete("network").await?;
    Ok(true)
}

/// Step 2: Enable Nix flakes and disable sandbox for disk operations
async fn step_enable_flakes(runner: &CommandRunner<'_>) -> Result<bool> {
    runner.out("Enabling Nix flakes...").await;
    std::env::set_var("NIX_CONFIG", NIX_CONFIG_VALUE);
    runner.step_complete("flakes").await?;
    Ok(true)
}

/// Step 3: Clone or prepare the configuration repository
async fn step_prepare_repository(
    runner: &CommandRunner<'_>,
    hostname: &str,
) -> Result<Option<std::path::PathBuf>> {
    let temp_config = constants::temp_config_dir();
    let temp_config_str = temp_config.to_string_lossy().to_string();
    let host_exists_in_temp = temp_config
        .join(constants::HOSTS_SUBDIR)
        .join(hostname)
        .join("default.nix")
        .exists();

    if host_exists_in_temp {
        runner
            .out("Using existing configuration (host already created)...")
            .await;
    } else {
        runner.out("Cloning configuration repository...").await;
        let _ = std::fs::remove_dir_all(&temp_config);

        let success = runner
            .run_with_timeout(
                "nix-shell",
                &[
                    "-p",
                    "git",
                    "--run",
                    &format!("git clone --depth 1 {} {}", REPO_URL, temp_config_str),
                ],
                constants::REPOSITORY_COMMAND_TIMEOUT_SECS,
            )
            .await?;

        if !success {
            runner
                .step_failed(
                    "repository",
                    "Failed to clone repository",
                    "Clone repository",
                )
                .await?;
            runner.done(false).await?;
            return Ok(None);
        }
    }

    validate_config_tree(&temp_config, hostname).with_context(|| {
        format!(
            "Source configuration is not usable for host '{}': {}",
            hostname,
            temp_config.display()
        )
    })?;

    runner.step_complete("repository").await?;
    Ok(Some(temp_config))
}

/// Regenerate the machine-detected hardware profile from the live environment.
async fn regenerate_hardware_config(
    runner: &CommandRunner<'_>,
    output_path: &std::path::Path,
) -> Result<()> {
    let temp_dir = format!("/tmp/forge-hw-config-{}", std::process::id());
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir)
        .with_context(|| format!("Failed to create temp hardware config dir: {}", temp_dir))?;

    let success = runner
        .run(
            "sudo",
            &[
                "nixos-generate-config",
                "--no-filesystems",
                "--dir",
                &temp_dir,
            ],
        )
        .await?;

    if !success {
        anyhow::bail!("nixos-generate-config failed");
    }

    let generated_path = std::path::Path::new(&temp_dir).join("hardware-configuration.nix");
    if !generated_path.exists() {
        anyhow::bail!("generated hardware profile not found");
    }

    std::fs::copy(&generated_path, output_path).with_context(|| {
        format!(
            "Failed to replace hardware profile at {}",
            output_path.display()
        )
    })?;

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(())
}

/// Step 4: Refresh machine-detected hardware profile from the live installer.
async fn step_refresh_hardware_config(
    runner: &CommandRunner<'_>,
    temp_config: &std::path::Path,
    hostname: &str,
    refresh_hardware_config: bool,
) -> Result<bool> {
    if !refresh_hardware_config {
        remove_if_exists(&detected_hardware_path(temp_config, hostname))?;
        runner
            .out("Hardware refresh disabled, keeping the checked-in hardware profile.")
            .await;
        runner.step_complete(super::steps::HW_CONFIG).await?;
        return Ok(true);
    }

    runner
        .out("Refreshing machine-detected hardware profile from the live system...")
        .await;

    let hardware_file = detected_hardware_path(temp_config, hostname);

    match regenerate_hardware_config(runner, &hardware_file).await {
        Ok(()) => {
            runner
                .out("Machine-detected hardware layer generated successfully.")
                .await;
        }
        Err(e) => {
            runner
                .err(&format!(
                    "Hardware detection refresh failed, continuing with the checked-in profile: {}",
                    e
                ))
                .await;
            let _ = remove_if_exists(&hardware_file);
        }
    }

    runner.step_complete(super::steps::HW_CONFIG).await?;
    Ok(true)
}

/// Step 5: Configure disk device and update disko configuration
async fn step_configure_disk(
    runner: &CommandRunner<'_>,
    temp_config: &std::path::Path,
    hostname: &str,
    disk: &str,
    username: &str,
    swap_mode: &SwapMode,
) -> Result<bool> {
    let temp_config_str = temp_config.to_string_lossy();
    runner
        .out(&format!("Preparing install profile for {}...", disk))
        .await;

    // Validate disk path format
    if !disk.starts_with("/dev/") {
        runner
            .step_failed(
                "disk",
                &format!("Invalid disk path: {}. Must start with /dev/", disk),
                "Disk validation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    // Check that disk device actually exists
    if !std::path::Path::new(disk).exists() {
        runner
            .step_failed(
                "disk",
                &format!("Disk device does not exist: {}", disk),
                "Disk validation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    // Check disko config file exists
    let disko_file = format!("{}/modules/disko/{}.nix", temp_config_str, hostname);
    if !std::path::Path::new(&disko_file).exists() {
        runner
            .step_failed(
                "disk",
                &format!(
                    "No disko configuration found for host '{}'. Expected: modules/disko/{}.nix",
                    hostname, hostname
                ),
                "Disk configuration",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    let uses_lvm = host_uses_lvm(temp_config, hostname)?;
    let generated = build_local_install_config(username, disk, swap_mode, uses_lvm, None, None);
    write_local_install_config(temp_config, hostname, &generated)?;

    if *swap_mode == SwapMode::HibernateSupport {
        if uses_lvm {
            runner
                .out("Detected LVM configuration - keeping the existing hibernate layout")
                .await;
        } else if let Some(swap_size_gb) = generated.hibernate_swap_size_gb {
            runner
                .out(&format!(
                    "Configuring hibernate swap in the generated install profile ({}GB)...",
                    swap_size_gb
                ))
                .await;
        }
    } else {
        runner.out("Using zram-only swap (no hibernate)").await;
    }

    runner.step_complete("disk").await?;
    Ok(true)
}

/// Step 5b: Configure GPU bus IDs for hybrid GPU systems
async fn step_configure_gpu(
    runner: &CommandRunner<'_>,
    temp_config: &std::path::Path,
    hostname: &str,
) -> Result<bool> {
    use crate::system::hardware::{detect_gpu, GpuVendor};

    runner.out("Detecting GPU configuration...").await;

    // Detect GPU on the live system
    let gpu = match detect_gpu() {
        Ok(gpu) => gpu,
        Err(e) => {
            runner.out(&format!("GPU detection skipped: {}", e)).await;
            return Ok(true); // Non-fatal, continue installation
        }
    };

    // Only configure bus IDs for hybrid GPU systems
    if gpu.vendor != GpuVendor::HybridNvidiaAmd {
        runner
            .out(&format!(
                "GPU: {} (no PRIME configuration needed)",
                gpu.vendor
            ))
            .await;
        return Ok(true);
    }

    let hybrid = match &gpu.hybrid {
        Some(h) => h,
        None => {
            runner
                .out("Hybrid GPU detected but no bus IDs available")
                .await;
            return Ok(true);
        }
    };

    let amd_bus_id = match &hybrid.amd_bus_id {
        Some(id) => id.clone(),
        None => {
            runner
                .out("AMD iGPU bus ID not detected, skipping PRIME configuration")
                .await;
            return Ok(true);
        }
    };

    let nvidia_bus_id = match &hybrid.nvidia_bus_id {
        Some(id) => id.clone(),
        None => {
            runner
                .out("NVIDIA dGPU bus ID not detected, skipping PRIME configuration")
                .await;
            return Ok(true);
        }
    };

    runner
        .out(&format!(
            "Hybrid GPU detected: AMD iGPU ({}), NVIDIA dGPU ({})",
            amd_bus_id, nvidia_bus_id
        ))
        .await;

    // Update host config with detected bus IDs
    let host_config_file = format!(
        "{}/hosts/{}/default.nix",
        temp_config.to_string_lossy(),
        hostname
    );

    if !std::path::Path::new(&host_config_file).exists() {
        runner
            .out("Host config not found, skipping GPU bus ID configuration")
            .await;
        return Ok(true);
    }

    let content = std::fs::read_to_string(&host_config_file)
        .with_context(|| format!("Failed to read host config: {}", host_config_file))?;

    // Check if this host has PRIME configuration
    if !content.contains("amdgpuBusId") || !content.contains("nvidiaBusId") {
        runner
            .out("No PRIME configuration found in host config, skipping")
            .await;
        return Ok(true);
    }

    // Update bus IDs
    let updated = update_gpu_bus_ids(&content, &amd_bus_id, &nvidia_bus_id);

    if updated != content {
        std::fs::write(&host_config_file, &updated)
            .with_context(|| format!("Failed to write host config: {}", host_config_file))?;
        runner.out("GPU bus IDs configured successfully").await;
    } else {
        runner.out("GPU bus IDs already configured").await;
    }

    Ok(true)
}

/// Step 6: Run disko to partition and format disks
async fn step_run_disko(
    runner: &CommandRunner<'_>,
    temp_config: &std::path::Path,
    hostname: &str,
    disk: &str,
    username: &str,
    password: &str,
    swap_mode: &SwapMode,
) -> Result<bool> {
    let temp_config_str = temp_config.to_string_lossy();

    runner.out("Running disko to partition and format...").await;
    runner
        .out("Using provided passphrase for LUKS encryption...")
        .await;

    // Write password to temp file for disko with atomic 0600 permissions
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(LUKS_PASSWORD_FILE)
            .with_context(|| {
                format!(
                    "Failed to create LUKS password file: {}",
                    LUKS_PASSWORD_FILE
                )
            })?;
        file.write_all(password.as_bytes()).with_context(|| {
            format!("Failed to write LUKS password file: {}", LUKS_PASSWORD_FILE)
        })?;
        file.sync_all()
            .with_context(|| "Failed to sync LUKS password file")?;
    }

    write_installer_override(temp_config, hostname)?;
    runner
        .out("Installer-only encrypted-disk overrides prepared")
        .await;

    // Stage all new and modified files so nix flake evaluation can see them.
    // Nix ignores untracked files in git repos, so files created by create_host
    // (host config, hardware config, disko config) and modified by configure_disk
    // must be staged with git add.
    runner.out("Staging configuration changes...").await;
    let _ = run_capture("git", &["-C", &temp_config_str, "add", "-A"]).await;

    // Clean up leftovers from previous install attempts before partitioning.
    cleanup_previous_disk_state(runner, disk).await;

    // Build disko with streaming output. On a cold machine this can take a while
    // if substitutes are unavailable and Nix has to compile locally.
    runner
        .out("Building disko (fetching dependencies, this may take a while on a cold build)...")
        .await;
    let disko_link = format!("/tmp/forge-disko-{}", std::process::id());
    let _ = std::fs::remove_file(&disko_link);
    let _ = std::fs::remove_dir_all(&disko_link);
    let build_ok = runner
        .run_without_timeout(
            "nix",
            &[
                "build",
                &format!("{}#disko", temp_config_str),
                "--out-link",
                &disko_link,
            ],
        )
        .await?;

    let disko_path = std::fs::read_link(&disko_link)
        .ok()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let _ = std::fs::remove_file(&disko_link);

    if !build_ok || disko_path.trim().is_empty() {
        shred_and_remove(LUKS_PASSWORD_FILE);
        runner.err("Failed to build disko").await;
        runner
            .step_failed("disko", "Failed to build disko", "Disko build")
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    let disko_bin = format!("{}/bin/disko", disko_path.trim());
    runner
        .out(&format!("Running disko from {}...", disko_bin))
        .await;

    // Run disko with sudo to ensure EUID=0 (required by disko)
    let success = runner
        .run_without_timeout(
            "sudo",
            &[
                &disko_bin,
                "--yes-wipe-all-disks",
                "--mode",
                "destroy,format,mount",
                "--flake",
                &format!("{}#{}", temp_config_str, hostname),
            ],
        )
        .await?;

    // Securely clean up password file (overwrite before delete)
    shred_and_remove(LUKS_PASSWORD_FILE);

    if !success {
        runner
            .step_failed("disko", "Disk partitioning failed", "Disko partitioning")
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    // Get the LUKS UUID and update config to use by-uuid instead of by-partlabel
    // This matches what the NixOS graphical installer does and is more reliable
    runner.out("").await;
    runner.out("=== LUKS UUID Detection ===").await;
    runner
        .out("Detecting LUKS UUID for boot configuration...")
        .await;

    // Try multiple methods to find the LUKS device
    let mut luks_uuid: Option<String> = None;

    // Method 1: Try by-partlabel (disko's default)
    runner
        .out("  Trying /dev/disk/by-partlabel/cryptroot...")
        .await;
    let (ok1, uuid1, err1) = run_capture(
        "sudo",
        &["cryptsetup", "luksUUID", "/dev/disk/by-partlabel/cryptroot"],
    )
    .await?;
    if ok1 && !uuid1.trim().is_empty() {
        luks_uuid = Some(uuid1.trim().to_string());
        runner
            .out(&format!("  Found via by-partlabel: {}", uuid1.trim()))
            .await;
    } else {
        runner
            .out(&format!("  by-partlabel failed: {}", err1.trim()))
            .await;
    }

    // Method 2: If method 1 failed, try to find the backing device of /dev/mapper/cryptroot
    if luks_uuid.is_none() {
        runner
            .out("  Trying to find backing device of /dev/mapper/cryptroot...")
            .await;
        let (ok2, dmsetup_out, _) = run_capture(
            "sudo",
            &[
                "sh",
                "-c",
                "dmsetup deps -o devname cryptroot 2>/dev/null | grep -oP '\\(\\K[^)]+' | head -1",
            ],
        )
        .await?;
        if ok2 && !dmsetup_out.trim().is_empty() {
            let backing_dev = format!("/dev/{}", dmsetup_out.trim());
            runner
                .out(&format!("  Backing device: {}", backing_dev))
                .await;
            let (ok3, uuid3, _) =
                run_capture("sudo", &["cryptsetup", "luksUUID", &backing_dev]).await?;
            if ok3 && !uuid3.trim().is_empty() {
                luks_uuid = Some(uuid3.trim().to_string());
                runner
                    .out(&format!("  Found via dmsetup: {}", uuid3.trim()))
                    .await;
            }
        }
    }

    // Method 3: Try common NVMe partition paths
    if luks_uuid.is_none() {
        runner.out("  Trying common partition paths...").await;
        for dev in &["/dev/nvme0n1p2", "/dev/sda2", "/dev/vda2"] {
            let (ok, uuid, _) = run_capture("sudo", &["cryptsetup", "luksUUID", dev]).await?;
            if ok && !uuid.trim().is_empty() {
                luks_uuid = Some(uuid.trim().to_string());
                runner
                    .out(&format!("  Found via {}: {}", dev, uuid.trim()))
                    .await;
                break;
            }
        }
    }

    if let Some(uuid) = &luks_uuid {
        runner
            .out(&format!("  SUCCESS: LUKS UUID = {}", uuid))
            .await;
        let uses_lvm = host_uses_lvm(temp_config, hostname)?;
        let generated = build_local_install_config(
            username,
            disk,
            swap_mode,
            uses_lvm,
            Some(uuid.clone()),
            None,
        );
        write_local_install_config(temp_config, hostname, &generated)?;
        runner
            .out("  Recorded the exact encrypted-root device for boot")
            .await;
    } else {
        runner
            .err("  Could not detect LUKS UUID (informational only)")
            .await;
    }
    runner.out("=== End LUKS UUID Detection ===").await;
    runner.out("").await;

    runner.step_complete("disko").await?;
    Ok(true)
}

async fn cleanup_previous_disk_state(runner: &CommandRunner<'_>, disk: &str) {
    runner
        .out("Cleaning up previous install state on the target disk...")
        .await;

    let cleanup_commands: [(&str, &[&str]); 4] = [
        ("sudo", &["sh", "-c", "umount -R /mnt 2>/dev/null || true"]),
        ("sudo", &["sh", "-c", "swapoff -a 2>/dev/null || true"]),
        (
            "sudo",
            &[
                "sh",
                "-c",
                "cryptsetup luksClose cryptroot 2>/dev/null || true",
            ],
        ),
        (
            "sudo",
            &["sh", "-c", "dmsetup remove cryptroot 2>/dev/null || true"],
        ),
    ];

    for (cmd, args) in cleanup_commands {
        let _ = runner.run(cmd, args).await;
    }

    runner
        .out("Attempting a full destructive wipe of the target SSD/NVMe...")
        .await;
    let discard_ok = runner.run("sudo", &["blkdiscard", "-f", disk]).await.ok() == Some(true);

    if !discard_ok {
        runner
            .out("Full discard was not available, falling back to clearing disk metadata...")
            .await;
        let _ = runner
            .run(
                "sudo",
                &[
                    "sh",
                    "-c",
                    r#"
disk="$1"
size_bytes="$(blockdev --getsize64 "$disk" 2>/dev/null || echo 0)"
dd if=/dev/zero of="$disk" bs=1M count=64 conv=fsync,notrunc status=none || true
if [ "$size_bytes" -gt 67108864 ]; then
  seek_mb=$(( size_bytes / 1024 / 1024 - 64 ))
  dd if=/dev/zero of="$disk" bs=1M seek="$seek_mb" count=64 conv=fsync,notrunc status=none || true
fi
"#,
                    "cleanup-disk",
                    disk,
                ],
            )
            .await;
    }

    let _ = runner.run("sudo", &["wipefs", "-a", "-f", disk]).await;
    let _ = runner.run("sudo", &["blockdev", "--rereadpt", disk]).await;
    let _ = runner
        .run_with_timeout("sudo", &["udevadm", "settle", "--timeout=30"], 45)
        .await;
}

/// Step 6b: Configure hibernate boot settings (after disko, before nixos-install)
/// Detects resume_offset from swapfile and injects boot settings into host config
/// For LVM configs, hibernate is pre-configured in the host's default.nix
async fn step_configure_hibernate(
    runner: &CommandRunner<'_>,
    temp_config: &std::path::Path,
    hostname: &str,
    username: &str,
    disk: &str,
    swap_mode: &SwapMode,
) -> Result<bool> {
    if *swap_mode != SwapMode::HibernateSupport {
        return Ok(true); // Skip if not hibernate mode
    }

    runner.out("Configuring hibernate boot settings...").await;

    if host_uses_lvm(temp_config, hostname)? {
        runner
            .out("  Hibernate is already handled by the existing LVM layout")
            .await;
        return Ok(true);
    }

    // The swapfile should be at /mnt/swap/swapfile after disko
    let swapfile_path = "/mnt/swap/swapfile";
    if !std::path::Path::new(swapfile_path).exists() {
        runner
            .err(&format!("Swapfile not found at {}", swapfile_path))
            .await;
        runner
            .err("Hibernate configuration skipped - swapfile missing")
            .await;
        // Non-fatal - continue with installation, user can configure manually
        return Ok(true);
    }

    // Get resume_offset using btrfs inspect-internal map-swapfile
    runner
        .out("  Detecting resume_offset for swapfile...")
        .await;
    let (success, output, _) = run_capture(
        "sudo",
        &[
            "btrfs",
            "inspect-internal",
            "map-swapfile",
            "-r",
            swapfile_path,
        ],
    )
    .await?;

    if !success || output.trim().is_empty() {
        runner.err("  Failed to detect resume_offset").await;
        runner
            .err("  Hibernate configuration skipped - you can configure manually")
            .await;
        return Ok(true);
    }

    let resume_offset: u64 = match output.trim().parse() {
        Ok(offset) => offset,
        Err(_) => {
            runner
                .err(&format!("  Invalid resume_offset value: {}", output.trim()))
                .await;
            runner.err("  Hibernate configuration skipped").await;
            return Ok(true);
        }
    };

    runner
        .out(&format!("  Resume offset: {}", resume_offset))
        .await;

    let uses_lvm = host_uses_lvm(temp_config, hostname)?;
    let generated = build_local_install_config(
        username,
        disk,
        swap_mode,
        uses_lvm,
        detect_existing_luks_uuid(temp_config, hostname)?,
        Some(resume_offset),
    );
    write_local_install_config(temp_config, hostname, &generated)?;

    runner.out("  Hibernate boot settings configured:").await;
    runner
        .out("    - boot.resumeDevice = /dev/mapper/cryptroot")
        .await;
    runner
        .out(&format!("    - resume_offset = {}", resume_offset))
        .await;
    runner.out("    - zramSwap disabled").await;

    Ok(true)
}

/// Step 7: Install NixOS
async fn step_install_nixos(
    runner: &CommandRunner<'_>,
    temp_config: &std::path::Path,
    hostname: &str,
    username: &str,
) -> Result<bool> {
    let temp_config_str = temp_config.to_string_lossy();

    runner.out("Installing NixOS...").await;
    validate_config_tree(temp_config, hostname).with_context(|| {
        format!(
            "Source configuration is incomplete before install copy: {}",
            temp_config.display()
        )
    })?;

    let config_dir = get_config_dir(username);
    let symlink_target = get_symlink_target(username);
    runner.out(&format!("  Config dir: {}", config_dir)).await;
    runner.out(&format!("  Source: {}", temp_config_str)).await;

    // Verify /mnt is mounted
    runner.out("  Checking mount point...").await;
    if !std::path::Path::new("/mnt").exists() {
        runner.err("ERROR: /mnt does not exist!").await;
        runner
            .step_failed("NixOS", "/mnt mount point missing", "NixOS installation")
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    // List what's in /mnt to debug
    if let Ok(entries) = std::fs::read_dir("/mnt") {
        let dirs: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        runner.out(&format!("  /mnt contents: {:?}", dirs)).await;
    }

    // Show mount points under /mnt for debugging
    let (_, mount_out, _) =
        run_capture("sh", &["-c", "mount | grep /mnt || echo 'No mounts found'"])
            .await
            .unwrap_or((false, "mount command failed".to_string(), String::new()));
    for line in mount_out.lines().take(10) {
        runner.out(&format!("  mount: {}", line)).await;
    }

    // Verify /mnt/home exists (btrfs @home subvolume should be mounted here)
    let mnt_home = std::path::Path::new("/mnt/home");
    if !mnt_home.exists() {
        runner
            .err(
                "ERROR: /mnt/home does not exist! Disko may not have mounted subvolumes correctly.",
            )
            .await;
        runner
            .step_failed(
                "NixOS",
                "/mnt/home missing - disko mount issue",
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }
    runner
        .out("  /mnt/home exists (btrfs subvolume mounted)")
        .await;

    // Remove installer-only overrides before copying the configuration into the
    // installed system.
    if installer_override_path(temp_config, hostname).exists() {
        remove_if_exists(&installer_override_path(temp_config, hostname))?;
        runner
            .out("  Removed installer-only encrypted-disk overrides")
            .await;
    }

    // Copy configuration to user home directory
    // Note: We use sudo for all file operations because nix run doesn't preserve EUID=0
    runner
        .out(&format!(
            "  Creating user directory: /mnt/home/{}",
            username
        ))
        .await;
    let config_parent = std::path::Path::new(&config_dir)
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid config directory path: {}", config_dir))?;
    runner
        .out(&format!("  Target path: {}", config_parent.display()))
        .await;

    // Use sudo mkdir -p since nix run doesn't preserve root privileges
    let mkdir_ok = runner
        .run("sudo", &["mkdir", "-p", &config_parent.to_string_lossy()])
        .await?;
    if !mkdir_ok {
        runner
            .err(&format!(
                "ERROR: Failed to create {}",
                config_parent.display()
            ))
            .await;
        runner
            .step_failed(
                "NixOS",
                &format!("Failed to create {}", config_parent.display()),
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }
    runner
        .out(&format!("  Created: {}", config_parent.display()))
        .await;

    if !ensure_home_skeleton_sudo(runner, config_parent).await? {
        runner
            .step_failed(
                "NixOS",
                "Failed to create user home skeleton",
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    runner.out("  Cleaning target configuration...").await;
    let clean_target_ok = runner.run("sudo", &["rm", "-rf", &config_dir]).await?;
    if !clean_target_ok {
        runner
            .step_failed(
                "NixOS",
                "Failed to remove previous target configuration",
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    let mkdir_config_ok = runner.run("sudo", &["mkdir", "-p", &config_dir]).await?;
    if !mkdir_config_ok {
        runner
            .step_failed(
                "NixOS",
                &format!("Failed to create {}", config_dir),
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    // Copy configuration contents using sudo. The trailing "/." keeps the
    // installed layout as /mnt/home/<user>/nixos-config/flake.nix and avoids
    // accidentally nesting the repo under nixos-config/nixos-config.
    runner.out("  Copying configuration...").await;
    let source_contents = format!("{}/.", temp_config_str);
    let copy_ok = runner
        .run(
            "sudo",
            &["cp", "-a", &source_contents, &format!("{}/.", config_dir)],
        )
        .await?;
    if !copy_ok {
        runner
            .err(&format!(
                "ERROR: Failed to copy {} to {}",
                temp_config_str, config_dir
            ))
            .await;
        runner
            .step_failed(
                "NixOS",
                "Failed to copy configuration",
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    // Remove .git from copied config
    let _ = runner
        .run("sudo", &["rm", "-rf", &format!("{}/.git", config_dir)])
        .await;

    // Create symlink using sudo
    runner.out("  Setting up symlink...").await;
    let symlink_ok = setup_config_symlink_sudo(runner, &symlink_target).await?;
    if !symlink_ok {
        runner
            .step_failed(
                "NixOS",
                "Failed to create /etc/nixos symlink",
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    if let Err(error) = verify_installed_config_layout_at(
        std::path::Path::new(INSTALL_MOUNT_POINT),
        username,
        hostname,
    ) {
        runner
            .step_failed(
                "NixOS",
                &format!("Installed config verification failed: {:#}", error),
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }
    runner
        .out("  Verified installed config flake and /etc/nixos link")
        .await;

    // Initialize git repo (optional, log failures)
    init_git_repo(runner, &config_dir).await;

    // Set ownership after all copy/git work so generated files are not left root-owned.
    if !set_home_ownership_sudo(runner, config_parent, PRIMARY_USER_UID, PRIMARY_USER_GID).await? {
        runner
            .step_failed(
                "NixOS",
                "Failed to set user home ownership",
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    if let Err(error) = verify_home_sanity_at(
        std::path::Path::new(INSTALL_MOUNT_POINT),
        username,
        PRIMARY_USER_UID,
        PRIMARY_USER_GID,
    ) {
        runner
            .step_failed(
                "NixOS",
                &format!("Pre-install home sanity check failed: {:#}", error),
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    runner.out("  Configuration ready.").await;

    // Verify the selected host evaluates before starting the long install build.
    runner.out("Checking NixOS configuration...").await;
    let flake_ref = format!("{}#{}", config_dir, hostname);
    let eval_attr = format!(
        "{}#nixosConfigurations.{}.config.system.build.toplevel.drvPath",
        config_dir, hostname
    );
    let eval_ok = runner
        .run(
            "sudo",
            &[
                "env",
                &format!("NIX_CONFIG={}", NIX_CONFIG_VALUE),
                "nix",
                "eval",
                &eval_attr,
                "--raw",
            ],
        )
        .await
        .unwrap_or(false);

    if !eval_ok {
        runner
            .err("Selected host failed to evaluate - refusing to continue with an invalid configuration.")
            .await;
        runner
            .step_failed(
                "NixOS",
                "selected NixOS host evaluation failed",
                "NixOS configuration evaluation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    // Run nixos-install with sudo (nix run doesn't preserve root privileges).
    // Fresh installs can legitimately run well past 30 minutes if cached binaries
    // are unavailable and the target system needs to compile packages locally.
    runner
        .out(&format!(
            "Running: sudo env NIX_CONFIG=... nixos-install --flake {}",
            flake_ref
        ))
        .await;
    runner
        .out("  (This can take a long time on a fresh install; compiler activity is normal.)")
        .await;
    let success = runner
        .run_without_timeout(
            "sudo",
            &[
                "env",
                &format!("NIX_CONFIG={}", NIX_CONFIG_VALUE),
                "nixos-install",
                "--flake",
                &flake_ref,
                "--no-root-passwd",
            ],
        )
        .await?;

    if !success {
        runner
            .err("nixos-install failed! Check the output above for errors.")
            .await;
        runner
            .step_failed("NixOS", "nixos-install failed", "NixOS installation")
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    if !ensure_home_skeleton_sudo(runner, config_parent).await? {
        runner
            .step_failed(
                "NixOS",
                "Failed to recreate user home skeleton after install",
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    let target_user = match read_target_user(std::path::Path::new(INSTALL_MOUNT_POINT), username) {
        Ok(user) => user,
        Err(error) => {
            runner
                .step_failed(
                    "NixOS",
                    &format!(
                        "Failed to read installed user before ownership fix: {:#}",
                        error
                    ),
                    "NixOS installation",
                )
                .await?;
            runner.done(false).await?;
            return Ok(false);
        }
    };

    if !set_home_ownership_sudo(runner, config_parent, target_user.uid, target_user.gid).await? {
        runner
            .step_failed(
                "NixOS",
                "Failed to set final user home ownership",
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    if let Err(error) = verify_installed_sanity_at(
        std::path::Path::new(INSTALL_MOUNT_POINT),
        username,
        hostname,
    ) {
        runner
            .step_failed(
                "NixOS",
                &format!(
                    "Post-install sanity gate failed; refusing to mark install successful: {:#}",
                    error
                ),
                "NixOS installation",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }
    runner
        .out("  Post-install config and home sanity checks passed")
        .await;

    runner.step_complete("NixOS").await?;
    Ok(true)
}

/// Step 8: Set user password
async fn step_set_user_password(
    runner: &CommandRunner<'_>,
    hostname: &str,
    username: &str,
    password: &str,
) -> Result<bool> {
    runner.out("Setting up user account...").await;

    // Use stdin so the password never appears in shell source or process args.
    // sudo is required because nix run does not preserve root privileges.
    let chpasswd_input = format!("{}:{}\n", username, password);
    let success = run_command_sensitive_with_input(
        runner.tx(),
        "sudo",
        &["nixos-enter", "--root", "/mnt", "-c", "chpasswd"],
        &chpasswd_input,
    )
    .await?;

    if !success {
        runner
            .step_failed(
                super::steps::INSTALL,
                "Failed to set installed user password",
                "Set user password",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    runner.out("Running final install sanity gate...").await;
    if let Err(error) = verify_installed_sanity_at(
        std::path::Path::new(INSTALL_MOUNT_POINT),
        username,
        hostname,
    ) {
        runner
            .step_failed(
                super::steps::INSTALL,
                &format!(
                    "Final install sanity gate failed; refusing to mark install successful: {:#}",
                    error
                ),
                "Install sanity verification",
            )
            .await?;
        runner.done(false).await?;
        return Ok(false);
    }

    runner.out("  Final install sanity gate passed").await;
    runner.step_complete(super::steps::INSTALL).await?;
    Ok(true)
}

/// Show installation completion message
async fn show_completion_message(runner: &CommandRunner<'_>, username: &str) -> Result<()> {
    runner.out("\n").await;
    runner.out("Installation complete!").await;
    runner.out("").await;
    runner.out("Next steps:").await;
    runner.out("  1. Reboot: reboot").await;
    runner.out("  2. Enter your LUKS passphrase at boot").await;
    runner.out("  3. Select a shell from the boot menu").await;
    runner
        .out(&format!(
            "  4. Login as '{}' with your chosen password",
            username
        ))
        .await;
    Ok(())
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Set up the /mnt/etc/nixos symlink using sudo (for nix run compatibility)
async fn setup_config_symlink_sudo(
    runner: &CommandRunner<'_>,
    symlink_target: &str,
) -> Result<bool> {
    let symlink_parent = std::path::Path::new(INSTALL_SYMLINK_PATH)
        .parent()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid symlink path: cannot determine parent of {}",
                INSTALL_SYMLINK_PATH
            )
        })?;

    // Create parent directory
    if !runner
        .run("sudo", &["mkdir", "-p", &symlink_parent.to_string_lossy()])
        .await?
    {
        runner
            .err(&format!("Failed to create {}", symlink_parent.display()))
            .await;
        return Ok(false);
    }

    // Remove existing symlink/file/directory
    let _ = runner
        .run("sudo", &["rm", "-rf", INSTALL_SYMLINK_PATH])
        .await;

    // Create symlink
    if !runner
        .run("sudo", &["ln", "-s", symlink_target, INSTALL_SYMLINK_PATH])
        .await?
    {
        runner
            .err(&format!(
                "Failed to create symlink {} -> {}",
                INSTALL_SYMLINK_PATH, symlink_target
            ))
            .await;
        return Ok(false);
    }

    Ok(true)
}

/// Initialize git repository in the config directory
async fn init_git_repo(runner: &CommandRunner<'_>, config_dir: &str) {
    // Use sudo for git operations since the config dir is owned by root at this point
    match runner
        .run(
            "sudo",
            &[
                "nix-shell",
                "-p",
                "git",
                "--run",
                &format!(
                    "cd {} && git init -b main && git remote add origin {} && git add -A && \
                    git -c user.name='NixOS Install' -c user.email='install@localhost' \
                    commit -m 'Initial configuration' && git fetch origin && \
                    git branch --set-upstream-to=origin/main main",
                    config_dir, REPO_URL
                ),
            ],
        )
        .await
    {
        Ok(true) => tracing::info!("Git repository initialized successfully"),
        Ok(false) => {
            tracing::warn!("Git repository initialization returned non-zero exit - continuing")
        }
        Err(e) => tracing::warn!("Git repository initialization error: {} - continuing", e),
    }
}

/// Create the user-owned directory skeleton that first login and Home Manager expect.
async fn ensure_home_skeleton_sudo(
    runner: &CommandRunner<'_>,
    home_dir: &std::path::Path,
) -> Result<bool> {
    let home_dir = home_dir.to_string_lossy().to_string();
    let paths = [
        home_dir.clone(),
        format!("{home_dir}/.config/fish"),
        format!("{home_dir}/.local/state"),
        format!("{home_dir}/.cache"),
    ];

    let mut args = vec!["mkdir", "-p"];
    args.extend(paths.iter().map(String::as_str));

    match runner.run("sudo", &args).await {
        Ok(true) => {
            tracing::info!("Created target home skeleton");
            Ok(true)
        }
        Ok(false) => {
            runner
                .err(&format!("Failed to create home skeleton under {home_dir}"))
                .await;
            Ok(false)
        }
        Err(e) => {
            runner
                .err(&format!(
                    "Failed to create home skeleton under {home_dir}: {e}"
                ))
                .await;
            Ok(false)
        }
    }
}

/// Set final ownership and directory permissions for the installed user's home.
async fn set_home_ownership_sudo(
    runner: &CommandRunner<'_>,
    home_dir: &std::path::Path,
    uid: u32,
    gid: u32,
) -> Result<bool> {
    let uid_gid = format!("{uid}:{gid}");
    let home_dir = home_dir.to_string_lossy().to_string();

    match runner
        .run("sudo", &["chown", "-R", &uid_gid, &home_dir])
        .await
    {
        Ok(true) => tracing::info!("Set ownership on installed home directory"),
        Ok(false) => {
            runner
                .err(&format!("Failed to set ownership on {home_dir}"))
                .await;
            return Ok(false);
        }
        Err(e) => {
            runner
                .err(&format!("Failed to set ownership on {home_dir}: {e}"))
                .await;
            return Ok(false);
        }
    }

    match runner
        .run(
            "sudo",
            &[
                "find", &home_dir, "-type", "d", "-exec", "chmod", "u+rwx", "{}", "+",
            ],
        )
        .await
    {
        Ok(true) => tracing::info!("Ensured installed home directories are user-writable"),
        Ok(false) => {
            runner
                .err(&format!(
                    "Failed to make home directories user-writable under {home_dir}"
                ))
                .await;
            return Ok(false);
        }
        Err(e) => {
            runner
                .err(&format!(
                    "Failed to make home directories user-writable under {home_dir}: {e}"
                ))
                .await;
            return Ok(false);
        }
    }

    Ok(true)
}

// =============================================================================
// Main Installation Function
// =============================================================================

async fn run_install(
    tx: &mpsc::Sender<CommandMessage>,
    hostname: &str,
    disk: &str,
    username: &str,
    password: &str,
    swap_mode: &SwapMode,
    refresh_hardware_config: bool,
) -> Result<()> {
    let runner = CommandRunner::new(tx);

    // Step 1: Check network
    if !step_check_network(&runner).await? {
        return Ok(());
    }

    // Step 2: Enable flakes
    if !step_enable_flakes(&runner).await? {
        return Ok(());
    }

    // Step 3: Prepare repository
    let temp_config = match step_prepare_repository(&runner, hostname).await? {
        Some(path) => path,
        None => return Ok(()),
    };
    reset_generated_install_state(&temp_config, hostname)?;

    // Step 4: Refresh machine-detected hardware profile
    if !step_refresh_hardware_config(&runner, &temp_config, hostname, refresh_hardware_config)
        .await?
    {
        return Ok(());
    }

    // Step 5: Configure disk (including swap mode)
    if !step_configure_disk(&runner, &temp_config, hostname, disk, username, swap_mode).await? {
        return Ok(());
    }

    // Step 5b: Configure GPU bus IDs (for hybrid GPU systems)
    if !step_configure_gpu(&runner, &temp_config, hostname).await? {
        return Ok(());
    }

    // Step 6: Run disko
    if !step_run_disko(
        &runner,
        &temp_config,
        hostname,
        disk,
        username,
        password,
        swap_mode,
    )
    .await?
    {
        return Ok(());
    }

    // Step 6b: Configure hibernate boot settings (if hibernate mode selected)
    if !step_configure_hibernate(&runner, &temp_config, hostname, username, disk, swap_mode).await?
    {
        return Ok(());
    }

    // Step 7: Install NixOS
    if !step_install_nixos(&runner, &temp_config, hostname, username).await? {
        return Ok(());
    }

    // Step 8: Set user password
    if !step_set_user_password(&runner, hostname, username, password).await? {
        return Ok(());
    }

    // Show completion message
    show_completion_message(&runner, username).await?;

    runner.done(true).await?;
    Ok(())
}

/// Update GPU bus IDs in host configuration for NVIDIA PRIME
fn update_gpu_bus_ids(content: &str, amd_bus_id: &str, nvidia_bus_id: &str) -> String {
    let amd_replacement = format!("amdgpuBusId = \"{}\"", amd_bus_id);
    let nvidia_replacement = format!("nvidiaBusId = \"{}\"", nvidia_bus_id);

    let result = AMD_BUS_ID_RE.replace_all(content, amd_replacement.as_str());
    let result = NVIDIA_BUS_ID_RE.replace_all(&result, nvidia_replacement.as_str());

    result.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_root(test_name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "forge-install-{}-{}-{}",
            test_name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn create_minimal_config_tree(root: &Path, hostname: &str) {
        std::fs::create_dir_all(root.join(constants::HOSTS_SUBDIR).join(hostname)).unwrap();
        std::fs::create_dir_all(root.join("modules").join("disko")).unwrap();
        std::fs::create_dir_all(root.join("home")).unwrap();
        std::fs::create_dir_all(root.join("packages")).unwrap();
        std::fs::create_dir_all(root.join("wallpapers")).unwrap();
        std::fs::write(root.join(constants::FLAKE_NIX), "{}\n").unwrap();
        std::fs::write(root.join("flake.lock"), "{}\n").unwrap();
        std::fs::write(
            root.join(constants::HOSTS_SUBDIR)
                .join(hostname)
                .join("default.nix"),
            "{}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("modules")
                .join("disko")
                .join(format!("{hostname}.nix")),
            "{}\n",
        )
        .unwrap();
    }

    fn create_home_skeleton(root: &Path, username: &str) {
        let home = target_home_dir(root, username);
        std::fs::create_dir_all(home.join(".config/fish")).unwrap();
        std::fs::create_dir_all(home.join(".local/state")).unwrap();
        std::fs::create_dir_all(home.join(".cache")).unwrap();
    }

    fn write_passwd(root: &Path, username: &str, uid: u32, gid: u32, home: &str, shell: &str) {
        std::fs::create_dir_all(root.join("etc")).unwrap();
        std::fs::write(
            root.join("etc/passwd"),
            format!(
                "root:x:0:0:System administrator:/root:/bin/sh\n{username}:x:{uid}:{gid}::{home}:{shell}\n"
            ),
        )
        .unwrap();
    }

    fn current_uid_gid(path: &Path) -> (u32, u32) {
        let metadata = std::fs::symlink_metadata(path).unwrap();
        (metadata.uid(), metadata.gid())
    }

    #[test]
    fn generated_install_config_renders_persisted_overrides() {
        let config = GeneratedInstallConfig {
            username: "alice".to_string(),
            disk: "/dev/nvme0n1".to_string(),
            hibernate_swap_size_gb: Some(34),
            luks_uuid: Some("1234-uuid".to_string()),
            resume_offset: Some(987654),
        };

        let rendered = render_local_install_config(&config);
        assert!(rendered.contains("forge.installer.username = \"alice\";"));
        assert!(rendered.contains("disko.devices.disk.main.device = lib.mkForce \"/dev/nvme0n1\";"));
        assert!(rendered.contains("\"@swap\""));
        assert!(rendered.contains("/dev/disk/by-uuid/1234-uuid"));
        assert!(rendered.contains("resume_offset=987654"));
    }

    #[test]
    fn generated_install_config_omits_optional_sections_when_not_needed() {
        let config = GeneratedInstallConfig {
            username: "john".to_string(),
            disk: "/dev/sda".to_string(),
            hibernate_swap_size_gb: None,
            luks_uuid: None,
            resume_offset: None,
        };

        let rendered = render_local_install_config(&config);
        assert!(rendered.contains("forge.installer.username = \"john\";"));
        assert!(!rendered.contains("\"@swap\""));
        assert!(!rendered.contains("resume_offset="));
        assert!(!rendered.contains("boot.resumeDevice"));
    }

    #[test]
    fn detect_existing_luks_uuid_reads_generated_local_override() {
        let temp_root = std::env::temp_dir().join(format!(
            "forge-install-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let host_path = temp_root.join(constants::HOSTS_SUBDIR).join("xps");
        std::fs::create_dir_all(&host_path).unwrap();

        let config = GeneratedInstallConfig {
            username: "john".to_string(),
            disk: "/dev/nvme0n1".to_string(),
            hibernate_swap_size_gb: None,
            luks_uuid: Some("uuid-from-test".to_string()),
            resume_offset: None,
        };
        write_local_install_config(&temp_root, "xps", &config).unwrap();

        let detected = detect_existing_luks_uuid(&temp_root, "xps").unwrap();
        assert_eq!(detected.as_deref(), Some("uuid-from-test"));

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn parse_passwd_user_reads_home_and_shell() {
        let user = parse_passwd_user(
            "john:x:1000:100::/home/john:/run/current-system/sw/bin/fish",
            "john",
        )
        .unwrap()
        .unwrap();

        assert_eq!(user.name, "john");
        assert_eq!(user.uid, 1000);
        assert_eq!(user.gid, 100);
        assert_eq!(user.home, PathBuf::from("/home/john"));
        assert_eq!(user.shell, PathBuf::from("/run/current-system/sw/bin/fish"));
    }

    #[test]
    fn verify_installed_config_layout_accepts_installed_system_symlink() {
        let mount_root = unique_temp_root("layout-ok");
        let hostname = "z2-mini-g1a";
        let username = "alice";
        let config_dir = target_config_dir(&mount_root, username);
        create_minimal_config_tree(&config_dir, hostname);
        std::fs::create_dir_all(mount_root.join("etc")).unwrap();
        std::os::unix::fs::symlink(
            get_symlink_target(username),
            target_etc_nixos_path(&mount_root),
        )
        .unwrap();

        verify_installed_config_layout_at(&mount_root, username, hostname).unwrap();

        let _ = std::fs::remove_dir_all(&mount_root);
    }

    #[test]
    fn verify_installed_config_layout_rejects_mnt_symlink_target() {
        let mount_root = unique_temp_root("layout-bad-symlink");
        let hostname = "z2-mini-g1a";
        let username = "alice";
        let config_dir = target_config_dir(&mount_root, username);
        create_minimal_config_tree(&config_dir, hostname);
        std::fs::create_dir_all(mount_root.join("etc")).unwrap();
        std::os::unix::fs::symlink(
            format!("/mnt/home/{}/{}", username, NIXOS_CONFIG_HOME_DIR),
            target_etc_nixos_path(&mount_root),
        )
        .unwrap();

        let error = verify_installed_config_layout_at(&mount_root, username, hostname).unwrap_err();
        assert!(error
            .to_string()
            .contains("Invalid /etc/nixos symlink target"));

        let _ = std::fs::remove_dir_all(&mount_root);
    }

    #[test]
    fn verify_installed_sanity_accepts_owned_home_and_config() {
        let mount_root = unique_temp_root("sanity-ok");
        let hostname = "z2-mini-g1a";
        let username = "alice";
        let config_dir = target_config_dir(&mount_root, username);
        create_minimal_config_tree(&config_dir, hostname);
        create_home_skeleton(&mount_root, username);
        std::fs::create_dir_all(mount_root.join("etc")).unwrap();
        std::os::unix::fs::symlink(
            get_symlink_target(username),
            target_etc_nixos_path(&mount_root),
        )
        .unwrap();
        let (uid, gid) = current_uid_gid(&target_home_dir(&mount_root, username));
        write_passwd(
            &mount_root,
            username,
            uid,
            gid,
            "/home/alice",
            "/run/current-system/sw/bin/fish",
        );

        verify_installed_sanity_at(&mount_root, username, hostname).unwrap();

        let _ = std::fs::remove_dir_all(&mount_root);
    }

    #[test]
    fn verify_installed_sanity_rejects_unwritable_home_for_user() {
        let mount_root = unique_temp_root("sanity-bad-home");
        let hostname = "z2-mini-g1a";
        let username = "alice";
        let config_dir = target_config_dir(&mount_root, username);
        create_minimal_config_tree(&config_dir, hostname);
        create_home_skeleton(&mount_root, username);
        std::fs::create_dir_all(mount_root.join("etc")).unwrap();
        std::os::unix::fs::symlink(
            get_symlink_target(username),
            target_etc_nixos_path(&mount_root),
        )
        .unwrap();
        let (uid, gid) = current_uid_gid(&target_home_dir(&mount_root, username));
        write_passwd(
            &mount_root,
            username,
            uid.saturating_add(1),
            gid,
            "/home/alice",
            "/run/current-system/sw/bin/fish",
        );

        let error = verify_installed_sanity_at(&mount_root, username, hostname).unwrap_err();
        assert!(error.to_string().contains("not writable"));

        let _ = std::fs::remove_dir_all(&mount_root);
    }

    #[test]
    fn verify_no_root_fish_artifacts_rejects_bad_xdg_paths() {
        let mount_root = unique_temp_root("root-fish-artifacts");
        std::fs::create_dir_all(&mount_root).unwrap();
        std::fs::write(mount_root.join("config.fish"), "# wrong place\n").unwrap();

        let error = verify_no_root_fish_artifacts(&mount_root).unwrap_err();
        assert!(error.to_string().contains("unexpected root-level"));

        let _ = std::fs::remove_dir_all(&mount_root);
    }

    #[test]
    fn validate_config_tree_rejects_missing_flake() {
        let root = unique_temp_root("missing-flake");
        std::fs::create_dir_all(&root).unwrap();

        let error = validate_config_tree(&root, "xps").unwrap_err();
        assert!(error.to_string().contains("missing flake.nix"));

        let _ = std::fs::remove_dir_all(&root);
    }
}
