//! Installation screens

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table},
    Frame,
};

use crate::app::state::validate_username;
use crate::app::{App, CredentialField, InstallCredentials, StepStatus, SwapMode};
use crate::system::config::HostConfig;
use crate::system::disk::DiskInfo;
use crate::ui::layout::{centered_rect, footer_hints, host_selection_layout, progress_layout};
use crate::ui::theme;
use crate::ui::widgets::{LogView, MenuList, ProgressSteps};

/// Draw repository cloning screen
pub fn draw_clone_repository(frame: &mut Frame, output: &[String], app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(centered_rect(70, 70, area));

    // Header with spinner
    let spinner_chars = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let spinner = spinner_chars[app.spinner_state % spinner_chars.len()];

    let header = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{} ", spinner), theme::info()),
            Span::styled("Preparing Installation", theme::title()),
        ]),
        Line::from(Span::styled(
            "Cloning configuration repository...",
            theme::dim(),
        )),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_active()),
    );
    frame.render_widget(header, chunks[0]);

    // Output log
    let log = crate::ui::widgets::LogView::new(output).title(" Output ");
    frame.render_widget(log, chunks[1]);

    // Footer
    let footer = Paragraph::new(footer_hints(&[("Ctrl+C", "Cancel")])).alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

/// Draw hostname selection screen
pub fn draw_host_selection(frame: &mut Frame, selected: usize, hosts: &[HostConfig], _app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(centered_rect(85, 85, area));

    // Header
    draw_header(frame, chunks[0], "Select Target Host");

    // Split content into list and preview, stacking vertically on narrow terminals
    let (list_area, preview_area) = host_selection_layout(chunks[1]);

    // Host list with "New host configuration" as first option, then existing hosts
    let mut items: Vec<String> = vec!["+ New host configuration".to_string()];
    items.extend(hosts.iter().map(|h| h.name.clone()));
    let items_ref: Vec<&str> = items.iter().map(|s| s.as_str()).collect();

    let menu = MenuList::new(items_ref, selected).title(" Hosts ");
    frame.render_widget(menu, list_area);

    // Preview panel
    draw_host_preview(frame, preview_area, selected, hosts);

    // Footer
    draw_footer(
        frame,
        chunks[2],
        &[
            ("↑↓/jk", "Navigate"),
            ("Enter", "Select"),
            ("Esc", "Back"),
            ("?", "Help"),
        ],
    );
}

/// Draw the host preview panel
fn draw_host_preview(frame: &mut Frame, area: Rect, selected: usize, hosts: &[HostConfig]) {
    // If "New host configuration" is selected (index 0), show placeholder
    if selected == 0 {
        let content = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("Create New Host", theme::title())),
            Line::from(""),
            Line::from(Span::styled(
                "Detect hardware and create a new",
                theme::dim(),
            )),
            Line::from(Span::styled(
                "host configuration for this machine.",
                theme::dim(),
            )),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border())
                .title(Span::styled(" Preview ", theme::title())),
        );
        frame.render_widget(content, area);
        return;
    }

    // Get the selected host (adjusted for "New host" option)
    let host = &hosts[selected - 1];

    // Build preview lines
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(&host.name, theme::title())),
        Line::from(Span::styled(&host.description, theme::dim())),
        Line::from(""),
    ];

    if let Some(ref metadata) = host.metadata {
        // Form Factor
        if let Some(ref form) = metadata.form_factor {
            lines.push(Line::from(vec![
                Span::styled("  Form:  ", theme::dim()),
                Span::styled(form, theme::text()),
            ]));
        }

        // CPU
        if let Some(ref cpu) = metadata.cpu {
            lines.push(Line::from(vec![
                Span::styled("  CPU:   ", theme::dim()),
                Span::styled(&cpu.model, theme::text()),
            ]));
        }

        // GPU
        if let Some(ref gpu) = metadata.gpu {
            let gpu_text = gpu
                .model
                .as_ref()
                .map(|m| format!("{} ({})", gpu.vendor, m))
                .unwrap_or_else(|| gpu.vendor.clone());
            lines.push(Line::from(vec![
                Span::styled("  GPU:   ", theme::dim()),
                Span::styled(gpu_text, theme::text()),
            ]));
        }

        // RAM
        if let Some(ref ram) = metadata.ram {
            lines.push(Line::from(vec![
                Span::styled("  RAM:   ", theme::dim()),
                Span::styled(ram, theme::text()),
            ]));
        }
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  No hardware info available",
            theme::dim(),
        )));
        lines.push(Line::from(Span::styled(
            "  (host-info.json not found)",
            theme::dim(),
        )));
    }

    let preview = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Preview ", theme::title())),
    );
    frame.render_widget(preview, area);
}

/// Draw disk selection screen
pub fn draw_disk_selection(
    frame: &mut Frame,
    host: &str,
    disks: &[DiskInfo],
    selected: usize,
    _app: &App,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(centered_rect(70, 80, area));

    // Header
    draw_header(frame, chunks[0], &format!("Select Disk for {}", host));

    // Handle empty disk list
    if disks.is_empty() {
        let message = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("No disks found!", theme::warning())),
            Line::from(""),
            Line::from(Span::styled(
                "Please check that disks are properly connected.",
                theme::dim(),
            )),
            Line::from(""),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::warning())
                .title(Span::styled(" Available Disks ", theme::title())),
        );
        frame.render_widget(message, chunks[1]);
        draw_footer(frame, chunks[2], &[("Esc", "Back")]);
        return;
    }

    // Disk table
    let header = Row::new(vec!["", "Device", "Size", "Model"])
        .style(theme::title())
        .bottom_margin(1);

    let rows: Vec<Row> = disks
        .iter()
        .enumerate()
        .map(|(i, disk)| {
            let prefix = if i == selected { ">" } else { " " };
            let style = if i == selected {
                theme::selected()
            } else {
                theme::text()
            };
            Row::new(vec![
                prefix.to_string(),
                disk.path.clone(),
                disk.size.clone(),
                disk.model.clone().unwrap_or_default(),
            ])
            .style(style)
        })
        .collect();

    let widths = if chunks[1].width < 72 {
        vec![
            Constraint::Length(2),
            Constraint::Length(12),
            Constraint::Length(8),
            Constraint::Min(12),
        ]
    } else {
        vec![
            Constraint::Length(2),
            Constraint::Length(15),
            Constraint::Length(10),
            Constraint::Min(20),
        ]
    };

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Available Disks ", theme::title())),
    );

    frame.render_widget(table, chunks[1]);

    // Footer
    draw_footer(
        frame,
        chunks[2],
        &[
            ("↑↓/jk", "Navigate"),
            ("Enter", "Select"),
            ("Esc", "Back"),
            ("?", "Help"),
        ],
    );
}

/// Draw credentials entry screen
pub fn draw_enter_credentials(
    frame: &mut Frame,
    host: &str,
    disk: &DiskInfo,
    credentials: &InstallCredentials,
    active_field: &CredentialField,
    error: Option<&str>,
    _app: &App,
) {
    let area = frame.area();
    let center = centered_rect(65, 70, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Length(12),
            Constraint::Length(3),
            Constraint::Min(3),
        ])
        .split(center);

    // Header
    draw_header(frame, chunks[0], "Enter User Credentials");

    // Host/Disk info
    let info = Paragraph::new(vec![Line::from(vec![
        Span::styled("  Host: ", theme::dim()),
        Span::styled(host, theme::text()),
        Span::styled("  |  Disk: ", theme::dim()),
        Span::styled(&disk.path, theme::text()),
        Span::styled(format!(" ({})", disk.size), theme::dim()),
    ])])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border()),
    );
    frame.render_widget(info, chunks[1]);

    // Credential fields
    let username_style = if *active_field == CredentialField::Username {
        theme::selected()
    } else {
        theme::text()
    };
    let password_style = if *active_field == CredentialField::Password {
        theme::selected()
    } else {
        theme::text()
    };
    let confirm_style = if *active_field == CredentialField::ConfirmPassword {
        theme::selected()
    } else {
        theme::text()
    };

    // Mask passwords with asterisks
    let password_masked = "*".repeat(credentials.password.len());
    let confirm_masked = "*".repeat(credentials.confirm_password.len());

    // Show cursor on active field
    let username_display = if *active_field == CredentialField::Username {
        format!("{}_", credentials.username)
    } else {
        credentials.username.clone()
    };
    let password_display = if *active_field == CredentialField::Password {
        format!("{}_", password_masked)
    } else {
        password_masked
    };
    let confirm_display = if *active_field == CredentialField::ConfirmPassword {
        format!("{}_", confirm_masked)
    } else {
        confirm_masked
    };

    // Compute inline validation indicators
    let username_indicator = if credentials.username.is_empty() {
        Span::styled("", theme::dim())
    } else if validate_username(&credentials.username).is_none() {
        Span::styled(" ✓", theme::success())
    } else {
        Span::styled(" ✗", theme::error())
    };

    let password_indicator = if credentials.password.is_empty() {
        Span::styled("", theme::dim())
    } else if credentials.password.len() >= 8 {
        Span::styled(" ✓", theme::success())
    } else {
        Span::styled(" ✗", theme::error())
    };

    let confirm_indicator = if credentials.confirm_password.is_empty() {
        Span::styled("", theme::dim())
    } else if !credentials.password.is_empty()
        && credentials.confirm_password == credentials.password
    {
        Span::styled(" ✓", theme::success())
    } else {
        Span::styled(" ✗", theme::error())
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Username:         ", theme::dim()),
            Span::styled(username_display, username_style),
            username_indicator,
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Password:         ", theme::dim()),
            Span::styled(password_display, password_style),
            password_indicator,
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Confirm Password: ", theme::dim()),
            Span::styled(confirm_display, confirm_style),
            confirm_indicator,
        ]),
        Line::from(""),
    ];

    // Show error if present, or inline hint for focused field
    if let Some(err) = error {
        lines.push(Line::from(Span::styled(
            format!("  ⚠ {}", err),
            theme::error(),
        )));
    } else if *active_field == CredentialField::Username && !credentials.username.is_empty() {
        if let Some(err) = validate_username(&credentials.username) {
            lines.push(Line::from(Span::styled(
                format!("  ⚠ {}", err),
                theme::warning(),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "  Password will be used for login and LUKS encryption",
                theme::dim(),
            )));
        }
    } else if *active_field == CredentialField::Password
        && !credentials.password.is_empty()
        && credentials.password.len() < 8
    {
        lines.push(Line::from(Span::styled(
            "  ⚠ Must be at least 8 characters",
            theme::warning(),
        )));
    } else if *active_field == CredentialField::ConfirmPassword
        && !credentials.confirm_password.is_empty()
        && credentials.confirm_password != credentials.password
    {
        lines.push(Line::from(Span::styled(
            "  ⚠ Passwords do not match",
            theme::warning(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "  Password will be used for login and LUKS encryption",
            theme::dim(),
        )));
    }

    let fields = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Credentials ", theme::title())),
    );
    frame.render_widget(fields, chunks[2]);

    // Requirements hint
    let hints = Paragraph::new(vec![
        Line::from(Span::styled(
            "  Username: lowercase letters, numbers, underscore, hyphen",
            theme::dim(),
        )),
        Line::from(Span::styled(
            "  Password: minimum 8 characters",
            theme::dim(),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border()),
    );
    frame.render_widget(hints, chunks[3]);

    // Footer
    draw_footer(
        frame,
        chunks[4],
        &[
            ("Tab/↑↓", "Switch field"),
            ("Enter", "Continue"),
            ("Esc", "Back"),
        ],
    );
}

/// Draw swap mode selection screen
pub fn draw_select_swap_mode(
    frame: &mut Frame,
    host: &str,
    disk: &DiskInfo,
    selected: usize,
    ram_gb: u64,
    _app: &App,
) {
    let area = frame.area();
    let center = centered_rect(65, 60, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Length(12),
            Constraint::Min(3),
        ])
        .split(center);

    // Header
    draw_header(frame, chunks[0], "Select Swap Configuration");

    // Host/Disk info
    let info = Paragraph::new(vec![Line::from(vec![
        Span::styled("  Host: ", theme::dim()),
        Span::styled(host, theme::text()),
        Span::styled("  |  Disk: ", theme::dim()),
        Span::styled(&disk.path, theme::text()),
        Span::styled(format!(" ({})", disk.size), theme::dim()),
    ])])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border()),
    );
    frame.render_widget(info, chunks[1]);

    // Calculate swap size for hibernate (RAM + 2GB)
    let swap_size_gb = ram_gb + 2;

    // Swap mode options
    let zram_style = if selected == 0 {
        theme::selected()
    } else {
        theme::text()
    };
    let hibernate_style = if selected == 1 {
        theme::selected()
    } else {
        theme::text()
    };

    let zram_indicator = if selected == 0 { ">" } else { " " };
    let hibernate_indicator = if selected == 1 { ">" } else { " " };

    let options = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!(" {} ", zram_indicator), zram_style),
            Span::styled("Zram Only", zram_style),
            Span::styled(" (Recommended)", theme::dim()),
        ]),
        Line::from(vec![
            Span::styled("     ", theme::dim()),
            Span::styled("Compressed RAM swap, fast, no hibernate", theme::dim()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!(" {} ", hibernate_indicator), hibernate_style),
            Span::styled("Hibernate Support", hibernate_style),
        ]),
        Line::from(vec![
            Span::styled("     ", theme::dim()),
            Span::styled(
                format!("Swapfile inside encrypted volume ({} GB)", swap_size_gb),
                theme::dim(),
            ),
        ]),
        Line::from(vec![
            Span::styled("     ", theme::dim()),
            Span::styled("Enables suspend-to-disk", theme::dim()),
        ]),
        Line::from(""),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Swap Mode ", theme::title())),
    );
    frame.render_widget(options, chunks[2]);

    // Footer
    draw_footer(
        frame,
        chunks[3],
        &[("↑↓/jk", "Navigate"), ("Enter", "Select"), ("Esc", "Back")],
    );
}

/// Draw hardware profile selection screen
pub fn draw_select_hardware_profile(
    frame: &mut Frame,
    host: &str,
    disk: &DiskInfo,
    selected: usize,
    _app: &App,
) {
    let area = frame.area();
    let center = centered_rect(68, 62, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Length(14),
            Constraint::Min(3),
        ])
        .split(center);

    draw_header(frame, chunks[0], "Select Hardware Profile");

    let info = Paragraph::new(vec![Line::from(vec![
        Span::styled("  Host: ", theme::dim()),
        Span::styled(host, theme::text()),
        Span::styled("  |  Disk: ", theme::dim()),
        Span::styled(&disk.path, theme::text()),
        Span::styled(format!(" ({})", disk.size), theme::dim()),
    ])])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border()),
    );
    frame.render_widget(info, chunks[1]);

    let keep_style = if selected == 0 {
        theme::selected()
    } else {
        theme::text()
    };
    let refresh_style = if selected == 1 {
        theme::selected()
    } else {
        theme::text()
    };
    let keep_indicator = if selected == 0 { ">" } else { " " };
    let refresh_indicator = if selected == 1 { ">" } else { " " };

    let options = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!(" {} ", keep_indicator), keep_style),
            Span::styled("Keep checked-in profile", keep_style),
            Span::styled(" (Recommended)", theme::dim()),
        ]),
        Line::from(vec![
            Span::styled("     ", theme::dim()),
            Span::styled(
                "Use the repo's known-good hardware settings without extra live detection.",
                theme::dim(),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!(" {} ", refresh_indicator), refresh_style),
            Span::styled("Refresh from live system", refresh_style),
        ]),
        Line::from(vec![
            Span::styled("     ", theme::dim()),
            Span::styled(
                "Generate a machine-detected hardware layer from this installer session.",
                theme::dim(),
            ),
        ]),
        Line::from(vec![
            Span::styled("     ", theme::dim()),
            Span::styled(
                "Useful for unusual devices, but less predictable than the checked-in profile.",
                theme::dim(),
            ),
        ]),
        Line::from(""),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(Span::styled(" Hardware Profile ", theme::title())),
    );
    frame.render_widget(options, chunks[2]);

    draw_footer(
        frame,
        chunks[3],
        &[("↑↓/jk", "Navigate"), ("Enter", "Select"), ("Esc", "Back")],
    );
}

/// Draw overview/confirmation screen
pub fn draw_overview(
    frame: &mut Frame,
    host: &str,
    disk: &DiskInfo,
    credentials: &InstallCredentials,
    input: &str,
    hardware_config: Option<&crate::app::state::NewHostConfig>,
    _app: &App,
) {
    let area = frame.area();
    let center = centered_rect(70, 70, area);

    // Calculate details height based on whether we have hardware info
    let details_height = if hardware_config.is_some() { 12 } else { 8 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(details_height),
            Constraint::Length(5),
            Constraint::Min(3),
        ])
        .split(center);

    // Warning header
    let warning = Paragraph::new(Line::from(vec![
        Span::styled("⚠ ", theme::warning()),
        Span::styled("WARNING: This will ERASE ALL DATA!", theme::warning()),
    ]))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::warning()),
    );
    frame.render_widget(warning, chunks[0]);

    // Build details lines
    let mut detail_lines = vec![Line::from("")];

    detail_lines.push(Line::from(vec![
        Span::styled("  Hostname: ", theme::dim()),
        Span::styled(host, theme::text()),
    ]));

    detail_lines.push(Line::from(vec![
        Span::styled("  Disk:     ", theme::dim()),
        Span::styled(&disk.path, theme::text()),
        Span::styled(format!(" ({})", disk.size), theme::dim()),
    ]));

    detail_lines.push(Line::from(vec![
        Span::styled("  User:     ", theme::dim()),
        Span::styled(&credentials.username, theme::text()),
    ]));

    // Show swap mode selection
    let swap_mode_text = match credentials.swap_mode {
        SwapMode::ZramOnly => "Zram Only (no hibernate)",
        SwapMode::HibernateSupport => "Hibernate Support (disk swapfile)",
    };
    detail_lines.push(Line::from(vec![
        Span::styled("  Swap:     ", theme::dim()),
        Span::styled(swap_mode_text, theme::text()),
    ]));

    let hardware_refresh_text = if credentials.refresh_hardware_config {
        "Live detection layer"
    } else {
        "Checked-in profile"
    };
    detail_lines.push(Line::from(vec![
        Span::styled("  Hardware: ", theme::dim()),
        Span::styled(hardware_refresh_text, theme::text()),
    ]));

    // Add hardware info if available (new host)
    if let Some(hw) = hardware_config {
        detail_lines.push(Line::from(vec![
            Span::styled("  CPU:      ", theme::dim()),
            Span::styled(format!("{}", hw.cpu.vendor), theme::text()),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled("  GPU:      ", theme::dim()),
            Span::styled(format!("{}", hw.gpu.vendor), theme::text()),
        ]));
        detail_lines.push(Line::from(vec![
            Span::styled("  Type:     ", theme::dim()),
            Span::styled(format!("{:?}", hw.form_factor), theme::text()),
        ]));
    }

    detail_lines.push(Line::from(""));

    let details = Paragraph::new(detail_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(" Installation Overview "),
    );
    frame.render_widget(details, chunks[1]);

    // Input prompt
    let prompt = Paragraph::new(vec![
        Line::from(Span::styled("Type 'yes' to continue:", theme::text())),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", theme::info()),
            Span::styled(input, theme::text()),
            Span::styled("_", theme::info()),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border()),
    );
    frame.render_widget(prompt, chunks[2]);

    // Footer
    draw_footer(
        frame,
        chunks[3],
        &[("Type 'yes' + Enter", "Confirm"), ("Esc", "Cancel")],
    );
}

/// Draw running installation screen
pub fn draw_running(
    frame: &mut Frame,
    host: &str,
    disk: &DiskInfo,
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

    // Header with host/disk info
    let header = Paragraph::new(Line::from(vec![
        Span::styled(" Installing NixOS | ", theme::title()),
        Span::styled("Host: ", theme::dim()),
        Span::styled(host, theme::text()),
        Span::styled(" | Disk: ", theme::dim()),
        Span::styled(&disk.path, theme::text()),
        Span::styled(format!(" ({})", disk.size), theme::dim()),
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
    let footer = Paragraph::new(footer_hints(&[("Ctrl+C", "Cancel")])).alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

/// Draw completion screen (shows output log)
pub fn draw_complete(
    frame: &mut Frame,
    success: bool,
    output: &[String],
    scroll_offset: Option<usize>,
    _app: &App,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    // Header
    let (title, style) = if success {
        (" ✓ Installation Complete ", theme::success())
    } else {
        (" ✗ Installation Failed ", theme::error())
    };
    let header = Paragraph::new(Line::from(Span::styled(title, style)))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).border_style(style));
    frame.render_widget(header, chunks[0]);

    // Output log
    let mut log = LogView::new(output).title(" Output ");
    if let Some(offset) = scroll_offset {
        log = log.scroll_offset(offset);
    }
    frame.render_widget(log, chunks[1]);

    // Footer - show reboot option on success
    let footer = if success {
        Paragraph::new(footer_hints(&[
            ("↑↓/jk", "Scroll"),
            ("r", "Reboot"),
            ("Enter", "Menu"),
            ("q", "Quit"),
        ]))
    } else {
        Paragraph::new(footer_hints(&[
            ("↑↓/jk", "Scroll"),
            ("Enter", "Menu"),
            ("Esc", "Back"),
            ("q", "Quit"),
        ]))
    };
    frame.render_widget(footer.alignment(Alignment::Center), chunks[2]);
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

fn draw_footer(frame: &mut Frame, area: Rect, hints: &[(&str, &str)]) {
    let footer = Paragraph::new(footer_hints(hints)).alignment(Alignment::Center);
    frame.render_widget(footer, area);
}
