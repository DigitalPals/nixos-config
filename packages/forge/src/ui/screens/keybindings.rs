//! Keybindings viewer screen

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::{App, Keybinding, KeybindingsPanel};
use crate::commands::keybindings::filter_by_category;
use crate::ui::theme;

/// Draw the loading screen
pub fn draw_loading(frame: &mut Frame, app: &App) {
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
    let header = Paragraph::new(Line::from(Span::styled(
        " Keybindings ",
        theme::title(),
    )))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border_active()),
    );
    frame.render_widget(header, chunks[0]);

    // Loading message with spinner
    let spinner_char = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏']
        [app.spinner_state % 10];
    let content = Paragraph::new(Line::from(vec![
        Span::styled(format!("{} ", spinner_char), theme::text()),
        Span::styled("Loading keybindings...", theme::text()),
    ]))
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border()),
    );
    frame.render_widget(content, chunks[1]);
}

/// Draw the keybindings viewer
pub fn draw_viewing(
    frame: &mut Frame,
    bindings: &[Keybinding],
    categories: &[String],
    selected_category: usize,
    selected_binding: usize,
    scroll_offset: usize,
    shell: &str,
    focus: KeybindingsPanel,
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

    // Header with shell name
    let header_text = format!(" Keybindings ({}) ", shell);
    let header = Paragraph::new(Line::from(Span::styled(header_text, theme::title())))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border_active()),
        );
    frame.render_widget(header, chunks[0]);

    // Main content area: categories on left, bindings on right
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(40)])
        .split(chunks[1]);

    // Categories list
    let categories_focused = focus == KeybindingsPanel::Categories;
    draw_categories(frame, content_chunks[0], categories, selected_category, categories_focused);

    // Bindings table
    let current_category = categories.get(selected_category).map(|s| s.as_str()).unwrap_or("");
    let filtered: Vec<&Keybinding> = filter_by_category(bindings, current_category);
    let bindings_focused = focus == KeybindingsPanel::Bindings;
    draw_bindings_table(frame, content_chunks[1], &filtered, selected_binding, scroll_offset, bindings_focused);

    // Footer
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("[", theme::dim()),
        Span::styled("Tab", theme::key_hint()),
        Span::styled("] Switch panel  [", theme::dim()),
        Span::styled("↑↓", theme::key_hint()),
        Span::styled("] Navigate  [", theme::dim()),
        Span::styled("Esc", theme::key_hint()),
        Span::styled("] Back", theme::dim()),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

fn draw_categories(frame: &mut Frame, area: Rect, categories: &[String], selected: usize, focused: bool) {
    let items: Vec<Line> = categories
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let style = if i == selected && focused {
                theme::selected()
            } else if i == selected {
                // Selected but not focused - just highlight
                Style::default().fg(theme::PRIMARY)
            } else {
                theme::text()
            };
            let prefix = if i == selected { "> " } else { "  " };
            Line::from(Span::styled(format!("{}{}", prefix, cat), style))
        })
        .collect();

    let border_style = if focused {
        theme::border_active()
    } else {
        theme::border()
    };

    let list = Paragraph::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(" Categories ", if focused { theme::title() } else { theme::dim() })),
    );
    frame.render_widget(list, area);
}

fn draw_bindings_table(
    frame: &mut Frame,
    area: Rect,
    bindings: &[&Keybinding],
    selected: usize,
    scroll_offset: usize,
    focused: bool,
) {
    // Calculate visible area
    let inner_height = area.height.saturating_sub(3) as usize; // Account for borders and header row

    // Create table rows
    let rows: Vec<Row> = bindings
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(inner_height)
        .map(|(i, binding)| {
            let style = if i == selected && focused {
                Style::default()
                    .fg(ratatui::style::Color::Black)
                    .bg(theme::PRIMARY)
                    .add_modifier(Modifier::BOLD)
            } else if i == selected {
                // Selected but not focused
                Style::default()
                    .fg(theme::PRIMARY)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme::TEXT)
            };

            // Format key display
            let key_display = if binding.modifiers.is_empty() || binding.modifiers == "," {
                binding.key.clone()
            } else {
                format!("{}+{}", binding.modifiers.replace(" ", "+"), binding.key)
            };

            Row::new(vec![
                Cell::from(key_display),
                Cell::from(binding.description.clone()),
            ])
            .style(style)
        })
        .collect();

    let header = Row::new(vec![
        Cell::from(Span::styled("Key", Style::default().add_modifier(Modifier::BOLD))),
        Cell::from(Span::styled("Description", Style::default().add_modifier(Modifier::BOLD))),
    ])
    .style(Style::default().fg(theme::DIM));

    let widths = [
        Constraint::Length(25),
        Constraint::Min(30),
    ];

    let border_style = if focused {
        theme::border_active()
    } else {
        theme::border()
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(Span::styled(
                    format!(" Bindings ({}) ", bindings.len()),
                    if focused { theme::title() } else { theme::dim() },
                )),
        );

    frame.render_widget(table, area);
}
