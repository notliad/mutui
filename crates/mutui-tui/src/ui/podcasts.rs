use crate::app::{App, PodcastSection};
use mutui_common::PodcastEpisode;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    // Split horizontally: channel list on the left, episode list on the right.
    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    render_channel_panel(frame, app, halves[0]);
    render_episode_panel(frame, app, halves[1]);
}

fn render_channel_panel(frame: &mut Frame, app: &App, area: Rect) {
    // Three vertical sections: search bar, search results, followed podcasts.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Min(3)])
        .split(area);

    render_search_bar(frame, app, chunks[0]);
    render_results_list(frame, app, chunks[1]);
    render_followed_list(frame, app, chunks[2]);
}

fn render_search_bar(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.podcast_input_mode;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let text = if app.podcast_searching {
        format!(" Searching…  {}", app.podcast_search_input)
    } else {
        format!("  {}", app.podcast_search_input)
    };
    let bar = Paragraph::new(text)
        .style(Style::default().fg(if focused { Color::Cyan } else { Color::White }))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Search Podcasts [/] "),
        );
    frame.render_widget(bar, area);
    if focused {
        let cursor_x = area.x + app.podcast_search_cursor as u16 + 3;
        let cursor_x = cursor_x.min(area.x + area.width.saturating_sub(2));
        frame.set_cursor_position(Position::new(cursor_x, area.y + 1));
    }
}

fn render_results_list(frame: &mut Frame, app: &App, area: Rect) {
    let is_active =
        !app.podcast_episode_focus && app.podcast_section == PodcastSection::Results;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    if app.podcast_search_results.is_empty() {
        let (text, color) = if let Some(err) = &app.podcast_last_error {
            (err.as_str(), Color::Red)
        } else if app.podcast_searching {
            ("Searching…", Color::DarkGray)
        } else if !app.podcast_search_input.is_empty() {
            ("No results", Color::DarkGray)
        } else {
            ("/ to search", Color::DarkGray)
        };
        let border = if color == Color::Red {
            Style::default().fg(Color::Red)
        } else {
            border_style
        };
        let p = Paragraph::new(text)
            .style(Style::default().fg(color))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border)
                    .title(" Results ")
                    .title_bottom(
                        Line::from(
                            " [f] follow  ",
                        )
                        .fg(Color::DarkGray),
                    ),
            );
        frame.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .podcast_search_results
        .iter()
        .enumerate()
        .map(|(i, ch)| {
            let is_followed = app.podcast_followed.iter().any(|f| f.feed_url == ch.feed_url);
            let marker = if is_followed { "★ " } else { "  " };
            let is_selected = i == app.podcast_result_selected;
            let style = if is_selected && is_active {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Yellow)),
                Span::styled(ch.title.as_str(), style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Results "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let sel = app
        .podcast_result_selected
        .min(app.podcast_search_results.len() - 1);
    let mut state = ListState::default().with_selected(Some(sel));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_followed_list(frame: &mut Frame, app: &App, area: Rect) {
    let is_active =
        !app.podcast_episode_focus && app.podcast_section == PodcastSection::Followed;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    if app.podcast_followed.is_empty() {
        let p = Paragraph::new("No followed podcasts\nf to follow selected result")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(" Followed "),
            );
        frame.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .podcast_followed
        .iter()
        .enumerate()
        .map(|(i, ch)| {
            let is_selected = i == app.podcast_followed_selected;
            let style = if is_selected && is_active {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(Line::from(vec![
                Span::styled("★ ", Style::default().fg(Color::Yellow)),
                Span::styled(ch.title.as_str(), style),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Followed ")
                .title_bottom(
                        Line::from(
                            " [f] unfollow  ",
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

    let sel = app
        .podcast_followed_selected
        .min(app.podcast_followed.len() - 1);
    let mut state = ListState::default().with_selected(Some(sel));
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_episode_panel(frame: &mut Frame, app: &App, area: Rect) {
    if app.podcast_episodes_loading {
        let p = Paragraph::new("Loading episodes…")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Episodes "),
            );
        frame.render_widget(p, area);
        return;
    }

    if app.podcast_episodes.is_empty() {
        let hint = if app.podcast_selected_feed.is_some() {
            "No episodes found"
        } else {
            "Select a podcast and press Enter"
        };
        let p = Paragraph::new(hint)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
                    .title(" Episodes "),
            );
        frame.render_widget(p, area);
        return;
    }

    // Split: filter bar on top, episode list below.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);

    render_episode_filter_bar(frame, app, chunks[0]);
    render_episode_list(frame, app, chunks[1]);
}

fn render_episode_filter_bar(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.podcast_episode_filter_mode;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let text = format!("  {}", app.podcast_episode_filter);
    let bar = Paragraph::new(text)
        .style(Style::default().fg(if focused { Color::Cyan } else { Color::White }))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Filter [f] "),
        );
    frame.render_widget(bar, area);
    if focused {
        let cursor_x = area.x + app.podcast_episode_filter_cursor as u16 + 3;
        let cursor_x = cursor_x.min(area.x + area.width.saturating_sub(2));
        frame.set_cursor_position(Position::new(cursor_x, area.y + 1));
    }
}

fn episode_items<'a>(app: &'a App) -> (Vec<ListItem<'a>>, usize) {
    let is_active = app.podcast_episode_focus;
    let filter = app.podcast_episode_filter.to_lowercase();
    let episodes: Vec<&PodcastEpisode> = if filter.is_empty() {
        app.podcast_episodes.iter().collect()
    } else {
        app.podcast_episodes
            .iter()
            .filter(|ep| ep.title.to_lowercase().contains(&filter))
            .collect()
    };
    let total = episodes.len();
    let items = episodes
        .into_iter()
        .enumerate()
        .map(|(i, ep)| {
            let duration = ep
                .duration
                .map(|d| {
                    let m = d as u64 / 60;
                    let s = d as u64 % 60;
                    format!("{m}:{s:02}")
                })
                .unwrap_or_default();
            let date = ep
                .pub_date
                .as_deref()
                .map(|d| if d.len() > 16 { &d[..16] } else { d })
                .unwrap_or("");
            let is_selected = i == app.podcast_episode_selected;
            let title_style = if is_selected && is_active {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let line = Line::from(vec![
                Span::styled(ep.title.as_str(), title_style),
                Span::raw("  "),
                Span::styled(duration, Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(date, Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(line)
        })
        .collect();
    (items, total)
}

fn render_episode_list(frame: &mut Frame, app: &App, area: Rect) {
    let is_active = app.podcast_episode_focus;
    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let (items, total) = episode_items(app);

    if items.is_empty() {
        let p = Paragraph::new("No episodes match the filter")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(" Episodes "),
            );
        frame.render_widget(p, area);
        return;
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Episodes ")
                .title_bottom(
                    Line::from(" [a] add  [Enter] play  [f] filter  ").fg(Color::DarkGray),
                ),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let sel = app.podcast_episode_selected.min(total - 1);
    let mut state = ListState::default().with_selected(Some(sel));
    frame.render_stateful_widget(list, area, &mut state);
}

