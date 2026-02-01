//! Shared helper functions for create host screens

use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::ui::layout::footer_hints;
use crate::ui::theme;

/// Draw a centered header with title
pub fn draw_header(frame: &mut Frame, area: Rect, title: &str) {
    let header = Paragraph::new(Line::from(Span::styled(title, theme::title())))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border_active()),
        );
    frame.render_widget(header, area);
}

/// Draw a footer with key hints
pub fn draw_footer(frame: &mut Frame, area: Rect, hints: &[(&str, &str)]) {
    let footer = Paragraph::new(footer_hints(hints)).alignment(Alignment::Center);
    frame.render_widget(footer, area);
}
