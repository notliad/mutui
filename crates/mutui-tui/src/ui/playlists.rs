use crate::app::{App, PlaylistView};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    match app.playlist_view {
        PlaylistView::List => render_list(frame, app, area),
    }

    // Overlay is rendered from ui::mod so it appears regardless of active tab.
}

fn render_list(frame: &mut Frame, app: &App, area: Rect) {
    if app.playlist_names.is_empty() {
        let p = Paragraph::new("No playlists yet - save the current queue with 's'")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Playlists "),
            );
        frame.render_widget(p, area);
        return;
    }

    let mut items: Vec<ListItem> = Vec::new();
    let mut selected_row = app.playlist_selected;

    for (i, name) in app.playlist_names.iter().enumerate() {
        let folder_prefix = if i == app.playlist_selected { "▾" } else { "▸" };
        let folder_line = Line::from(vec![
            Span::styled(
                format!("{folder_prefix} {:2}. ", i + 1),
                Style::default().fg(if i == app.playlist_selected {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                name.as_str(),
                if i == app.playlist_selected {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ]);
        items.push(ListItem::new(folder_line));

        if i == app.playlist_selected {
            for (t_idx, track) in app.playlist_tracks.iter().enumerate() {
                let duration = track
                    .duration
                    .map(|d| {
                        let m = d as u64 / 60;
                        let s = d as u64 % 60;
                        format!("{m}:{s:02}")
                    })
                    .unwrap_or_default();

                let line = Line::from(vec![
                    Span::styled("    └─ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        track.title.as_str(),
                        if app.playlist_track_focus && app.playlist_track_selected == t_idx {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                    Span::styled("  ", Style::default()),
                    Span::styled(duration, Style::default().fg(Color::DarkGray)),
                ]);
                items.push(ListItem::new(line));
            }

            if app.playlist_track_focus {
                selected_row = app.playlist_selected + 1 + app.playlist_track_selected;
            }
        }
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Playlists "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut state = ListState::default().with_selected(if app.playlist_names.is_empty() {
        None
    } else {
        Some(selected_row)
    });
    frame.render_stateful_widget(list, area, &mut state);
}

pub fn render_name_input_overlay(frame: &mut Frame, app: &App, area: Rect) {
    let popup = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .split(area);

    let inner = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(popup[1]);

    let input = Paragraph::new(format!("  {}", app.new_playlist_name))
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Playlist Name ")
                .style(Style::default().bg(Color::Black)),
        );

    frame.render_widget(Clear, inner[1]);
    frame.render_widget(input, inner[1]);
    let max_cursor_x = inner[1].x + inner[1].width.saturating_sub(2);
    let desired_x = inner[1].x + app.new_playlist_cursor as u16 + 3;
    let cursor_x = desired_x.min(max_cursor_x);
    frame.set_cursor_position(Position::new(cursor_x, inner[1].y + 1));
}
