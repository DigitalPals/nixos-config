//! Command execution module

pub mod browser;
pub mod executor;
pub mod install;
pub mod update;

/// Standard step names for consistent messaging
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
}

/// Messages sent from command execution to UI
#[derive(Debug, Clone)]
pub enum CommandMessage {
    /// Standard output line
    Stdout(String),
    /// Standard error line
    Stderr(String),
    /// Step completed successfully
    StepComplete { step: String },
    /// Step failed with error
    StepFailed { step: String, error: String },
    /// Step was skipped
    StepSkipped { step: String },
    /// Command fully completed
    Done { success: bool },
}
