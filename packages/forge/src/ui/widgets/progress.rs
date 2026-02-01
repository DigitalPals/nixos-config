//! Multi-step progress widget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::app::{StepState, StepStatus};
use crate::ui::theme;
use crate::ui::widgets::Spinner;

pub struct ProgressSteps<'a> {
    steps: &'a [StepStatus],
    spinner_state: usize,
    title: Option<&'a str>,
}

impl<'a> ProgressSteps<'a> {
    pub fn new(steps: &'a [StepStatus], spinner_state: usize) -> Self {
        Self {
            steps,
            spinner_state,
            title: None,
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }
}

impl Widget for ProgressSteps<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut lines: Vec<Line> = Vec::new();
        for step in self.steps {
            let (icon, style) = match step.status {
                StepState::Pending => ("[ ]", theme::dim()),
                StepState::Running => {
                    let spinner = Spinner::new(self.spinner_state);
                    let elapsed_str = step.elapsed_secs().map(|s| {
                        if s >= 60 {
                            format!(" ({}m{}s)", s / 60, s % 60)
                        } else if s >= 5 {
                            format!(" ({}s)", s)
                        } else {
                            String::new()
                        }
                    }).unwrap_or_default();
                    lines.push(Line::from(vec![
                        Span::styled(format!(" [{}] ", spinner.char()), theme::info()),
                        Span::styled(&step.name, theme::text()),
                        Span::styled("...", theme::dim()),
                        Span::styled(elapsed_str, theme::dim()),
                    ]));
                    if let Some(ref detail) = step.detail {
                        lines.push(Line::from(vec![
                            Span::styled("       ", theme::dim()),
                            Span::styled(detail.clone(), theme::dim()),
                        ]));
                    }
                    continue;
                }
                StepState::Complete => ("[✓]", theme::success()),
                StepState::Failed => ("[✗]", theme::error()),
                StepState::Skipped => ("[-]", theme::dim()),
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", icon), style),
                Span::styled(&step.name, theme::text()),
            ]));
        }

        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme::border());

        if let Some(title) = self.title {
            block = block.title(Span::styled(title, theme::title()));
        }

        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(area, buf);
    }
}
