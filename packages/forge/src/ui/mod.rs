//! UI rendering module

mod layout;
mod screens;
pub mod theme;
pub mod widgets;

use ratatui::Frame;

use crate::app::{App, AppMode, BrowserState, InstallState, UpdateState};

/// Main draw function - dispatches to appropriate screen
pub fn draw(frame: &mut Frame, app: &App) {
    match &app.mode {
        AppMode::MainMenu { selected } => {
            screens::main_menu::draw(frame, *selected, app);
        }
        AppMode::Install(state) => match state {
            InstallState::SelectHost { selected } => {
                screens::install::draw_host_selection(frame, *selected, app);
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
                success,
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
        AppMode::Quit => {}
    }
}
