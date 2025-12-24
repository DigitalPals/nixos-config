//! Update screens

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, StepStatus};
use crate::ui::layout::progress_layout;
use crate::ui::theme;
use crate::ui::widgets::{LogView, ProgressSteps};

/// Draw running/complete update screen
pub fn draw_running(
    frame: &mut Frame,
    steps: &[StepStatus],
    output: &[String],
    complete: bool,
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

    let log = LogView::new(output).title(" Output ");
    frame.render_widget(log, output_area);

    // Footer
    let footer = if complete {
        Paragraph::new(Line::from(vec![
            Span::styled("[", theme::dim()),
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
