//! Keyboard input handlers for the application

use anyhow::Result;
use crossterm::event::KeyCode;
use std::mem;

use super::state::*;
use super::App;
use crate::commands;
use crate::constants::MAX_INPUT_LENGTH;
use crate::system::hardware::{CpuInfo, CpuVendor, FormFactor, GpuInfo, GpuVendor};

impl App {
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

        // Handle commit list view
        if self.pending_updates.viewing_commits {
            match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.pending_updates.commit_scroll > 0 {
                        self.pending_updates.commit_scroll -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = self.pending_updates.commits.len().saturating_sub(1);
                    if self.pending_updates.commit_scroll < max {
                        self.pending_updates.commit_scroll += 1;
                    }
                }
                KeyCode::Enter => {
                    // Run system update
                    self.pending_updates.clear();
                    self.mode = AppMode::Update(UpdateState::new());
                    self.start_initial_command().await?;
                }
                KeyCode::Esc | KeyCode::Backspace => {
                    // Go back to main dialog
                    self.pending_updates.viewing_commits = false;
                    self.pending_updates.commit_scroll = 0;
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle update dialog
        if self.pending_updates.has_updates() {
            match key {
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.pending_updates.selected > 0 {
                        self.pending_updates.selected -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let max = get_update_dialog_option_count(&self.pending_updates) - 1;
                    if self.pending_updates.selected < max {
                        self.pending_updates.selected += 1;
                    }
                }
                KeyCode::Enter => {
                    self.handle_update_dialog_select().await?;
                }
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.pending_updates.clear();
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
            AppMode::Install(InstallState::EnterCredentials { host, .. }) => {
                Some(("install_credentials", 0, Some(host.clone()), None))
            }
            AppMode::Install(InstallState::Confirm { host, disk: _, .. }) => {
                Some(("install_confirm", 0, Some(host.clone()), None))
            }
            AppMode::Install(InstallState::Complete { .. })
            | AppMode::Update(UpdateState::Complete { .. })
            | AppMode::Apps(AppProfileState::Complete { .. })
            | AppMode::Keys(KeysState::Complete { .. }) => match key {
                KeyCode::Enter => Some(("complete", 0, None, None)),
                KeyCode::Up | KeyCode::Down => Some(("scroll", 0, None, None)),
                _ => None,
            },
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
            Some(("install_credentials", _, Some(host), _)) => {
                self.handle_credentials_key(key, &host).await?;
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

    /// Handle selection in the update dialog
    pub(super) async fn handle_update_dialog_select(&mut self) -> Result<()> {
        let has_nixos = self.pending_updates.nixos_config;
        let has_apps = self.pending_updates.app_profiles;
        let both = has_nixos && has_apps;
        let selected = self.pending_updates.selected;

        let mut idx = 0;

        // Check if View NixOS updates was selected
        if has_nixos {
            if selected == idx {
                self.pending_updates.viewing_commits = true;
                self.pending_updates.commit_scroll = 0;
                return Ok(());
            }
            idx += 1;
        }

        // Check if Update app profiles was selected
        if has_apps {
            if selected == idx {
                self.pending_updates.clear();
                self.mode = AppMode::Apps(AppProfileState::new_restore(false));
                self.start_initial_command().await?;
                return Ok(());
            }
            idx += 1;
        }

        // Check if Update all was selected
        if both && selected == idx {
            self.pending_updates.clear();
            self.mode = AppMode::Update(UpdateState::new());
            self.start_initial_command().await?;
            return Ok(());
        }

        // Dismiss selected (or fallback)
        self.pending_updates.clear();
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
                // Install - check if running from Live ISO
                if !crate::system::is_live_iso_environment() {
                    use std::collections::VecDeque;
                    let mut output = VecDeque::new();
                    output.push_back(
                        "Error: Install can only be run from a NixOS Live ISO.".to_string(),
                    );
                    output.push_back("".to_string());
                    output.push_back(
                        "You appear to be running on an installed system.".to_string(),
                    );
                    output.push_back("".to_string());
                    output.push_back("To install NixOS:".to_string());
                    output.push_back("  1. Boot from a NixOS minimal ISO".to_string());
                    output.push_back("  2. Connect to WiFi: nmtui".to_string());
                    output.push_back("  3. Run: nix run github:DigitalPals/nixos-config".to_string());
                    output.push_back("  4. Select 'Install NixOS' from the menu".to_string());
                    self.mode = AppMode::Install(InstallState::Complete {
                        success: false,
                        output,
                        scroll_offset: 0,
                    });
                } else {
                    self.mode = AppMode::Install(InstallState::SelectHost { selected: 0 });
                }
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
        disks: &[crate::system::disk::DiskInfo],
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
                    if !disks.is_empty() {
                        *selected = (*selected + 1).min(disks.len() - 1);
                    }
                }
            }
            KeyCode::Enter => {
                if !disks.is_empty() {
                    self.mode = AppMode::Install(InstallState::EnterCredentials {
                        host: host.to_string(),
                        disk: disks[selected].clone(),
                        credentials: InstallCredentials::default(),
                        active_field: CredentialField::Username,
                        error: None,
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_credentials_key(&mut self, key: KeyCode, _host: &str) -> Result<()> {
        if let AppMode::Install(InstallState::EnterCredentials {
            host,
            disk,
            credentials,
            active_field,
            error,
        }) = &mut self.mode
        {
            match key {
                KeyCode::Tab | KeyCode::Down => {
                    // Move to next field
                    *active_field = match active_field {
                        CredentialField::Username => CredentialField::Password,
                        CredentialField::Password => CredentialField::ConfirmPassword,
                        CredentialField::ConfirmPassword => CredentialField::Username,
                    };
                    *error = None;
                }
                KeyCode::BackTab | KeyCode::Up => {
                    // Move to previous field
                    *active_field = match active_field {
                        CredentialField::Username => CredentialField::ConfirmPassword,
                        CredentialField::Password => CredentialField::Username,
                        CredentialField::ConfirmPassword => CredentialField::Password,
                    };
                    *error = None;
                }
                KeyCode::Char(c) => {
                    let field = match active_field {
                        CredentialField::Username => &mut credentials.username,
                        CredentialField::Password => &mut credentials.password,
                        CredentialField::ConfirmPassword => &mut credentials.confirm_password,
                    };
                    if field.len() < MAX_INPUT_LENGTH {
                        // Auto-convert username to lowercase
                        let c = if *active_field == CredentialField::Username {
                            c.to_ascii_lowercase()
                        } else {
                            c
                        };
                        field.push(c);
                    }
                    *error = None;
                }
                KeyCode::Backspace => {
                    let field = match active_field {
                        CredentialField::Username => &mut credentials.username,
                        CredentialField::Password => &mut credentials.password,
                        CredentialField::ConfirmPassword => &mut credentials.confirm_password,
                    };
                    field.pop();
                    *error = None;
                }
                KeyCode::Enter => {
                    // Validate and proceed to confirmation
                    if let Some(err) = validate_username(&credentials.username) {
                        *error = Some(err);
                    } else if let Some(err) = validate_password(&credentials.password, &credentials.confirm_password) {
                        *error = Some(err);
                    } else {
                        // All valid, proceed to confirmation
                        self.mode = AppMode::Install(InstallState::Confirm {
                            host: host.clone(),
                            disk: disk.clone(),
                            credentials: credentials.clone(),
                            input: String::new(),
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    async fn handle_confirm_key_action(&mut self, key: KeyCode, host: &str) -> Result<()> {
        let (disk, credentials, should_start) = if let AppMode::Install(InstallState::Confirm {
            disk,
            credentials,
            input,
            ..
        }) = &mut self.mode
        {
            match key {
                KeyCode::Char(c) => {
                    if input.len() < MAX_INPUT_LENGTH {
                        input.push(c);
                    }
                    (None, None, false)
                }
                KeyCode::Backspace => {
                    input.pop();
                    (None, None, false)
                }
                KeyCode::Enter => {
                    if input.trim().eq_ignore_ascii_case("yes") {
                        (Some(disk.clone()), Some(credentials.clone()), true)
                    } else {
                        (None, None, false)
                    }
                }
                _ => (None, None, false),
            }
        } else {
            (None, None, false)
        };

        if should_start {
            if let (Some(disk), Some(creds)) = (disk, credentials) {
                let mut steps = vec![
                    StepStatus::new("Checking network connectivity"),
                    StepStatus::new("Enabling Nix flakes"),
                    StepStatus::new("Cloning configuration repository"),
                    StepStatus::new("Configuring disk device"),
                    StepStatus::new("Running disko (partitioning)"),
                    StepStatus::new("Installing NixOS"),
                    StepStatus::new("Setting up user account"),
                ];
                steps[0].status = StepState::Running;

                self.mode = AppMode::Install(InstallState::Running {
                    host: host.to_string(),
                    disk: disk.clone(),
                    credentials: creds.clone(),
                    step: 0,
                    steps,
                    output: std::collections::VecDeque::new(),
                });
                if let Some(tx) = &self.cmd_tx {
                    commands::install::start_install(
                        tx.clone(),
                        host,
                        &disk.path,
                        &creds.username,
                        &creds.password,
                    ).await?;
                }
            }
        }
        Ok(())
    }

    /// Handle keyboard input for create host wizard
    async fn handle_create_host_key(&mut self, key: KeyCode) -> Result<()> {
        // For keys that don't transition state, handle them with mutable borrow
        let should_transition = match &mut self.mode {
            AppMode::CreateHost(CreateHostState::ConfirmCpu {
                override_menu,
                selected,
                ..
            }) => {
                if *override_menu {
                    match key {
                        KeyCode::Up | KeyCode::Char('k') => {
                            *selected = selected.saturating_sub(1);
                            false
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *selected = (*selected + 1).min(1);
                            false
                        }
                        KeyCode::Enter => true,
                        _ => false,
                    }
                } else {
                    match key {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => true,
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            *override_menu = true;
                            false
                        }
                        _ => false,
                    }
                }
            }
            AppMode::CreateHost(CreateHostState::ConfirmGpu {
                override_menu,
                selected,
                ..
            }) => {
                if *override_menu {
                    match key {
                        KeyCode::Up | KeyCode::Char('k') => {
                            *selected = selected.saturating_sub(1);
                            false
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *selected = (*selected + 1).min(3);
                            false
                        }
                        KeyCode::Enter => true,
                        _ => false,
                    }
                } else {
                    match key {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => true,
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            *override_menu = true;
                            false
                        }
                        _ => false,
                    }
                }
            }
            AppMode::CreateHost(CreateHostState::ConfirmFormFactor {
                override_menu,
                selected,
                ..
            }) => {
                if *override_menu {
                    match key {
                        KeyCode::Up | KeyCode::Char('k') => {
                            *selected = selected.saturating_sub(1);
                            false
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            *selected = (*selected + 1).min(1);
                            false
                        }
                        KeyCode::Enter => true,
                        _ => false,
                    }
                } else {
                    match key {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => true,
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            *override_menu = true;
                            false
                        }
                        _ => false,
                    }
                }
            }
            AppMode::CreateHost(CreateHostState::SelectDisk { disks, selected, .. }) => {
                match key {
                    KeyCode::Up | KeyCode::Char('k') => {
                        *selected = selected.saturating_sub(1);
                        false
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if !disks.is_empty() {
                            *selected = (*selected + 1).min(disks.len() - 1);
                        }
                        false
                    }
                    KeyCode::Enter if !disks.is_empty() => true,
                    _ => false,
                }
            }
            AppMode::CreateHost(CreateHostState::EnterHostname { input, error, .. }) => {
                match key {
                    KeyCode::Char(c) => {
                        if input.len() < MAX_INPUT_LENGTH && (c.is_alphanumeric() || c == '-') {
                            input.push(c.to_ascii_lowercase());
                            *error = None;
                        }
                        false
                    }
                    KeyCode::Backspace => {
                        input.pop();
                        *error = None;
                        false
                    }
                    KeyCode::Enter => true,
                    _ => false,
                }
            }
            AppMode::CreateHost(CreateHostState::Review { .. }) => key == KeyCode::Enter,
            AppMode::CreateHost(CreateHostState::Complete { success, .. }) => {
                if *success {
                    matches!(
                        key,
                        KeyCode::Char('y')
                            | KeyCode::Char('Y')
                            | KeyCode::Enter
                            | KeyCode::Char('n')
                            | KeyCode::Char('N')
                    )
                } else {
                    key == KeyCode::Enter
                }
            }
            _ => false,
        };

        if !should_transition {
            return Ok(());
        }

        // Take ownership for state transitions to avoid cloning
        let old_mode = mem::replace(&mut self.mode, AppMode::MainMenu { selected: 0 });
        let mut needs_initial_command = false;

        self.mode = match old_mode {
            AppMode::CreateHost(CreateHostState::ConfirmCpu {
                cpu,
                detected_gpu,
                detected_form_factor,
                override_menu,
                selected,
            }) => {
                let gpu_override = detected_gpu.vendor == GpuVendor::None;
                if override_menu {
                    let new_vendor = if selected == 0 {
                        CpuVendor::AMD
                    } else {
                        CpuVendor::Intel
                    };
                    AppMode::CreateHost(CreateHostState::ConfirmGpu {
                        cpu: CpuInfo {
                            vendor: new_vendor,
                            model_name: format!("{} (manually selected)", new_vendor),
                        },
                        gpu: detected_gpu,
                        detected_form_factor,
                        override_menu: gpu_override,
                        selected: 0,
                    })
                } else {
                    AppMode::CreateHost(CreateHostState::ConfirmGpu {
                        cpu,
                        gpu: detected_gpu,
                        detected_form_factor,
                        override_menu: gpu_override,
                        selected: 0,
                    })
                }
            }
            AppMode::CreateHost(CreateHostState::ConfirmGpu {
                cpu,
                gpu,
                detected_form_factor,
                override_menu,
                selected,
            }) => {
                if override_menu {
                    let new_vendor = match selected {
                        0 => GpuVendor::NVIDIA,
                        1 => GpuVendor::AMD,
                        2 => GpuVendor::Intel,
                        _ => GpuVendor::None,
                    };
                    AppMode::CreateHost(CreateHostState::ConfirmFormFactor {
                        cpu,
                        gpu: GpuInfo {
                            vendor: new_vendor,
                            model: Some(format!("{} (manually selected)", new_vendor)),
                        },
                        form_factor: detected_form_factor,
                        override_menu: false,
                        selected: 0,
                    })
                } else {
                    AppMode::CreateHost(CreateHostState::ConfirmFormFactor {
                        cpu,
                        gpu,
                        form_factor: detected_form_factor,
                        override_menu: false,
                        selected: 0,
                    })
                }
            }
            AppMode::CreateHost(CreateHostState::ConfirmFormFactor {
                cpu,
                gpu,
                form_factor,
                override_menu,
                selected,
            }) => {
                needs_initial_command = true;
                let ff = if override_menu {
                    if selected == 0 {
                        FormFactor::Desktop
                    } else {
                        FormFactor::Laptop
                    }
                } else {
                    form_factor
                };
                AppMode::CreateHost(CreateHostState::SelectDisk {
                    cpu,
                    gpu,
                    form_factor: ff,
                    disks: Vec::new(),
                    selected: 0,
                })
            }
            AppMode::CreateHost(CreateHostState::SelectDisk {
                cpu,
                gpu,
                form_factor,
                disks,
                selected,
            }) => {
                let disk = disks.into_iter().nth(selected).unwrap();
                AppMode::CreateHost(CreateHostState::EnterHostname {
                    cpu,
                    gpu,
                    form_factor,
                    disk,
                    input: String::new(),
                    error: None,
                })
            }
            AppMode::CreateHost(CreateHostState::EnterHostname {
                cpu,
                gpu,
                form_factor,
                disk,
                input,
                ..
            }) => {
                let hostname = input.trim().to_string();
                if let Some(err) = validate_hostname(&hostname, &self.hosts) {
                    AppMode::CreateHost(CreateHostState::EnterHostname {
                        cpu,
                        gpu,
                        form_factor,
                        disk,
                        input,
                        error: Some(err),
                    })
                } else {
                    AppMode::CreateHost(CreateHostState::Review {
                        config: NewHostConfig {
                            hostname,
                            cpu,
                            gpu,
                            form_factor,
                            disk,
                        },
                    })
                }
            }
            AppMode::CreateHost(CreateHostState::Review { config }) => {
                let mut steps = if crate::system::is_live_iso_environment() {
                    vec![
                        StepStatus::new("Cloning configuration repository"),
                        StepStatus::new("Creating host directory"),
                        StepStatus::new("Generating hardware configuration"),
                        StepStatus::new("Creating host configuration"),
                        StepStatus::new("Creating disko configuration"),
                        StepStatus::new("Updating flake.nix"),
                        StepStatus::new("Generating host metadata"),
                    ]
                } else {
                    vec![
                        StepStatus::new("Creating host directory"),
                        StepStatus::new("Generating hardware configuration"),
                        StepStatus::new("Creating host configuration"),
                        StepStatus::new("Creating disko configuration"),
                        StepStatus::new("Updating flake.nix"),
                        StepStatus::new("Generating host metadata"),
                    ]
                };
                steps[0].status = StepState::Running;

                let new_mode = AppMode::CreateHost(CreateHostState::Generating {
                    config,
                    step: 0,
                    steps,
                    output: std::collections::VecDeque::new(),
                });

                if let Some(tx) = &self.cmd_tx {
                    commands::create_host::start_create_host(tx.clone(), new_mode.clone()).await?;
                }
                new_mode
            }
            AppMode::CreateHost(CreateHostState::Complete {
                hostname,
                disk,
                success,
                ..
            }) => {
                if success {
                    match key {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                            AppMode::Install(InstallState::EnterCredentials {
                                host: hostname,
                                disk,
                                credentials: InstallCredentials::default(),
                                active_field: CredentialField::Username,
                                error: None,
                            })
                        }
                        _ => AppMode::Install(InstallState::SelectHost { selected: 0 }),
                    }
                } else {
                    AppMode::Install(InstallState::SelectHost { selected: 0 })
                }
            }
            other => other,
        };

        if needs_initial_command {
            self.start_initial_command().await?;
        }

        Ok(())
    }

    pub(super) async fn handle_back(&mut self) -> Result<()> {
        // Take ownership of the mode to avoid cloning
        let old_mode = mem::replace(&mut self.mode, AppMode::MainMenu { selected: 0 });

        let needs_disk_refresh = matches!(
            old_mode,
            AppMode::Install(InstallState::EnterCredentials { .. })
                | AppMode::Install(InstallState::Confirm { .. })
                | AppMode::CreateHost(CreateHostState::EnterHostname { .. })
        );

        self.mode = match old_mode {
            AppMode::Apps(AppProfileState::Menu { .. }) => AppMode::MainMenu { selected: 2 },
            AppMode::Apps(AppProfileState::Complete { .. })
            | AppMode::Apps(AppProfileState::Status { .. }) => {
                AppMode::Apps(AppProfileState::Menu { selected: 0 })
            }
            AppMode::Keys(KeysState::Complete { .. }) => AppMode::MainMenu { selected: 2 },
            AppMode::Install(InstallState::SelectHost { .. }) => {
                AppMode::MainMenu { selected: 0 }
            }
            AppMode::Install(InstallState::SelectDisk { .. }) => {
                AppMode::Install(InstallState::SelectHost { selected: 0 })
            }
            AppMode::Install(InstallState::EnterCredentials { host, disk, .. }) => {
                // Go back to disk selection
                AppMode::Install(InstallState::SelectDisk {
                    host,
                    disks: vec![disk], // Keep the selected disk
                    selected: 0,
                })
            }
            AppMode::Install(InstallState::Confirm { host, disk, credentials, .. }) => {
                // Go back to credentials entry, keeping the entered credentials
                AppMode::Install(InstallState::EnterCredentials {
                    host,
                    disk,
                    credentials,
                    active_field: CredentialField::Username,
                    error: None,
                })
            }
            AppMode::Install(InstallState::Complete { .. }) => AppMode::MainMenu { selected: 0 },
            AppMode::Update(UpdateState::Complete { .. }) => AppMode::MainMenu { selected: 1 },
            // CreateHost back navigation - take ownership to avoid clones
            AppMode::CreateHost(CreateHostState::DetectingHardware) => {
                AppMode::Install(InstallState::SelectHost { selected: 0 })
            }
            AppMode::CreateHost(CreateHostState::ConfirmCpu { .. }) => {
                AppMode::Install(InstallState::SelectHost { selected: 0 })
            }
            AppMode::CreateHost(CreateHostState::ConfirmGpu {
                cpu,
                gpu,
                detected_form_factor,
                ..
            }) => AppMode::CreateHost(CreateHostState::ConfirmCpu {
                cpu,
                detected_gpu: gpu,
                detected_form_factor,
                override_menu: false,
                selected: 0,
            }),
            AppMode::CreateHost(CreateHostState::ConfirmFormFactor {
                cpu,
                gpu,
                form_factor,
                ..
            }) => AppMode::CreateHost(CreateHostState::ConfirmGpu {
                cpu,
                gpu,
                detected_form_factor: form_factor,
                override_menu: false,
                selected: 0,
            }),
            AppMode::CreateHost(CreateHostState::SelectDisk {
                cpu,
                gpu,
                form_factor,
                ..
            }) => AppMode::CreateHost(CreateHostState::ConfirmFormFactor {
                cpu,
                gpu,
                form_factor,
                override_menu: false,
                selected: 0,
            }),
            AppMode::CreateHost(CreateHostState::EnterHostname {
                cpu,
                gpu,
                form_factor,
                ..
            }) => AppMode::CreateHost(CreateHostState::SelectDisk {
                cpu,
                gpu,
                form_factor,
                disks: Vec::new(),
                selected: 0,
            }),
            AppMode::CreateHost(CreateHostState::Review { config }) => {
                // Destructure to take ownership of all fields
                let NewHostConfig {
                    hostname,
                    cpu,
                    gpu,
                    form_factor,
                    disk,
                } = config;
                AppMode::CreateHost(CreateHostState::EnterHostname {
                    cpu,
                    gpu,
                    form_factor,
                    disk,
                    input: hostname,
                    error: None,
                })
            }
            AppMode::CreateHost(CreateHostState::Complete { .. }) => {
                AppMode::Install(InstallState::SelectHost { selected: 0 })
            }
            other => {
                // Restore the original mode if no match
                self.mode = other;
                return Ok(());
            }
        };

        if needs_disk_refresh {
            self.start_initial_command().await?;
        }

        Ok(())
    }
}
