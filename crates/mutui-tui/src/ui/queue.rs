use crate::app::App;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    if app.status.queue.is_empty() {
        let p = Paragraph::new("Queue is empty - search tracks and add them to the queue")
                .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                        .title(" Playback Queue "),
            );
        frame.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .status
        .queue
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_current = i == app.status.queue_index;

            let prefix = if is_current { "♪ " } else { "  " };

            let duration = track
                .duration
                .map(|d| {
                    let m = d as u64 / 60;
                    let s = d as u64 % 60;
                    format!("{m}:{s:02}")
                })
                .unwrap_or_default();

            let title_style = if is_current {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let content = Line::from(vec![
                Span::styled(
                    format!("{prefix}{:2}. ", i + 1),
                    if is_current {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(&track.title, title_style),
                Span::raw(" "),
                Span::styled(
                    &track.artist,
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(duration, Style::default().fg(Color::DarkGray)),
            ]);

            ListItem::new(content)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Playback Queue ")
                .title_bottom(
                        Line::from(" Enter=play  d=remove  J/K=move  c=clear  s=save playlist ")
                        .fg(Color::DarkGray),
                ),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut state = ListState::default().with_selected(Some(
        app.queue_selected
            .min(app.status.queue.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, area, &mut state);
}
