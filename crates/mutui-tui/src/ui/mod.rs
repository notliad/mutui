mod player_bar;
mod now_playing;
mod queue_panel;
mod playlists;
mod podcasts;
mod search;
mod library;

use crate::app::App;
use crate::mouse::{ButtonAction, HitRegion, ListId, MouseMap};
use ratatui::prelude::*;
use ratatui::widgets::*;
use unicode_width::UnicodeWidthStr;

pub fn render(frame: &mut Frame, app: &App, mouse_map: &mut MouseMap) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tabs
            Constraint::Min(5),    // main content
            Constraint::Length(1), // one-line shortcuts bar
        ])
        .split(frame.area());

    render_tabs(frame, app, mouse_map, chunks[0]);

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
            crate::app::View::Search => search::render(frame, app, mouse_map, main[0]),
            crate::app::View::Playlists => playlists::render(frame, app, mouse_map, main[0]),
            crate::app::View::Library => library::render(frame, app, mouse_map, main[0]),
            crate::app::View::Podcasts => podcasts::render(frame, app, mouse_map, main[0]),
        }

        now_playing::render(frame, app, mouse_map, main[1], false);
        queue_panel::render(frame, app, mouse_map, main[2]);
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
            crate::app::View::Search => search::render(frame, app, mouse_map, main[0]),
            crate::app::View::Playlists => playlists::render(frame, app, mouse_map, main[0]),
            crate::app::View::Library => library::render(frame, app, mouse_map, main[0]),
            crate::app::View::Podcasts => podcasts::render(frame, app, mouse_map, main[0]),
        }

        now_playing::render(frame, app, mouse_map, right[0], false);
        queue_panel::render(frame, app, mouse_map, right[1]);
    }

    if app.input_mode == crate::app::InputMode::PlaylistName {
        playlists::render_name_input_overlay(frame, app, mouse_map, frame.area());
    }

    if app.input_mode == crate::app::InputMode::LibraryFolderPath {
        library::render_folder_input_overlay(frame, app, mouse_map, frame.area());
    }

    if app.playlist_delete_confirm_name.is_some() {
        render_delete_playlist_confirm_popup(frame, app, mouse_map);
    }

    if app.library_delete_confirm_selected.is_some() {
        render_delete_library_folder_select_popup(frame, app, mouse_map);
    }

    player_bar::render(frame, app, mouse_map, chunks[2]);

    if app.show_shortcuts_popup {
        render_shortcuts_popup(frame, app, mouse_map);
    }
}

fn render_tabs(frame: &mut Frame, app: &App, mouse_map: &mut MouseMap, area: Rect) {
    let selected_idx = crate::app::View::all()
        .iter()
        .position(|v| *v == app.view)
        .unwrap_or(0);

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
        .select(selected_idx)
        .divider(Span::styled("│", Style::default().fg(Color::DarkGray)))
        .highlight_style(Style::default().fg(Color::Cyan));

    frame.render_widget(tabs, area);

    // Register clickable regions for each tab, replicating the Tabs layout
    // (padding of one space each side, "│" divider between tabs).
    let mut x = area.x;
    let views = crate::app::View::all();
    for (i, view) in views.iter().enumerate() {
        let width = format!(" {} ", view.label()).width() + 2;
        mouse_map.push(HitRegion::Tab(Rect::new(x, area.y, width as u16, 1), *view));
        x += width as u16;
        if i + 1 < views.len() {
            x += 1; // divider
        }
    }
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

fn render_shortcuts_popup(frame: &mut Frame, app: &App, mouse_map: &mut MouseMap) {
    let (title, is_shortcuts) = match app.help_popup_page {
        crate::app::HelpPopupPage::Shortcuts => (" Help - Shortcuts ", true),
        crate::app::HelpPopupPage::About => (" Help - About ", false),
    };
    let (percent, height) = if is_shortcuts { (72, 24) } else { (72, 16) };
    let area = centered_rect(percent, height, frame.area());
    mouse_map.push(HitRegion::Popup(area));

    if is_shortcuts {
        render_shortcuts_page(frame, title);
    } else {
        render_about_page(frame, title);
    }
}

fn render_shortcuts_page(frame: &mut Frame, title: &str) {
    let area = centered_rect(72, 24, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(title)
        .title_bottom(
            Line::from(" Tab/Shift+Tab: Shortcuts <-> About  ?/Esc close ").fg(Color::DarkGray),
        )
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
        Row::new(vec!["o", "Open current track externally"]),
        Row::new(vec!["A", "Toggle automatic playback from last search"]),
        Row::new(vec!["", ""]),
        Row::new(vec!["J / K", "Select queue item"]),
        Row::new(vec!["T", "Play selected queue item"]),
        Row::new(vec!["D", "Remove selected queue item"]),
        Row::new(vec!["H / L", "Move selected queue item"]),
        Row::new(vec!["", ""]),
        Row::new(vec!["/", "Focus search input"]),
        Row::new(vec!["Ctrl+J / Ctrl+K", "Jump between track and playlist sections"]),
        Row::new(vec!["j / k", "Navigate inside current section"]),
        Row::new(vec!["Enter / -> / l", "Tracks: play | Playlists: open/close folder"]),
        Row::new(vec!["<- / h", "Close opened playlist folder"]),
        Row::new(vec!["a", "Tracks: queue track | Playlists: queue all tracks"]),
        Row::new(vec!["", ""]),
        Row::new(vec!["Enter / -> / l", "Open selected playlist"]),
        Row::new(vec!["Enter / <- / h", "Close selected playlist"]),
        Row::new(vec!["a", "Load selected playlist into queue"]),
        Row::new(vec!["d", "Delete selected playlist or playlist track"]),
        Row::new(vec!["s", "Save queue as playlist"]),
        Row::new(vec!["", ""]),
        Row::new(vec!["f", "Add library folder (Library tab)"]),
        Row::new(vec!["R", "Choose and remove folder (Library tab)"]),
        Row::new(vec!["r", "Rescan library (Library tab)"]),
        Row::new(vec!["", ""]),
        Row::new(vec!["Tab", "Switch between tabs"]),
        Row::new(vec!["q", "Close app"]),
        Row::new(vec!["Q", "Shutdown"]),
    ];

    let table = Table::new(rows, [Constraint::Length(10), Constraint::Fill(1)])
        .header(header)
        .column_spacing(2)
        .block(block)
        .style(Style::default().fg(Color::Gray));

    frame.render_widget(table, area);
}

fn render_about_page(frame: &mut Frame, title: &str) {
    let version = env!("CARGO_PKG_VERSION");
    let license = option_env!("CARGO_PKG_LICENSE").unwrap_or("not specified");
    let repository = "https://github.com/notliad/mutui";

    let area = centered_rect(72, 16, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(title)
        .title_bottom(
            Line::from(" Tab/Shift+Tab: Shortcuts <-> About  ?/Esc close ").fg(Color::DarkGray),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let text = vec![
        Line::from(vec![
            Span::styled("mutui", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!(" v{version}"),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(""),
        Line::from("Terminal client for the mutui music daemon."),
        Line::from("Browse your library, search, manage playlists and queue playback."),
        Line::from(""),
        Line::from(vec![
            Span::styled("License: ", Style::default().fg(Color::DarkGray)),
            Span::styled(license, Style::default().fg(Color::Gray)),
        ]),
        Line::from(vec![
            Span::styled("Repo: ", Style::default().fg(Color::DarkGray)),
            Span::styled(repository, Style::default().fg(Color::LightBlue)),
        ]),
    ];

    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left)
        .style(Style::default().fg(Color::Gray));

    frame.render_widget(paragraph, inner);
}

fn render_delete_playlist_confirm_popup(frame: &mut Frame, app: &App, mouse_map: &mut MouseMap) {
    let Some(name) = app.playlist_delete_confirm_name.as_deref() else {
        return;
    };

    let area = centered_rect(56, 7, frame.area());
    frame.render_widget(Clear, area);
    mouse_map.push(HitRegion::Popup(area));

    let block = Block::default()
        .title(" Confirm Delete ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(inner);

    let text = Paragraph::new(Line::from(format!("Delete playlist '{name}'?")))
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(text, rows[1]);

    render_button_row(
        frame,
        mouse_map,
        rows[2],
        &[
            (ButtonAction::Confirm, "[Y] Confirm"),
            (ButtonAction::Cancel, "[N] Cancel"),
        ],
    );
}

fn render_delete_library_folder_select_popup(frame: &mut Frame, app: &App, mouse_map: &mut MouseMap) {
    let Some(selected) = app.library_delete_confirm_selected else {
        return;
    };

    let area = centered_rect(70, 14, frame.area());
    frame.render_widget(Clear, area);
    mouse_map.push(HitRegion::Popup(area));

    let block = Block::default()
        .title(" Remove Library Folder ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(4), Constraint::Length(1)])
        .split(inner);

    let title = Paragraph::new("Select a folder to remove")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    frame.render_widget(title, rows[0]);

    let items: Vec<ListItem> = app
        .library_folders
        .iter()
        .map(|folder| ListItem::new(Line::from(folder.as_str())))
        .collect();

    let list = List::new(items)
        .highlight_symbol("▸ ")
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    let mut state = ListState::default().with_selected(Some(
        selected.min(app.library_folders.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, rows[1], &mut state);
    mouse_map.push_list(
        ListId::LibraryFolderDelete,
        rows[1],
        state.offset(),
        app.library_folders.len(),
    );

    render_button_row(
        frame,
        mouse_map,
        rows[2],
        &[
            (ButtonAction::RemoveFolder, "[Enter] Remove"),
            (ButtonAction::Cancel, "[Esc] Cancel"),
        ],
    );
}

/// Render a centered row of clickable buttons and register their hit regions.
fn render_button_row(
    frame: &mut Frame,
    mouse_map: &mut MouseMap,
    area: Rect,
    buttons: &[(ButtonAction, &'static str)],
) {
    let separator_width = 4;
    let total = buttons
        .iter()
        .map(|(_, label)| label.width())
        .sum::<usize>()
        + separator_width * buttons.len().saturating_sub(1);

    let mut x = area.x + area.width.saturating_sub(total as u16).saturating_div(2);
    for (action, label) in buttons.iter() {
        let width = label.width() as u16;
        let button_area = Rect::new(x, area.y, width, 1);
        mouse_map.push(HitRegion::Button(*action, button_area));
        frame.render_widget(
            Paragraph::new(Span::styled(*label, Style::default().fg(Color::Yellow))),
            button_area,
        );
        x += width + separator_width as u16;
    }
}
