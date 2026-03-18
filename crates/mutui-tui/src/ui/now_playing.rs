use crate::app::App;
use mutui_common::PlayerState;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect, compact_mode: bool) {
    let block = Block::default()
        .title(" Now Playing ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 24 || inner.height < 10 {
        render_compact(frame, app, inner);
        return;
    }

    if compact_mode {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(58),
                Constraint::Percentage(42),
                Constraint::Min(1),
            ])
            .split(inner);

        render_now_playing_top(frame, app, chunks[0]);
        render_queue_compact(frame, app, chunks[1], true);
    } else {
        render_now_playing_top(frame, app, inner);
    }
}

fn render_compact(frame: &mut Frame, app: &App, area: Rect) {
    let state = status_text(app.status.state);
    let text = if let Some(track) = &app.status.current_track {
        format!("{}\n{}\n{}", state, track.title, track.artist)
    } else {
        format!("{}\nNo track playing", state)
    };
    let p = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

fn render_now_playing_top(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(area);

    render_track_title(frame, app, chunks[1]);
    render_meta_and_progress(frame, app, chunks[3]);

    let state = status_text(app.status.state);
    let color = match app.status.state {
        PlayerState::Playing => Color::Green,
        PlayerState::Paused => Color::Yellow,
        PlayerState::Stopped => Color::DarkGray,
    };
    let pill = Line::from(vec![
        Span::styled("[", Style::default().fg(Color::DarkGray)),
        Span::styled(state, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::styled("]", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(pill), chunks[5]);
}

fn render_track_title(frame: &mut Frame, app: &App, area: Rect) {
    let p = if let Some(track) = &app.status.current_track {
        let state = match app.status.state {
            PlayerState::Playing => "▶",
            PlayerState::Paused => "⏸",
            PlayerState::Stopped => "⏹",
        };

        Paragraph::new(Line::from(vec![
            Span::styled(format!(" {state} "), Style::default().fg(Color::Cyan)),
            Span::styled(
                track.title.as_str(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]))
        .wrap(Wrap { trim: false })
    } else {
        Paragraph::new(Line::from(Span::styled(
            " ⏹ No track playing",
            Style::default().fg(Color::DarkGray),
        )))
    };

    frame.render_widget(p, area);
}

fn render_meta_and_progress(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let artist = app
        .status
        .current_track
        .as_ref()
        .map(|t| t.artist.clone())
        .unwrap_or_else(|| "-".to_string());

    frame.render_widget(
        Paragraph::new(artist)
            .style(Style::default().fg(Color::Gray))
            .wrap(Wrap { trim: false }),
        chunks[0],
    );

    let time = format!(
        "{} / {}",
        format_time(app.status.position),
        format_time(app.status.duration)
    );
    let volume = format!("Vol {}%", app.status.volume);
    let row = Line::from(vec![
        Span::styled(time, Style::default().fg(Color::DarkGray)),
        Span::styled("  ", Style::default()),
        Span::styled(volume, Style::default().fg(Color::Gray)),
    ]);
    frame.render_widget(Paragraph::new(row), chunks[2]);

    let ratio = if app.status.duration > 0.0 {
        (app.status.position / app.status.duration).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .ratio(ratio)
        .label(" ");
    frame.render_widget(gauge, chunks[4]);
}

fn render_queue_compact(frame: &mut Frame, app: &App, area: Rect, two_columns: bool) {
    let block = Block::default()
        .title(" Queue ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title_bottom(Line::from(" T=play  D=remove  J/K=navigate ").fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.status.queue.is_empty() {
        frame.render_widget(
            Paragraph::new("empty")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let max_rows = inner.height.saturating_sub(1) as usize;
    let center = app.queue_selected;
    let start = center.saturating_sub(max_rows / 2);
    let end = (start + max_rows).min(app.status.queue.len());

    let mut rendered_rows: Vec<Line> = Vec::new();
    for (idx, track) in app.status.queue[start..end].iter().enumerate() {
        let real_idx = start + idx;
        let current = real_idx == app.status.queue_index;
        let selected = real_idx == app.queue_selected;
        let prefix = match (current, selected) {
            (true, true) => "◆",
            (true, false) => "▶",
            (false, true) => "▸",
            (false, false) => " ",
        };
        let style = if current {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else if selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        rendered_rows.push(Line::from(vec![
            Span::styled(format!("{prefix}{:02} ", real_idx + 1), style),
            Span::styled(
                fit_inline(&track.title, inner.width.saturating_sub(5) as usize),
                if current {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else if selected {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Gray)
                },
            ),
        ]));
    }

    if two_columns && inner.width >= 42 {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);

        let mid = rendered_rows.len().div_ceil(2);
        let left_rows = rendered_rows[..mid].to_vec();
        let right_rows = rendered_rows[mid..].to_vec();

        frame.render_widget(Paragraph::new(left_rows), cols[0]);
        frame.render_widget(Paragraph::new(right_rows), cols[1]);
    } else {
        frame.render_widget(Paragraph::new(rendered_rows), inner);
    }
}

fn status_text(state: PlayerState) -> &'static str {
    match state {
        PlayerState::Playing => "PLAYING",
        PlayerState::Paused => "PAUSED",
        PlayerState::Stopped => "STOPPED",
    }
}

fn format_time(secs: f64) -> String {
    let total = secs as u64;
    let m = total / 60;
    let s = total % 60;
    format!("{m}:{s:02}")
}

fn fit_inline(input: &str, max: usize) -> String {
    if input.chars().count() <= max {
        return input.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let mut out = String::new();
    for c in input.chars().take(max - 1) {
        out.push(c);
    }
    out.push('…');
    out
}


