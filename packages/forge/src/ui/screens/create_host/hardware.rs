//! Hardware detection and confirmation screens

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::helpers::{draw_footer, draw_header};
use crate::app::App;
use crate::system::hardware::{CpuInfo, FormFactor, GpuInfo};
use crate::ui::layout::centered_rect;
use crate::ui::theme;
use crate::ui::widgets::MenuList;

/// Draw detecting hardware screen (spinner) - entry point for new host wizard
pub fn draw_detecting_hardware(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let center = centered_rect(50, 30, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(5)])
        .split(center);

    draw_header(frame, chunks[0], "New Host Configuration");

    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spin_char = spinner[app.spinner_state % spinner.len()];

    let detecting = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(spin_char, theme::info()),
            Span::styled(" Detecting hardware...", theme::text()),
        ]),
        Line::from(""),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border()),
    );
    frame.render_widget(detecting, chunks[1]);
}

/// Draw CPU confirmation screen
pub fn draw_confirm_cpu(
    frame: &mut Frame,
    cpu: &CpuInfo,
    override_menu: bool,
    selected: usize,
    _app: &App,
) {
    let area = frame.area();
    let center = centered_rect(65, 60, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(7),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(center);

    draw_header(frame, chunks[0], "Confirm CPU");

    // Detected CPU info
    let cpu_info = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled("Detected CPU:", theme::dim())),
        Line::from(vec![
            Span::styled("  Vendor: ", theme::dim()),
            Span::styled(format!("{}", cpu.vendor), theme::info()),
        ]),
        Line::from(vec![
            Span::styled("  Model:  ", theme::dim()),
            Span::styled(&cpu.model_name, theme::text()),
        ]),
        Line::from(""),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" CPU Detection ", theme::title())),
    );
    frame.render_widget(cpu_info, chunks[1]);

    if override_menu {
        // Show selection menu
        let items = vec!["AMD", "Intel"];
        let menu = MenuList::new(items, selected).title(" Select CPU Vendor ");
        frame.render_widget(menu, chunks[2]);
        draw_footer(frame, chunks[3], &["↑↓ Navigate", "Enter Select", "Esc Back"]);
    } else {
        // Show confirmation prompt
        let confirm = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("Is this correct?", theme::text())),
            Line::from(""),
            Line::from(vec![
                Span::styled("[", theme::dim()),
                Span::styled("Y", theme::key_hint()),
                Span::styled("]es  [", theme::dim()),
                Span::styled("N", theme::key_hint()),
                Span::styled("]o, let me choose", theme::dim()),
            ]),
            Line::from(""),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border()),
        );
        frame.render_widget(confirm, chunks[2]);
        draw_footer(frame, chunks[3], &["y Confirm", "n Override", "Esc Back"]);
    }
}

/// Draw GPU confirmation screen
pub fn draw_confirm_gpu(
    frame: &mut Frame,
    cpu: &CpuInfo,
    gpu: &GpuInfo,
    override_menu: bool,
    selected: usize,
    _app: &App,
) {
    let area = frame.area();
    let center = centered_rect(65, 65, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(center);

    draw_header(frame, chunks[0], "Confirm GPU");

    // Detected GPU info - show different message if detection failed
    let (title, lines) = if override_menu && gpu.model.is_none() {
        // Detection failed - show selection prompt
        (
            " GPU Selection ",
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "GPU could not be auto-detected.",
                    theme::warning(),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Please select your GPU vendor below:",
                    theme::text(),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  CPU:    ", theme::dim()),
                    Span::styled(format!("{}", cpu.vendor), theme::dim()),
                ]),
            ],
        )
    } else {
        // Detection succeeded
        let model_str = gpu.model.as_deref().unwrap_or("Unknown");
        (
            " GPU Detection ",
            vec![
                Line::from(""),
                Line::from(Span::styled("Detected GPU:", theme::dim())),
                Line::from(vec![
                    Span::styled("  Vendor: ", theme::dim()),
                    Span::styled(format!("{}", gpu.vendor), theme::info()),
                ]),
                Line::from(vec![
                    Span::styled("  Model:  ", theme::dim()),
                    Span::styled(model_str, theme::text()),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  CPU:    ", theme::dim()),
                    Span::styled(format!("{}", cpu.vendor), theme::dim()),
                ]),
            ],
        )
    };

    let gpu_info = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(title, theme::title())),
    );
    frame.render_widget(gpu_info, chunks[1]);

    if override_menu {
        let items = vec!["NVIDIA", "AMD", "Intel", "None (integrated/software)"];
        let menu = MenuList::new(items, selected).title(" Select GPU Vendor ");
        frame.render_widget(menu, chunks[2]);
        draw_footer(frame, chunks[3], &["↑↓ Navigate", "Enter Select", "Esc Back"]);
    } else {
        let confirm = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("Is this correct?", theme::text())),
            Line::from(""),
            Line::from(vec![
                Span::styled("[", theme::dim()),
                Span::styled("Y", theme::key_hint()),
                Span::styled("]es  [", theme::dim()),
                Span::styled("N", theme::key_hint()),
                Span::styled("]o, let me choose", theme::dim()),
            ]),
            Line::from(""),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border()),
        );
        frame.render_widget(confirm, chunks[2]);
        draw_footer(frame, chunks[3], &["y Confirm", "n Override", "Esc Back"]);
    }
}

/// Draw form factor confirmation screen
pub fn draw_confirm_form_factor(
    frame: &mut Frame,
    cpu: &CpuInfo,
    gpu: &GpuInfo,
    form_factor: &FormFactor,
    override_menu: bool,
    selected: usize,
    _app: &App,
) {
    let area = frame.area();
    let center = centered_rect(65, 65, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(9),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(center);

    draw_header(frame, chunks[0], "Confirm Form Factor");

    // Summary and form factor
    let ff_str = match form_factor {
        FormFactor::Laptop => "Laptop",
        FormFactor::Desktop => "Desktop",
    };
    let ff_hint = match form_factor {
        FormFactor::Laptop => "(battery detected)",
        FormFactor::Desktop => "(no battery detected)",
    };

    let info = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled("Detected Form Factor:", theme::dim())),
        Line::from(vec![
            Span::styled("  Type: ", theme::dim()),
            Span::styled(ff_str, theme::info()),
            Span::styled(format!(" {}", ff_hint), theme::dim()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  CPU: ", theme::dim()),
            Span::styled(format!("{}", cpu.vendor), theme::dim()),
            Span::styled(" | GPU: ", theme::dim()),
            Span::styled(format!("{}", gpu.vendor), theme::dim()),
        ]),
        Line::from(""),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Form Factor ", theme::title())),
    );
    frame.render_widget(info, chunks[1]);

    if override_menu {
        let items = vec!["Desktop", "Laptop"];
        let menu = MenuList::new(items, selected).title(" Select Form Factor ");
        frame.render_widget(menu, chunks[2]);
        draw_footer(frame, chunks[3], &["↑↓ Navigate", "Enter Select", "Esc Back"]);
    } else {
        let hint = match form_factor {
            FormFactor::Laptop => "Laptops use TLP for battery optimization",
            FormFactor::Desktop => "Desktops use power-profiles-daemon",
        };
        let confirm = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(hint, theme::dim())),
            Line::from(""),
            Line::from(Span::styled("Is this correct?", theme::text())),
            Line::from(""),
            Line::from(vec![
                Span::styled("[", theme::dim()),
                Span::styled("Y", theme::key_hint()),
                Span::styled("]es  [", theme::dim()),
                Span::styled("N", theme::key_hint()),
                Span::styled("]o, let me choose", theme::dim()),
            ]),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border()),
        );
        frame.render_widget(confirm, chunks[2]);
        draw_footer(frame, chunks[3], &["y Confirm", "n Override", "Esc Back"]);
    }
}
