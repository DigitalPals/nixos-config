//! Fresh NixOS installation command

use anyhow::{Context, Result};
use std::sync::LazyLock;
use tokio::sync::mpsc;

use super::executor::{run_capture, run_command, run_command_sensitive};
use super::CommandMessage;
use crate::constants::{
    self, INSTALL_MOUNT_POINT, INSTALL_SYMLINK_PATH, NIXOS_CONFIG_HOME_DIR,
    PRIMARY_USER_GID, PRIMARY_USER_UID,
};

/// Path to the temporary LUKS password file (used by disko)
const LUKS_PASSWORD_FILE: &str = "/tmp/luks-password";

const REPO_URL: &str = "https://github.com/DigitalPals/nixos-config.git";

/// Regex to match disko device declarations.
/// This pattern is a compile-time constant and cannot fail to compile.
static DISK_DEVICE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"device = "/dev/[^"]*""#)
        .expect("Disk device regex pattern is statically validated")
});

/// Regex to match the LUKS content section where we need to inject passwordFile.
/// Matches: type = "luks"; name = "cryptroot";
static LUKS_NAME_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r#"(name = "cryptroot";)"#)
        .expect("LUKS name regex pattern is statically validated")
});

/// Default username used in the configuration
const DEFAULT_USERNAME: &str = "john";

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
                    error: e.to_string(),
                })
                .await;
            let _ = tx.send(CommandMessage::Done { success: false }).await;
        }
    });
    Ok(())
}

async fn run_install(
    tx: &mpsc::Sender<CommandMessage>,
    hostname: &str,
    disk: &str,
    username: &str,
    password: &str,
) -> Result<()> {
    // Step 1: Check network connectivity
    tx.send(CommandMessage::Stdout("Checking network connectivity...".to_string()))
        .await?;

    let (success, _, _) = run_capture("ping", &["-c", "1", "-W", "5", "github.com"]).await?;
    if !success {
        tx.send(CommandMessage::StepFailed {
            step: "network".to_string(),
            error: "No network connection. Please connect to WiFi using nmtui.".to_string(),
        })
        .await?;
        tx.send(CommandMessage::Done { success: false }).await?;
        return Ok(());
    }

    tx.send(CommandMessage::StepComplete {
        step: "network".to_string(),
    })
    .await?;

    // Step 2: Enable flakes
    tx.send(CommandMessage::Stdout("Enabling Nix flakes...".to_string()))
        .await?;

    std::env::set_var("NIX_CONFIG", "experimental-features = nix-command flakes");

    tx.send(CommandMessage::StepComplete {
        step: "flakes".to_string(),
    })
    .await?;

    // Step 3: Clone configuration repository (skip if already cloned with new host)
    let temp_config = constants::temp_config_dir();
    let temp_config_str = temp_config.to_string_lossy();
    let host_exists_in_temp = temp_config
        .join(constants::HOSTS_SUBDIR)
        .join(hostname)
        .join("default.nix")
        .exists();

    if host_exists_in_temp {
        // Host was just created by create_host flow, reuse the existing clone
        tx.send(CommandMessage::Stdout(
            "Using existing configuration (host already created)...".to_string(),
        ))
        .await?;
    } else {
        // Fresh install of existing host, clone from GitHub
        tx.send(CommandMessage::Stdout("Cloning configuration repository...".to_string()))
            .await?;

        let _ = std::fs::remove_dir_all(&temp_config);

        let success = run_command(
            tx,
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
            tx.send(CommandMessage::StepFailed {
                step: "repository".to_string(),
                error: "Failed to clone repository".to_string(),
            })
            .await?;
            tx.send(CommandMessage::Done { success: false }).await?;
            return Ok(());
        }
    }

    tx.send(CommandMessage::StepComplete {
        step: "repository".to_string(),
    })
    .await?;

    // Step 4: Configure disk device
    tx.send(CommandMessage::Stdout(format!(
        "Configuring disk device {}...",
        disk
    )))
    .await?;

    // Validate disk path format
    if !disk.starts_with("/dev/") {
        tx.send(CommandMessage::StepFailed {
            step: "disk".to_string(),
            error: format!("Invalid disk path: {}. Must start with /dev/", disk),
        })
        .await?;
        tx.send(CommandMessage::Done { success: false }).await?;
        return Ok(());
    }

    // Check that disk device actually exists
    if !std::path::Path::new(disk).exists() {
        tx.send(CommandMessage::StepFailed {
            step: "disk".to_string(),
            error: format!("Disk device does not exist: {}", disk),
        })
        .await?;
        tx.send(CommandMessage::Done { success: false }).await?;
        return Ok(());
    }

    // Check disko config file exists
    let disko_file = format!("{}/modules/disko/{}.nix", temp_config_str, hostname);
    if !std::path::Path::new(&disko_file).exists() {
        tx.send(CommandMessage::StepFailed {
            step: "disk".to_string(),
            error: format!(
                "No disko configuration found for host '{}'. Expected: modules/disko/{}.nix",
                hostname, hostname
            ),
        })
        .await?;
        tx.send(CommandMessage::Done { success: false }).await?;
        return Ok(());
    }

    let disko_content = std::fs::read_to_string(&disko_file)
        .with_context(|| format!("Failed to read disko config: {}", disko_file))?;
    let updated_content = update_disk_device(&disko_content, disk);
    std::fs::write(&disko_file, &updated_content)
        .with_context(|| format!("Failed to write disko config: {}", disko_file))?;

    // Update flake.nix with username if it differs from default
    if username != DEFAULT_USERNAME {
        tx.send(CommandMessage::Stdout(format!(
            "Configuring username '{}'...",
            username
        )))
        .await?;

        let flake_file = format!("{}/flake.nix", temp_config_str);
        let flake_content = std::fs::read_to_string(&flake_file)
            .with_context(|| format!("Failed to read flake.nix: {}", flake_file))?;
        let updated_flake = update_flake_username(&flake_content, hostname, username);
        std::fs::write(&flake_file, &updated_flake)
            .with_context(|| format!("Failed to write flake.nix: {}", flake_file))?;
    }

    tx.send(CommandMessage::StepComplete {
        step: "disk".to_string(),
    })
    .await?;

    // Step 5: Run disko
    tx.send(CommandMessage::Stdout("Running disko to partition and format...".to_string()))
        .await?;
    tx.send(CommandMessage::Stdout(
        "Using provided passphrase for LUKS encryption...".to_string(),
    ))
    .await?;

    // Write password to temp file for disko (disko reads from passwordFile, not stdin)
    std::fs::write(LUKS_PASSWORD_FILE, password.as_bytes())
        .with_context(|| format!("Failed to write LUKS password file: {}", LUKS_PASSWORD_FILE))?;
    // Restrict permissions to root only
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

    // Verify passwordFile injection succeeded
    if updated_disko.contains("passwordFile") {
        tx.send(CommandMessage::Stdout(
            "LUKS passwordFile configured successfully".to_string(),
        ))
        .await?;
        tracing::info!("passwordFile injection confirmed in disko config");
    } else {
        tx.send(CommandMessage::Stderr(
            "WARNING: passwordFile injection may have failed!".to_string(),
        ))
        .await?;
        tracing::error!("passwordFile NOT found in modified disko config");
    }

    // Pre-fetch disko (optional optimization, log if it fails)
    match run_command(tx, "nix", &["build", &format!("{}#disko", temp_config_str), "--no-link"]).await {
        Ok(true) => tracing::info!("Disko pre-fetch succeeded"),
        Ok(false) => tracing::warn!("Disko pre-fetch failed - continuing anyway"),
        Err(e) => tracing::warn!("Disko pre-fetch error: {} - continuing anyway", e),
    }

    // Run disko with passwordFile (reads password from file, no stdin needed)
    // --yes-wipe-all-disks skips the "are you sure?" confirmation (already confirmed in wizard)
    let success = run_command(
        tx,
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

    // Clean up password file immediately after disko (security)
    if let Err(e) = std::fs::remove_file(LUKS_PASSWORD_FILE) {
        tracing::warn!("Failed to remove LUKS password file: {}", e);
    }

    if !success {
        tx.send(CommandMessage::StepFailed {
            step: "disko".to_string(),
            error: "Disk partitioning failed".to_string(),
        })
        .await?;
        tx.send(CommandMessage::Done { success: false }).await?;
        return Ok(());
    }

    tx.send(CommandMessage::StepComplete {
        step: "disko".to_string(),
    })
    .await?;

    // Step 6: Install NixOS
    tx.send(CommandMessage::Stdout("Installing NixOS...".to_string()))
        .await?;

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
    let symlink_parent = std::path::Path::new(INSTALL_SYMLINK_PATH)
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid symlink path: cannot determine parent of {}", INSTALL_SYMLINK_PATH))?;
    std::fs::create_dir_all(symlink_parent)
        .with_context(|| format!("Failed to create directory: {}", symlink_parent.display()))?;
    // Remove existing path (could be file, symlink, or directory)
    // Check is_symlink() FIRST since symlinks to dirs return true for is_dir()
    let symlink_path = std::path::Path::new(INSTALL_SYMLINK_PATH);
    if symlink_path.is_symlink() || symlink_path.is_file() {
        std::fs::remove_file(INSTALL_SYMLINK_PATH)
            .with_context(|| format!("Failed to remove existing symlink/file at {}", INSTALL_SYMLINK_PATH))?;
    } else if symlink_path.is_dir() {
        std::fs::remove_dir_all(INSTALL_SYMLINK_PATH)
            .with_context(|| format!("Failed to remove existing directory at {}", INSTALL_SYMLINK_PATH))?;
    }
    std::os::unix::fs::symlink(&symlink_target, INSTALL_SYMLINK_PATH)
        .with_context(|| format!("Failed to create symlink {} -> {}", INSTALL_SYMLINK_PATH, symlink_target))?;

    // Initialize git repo (optional, log failures)
    match run_command(
        tx,
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

    // Set ownership using UID:GID since user doesn't exist yet on live ISO
    let uid_gid = format!("{}:{}", PRIMARY_USER_UID, PRIMARY_USER_GID);
    let config_parent_str = config_parent.to_str().unwrap_or(".");

    match run_command(tx, "chown", &[&uid_gid, config_parent_str]).await {
        Ok(true) => tracing::info!("Set ownership on config parent directory"),
        Ok(false) | Err(_) => tracing::warn!("Failed to set ownership on config parent directory"),
    }

    match run_command(tx, "chown", &["-R", &uid_gid, &config_dir]).await {
        Ok(true) => tracing::info!("Set ownership on config directory"),
        Ok(false) | Err(_) => tracing::warn!("Failed to set ownership on config directory"),
    }

    // Run nixos-install
    let success = run_command(
        tx,
        "nixos-install",
        &[
            "--flake",
            &format!("{}#{}", config_dir, hostname),
            "--no-root-passwd",
        ],
    )
    .await?;

    if !success {
        tx.send(CommandMessage::StepFailed {
            step: "NixOS".to_string(),
            error: "nixos-install failed".to_string(),
        })
        .await?;
        tx.send(CommandMessage::Done { success: false }).await?;
        return Ok(());
    }

    tx.send(CommandMessage::StepComplete {
        step: "NixOS".to_string(),
    })
    .await?;

    // Step 7: Set user password
    tx.send(CommandMessage::Stdout("Setting up user account...".to_string()))
        .await?;

    // Use chpasswd to set the user password
    // Use run_command_sensitive to avoid logging the password
    let escaped_password = password.replace('\'', "'\"'\"'");
    let chpasswd_script = format!(
        "echo '{}:{}' | nixos-enter --root /mnt -c 'chpasswd'",
        username, escaped_password
    );
    let success = run_command_sensitive(tx, "sh", &["-c", &chpasswd_script]).await?;

    if !success {
        tx.send(CommandMessage::Stdout(
            "Warning: Failed to set user password. You can set it after first boot with 'passwd'.".to_string(),
        ))
        .await?;
    }

    tx.send(CommandMessage::StepComplete {
        step: "user".to_string(),
    })
    .await?;

    tx.send(CommandMessage::Stdout("\n".to_string())).await?;
    tx.send(CommandMessage::Stdout("Installation complete!".to_string()))
        .await?;
    tx.send(CommandMessage::Stdout("".to_string())).await?;
    tx.send(CommandMessage::Stdout("Next steps:".to_string()))
        .await?;
    tx.send(CommandMessage::Stdout("  1. Reboot: reboot".to_string()))
        .await?;
    tx.send(CommandMessage::Stdout("  2. Enter your LUKS passphrase at boot".to_string()))
        .await?;
    tx.send(CommandMessage::Stdout("  3. Select a shell from the boot menu".to_string()))
        .await?;
    tx.send(CommandMessage::Stdout(format!("  4. Login as '{}' with your chosen password", username)))
        .await?;

    tx.send(CommandMessage::Done { success: true }).await?;
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
