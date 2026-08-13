use crate::app::App;
use crate::mouse::{ButtonAction, HitRegion, MouseMap};
use ratatui::prelude::*;
use ratatui::widgets::*;
use unicode_width::UnicodeWidthStr;

pub fn render(frame: &mut Frame, app: &App, mouse_map: &mut MouseMap, area: Rect) {
    let autoplay_color = if app.status.autoplay_enabled {
        Color::Green
    } else {
        Color::DarkGray
    };
    let autoplay_state = if app.status.autoplay_enabled { "ON" } else { "OFF" };

    let sep = "  ·  ";
    let controls: Vec<(ButtonAction, &str, String)> = vec![
        (ButtonAction::TogglePlay, "Space", "Play/Pause".into()),
        (ButtonAction::Search, "/", "Search".into()),
        (ButtonAction::ToggleAutoplay, "A", format!("Auto:{autoplay_state}")),
        (ButtonAction::Help, "?", "Help".into()),
        (ButtonAction::Quit, "q", "Quit".into()),
    ];

    let mut spans: Vec<Span> = Vec::new();
    for (i, (_, key, label)) in controls.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(sep, Style::default().fg(Color::DarkGray)));
        }
        let fg = if *key == "A" { autoplay_color } else { Color::DarkGray };
        spans.push(Span::styled("[", Style::default().fg(fg)));
        spans.push(Span::styled(
            *key,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!("] {label}"), Style::default().fg(fg)));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center),
        area,
    );

    // Register clickable regions using the same centering as the line above.
    let total: usize = controls
        .iter()
        .map(|(_, key, label)| 3 + key.width() + label.width())
        .sum::<usize>()
        + sep.width() * controls.len().saturating_sub(1);
    let mut x = area.x + area.width.saturating_sub(total as u16).saturating_div(2);
    for (action, key, label) in controls.iter() {
        let width = (3 + key.width() + label.width()) as u16;
        mouse_map.push(HitRegion::Button(*action, Rect::new(x, area.y, width, 1)));
        x += width + sep.width() as u16;
    }
}
