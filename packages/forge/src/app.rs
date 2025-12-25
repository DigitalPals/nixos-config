//! Application state management

use anyhow::Result;
use crossterm::event::KeyCode;
use regex::Regex;
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::commands::{self, CommandMessage};
use crate::constants::{MAX_INPUT_LENGTH, OUTPUT_BUFFER_SIZE, SPINNER_TICK_MS};
use crate::system::config::{discover_hosts, HostConfig};
use crate::system::disk::DiskInfo;
use crate::system::hardware::{CpuInfo, CpuVendor, FormFactor, GpuInfo, GpuVendor};

/// Regex to match ANSI escape codes.
/// This pattern is a compile-time constant and cannot fail to compile.
/// The unwrap is safe because the pattern is statically validated.
static ANSI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap());

/// Strip ANSI escape codes from a string
fn strip_ansi_codes(s: &str) -> String {
    ANSI_RE.replace_all(s, "").to_string()
}

/// Check if a host directory already exists on the filesystem
fn host_dir_exists(hostname: &str) -> bool {
    // Check common config locations for existing host directory
    let locations = [
        format!("/tmp/nixos-config/hosts/{}", hostname),
        format!(
            "{}/nixos-config/hosts/{}",
            std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()),
            hostname
        ),
        format!("/etc/nixos/hosts/{}", hostname),
    ];

    locations.iter().any(|p| std::path::Path::new(p).exists())
}

// Host configurations are now discovered dynamically - see discover_hosts() in system/config.rs

/// Main menu items
pub const MAIN_MENU_ITEMS: &[&str] = &[
    "Install NixOS (fresh installation)",
    "Update system",
    "App profiles",
    "Exit",
];

/// App profile menu items (browsers, Termius, etc.)
pub const APP_MENU_ITEMS: &[&str] = &[
    "Backup & push to GitHub",
    "Pull & restore from GitHub",
    "Check for updates",
    "Back to main menu",
];

/// Application mode/screen
#[derive(Debug, Clone)]
pub enum AppMode {
    MainMenu { selected: usize },
    Install(InstallState),
    CreateHost(CreateHostState),
    Update(UpdateState),
    Apps(AppProfileState),
    Keys(KeysState),
    #[allow(dead_code)]
    Quit,
}

/// Installation state machine
#[derive(Debug, Clone)]
pub enum InstallState {
    SelectHost {
        selected: usize,
    },
    SelectDisk {
        host: String,
        disks: Vec<DiskInfo>,
        selected: usize,
    },
    Confirm {
        host: String,
        disk: DiskInfo,
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
        scroll_offset: usize,
    },
}

impl InstallState {
    pub fn new(hostname: Option<String>, disk: Option<String>) -> Self {
        match (hostname, disk) {
            (Some(host), Some(disk_path)) => {
                // Direct install with provided args
                let disk = DiskInfo {
                    path: disk_path,
                    size: "Unknown".to_string(),
                    size_bytes: 0,
                    model: None,
                    partitions: vec![],
                };
                InstallState::Confirm {
                    host,
                    disk,
                    input: String::new(),
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
        hostname: String,
        disk: DiskInfo,
        #[allow(dead_code)]
        proceed_to_install: Option<bool>,
    },
}

impl CreateHostState {
    pub fn new() -> Self {
        CreateHostState::DetectingHardware
    }
}

/// Update state machine
#[derive(Debug, Clone)]
pub enum UpdateState {
    Running {
        step: usize,
        steps: Vec<StepStatus>,
        output: VecDeque<String>,
    },
    Complete {
        #[allow(dead_code)]
        success: bool,
        steps: Vec<StepStatus>,
        output: VecDeque<String>,
        scroll_offset: usize,
    },
}

impl UpdateState {
    pub fn new() -> Self {
        UpdateState::Running {
            step: 0,
            steps: vec![
                StepStatus::new("Updating flake inputs"),
                StepStatus::new("Rebuilding system"),
                StepStatus::new("Comparing packages"),
                StepStatus::new("Updating Claude Code"),
                StepStatus::new("Updating Codex CLI"),
                StepStatus::new("Checking browser profiles"),
            ],
            output: VecDeque::new(),
        }
    }
}

/// App profile management state (browsers, Termius, etc.)
#[derive(Debug, Clone)]
pub enum AppProfileState {
    Menu { selected: usize },
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
        scroll_offset: usize,
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
        AppProfileState::Status { output: VecDeque::new() }
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
        scroll_offset: usize,
    },
}

impl KeysState {
    pub fn new_setup() -> Self {
        KeysState::Running {
            operation: KeysOp::Setup,
            output: VecDeque::new(),
            force: false,
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

/// Step progress status
#[derive(Debug, Clone)]
pub struct StepStatus {
    pub name: String,
    pub status: StepState,
}

impl StepStatus {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: StepState::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StepState {
    Pending,
    Running,
    Complete,
    Failed,
    Skipped,
}

/// Update summary data
#[derive(Debug, Clone, Default)]
pub struct UpdateSummary {
    pub flake_changes: Vec<(String, String, String)>, // (input, old_rev, new_rev)
    pub package_changes: Vec<(String, String, String)>, // (pkg, old_ver, new_ver)
    pub claude_old: Option<String>,
    pub claude_new: Option<String>,
    pub codex_old: Option<String>,
    pub codex_new: Option<String>,
    pub browser_status: String,
    pub rebuild_skipped: bool,
    pub rebuild_failed: bool,
    pub log_path: String,
}

/// Main application state
pub struct App {
    pub mode: AppMode,
    pub should_quit: bool,
    pub show_exit_confirm: bool,
    /// Show dialog when app profile updates are available
    pub show_update_dialog: bool,
    /// Whether the startup update check is in progress
    pub startup_check_running: bool,
    pub spinner_state: usize,
    pub last_tick: Instant,
    pub error: Option<String>,
    pub hosts: Vec<HostConfig>,
    cmd_tx: Option<mpsc::Sender<CommandMessage>>,
    screen_log: Option<File>,
    pub screen_log_path: PathBuf,
}

impl App {
    pub fn new(initial_mode: AppMode) -> Self {
        // Set up screen log file
        let log_dir = dirs::home_dir()
            .map(|h| h.join(".local/share/forge"))
            .unwrap_or_else(|| PathBuf::from("/tmp/forge"));
        let _ = std::fs::create_dir_all(&log_dir);
        let screen_log_path = log_dir.join("screen.log");

        // Open log file (truncate existing)
        let mut screen_log = match File::create(&screen_log_path) {
            Ok(file) => Some(file),
            Err(e) => {
                tracing::warn!("Failed to create screen log file: {}", e);
                None
            }
        };

        // Write header to log
        if let Some(ref mut file) = screen_log {
            let _ = writeln!(file, "=== Forge Screen Log ===\n");
            let _ = file.flush();
        }

        Self {
            mode: initial_mode,
            should_quit: false,
            show_exit_confirm: false,
            show_update_dialog: false,
            startup_check_running: false,
            spinner_state: 0,
            last_tick: Instant::now(),
            error: None,
            hosts: discover_hosts(),
            cmd_tx: None,
            screen_log,
            screen_log_path,
        }
    }

    pub fn set_command_sender(&mut self, tx: mpsc::Sender<CommandMessage>) {
        self.cmd_tx = Some(tx);
    }

    /// Write a line to the screen log file
    pub fn log_to_screen(&mut self, line: &str) {
        if let Some(ref mut file) = self.screen_log {
            let _ = writeln!(file, "{}", line);
            let _ = file.flush();
        }
    }

    /// Called on each tick to update animations
    pub fn tick(&mut self) {
        if self.last_tick.elapsed().as_millis() >= SPINNER_TICK_MS {
            self.spinner_state = (self.spinner_state + 1) % 10;
            self.last_tick = Instant::now();
        }
    }

    /// Start initial command if mode requires it
    pub async fn start_initial_command(&mut self) -> Result<()> {
        match &mut self.mode {
            AppMode::Update(UpdateState::Running { steps, .. }) => {
                if !steps.is_empty() {
                    steps[0].status = StepState::Running;
                }
                if let Some(tx) = &self.cmd_tx {
                    commands::update::start_update(tx.clone()).await?;
                }
            }
            AppMode::Apps(AppProfileState::Running {
                operation, force, ..
            }) => {
                if let Some(tx) = &self.cmd_tx {
                    match operation {
                        AppOp::Backup => {
                            commands::apps::start_backup(tx.clone(), *force).await?;
                        }
                        AppOp::Restore => {
                            commands::apps::start_restore(tx.clone(), *force).await?;
                        }
                    }
                }
            }
            AppMode::Apps(AppProfileState::Status { .. }) => {
                if let Some(tx) = &self.cmd_tx {
                    commands::apps::start_status(tx.clone()).await?;
                }
            }
            AppMode::Keys(KeysState::Running {
                operation, force, ..
            }) => {
                if let Some(tx) = &self.cmd_tx {
                    match operation {
                        KeysOp::Setup => {
                            commands::keys::start_setup(tx.clone()).await?;
                        }
                        KeysOp::Backup => {
                            commands::keys::start_backup(tx.clone()).await?;
                        }
                        KeysOp::Restore => {
                            commands::keys::start_restore(tx.clone(), *force).await?;
                        }
                        KeysOp::Status => {
                            commands::keys::start_status(tx.clone()).await?;
                        }
                    }
                }
            }
            AppMode::Install(InstallState::SelectDisk { disks, .. }) => {
                // Load disk list
                *disks = crate::system::disk::get_available_disks()?;
            }
            AppMode::CreateHost(CreateHostState::DetectingHardware) => {
                // Detect hardware and transition to ConfirmCpu
                match crate::system::hardware::detect_all() {
                    Ok(hw) => {
                        // If CPU is unknown, force manual selection
                        let cpu_override = hw.cpu.vendor == CpuVendor::Unknown;
                        self.mode = AppMode::CreateHost(CreateHostState::ConfirmCpu {
                            cpu: hw.cpu,
                            detected_gpu: hw.gpu,
                            detected_form_factor: hw.form_factor,
                            override_menu: cpu_override,
                            selected: 0,
                        });
                    }
                    Err(e) => {
                        tracing::error!("Hardware detection failed: {}", e);
                        // Fall back to manual selection with defaults
                        self.mode = AppMode::CreateHost(CreateHostState::ConfirmCpu {
                            cpu: crate::system::hardware::CpuInfo {
                                vendor: CpuVendor::Unknown,
                                model_name: "Unknown (detection failed)".to_string(),
                            },
                            detected_gpu: GpuInfo {
                                vendor: GpuVendor::None,
                                model: None,
                            },
                            detected_form_factor: FormFactor::Desktop,
                            override_menu: true, // Force manual selection
                            selected: 0,
                        });
                    }
                }
            }
            AppMode::CreateHost(CreateHostState::SelectDisk { disks, .. }) => {
                // Load disk list
                *disks = crate::system::disk::get_available_disks()?;
            }
            AppMode::MainMenu { .. } => {
                // Start background check for app profile updates
                if let Some(tx) = &self.cmd_tx {
                    self.startup_check_running = true;
                    commands::apps::start_quick_update_check(tx.clone()).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle keyboard input
    pub async fn handle_key(&mut self, key: KeyCode) -> Result<()> {
        // Handle exit confirmation dialog
        if self.show_exit_confirm {
            match key {
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.should_quit = true;
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.show_exit_confirm = false;
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle update dialog
        if self.show_update_dialog {
            match key {
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.show_update_dialog = false;
                    // Navigate to restore and start it
                    self.mode = AppMode::Apps(AppProfileState::new_restore(false));
                    self.start_initial_command().await?;
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.show_update_dialog = false;
                }
                _ => {}
            }
            return Ok(());
        }

        // Global quit
        if matches!(key, KeyCode::Char('q') | KeyCode::Char('Q'))
            && matches!(
                self.mode,
                AppMode::MainMenu { .. }
                    | AppMode::Apps(AppProfileState::Menu { .. })
                    | AppMode::Apps(AppProfileState::Complete { .. })
                    | AppMode::Apps(AppProfileState::Status { .. })
                    | AppMode::Keys(KeysState::Complete { .. })
                    | AppMode::Update(UpdateState::Complete { .. })
                    | AppMode::Install(InstallState::Complete { .. })
                    | AppMode::CreateHost(CreateHostState::Complete { .. })
            )
        {
            self.show_exit_confirm = true;
            return Ok(());
        }

        // Escape to go back (show confirm if on main menu)
        if key == KeyCode::Esc {
            if matches!(self.mode, AppMode::MainMenu { .. }) {
                self.show_exit_confirm = true;
                return Ok(());
            }
            self.handle_back().await?;
            return Ok(());
        }

        // Extract values from mode to avoid borrow conflicts
        let action = match &self.mode {
            AppMode::MainMenu { selected } => Some(("main_menu", *selected, None, None)),
            AppMode::Apps(AppProfileState::Menu { selected }) => {
                Some(("browser_menu", *selected, None, None))
            }
            AppMode::Install(InstallState::SelectHost { selected }) => {
                Some(("install_host", *selected, None, None))
            }
            AppMode::Install(InstallState::SelectDisk {
                host,
                disks,
                selected,
            }) => Some((
                "install_disk",
                *selected,
                Some(host.clone()),
                Some(disks.clone()),
            )),
            AppMode::Install(InstallState::Confirm { host, disk: _, .. }) => {
                Some(("install_confirm", 0, Some(host.clone()), None))
            }
            AppMode::Install(InstallState::Complete { .. })
            | AppMode::Update(UpdateState::Complete { .. })
            | AppMode::Apps(AppProfileState::Complete { .. })
            | AppMode::Keys(KeysState::Complete { .. }) => {
                match key {
                    KeyCode::Enter => Some(("complete", 0, None, None)),
                    KeyCode::Up | KeyCode::Down => Some(("scroll", 0, None, None)),
                    _ => None,
                }
            }
            AppMode::Apps(AppProfileState::Status { .. }) => {
                if key == KeyCode::Enter {
                    Some(("browser_done", 0, None, None))
                } else {
                    None
                }
            }
            AppMode::CreateHost(_) => Some(("create_host", 0, None, None)),
            _ => None,
        };

        match action {
            Some(("main_menu", selected, _, _)) => {
                self.handle_main_menu_key(key, selected).await?;
            }
            Some(("browser_menu", selected, _, _)) => {
                self.handle_app_menu_key(key, selected).await?;
            }
            Some(("install_host", selected, _, _)) => {
                self.handle_install_host_key(key, selected).await?;
            }
            Some(("install_disk", selected, Some(host), Some(disks))) => {
                self.handle_install_disk_key(key, &host, &disks, selected)
                    .await?;
            }
            Some(("install_confirm", _, Some(host), _)) => {
                self.handle_confirm_key_action(key, &host).await?;
            }
            Some(("complete", _, _, _)) => {
                self.mode = AppMode::MainMenu { selected: 0 };
            }
            Some(("scroll", _, _, _)) => {
                self.handle_scroll(key);
            }
            Some(("browser_done", _, _, _)) => {
                self.mode = AppMode::Apps(AppProfileState::Menu { selected: 0 });
            }
            Some(("create_host", _, _, _)) => {
                self.handle_create_host_key(key).await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn handle_main_menu_key(&mut self, key: KeyCode, current_selected: usize) -> Result<()> {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                if let AppMode::MainMenu { selected } = &mut self.mode {
                    *selected = selected.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let AppMode::MainMenu { selected } = &mut self.mode {
                    *selected = (*selected + 1).min(MAIN_MENU_ITEMS.len() - 1);
                }
            }
            KeyCode::Enter => {
                self.handle_main_menu_select(current_selected).await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_main_menu_select(&mut self, selected: usize) -> Result<()> {
        match selected {
            0 => {
                // Install
                self.mode = AppMode::Install(InstallState::SelectHost { selected: 0 });
            }
            1 => {
                // Update
                self.mode = AppMode::Update(UpdateState::new());
                self.start_initial_command().await?;
            }
            2 => {
                // App profiles
                self.mode = AppMode::Apps(AppProfileState::Menu { selected: 0 });
            }
            3 => {
                // Exit
                self.should_quit = true;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle scroll keys for complete screens
    fn handle_scroll(&mut self, key: KeyCode) {
        match &mut self.mode {
            AppMode::Install(InstallState::Complete {
                output,
                scroll_offset,
                ..
            })
            | AppMode::Update(UpdateState::Complete {
                output,
                scroll_offset,
                ..
            })
            | AppMode::Apps(AppProfileState::Complete {
                output,
                scroll_offset,
                ..
            })
            | AppMode::Keys(KeysState::Complete {
                output,
                scroll_offset,
                ..
            }) => match key {
                KeyCode::Up => {
                    *scroll_offset = scroll_offset.saturating_sub(1);
                }
                KeyCode::Down => {
                    if *scroll_offset < output.len().saturating_sub(1) {
                        *scroll_offset += 1;
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    async fn handle_app_menu_key(&mut self, key: KeyCode, selected: usize) -> Result<()> {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                if let AppMode::Apps(AppProfileState::Menu { selected }) = &mut self.mode {
                    *selected = selected.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let AppMode::Apps(AppProfileState::Menu { selected }) = &mut self.mode {
                    *selected = (*selected + 1).min(APP_MENU_ITEMS.len() - 1);
                }
            }
            KeyCode::Enter => match selected {
                0 => {
                    // Backup
                    self.mode = AppMode::Apps(AppProfileState::new_backup(false));
                    self.start_initial_command().await?;
                }
                1 => {
                    // Restore
                    self.mode = AppMode::Apps(AppProfileState::new_restore(false));
                    self.start_initial_command().await?;
                }
                2 => {
                    // Status
                    self.mode = AppMode::Apps(AppProfileState::new_status());
                    self.start_initial_command().await?;
                }
                3 => {
                    // Back
                    self.mode = AppMode::MainMenu { selected: 2 };
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    async fn handle_install_host_key(&mut self, key: KeyCode, selected: usize) -> Result<()> {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                if let AppMode::Install(InstallState::SelectHost { selected }) = &mut self.mode {
                    *selected = selected.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let AppMode::Install(InstallState::SelectHost { selected }) = &mut self.mode {
                    // +1 for "New host configuration" option
                    *selected = (*selected + 1).min(self.hosts.len());
                }
            }
            KeyCode::Enter => {
                if selected == 0 {
                    // "New host configuration" selected
                    self.mode = AppMode::CreateHost(CreateHostState::new());
                    self.start_initial_command().await?;
                } else {
                    // Existing host selected (index - 1 because of "New host" option)
                    let host = self.hosts[selected - 1].name.clone();
                    self.mode = AppMode::Install(InstallState::SelectDisk {
                        host,
                        disks: Vec::new(),
                        selected: 0,
                    });
                    self.start_initial_command().await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_install_disk_key(
        &mut self,
        key: KeyCode,
        host: &str,
        disks: &[DiskInfo],
        selected: usize,
    ) -> Result<()> {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                if let AppMode::Install(InstallState::SelectDisk { selected, .. }) = &mut self.mode
                {
                    *selected = selected.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let AppMode::Install(InstallState::SelectDisk { selected, disks, .. }) =
                    &mut self.mode
                {
                    // Only navigate if we have disks
                    if !disks.is_empty() {
                        *selected = (*selected + 1).min(disks.len() - 1);
                    }
                }
            }
            KeyCode::Enter => {
                if !disks.is_empty() {
                    self.mode = AppMode::Install(InstallState::Confirm {
                        host: host.to_string(),
                        disk: disks[selected].clone(),
                        input: String::new(),
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_confirm_key_action(&mut self, key: KeyCode, host: &str) -> Result<()> {
        // Get disk from current state
        let (disk, should_start) = if let AppMode::Install(InstallState::Confirm {
            disk,
            input,
            ..
        }) = &mut self.mode
        {
            match key {
                KeyCode::Char(c) => {
                    // Limit input length to prevent memory exhaustion
                    if input.len() < MAX_INPUT_LENGTH {
                        input.push(c);
                    }
                    (None, false)
                }
                KeyCode::Backspace => {
                    input.pop();
                    (None, false)
                }
                KeyCode::Enter => {
                    if input.trim().eq_ignore_ascii_case("yes") {
                        (Some(disk.clone()), true)
                    } else {
                        (None, false)
                    }
                }
                _ => (None, false),
            }
        } else {
            (None, false)
        };

        if should_start {
            if let Some(disk) = disk {
                // Start installation
                let mut steps = vec![
                    StepStatus::new("Checking network connectivity"),
                    StepStatus::new("Enabling Nix flakes"),
                    StepStatus::new("Cloning configuration repository"),
                    StepStatus::new("Configuring disk device"),
                    StepStatus::new("Running disko (partitioning)"),
                    StepStatus::new("Installing NixOS"),
                ];
                // Mark first step as running
                steps[0].status = StepState::Running;

                self.mode = AppMode::Install(InstallState::Running {
                    host: host.to_string(),
                    disk: disk.clone(),
                    step: 0,
                    steps,
                    output: VecDeque::new(),
                });
                if let Some(tx) = &self.cmd_tx {
                    commands::install::start_install(tx.clone(), host, &disk.path).await?;
                }
            }
        }
        Ok(())
    }

    /// Handle keyboard input for create host wizard
    async fn handle_create_host_key(&mut self, key: KeyCode) -> Result<()> {
        match &mut self.mode {
            AppMode::CreateHost(CreateHostState::ConfirmCpu {
                cpu,
                detected_gpu,
                detected_form_factor,
                override_menu,
                selected,
            }) => {
                if *override_menu {
                    // Menu selection mode
                    match key {
                        KeyCode::Up | KeyCode::Char('k') => {
                            *selected = selected.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *selected = (*selected + 1).min(1); // AMD or Intel
                        }
                        KeyCode::Enter => {
                            let new_vendor = if *selected == 0 {
                                CpuVendor::AMD
                            } else {
                                CpuVendor::Intel
                            };
                            let gpu = detected_gpu.clone();
                            let form_factor = *detected_form_factor;
                            // If GPU is None, force manual selection
                            let gpu_override = gpu.vendor == GpuVendor::None;
                            self.mode = AppMode::CreateHost(CreateHostState::ConfirmGpu {
                                cpu: CpuInfo {
                                    vendor: new_vendor,
                                    model_name: format!("{} (manually selected)", new_vendor),
                                },
                                gpu,
                                detected_form_factor: form_factor,
                                override_menu: gpu_override,
                                selected: 0,
                            });
                        }
                        _ => {}
                    }
                } else {
                    // Confirmation mode
                    match key {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                            let cpu = cpu.clone();
                            let gpu = detected_gpu.clone();
                            let form_factor = *detected_form_factor;
                            // If GPU is None, force manual selection
                            let gpu_override = gpu.vendor == GpuVendor::None;
                            self.mode = AppMode::CreateHost(CreateHostState::ConfirmGpu {
                                cpu,
                                gpu,
                                detected_form_factor: form_factor,
                                override_menu: gpu_override,
                                selected: 0,
                            });
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            *override_menu = true;
                        }
                        _ => {}
                    }
                }
            }

            AppMode::CreateHost(CreateHostState::ConfirmGpu {
                cpu,
                gpu,
                detected_form_factor,
                override_menu,
                selected,
            }) => {
                if *override_menu {
                    // Menu selection mode (4 options: NVIDIA, AMD, Intel, None)
                    match key {
                        KeyCode::Up | KeyCode::Char('k') => {
                            *selected = selected.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *selected = (*selected + 1).min(3);
                        }
                        KeyCode::Enter => {
                            let new_vendor = match *selected {
                                0 => GpuVendor::NVIDIA,
                                1 => GpuVendor::AMD,
                                2 => GpuVendor::Intel,
                                _ => GpuVendor::None,
                            };
                            let cpu = cpu.clone();
                            let form_factor = *detected_form_factor;
                            self.mode = AppMode::CreateHost(CreateHostState::ConfirmFormFactor {
                                cpu,
                                gpu: GpuInfo {
                                    vendor: new_vendor,
                                    model: Some(format!("{} (manually selected)", new_vendor)),
                                },
                                form_factor,
                                override_menu: false,
                                selected: 0,
                            });
                        }
                        _ => {}
                    }
                } else {
                    match key {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                            let cpu = cpu.clone();
                            let gpu = gpu.clone();
                            let form_factor = *detected_form_factor;
                            self.mode = AppMode::CreateHost(CreateHostState::ConfirmFormFactor {
                                cpu,
                                gpu,
                                form_factor,
                                override_menu: false,
                                selected: 0,
                            });
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            *override_menu = true;
                        }
                        _ => {}
                    }
                }
            }

            AppMode::CreateHost(CreateHostState::ConfirmFormFactor {
                cpu,
                gpu,
                form_factor,
                override_menu,
                selected,
            }) => {
                if *override_menu {
                    // Menu selection mode (2 options: Desktop, Laptop)
                    match key {
                        KeyCode::Up | KeyCode::Char('k') => {
                            *selected = selected.saturating_sub(1);
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *selected = (*selected + 1).min(1);
                        }
                        KeyCode::Enter => {
                            let new_form_factor = if *selected == 0 {
                                FormFactor::Desktop
                            } else {
                                FormFactor::Laptop
                            };
                            let cpu = cpu.clone();
                            let gpu = gpu.clone();
                            self.mode = AppMode::CreateHost(CreateHostState::SelectDisk {
                                cpu,
                                gpu,
                                form_factor: new_form_factor,
                                disks: Vec::new(),
                                selected: 0,
                            });
                            self.start_initial_command().await?;
                        }
                        _ => {}
                    }
                } else {
                    match key {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                            let cpu = cpu.clone();
                            let gpu = gpu.clone();
                            let ff = *form_factor;
                            self.mode = AppMode::CreateHost(CreateHostState::SelectDisk {
                                cpu,
                                gpu,
                                form_factor: ff,
                                disks: Vec::new(),
                                selected: 0,
                            });
                            self.start_initial_command().await?;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            *override_menu = true;
                        }
                        _ => {}
                    }
                }
            }

            AppMode::CreateHost(CreateHostState::SelectDisk {
                cpu,
                gpu,
                form_factor,
                disks,
                selected,
            }) => {
                match key {
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = selected.saturating_sub(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !disks.is_empty() {
                            *selected = (*selected + 1).min(disks.len() - 1);
                        }
                    }
                    KeyCode::Enter => {
                        // Transition to EnterHostname (hostname is now entered after disk selection)
                        if !disks.is_empty() {
                            self.mode = AppMode::CreateHost(CreateHostState::EnterHostname {
                                cpu: cpu.clone(),
                                gpu: gpu.clone(),
                                form_factor: *form_factor,
                                disk: disks[*selected].clone(),
                                input: String::new(),
                                error: None,
                            });
                        }
                    }
                    _ => {}
                }
            }

            AppMode::CreateHost(CreateHostState::EnterHostname {
                cpu,
                gpu,
                form_factor,
                disk,
                input,
                error,
            }) => {
                match key {
                    KeyCode::Char(c) => {
                        if input.len() < MAX_INPUT_LENGTH {
                            // Only allow valid hostname characters
                            if c.is_alphanumeric() || c == '-' {
                                input.push(c.to_ascii_lowercase());
                                *error = None;
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        input.pop();
                        *error = None;
                    }
                    KeyCode::Enter => {
                        let hostname = input.trim().to_string();
                        // Validate hostname
                        if hostname.is_empty() {
                            *error = Some("Hostname cannot be empty".to_string());
                        } else if hostname.len() > 63 {
                            *error = Some("Hostname too long (max 63 characters)".to_string());
                        } else if !hostname.chars().next().unwrap().is_alphanumeric() {
                            *error = Some("Hostname must start with a letter or number".to_string());
                        } else if !hostname.chars().all(|c| c.is_alphanumeric() || c == '-') {
                            *error = Some("Hostname can only contain letters, numbers, and hyphens".to_string());
                        } else {
                            // Check if host already exists (dynamic list or filesystem)
                            let host_exists = self.hosts.iter().any(|h| h.name == hostname)
                                || host_dir_exists(&hostname);
                            if host_exists {
                                *error = Some(format!("Host '{}' already exists", hostname));
                            } else {
                                // Proceed to Review with complete config
                                let config = NewHostConfig {
                                    hostname,
                                    cpu: cpu.clone(),
                                    gpu: gpu.clone(),
                                    form_factor: *form_factor,
                                    disk: disk.clone(),
                                };
                                self.mode = AppMode::CreateHost(CreateHostState::Review { config });
                            }
                        }
                    }
                    _ => {}
                }
            }

            AppMode::CreateHost(CreateHostState::Review { config }) => {
                if key == KeyCode::Enter {
                    // Start generating files
                    let config = config.clone();
                    let mut steps = vec![
                        StepStatus::new("Creating host directory"),
                        StepStatus::new("Generating hardware configuration"),
                        StepStatus::new("Creating host configuration"),
                        StepStatus::new("Creating disko configuration"),
                        StepStatus::new("Updating flake.nix"),
                    ];
                    steps[0].status = StepState::Running;

                    self.mode = AppMode::CreateHost(CreateHostState::Generating {
                        config,
                        step: 0,
                        steps,
                        output: VecDeque::new(),
                    });

                    if let Some(tx) = &self.cmd_tx {
                        commands::create_host::start_create_host(tx.clone(), self.mode.clone())
                            .await?;
                    }
                }
            }

            AppMode::CreateHost(CreateHostState::Complete {
                hostname,
                disk,
                proceed_to_install: _,
                success,
            }) => {
                if *success {
                    match key {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                            // Proceed to install
                            let hostname = hostname.clone();
                            let disk = disk.clone();
                            self.mode = AppMode::Install(InstallState::Confirm {
                                host: hostname,
                                disk,
                                input: String::new(),
                            });
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            self.mode = AppMode::Install(InstallState::SelectHost { selected: 0 });
                        }
                        _ => {}
                    }
                } else {
                    // Failed - just go back on any key
                    if key == KeyCode::Enter {
                        self.mode = AppMode::Install(InstallState::SelectHost { selected: 0 });
                    }
                }
            }

            _ => {}
        }
        Ok(())
    }

    async fn handle_back(&mut self) -> Result<()> {
        let needs_disk_refresh = matches!(
            self.mode,
            AppMode::Install(InstallState::Confirm { .. })
        );

        let needs_create_host_disk_refresh = matches!(
            self.mode,
            AppMode::CreateHost(CreateHostState::EnterHostname { .. })
        );

        self.mode = match &self.mode {
            AppMode::Apps(AppProfileState::Menu { .. }) => AppMode::MainMenu { selected: 2 },
            AppMode::Apps(AppProfileState::Complete { .. })
            | AppMode::Apps(AppProfileState::Status { .. }) => {
                AppMode::Apps(AppProfileState::Menu { selected: 0 })
            }
            AppMode::Keys(KeysState::Complete { .. }) => {
                AppMode::MainMenu { selected: 2 }
            }
            AppMode::Install(InstallState::SelectHost { .. }) => {
                AppMode::MainMenu { selected: 0 }
            }
            AppMode::Install(InstallState::SelectDisk { .. }) => {
                AppMode::Install(InstallState::SelectHost { selected: 0 })
            }
            AppMode::Install(InstallState::Confirm { host, .. }) => {
                // Go back to disk selection, not host selection
                AppMode::Install(InstallState::SelectDisk {
                    host: host.clone(),
                    disks: Vec::new(),
                    selected: 0,
                })
            }
            AppMode::Install(InstallState::Complete { .. }) => AppMode::MainMenu { selected: 0 },
            AppMode::Update(UpdateState::Complete { .. }) => AppMode::MainMenu { selected: 1 },
            // CreateHost back navigation
            // New flow: DetectingHardware → ConfirmCpu → ConfirmGpu → ConfirmFormFactor → SelectDisk → EnterHostname → Review
            AppMode::CreateHost(CreateHostState::DetectingHardware) => {
                AppMode::Install(InstallState::SelectHost { selected: 0 })
            }
            AppMode::CreateHost(CreateHostState::ConfirmCpu { .. }) => {
                // Go back to Install host selection (DetectingHardware auto-transitions)
                AppMode::Install(InstallState::SelectHost { selected: 0 })
            }
            AppMode::CreateHost(CreateHostState::ConfirmGpu {
                cpu,
                gpu,
                detected_form_factor,
                ..
            }) => AppMode::CreateHost(CreateHostState::ConfirmCpu {
                cpu: cpu.clone(),
                detected_gpu: gpu.clone(),
                detected_form_factor: *detected_form_factor,
                override_menu: false,
                selected: 0,
            }),
            AppMode::CreateHost(CreateHostState::ConfirmFormFactor {
                cpu,
                gpu,
                form_factor,
                ..
            }) => AppMode::CreateHost(CreateHostState::ConfirmGpu {
                cpu: cpu.clone(),
                gpu: gpu.clone(),
                detected_form_factor: *form_factor,
                override_menu: false,
                selected: 0,
            }),
            AppMode::CreateHost(CreateHostState::SelectDisk {
                cpu,
                gpu,
                form_factor,
                ..
            }) => AppMode::CreateHost(CreateHostState::ConfirmFormFactor {
                cpu: cpu.clone(),
                gpu: gpu.clone(),
                form_factor: *form_factor,
                override_menu: false,
                selected: 0,
            }),
            AppMode::CreateHost(CreateHostState::EnterHostname {
                cpu,
                gpu,
                form_factor,
                ..
            }) => {
                // Go back to disk selection
                AppMode::CreateHost(CreateHostState::SelectDisk {
                    cpu: cpu.clone(),
                    gpu: gpu.clone(),
                    form_factor: *form_factor,
                    disks: Vec::new(),
                    selected: 0,
                })
            }
            AppMode::CreateHost(CreateHostState::Review { config }) => {
                // Go back to hostname entry
                AppMode::CreateHost(CreateHostState::EnterHostname {
                    cpu: config.cpu.clone(),
                    gpu: config.gpu.clone(),
                    form_factor: config.form_factor,
                    disk: config.disk.clone(),
                    input: config.hostname.clone(),
                    error: None,
                })
            }
            AppMode::CreateHost(CreateHostState::Complete { .. }) => {
                AppMode::Install(InstallState::SelectHost { selected: 0 })
            }
            _ => return Ok(()),
        };

        // Repopulate disk list if needed
        if needs_disk_refresh || needs_create_host_disk_refresh {
            self.start_initial_command().await?;
        }

        Ok(())
    }

    /// Handle messages from running commands
    pub async fn handle_command_message(&mut self, msg: CommandMessage) -> Result<()> {
        match msg {
            CommandMessage::Stdout(line) | CommandMessage::Stderr(line) => {
                self.append_output(&line);
            }
            CommandMessage::StepComplete { step } => {
                self.mark_step_complete(&step);
            }
            CommandMessage::StepFailed { step, error } => {
                self.mark_step_failed(&step, &error);
            }
            CommandMessage::StepSkipped { step } => {
                self.mark_step_skipped(&step);
            }
            CommandMessage::Done { success } => {
                self.handle_command_done(success);
            }
            CommandMessage::AppUpdatesAvailable { available } => {
                self.startup_check_running = false;
                if available && matches!(self.mode, AppMode::MainMenu { .. }) {
                    self.show_update_dialog = true;
                }
            }
        }
        Ok(())
    }

    fn append_output(&mut self, line: &str) {
        // Strip ANSI escape codes from the line
        let clean_line = strip_ansi_codes(line);

        // Log to screen file
        self.log_to_screen(&clean_line);

        match &mut self.mode {
            AppMode::Update(UpdateState::Running { output, .. })
            | AppMode::Update(UpdateState::Complete { output, .. }) => {
                output.push_back(clean_line.clone());
                // Keep last OUTPUT_BUFFER_SIZE lines - O(1) removal from front
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            AppMode::Install(InstallState::Running { output, .. }) => {
                output.push_back(clean_line.clone());
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            AppMode::Apps(AppProfileState::Running { output, .. }) => {
                output.push_back(clean_line.clone());
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            AppMode::Apps(AppProfileState::Status { output }) => {
                output.push_back(clean_line.clone());
            }
            AppMode::Keys(KeysState::Running { output, .. }) => {
                output.push_back(clean_line.clone());
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            AppMode::CreateHost(CreateHostState::Generating { output, .. }) => {
                output.push_back(clean_line.clone());
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            _ => {}
        }
    }

    /// Check if a step matches the given step name.
    /// Uses case-insensitive matching and checks both full name and first word.
    fn step_matches(step: &StepStatus, step_name: &str) -> bool {
        let step_lower = step.name.to_lowercase();
        let name_lower = step_name.to_lowercase();

        // Check if the step name contains the search term
        if step_lower.contains(&name_lower) {
            return true;
        }

        // Check if the search term matches the first word of the step name
        if let Some(first_word) = step_lower.split_whitespace().next() {
            if first_word == name_lower || name_lower.contains(first_word) {
                return true;
            }
        }

        false
    }

    fn mark_step_complete(&mut self, step_name: &str) {
        self.log_to_screen(&format!("[✓] Step complete: {}", step_name));

        match &mut self.mode {
            AppMode::Update(UpdateState::Running { steps, step, .. }) => {
                if let Some(s) = steps.iter_mut().find(|s| Self::step_matches(s, step_name)) {
                    s.status = StepState::Complete;
                }
                *step = (*step + 1).min(steps.len());
                if *step < steps.len() {
                    steps[*step].status = StepState::Running;
                }
            }
            AppMode::Install(InstallState::Running { steps, step, .. }) => {
                if let Some(s) = steps.iter_mut().find(|s| Self::step_matches(s, step_name)) {
                    s.status = StepState::Complete;
                }
                *step = (*step + 1).min(steps.len());
                if *step < steps.len() {
                    steps[*step].status = StepState::Running;
                }
            }
            AppMode::CreateHost(CreateHostState::Generating { steps, step, .. }) => {
                if let Some(s) = steps.iter_mut().find(|s| Self::step_matches(s, step_name)) {
                    s.status = StepState::Complete;
                }
                *step = (*step + 1).min(steps.len());
                if *step < steps.len() {
                    steps[*step].status = StepState::Running;
                }
            }
            _ => {}
        }
    }

    fn mark_step_failed(&mut self, step_name: &str, error: &str) {
        self.log_to_screen(&format!("[✗] Step failed: {} - {}", step_name, error));

        match &mut self.mode {
            AppMode::Update(UpdateState::Running { steps, .. }) => {
                if let Some(s) = steps.iter_mut().find(|s| Self::step_matches(s, step_name)) {
                    s.status = StepState::Failed;
                }
                self.error = Some(error.to_string());
            }
            AppMode::Install(InstallState::Running { steps, .. }) => {
                if let Some(s) = steps.iter_mut().find(|s| Self::step_matches(s, step_name)) {
                    s.status = StepState::Failed;
                }
                self.error = Some(error.to_string());
            }
            AppMode::CreateHost(CreateHostState::Generating { steps, .. }) => {
                if let Some(s) = steps.iter_mut().find(|s| Self::step_matches(s, step_name)) {
                    s.status = StepState::Failed;
                }
                self.error = Some(error.to_string());
            }
            _ => {}
        }
    }

    fn mark_step_skipped(&mut self, step_name: &str) {
        self.log_to_screen(&format!("[-] Step skipped: {}", step_name));

        match &mut self.mode {
            AppMode::Update(UpdateState::Running { steps, step, .. }) => {
                if let Some(s) = steps.iter_mut().find(|s| Self::step_matches(s, step_name)) {
                    s.status = StepState::Skipped;
                }
                *step = (*step + 1).min(steps.len());
                if *step < steps.len() {
                    steps[*step].status = StepState::Running;
                }
            }
            _ => {}
        }
    }

    fn handle_command_done(&mut self, success: bool) {
        self.log_to_screen(&format!(
            "\n=== Operation {} ===\n",
            if success { "COMPLETED" } else { "FAILED" }
        ));

        match &mut self.mode {
            AppMode::Apps(AppProfileState::Running { output, .. }) => {
                self.mode = AppMode::Apps(AppProfileState::Complete {
                    success,
                    output: output.clone(),
                    scroll_offset: 0,
                });
            }
            AppMode::Keys(KeysState::Running { output, .. }) => {
                self.mode = AppMode::Keys(KeysState::Complete {
                    success,
                    output: output.clone(),
                    scroll_offset: 0,
                });
            }
            AppMode::Install(InstallState::Running { output, .. }) => {
                self.mode = AppMode::Install(InstallState::Complete {
                    success,
                    output: output.clone(),
                    scroll_offset: 0,
                });
            }
            AppMode::Update(UpdateState::Running { steps, output, .. }) => {
                self.mode = AppMode::Update(UpdateState::Complete {
                    success,
                    steps: steps.clone(),
                    output: output.clone(),
                    scroll_offset: 0,
                });
            }
            AppMode::CreateHost(CreateHostState::Generating { config, .. }) => {
                self.mode = AppMode::CreateHost(CreateHostState::Complete {
                    success,
                    hostname: config.hostname.clone(),
                    disk: config.disk.clone(),
                    proceed_to_install: None,
                });
            }
            _ => {}
        }
    }
}
