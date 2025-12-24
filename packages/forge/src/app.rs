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
use crate::system::disk::DiskInfo;

/// Regex to match ANSI escape codes.
/// This pattern is a compile-time constant and cannot fail to compile.
/// The unwrap is safe because the pattern is statically validated.
static ANSI_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap());

/// Strip ANSI escape codes from a string
fn strip_ansi_codes(s: &str) -> String {
    ANSI_RE.replace_all(s, "").to_string()
}

/// Available host configurations
pub const HOSTS: &[(&str, &str)] = &[
    ("kraken", "Desktop with NVIDIA RTX 5090"),
    ("G1a", "HP ZBook Ultra G1a (AMD Strix Halo)"),
];

/// Main menu items
pub const MAIN_MENU_ITEMS: &[&str] = &[
    "Install NixOS (fresh installation)",
    "Update system",
    "Browser profiles",
    "Exit",
];

/// Browser menu items
pub const BROWSER_MENU_ITEMS: &[&str] = &[
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
    Update(UpdateState),
    Browser(BrowserState),
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

/// Update state machine
#[derive(Debug, Clone)]
pub enum UpdateState {
    Running {
        step: usize,
        steps: Vec<StepStatus>,
        output: VecDeque<String>,
    },
    Complete {
        success: bool,
        steps: Vec<StepStatus>,
        output: VecDeque<String>,
    },
}

impl UpdateState {
    pub fn new() -> Self {
        UpdateState::Running {
            step: 0,
            steps: vec![
                StepStatus::new("Updating flake inputs"),
                StepStatus::new("Rebuilding system"),
                StepStatus::new("Updating Claude Code"),
                StepStatus::new("Updating Codex CLI"),
                StepStatus::new("Checking browser profiles"),
            ],
            output: VecDeque::new(),
        }
    }
}

/// Browser management state
#[derive(Debug, Clone)]
pub enum BrowserState {
    Menu { selected: usize },
    Running {
        operation: BrowserOp,
        output: VecDeque<String>,
        force: bool,
    },
    Status {
        output: VecDeque<String>,
    },
    Complete {
        success: bool,
        output: VecDeque<String>,
    },
}

impl BrowserState {
    pub fn new_menu() -> Self {
        BrowserState::Menu { selected: 0 }
    }

    pub fn new_backup(force: bool) -> Self {
        BrowserState::Running {
            operation: BrowserOp::Backup,
            output: VecDeque::new(),
            force,
        }
    }

    pub fn new_restore(force: bool) -> Self {
        BrowserState::Running {
            operation: BrowserOp::Restore,
            output: VecDeque::new(),
            force,
        }
    }

    pub fn new_status() -> Self {
        BrowserState::Status { output: VecDeque::new() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BrowserOp {
    Backup,
    Restore,
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
    pub spinner_state: usize,
    pub last_tick: Instant,
    pub error: Option<String>,
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
            spinner_state: 0,
            last_tick: Instant::now(),
            error: None,
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
            AppMode::Browser(BrowserState::Running {
                operation, force, ..
            }) => {
                if let Some(tx) = &self.cmd_tx {
                    match operation {
                        BrowserOp::Backup => {
                            commands::browser::start_backup(tx.clone(), *force).await?;
                        }
                        BrowserOp::Restore => {
                            commands::browser::start_restore(tx.clone(), *force).await?;
                        }
                    }
                }
            }
            AppMode::Browser(BrowserState::Status { .. }) => {
                if let Some(tx) = &self.cmd_tx {
                    commands::browser::start_status(tx.clone()).await?;
                }
            }
            AppMode::Install(InstallState::SelectDisk { disks, .. }) => {
                // Load disk list
                *disks = crate::system::disk::get_available_disks()?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle keyboard input
    pub async fn handle_key(&mut self, key: KeyCode) -> Result<()> {
        // Global quit
        if matches!(key, KeyCode::Char('q') | KeyCode::Char('Q'))
            && matches!(
                self.mode,
                AppMode::MainMenu { .. }
                    | AppMode::Browser(BrowserState::Menu { .. })
                    | AppMode::Browser(BrowserState::Complete { .. })
                    | AppMode::Browser(BrowserState::Status { .. })
                    | AppMode::Update(UpdateState::Complete { .. })
                    | AppMode::Install(InstallState::Complete { .. })
            )
        {
            self.should_quit = true;
            return Ok(());
        }

        // Escape to go back
        if key == KeyCode::Esc {
            self.handle_back().await?;
            return Ok(());
        }

        // Extract values from mode to avoid borrow conflicts
        let action = match &self.mode {
            AppMode::MainMenu { selected } => Some(("main_menu", *selected, None, None)),
            AppMode::Browser(BrowserState::Menu { selected }) => {
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
            AppMode::Install(InstallState::Confirm { host, disk, .. }) => {
                Some(("install_confirm", 0, Some(host.clone()), None))
            }
            AppMode::Install(InstallState::Complete { .. })
            | AppMode::Update(UpdateState::Complete { .. })
            | AppMode::Browser(BrowserState::Complete { .. }) => {
                if key == KeyCode::Enter {
                    Some(("complete", 0, None, None))
                } else {
                    None
                }
            }
            AppMode::Browser(BrowserState::Status { .. }) => {
                if key == KeyCode::Enter {
                    Some(("browser_done", 0, None, None))
                } else {
                    None
                }
            }
            _ => None,
        };

        match action {
            Some(("main_menu", selected, _, _)) => {
                self.handle_main_menu_key(key, selected).await?;
            }
            Some(("browser_menu", selected, _, _)) => {
                self.handle_browser_menu_key(key, selected).await?;
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
            Some(("browser_done", _, _, _)) => {
                self.mode = AppMode::Browser(BrowserState::Menu { selected: 0 });
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
                // Browser
                self.mode = AppMode::Browser(BrowserState::Menu { selected: 0 });
            }
            3 => {
                // Exit
                self.should_quit = true;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_browser_menu_key(&mut self, key: KeyCode, selected: usize) -> Result<()> {
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                if let AppMode::Browser(BrowserState::Menu { selected }) = &mut self.mode {
                    *selected = selected.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let AppMode::Browser(BrowserState::Menu { selected }) = &mut self.mode {
                    *selected = (*selected + 1).min(BROWSER_MENU_ITEMS.len() - 1);
                }
            }
            KeyCode::Enter => match selected {
                0 => {
                    // Backup
                    self.mode = AppMode::Browser(BrowserState::new_backup(false));
                    self.start_initial_command().await?;
                }
                1 => {
                    // Restore
                    self.mode = AppMode::Browser(BrowserState::new_restore(false));
                    self.start_initial_command().await?;
                }
                2 => {
                    // Status
                    self.mode = AppMode::Browser(BrowserState::new_status());
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
                    *selected = (*selected + 1).min(HOSTS.len() - 1);
                }
            }
            KeyCode::Enter => {
                let host = HOSTS[selected].0.to_string();
                self.mode = AppMode::Install(InstallState::SelectDisk {
                    host,
                    disks: Vec::new(),
                    selected: 0,
                });
                self.start_initial_command().await?;
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

    async fn handle_back(&mut self) -> Result<()> {
        let needs_disk_refresh = matches!(
            self.mode,
            AppMode::Install(InstallState::Confirm { .. })
        );

        self.mode = match &self.mode {
            AppMode::Browser(BrowserState::Menu { .. }) => AppMode::MainMenu { selected: 2 },
            AppMode::Browser(BrowserState::Complete { .. })
            | AppMode::Browser(BrowserState::Status { .. }) => {
                AppMode::Browser(BrowserState::Menu { selected: 0 })
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
            _ => return Ok(()),
        };

        // Repopulate disk list if needed
        if needs_disk_refresh {
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
            _ => {}
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
            AppMode::Browser(BrowserState::Running { output, .. }) => {
                output.push_back(clean_line.clone());
                while output.len() > OUTPUT_BUFFER_SIZE {
                    output.pop_front();
                }
            }
            AppMode::Browser(BrowserState::Status { output }) => {
                output.push_back(clean_line);
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
            AppMode::Browser(BrowserState::Running { output, .. }) => {
                self.mode = AppMode::Browser(BrowserState::Complete {
                    success,
                    output: output.clone(),
                });
            }
            AppMode::Install(InstallState::Running { output, .. }) => {
                self.mode = AppMode::Install(InstallState::Complete {
                    success,
                    output: output.clone(),
                });
            }
            AppMode::Update(UpdateState::Running { steps, output, .. }) => {
                self.mode = AppMode::Update(UpdateState::Complete {
                    success,
                    steps: steps.clone(),
                    output: output.clone(),
                });
            }
            _ => {}
        }
    }
}
