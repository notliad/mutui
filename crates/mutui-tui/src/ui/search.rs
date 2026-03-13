use crate::app::{App, InputMode};
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search input
            Constraint::Min(3),   // results
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
        line.push(Span::raw("  Press / to search..."));
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

    let input = Paragraph::new(Line::from(line))
        .style(input_style)
        .block(
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
    if app.search_results.is_empty() {
        let help = if app.searching {
            "Searching..."
        } else if app.search_input.is_empty() {
            "Use / to start a search"
        } else {
            "No results found"
        };

        let p = Paragraph::new(help)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray))
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

            let content = Line::from(vec![
                Span::styled(
                    format!("{:2}. ", i + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(&track.title, Style::default().fg(Color::White)),
                Span::raw(" "),
                Span::styled(&track.artist, Style::default().fg(Color::DarkGray)),
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
                .title(" Results "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");

    let mut state = ListState::default().with_selected(Some(app.search_selected));
    frame.render_stateful_widget(list, area, &mut state);
}
