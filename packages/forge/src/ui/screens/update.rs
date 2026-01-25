//! Update screens

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::app::{App, StepStatus, UpdateSummary};
use crate::ui::layout::progress_layout;
use crate::ui::theme;
use crate::ui::widgets::{LogView, ProgressSteps};

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
        Paragraph::new(Line::from(vec![
            Span::styled("[", theme::dim()),
            Span::styled("↑↓", theme::key_hint()),
            Span::styled("] Scroll  [", theme::dim()),
            Span::styled("Enter", theme::key_hint()),
            Span::styled("] Done  [", theme::dim()),
            Span::styled("q", theme::key_hint()),
            Span::styled("] Quit", theme::dim()),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled("[", theme::dim()),
            Span::styled("Ctrl+C", theme::key_hint()),
            Span::styled("] Cancel", theme::dim()),
        ]))
    }
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

/// Options for local changes resolution
const LOCAL_CHANGES_OPTIONS: &[&str] = &[
    "Overwrite - Discard all local changes",
    "Stash - Save changes, restore after update",
    "Cancel - Keep changes, abort update",
];

/// Draw local changes prompt dialog
pub fn draw_local_changes_prompt(frame: &mut Frame, changed_files: &[String], selected: usize) {
    let area = frame.area();

    // Calculate popup dimensions
    let popup_width = 60.min(area.width.saturating_sub(4));
    let file_list_height = changed_files.len().min(8) as u16;
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
            Constraint::Length(2),              // Title
            Constraint::Length(file_list_height + 2), // File list
            Constraint::Length(5),              // Options
            Constraint::Length(2),              // Footer
        ])
        .split(inner_area);

    // Draw border
    let border = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::warning())
        .title(Span::styled(" Local Changes Detected ", theme::warning()));
    frame.render_widget(border, popup_area);

    // Title/description
    let title = Paragraph::new(Line::from(vec![
        Span::styled("Your local repository has uncommitted changes:", theme::text()),
    ]))
    .alignment(Alignment::Left);
    frame.render_widget(title, chunks[0]);

    // File list
    let files: Vec<ListItem> = changed_files
        .iter()
        .take(8)
        .map(|f| {
            ListItem::new(Line::from(vec![
                Span::styled("  ", theme::dim()),
                Span::styled(f.clone(), theme::info()),
            ]))
        })
        .collect();

    let more_indicator = if changed_files.len() > 8 {
        format!("  ... and {} more", changed_files.len() - 8)
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
            ListItem::new(Line::from(Span::styled(format!("{}{}", prefix, opt), style)))
        })
        .collect();

    let options_list = List::new(options);
    frame.render_widget(options_list, chunks[2]);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("[", theme::dim()),
        Span::styled("↑↓", theme::key_hint()),
        Span::styled("] Navigate  [", theme::dim()),
        Span::styled("Enter", theme::key_hint()),
        Span::styled("] Select  [", theme::dim()),
        Span::styled("Esc", theme::key_hint()),
        Span::styled("] Cancel", theme::dim()),
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
    app: &App,
) {
    // Draw the log view in the background
    draw_running(frame, steps, output, true, scroll_offset, app);

    // Draw the summary modal on top
    draw_summary_modal(frame, summary);
}

/// Draw the summary modal overlay
fn draw_summary_modal(frame: &mut Frame, summary: &UpdateSummary) {
    let area = frame.area();

    // Calculate modal dimensions (60% width, auto height based on content)
    let modal_width = ((area.width as u32 * 3) / 5).min(70).max(50) as u16;

    // Calculate content height
    let mut content_lines = 0u16;
    content_lines += 2; // Header padding

    // Flake changes section
    if !summary.flake_changes.is_empty() {
        content_lines += 2; // Section header + blank
        content_lines += summary.flake_changes.len().min(5) as u16;
        if summary.flake_changes.len() > 5 {
            content_lines += 1; // "... and N more"
        }
        content_lines += 1; // Trailing blank
    }

    // Package changes section
    if !summary.package_changes.is_empty() {
        content_lines += 2; // Section header + blank
        content_lines += summary.package_changes.len().min(8) as u16;
        if summary.package_changes.len() > 8 {
            content_lines += 1; // "... and N more"
        }
        content_lines += 1; // Trailing blank
    }

    // CLI tools section
    let claude_changed = summary.claude_old.is_some()
        && summary.claude_new.is_some()
        && summary.claude_old != summary.claude_new;
    let codex_changed = summary.codex_old.is_some()
        && summary.codex_new.is_some()
        && summary.codex_old != summary.codex_new;
    if claude_changed || codex_changed {
        content_lines += 2; // Section header + blank
        if claude_changed {
            content_lines += 1;
        }
        if codex_changed {
            content_lines += 1;
        }
        content_lines += 1; // Trailing blank
    }

    // Reboot warning
    if !summary.reboot_reasons.is_empty() {
        content_lines += 2; // Warning + blank
    }

    // Status line for rebuild skipped/failed
    if summary.rebuild_skipped || summary.rebuild_failed {
        content_lines += 1;
    }

    // Footer
    content_lines += 3; // Separator + key hints

    // Minimum height for "nothing changed" case
    content_lines = content_lines.max(10);

    let modal_height = (content_lines + 2).min(area.height.saturating_sub(4)); // +2 for borders
    let modal_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
    let modal_y = area.y + (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(modal_x, modal_y, modal_width, modal_height);

    // Clear area behind modal
    frame.render_widget(Clear, modal_area);

    // Build content lines
    let mut lines = vec![Line::from("")];

    // Flake changes section
    if !summary.flake_changes.is_empty() {
        lines.push(Line::from(Span::styled("Flake Inputs:", theme::title())));
        for (i, change) in summary.flake_changes.iter().enumerate() {
            if i >= 5 {
                lines.push(Line::from(Span::styled(
                    format!("  ... and {} more", summary.flake_changes.len() - 5),
                    theme::dim(),
                )));
                break;
            }
            let commit_text = if change.total_commits == 1 {
                "commit".to_string()
            } else {
                "commits".to_string()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {:20}", change.name), theme::text()),
                Span::styled(format!("+{} {}", change.total_commits, commit_text), theme::success()),
            ]));
        }
        lines.push(Line::from(""));
    }

    // Package changes section
    if !summary.package_changes.is_empty() {
        let pkg_count = summary.package_changes.len();
        lines.push(Line::from(Span::styled(
            format!("Packages ({} changed):", pkg_count),
            theme::title(),
        )));
        for (i, (name, old, new)) in summary.package_changes.iter().enumerate() {
            if i >= 8 {
                lines.push(Line::from(Span::styled(
                    format!("  ... and {} more", pkg_count - 8),
                    theme::dim(),
                )));
                break;
            }
            // Truncate package name if too long
            let display_name = if name.len() > 25 {
                format!("{}...", &name[..22])
            } else {
                name.clone()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {:25}", display_name), theme::text()),
                Span::styled(format!("{} → {}", old, new), theme::info()),
            ]));
        }
        lines.push(Line::from(""));
    }

    // CLI tools section
    if claude_changed || codex_changed {
        lines.push(Line::from(Span::styled("CLI Tools:", theme::title())));
        if claude_changed {
            lines.push(Line::from(vec![
                Span::styled("  Claude Code       ", theme::text()),
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
            lines.push(Line::from(vec![
                Span::styled("  Codex CLI         ", theme::text()),
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
        lines.push(Line::from(""));
    }

    // Status for rebuild skipped/failed
    if summary.rebuild_skipped {
        lines.push(Line::from(Span::styled(
            "  System already up to date",
            theme::dim(),
        )));
    } else if summary.rebuild_failed {
        lines.push(Line::from(Span::styled(
            "  ✗ System rebuild failed",
            theme::error(),
        )));
    }

    // Reboot warning
    if !summary.reboot_reasons.is_empty() {
        lines.push(Line::from(""));
        let reasons = summary.reboot_reasons.join(", ");
        lines.push(Line::from(Span::styled(
            format!("⚠ Reboot recommended ({})", reasons),
            theme::warning(),
        )));
    }

    // "Nothing changed" case
    if summary.flake_changes.is_empty()
        && summary.package_changes.is_empty()
        && !claude_changed
        && !codex_changed
        && !summary.rebuild_failed
    {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  System is up to date - no changes",
            theme::success(),
        )));
        lines.push(Line::from(""));
    }

    // Add padding before footer
    while lines.len() < (modal_height.saturating_sub(5)) as usize {
        lines.push(Line::from(""));
    }

    // Footer with key hints
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "─".repeat((modal_width - 4) as usize),
        theme::dim(),
    )));

    // Build key hints based on available actions
    let mut key_hints = vec![
        Span::styled("[", theme::dim()),
        Span::styled("Enter", theme::key_hint()),
        Span::styled("] Done  [", theme::dim()),
        Span::styled("v", theme::key_hint()),
        Span::styled("] View log", theme::dim()),
    ];

    if !summary.reboot_reasons.is_empty() {
        key_hints.extend(vec![
            Span::styled("  [", theme::dim()),
            Span::styled("r", theme::key_hint()),
            Span::styled("] Reboot", theme::dim()),
        ]);
    }

    lines.push(Line::from(key_hints));

    let block = Block::default()
        .title(Span::styled(" Update Summary ", theme::title()))
        .borders(Borders::ALL)
        .border_style(theme::border_active());

    let content = Paragraph::new(lines).block(block);
    frame.render_widget(content, modal_area);
}
