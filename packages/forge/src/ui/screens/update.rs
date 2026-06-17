//! Update screens

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{
    App, LocalChange, LocalChangesResolution, NixProgressRow, NixProgressState, NixProgressStatus,
    StepStatus, UpdateCoreStatus, UpdateDryRunStatus, UpdateOptions, UpdatePreflightField,
    UpdatePreflightReport, UpdateSummary,
};
use crate::ui::layout::{centered_fixed, footer_hints, progress_layout};
use crate::ui::theme;
use crate::ui::widgets::{LogView, ProgressSteps};

/// Draw update preflight configuration screen
pub fn draw_preflight(
    frame: &mut Frame,
    options: &UpdateOptions,
    selected: UpdatePreflightField,
    input_buffer: &str,
    editing_inputs: bool,
) {
    let area = centered_fixed(
        72.min(frame.area().width.saturating_sub(2)),
        16,
        frame.area(),
    );
    frame.render_widget(Clear, area);

    let mode_line = format!("Mode: {}", options.mode_label());
    let inputs_line = if input_buffer.trim().is_empty() {
        "Inputs: all flake inputs".to_string()
    } else {
        format!("Inputs: {}", input_buffer)
    };

    let rows = [
        (UpdatePreflightField::Start, "Start update".to_string()),
        (UpdatePreflightField::Mode, mode_line),
        (
            UpdatePreflightField::Inputs,
            if editing_inputs {
                format!("Inputs: {}_", input_buffer)
            } else {
                inputs_line
            },
        ),
        (UpdatePreflightField::Back, "Back to main menu".to_string()),
    ];

    let mut lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Configure what forge update should run before we touch the system.",
            theme::text(),
        )),
        Line::from(""),
    ];

    for (field, label) in rows {
        let is_selected = field == selected;
        let style = if is_selected {
            theme::selected()
        } else {
            theme::text()
        };
        let prefix = if is_selected { "▶ " } else { "  " };
        lines.push(Line::from(Span::styled(
            format!("{}{}", prefix, label),
            style,
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Specific inputs accept commas or spaces, for example: nixpkgs home-manager",
        theme::dim(),
    )));

    let hints = if editing_inputs {
        footer_hints(&[
            ("Type", "Edit inputs"),
            ("Enter", "Save"),
            ("Esc", "Cancel"),
        ])
    } else {
        footer_hints(&[
            ("↑↓/jk", "Navigate"),
            ("Enter", "Select"),
            ("←→/Space", "Cycle mode"),
            ("Esc", "Back"),
        ])
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border_active())
        .title(Span::styled(" Update Preflight ", theme::title()));
    frame.render_widget(Paragraph::new(lines).block(block), area);

    let footer_area = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(2),
        area.width,
        1,
    );
    frame.render_widget(
        Paragraph::new(hints).alignment(Alignment::Center),
        footer_area,
    );
}

/// Draw final preflight review before update starts.
pub fn draw_preparing(
    frame: &mut Frame,
    steps: &[StepStatus],
    output: &[String],
    scroll_offset: Option<usize>,
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

    let header = Paragraph::new(Line::from(Span::styled(
        " Preparing Update ",
        theme::title(),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_active()),
    );
    frame.render_widget(header, chunks[0]);

    let (steps_area, output_area) = progress_layout(chunks[1]);
    let progress = ProgressSteps::new(steps, app.spinner_state).title(" Checks ");
    frame.render_widget(progress, steps_area);

    let mut log = LogView::new(output).title(" Details ");
    if let Some(offset) = scroll_offset {
        log = log.scroll_offset(offset);
    }
    frame.render_widget(log, output_area);

    let footer = Paragraph::new(footer_hints(&[
        ("↑↓/jk", "Scroll"),
        ("f", "Follow"),
        ("Ctrl+C", "Cancel"),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

/// Draw final preflight review before update starts.
pub fn draw_review_preflight(
    frame: &mut Frame,
    options: &UpdateOptions,
    report: &UpdatePreflightReport,
    selected: usize,
) {
    let area = centered_fixed(
        78.min(frame.area().width.saturating_sub(2)),
        24.min(frame.area().height.saturating_sub(2)),
        frame.area(),
    );
    frame.render_widget(Clear, area);

    let inputs_line = if options.inputs.is_empty() {
        "all flake inputs".to_string()
    } else {
        options.inputs.join(", ")
    };
    let local_resolution = match report.pending_resolution {
        Some(LocalChangesResolution::Overwrite) => "discard local changes",
        Some(LocalChangesResolution::Stash) => "stash local changes",
        Some(LocalChangesResolution::Cancel) => "cancel",
        None => "no local resolution needed",
    };
    let remote_line = if let Some(error) = &report.remote.error {
        format!("Remote status: unavailable ({})", error)
    } else if report.remote.checked {
        format!(
            "Remote status: {} ahead, {} behind on {}",
            report.remote.ahead,
            report.remote.behind,
            report.remote.upstream.as_deref().unwrap_or("upstream")
        )
    } else {
        "Remote status: not available".to_string()
    };
    let dry_run_line = match &report.dry_run {
        UpdateDryRunStatus::Passed => "Dry run: passed".to_string(),
        UpdateDryRunStatus::Failed(message) => format!("Dry run: failed ({})", message),
        UpdateDryRunStatus::Skipped(message) => format!("Dry run: skipped ({})", message),
    };

    let mut lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Review the update before anything changes on disk.",
            theme::text(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Mode: {}", options.mode_label()),
            theme::text(),
        )),
        Line::from(Span::styled(
            format!("Inputs: {}", inputs_line),
            theme::text(),
        )),
        Line::from(Span::styled(
            format!(
                "Local changes: {} tracked, {} untracked",
                report.tracked_count, report.untracked_count
            ),
            theme::text(),
        )),
        Line::from(Span::styled(
            format!("Resolution: {}", local_resolution),
            theme::text(),
        )),
        Line::from(Span::styled(remote_line, theme::text())),
        Line::from(Span::styled(dry_run_line, theme::text())),
        Line::from(""),
    ];

    if !report.missing_required_tools.is_empty() {
        lines.push(Line::from(Span::styled(
            "Required tools missing:",
            theme::warning(),
        )));
        for tool in &report.missing_required_tools {
            lines.push(Line::from(Span::styled(
                format!("  - {}", tool),
                theme::warning(),
            )));
        }
        lines.push(Line::from(""));
    }

    if !report.missing_optional_tools.is_empty() {
        lines.push(Line::from(Span::styled(
            "Optional tools missing:",
            theme::dim(),
        )));
        for tool in &report.missing_optional_tools {
            lines.push(Line::from(Span::styled(
                format!("  - {}", tool),
                theme::dim(),
            )));
        }
        lines.push(Line::from(""));
    }

    let status_style = if report.can_continue() {
        theme::success()
    } else {
        theme::warning()
    };
    let status_text = if report.can_continue() {
        "Ready to continue"
    } else {
        "Resolve the preflight blockers before continuing"
    };
    lines.push(Line::from(Span::styled(status_text, status_style)));
    lines.push(Line::from(""));

    for (index, label) in ["Continue update", "Back to setup"].iter().enumerate() {
        let is_selected = index == selected;
        let prefix = if is_selected { "▶ " } else { "  " };
        let style = if is_selected {
            theme::selected()
        } else {
            theme::text()
        };
        lines.push(Line::from(Span::styled(
            format!("{}{}", prefix, label),
            style,
        )));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border_active())
        .title(Span::styled(" Update Review ", theme::title()));
    frame.render_widget(Paragraph::new(lines).block(block), area);

    let footer_area = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(2),
        area.width,
        1,
    );
    frame.render_widget(
        Paragraph::new(footer_hints(&[
            ("↑↓/jk", "Navigate"),
            ("Enter", "Select"),
            ("Esc", "Back"),
        ]))
        .alignment(Alignment::Center),
        footer_area,
    );
}

/// Draw the destructive overwrite confirmation.
pub fn draw_overwrite_confirm(
    frame: &mut Frame,
    changes: &[LocalChange],
    tracked_count: usize,
    untracked_count: usize,
    selected: usize,
) {
    let area = centered_fixed(
        64.min(frame.area().width.saturating_sub(2)),
        18.min(frame.area().height.saturating_sub(2)),
        frame.area(),
    );
    frame.render_widget(Clear, area);

    let preview_count = changes.len().min(5);
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(Span::styled(
            "This will permanently discard local tracked and untracked changes.",
            theme::warning(),
        )),
        Line::from(Span::styled(
            format!(
                "{} tracked, {} untracked changes will be removed.",
                tracked_count, untracked_count
            ),
            theme::text(),
        )),
        Line::from(""),
        Line::from(Span::styled("Preview:", theme::title())),
    ];

    for change in changes.iter().take(preview_count) {
        lines.push(Line::from(Span::styled(
            format!(
                "  {} {}",
                if change.tracked { "[tracked]" } else { "[new]" },
                change.path
            ),
            theme::dim(),
        )));
    }
    if changes.len() > preview_count {
        lines.push(Line::from(Span::styled(
            format!("  ... and {} more", changes.len() - preview_count),
            theme::dim(),
        )));
    }
    lines.push(Line::from(""));

    for (index, label) in ["Discard changes and continue", "Back"].iter().enumerate() {
        let is_selected = index == selected;
        let prefix = if is_selected { "▶ " } else { "  " };
        let style = if is_selected {
            theme::selected()
        } else {
            theme::text()
        };
        lines.push(Line::from(Span::styled(
            format!("{}{}", prefix, label),
            style,
        )));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::warning())
        .title(Span::styled(" Confirm Discard ", theme::warning()));
    frame.render_widget(Paragraph::new(lines).block(block), area);

    let footer_area = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(2),
        area.width,
        1,
    );
    frame.render_widget(
        Paragraph::new(footer_hints(&[
            ("↑↓/jk", "Navigate"),
            ("Enter", "Select"),
            ("Esc", "Back"),
        ]))
        .alignment(Alignment::Center),
        footer_area,
    );
}

/// Draw running/complete update screen
pub fn draw_running(
    frame: &mut Frame,
    steps: &[StepStatus],
    output: &[String],
    complete: bool,
    scroll_offset: Option<usize>,
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
    let title = if complete {
        " Update Complete "
    } else {
        " NixOS System Update "
    };
    let header = Paragraph::new(Line::from(Span::styled(title, theme::title())))
        .alignment(Alignment::Center)
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

    let mut log = LogView::new(output).title(" Output ");
    if let Some(offset) = scroll_offset {
        log = log.scroll_offset(offset);
    }
    frame.render_widget(log, output_area);

    // Footer
    let footer = if complete {
        Paragraph::new(footer_hints(&[
            ("↑↓/jk", "Scroll"),
            ("f", "Follow"),
            ("Enter", "Menu"),
            ("Esc", "Back"),
            ("q", "Quit"),
        ]))
    } else {
        Paragraph::new(footer_hints(&[
            ("↑↓/jk", "Scroll"),
            ("f", "Follow"),
            ("Ctrl+C", "Cancel"),
        ]))
    }
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

/// Draw direct `forge update` with structured Nix progress.
pub fn draw_modern_running(
    frame: &mut Frame,
    steps: &[StepStatus],
    nix_progress: &NixProgressState,
    output: &[String],
    complete: bool,
    scroll_offset: Option<usize>,
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

    let title = if complete {
        " Update Complete "
    } else {
        " NixOS System Update "
    };
    let header = Paragraph::new(Line::from(Span::styled(title, theme::title())))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border_active()),
        );
    frame.render_widget(header, chunks[0]);

    let body = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if chunks[1].height < 18 { 7 } else { 9 }),
            Constraint::Min(7),
            Constraint::Length(6.min(chunks[1].height.saturating_sub(9)).max(3)),
        ])
        .split(chunks[1]);

    let progress = ProgressSteps::new(steps, app.spinner_state).title(" Progress ");
    frame.render_widget(progress, body[0]);

    draw_nix_progress_table(frame, body[1], nix_progress);

    let mut log = LogView::new(output).title(" Log ");
    if let Some(offset) = scroll_offset {
        log = log.scroll_offset(offset);
    }
    frame.render_widget(log, body[2]);

    let footer = if complete {
        Paragraph::new(footer_hints(&[
            ("↑↓/jk", "Scroll"),
            ("f", "Follow"),
            ("Enter", "Menu"),
            ("Esc", "Back"),
            ("q", "Quit"),
        ]))
    } else {
        Paragraph::new(footer_hints(&[
            ("↑↓/jk", "Scroll"),
            ("f", "Follow"),
            ("Ctrl+C", "Cancel"),
        ]))
    }
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

fn draw_nix_progress_table(frame: &mut Frame, area: Rect, progress: &NixProgressState) {
    let inner_width = area.width.saturating_sub(4).max(20) as usize;
    let name_width = if inner_width >= 92 {
        34
    } else if inner_width >= 72 {
        28
    } else {
        22
    };
    let bar_width = if inner_width >= 92 {
        28
    } else if inner_width >= 72 {
        22
    } else {
        14
    };

    let mut lines: Vec<Line<'static>> = vec![Line::from(vec![
        Span::styled(":: ", theme::title()),
        Span::styled(progress.section.clone(), theme::title()),
    ])];

    if progress.rows.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Waiting for Nix progress events...",
            theme::dim(),
        )));
    } else {
        for row in progress.rows.iter().rev().take(8).rev() {
            lines.push(render_progress_row(row, name_width, bar_width));
        }
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Span::styled(" Downloads and Builds ", theme::title()));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_progress_row(row: &NixProgressRow, name_width: usize, bar_width: usize) -> Line<'static> {
    let name = truncate_middle(&row.name, name_width);
    let mut spans = vec![Span::styled(
        format!("{:<width$} ", name, width = name_width),
        theme::text(),
    )];

    match row.status {
        NixProgressStatus::Downloading => {
            let transferred = row.transferred.unwrap_or(0);
            if let Some(total) = row.total {
                spans.push(Span::styled(
                    format!("{:>9} ", format_bytes(total)),
                    theme::dim(),
                ));
                spans.push(Span::styled(
                    format!(
                        "{:>10} ",
                        row.speed_bps
                            .map(format_speed)
                            .unwrap_or_else(|| "--".to_string())
                    ),
                    theme::dim(),
                ));
                spans.push(Span::styled(
                    format!(
                        "{:>6} ",
                        row.eta_secs
                            .map(format_eta)
                            .unwrap_or_else(|| "--:--".to_string())
                    ),
                    theme::dim(),
                ));
                spans.push(Span::styled("[", theme::dim()));
                let (filled, empty, percent) = progress_parts(transferred, total, bar_width);
                spans.push(Span::styled("█".repeat(filled), theme::success()));
                spans.push(Span::styled("░".repeat(empty), theme::dim()));
                spans.push(Span::styled("] ", theme::dim()));
                spans.push(Span::styled(format!("{:>3}%", percent), theme::success()));
            } else {
                spans.push(Span::styled(
                    format!("{:>12} ", "downloading"),
                    theme::info(),
                ));
                spans.push(Span::styled(
                    format!("{:>9} ", format_bytes(transferred)),
                    theme::dim(),
                ));
                if let Some(speed) = row.speed_bps {
                    spans.push(Span::styled(
                        format!("{:>10}", format_speed(speed)),
                        theme::dim(),
                    ));
                }
            }
        }
        NixProgressStatus::Building => {
            spans.push(Span::styled("building", theme::warning()));
        }
        NixProgressStatus::Activating => {
            spans.push(Span::styled(
                "activating new system generation",
                theme::title(),
            ));
        }
        NixProgressStatus::Complete => {
            spans.push(Span::styled("complete", theme::success()));
        }
        NixProgressStatus::Failed => {
            spans.push(Span::styled("failed", theme::error()));
        }
    }

    Line::from(spans)
}

fn progress_parts(transferred: u64, total: u64, width: usize) -> (usize, usize, u64) {
    if total == 0 || width == 0 {
        return (0, width, 0);
    }
    let clamped = transferred.min(total);
    let filled = ((clamped as f64 / total as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    let percent = ((clamped as f64 / total as f64) * 100.0).round() as u64;
    (filled, width.saturating_sub(filled), percent.min(100))
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = bytes as f64;
    if value >= GIB {
        format!("{:.1} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{} B", bytes)
    }
}

fn format_speed(bytes_per_second: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_second.max(0.0) as u64))
}

fn format_eta(seconds: u64) -> String {
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

fn truncate_middle(value: &str, max_len: usize) -> String {
    if value.chars().count() <= max_len {
        return value.to_string();
    }
    if max_len <= 3 {
        return ".".repeat(max_len);
    }
    let head_len = (max_len - 3) / 2;
    let tail_len = max_len - 3 - head_len;
    let head: String = value.chars().take(head_len).collect();
    let tail: String = value
        .chars()
        .rev()
        .take(tail_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{}...{}", head, tail)
}

/// Options for local changes resolution
const LOCAL_CHANGES_OPTIONS: &[&str] = &[
    "Overwrite - Discard all local changes",
    "Stash - Save changes, restore after update",
    "Cancel - Keep changes, abort update",
];

/// Draw local changes prompt dialog
pub fn draw_local_changes_prompt(
    frame: &mut Frame,
    changes: &[LocalChange],
    tracked_count: usize,
    untracked_count: usize,
    selected: usize,
) {
    let area = frame.area();

    // Calculate popup dimensions
    let popup_width = 60.min(area.width.saturating_sub(4));
    let file_list_height = changes.len().min(8) as u16;
    let popup_height = (12 + file_list_height).min(area.height.saturating_sub(4));

    // Center the popup
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    // Create layout inside popup
    let inner_area = Rect::new(
        popup_area.x + 1,
        popup_area.y + 1,
        popup_area.width.saturating_sub(2),
        popup_area.height.saturating_sub(2),
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),                    // Title
            Constraint::Length(file_list_height + 2), // File list
            Constraint::Length(5),                    // Options
            Constraint::Length(2),                    // Footer
        ])
        .split(inner_area);

    // Draw border
    let border = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::warning())
        .title(Span::styled(" Local Changes Detected ", theme::warning()));
    frame.render_widget(border, popup_area);

    // Title/description
    let title = Paragraph::new(Line::from(vec![Span::styled(
        format!(
            "Detected {} tracked and {} untracked local changes:",
            tracked_count, untracked_count
        ),
        theme::text(),
    )]))
    .alignment(Alignment::Left);
    frame.render_widget(title, chunks[0]);

    // File list
    let files: Vec<ListItem> = changes
        .iter()
        .take(8)
        .map(|change| {
            ListItem::new(Line::from(vec![
                Span::styled("  ", theme::dim()),
                Span::styled(
                    if change.tracked {
                        "[tracked] "
                    } else {
                        "[new] "
                    },
                    theme::dim(),
                ),
                Span::styled(change.path.clone(), theme::info()),
            ]))
        })
        .collect();

    let more_indicator = if changes.len() > 8 {
        format!("  ... and {} more", changes.len() - 8)
    } else {
        String::new()
    };

    let mut file_items = files;
    if !more_indicator.is_empty() {
        file_items.push(ListItem::new(Line::from(Span::styled(
            more_indicator,
            theme::dim(),
        ))));
    }

    let file_list = List::new(file_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border())
            .title(" Changed Files "),
    );
    frame.render_widget(file_list, chunks[1]);

    // Options
    let options: Vec<ListItem> = LOCAL_CHANGES_OPTIONS
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let style = if i == selected {
                theme::selected()
            } else {
                theme::text()
            };
            let prefix = if i == selected { "▶ " } else { "  " };
            ListItem::new(Line::from(Span::styled(
                format!("{}{}", prefix, opt),
                style,
            )))
        })
        .collect();

    let options_list = List::new(options);
    frame.render_widget(options_list, chunks[2]);

    // Footer
    let footer = Paragraph::new(footer_hints(&[
        ("↑↓/jk", "Navigate"),
        ("Enter", "Select"),
        ("Esc", "Cancel"),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[3]);
}

/// Draw the ShowingSummary state - shows the log with a summary modal overlay
pub fn draw_showing_summary(
    frame: &mut Frame,
    steps: &[StepStatus],
    output: &[String],
    summary: &UpdateSummary,
    scroll_offset: Option<usize>,
    summary_scroll: usize,
    app: &App,
) {
    // Draw the log view in the background
    draw_running(frame, steps, output, true, scroll_offset, app);

    // Draw the summary modal on top
    draw_summary_modal(frame, summary, summary_scroll);
}

/// Draw the summary modal overlay
fn draw_summary_modal(frame: &mut Frame, summary: &UpdateSummary, summary_scroll: usize) {
    let area = frame.area();

    let modal_width = ((area.width as u32 * 3) / 5).min(70).max(50) as u16;

    let claude_changed = summary.claude_old.is_some()
        && summary.claude_new.is_some()
        && summary.claude_old != summary.claude_new;
    let codex_changed = summary.codex_old.is_some()
        && summary.codex_new.is_some()
        && summary.codex_old != summary.codex_new;

    // Build ALL content lines (no truncation)
    let mut all_lines: Vec<Line<'static>> = vec![Line::from("")];

    // Configuration commits pulled from upstream
    if !summary.config_commits.is_empty() {
        all_lines.push(Line::from(Span::styled(
            "Configuration Updates:".to_string(),
            theme::title(),
        )));
        let commit_count = summary.config_commits.len();
        all_lines.push(Line::from(Span::styled(
            format!(
                "  Pulled {} commit{}",
                commit_count,
                if commit_count == 1 { "" } else { "s" }
            ),
            theme::success(),
        )));
        for commit in &summary.config_commits {
            all_lines.push(Line::from(Span::styled(
                format!("    {} {}", commit.hash, commit.message),
                theme::info(),
            )));
        }
        all_lines.push(Line::from(""));
    }

    // Flake changes section (no limit)
    if !summary.flake_changes.is_empty() {
        all_lines.push(Line::from(Span::styled(
            "Flake Inputs:".to_string(),
            theme::title(),
        )));
        for change in &summary.flake_changes {
            let commit_text = if change.total_commits == 1 {
                "commit"
            } else {
                "commits"
            };
            all_lines.push(Line::from(vec![
                Span::styled(format!("  {:20}", change.name), theme::text()),
                Span::styled(
                    format!("+{} {}", change.total_commits, commit_text),
                    theme::success(),
                ),
            ]));
            all_lines.push(Line::from(Span::styled(
                format!(
                    "    {} -> {}",
                    &change.old_rev[..7.min(change.old_rev.len())],
                    &change.new_rev[..7.min(change.new_rev.len())]
                ),
                theme::dim(),
            )));
            for commit in change.commits.iter().take(3) {
                all_lines.push(Line::from(Span::styled(
                    format!("    {} {}", commit.hash, commit.message),
                    theme::info(),
                )));
            }
            if let Some(compare_url) = &change.compare_url {
                all_lines.push(Line::from(Span::styled(
                    format!("    {}", compare_url),
                    theme::dim(),
                )));
            }
        }
        all_lines.push(Line::from(""));
    }

    // Package changes section (no limit)
    if !summary.package_changes.is_empty() {
        let pkg_count = summary.package_changes.len();
        all_lines.push(Line::from(Span::styled(
            format!("Packages ({} changed):", pkg_count),
            theme::title(),
        )));
        for (name, old, new) in &summary.package_changes {
            let display_name = if name.len() > 25 {
                format!("{}...", &name[..22])
            } else {
                name.clone()
            };
            all_lines.push(Line::from(vec![
                Span::styled(format!("  {:25}", display_name), theme::text()),
                Span::styled(format!("{} → {}", old, new), theme::info()),
            ]));
        }
        all_lines.push(Line::from(""));
    }

    // Added packages section
    if !summary.packages_added.is_empty() {
        all_lines.push(Line::from(Span::styled(
            format!("Added ({}):", summary.packages_added.len()),
            theme::title(),
        )));
        for (name, ver) in &summary.packages_added {
            let display_name = if name.len() > 25 {
                format!("{}...", &name[..22])
            } else {
                name.clone()
            };
            all_lines.push(Line::from(vec![
                Span::styled(format!("  + {:25}", display_name), theme::success()),
                Span::styled(ver.to_string(), theme::success()),
            ]));
        }
        all_lines.push(Line::from(""));
    }

    // Removed packages section
    if !summary.packages_removed.is_empty() {
        all_lines.push(Line::from(Span::styled(
            format!("Removed ({}):", summary.packages_removed.len()),
            theme::title(),
        )));
        for (name, ver) in &summary.packages_removed {
            let display_name = if name.len() > 25 {
                format!("{}...", &name[..22])
            } else {
                name.clone()
            };
            all_lines.push(Line::from(vec![
                Span::styled(format!("  - {:25}", display_name), theme::error()),
                Span::styled(ver.to_string(), theme::error()),
            ]));
        }
        all_lines.push(Line::from(""));
    }

    // CLI tools section
    if claude_changed || codex_changed {
        all_lines.push(Line::from(Span::styled(
            "CLI Tools:".to_string(),
            theme::title(),
        )));
        if claude_changed {
            all_lines.push(Line::from(vec![
                Span::styled("  Claude Code       ".to_string(), theme::text()),
                Span::styled(
                    format!(
                        "{} → {}",
                        summary.claude_old.as_deref().unwrap_or(""),
                        summary.claude_new.as_deref().unwrap_or("")
                    ),
                    theme::info(),
                ),
            ]));
        }
        if codex_changed {
            all_lines.push(Line::from(vec![
                Span::styled("  Codex CLI         ".to_string(), theme::text()),
                Span::styled(
                    format!(
                        "{} → {}",
                        summary.codex_old.as_deref().unwrap_or(""),
                        summary.codex_new.as_deref().unwrap_or("")
                    ),
                    theme::info(),
                ),
            ]));
        }
        all_lines.push(Line::from(""));
    }

    let (system_text, system_style) = match summary.core_status {
        UpdateCoreStatus::Pending => ("System status pending".to_string(), theme::dim()),
        UpdateCoreStatus::Success => ("System updated successfully".to_string(), theme::success()),
        UpdateCoreStatus::UpToDate => ("System already up to date".to_string(), theme::dim()),
        UpdateCoreStatus::Partial => (
            "System update only partially completed".to_string(),
            theme::warning(),
        ),
        UpdateCoreStatus::Cancelled => ("System update cancelled".to_string(), theme::warning()),
    };
    all_lines.push(Line::from(Span::styled(
        format!("  {}", system_text),
        system_style,
    )));

    if let Some(partial_state) = &summary.partial_state {
        all_lines.push(Line::from(Span::styled(
            format!("  {}", partial_state),
            theme::warning(),
        )));
    }

    if !summary.desktop_shell_status.is_empty() {
        all_lines.push(Line::from(Span::styled(
            format!("  Desktop shell: {}", summary.desktop_shell_status),
            theme::info(),
        )));
    }

    // Reboot warning
    if !summary.reboot_reasons.is_empty() {
        all_lines.push(Line::from(""));
        let reasons = summary.reboot_reasons.join(", ");
        all_lines.push(Line::from(Span::styled(
            format!("⚠ Reboot recommended ({})", reasons),
            theme::warning(),
        )));
    }

    if !summary.follow_up_warnings.is_empty() {
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(Span::styled(
            "Follow-up Warnings:".to_string(),
            theme::title(),
        )));
        for warning in &summary.follow_up_warnings {
            all_lines.push(Line::from(Span::styled(
                format!("  ! {}", warning),
                theme::warning(),
            )));
        }
    }

    // "Nothing changed" case
    if summary.flake_changes.is_empty()
        && summary.package_changes.is_empty()
        && summary.packages_added.is_empty()
        && summary.packages_removed.is_empty()
        && !claude_changed
        && !codex_changed
        && summary.follow_up_warnings.is_empty()
        && summary.core_status != UpdateCoreStatus::Partial
    {
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(Span::styled(
            "  System is up to date - no changes".to_string(),
            theme::success(),
        )));
        all_lines.push(Line::from(""));
    }

    // Calculate modal height: content area + borders(2) + footer(3)
    let max_modal_height = area.height.saturating_sub(4);
    let footer_lines = 3u16; // separator + hints + blank
    let available_content = max_modal_height.saturating_sub(2 + footer_lines) as usize;
    let total_content = all_lines.len();

    // Clamp scroll
    let max_scroll = total_content.saturating_sub(available_content);
    let scroll = summary_scroll.min(max_scroll);

    // Slice visible content
    let visible_end = (scroll + available_content).min(total_content);
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Scroll indicator (top)
    if scroll > 0 {
        lines.push(Line::from(Span::styled(
            format!("  ↑ {} more above", scroll),
            theme::dim(),
        )));
    }

    lines.extend(all_lines[scroll..visible_end].to_vec());

    // Scroll indicator (bottom)
    let remaining_below = total_content.saturating_sub(visible_end);
    if remaining_below > 0 {
        lines.push(Line::from(Span::styled(
            format!("  ↓ {} more below", remaining_below),
            theme::dim(),
        )));
    }

    // Pad to fill available space
    while lines.len() < available_content {
        lines.push(Line::from(""));
    }

    // Footer
    lines.push(Line::from(Span::styled(
        "─".repeat((modal_width - 4) as usize),
        theme::dim(),
    )));

    let mut hints: Vec<(&str, &str)> = vec![
        ("↑↓/jk", "Scroll"),
        ("Enter/Esc", "Done"),
        ("v", "View log"),
    ];
    if !summary.reboot_reasons.is_empty() {
        hints.push(("r", "Reboot"));
    }
    hints.push(("q", "Quit"));
    lines.push(footer_hints(&hints));

    let actual_height = (lines.len() as u16 + 2).min(max_modal_height).max(10);
    let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = area.y + (area.height.saturating_sub(actual_height)) / 2;
    let modal_area = Rect::new(modal_x, modal_y, modal_width, actual_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(Span::styled(" Update Summary ", theme::title()))
        .borders(Borders::ALL)
        .border_style(theme::border_active());

    let content = Paragraph::new(lines).block(block);
    frame.render_widget(content, modal_area);
}
