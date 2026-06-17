//! Command execution module

pub mod apps;
pub mod create_host;
pub mod errors;
pub mod executor;
pub mod health;
pub mod install;
pub mod keybindings;
pub mod keys;
pub mod retry;
pub mod runner;
pub mod update;

pub use errors::ParsedError;

/// Standard step names for consistent messaging
#[allow(dead_code)]
pub mod steps {
    // Install steps
    pub const NETWORK: &str = "network";
    pub const FLAKES: &str = "flakes";
    pub const REPOSITORY: &str = "repository";
    pub const DISK: &str = "disk";
    pub const DISKO: &str = "disko";
    pub const NIXOS: &str = "NixOS";
    pub const INSTALL: &str = "Install";

    // Update steps
    pub const FLAKE_UPDATE: &str = "flake";
    pub const REBUILD: &str = "Rebuild";
    pub const CLAUDE: &str = "Claude";
    pub const CODEX: &str = "Codex";
    pub const BROWSER: &str = "browser";

    // Browser steps
    pub const BACKUP: &str = "Backup";
    pub const RESTORE: &str = "Restore";
    pub const UPDATE: &str = "Update";

    // Create host steps
    pub const HOST_DIR: &str = "host";
    pub const HW_CONFIG: &str = "hardware";
    pub const HOST_CONFIG: &str = "configuration";
    pub const DISKO_CONFIG: &str = "disko";
    pub const FLAKE_NIX: &str = "flake";
}

use crate::app::{NixProgressEvent, UpdatePreflightReport, UpdateSummary};
use crate::commands::update::flake::FlakeInputChange;

/// Messages sent from command execution to UI
#[derive(Debug, Clone)]
pub enum CommandMessage {
    /// Standard output line
    Stdout(String),
    /// Standard error line
    Stderr(String),
    /// Step completed successfully
    StepComplete { step: String },
    /// Step failed with rich error information
    StepFailed { step: String, error: ParsedError },
    /// Step completed with a non-fatal warning
    StepWarning { step: String, detail: String },
    /// Step was skipped
    StepSkipped {
        step: String,
        reason: Option<String>,
    },
    /// Sub-step detail update (e.g. "Downloading nixpkgs (2/5)")
    StepDetail { step: String, detail: String },
    /// Structured Nix build/download progress for the modern update screen
    NixProgress(NixProgressEvent),
    /// Flake input changes discovered after mutation and before rebuild starts
    UpdateFlakePreview { changes: Vec<FlakeInputChange> },
    /// Command fully completed
    Done { success: bool },
    /// Operation was cancelled by user
    Cancelled,
    /// Updates available notification (sent after startup checks complete)
    UpdatesAvailable {
        nixos_config: bool,
        app_profiles: bool,
        /// Pending commits for nixos-config (hash, message)
        commits: Vec<(String, String)>,
    },
    /// Repository clone completed (for install host discovery)
    CloneComplete { success: bool },
    /// Update summary data (sent before Done for update command)
    UpdateSummaryData { summary: UpdateSummary },
    /// Update preflight checks finished and are ready for next UI step
    UpdatePreflightReady { report: UpdatePreflightReport },
    /// Update preflight checks failed unexpectedly
    UpdatePreflightFailed { error: String },
    /// Rollback available after rebuild failure
    RollbackAvailable { generation: u32 },
}
