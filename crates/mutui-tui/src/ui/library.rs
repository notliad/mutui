use crate::app::{App, InputMode, LibraryMode};
use mutui_common::Track;
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::collections::BTreeMap;

// --- Data helpers ---

fn filtered_tracks<'a>(tracks: &'a [Track], filter: &str) -> Vec<&'a Track> {
    if filter.is_empty() {
        tracks.iter().collect()
    } else {
        let f = filter.to_lowercase();
        tracks
            .iter()
            .filter(|t| {
                t.title.to_lowercase().contains(&f)
                    || t.artist.to_lowercase().contains(&f)
                    || t.album
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&f)
            })
            .collect()
    }
}

fn grouped_by_artist<'a>(tracks: &'a [Track], filter: &str) -> Vec<(String, Vec<&'a Track>)> {
    let mut map: BTreeMap<String, Vec<&Track>> = BTreeMap::new();
    for t in tracks {
        map.entry(t.artist.clone()).or_default().push(t);
    }
    let f = filter.to_lowercase();
    map.into_iter()
        .filter(|(name, _)| f.is_empty() || name.to_lowercase().contains(&f))
        .collect()
}

fn grouped_by_album<'a>(tracks: &'a [Track], filter: &str) -> Vec<(String, Vec<&'a Track>)> {
    let mut map: BTreeMap<String, Vec<&Track>> = BTreeMap::new();
    for t in tracks {
        let album = t.album.as_deref().unwrap_or("Unknown Album").to_string();
        map.entry(album).or_default().push(t);
    }
    let f = filter.to_lowercase();
    map.into_iter()
        .filter(|(name, _)| f.is_empty() || name.to_lowercase().contains(&f))
        .collect()
}

fn fmt_duration(d: Option<f64>) -> String {
    d.map(|d| {
        let m = d as u64 / 60;
        let s = d as u64 % 60;
        format!("{m}:{s:02}")
    })
    .unwrap_or_default()
}

#[derive(Clone, Copy)]
enum GroupKind {
    Artist,
    Album,
}

// --- Public render entry point ---

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Folders strip
            Constraint::Length(3), // Mode tabs + filter bar
            Constraint::Min(3),    // Main content
        ])
        .split(area);

    render_folders(frame, app, chunks[0]);
    render_mode_bar(frame, app, chunks[1]);

    match app.library_mode {
        LibraryMode::AllTracks => render_all_tracks(frame, app, chunks[2]),
        LibraryMode::ByArtist => render_grouped(frame, app, chunks[2], GroupKind::Artist),
        LibraryMode::ByAlbum => render_grouped(frame, app, chunks[2], GroupKind::Album),
    }
}

fn render_folders(frame: &mut Frame, app: &App, area: Rect) {
    let folders_text = if app.library_folders.is_empty() {
        "No folders — press 'f' to add a music folder".to_string()
    } else {
        app.library_folders
            .iter()
            .map(|f| {
                let home = std::env::var("HOME").unwrap_or_default();
                if f.starts_with(&home) {
                    format!("~{}", &f[home.len()..])
                } else {
                    f.clone()
                }
            })
            .collect::<Vec<_>>()
            .join("  │  ")
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Library Folders ");

    let p = Paragraph::new(format!("  {folders_text}"))
        .style(Style::default().fg(Color::Gray))
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(p, area);
}

fn render_mode_bar(frame: &mut Frame, app: &App, area: Rect) {
    let modes = [LibraryMode::ByArtist, LibraryMode::ByAlbum, LibraryMode::AllTracks];
    let mut spans: Vec<Span> = vec![Span::raw("  ")];
    for (i, mode) in modes.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  │  ", Style::default().fg(Color::DarkGray)));
        }
        if *mode == app.library_mode {
            spans.push(Span::styled(
                format!("▶ {}", mode.label()),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                mode.label().to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    if app.input_mode == InputMode::LibraryFilter || !app.library_filter.is_empty() {
        spans.push(Span::styled("    Filter: ", Style::default().fg(Color::Yellow)));
        spans.push(Span::styled(
            app.library_filter.clone(),
            Style::default().fg(Color::White),
        ));
        if app.input_mode == InputMode::LibraryFilter {
            spans.push(Span::styled("█", Style::default().fg(Color::Yellow)));
        }
    } else {
        spans.push(Span::styled(
            "    /=filter  m=mode",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let line = Line::from(spans);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    frame.render_widget(Paragraph::new(line).block(block), area);
}

// --- All Tracks mode ---

fn render_all_tracks(frame: &mut Frame, app: &App, area: Rect) {
    let tracks = filtered_tracks(&app.library_tracks, &app.library_filter);

    if tracks.is_empty() {
        let msg = if app.library_folders.is_empty() {
            "Add a music folder with 'f'\nExample: '/home/user/Music'"
        } else if !app.library_filter.is_empty() {
            "No tracks match the filter"
        } else {
            "No audio files found — press 'r' to rescan"
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Tracks "),
            );
        frame.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let duration = fmt_duration(track.duration);
            let album_span = if let Some(album) = &track.album {
                Span::styled(format!("  {album}"), Style::default().fg(Color::DarkGray))
            } else {
                Span::raw("")
            };
            let content = Line::from(vec![
                Span::styled(format!("{:3}. ", i + 1), Style::default().fg(Color::DarkGray)),
                Span::styled(track.title.as_str(), Style::default().fg(Color::White)),
                Span::raw("  "),
                Span::styled(track.artist.as_str(), Style::default().fg(Color::Cyan)),
                album_span,
                Span::raw("  "),
                Span::styled(duration, Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(content)
        })
        .collect();

    let sel = app.library_selected.min(tracks.len().saturating_sub(1));
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(format!(" Tracks ({}) ", tracks.len()))
                .title_bottom(
                    Line::from(
                        " Enter=play  a=queue  f=add folder  R=remove folder  r=rescan  o=open ",
                    )
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

    let mut state = ListState::default().with_selected(Some(sel));
    frame.render_stateful_widget(list, area, &mut state);
}

// --- Grouped mode (Artists / Albums) ---

fn render_grouped(frame: &mut Frame, app: &App, area: Rect, kind: GroupKind) {
    let groups: Vec<(String, Vec<&Track>)> = match kind {
        GroupKind::Artist => grouped_by_artist(&app.library_tracks, &app.library_filter),
        GroupKind::Album => grouped_by_album(&app.library_tracks, &app.library_filter),
    };
    let kind_label = match kind {
        GroupKind::Artist => "Artists",
        GroupKind::Album => "Albums",
    };

    if groups.is_empty() {
        let msg = if app.library_folders.is_empty() {
            "Add a music folder with 'f'"
        } else if !app.library_filter.is_empty() {
            "No results match the filter"
        } else {
            "No audio files found — press 'r' to rescan"
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(format!(" {kind_label} ")),
            );
        frame.render_widget(p, area);
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(area);

    let group_sel = app.library_group_selected.min(groups.len().saturating_sub(1));
    let selected_group = &groups[group_sel];
    let group_tracks = &selected_group.1;
    let track_sel = app
        .library_group_track_selected
        .min(group_tracks.len().saturating_sub(1));

    // Left panel
    {
        let focused = !app.library_group_focus;
        let border_color = if focused { Color::Cyan } else { Color::DarkGray };
        let items: Vec<ListItem> = groups
            .iter()
            .map(|(name, tracks)| {
                ListItem::new(Line::from(vec![
                    Span::styled(name.as_str(), Style::default().fg(Color::White)),
                    Span::styled(
                        format!("  ({})", tracks.len()),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color))
                    .title(format!(" {kind_label} ({}) ", groups.len()))
                    .title_bottom(
                        Line::from(" l/Enter=tracks  a=add all  j/k=navigate ").fg(Color::DarkGray),
                    ),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▸ ");

        let mut state = ListState::default().with_selected(Some(group_sel));
        frame.render_stateful_widget(list, cols[0], &mut state);
    }

    // Right panel
    {
        let focused = app.library_group_focus;
        let border_color = if focused { Color::Cyan } else { Color::DarkGray };

        if group_tracks.is_empty() {
            let p = Paragraph::new("No tracks")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color))
                        .title(format!(" {} — {} ", kind_label, selected_group.0)),
                );
            frame.render_widget(p, cols[1]);
        } else {
            let items: Vec<ListItem> = group_tracks
                .iter()
                .enumerate()
                .map(|(i, track)| {
                    let secondary = match kind {
                        GroupKind::Artist => track.album.as_deref().unwrap_or("").to_string(),
                        GroupKind::Album => track.artist.clone(),
                    };
                    let duration = fmt_duration(track.duration);
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{:3}. ", i + 1),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(track.title.as_str(), Style::default().fg(Color::White)),
                        Span::raw("  "),
                        Span::styled(secondary, Style::default().fg(Color::DarkGray)),
                        Span::raw("  "),
                        Span::styled(duration, Style::default().fg(Color::DarkGray)),
                    ]))
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color))
                        .title(format!(
                            " {} — {} ({}) ",
                            kind_label,
                            selected_group.0,
                            group_tracks.len()
                        ))
                        .title_bottom(
                            Line::from(" Enter=play  a=queue  h=back  o=open ").fg(Color::DarkGray),
                        ),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::DarkGray)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▸ ");

            let mut state = ListState::default().with_selected(if focused {
                Some(track_sel)
            } else {
                None
            });
            frame.render_stateful_widget(list, cols[1], &mut state);
        }
    }
}

pub fn render_folder_input_overlay(frame: &mut Frame, app: &App, area: Rect) {
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
            Constraint::Percentage(15),
            Constraint::Percentage(70),
            Constraint::Percentage(15),
        ])
        .split(popup[1]);

    let input = Paragraph::new(format!("  {}", app.library_folder_input))
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Folder Path ")
                .style(Style::default().bg(Color::Black)),
        );

    frame.render_widget(Clear, inner[1]);
    frame.render_widget(input, inner[1]);
    let max_cursor_x = inner[1].x + inner[1].width.saturating_sub(2);
    let desired_x = inner[1].x + app.library_folder_cursor as u16 + 3;
    let cursor_x = desired_x.min(max_cursor_x);
    frame.set_cursor_position(Position::new(cursor_x, inner[1].y + 1));
}
