use crate::app::{App, InputMode, SearchSection};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search input
            Constraint::Min(3),    // results
        ])
        .split(area);

    render_input(frame, app, chunks[0]);
    render_results(frame, app, chunks[1]);
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let input_style = if app.input_mode == InputMode::Search {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };

    let spinner = match app.search_spinner_frame % 4 {
        0 => "⠋",
        1 => "⠙",
        2 => "⠹",
        _ => "⠸",
    };
    let searching_indicator = if app.searching {
        format!("  {spinner} searching...")
    } else {
        String::new()
    };

    let mut line: Vec<Span<'static>> = Vec::new();

    if app.search_input.is_empty() && app.input_mode != InputMode::Search {
        line.push(Span::raw("  Search for tracks, artists and playlists..."));
    } else {
        line.push(Span::raw("  "));
        if let Some((start, end)) = search_selection_range(app) {
            for (byte_idx, ch) in app.search_input.char_indices() {
                let selected = byte_idx >= start && byte_idx < end;
                let style = if selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };
                line.push(Span::styled(ch.to_string(), style));
            }
        } else {
            line.push(Span::raw(app.search_input.clone()));
        }
    }

    if !searching_indicator.is_empty() {
        line.push(Span::raw(searching_indicator));
    }

    let input = Paragraph::new(Line::from(line)).style(input_style).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if app.input_mode == InputMode::Search {
                Color::Cyan
            } else {
                Color::DarkGray
            }))
            .title(" Search "),
    );

    frame.render_widget(input, area);

    // Show cursor when in search mode
    if app.input_mode == InputMode::Search {
        frame.set_cursor_position(Position::new(
            area.x + app.search_cursor as u16 + 3,
            area.y + 1,
        ));
    }
}

fn search_selection_range(app: &App) -> Option<(usize, usize)> {
    let anchor = app.search_selection_anchor?;
    if anchor == app.search_cursor {
        return None;
    }
    if anchor < app.search_cursor {
        Some((anchor, app.search_cursor))
    } else {
        Some((app.search_cursor, anchor))
    }
}

fn render_results(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    render_track_results(frame, app, chunks[0]);
    render_playlist_results(frame, app, chunks[1]);
}

fn render_track_results(frame: &mut Frame, app: &App, area: Rect) {
    if app.search_results.is_empty() {
        let help = if app.searching {
            "Searching tracks..."
        } else if app.search_input.is_empty() {
            "Tracks results"
        } else {
            "No track results"
        };
        let p = Paragraph::new(help)
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(
                        if app.search_section == SearchSection::Tracks {
                            Color::Cyan
                        } else {
                            Color::DarkGray
                        },
                    ))
                    .title(" Results "),
            );
        frame.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .search_results
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let duration = track
                .duration
                .map(|d| {
                    let m = d as u64 / 60;
                    let s = d as u64 % 60;
                    format!("{m}:{s:02}")
                })
                .unwrap_or_default();

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:2}. ", i + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(track.title.as_str(), Style::default().fg(Color::White)),
                Span::raw(" "),
                Span::styled(track.artist.as_str(), Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(duration, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let mut state = ListState::default().with_selected(Some(app.search_selected));
    if app.search_section != SearchSection::Tracks {
        state.select(None);
    }
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(
                    Style::default().fg(if app.search_section == SearchSection::Tracks {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    }),
                )
                .title(" Results "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_playlist_results(frame: &mut Frame, app: &App, area: Rect) {
    if app.search_playlist_results.is_empty() {
        let help = if app.searching {
            "Searching playlists..."
        } else if app.search_input.is_empty() {
            "Playlist results"
        } else {
            "No playlist results"
        };
        let p = Paragraph::new(help)
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(
                        if app.search_section == SearchSection::Playlists {
                            Color::Cyan
                        } else {
                            Color::DarkGray
                        },
                    ))
                    .title(" Playlists "),
            );
        frame.render_widget(p, area);
        return;
    }

    let mut items: Vec<ListItem> = Vec::new();
    let mut selected_row = app.search_playlist_selected;

    for (i, playlist) in app.search_playlist_results.iter().enumerate() {
        let songs = playlist
            .album
            .as_deref()
            .and_then(|album| album.strip_prefix("youtube-playlist:"))
            .and_then(|count| count.parse::<u64>().ok())
            .map(|count| format!("{count} songs"))
            .unwrap_or_else(|| "songs: ?".to_string());

        let folder_prefix = if i == app.search_playlist_selected && app.search_playlist_expanded {
            "▾"
        } else {
            "▸"
        };

        let is_active_selection =
            app.search_section == SearchSection::Playlists && i == app.search_playlist_selected;

        let folder_line = Line::from(vec![
            Span::styled(
                format!("{folder_prefix} {:2}. ", i + 1),
                Style::default().fg(if is_active_selection {
                    Color::Cyan
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                playlist.title.as_str(),
                if is_active_selection {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
            Span::raw("  "),
            Span::styled("owner: ", Style::default().fg(Color::DarkGray)),
            Span::styled(playlist.artist.as_str(), Style::default().fg(Color::White)),
            Span::raw("  "),
            Span::styled(songs, Style::default().fg(Color::DarkGray)),
        ]);
        items.push(ListItem::new(folder_line));

        if i == app.search_playlist_selected && app.search_playlist_expanded {
            if app.search_playlist_loading {
                items.push(ListItem::new(Line::from(vec![
                    Span::styled("    └─ ", Style::default().fg(Color::DarkGray)),
                    Span::styled("loading tracks...", Style::default().fg(Color::Yellow)),
                ])));
            }

            for (t_idx, track) in app.search_playlist_tracks.iter().enumerate() {
                let duration = track
                    .duration
                    .map(|d| {
                        let m = d as u64 / 60;
                        let s = d as u64 % 60;
                        format!("{m}:{s:02}")
                    })
                    .unwrap_or_default();

                items.push(ListItem::new(Line::from(vec![
                    Span::styled("    └─ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        track.title.as_str(),
                        if app.search_playlist_track_focus
                            && app.search_playlist_track_selected == t_idx
                        {
                            Style::default().fg(Color::Yellow)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                    Span::styled("  ", Style::default()),
                    Span::styled(duration, Style::default().fg(Color::DarkGray)),
                ])));
            }

            if app.search_playlist_track_focus {
                selected_row =
                    app.search_playlist_selected + 1 + app.search_playlist_track_selected;
            }
        }
    }

    let mut state = ListState::default().with_selected(Some(selected_row));
    if app.search_section != SearchSection::Playlists {
        state.select(None);
    }
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(
                    if app.search_section == SearchSection::Playlists {
                        Color::Cyan
                    } else {
                        Color::DarkGray
                    },
                ))
                .title(" Playlists "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    frame.render_stateful_widget(list, area, &mut state);
}
