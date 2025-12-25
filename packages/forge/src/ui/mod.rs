//! UI rendering module

mod layout;
mod screens;
pub mod theme;
pub mod widgets;

use ratatui::Frame;

use crate::app::{App, AppMode, BrowserState, CreateHostState, InstallState, UpdateState};

/// Main draw function - dispatches to appropriate screen
pub fn draw(frame: &mut Frame, app: &App) {
    match &app.mode {
        AppMode::MainMenu { selected } => {
            screens::main_menu::draw(frame, *selected, app);
        }
        AppMode::Install(state) => match state {
            InstallState::SelectHost { selected } => {
                screens::install::draw_host_selection(frame, *selected, &app.hosts, app);
            }
            InstallState::SelectDisk {
                host,
                disks,
                selected,
            } => {
                screens::install::draw_disk_selection(frame, host, disks, *selected, app);
            }
            InstallState::Confirm { host, disk, input } => {
                screens::install::draw_confirm(frame, host, disk, input, app);
            }
            InstallState::Running {
                host,
                disk,
                steps,
                output,
                ..
            } => {
                // Convert VecDeque to Vec for UI rendering
                let output_vec: Vec<String> = output.iter().cloned().collect();
                screens::install::draw_running(frame, host, disk, steps, &output_vec, app);
            }
            InstallState::Complete { success, output } => {
                let output_vec: Vec<String> = output.iter().cloned().collect();
                screens::install::draw_complete(frame, *success, &output_vec, app);
            }
        },
        AppMode::Update(state) => match state {
            UpdateState::Running {
                steps, output, ..
            } => {
                let output_vec: Vec<String> = output.iter().cloned().collect();
                screens::update::draw_running(frame, steps, &output_vec, false, app);
            }
            UpdateState::Complete {
                steps,
                output,
                success: _,
            } => {
                let output_vec: Vec<String> = output.iter().cloned().collect();
                screens::update::draw_running(frame, steps, &output_vec, true, app);
            }
        },
        AppMode::Browser(state) => match state {
            BrowserState::Menu { selected } => {
                screens::browser::draw_menu(frame, *selected, app);
            }
            BrowserState::Running {
                operation, output, ..
            } => {
                let output_vec: Vec<String> = output.iter().cloned().collect();
                screens::browser::draw_running(frame, operation, &output_vec, app);
            }
            BrowserState::Status { output } => {
                let output_vec: Vec<String> = output.iter().cloned().collect();
                screens::browser::draw_status(frame, &output_vec, app);
            }
            BrowserState::Complete { success, output } => {
                let output_vec: Vec<String> = output.iter().cloned().collect();
                screens::browser::draw_complete(frame, *success, &output_vec, app);
            }
        },
        AppMode::CreateHost(state) => match state {
            CreateHostState::DetectingHardware => {
                screens::create_host::draw_detecting_hardware(frame, app);
            }
            CreateHostState::ConfirmCpu {
                cpu,
                override_menu,
                selected,
                ..
            } => {
                screens::create_host::draw_confirm_cpu(frame, cpu, *override_menu, *selected, app);
            }
            CreateHostState::ConfirmGpu {
                cpu,
                gpu,
                override_menu,
                selected,
                ..
            } => {
                screens::create_host::draw_confirm_gpu(frame, cpu, gpu, *override_menu, *selected, app);
            }
            CreateHostState::ConfirmFormFactor {
                cpu,
                gpu,
                form_factor,
                override_menu,
                selected,
            } => {
                screens::create_host::draw_confirm_form_factor(
                    frame, cpu, gpu, form_factor, *override_menu, *selected, app,
                );
            }
            CreateHostState::SelectDisk {
                cpu,
                gpu,
                form_factor,
                disks,
                selected,
            } => {
                screens::create_host::draw_select_disk(
                    frame, cpu, gpu, form_factor, disks, *selected, app,
                );
            }
            CreateHostState::EnterHostname {
                cpu,
                gpu,
                form_factor,
                disk,
                input,
                error,
            } => {
                screens::create_host::draw_enter_hostname(
                    frame, cpu, gpu, form_factor, disk, input, error.as_deref(), app,
                );
            }
            CreateHostState::Review { config } => {
                screens::create_host::draw_review(frame, config, app);
            }
            CreateHostState::Generating {
                config,
                steps,
                output,
                ..
            } => {
                let output_vec: Vec<String> = output.iter().cloned().collect();
                screens::create_host::draw_generating(frame, config, steps, &output_vec, app);
            }
            CreateHostState::Complete {
                success,
                hostname,
                disk,
                ..
            } => {
                screens::create_host::draw_complete(frame, *success, hostname, disk, app);
            }
        },
        AppMode::Quit => {}
    }
}
