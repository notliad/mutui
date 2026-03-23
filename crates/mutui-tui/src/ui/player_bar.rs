use crate::app::App;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    render_controls(frame, app, area);
}

fn render_controls(frame: &mut Frame, app: &App, area: Rect) {
    let autoplay_color = if app.status.autoplay_enabled {
        Color::Green
    } else {
        Color::DarkGray
    };
    let autoplay_state = if app.status.autoplay_enabled { "ON" } else { "OFF" };

    let sep = Span::styled("  ·  ", Style::default().fg(Color::DarkGray));
    let kb = |key: &'static str| Span::styled(key, Style::default().fg(Color::White));
    let lb = Span::styled("[", Style::default().fg(Color::DarkGray));
    let rb_label = |s: &'static str| Span::styled(format!("] {s}"), Style::default().fg(Color::DarkGray));

    let line = Line::from(vec![
        lb.clone(), kb("Space"), rb_label("Play/Pause"),
        sep.clone(),
        lb.clone(), kb("/"), rb_label("Search"),
        sep.clone(),
        Span::styled("[", Style::default().fg(autoplay_color)),
        Span::styled("A", Style::default().fg(autoplay_color).add_modifier(Modifier::BOLD)),
        Span::styled(format!("] Auto:{autoplay_state}"), Style::default().fg(autoplay_color)),
        sep.clone(),
        lb.clone(), kb("?"), rb_label("Help"),
        sep.clone(),
        lb.clone(), kb("q"), rb_label("Quit"),
    ]);

    frame.render_widget(
        Paragraph::new(line)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        area,
    );
}