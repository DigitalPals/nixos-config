//! Scrollable log output widget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::ui::theme;

pub struct LogView<'a> {
    lines: &'a [String],
    title: Option<&'a str>,
    auto_scroll: bool,
    scroll_offset: Option<usize>,
}

impl<'a> LogView<'a> {
    pub fn new(lines: &'a [String]) -> Self {
        Self {
            lines,
            title: None,
            auto_scroll: true,
            scroll_offset: None,
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    #[allow(dead_code)]
    pub fn auto_scroll(mut self, auto_scroll: bool) -> Self {
        self.auto_scroll = auto_scroll;
        self
    }

    /// Set a manual scroll offset (disables auto_scroll)
    pub fn scroll_offset(mut self, offset: usize) -> Self {
        self.scroll_offset = Some(offset);
        self.auto_scroll = false;
        self
    }
}

impl Widget for LogView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border());

        if let Some(title) = self.title {
            block = block.title(Span::styled(title, theme::title()));
        }

        // Calculate visible lines based on rendered width so scrolling stays consistent.
        let inner_height = area.height.saturating_sub(2) as usize; // Account for borders
        let inner_width = area.width.saturating_sub(2) as usize;
        let mut visual_lines: Vec<String> = Vec::new();

        if inner_width > 0 {
            for line in self.lines {
                if line.is_empty() {
                    visual_lines.push(String::new());
                    continue;
                }

                let chars: Vec<char> = line.chars().collect();
                for chunk in chars.chunks(inner_width.max(1)) {
                    let rendered = chunk.iter().collect::<String>();
                    visual_lines.push(rendered);
                }
            }
        }

        let start = if let Some(offset) = self.scroll_offset {
            // Manual scroll mode: use provided offset
            offset.min(visual_lines.len().saturating_sub(1))
        } else if self.auto_scroll && visual_lines.len() > inner_height && inner_height > 0 {
            // Auto-scroll mode: show most recent lines
            visual_lines.len().saturating_sub(inner_height)
        } else {
            0
        };
        let end = (start + inner_height).min(visual_lines.len());

        let visible_lines: Vec<Line> = visual_lines[start..end]
            .iter()
            .map(|line| {
                // Simple color parsing for common patterns
                let style = if line.contains("[ERROR]") || line.contains("error:") {
                    theme::error()
                } else if line.contains("[WARN]") || line.contains("warning:") {
                    theme::warning()
                } else if line.contains("[SUCCESS]") || line.starts_with("✓") {
                    theme::success()
                } else if line.starts_with('>') || line.starts_with("  >") {
                    theme::info()
                } else {
                    theme::text()
                };
                Line::from(Span::styled(line.as_str(), style))
            })
            .collect();

        let paragraph = Paragraph::new(visible_lines).block(block);

        paragraph.render(area, buf);
    }
}
