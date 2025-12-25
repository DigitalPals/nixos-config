//! Create host wizard screens

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

use crate::app::{App, NewHostConfig, StepStatus};
use crate::system::disk::DiskInfo;
use crate::system::hardware::{CpuInfo, FormFactor, GpuInfo};
use crate::ui::layout::{centered_rect, progress_layout};
use crate::ui::theme;
use crate::ui::widgets::{LogView, MenuList, ProgressSteps};

/// Draw hostname entry screen (comes after disk selection)
pub fn draw_enter_hostname(
    frame: &mut Frame,
    cpu: &CpuInfo,
    gpu: &GpuInfo,
    form_factor: &FormFactor,
    disk: &DiskInfo,
    input: &str,
    error: Option<&str>,
    _app: &App,
) {
    let area = frame.area();
    let center = centered_rect(65, 65, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Length(3),
            Constraint::Min(3),
        ])
        .split(center);

    // Header
    draw_header(frame, chunks[0], "Enter Hostname");

    // Hardware summary
    let disk_model = disk.model.as_deref().unwrap_or("Unknown");
    let summary = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("  CPU: ", theme::dim()),
            Span::styled(format!("{}", cpu.vendor), theme::text()),
            Span::styled(" | GPU: ", theme::dim()),
            Span::styled(format!("{}", gpu.vendor), theme::text()),
            Span::styled(" | Form: ", theme::dim()),
            Span::styled(format!("{}", form_factor), theme::text()),
        ]),
        Line::from(vec![
            Span::styled("  Disk: ", theme::dim()),
            Span::styled(&disk.path, theme::text()),
            Span::styled(format!(" ({}, {})", disk.size, disk_model), theme::dim()),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Configuration ", theme::title())),
    );
    frame.render_widget(summary, chunks[1]);

    // Instructions and input
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Enter a hostname for this configuration:",
            theme::text(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", theme::info()),
            Span::styled(input, theme::text()),
            Span::styled("_", theme::info()),
        ]),
    ];

    if let Some(err) = error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(err, theme::error())));
    }

    let input_block = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Hostname ", theme::title())),
    );
    frame.render_widget(input_block, chunks[2]);

    // Hint
    let hint = Paragraph::new(Line::from(Span::styled(
        "Use lowercase letters, numbers, and hyphens only",
        theme::dim(),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[3]);

    // Footer
    draw_footer(frame, chunks[4], &["Enter Continue", "Esc Back"]);
}

/// Draw detecting hardware screen (spinner) - entry point for new host wizard
pub fn draw_detecting_hardware(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let center = centered_rect(50, 30, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(5),
        ])
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

/// Draw disk selection screen with partition tree view
pub fn draw_select_disk(
    frame: &mut Frame,
    cpu: &CpuInfo,
    gpu: &GpuInfo,
    form_factor: &FormFactor,
    disks: &[DiskInfo],
    selected: usize,
    _app: &App,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(4),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(centered_rect(80, 85, area));

    draw_header(frame, chunks[0], "Select Target Disk");

    // Hardware summary
    let summary = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" CPU: ", theme::dim()),
            Span::styled(format!("{}", cpu.vendor), theme::text()),
            Span::styled(" | GPU: ", theme::dim()),
            Span::styled(format!("{}", gpu.vendor), theme::text()),
            Span::styled(" | Form: ", theme::dim()),
            Span::styled(format!("{}", form_factor), theme::text()),
        ]),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border()),
    );
    frame.render_widget(summary, chunks[1]);

    if disks.is_empty() {
        let message = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("No disks found!", theme::warning())),
            Line::from(""),
            Line::from(Span::styled(
                "Please check that disks are properly connected.",
                theme::dim(),
            )),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::warning())
                .title(Span::styled(" Available Disks ", theme::title())),
        );
        frame.render_widget(message, chunks[2]);
        draw_footer(frame, chunks[3], &["Esc Back"]);
        return;
    }

    // Build rows with disks and their partitions
    let mut rows: Vec<Row> = Vec::new();

    for (i, disk) in disks.iter().enumerate() {
        let prefix = if i == selected { ">" } else { " " };
        let style = if i == selected {
            theme::selected()
        } else {
            theme::text()
        };

        // Disk row
        rows.push(
            Row::new(vec![
                prefix.to_string(),
                disk.path.clone(),
                disk.size.clone(),
                disk.model.clone().unwrap_or_default(),
            ])
            .style(style),
        );

        // Partition rows (indented tree view)
        let part_count = disk.partitions.len();
        for (j, part) in disk.partitions.iter().enumerate() {
            let is_last = j == part_count - 1;
            let tree_char = if is_last { "└─" } else { "├─" };

            // Format: short device name, size, fstype (OS)
            let device_short = part.path.replace("/dev/", "");
            let os_str = part
                .os_type
                .as_ref()
                .map(|os| format!(" ({})", os))
                .unwrap_or_default();

            rows.push(
                Row::new(vec![
                    "".to_string(),
                    format!("   {} {}", tree_char, device_short),
                    part.size.clone(),
                    format!("{}{}", part.fstype, os_str),
                ])
                .style(theme::dim()),
            );
        }
    }

    // Disk table with partitions
    let header = Row::new(vec!["", "Device", "Size", "Type/Model"])
        .style(theme::title())
        .bottom_margin(1);

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(20),
            Constraint::Length(10),
            Constraint::Min(25),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Select Target Disk ", theme::title())),
    );
    frame.render_widget(table, chunks[2]);

    draw_footer(frame, chunks[3], &["↑↓ Navigate", "Enter Select", "Esc Back"]);
}

/// Draw review screen
pub fn draw_review(frame: &mut Frame, config: &NewHostConfig, _app: &App) {
    let area = frame.area();
    let center = centered_rect(70, 70, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(12),
            Constraint::Length(5),
            Constraint::Length(3),
        ])
        .split(center);

    draw_header(frame, chunks[0], "Review Configuration");

    // Configuration summary
    let gpu_model = config.gpu.model.as_deref().unwrap_or("N/A");
    let disk_model = config.disk.model.as_deref().unwrap_or("Unknown");

    let summary = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Hostname:    ", theme::dim()),
            Span::styled(&config.hostname, theme::info()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  CPU:         ", theme::dim()),
            Span::styled(format!("{}", config.cpu.vendor), theme::text()),
            Span::styled(format!(" ({})", config.cpu.model_name), theme::dim()),
        ]),
        Line::from(vec![
            Span::styled("  GPU:         ", theme::dim()),
            Span::styled(format!("{}", config.gpu.vendor), theme::text()),
            Span::styled(format!(" ({})", gpu_model), theme::dim()),
        ]),
        Line::from(vec![
            Span::styled("  Form Factor: ", theme::dim()),
            Span::styled(format!("{}", config.form_factor), theme::text()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Disk:        ", theme::dim()),
            Span::styled(&config.disk.path, theme::text()),
            Span::styled(format!(" ({}, {})", config.disk.size, disk_model), theme::dim()),
        ]),
        Line::from(""),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Configuration Summary ", theme::title())),
    );
    frame.render_widget(summary, chunks[1]);

    // Files to be created
    let files = Paragraph::new(vec![
        Line::from(Span::styled("Files to be created:", theme::dim())),
        Line::from(vec![
            Span::styled("  • ", theme::info()),
            Span::styled(format!("hosts/{}/default.nix", config.hostname), theme::text()),
        ]),
        Line::from(vec![
            Span::styled("  • ", theme::info()),
            Span::styled(format!("hosts/{}/hardware-configuration.nix", config.hostname), theme::text()),
        ]),
        Line::from(vec![
            Span::styled("  • ", theme::info()),
            Span::styled(format!("modules/disko/{}.nix", config.hostname), theme::text()),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border()),
    );
    frame.render_widget(files, chunks[2]);

    draw_footer(frame, chunks[3], &["Enter Create", "Esc Back"]);
}

/// Draw generating screen
pub fn draw_generating(
    frame: &mut Frame,
    config: &NewHostConfig,
    steps: &[StepStatus],
    output: &[String],
    app: &App,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" Creating Host Configuration | ", theme::title()),
        Span::styled("Host: ", theme::dim()),
        Span::styled(&config.hostname, theme::text()),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_active()),
    );
    frame.render_widget(header, chunks[0]);

    // Progress and output
    let (steps_area, output_area) = progress_layout(chunks[1]);

    let progress = ProgressSteps::new(steps, app.spinner_state).title(" Progress ");
    frame.render_widget(progress, steps_area);

    let log = LogView::new(output).title(" Output ");
    frame.render_widget(log, output_area);

    // Footer
    let footer = Paragraph::new(Line::from(Span::styled(
        "Creating configuration files...",
        theme::dim(),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

/// Draw completion screen
pub fn draw_complete(
    frame: &mut Frame,
    success: bool,
    hostname: &str,
    _disk: &DiskInfo,
    _app: &App,
) {
    let area = frame.area();
    let center = centered_rect(60, 50, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(center);

    // Header
    let (title, style) = if success {
        (" ✓ Host Created Successfully ", theme::success())
    } else {
        (" ✗ Host Creation Failed ", theme::error())
    };
    let header = Paragraph::new(Line::from(Span::styled(title, style)))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(style),
        );
    frame.render_widget(header, chunks[0]);

    if success {
        // Success message with install prompt
        let message = Paragraph::new(vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("Host '", theme::text()),
                Span::styled(hostname, theme::info()),
                Span::styled("' has been created.", theme::text()),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Would you like to proceed with installation?",
                theme::text(),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("[", theme::dim()),
                Span::styled("Y", theme::key_hint()),
                Span::styled("]es, install now  [", theme::dim()),
                Span::styled("N", theme::key_hint()),
                Span::styled("]o, return to menu", theme::dim()),
            ]),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border()),
        );
        frame.render_widget(message, chunks[1]);
        draw_footer(frame, chunks[2], &["y Install", "n Menu", "q Quit"]);
    } else {
        // Failure message
        let message = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Failed to create host configuration.",
                theme::error(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Check the output for error details.",
                theme::dim(),
            )),
            Line::from(""),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::error()),
        );
        frame.render_widget(message, chunks[1]);
        draw_footer(frame, chunks[2], &["Enter Menu", "q Quit"]);
    }
}

fn draw_header(frame: &mut Frame, area: Rect, title: &str) {
    let header = Paragraph::new(Line::from(Span::styled(title, theme::title())))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border_active()),
        );
    frame.render_widget(header, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, hints: &[&str]) {
    let spans: Vec<Span> = hints
        .iter()
        .enumerate()
        .flat_map(|(i, hint)| {
            let mut v = vec![];
            if i > 0 {
                v.push(Span::styled("  ", theme::dim()));
            }
            v.push(Span::styled("[", theme::dim()));
            let parts: Vec<&str> = hint.splitn(2, ' ').collect();
            if parts.len() == 2 {
                v.push(Span::styled(parts[0], theme::key_hint()));
                v.push(Span::styled(format!("] {}", parts[1]), theme::dim()));
            } else {
                v.push(Span::styled(*hint, theme::key_hint()));
                v.push(Span::styled("]", theme::dim()));
            }
            v
        })
        .collect();

    let footer = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(footer, area);
}
