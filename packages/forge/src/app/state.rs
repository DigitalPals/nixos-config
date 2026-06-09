//! Application state types and enums

use std::collections::VecDeque;
use std::time::Instant;

use crate::commands::update::flake::FlakeInputChange;
use crate::system::config::HostConfig;
use crate::system::disk::DiskInfo;
use crate::system::hardware::{CpuInfo, FormFactor, GpuInfo};

/// Main menu items
pub const MAIN_MENU_ITEMS: &[&str] = &[
    "Install NixOS (fresh installation)",
    "Update system",
    "App profiles",
    "Keybindings",
    "Exit",
];

/// App profile menu items (browsers, Portal, etc.)
pub const APP_MENU_ITEMS: &[&str] = &[
    "Backup & push to GitHub",
    "Pull & restore from GitHub",
    "Check for updates",
    "Setup local keys from 1Password",
    "Back to main menu",
];

/// Application mode/screen
#[derive(Debug, Clone)]
pub enum AppMode {
    MainMenu {
        selected: usize,
    },
    Install(InstallState),
    CreateHost(CreateHostState),
    Update(UpdateState),
    Apps(AppProfileState),
    Keys(KeysState),
    Keybindings(KeybindingsState),
    #[allow(dead_code)]
    Quit,
}

/// Which credential field is currently active
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CredentialField {
    #[default]
    Username,
    Password,
    ConfirmPassword,
}

/// Swap mode selection for installation
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SwapMode {
    /// Zram only - compressed RAM swap, no hibernate support
    #[default]
    ZramOnly,
    /// Hibernate support - disk swapfile sized at RAM + 2GB
    HibernateSupport,
}

/// User credentials and options collected during installation
#[derive(Debug, Clone)]
pub struct InstallCredentials {
    pub username: String,
    pub password: String,
    pub confirm_password: String,
    pub swap_mode: SwapMode,
    pub refresh_hardware_config: bool,
}

impl Default for InstallCredentials {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            confirm_password: String::new(),
            swap_mode: SwapMode::default(),
            refresh_hardware_config: false,
        }
    }
}

/// Installation state machine
#[derive(Debug, Clone)]
pub enum InstallState {
    /// Cloning repository from GitHub (live ISO only)
    CloneRepository {
        output: VecDeque<String>,
    },
    SelectHost {
        selected: usize,
    },
    SelectDisk {
        host: String,
        disks: Vec<DiskInfo>,
        selected: usize,
    },
    EnterCredentials {
        host: String,
        disk: DiskInfo,
        credentials: InstallCredentials,
        active_field: CredentialField,
        error: Option<String>,
    },
    SelectSwapMode {
        host: String,
        disk: DiskInfo,
        credentials: InstallCredentials,
        selected: usize,
        /// Total RAM in GB (for display)
        ram_gb: u64,
    },
    SelectHardwareProfile {
        host: String,
        disk: DiskInfo,
        credentials: InstallCredentials,
        selected: usize,
    },
    Overview {
        host: String,
        disk: DiskInfo,
        credentials: InstallCredentials,
        hardware_config: Option<NewHostConfig>,
        input: String,
    },
    Running {
        host: String,
        disk: DiskInfo,
        step: usize,
        steps: Vec<StepStatus>,
        output: VecDeque<String>,
    },
    Complete {
        success: bool,
        output: VecDeque<String>,
        /// None = auto-scroll, Some(n) = manual scroll at position n
        scroll_offset: Option<usize>,
    },
}

impl InstallState {
    pub fn new(
        hostname: Option<String>,
        disk: Option<String>,
        refresh_hardware_config: bool,
    ) -> Self {
        match (hostname, disk) {
            (Some(host), Some(disk_path)) => {
                // Direct install with provided args - go to credentials
                let disk = DiskInfo {
                    path: disk_path,
                    size: "Unknown".to_string(),
                    size_bytes: 0,
                    model: None,
                    partitions: vec![],
                };
                let credentials = InstallCredentials {
                    refresh_hardware_config,
                    ..InstallCredentials::default()
                };
                InstallState::EnterCredentials {
                    host,
                    disk,
                    credentials,
                    active_field: CredentialField::Username,
                    error: None,
                }
            }
            (Some(host), None) => {
                // Host provided, need disk selection
                InstallState::SelectDisk {
                    host,
                    disks: Vec::new(),
                    selected: 0,
                }
            }
            _ => InstallState::SelectHost { selected: 0 },
        }
    }
}

/// Validate a username for NixOS user creation
pub fn validate_username(username: &str) -> Option<String> {
    if username.is_empty() {
        return Some("Username cannot be empty".to_string());
    }
    if username.len() > 32 {
        return Some("Username too long (max 32 characters)".to_string());
    }
    // Safe: we already checked that username is not empty above
    if !username
        .chars()
        .next()
        .expect("username is not empty")
        .is_ascii_lowercase()
    {
        return Some("Username must start with a lowercase letter".to_string());
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        return Some(
            "Username can only contain lowercase letters, numbers, underscore, and hyphen"
                .to_string(),
        );
    }
    // Reserved usernames
    let reserved = [
        "root",
        "nobody",
        "daemon",
        "bin",
        "sys",
        "sync",
        "games",
        "man",
        "lp",
        "mail",
        "news",
        "uucp",
        "proxy",
        "www-data",
        "backup",
        "list",
        "irc",
        "gnats",
        "systemd-network",
        "systemd-resolve",
    ];
    if reserved.contains(&username) {
        return Some(format!("'{}' is a reserved username", username));
    }
    None
}

/// Validate password requirements
pub fn validate_password(password: &str, confirm: &str) -> Option<String> {
    if password.is_empty() {
        return Some("Password cannot be empty".to_string());
    }
    if password.len() < 8 {
        return Some("Password must be at least 8 characters".to_string());
    }
    if password != confirm {
        return Some("Passwords do not match".to_string());
    }
    None
}

/// Configuration being built during host creation wizard
#[derive(Debug, Clone)]
pub struct NewHostConfig {
    pub hostname: String,
    pub cpu: CpuInfo,
    pub gpu: GpuInfo,
    pub form_factor: FormFactor,
    pub disk: DiskInfo,
}

/// Create host wizard state machine
/// Flow: DetectingHardware → ConfirmCpu → ConfirmGpu → ConfirmFormFactor → SelectDisk → EnterHostname → Review → Generating → Complete
#[derive(Debug, Clone)]
pub enum CreateHostState {
    DetectingHardware,
    ConfirmCpu {
        cpu: CpuInfo,
        detected_gpu: GpuInfo,
        detected_form_factor: FormFactor,
        override_menu: bool,
        selected: usize,
    },
    ConfirmGpu {
        cpu: CpuInfo,
        gpu: GpuInfo,
        detected_form_factor: FormFactor,
        override_menu: bool,
        selected: usize,
    },
    ConfirmFormFactor {
        cpu: CpuInfo,
        gpu: GpuInfo,
        form_factor: FormFactor,
        override_menu: bool,
        selected: usize,
    },
    SelectDisk {
        cpu: CpuInfo,
        gpu: GpuInfo,
        form_factor: FormFactor,
        disks: Vec<DiskInfo>,
        selected: usize,
    },
    EnterHostname {
        cpu: CpuInfo,
        gpu: GpuInfo,
        form_factor: FormFactor,
        disk: DiskInfo,
        input: String,
        error: Option<String>,
    },
    Review {
        config: NewHostConfig,
    },
    Generating {
        config: NewHostConfig,
        step: usize,
        steps: Vec<StepStatus>,
        output: VecDeque<String>,
    },
    Complete {
        success: bool,
        config: NewHostConfig,
    },
}

impl CreateHostState {
    pub fn new() -> Self {
        CreateHostState::DetectingHardware
    }
}

/// Resolution options for local git changes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LocalChangesResolution {
    /// Discard all changes and untracked files (git reset --hard + git clean -fd)
    Overwrite,
    /// Stash changes, update, then restore (git stash push + pop)
    Stash,
    /// Cancel the update
    Cancel,
}

/// A local git change discovered before running update
#[derive(Debug, Clone)]
pub struct LocalChange {
    pub path: String,
    pub tracked: bool,
}

/// Metadata for an autostash created by forge update
#[derive(Debug, Clone)]
pub struct StashInfo {
    pub reference: String,
    pub message: String,
}

/// Status of the local upstream branch before update begins
#[derive(Debug, Clone, Default)]
pub struct UpdateRemoteStatus {
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub checked: bool,
    pub error: Option<String>,
}

/// Result of a non-mutating preflight dry run
#[derive(Debug, Clone)]
pub enum UpdateDryRunStatus {
    Passed,
    Failed(String),
    Skipped(String),
}

/// Review data collected before the update starts mutating state
#[derive(Debug, Clone)]
pub struct UpdatePreflightReport {
    pub missing_required_tools: Vec<String>,
    pub missing_optional_tools: Vec<String>,
    pub tracked_count: usize,
    pub untracked_count: usize,
    pub pending_resolution: Option<LocalChangesResolution>,
    pub remote: UpdateRemoteStatus,
    pub dry_run: UpdateDryRunStatus,
}

impl UpdatePreflightReport {
    pub fn can_continue(&self) -> bool {
        self.missing_required_tools.is_empty()
            && matches!(
                self.dry_run,
                UpdateDryRunStatus::Passed | UpdateDryRunStatus::Skipped(_)
            )
    }

    pub fn should_auto_continue(&self) -> bool {
        self.missing_required_tools.is_empty()
            && self.missing_optional_tools.is_empty()
            && self.tracked_count == 0
            && self.untracked_count == 0
            && self.pending_resolution.is_none()
            && self.remote.checked
            && self.remote.error.is_none()
            && self.remote.ahead == 0
            && self.remote.behind == 0
            && matches!(self.dry_run, UpdateDryRunStatus::Passed)
    }
}

/// Which row is selected in the update preflight screen
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpdatePreflightField {
    Start,
    Mode,
    Inputs,
    Back,
}

/// Update state machine
#[derive(Debug, Clone)]
pub enum UpdateState {
    /// Configure update options before starting
    Preflight {
        options: UpdateOptions,
        selected: UpdatePreflightField,
        input_buffer: String,
        editing_inputs: bool,
        auto_start: bool,
    },
    /// Running non-mutating checks before update starts
    Preparing {
        options: UpdateOptions,
        steps: Vec<StepStatus>,
        output: VecDeque<String>,
        scroll_offset: Option<usize>,
    },
    /// Prompt user about local changes before updating
    LocalChangesPrompt {
        changes: Vec<LocalChange>,
        tracked_count: usize,
        untracked_count: usize,
        selected: usize, // 0=Overwrite, 1=Stash, 2=Cancel
        options: UpdateOptions,
    },
    /// Final safety check before anything mutates
    ReviewPreflight {
        options: UpdateOptions,
        report: UpdatePreflightReport,
        selected: usize, // 0=Continue, 1=Back
    },
    /// Extra confirmation before discarding local changes
    OverwriteConfirm {
        changes: Vec<LocalChange>,
        tracked_count: usize,
        untracked_count: usize,
        options: UpdateOptions,
        selected: usize, // 0=Discard, 1=Back
    },
    Running {
        step: usize,
        steps: Vec<StepStatus>,
        output: VecDeque<String>,
        /// Structured Nix build/download progress for the modern direct update view
        nix_progress: NixProgressState,
        /// Manual scroll position for the live log
        scroll_offset: Option<usize>,
        /// Autostash metadata that should be restored at the end of a successful update
        stash: Option<StashInfo>,
        /// Selective update options
        options: UpdateOptions,
    },
    /// Show summary modal after completion
    ShowingSummary {
        success: bool,
        steps: Vec<StepStatus>,
        output: VecDeque<String>,
        summary: UpdateSummary,
        /// None = auto-scroll, Some(n) = manual scroll at position n
        scroll_offset: Option<usize>,
        /// Scroll position within the summary modal content
        summary_scroll: usize,
    },
    Complete {
        #[allow(dead_code)]
        success: bool,
        steps: Vec<StepStatus>,
        output: VecDeque<String>,
        /// None = auto-scroll, Some(n) = manual scroll at position n
        scroll_offset: Option<usize>,
    },
}

/// Update presentation selected by the entrypoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpdatePresentation {
    #[default]
    Classic,
    Modern,
}

/// Options for selective updates
#[derive(Debug, Clone)]
pub struct UpdateOptions {
    pub rebuild_only: bool,
    pub flake_only: bool,
    pub inputs: Vec<String>,
    pub presentation: UpdatePresentation,
    /// Skip the NVIDIA driver compatibility pre-flight build on kernel bumps.
    pub skip_nvidia_check: bool,
}

impl Default for UpdateOptions {
    fn default() -> Self {
        Self {
            rebuild_only: false,
            flake_only: false,
            inputs: Vec::new(),
            presentation: UpdatePresentation::Classic,
            skip_nvidia_check: false,
        }
    }
}

/// Live rebuild progress shown by the modern direct update screen.
#[derive(Debug, Clone)]
pub struct NixProgressState {
    pub section: String,
    pub rows: Vec<NixProgressRow>,
}

impl Default for NixProgressState {
    fn default() -> Self {
        Self {
            section: "Waiting for rebuild".to_string(),
            rows: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NixProgressRow {
    pub name: String,
    pub status: NixProgressStatus,
    pub transferred: Option<u64>,
    pub total: Option<u64>,
    pub speed_bps: Option<f64>,
    pub eta_secs: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NixProgressStatus {
    Downloading,
    Building,
    Activating,
    Complete,
    Failed,
}

#[derive(Debug, Clone)]
pub enum NixProgressEvent {
    Download {
        name: String,
        transferred: u64,
        total: Option<u64>,
        speed_bps: Option<f64>,
        eta_secs: Option<u64>,
    },
    Building {
        name: String,
    },
    Activating,
    Complete {
        name: String,
    },
    Failed {
        name: String,
    },
    Section {
        title: String,
    },
}

impl UpdateOptions {
    pub fn validate(&self) -> Option<String> {
        if self.rebuild_only && self.flake_only {
            Some("Update mode cannot be both 'rebuild only' and 'flake only'.".to_string())
        } else {
            None
        }
    }

    pub fn mode_label(&self) -> &'static str {
        if self.rebuild_only {
            "Rebuild only"
        } else if self.flake_only {
            "Flake inputs only"
        } else {
            "Full update"
        }
    }

    pub fn cycle_mode(&mut self) {
        match (self.rebuild_only, self.flake_only) {
            (false, false) => {
                self.rebuild_only = true;
                self.flake_only = false;
            }
            (true, false) => {
                self.rebuild_only = false;
                self.flake_only = true;
            }
            _ => {
                self.rebuild_only = false;
                self.flake_only = false;
            }
        }
    }

    pub fn is_modern(&self) -> bool {
        self.presentation == UpdatePresentation::Modern
    }
}

impl UpdateState {
    pub fn preflight(options: UpdateOptions, auto_start: bool) -> Self {
        let input_buffer = if options.inputs.is_empty() {
            String::new()
        } else {
            options.inputs.join(", ")
        };
        UpdateState::Preflight {
            options,
            selected: UpdatePreflightField::Start,
            input_buffer,
            editing_inputs: false,
            auto_start,
        }
    }

    pub fn new_with_options(options: UpdateOptions, stash: Option<StashInfo>) -> Self {
        let mut steps = Vec::new();

        if !options.rebuild_only {
            steps.push(StepStatus::new_with_id(
                "update.pull",
                "Pulling configuration updates",
            ));
            steps.push(StepStatus::new_with_id(
                "update.flake",
                "Updating flake inputs",
            ));
        }

        if !options.flake_only {
            steps.push(StepStatus::new_with_id(
                "update.rebuild",
                "Rebuilding system",
            ));
            steps.push(StepStatus::new_with_id(
                "update.packages",
                "Comparing packages",
            ));
        }

        if !options.rebuild_only && !options.flake_only {
            steps.push(StepStatus::new_with_id(
                "update.claude",
                "Claude Code follow-up",
            ));
            steps.push(StepStatus::new_with_id(
                "update.codex",
                "Codex CLI follow-up",
            ));
            steps.push(StepStatus::new_with_id(
                "update.browser",
                "Browser profile follow-up",
            ));
            steps.push(StepStatus::new_with_id(
                "update.firmware",
                "Firmware follow-up",
            ));
        }

        UpdateState::Running {
            step: 0,
            steps,
            output: VecDeque::new(),
            nix_progress: NixProgressState::default(),
            scroll_offset: None,
            stash,
            options,
        }
    }

    pub fn preparing(
        options: UpdateOptions,
        _pending_resolution: Option<LocalChangesResolution>,
    ) -> Self {
        let mut steps = vec![
            StepStatus::new_with_id("update.preflight.health", "Checking required tools"),
            StepStatus::new_with_id("update.preflight.remote", "Checking repository status"),
            StepStatus::new_with_id("update.preflight.dryrun", "Running dry-run build"),
        ];
        if let Some(first) = steps.first_mut() {
            first.status = StepState::Running;
            first.started_at = Some(Instant::now());
        }

        UpdateState::Preparing {
            options,
            steps,
            output: VecDeque::new(),
            scroll_offset: None,
        }
    }
}

/// App profile management state (browsers, Portal, etc.)
#[derive(Debug, Clone)]
pub enum AppProfileState {
    Menu {
        selected: usize,
    },
    Running {
        operation: AppOp,
        output: VecDeque<String>,
        force: bool,
    },
    Status {
        output: VecDeque<String>,
    },
    Complete {
        success: bool,
        output: VecDeque<String>,
        /// None = auto-scroll, Some(n) = manual scroll at position n
        scroll_offset: Option<usize>,
    },
}

impl AppProfileState {
    pub fn new_menu() -> Self {
        AppProfileState::Menu { selected: 0 }
    }

    pub fn new_backup(force: bool) -> Self {
        AppProfileState::Running {
            operation: AppOp::Backup,
            output: VecDeque::new(),
            force,
        }
    }

    pub fn new_restore(force: bool) -> Self {
        AppProfileState::Running {
            operation: AppOp::Restore,
            output: VecDeque::new(),
            force,
        }
    }

    pub fn new_status() -> Self {
        AppProfileState::Status {
            output: VecDeque::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppOp {
    Backup,
    Restore,
}

/// Key management state
#[derive(Debug, Clone)]
pub enum KeysState {
    Running {
        operation: KeysOp,
        output: VecDeque<String>,
        force: bool,
    },
    Complete {
        success: bool,
        output: VecDeque<String>,
        /// None = auto-scroll, Some(n) = manual scroll at position n
        scroll_offset: Option<usize>,
    },
}

impl KeysState {
    pub fn new_setup(force: bool) -> Self {
        KeysState::Running {
            operation: KeysOp::Setup,
            output: VecDeque::new(),
            force,
        }
    }

    pub fn new_backup() -> Self {
        KeysState::Running {
            operation: KeysOp::Backup,
            output: VecDeque::new(),
            force: false,
        }
    }

    pub fn new_restore(force: bool) -> Self {
        KeysState::Running {
            operation: KeysOp::Restore,
            output: VecDeque::new(),
            force,
        }
    }

    pub fn new_status() -> Self {
        KeysState::Running {
            operation: KeysOp::Status,
            output: VecDeque::new(),
            force: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum KeysOp {
    Setup,
    Backup,
    Restore,
    Status,
}

/// A parsed keybinding entry
#[derive(Debug, Clone)]
pub struct Keybinding {
    pub modifiers: String,
    pub key: String,
    pub category: String,
    pub description: String,
}

/// Which panel is focused in the keybindings viewer
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum KeybindingsPanel {
    #[default]
    Categories,
    Bindings,
}

/// Keybindings viewer state
#[derive(Debug, Clone)]
pub enum KeybindingsState {
    Loading,
    Viewing {
        bindings: Vec<Keybinding>,
        categories: Vec<String>,
        selected_category: usize,
        selected_binding: usize,
        scroll_offset: usize,
        shell: String,
        focus: KeybindingsPanel,
    },
}

/// Modal dialog types that can be stacked
#[derive(Debug, Clone)]
pub enum ModalDialog {
    /// Confirm exit
    ExitConfirm,
    /// Confirm reboot with reasons
    RebootConfirm { reasons: Vec<String> },
    /// Help overlay
    Help,
    /// Rollback prompt after failed rebuild
    RollbackPrompt { generation: u32, selected: usize },
    /// Resume incomplete operation prompt
    ResumePrompt,
}

/// Step progress status
#[derive(Debug, Clone)]
pub struct StepStatus {
    pub id: String,
    pub name: String,
    pub status: StepState,
    /// Optional sub-step detail shown below the step name (e.g. "Downloading nixpkgs (2/5)")
    pub detail: Option<String>,
    /// When this step started running
    pub started_at: Option<Instant>,
}

impl StepStatus {
    pub fn new_with_id(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            status: StepState::Pending,
            detail: None,
            started_at: None,
        }
    }

    /// Get elapsed time since step started (if running)
    pub fn elapsed_secs(&self) -> Option<u64> {
        self.started_at.map(|t| t.elapsed().as_secs())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepState {
    Pending,
    Running,
    Complete,
    Failed,
    Warning,
    Skipped,
}

/// Update summary data
#[derive(Debug, Clone, Default)]
pub struct UpdateSummary {
    pub config_commits: Vec<CommitInfo>, // nixos-config commits pulled from upstream
    pub flake_changes: Vec<FlakeInputChange>, // Flake input changes with commits
    pub package_changes: Vec<(String, String, String)>, // (pkg, old_ver, new_ver)
    pub packages_added: Vec<(String, String)>, // (pkg, version)
    pub packages_removed: Vec<(String, String)>, // (pkg, version)
    pub closure_summary: Option<String>,      // nvd closure size summary
    pub claude_old: Option<String>,
    pub claude_new: Option<String>,
    pub codex_old: Option<String>,
    pub codex_new: Option<String>,
    pub browser_status: String,
    pub firmware_status: String,
    pub rebuild_skipped: bool,
    pub rebuild_failed: bool,
    pub reboot_reasons: Vec<String>,
    pub core_status: UpdateCoreStatus,
    pub partial_state: Option<String>,
    pub follow_up_warnings: Vec<String>,
    pub system_before: Option<String>,
    pub system_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum UpdateCoreStatus {
    #[default]
    Pending,
    Success,
    UpToDate,
    Partial,
    Cancelled,
}

/// Information about a pending commit
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
}

/// Tracks which updates are available for the combined dialog
#[derive(Debug, Clone, Default)]
pub struct PendingUpdates {
    pub nixos_config: bool,
    pub app_profiles: bool,
    /// Pending commits for nixos-config
    pub commits: Vec<CommitInfo>,
    /// Selected option in the dialog (0 = first option)
    pub selected: usize,
    /// True when viewing the commit list
    pub viewing_commits: bool,
    /// Scroll position in commit list
    pub commit_scroll: usize,
    /// Selected commit index
    pub selected_commit: usize,
}

impl PendingUpdates {
    pub fn has_updates(&self) -> bool {
        self.nixos_config || self.app_profiles
    }

    pub fn clear(&mut self) {
        self.nixos_config = false;
        self.app_profiles = false;
        self.commits.clear();
        self.selected = 0;
        self.viewing_commits = false;
        self.commit_scroll = 0;
        self.selected_commit = 0;
    }
}

/// Check if a host directory already exists on the filesystem
pub fn host_dir_exists(hostname: &str) -> bool {
    crate::constants::host_dir_paths(hostname)
        .iter()
        .any(|p| p.exists())
}

/// Get the number of options in the update dialog based on available updates
pub fn get_update_dialog_option_count(pending: &PendingUpdates) -> usize {
    let mut count = 0;
    if pending.nixos_config {
        count += 1; // "View NixOS updates"
    }
    if pending.app_profiles {
        count += 1; // "Update app profiles"
    }
    if pending.nixos_config && pending.app_profiles {
        count += 1; // "Update all"
    }
    count += 1; // "Dismiss"
    count
}

/// Validate a hostname for NixOS configuration
pub fn validate_hostname(hostname: &str, hosts: &[HostConfig]) -> Option<String> {
    if hostname.is_empty() {
        return Some("Hostname cannot be empty".to_string());
    }
    if hostname.len() > 63 {
        return Some("Hostname too long (max 63 characters)".to_string());
    }
    // Safe: we already checked that hostname is not empty above
    if !hostname
        .chars()
        .next()
        .expect("hostname is not empty")
        .is_alphanumeric()
    {
        return Some("Hostname must start with a letter or number".to_string());
    }
    if !hostname.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Some("Hostname can only contain letters, numbers, and hyphens".to_string());
    }
    // Check if host already exists
    let host_exists = hosts.iter().any(|h| h.name == hostname) || host_dir_exists(hostname);
    if host_exists {
        return Some(format!("Host '{}' already exists", hostname));
    }
    None
}
