mod app;
mod client;
mod ui;

use anyhow::Result;
use app::{App, InputMode, PlaylistView, View};
use client::DaemonClient;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use mutui_common::{Playlist, Request, Response};
use ratatui::prelude::*;
use std::time::Duration;
use tokio::task::JoinHandle;

#[tokio::main]
async fn main() -> Result<()> {
    // Connect to daemon (starting it if needed)
    let mut daemon = client::ensure_daemon().await?;

    // Setup terminal
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, &mut daemon).await;

    // Restore terminal
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    terminal.show_cursor()?;

    result
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    daemon: &mut DaemonClient,
) -> Result<()> {
    let mut app = App::new();
    let mut search_task: Option<JoinHandle<anyhow::Result<Response>>> = None;

    // Initial status fetch
    if let Ok(Response::Status(status)) = daemon.send(&Request::GetStatus).await {
        app.status = *status;
        clamp_queue_selection(&mut app);
    }
    // Initial playlist list
    if let Ok(Response::Playlists(names)) = daemon.send(&Request::ListPlaylists).await {
        app.playlist_names = names;
    }


    let mut tick_counter: u8 = 0;

    loop {
        terminal.draw(|frame| ui::render(frame, &app))?;

        if search_task
            .as_ref()
            .map(|task| task.is_finished())
            .unwrap_or(false)
        {
            if let Some(task) = search_task.take() {
                match task.await {
                    Ok(Ok(Response::SearchResults(results))) => {
                        app.search_results = results;
                        app.search_selected = 0;
                    }
                    Ok(Ok(Response::Error(e))) => {
                        app.notify(format!("Search error: {e}"));
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        app.notify(format!("Search error: {e}"));
                    }
                    Err(e) => {
                        app.notify(format!("Search task failed: {e}"));
                    }
                }
                app.searching = false;
            }
        }

        if search_task.is_none() {
            if let Some(query) = app.pending_search_query.take() {
                search_task = Some(tokio::spawn(async move {
                    crate::client::send_once(Request::Search(query)).await
                }));
            }
        }

        // Poll for events with timeout for status ticks
        let timeout = Duration::from_millis(50);
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                handle_key(&mut app, daemon, key).await?;
            }
        }

        if app.searching {
            app.search_spinner_frame = app.search_spinner_frame.wrapping_add(1) % 4;
        }

        // Periodic status update (~500ms)
        tick_counter += 1;
        if tick_counter >= 10 {
            tick_counter = 0;
            if let Ok(Response::Status(status)) =
                daemon.send(&Request::GetStatus).await
            {
                app.status = *status;
                clamp_queue_selection(&mut app);
            }
            app.tick_notification();
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

async fn handle_key(
    app: &mut App,
    daemon: &mut DaemonClient,
    key: event::KeyEvent,
) -> Result<()> {
    if app.playlist_delete_confirm_name.is_some() {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(name) = app.playlist_delete_confirm_name.take() {
                    let _ = daemon.send(&Request::DeletePlaylist(name.clone())).await;
                    app.playlist_names.retain(|n| *n != name);
                    app.playlist_selected = app
                        .playlist_selected
                        .min(app.playlist_names.len().saturating_sub(1));
                    app.playlist_track_focus = false;
                    refresh_selected_playlist(app, daemon).await;
                    app.notify(format!("Playlist '{name}' deleted"));
                }
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                app.playlist_delete_confirm_name = None;
                app.notify("Delete canceled");
            }
            _ => {}
        }
        return Ok(());
    }

    if app.show_shortcuts_popup {
        match key.code {
            KeyCode::Char('?') | KeyCode::Esc => {
                app.show_shortcuts_popup = false;
            }
            _ => {}
        }
        return Ok(());
    }

    // Handle input modes first
    match app.input_mode {
        InputMode::Search => {
            handle_search_input(app, daemon, key).await?;
            return Ok(());
        }
        InputMode::PlaylistName => {
            handle_playlist_name_input(app, daemon, key).await?;
            return Ok(());
        }
        InputMode::Normal => {}
    }

    match key.code {
        // Quit
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('Q') => {
            let _ = daemon.send(&Request::Shutdown).await;
            app.should_quit = true;
        }
        KeyCode::Char('?') => app.show_shortcuts_popup = true,

        // Tab navigation
        KeyCode::Tab => {
            app.view = app.view.next();
            if app.view == View::Playlists {
                refresh_selected_playlist(app, daemon).await;
            }
        }
        KeyCode::BackTab => {
            app.view = app.view.prev();
            if app.view == View::Playlists {
                refresh_selected_playlist(app, daemon).await;
            }
        }
        KeyCode::Char('1') => app.view = View::Search,
        KeyCode::Char('2') | KeyCode::Char('3') => {
            app.view = View::Playlists;
            refresh_selected_playlist(app, daemon).await;
        }

        // Global playback controls
        KeyCode::Char(' ') => {
            let _ = daemon.send(&Request::Toggle).await;
        }
        KeyCode::Char('n') if app.view != View::Search || app.input_mode != InputMode::Search => {
            let _ = daemon.send(&Request::Next).await;
        }
        KeyCode::Char('p') if app.view != View::Search || app.input_mode != InputMode::Search => {
            let _ = daemon.send(&Request::Previous).await;
        }
        KeyCode::Char('s') => {
            if !app.status.queue.is_empty() {
                app.input_mode = InputMode::PlaylistName;
                app.new_playlist_name.clear();
                app.new_playlist_cursor = 0;
            } else {
                app.notify("Queue is empty: nothing to save");
            }
        }
        KeyCode::Char('J') => {
            if !app.status.queue.is_empty() {
                app.queue_selected =
                    (app.queue_selected + 1).min(app.status.queue.len().saturating_sub(1));
            }
        }
        KeyCode::Char('K') => {
            app.queue_selected = app.queue_selected.saturating_sub(1);
        }
        KeyCode::Char('D') => {
            if let Some(idx) = app.selected_queue_track() {
                let _ = daemon.send(&Request::RemoveFromQueue(idx)).await;
                if app.queue_selected > 0 && idx == app.queue_selected {
                    app.queue_selected -= 1;
                }
                app.notify("Track removed from queue");
            }
        }
        KeyCode::Char('H') => {
            if let Some(idx) = app.selected_queue_track() {
                if idx > 0 {
                    let _ = daemon
                        .send(&Request::MoveInQueue {
                            from: idx,
                            to: idx - 1,
                        })
                        .await;
                    app.queue_selected = idx - 1;
                }
            }
        }
        KeyCode::Char('L') => {
            if let Some(idx) = app.selected_queue_track() {
                if idx + 1 < app.status.queue.len() {
                    let _ = daemon
                        .send(&Request::MoveInQueue {
                            from: idx,
                            to: idx + 1,
                        })
                        .await;
                    app.queue_selected = idx + 1;
                }
            }
        }
        KeyCode::Char('T') => {
            if let Some(idx) = app.selected_queue_track() {
                let _ = daemon.send(&Request::PlayIndex(idx)).await;
                app.notify("Playing selected queue track");
            }
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            let vol = (app.status.volume + 5).min(150);
            let _ = daemon.send(&Request::SetVolume(vol)).await;
        }
        KeyCode::Char('-') => {
            let vol = (app.status.volume - 5).max(0);
            let _ = daemon.send(&Request::SetVolume(vol)).await;
        }
        KeyCode::Left => {
            let pos = (app.status.position - 5.0).max(0.0);
            let _ = daemon.send(&Request::Seek(pos)).await;
        }
        KeyCode::Right => {
            let pos = app.status.position + 5.0;
            let _ = daemon.send(&Request::Seek(pos)).await;
        }

        // View-specific keys
        _ => match app.view {
            View::Search => handle_search_normal(app, daemon, key).await?,
            View::Playlists => handle_playlists(app, daemon, key).await?,
        },
    }

    Ok(())
}

fn clamp_queue_selection(app: &mut App) {
    if app.status.queue.is_empty() {
        app.queue_selected = 0;
    } else {
        app.queue_selected = app.queue_selected.min(app.status.queue.len().saturating_sub(1));
    }
}

// --- Search View ---

async fn handle_search_normal(
    app: &mut App,
    daemon: &mut DaemonClient,
    key: event::KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Char('/') => {
            app.input_mode = InputMode::Search;
            app.search_selection_anchor = None;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.search_results.is_empty() {
                app.search_selected =
                    (app.search_selected + 1).min(app.search_results.len() - 1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.search_selected = app.search_selected.saturating_sub(1);
        }
        KeyCode::Enter => {
            // Play selected track immediately
            if let Some(track) = app.selected_search_track().cloned() {
                if app.status.queue.is_empty() {
                    // First track: add and let daemon auto-start playback.
                    let _ = daemon.send(&Request::AddToQueue(track)).await;
                } else {
                    // Non-empty queue: insert after current and jump to it.
                    let target = (app.status.queue_index + 1).min(app.status.queue.len());
                    let _ = daemon.send(&Request::InsertNext(track)).await;
                    let _ = daemon.send(&Request::PlayIndex(target)).await;
                }
                app.notify("Playing now!");
            }
        }
        KeyCode::Char('a') => {
            // Add to queue
            if let Some(track) = app.selected_search_track().cloned() {
                let name = track.title.clone();
                let _ = daemon.send(&Request::AddToQueue(track)).await;
                app.notify(format!("Added to queue: {name}"));
            }
        }
        _ => {}
    }
    Ok(())
}

async fn handle_search_input(
    app: &mut App,
    _daemon: &mut DaemonClient,
    key: event::KeyEvent,
) -> Result<()> {
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.search_selection_anchor = None;
        }
        KeyCode::Enter => {
            if !app.search_input.is_empty() {
                app.searching = true;
                app.input_mode = InputMode::Normal;
                app.search_selection_anchor = None;
                let query = app.search_input.clone();
                app.pending_search_query = Some(query);
            }
        }
        KeyCode::Char(c) => {
            delete_search_selection_if_any(app);
            app.search_input.insert(app.search_cursor, c);
            app.search_cursor += 1;
            app.search_selection_anchor = None;
        }
        KeyCode::Backspace => {
            if delete_search_selection_if_any(app) {
                // selection handled
            } else if app.search_cursor > 0 {
                app.search_cursor -= 1;
                app.search_input.remove(app.search_cursor);
            }
            app.search_selection_anchor = None;
        }
        KeyCode::Delete => {
            if delete_search_selection_if_any(app) {
                // selection handled
            } else if app.search_cursor < app.search_input.len() {
                app.search_input.remove(app.search_cursor);
            }
            app.search_selection_anchor = None;
        }
        KeyCode::Left => {
            if shift {
                if app.search_selection_anchor.is_none() {
                    app.search_selection_anchor = Some(app.search_cursor);
                }
            } else {
                app.search_selection_anchor = None;
            }
            app.search_cursor = app.search_cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            if shift {
                if app.search_selection_anchor.is_none() {
                    app.search_selection_anchor = Some(app.search_cursor);
                }
            } else {
                app.search_selection_anchor = None;
            }
            app.search_cursor = (app.search_cursor + 1).min(app.search_input.len());
        }
        KeyCode::Home => {
            if shift {
                if app.search_selection_anchor.is_none() {
                    app.search_selection_anchor = Some(app.search_cursor);
                }
            } else {
                app.search_selection_anchor = None;
            }
            app.search_cursor = 0;
        }
        KeyCode::End => {
            if shift {
                if app.search_selection_anchor.is_none() {
                    app.search_selection_anchor = Some(app.search_cursor);
                }
            } else {
                app.search_selection_anchor = None;
            }
            app.search_cursor = app.search_input.len();
        }
        _ => {}
    }
    Ok(())
}

fn delete_search_selection_if_any(app: &mut App) -> bool {
    let Some(anchor) = app.search_selection_anchor else {
        return false;
    };

    if anchor == app.search_cursor {
        app.search_selection_anchor = None;
        return false;
    }

    let (start, end) = if anchor < app.search_cursor {
        (anchor, app.search_cursor)
    } else {
        (app.search_cursor, anchor)
    };

    app.search_input.replace_range(start..end, "");
    app.search_cursor = start;
    app.search_selection_anchor = None;
    true
}

// --- Playlists View ---

async fn handle_playlists(
    app: &mut App,
    daemon: &mut DaemonClient,
    key: event::KeyEvent,
) -> Result<()> {
    match app.playlist_view {
        PlaylistView::List => handle_playlist_list(app, daemon, key).await,
    }
}

async fn refresh_selected_playlist(app: &mut App, daemon: &mut DaemonClient) {
    if let Some(name) = app.playlist_names.get(app.playlist_selected).cloned() {
        if let Ok(Response::Playlist(pl)) = daemon.send(&Request::GetPlaylist(name)).await {
            app.playlist_tracks = pl.tracks;
            app.playlist_track_selected = app
                .playlist_track_selected
                .min(app.playlist_tracks.len().saturating_sub(1));
        } else {
            app.playlist_tracks.clear();
            app.playlist_track_selected = 0;
        }
    }
}

async fn handle_playlist_list(
    app: &mut App,
    daemon: &mut DaemonClient,
    key: event::KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if app.playlist_track_focus {
                if app.playlist_track_selected + 1 < app.playlist_tracks.len() {
                    app.playlist_track_selected += 1;
                } else if app.playlist_selected + 1 < app.playlist_names.len() {
                    app.playlist_track_focus = false;
                    app.playlist_selected += 1;
                    app.playlist_track_selected = 0;
                    refresh_selected_playlist(app, daemon).await;
                }
            } else if !app.playlist_tracks.is_empty() {
                app.playlist_track_focus = true;
                app.playlist_track_selected = 0;
            } else if app.playlist_selected + 1 < app.playlist_names.len() {
                app.playlist_selected += 1;
                app.playlist_track_selected = 0;
                refresh_selected_playlist(app, daemon).await;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.playlist_track_focus {
                if app.playlist_track_selected > 0 {
                    app.playlist_track_selected -= 1;
                } else {
                    app.playlist_track_focus = false;
                }
            } else if app.playlist_selected > 0 {
                app.playlist_selected -= 1;
                app.playlist_track_selected = 0;
                refresh_selected_playlist(app, daemon).await;
            }
        }
        KeyCode::Enter => {
            app.playlist_track_focus = !app.playlist_track_focus;
            if app.playlist_track_focus {
                refresh_selected_playlist(app, daemon).await;
            }
        }
        KeyCode::Char('l') => {
            // Load playlist into queue
            if let Some(name) = app.playlist_names.get(app.playlist_selected).cloned() {
                let _ = daemon.send(&Request::LoadPlaylist(name.clone())).await;
                app.notify(format!("Playlist '{name}' loaded into queue"));
            }
        }
        KeyCode::Char('d') => {
            if app.playlist_track_focus {
                // Delete track from selected playlist
                if app.playlist_track_selected < app.playlist_tracks.len() {
                    app.playlist_tracks.remove(app.playlist_track_selected);
                    if let Some(name) = app.playlist_names.get(app.playlist_selected).cloned() {
                        let updated = Playlist {
                            name: name.clone(),
                            tracks: app.playlist_tracks.clone(),
                        };
                        let _ = daemon.send(&Request::SavePlaylist(updated)).await;
                        app.notify(format!("Track removed from playlist '{name}'"));
                    }
                    if app.playlist_track_selected >= app.playlist_tracks.len() && app.playlist_track_selected > 0 {
                        app.playlist_track_selected -= 1;
                    }
                }
            } else {
                // Ask confirmation before deleting playlist
                if let Some(name) = app.playlist_names.get(app.playlist_selected).cloned() {
                    app.playlist_delete_confirm_name = Some(name);
                }
            }
        }
        KeyCode::Char('r') => {
            // Refresh playlist list
            if let Ok(Response::Playlists(names)) =
                daemon.send(&Request::ListPlaylists).await
            {
                app.playlist_names = names;
                app.playlist_selected = app
                    .playlist_selected
                    .min(app.playlist_names.len().saturating_sub(1));
                app.playlist_track_focus = false;
                refresh_selected_playlist(app, daemon).await;
            }
        }
        _ => {}
    }
    Ok(())
}

// --- Playlist Name Input ---

async fn handle_playlist_name_input(
    app: &mut App,
    daemon: &mut DaemonClient,
    key: event::KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            if !app.new_playlist_name.is_empty() {
                let playlist = Playlist {
                    name: app.new_playlist_name.clone(),
                    tracks: app.status.queue.clone(),
                };
                let _ = daemon.send(&Request::SavePlaylist(playlist)).await;
                app.notify(format!("Playlist '{}' saved!", app.new_playlist_name));
                app.input_mode = InputMode::Normal;

                // Refresh playlist list
                if let Ok(Response::Playlists(names)) =
                    daemon.send(&Request::ListPlaylists).await
                {
                    app.playlist_names = names;
                }
            }
        }
        KeyCode::Char(c) => {
            app.new_playlist_name.insert(app.new_playlist_cursor, c);
            app.new_playlist_cursor += 1;
        }
        KeyCode::Backspace => {
            if app.new_playlist_cursor > 0 {
                app.new_playlist_cursor -= 1;
                app.new_playlist_name.remove(app.new_playlist_cursor);
            }
        }
        KeyCode::Left => {
            app.new_playlist_cursor = app.new_playlist_cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            app.new_playlist_cursor =
                (app.new_playlist_cursor + 1).min(app.new_playlist_name.len());
        }
        _ => {}
    }
    Ok(())
}
