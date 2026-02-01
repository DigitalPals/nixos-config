//! Context-sensitive help overlay

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::{AppMode, AppProfileState, CreateHostState, InstallState, KeybindingsState, KeysState, UpdateState};
use crate::ui::theme;

/// Draw the help overlay modal centered on screen
pub fn draw_help(frame: &mut Frame, mode: &AppMode) {
    let area = frame.area();

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(""));

    let title = match mode {
        AppMode::MainMenu { .. } => {
            add_section(&mut lines, "Navigation", &[
                ("↑ / k", "Move selection up"),
                ("↓ / j", "Move selection down"),
                ("Enter", "Select menu item"),
                ("q", "Quit Forge"),
                ("Esc", "Quit Forge"),
            ]);
            lines.push(Line::from(""));
            add_section(&mut lines, "Menu Items", &[
                ("Install NixOS", "Fresh install from Live ISO"),
                ("Update system", "Pull, rebuild, update tools"),
                ("App profiles", "Backup/restore browser data"),
                ("Keybindings", "View desktop keybindings"),
                ("Exit", "Close Forge"),
            ]);
            " Help - Main Menu "
        }
        AppMode::Install(InstallState::SelectHost { .. }) => {
            add_section(&mut lines, "Keys", &[
                ("↑ / k", "Move selection up"),
                ("↓ / j", "Move selection down"),
                ("Enter", "Select host"),
                ("Esc", "Back to main menu"),
            ]);
            " Help - Host Selection "
        }
        AppMode::Install(InstallState::SelectDisk { .. }) => {
            add_section(&mut lines, "Keys", &[
                ("↑ / k", "Move selection up"),
                ("↓ / j", "Move selection down"),
                ("Enter", "Select disk"),
                ("Esc", "Back"),
            ]);
            " Help - Disk Selection "
        }
        AppMode::Install(InstallState::EnterCredentials { .. }) => {
            add_section(&mut lines, "Keys", &[
                ("Tab / ↓", "Next field"),
                ("Shift+Tab / ↑", "Previous field"),
                ("Enter", "Continue (validates all fields)"),
                ("Esc", "Back"),
            ]);
            " Help - Credentials "
        }
        AppMode::Install(InstallState::Running { .. })
        | AppMode::Update(UpdateState::Running { .. })
        | AppMode::Apps(AppProfileState::Running { .. })
        | AppMode::Keys(KeysState::Running { .. }) => {
            add_section(&mut lines, "Keys", &[
                ("Ctrl+C", "Cancel the operation"),
            ]);
            " Help - Running "
        }
        AppMode::Install(InstallState::Complete { .. })
        | AppMode::Update(UpdateState::Complete { .. })
        | AppMode::Apps(AppProfileState::Complete { .. })
        | AppMode::Keys(KeysState::Complete { .. }) => {
            add_section(&mut lines, "Keys", &[
                ("↑ / k", "Scroll up"),
                ("↓ / j", "Scroll down"),
                ("Enter", "Return to menu"),
                ("Esc", "Go back"),
                ("q", "Quit Forge"),
            ]);
            " Help - Complete "
        }
        AppMode::Apps(AppProfileState::Menu { .. }) => {
            add_section(&mut lines, "Keys", &[
                ("↑ / k", "Move selection up"),
                ("↓ / j", "Move selection down"),
                ("Enter", "Select option"),
                ("Esc", "Back to main menu"),
                ("q", "Quit Forge"),
            ]);
            " Help - App Profiles "
        }
        AppMode::Update(UpdateState::ShowingSummary { .. }) => {
            add_section(&mut lines, "Keys", &[
                ("↑ / k", "Scroll up"),
                ("↓ / j", "Scroll down"),
                ("Enter / v", "View full log"),
                ("r", "Reboot (if recommended)"),
                ("q", "Quit Forge"),
            ]);
            " Help - Update Summary "
        }
        AppMode::Keybindings(KeybindingsState::Viewing { .. }) => {
            add_section(&mut lines, "Keys", &[
                ("Tab", "Switch between panels"),
                ("↑ / k", "Navigate up"),
                ("↓ / j", "Navigate down"),
                ("Esc", "Back to main menu"),
                ("q", "Quit Forge"),
            ]);
            " Help - Keybindings Viewer "
        }
        AppMode::CreateHost(_) => {
            add_section(&mut lines, "Keys", &[
                ("↑ / k", "Move selection up"),
                ("↓ / j", "Move selection down"),
                ("Enter / y", "Confirm / select"),
                ("n", "Override detected value"),
                ("Esc", "Back"),
            ]);
            " Help - Create Host "
        }
        _ => {
            add_section(&mut lines, "Keys", &[
                ("Esc", "Go back"),
                ("q", "Quit Forge"),
            ]);
            " Help "
        }
    };

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press Esc, Enter, or ? to close",
        theme::dim(),
    )));

    let content_height = lines.len() as u16 + 2; // +2 for borders
    let modal_width = 50u16.min(area.width.saturating_sub(4));
    let modal_height = content_height.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(Span::styled(title, theme::title()))
        .borders(Borders::ALL)
        .border_style(theme::border_active());

    let content = Paragraph::new(lines).block(block);
    frame.render_widget(content, modal_area);
}

fn add_section(lines: &mut Vec<Line<'static>>, title: &str, items: &[(&str, &str)]) {
    lines.push(Line::from(Span::styled(
        format!("  {}", title),
        theme::title(),
    )));
    lines.push(Line::from(""));
    for (key, desc) in items {
        lines.push(Line::from(vec![
            Span::styled(format!("    {:20}", key), theme::key_hint()),
            Span::styled(desc.to_string(), theme::text()),
        ]));
    }
}
