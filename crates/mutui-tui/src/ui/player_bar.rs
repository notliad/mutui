use crate::app::App;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    render_controls(frame, app, area);
}

fn render_controls(frame: &mut Frame, _app: &App, area: Rect) {
    let line = Line::from(vec![
        Span::styled(" Space", Style::default().fg(Color::DarkGray)),
        Span::styled(" play/pause  ", Style::default().fg(Color::DarkGray)),
        Span::styled("?", Style::default().fg(Color::DarkGray)),
        Span::styled(" help  ", Style::default().fg(Color::DarkGray)),
        Span::styled("q/Q", Style::default().fg(Color::DarkGray)),
        Span::styled(" close/shutdown", Style::default().fg(Color::DarkGray)),
    ]);

    frame.render_widget(
        Paragraph::new(line)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        area,
    );
}
