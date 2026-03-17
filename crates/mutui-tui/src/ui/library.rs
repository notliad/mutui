use crate::app::App;
use ratatui::prelude::*;
use ratatui::widgets::*;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // folders header
            Constraint::Min(3),   // tracks list
        ])
        .split(area);

    render_folders(frame, app, chunks[0]);
    render_tracks(frame, app, chunks[1]);
}

fn render_folders(frame: &mut Frame, app: &App, area: Rect) {
    let folders_text = if app.library_folders.is_empty() {
        "No folders — press 'f' to add a music folder".to_string()
    } else {
        app.library_folders
            .iter()
            .map(|f| {
                // Show abbreviated path
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

fn render_tracks(frame: &mut Frame, app: &App, area: Rect) {
    if app.library_tracks.is_empty() {
        let help = if app.library_folders.is_empty() {
            "Add a music folder with 'f' to start browsing local files\nExample: '/home/user/Music'"
        } else {
            "No audio files found — press 'r' to rescan"
        };

        let p = Paragraph::new(help)
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

    let items: Vec<ListItem> = app
        .library_tracks
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
                    format!("{:3}. ", i + 1),
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
                .title(" Tracks ")
                .title_bottom(
                    Line::from(" Enter=play  a=add to queue  f=add folder  R=remove folder  r=rescan  o=open ")
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
        app.library_selected
            .min(app.library_tracks.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, area, &mut state);
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
