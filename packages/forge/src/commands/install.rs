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
use std::sync::LazyLock;
use tokio::sync::mpsc;

use super::errors::{ErrorContext, ParsedError};
use super::executor::{run_capture, run_command_sensitive};
use super::runner::CommandRunner;
use super::CommandMessage;
use crate::constants::{
    self, INSTALL_MOUNT_POINT, INSTALL_SYMLINK_PATH, NIXOS_CONFIG_HOME_DIR,
    PRIMARY_USER_GID, PRIMARY_USER_UID,
};

// =============================================================================
// Install Constants
// =============================================================================

/// Path to the temporary LUKS password file (used by disko)
const LUKS_PASSWORD_FILE: &str = "/tmp/luks-password";

/// GitHub repository URL for the NixOS configuration
const REPO_URL: &str = "https://github.com/DigitalPals/nixos-config.git";

/// Default username used in the configuration
const DEFAULT_USERNAME: &str = "john";

// =============================================================================
// Regex Patterns
// =============================================================================

/// Regex to match disko device declarations.
static DISK_DEVICE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"device = "/dev/[^"]*""#)
        .expect("Disk device regex pattern is statically validated")
});

/// Regex to match the LUKS content section where we need to inject passwordFile.
static LUKS_NAME_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(name = "cryptroot";)"#)
        .expect("LUKS name regex pattern is statically validated")
});

/// Get the config directory path for the mounted system
fn get_config_dir(username: &str) -> String {
    format!("{}/home/{}/{}", INSTALL_MOUNT_POINT, username, NIXOS_CONFIG_HOME_DIR)
}

/// Get the symlink target (path on the installed system, not /mnt)
fn get_symlink_target(username: &str) -> String {
    format!("/home/{}/{}", username, NIXOS_CONFIG_HOME_DIR)
}

/// Start the installation process
pub async fn start_install(
    tx: mpsc::Sender<CommandMessage>,
    hostname: &str,
    disk: &str,
    username: &str,
    password: &str,
) -> Result<()> {
    let hostname = hostname.to_string();
    let disk = disk.to_string();
    let username = username.to_string();
    let password = password.to_string();

    tokio::spawn(async move {
        if let Err(e) = run_install(&tx, &hostname, &disk, &username, &password).await {
            tracing::error!("Installation failed: {}", e);
            let _ = tx
                .send(CommandMessage::StepFailed {
                    step: "Install".to_string(),
                    error: ParsedError::from_stderr(
                        &e.to_string(),
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

// =============================================================================
// Installation Steps
// =============================================================================

/// Step 1: Check network connectivity
async fn step_check_network(runner: &CommandRunner<'_>) -> Result<bool> {
    runner.out("Checking network connectivity...").await;

    let (success, _, _) = run_capture("ping", &["-c", "1", "-W", "5", "github.com"]).await?;
    if !success {
        runner.step_failed("network", "No network connection", "Network check").await?;
        runner.done(false).await?;
        return Ok(false);
    }

    runner.step_complete("network").await?;
    Ok(true)
}

/// Step 2: Enable Nix flakes
async fn step_enable_flakes(runner: &CommandRunner<'_>) -> Result<bool> {
    runner.out("Enabling Nix flakes...").await;
    std::env::set_var("NIX_CONFIG", "experimental-features = nix-command flakes");
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
        runner.out("Using existing configuration (host already created)...").await;
    } else {
        runner.out("Cloning configuration repository...").await;
        let _ = std::fs::remove_dir_all(&temp_config);

        let success = runner
            .run(
                "nix-shell",
                &[
                    "-p",
                    "git",
                    "--run",
                    &format!("git clone --depth 1 {} {}", REPO_URL, temp_config_str),
                ],
            )
            .await?;

        if !success {
            runner.step_failed("repository", "Failed to clone repository", "Clone repository").await?;
            runner.done(false).await?;
            return Ok(None);
        }
    }

    runner.step_complete("repository").await?;
    Ok(Some(temp_config))
}

/// Step 4: Configure disk device and update disko configuration
async fn step_configure_disk(
    runner: &CommandRunner<'_>,
    temp_config: &std::path::Path,
    hostname: &str,
    disk: &str,
    username: &str,
) -> Result<bool> {
    let temp_config_str = temp_config.to_string_lossy();
    runner.out(&format!("Configuring disk device {}...", disk)).await;

    // Validate disk path format
    if !disk.starts_with("/dev/") {
        runner.step_failed(
            "disk",
            &format!("Invalid disk path: {}. Must start with /dev/", disk),
            "Disk validation",
        ).await?;
        runner.done(false).await?;
        return Ok(false);
    }

    // Check that disk device actually exists
    if !std::path::Path::new(disk).exists() {
        runner.step_failed(
            "disk",
            &format!("Disk device does not exist: {}", disk),
            "Disk validation",
        ).await?;
        runner.done(false).await?;
        return Ok(false);
    }

    // Check disko config file exists
    let disko_file = format!("{}/modules/disko/{}.nix", temp_config_str, hostname);
    if !std::path::Path::new(&disko_file).exists() {
        runner.step_failed(
            "disk",
            &format!(
                "No disko configuration found for host '{}'. Expected: modules/disko/{}.nix",
                hostname, hostname
            ),
            "Disk configuration",
        ).await?;
        runner.done(false).await?;
        return Ok(false);
    }

    // Update disko config with disk device
    let disko_content = std::fs::read_to_string(&disko_file)
        .with_context(|| format!("Failed to read disko config: {}", disko_file))?;
    let updated_content = update_disk_device(&disko_content, disk);
    std::fs::write(&disko_file, &updated_content)
        .with_context(|| format!("Failed to write disko config: {}", disko_file))?;

    // Update flake.nix with username if it differs from default
    if username != DEFAULT_USERNAME {
        runner.out(&format!("Configuring username '{}'...", username)).await;

        let flake_file = format!("{}/flake.nix", temp_config_str);
        let flake_content = std::fs::read_to_string(&flake_file)
            .with_context(|| format!("Failed to read flake.nix: {}", flake_file))?;
        let updated_flake = update_flake_username(&flake_content, hostname, username);
        std::fs::write(&flake_file, &updated_flake)
            .with_context(|| format!("Failed to write flake.nix: {}", flake_file))?;
    }

    runner.step_complete("disk").await?;
    Ok(true)
}

/// Step 5: Run disko to partition and format disks
async fn step_run_disko(
    runner: &CommandRunner<'_>,
    temp_config: &std::path::Path,
    hostname: &str,
    password: &str,
) -> Result<bool> {
    let temp_config_str = temp_config.to_string_lossy();

    runner.out("Running disko to partition and format...").await;
    runner.out("Using provided passphrase for LUKS encryption...").await;

    // Write password to temp file for disko
    std::fs::write(LUKS_PASSWORD_FILE, password.as_bytes())
        .with_context(|| format!("Failed to write LUKS password file: {}", LUKS_PASSWORD_FILE))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(LUKS_PASSWORD_FILE, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Failed to set permissions on {}", LUKS_PASSWORD_FILE))?;
    }

    // Inject passwordFile into disko default.nix
    let disko_default_file = format!("{}/modules/disko/default.nix", temp_config_str);
    let disko_default_content = std::fs::read_to_string(&disko_default_file)
        .with_context(|| format!("Failed to read disko default.nix: {}", disko_default_file))?;
    let updated_disko = inject_luks_password_file(&disko_default_content);
    std::fs::write(&disko_default_file, &updated_disko)
        .with_context(|| format!("Failed to write disko default.nix: {}", disko_default_file))?;

    // Verify passwordFile injection
    if updated_disko.contains("passwordFile") {
        runner.out("LUKS passwordFile configured successfully").await;
        tracing::info!("passwordFile injection confirmed in disko config");
    } else {
        runner.err("WARNING: passwordFile injection may have failed!").await;
        tracing::error!("passwordFile NOT found in modified disko config");
    }

    // Pre-fetch disko (optional optimization)
    match runner.run("nix", &["build", &format!("{}#disko", temp_config_str), "--no-link"]).await {
        Ok(true) => tracing::info!("Disko pre-fetch succeeded"),
        Ok(false) => tracing::warn!("Disko pre-fetch failed - continuing anyway"),
        Err(e) => tracing::warn!("Disko pre-fetch error: {} - continuing anyway", e),
    }

    // Run disko
    let success = runner
        .run(
            "nix",
            &[
                "run",
                &format!("{}#disko", temp_config_str),
                "--",
                "--yes-wipe-all-disks",
                "--mode",
                "destroy,format,mount",
                "--flake",
                &format!("{}#{}", temp_config_str, hostname),
            ],
        )
        .await?;

    // Clean up password file immediately (security)
    if let Err(e) = std::fs::remove_file(LUKS_PASSWORD_FILE) {
        tracing::warn!("Failed to remove LUKS password file: {}", e);
    }

    if !success {
        runner.step_failed("disko", "Disk partitioning failed", "Disko partitioning").await?;
        runner.done(false).await?;
        return Ok(false);
    }

    runner.step_complete("disko").await?;
    Ok(true)
}

/// Step 6: Install NixOS
async fn step_install_nixos(
    runner: &CommandRunner<'_>,
    temp_config: &std::path::Path,
    hostname: &str,
    username: &str,
) -> Result<bool> {
    let temp_config_str = temp_config.to_string_lossy();

    runner.out("Installing NixOS...").await;

    let config_dir = get_config_dir(username);
    let symlink_target = get_symlink_target(username);

    // Copy configuration to user home directory
    let config_parent = std::path::Path::new(&config_dir)
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid config directory path"))?;
    std::fs::create_dir_all(config_parent)?;
    copy_dir_recursive(&temp_config_str, &config_dir)?;

    // Remove .git from copied config
    let _ = std::fs::remove_dir_all(format!("{}/.git", config_dir));

    // Create symlink
    setup_config_symlink(&symlink_target)?;

    // Initialize git repo (optional, log failures)
    init_git_repo(runner, &config_dir).await;

    // Set ownership
    set_config_ownership(runner, config_parent, &config_dir).await;

    // Run nixos-install
    let success = runner
        .run(
            "nixos-install",
            &[
                "--flake",
                &format!("{}#{}", config_dir, hostname),
                "--no-root-passwd",
            ],
        )
        .await?;

    if !success {
        runner.step_failed("NixOS", "nixos-install failed", "NixOS installation").await?;
        runner.done(false).await?;
        return Ok(false);
    }

    runner.step_complete("NixOS").await?;
    Ok(true)
}

/// Step 7: Set user password
async fn step_set_user_password(
    runner: &CommandRunner<'_>,
    username: &str,
    password: &str,
) -> Result<bool> {
    runner.out("Setting up user account...").await;

    let escaped_password = password.replace('\'', "'\"'\"'");
    let chpasswd_script = format!(
        "echo '{}:{}' | nixos-enter --root /mnt -c 'chpasswd'",
        username, escaped_password
    );
    let success = run_command_sensitive(runner.tx(), "sh", &["-c", &chpasswd_script]).await?;

    if !success {
        runner.out("Warning: Failed to set user password. You can set it after first boot with 'passwd'.").await;
    }

    runner.step_complete("user").await?;
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
    runner.out(&format!("  4. Login as '{}' with your chosen password", username)).await;
    Ok(())
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Set up the /mnt/etc/nixos symlink
fn setup_config_symlink(symlink_target: &str) -> Result<()> {
    let symlink_parent = std::path::Path::new(INSTALL_SYMLINK_PATH)
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid symlink path: cannot determine parent of {}", INSTALL_SYMLINK_PATH))?;
    std::fs::create_dir_all(symlink_parent)
        .with_context(|| format!("Failed to create directory: {}", symlink_parent.display()))?;

    let symlink_path = std::path::Path::new(INSTALL_SYMLINK_PATH);
    if symlink_path.is_symlink() || symlink_path.is_file() {
        std::fs::remove_file(INSTALL_SYMLINK_PATH)
            .with_context(|| format!("Failed to remove existing symlink/file at {}", INSTALL_SYMLINK_PATH))?;
    } else if symlink_path.is_dir() {
        std::fs::remove_dir_all(INSTALL_SYMLINK_PATH)
            .with_context(|| format!("Failed to remove existing directory at {}", INSTALL_SYMLINK_PATH))?;
    }
    std::os::unix::fs::symlink(symlink_target, INSTALL_SYMLINK_PATH)
        .with_context(|| format!("Failed to create symlink {} -> {}", INSTALL_SYMLINK_PATH, symlink_target))?;

    Ok(())
}

/// Initialize git repository in the config directory
async fn init_git_repo(runner: &CommandRunner<'_>, config_dir: &str) {
    match runner
        .run(
            "nix-shell",
            &[
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
        Ok(false) => tracing::warn!("Git repository initialization returned non-zero exit - continuing"),
        Err(e) => tracing::warn!("Git repository initialization error: {} - continuing", e),
    }
}

/// Set ownership of config directory
async fn set_config_ownership(
    runner: &CommandRunner<'_>,
    config_parent: &std::path::Path,
    config_dir: &str,
) {
    let uid_gid = format!("{}:{}", PRIMARY_USER_UID, PRIMARY_USER_GID);
    let config_parent_str = config_parent.to_str().unwrap_or(".");

    match runner.run("chown", &[&uid_gid, config_parent_str]).await {
        Ok(true) => tracing::info!("Set ownership on config parent directory"),
        Ok(false) | Err(_) => tracing::warn!("Failed to set ownership on config parent directory"),
    }

    match runner.run("chown", &["-R", &uid_gid, config_dir]).await {
        Ok(true) => tracing::info!("Set ownership on config directory"),
        Ok(false) | Err(_) => tracing::warn!("Failed to set ownership on config directory"),
    }
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

    // Step 4: Configure disk
    if !step_configure_disk(&runner, &temp_config, hostname, disk, username).await? {
        return Ok(());
    }

    // Step 5: Run disko
    if !step_run_disko(&runner, &temp_config, hostname, password).await? {
        return Ok(());
    }

    // Step 6: Install NixOS
    if !step_install_nixos(&runner, &temp_config, hostname, username).await? {
        return Ok(());
    }

    // Step 7: Set user password
    step_set_user_password(&runner, username, password).await?;

    // Show completion message
    show_completion_message(&runner, username).await?;

    runner.done(true).await?;
    Ok(())
}

fn update_disk_device(content: &str, disk: &str) -> String {
    // Replace device = "/dev/..." with the new disk
    let replacement = format!("device = \"{}\"", disk);
    let result = DISK_DEVICE_RE.replace_all(content, replacement.as_str());

    // Log warning if no replacement occurred (pattern not found)
    if result == content && !content.contains(&format!("device = \"{}\"", disk)) {
        tracing::warn!(
            "Disk device replacement may have failed - pattern not found in disko config"
        );
    }

    result.to_string()
}

/// Inject passwordFile into disko LUKS configuration
/// Adds `passwordFile = "/tmp/luks-password";` after `name = "cryptroot";`
fn inject_luks_password_file(content: &str) -> String {
    let replacement = format!(
        r#"$1
              passwordFile = "{}";"#,
        LUKS_PASSWORD_FILE
    );
    LUKS_NAME_RE.replace_all(content, replacement.as_str()).to_string()
}

fn copy_dir_recursive(src: &str, dst: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dst_path = format!("{}/{}", dst, entry.file_name().to_string_lossy());

        // Skip symlinks to avoid loops and external references
        if path.is_symlink() {
            continue;
        }

        if path.is_dir() {
            // Skip paths with invalid UTF-8 (rare edge case)
            if let Some(path_str) = path.to_str() {
                copy_dir_recursive(path_str, &dst_path)?;
            }
        } else {
            std::fs::copy(&path, &dst_path)?;
        }
    }
    Ok(())
}

/// Update flake.nix to set username for a specific host configuration
/// Only modifies the file if username differs from the default
fn update_flake_username(content: &str, hostname: &str, username: &str) -> String {
    if username == DEFAULT_USERNAME {
        // No modification needed for default username
        return content.to_string();
    }

    // Check if this host entry already has a username line
    // Pattern: hostname = mkNixosSystem { ... username = "..."; ... }
    let username_check_pattern = format!(
        r#"(?s){} = mkNixosSystem \{{[^}}]*username\s*="#,
        regex::escape(hostname)
    );
    if let Ok(re) = regex::Regex::new(&username_check_pattern) {
        if re.is_match(content) {
            // Username already exists, replace it instead of adding
            let replace_pattern = format!(
                r#"({}\s*=\s*mkNixosSystem\s*\{{[^}}]*username\s*=\s*")[^"]*"#,
                regex::escape(hostname)
            );
            if let Ok(re_replace) = regex::Regex::new(&replace_pattern) {
                let replacement = format!("${{1}}{}", username);
                return re_replace.replace(content, replacement.as_str()).to_string();
            }
        }
    }

    // Look for the host entry pattern: hostname = mkNixosSystem {
    // and add username parameter after hostname line
    let host_pattern = format!(
        r#"(?m)^(\s*){} = mkNixosSystem \{{\s*\n(\s*)hostname = "{}";"#,
        regex::escape(hostname),
        regex::escape(hostname)
    );

    if let Ok(re) = regex::Regex::new(&host_pattern) {
        if re.is_match(content) {
            // Add username after hostname line
            let replacement = format!(
                "${{1}}{} = mkNixosSystem {{\n${{2}}hostname = \"{}\";\n${{2}}username = \"{}\";",
                hostname, hostname, username
            );
            return re.replace(content, replacement.as_str()).to_string();
        }
    }

    // If pattern not found, log warning and return unchanged
    tracing::warn!(
        "Could not update flake.nix with username for host '{}' - pattern not found",
        hostname
    );
    content.to_string()
}
