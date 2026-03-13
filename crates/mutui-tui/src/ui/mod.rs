mod player_bar;
mod now_playing;
mod queue_panel;
mod playlists;
mod search;

use crate::app::App;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tabs
            Constraint::Min(5),    // main content
            Constraint::Length(1), // one-line shortcuts bar
        ])
        .split(frame.area());

    render_tabs(frame, app, chunks[0]);

    let small_screen = frame.area().width < 80 || frame.area().height < 28;

    if small_screen {
        let main = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(44),
                Constraint::Percentage(25),
                Constraint::Percentage(31),
            ])
            .split(chunks[1]);

        match app.view {
            crate::app::View::Search => search::render(frame, app, main[0]),
            crate::app::View::Playlists => playlists::render(frame, app, main[0]),
        }

        now_playing::render(frame, app, main[1], false);
        queue_panel::render(frame, app, main[2]);
    } else {
        let main = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(chunks[1]);

        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(47), Constraint::Percentage(53)])
            .split(main[1]);

        match app.view {
            crate::app::View::Search => search::render(frame, app, main[0]),
            crate::app::View::Playlists => playlists::render(frame, app, main[0]),
        }

        now_playing::render(frame, app, right[0], false);
        queue_panel::render(frame, app, right[1]);
    }

    if app.input_mode == crate::app::InputMode::PlaylistName {
        playlists::render_name_input_overlay(frame, app, frame.area());
    }

    if app.playlist_delete_confirm_name.is_some() {
        render_delete_playlist_confirm_popup(frame, app);
    }

    player_bar::render(frame, app, chunks[2]);

    if app.show_shortcuts_popup {
        render_shortcuts_popup(frame);
    }
}

fn render_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = crate::app::View::all()
        .iter()
        .map(|v| {
            let style = if *v == app.view {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::styled(format!(" {} ", v.label()), style)
        })
        .collect();

    let tabs = Tabs::new(titles)
        .divider(Span::styled("│", Style::default().fg(Color::DarkGray)))
        .highlight_style(Style::default().fg(Color::Cyan));

    frame.render_widget(tabs, area);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(height),
            Constraint::Fill(1),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn render_shortcuts_popup(frame: &mut Frame) {
    let area = centered_rect(72, 19, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Help ")
        .title_bottom(Line::from(" ? / Esc close ").fg(Color::DarkGray))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(Color::Black));

    let header = Row::new(vec![
        Cell::from("Shortcut").style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Cell::from("Description")
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]);

    let rows = vec![
        Row::new(vec!["Space", "Play / pause"]),
        Row::new(vec!["n / p", "Next / previous track"]),
        Row::new(vec!["<- / ->", "Seek backward / forward"]),
        Row::new(vec!["+ / -", "Volume up / down"]),
        Row::new(vec!["", ""]),
        Row::new(vec!["J / K", "Select queue item"]),
        Row::new(vec!["T", "Play selected queue item"]),
        Row::new(vec!["D", "Remove selected queue item"]),
        Row::new(vec!["H / L", "Move selected queue item"]),
        Row::new(vec!["", ""]),
        Row::new(vec!["/", "Focus search input"]),
        Row::new(vec!["Enter", "Play selected search result"]),
        Row::new(vec!["a", "Add selected search result to queue"]),
        Row::new(vec!["", ""]),
        Row::new(vec!["Enter", "Expand / collapse selected playlist"]),
        Row::new(vec!["l", "Load selected playlist into queue"]),
        Row::new(vec!["d", "Delete selected playlist or playlist track"]),
        Row::new(vec!["s", "Save queue as playlist"]),
        Row::new(vec!["", ""]),
        Row::new(vec!["Tab", "Switch between tabs"]),
        Row::new(vec!["q", "Quit app"]),
        Row::new(vec!["Q", "Shutdown daemon"]),
    ];

    let table = Table::new(rows, [Constraint::Length(10), Constraint::Fill(1)])
        .header(header)
        .column_spacing(2)
        .block(block)
        .style(Style::default().fg(Color::Gray));

    frame.render_widget(table, area);
}

fn render_delete_playlist_confirm_popup(frame: &mut Frame, app: &App) {
    let Some(name) = app.playlist_delete_confirm_name.as_deref() else {
        return;
    };

    let area = centered_rect(56, 6, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Confirm Delete ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Fill(1), Constraint::Length(2), Constraint::Fill(1)])
        .split(inner);

    let text = Paragraph::new(vec![
        Line::from(format!("Delete playlist '{name}'?")),
        Line::from("Enter/Y confirm  Esc/N cancel"),
    ])
    .style(Style::default().fg(Color::Gray))
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true });

    frame.render_widget(text, rows[1]);
}
