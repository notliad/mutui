mod app;
mod client;
mod ui;

use anyhow::Result;
use app::{App, HelpPopupPage, InputMode, LibraryMode, PlaylistView, PodcastSection, SearchSection, View};
use client::DaemonClient;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use mutui_common::{Playlist, PodcastChannel, Request, Response};
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
    let mut search_task: Option<JoinHandle<anyhow::Result<(Response, Response)>>> = None;
    let mut playlist_tracks_task: Option<JoinHandle<anyhow::Result<(String, Response)>>> = None;
    let mut podcast_search_task: Option<JoinHandle<anyhow::Result<Response>>> = None;
    let mut podcast_episodes_task: Option<JoinHandle<anyhow::Result<(String, Response)>>> = None;

    // Initial status fetch
    if let Ok(Response::Status(status)) = daemon.send(&Request::GetStatus).await {
        app.status = *status;
        clamp_queue_selection(&mut app);
    }
    // Initial playlist list
    if let Ok(Response::Playlists(names)) = daemon.send(&Request::ListPlaylists).await {
        app.playlist_names = names;
    }
    // Initial library folders
    if let Ok(Response::LibraryFolders(folders)) = daemon.send(&Request::ListLibraryFolders).await {
        app.library_folders = folders;
    }
    // Initial followed podcasts
    if let Ok(Response::PodcastChannels(channels)) = daemon.send(&Request::ListFollowedPodcasts).await {
        app.podcast_followed = channels;
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
                    Ok(Ok((tracks_resp, playlists_resp))) => {
                        match tracks_resp {
                            Response::SearchResults(results) => {
                                app.search_results = results;
                                app.search_selected = 0;
                            }
                            Response::Error(e) => app.notify(format!("Search tracks error: {e}")),
                            _ => {}
                        }

                        match playlists_resp {
                            Response::SearchResults(results) => {
                                app.search_playlist_results = results;
                                app.search_playlist_selected = 0;
                            }
                            Response::Error(e) => {
                                app.notify(format!("Search playlists error: {e}"))
                            }
                            _ => {}
                        }

                        if app.search_section == SearchSection::Tracks
                            && app.search_results.is_empty()
                            && !app.search_playlist_results.is_empty()
                        {
                            app.search_section = SearchSection::Playlists;
                        }

                        app.search_playlist_expanded = false;
                        app.search_playlist_loading = false;
                        app.pending_search_playlist_url = None;
                        app.pending_search_playlist_id = None;
                        app.search_playlist_track_focus = false;
                        app.search_playlist_track_selected = 0;
                        app.search_playlist_tracks.clear();
                    }
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

        if playlist_tracks_task
            .as_ref()
            .map(|task| task.is_finished())
            .unwrap_or(false)
        {
            if let Some(task) = playlist_tracks_task.take() {
                app.search_playlist_loading = false;
                match task.await {
                    Ok(Ok((playlist_id, Response::SearchResults(tracks)))) => {
                        let selected_same_playlist = app
                            .selected_search_playlist()
                            .map(|p| p.id == playlist_id)
                            .unwrap_or(false);

                        if app.search_playlist_expanded && selected_same_playlist {
                            app.search_playlist_tracks = tracks.clone();
                            app.search_playlist_track_selected = app
                                .search_playlist_track_selected
                                .min(app.search_playlist_tracks.len().saturating_sub(1));
                        }

                        if let Some(pl) = app
                            .search_playlist_results
                            .iter_mut()
                            .find(|p| p.id == playlist_id)
                        {
                            if (pl.artist.trim().is_empty() || pl.artist == "Unknown")
                                && !tracks.is_empty()
                            {
                                pl.artist = tracks[0].artist.clone();
                            }
                            pl.album = Some(format!("youtube-playlist:{}", tracks.len()));
                        }
                    }
                    Ok(Ok((_, Response::Error(e)))) => {
                        app.notify(format!("Failed to load playlist tracks: {e}"));
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        app.notify(format!("Failed to load playlist tracks: {e}"));
                    }
                    Err(e) => {
                        app.notify(format!("Playlist load task failed: {e}"));
                    }
                }
            }
        }

        if search_task.is_none() {
            if let Some(query) = app.pending_search_query.take() {
                search_task = Some(tokio::spawn(async move {
                    let tracks = crate::client::send_once(Request::Search(query.clone())).await?;
                    let playlists = crate::client::send_once(Request::SearchPlaylists(query)).await?;
                    Ok((tracks, playlists))
                }));
            }
        }

        if playlist_tracks_task.is_none() {
            if let (Some(url), Some(id)) = (
                app.pending_search_playlist_url.take(),
                app.pending_search_playlist_id.take(),
            ) {
                app.search_playlist_loading = true;
                playlist_tracks_task = Some(tokio::spawn(async move {
                    let resp = crate::client::send_once(Request::GetYoutubePlaylistTracks(url)).await?;
                    Ok((id, resp))
                }));
            }
        }

        // Podcast search task
        if podcast_search_task
            .as_ref()
            .map(|t| t.is_finished())
            .unwrap_or(false)
        {
            if let Some(task) = podcast_search_task.take() {
                app.podcast_searching = false;
                match task.await {
                    Ok(Ok(Response::PodcastChannels(channels))) => {
                        app.podcast_last_error = None;
                        app.podcast_search_results = channels;
                        app.podcast_result_selected = 0;
                        app.podcast_section = PodcastSection::Results;
                    }
                    Ok(Ok(Response::Error(e))) => {
                        app.podcast_last_error = Some(e.clone());
                        app.notify(format!("Podcast search error: {e}"));
                    }
                    Ok(Err(e)) => {
                        let msg = e.to_string();
                        app.podcast_last_error = Some(msg.clone());
                        app.notify(format!("Podcast search error: {msg}"));
                    }
                    _ => {}
                }
            }
        }

        if podcast_search_task.is_none() {
            if let Some(query) = app.pending_podcast_search.take() {
                app.podcast_searching = true;
                app.podcast_last_error = None;
                podcast_search_task = Some(tokio::spawn(async move {
                    crate::client::send_once(Request::SearchPodcasts(query)).await
                }));
            }
        }

        // Podcast episode fetch task
        if podcast_episodes_task
            .as_ref()
            .map(|t| t.is_finished())
            .unwrap_or(false)
        {
            if let Some(task) = podcast_episodes_task.take() {
                app.podcast_episodes_loading = false;
                match task.await {
                    Ok(Ok((feed_url, Response::PodcastEpisodes(episodes)))) => {
                        if app.podcast_selected_feed.as_deref() == Some(&feed_url) {
                            app.podcast_last_error = None;
                            app.podcast_episodes = episodes;
                            app.podcast_episode_selected = 0;
                        }
                    }
                    Ok(Ok((_, Response::Error(e)))) => {
                        app.podcast_last_error = Some(e.clone());
                        app.notify(format!("Failed to load episodes: {e}"));
                    }
                    Ok(Err(e)) => {
                        let msg = e.to_string();
                        app.podcast_last_error = Some(msg.clone());
                        app.notify(format!("Failed to load episodes: {msg}"));
                    }
                    _ => {}
                }
            }
        }

        if podcast_episodes_task.is_none() {
            if let Some(feed_url) = app.pending_podcast_episodes.take() {
                app.podcast_episodes_loading = true;
                podcast_episodes_task = Some(tokio::spawn(async move {
                    let feed_for_key = feed_url.clone();
                    let resp = crate::client::send_once(Request::GetPodcastEpisodes(feed_url)).await?;
                    Ok((feed_for_key, resp))
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
    if app.library_delete_confirm_selected.is_some() {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(selected) = app.library_delete_confirm_selected.as_mut() {
                    if !app.library_folders.is_empty() {
                        *selected = (*selected + 1).min(app.library_folders.len() - 1);
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(selected) = app.library_delete_confirm_selected.as_mut() {
                    *selected = selected.saturating_sub(1);
                }
            }
            KeyCode::Enter => {
                let selected = app.library_delete_confirm_selected.unwrap_or(0);
                if let Some(folder) = app.library_folders.get(selected).cloned() {
                    if let Ok(Response::LibraryFolders(folders)) =
                        daemon.send(&Request::RemoveLibraryFolder(folder.clone())).await
                    {
                        app.library_folders = folders;
                        app.library_delete_confirm_selected = None;
                        app.notify(format!("Removed folder: {folder}"));
                        refresh_library(app, daemon).await;
                    }
                }
            }
            KeyCode::Esc => {
                app.library_delete_confirm_selected = None;
                app.notify("Delete canceled");
            }
            _ => {}
        }
        return Ok(());
    }

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
                    app.playlist_expanded = false;
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
                app.help_popup_page = HelpPopupPage::Shortcuts;
            }
            KeyCode::Tab => {
                app.help_popup_page = app.help_popup_page.next();
            }
            KeyCode::BackTab => {
                app.help_popup_page = app.help_popup_page.prev();
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
        InputMode::LibraryFolderPath => {
            handle_library_folder_input(app, daemon, key).await?;
            return Ok(());
        }
        InputMode::LibraryFilter => {
            handle_library_filter_input(app, key);
            return Ok(());
        }
        InputMode::Normal => {}
    }

    // Podcast search input is tracked separately (not an InputMode variant)
    if app.view == View::Podcasts && app.podcast_input_mode {
        handle_podcast_search_input(app, key);
        return Ok(());
    }
    if app.view == View::Podcasts && app.podcast_episode_filter_mode {
        handle_podcast_episode_filter_input(app, key);
        return Ok(());
    }

    match key.code {
        // Quit
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('Q') => {
            crate::client::stop_tray();
            let _ = daemon.send(&Request::Shutdown).await;
            app.should_quit = true;
        }
        KeyCode::Char('?') => {
            app.show_shortcuts_popup = true;
            app.help_popup_page = HelpPopupPage::Shortcuts;
        }

        // Tab navigation
        KeyCode::Tab => {
            app.view = app.view.next();
            if app.view == View::Playlists {
                app.playlist_expanded = false;
                app.playlist_track_focus = false;
                refresh_selected_playlist(app, daemon).await;
            }
            if app.view == View::Library {
                refresh_library(app, daemon).await;
            }
        }
        KeyCode::BackTab => {
            app.view = app.view.prev();
            if app.view == View::Playlists {
                app.playlist_expanded = false;
                app.playlist_track_focus = false;
                refresh_selected_playlist(app, daemon).await;
            }
            if app.view == View::Library {
                refresh_library(app, daemon).await;
            }
        }
        KeyCode::Char('1') => app.view = View::Search,
        KeyCode::Char('2') => {
            app.view = View::Playlists;
            app.playlist_expanded = false;
            app.playlist_track_focus = false;
            refresh_selected_playlist(app, daemon).await;
        }
        KeyCode::Char('3') => {
            app.view = View::Library;
            refresh_library(app, daemon).await;
        }
        KeyCode::Char('4') => {
            app.view = View::Podcasts;
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
        KeyCode::Char('A') => {
            let _ = daemon.send(&Request::ToggleAutoplay).await;
            app.status.autoplay_enabled = !app.status.autoplay_enabled;
            let mode = if app.status.autoplay_enabled { "ON" } else { "OFF" };
            app.notify(format!("Auto-play: {mode}"));
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
        KeyCode::Char('R') if !matches!(app.view, View::Library) => {
            let _ = daemon.send(&Request::ClearQueue).await;
            app.queue_selected = 0;
            app.notify("Queue cleared");
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            let vol = (app.status.volume + 5).min(150);
            let _ = daemon.send(&Request::SetVolume(vol)).await;
        }
        KeyCode::Char('-') => {
            let vol = (app.status.volume - 5).max(0);
            let _ = daemon.send(&Request::SetVolume(vol)).await;
        }
        KeyCode::Left if app.view != View::Playlists => {
            let pos = (app.status.position - 5.0).max(0.0);
            let _ = daemon.send(&Request::Seek(pos)).await;
        }
        KeyCode::Right if app.view != View::Playlists => {
            let pos = app.status.position + 5.0;
            let _ = daemon.send(&Request::Seek(pos)).await;
        }
        KeyCode::Char('o') => {
            open_external_current(app);
        }

        // View-specific keys
        _ => match app.view {
            View::Search => handle_search_normal(app, daemon, key).await?,
            View::Playlists => handle_playlists(app, daemon, key).await?,
            View::Library => handle_library(app, daemon, key).await?,
            View::Podcasts => handle_podcasts(app, daemon, key).await?,
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
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Char('/') => {
            app.input_mode = InputMode::Search;
            app.search_selection_anchor = None;
        }
        KeyCode::Char('j') if ctrl => {
            if !app.search_playlist_results.is_empty() {
                app.search_section = SearchSection::Playlists;
            }
        }
        KeyCode::Char('k') if ctrl => {
            if !app.search_results.is_empty() {
                app.search_section = SearchSection::Tracks;
                app.search_playlist_track_focus = false;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            match app.search_section {
                SearchSection::Tracks => {
                    if !app.search_results.is_empty() {
                        if app.search_selected + 1 < app.search_results.len() {
                            app.search_selected += 1;
                        } else if !app.search_playlist_results.is_empty() {
                            app.search_section = SearchSection::Playlists;
                        }
                    } else if !app.search_playlist_results.is_empty() {
                        app.search_section = SearchSection::Playlists;
                    }
                }
                SearchSection::Playlists => {
                    if app.search_playlist_expanded && app.search_playlist_track_focus {
                        if app.search_playlist_track_selected + 1 < app.search_playlist_tracks.len() {
                            app.search_playlist_track_selected += 1;
                        } else if app.search_playlist_selected + 1 < app.search_playlist_results.len() {
                            app.search_playlist_track_focus = false;
                            app.search_playlist_selected += 1;
                            app.search_playlist_track_selected = 0;
                            app.search_playlist_expanded = false;
                            refresh_selected_search_playlist(app);
                        }
                    } else if app.search_playlist_expanded && !app.search_playlist_tracks.is_empty() {
                        app.search_playlist_track_focus = true;
                        app.search_playlist_track_selected = 0;
                    } else if app.search_playlist_selected + 1 < app.search_playlist_results.len() {
                        app.search_playlist_selected += 1;
                        app.search_playlist_track_selected = 0;
                        app.search_playlist_expanded = false;
                        refresh_selected_search_playlist(app);
                    }
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            match app.search_section {
                SearchSection::Tracks => {
                    app.search_selected = app.search_selected.saturating_sub(1);
                }
                SearchSection::Playlists => {
                    if app.search_playlist_track_focus {
                        if app.search_playlist_track_selected > 0 {
                            app.search_playlist_track_selected -= 1;
                        } else {
                            app.search_playlist_track_focus = false;
                        }
                    } else if app.search_playlist_selected > 0 {
                        app.search_playlist_selected -= 1;
                        app.search_playlist_track_selected = 0;
                        app.search_playlist_expanded = false;
                        refresh_selected_search_playlist(app);
                    } else if !app.search_results.is_empty() {
                        app.search_section = SearchSection::Tracks;
                        app.search_selected = app.search_results.len().saturating_sub(1);
                    }
                }
            }
        }
        KeyCode::Enter => {
            match app.search_section {
                SearchSection::Tracks => {
                    if let Some(track) = app.selected_search_track().cloned() {
                        if app.status.queue.is_empty() {
                            let _ = daemon.send(&Request::AddToQueue(track)).await;
                        } else {
                            let target = (app.status.queue_index + 1).min(app.status.queue.len());
                            let _ = daemon.send(&Request::InsertNext(track)).await;
                            let _ = daemon.send(&Request::PlayIndex(target)).await;
                        }
                        app.notify("Playing now!");
                    }
                }
                SearchSection::Playlists => {
                    if app.search_playlist_expanded && app.search_playlist_track_focus {
                        if let Some(track) = app
                            .search_playlist_tracks
                            .get(app.search_playlist_track_selected)
                            .cloned()
                        {
                            play_or_queue_now(app, daemon, track).await;
                            app.notify("Playing now!");
                        }
                    } else {
                        app.search_playlist_expanded = !app.search_playlist_expanded;
                        app.search_playlist_track_focus = false;
                        app.search_playlist_track_selected = 0;
                        refresh_selected_search_playlist(app);
                    }
                }
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if app.search_section == SearchSection::Playlists {
                if app.search_playlist_expanded && app.search_playlist_track_focus {
                    if let Some(track) = app
                        .search_playlist_tracks
                        .get(app.search_playlist_track_selected)
                        .cloned()
                    {
                        play_or_queue_now(app, daemon, track).await;
                        app.notify("Playing now!");
                    }
                } else if !app.search_playlist_expanded {
                    app.search_playlist_expanded = true;
                    app.search_playlist_track_focus = false;
                    app.search_playlist_track_selected = 0;
                    refresh_selected_search_playlist(app);
                }
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if app.search_section == SearchSection::Playlists && app.search_playlist_expanded {
                app.search_playlist_expanded = false;
                app.search_playlist_track_focus = false;
                app.search_playlist_track_selected = 0;
                refresh_selected_search_playlist(app);
            }
        }
        KeyCode::Char('a') => {
            match app.search_section {
                SearchSection::Tracks => {
                    if let Some(track) = app.selected_search_track().cloned() {
                        let name = track.title.clone();
                        let _ = daemon.send(&Request::AddToQueue(track)).await;
                        app.notify(format!("Added to queue: {name}"));
                    }
                }
                SearchSection::Playlists => {
                    if app.search_playlist_expanded && app.search_playlist_track_focus {
                        if let Some(track) = app
                            .search_playlist_tracks
                            .get(app.search_playlist_track_selected)
                            .cloned()
                        {
                            let name = track.title.clone();
                            let _ = daemon.send(&Request::AddToQueue(track)).await;
                            app.notify(format!("Added to queue: {name}"));
                        }
                    } else if let Some(playlist) = app.selected_search_playlist().cloned() {
                        let playlist_name = playlist.title.clone();
                        match daemon
                            .send(&Request::AddYoutubePlaylistToQueue(playlist.url.clone()))
                            .await
                        {
                            Ok(Response::Ok) => {
                                app.notify(format!("Playlist added to queue: {playlist_name}"));
                            }
                            Ok(Response::Error(e)) => {
                                app.notify(format!("Failed to add playlist: {e}"));
                            }
                            Ok(_) => {}
                            Err(e) => app.notify(format!("Failed to add playlist: {e}")),
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn refresh_selected_search_playlist(app: &mut App) {
    if !app.search_playlist_expanded {
        app.search_playlist_loading = false;
        app.pending_search_playlist_url = None;
        app.pending_search_playlist_id = None;
        app.search_playlist_tracks.clear();
        app.search_playlist_track_selected = 0;
        app.search_playlist_track_focus = false;
        return;
    }

    app.search_playlist_tracks.clear();
    app.search_playlist_track_selected = 0;
    app.search_playlist_track_focus = false;

    if let Some(playlist) = app.selected_search_playlist().cloned() {
        app.search_playlist_loading = true;
        app.pending_search_playlist_url = Some(playlist.url.clone());
        app.pending_search_playlist_id = Some(playlist.id.clone());
    }
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
            app.search_input.insert(char_to_byte_idx(&app.search_input, app.search_cursor), c);
            app.search_cursor += 1;
            app.search_selection_anchor = None;
        }
        KeyCode::Backspace => {
            if delete_search_selection_if_any(app) {
                // selection handled
            } else if app.search_cursor > 0 {
                app.search_cursor -= 1;
                app.search_input.remove(char_to_byte_idx(&app.search_input, app.search_cursor));
            }
            app.search_selection_anchor = None;
        }
        KeyCode::Delete => {
            if delete_search_selection_if_any(app) {
                // selection handled
            } else if app.search_cursor < app.search_input.chars().count() {
                app.search_input.remove(char_to_byte_idx(&app.search_input, app.search_cursor));
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
            app.search_cursor = (app.search_cursor + 1).min(app.search_input.chars().count());
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
            app.search_cursor = app.search_input.chars().count();
        }
        _ => {}
    }
    Ok(())
}

/// Convert a char-count cursor position to a UTF-8 byte index.
fn char_to_byte_idx(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
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

    let byte_start = char_to_byte_idx(&app.search_input, start);
    let byte_end = char_to_byte_idx(&app.search_input, end);
    app.search_input.replace_range(byte_start..byte_end, "");
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
    if !app.playlist_expanded {
        app.playlist_tracks.clear();
        app.playlist_track_selected = 0;
        app.playlist_track_focus = false;
        return;
    }

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
            if app.playlist_expanded && app.playlist_track_focus {
                if app.playlist_track_selected + 1 < app.playlist_tracks.len() {
                    app.playlist_track_selected += 1;
                } else if app.playlist_selected + 1 < app.playlist_names.len() {
                    app.playlist_track_focus = false;
                    app.playlist_selected += 1;
                    app.playlist_track_selected = 0;
                    app.playlist_expanded = false;
                    refresh_selected_playlist(app, daemon).await;
                }
            } else if app.playlist_expanded && !app.playlist_tracks.is_empty() {
                app.playlist_track_focus = true;
                app.playlist_track_selected = 0;
            } else if app.playlist_selected + 1 < app.playlist_names.len() {
                app.playlist_selected += 1;
                app.playlist_track_selected = 0;
                app.playlist_expanded = false;
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
                app.playlist_expanded = false;
                refresh_selected_playlist(app, daemon).await;
            }
        }
        KeyCode::Enter => {
            if app.playlist_track_focus {
                if let Some(track) = app.playlist_tracks.get(app.playlist_track_selected).cloned() {
                    play_or_queue_now(app, daemon, track).await;
                    app.notify("Playing now!");
                }
            } else if !app.playlist_expanded {
                app.playlist_expanded = true;
                app.playlist_track_focus = false;
                app.playlist_track_selected = 0;
                refresh_selected_playlist(app, daemon).await;
            } else {
                app.playlist_expanded = false;
                app.playlist_track_focus = false;
                app.playlist_track_selected = 0;
                refresh_selected_playlist(app, daemon).await;
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if !app.playlist_expanded {
                app.playlist_expanded = true;
                app.playlist_track_focus = false;
                app.playlist_track_selected = 0;
                refresh_selected_playlist(app, daemon).await;
            } else {
                app.playlist_expanded = false;
                app.playlist_track_focus = false;
                app.playlist_track_selected = 0;
                refresh_selected_playlist(app, daemon).await;
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            if app.playlist_expanded {
                app.playlist_expanded = false;
                app.playlist_track_focus = false;
                app.playlist_track_selected = 0;
                refresh_selected_playlist(app, daemon).await;
            }
        }
        KeyCode::Char('a') => {
            if app.playlist_track_focus {
                if let Some(track) = app.playlist_tracks.get(app.playlist_track_selected).cloned() {
                    let name = track.title.clone();
                    let _ = daemon.send(&Request::AddToQueue(track)).await;
                    app.notify(format!("Added to queue: {name}"));
                }
            } else if let Some(name) = app.playlist_names.get(app.playlist_selected).cloned() {
                let _ = daemon.send(&Request::LoadPlaylist(name.clone())).await;
                app.notify(format!("Playlist '{name}' loaded into queue"));
            }
        }
        KeyCode::Char('d') => {
            if app.playlist_expanded && app.playlist_track_focus {
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
                app.playlist_expanded = false;
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
            app.new_playlist_name.insert(char_to_byte_idx(&app.new_playlist_name, app.new_playlist_cursor), c);
            app.new_playlist_cursor += 1;
        }
        KeyCode::Backspace => {
            if app.new_playlist_cursor > 0 {
                app.new_playlist_cursor -= 1;
                app.new_playlist_name.remove(char_to_byte_idx(&app.new_playlist_name, app.new_playlist_cursor));
            }
        }
        KeyCode::Left => {
            app.new_playlist_cursor = app.new_playlist_cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            app.new_playlist_cursor =
                (app.new_playlist_cursor + 1).min(app.new_playlist_name.chars().count());
        }
        _ => {}
    }
    Ok(())
}

// --- Library View ---

async fn refresh_library(app: &mut App, daemon: &mut DaemonClient) {
    if let Ok(Response::LibraryFolders(folders)) = daemon.send(&Request::ListLibraryFolders).await {
        app.library_folders = folders;
    }
    if let Ok(Response::LibraryTracks(tracks)) = daemon.send(&Request::ScanLibrary).await {
        app.library_tracks = tracks;
        app.library_selected = 0;
        app.library_group_selected = 0;
        app.library_group_track_selected = 0;
        app.library_group_focus = false;
    }
}

async fn handle_library(
    app: &mut App,
    daemon: &mut DaemonClient,
    key: event::KeyEvent,
) -> Result<()> {
    // --- Mode-cycle and filter activation (always available) ---
    match key.code {
        KeyCode::Char('m') => {
            app.library_mode = app.library_mode.next();
            app.library_group_selected = 0;
            app.library_group_track_selected = 0;
            app.library_group_focus = false;
            return Ok(());
        }
        KeyCode::Char('/') => {
            app.input_mode = InputMode::LibraryFilter;
            return Ok(());
        }
        KeyCode::Char('f') => {
            app.input_mode = InputMode::LibraryFolderPath;
            app.library_folder_input.clear();
            app.library_folder_cursor = 0;
            return Ok(());
        }
        KeyCode::Char('R') => {
            if app.library_folders.is_empty() {
                app.notify("No folders to remove");
            } else {
                app.library_delete_confirm_selected = Some(0);
            }
            return Ok(());
        }
        KeyCode::Char('r') => {
            refresh_library(app, daemon).await;
            app.notify("Library rescanned");
            return Ok(());
        }
        _ => {}
    }

    match app.library_mode {
        LibraryMode::AllTracks => handle_library_all_tracks(app, daemon, key).await,
        LibraryMode::ByArtist | LibraryMode::ByAlbum => {
            handle_library_grouped(app, daemon, key).await
        }
    }
}

async fn handle_library_all_tracks(
    app: &mut App,
    daemon: &mut DaemonClient,
    key: event::KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            let len = library_filtered_track_count(app);
            if len > 0 {
                app.library_selected = (app.library_selected + 1).min(len - 1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.library_selected = app.library_selected.saturating_sub(1);
        }
        KeyCode::Enter => {
            if let Some(track) = library_get_filtered_track(app, app.library_selected).cloned() {
                play_or_queue_now(app, daemon, track).await;
                app.notify("Playing now!");
            }
        }
        KeyCode::Char('a') => {
            if let Some(track) = library_get_filtered_track(app, app.library_selected).cloned() {
                let name = track.title.clone();
                let _ = daemon.send(&Request::AddToQueue(track)).await;
                app.notify(format!("Added to queue: {name}"));
            }
        }
        KeyCode::Char('o') => open_external_current(app),
        _ => {}
    }
    Ok(())
}

async fn handle_library_grouped(
    app: &mut App,
    daemon: &mut DaemonClient,
    key: event::KeyEvent,
) -> Result<()> {
    let groups = library_current_groups(app);
    let group_count = groups.len();
    let group_sel = app.library_group_selected.min(group_count.saturating_sub(1));
    let track_count = if group_count > 0 { groups[group_sel].1.len() } else { 0 };

    match key.code {
        KeyCode::Down | KeyCode::Char('j') => {
            if app.library_group_focus {
                if track_count > 0 {
                    app.library_group_track_selected =
                        (app.library_group_track_selected + 1).min(track_count - 1);
                }
            } else if group_count > 0 {
                app.library_group_selected = (group_sel + 1).min(group_count - 1);
                app.library_group_track_selected = 0;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.library_group_focus {
                app.library_group_track_selected =
                    app.library_group_track_selected.saturating_sub(1);
            } else {
                app.library_group_selected = group_sel.saturating_sub(1);
                app.library_group_track_selected = 0;
            }
        }
        KeyCode::Char('l') | KeyCode::Enter if !app.library_group_focus => {
            if track_count > 0 {
                app.library_group_focus = true;
                app.library_group_track_selected = 0;
            }
        }
        KeyCode::Char('h') | KeyCode::Esc if app.library_group_focus => {
            app.library_group_focus = false;
        }
        KeyCode::Enter if app.library_group_focus => {
            let track_sel = app
                .library_group_track_selected
                .min(track_count.saturating_sub(1));
            if let Some(track) = groups
                .get(group_sel)
                .and_then(|(_, tracks)| tracks.get(track_sel))
                .map(|t| (*t).clone())
            {
                play_or_queue_now(app, daemon, track).await;
                app.notify("Playing now!");
            }
        }
        KeyCode::Char('a') if app.library_group_focus => {
            let track_sel = app
                .library_group_track_selected
                .min(track_count.saturating_sub(1));
            if let Some(track) = groups
                .get(group_sel)
                .and_then(|(_, tracks)| tracks.get(track_sel))
                .map(|t| (*t).clone())
            {
                let name = track.title.clone();
                let _ = daemon.send(&Request::AddToQueue(track)).await;
                app.notify(format!("Added to queue: {name}"));
            }
        }
        KeyCode::Char('a') if !app.library_group_focus => {
            // Add all tracks from the selected group (left panel)
            if let Some((name, tracks)) = groups.get(group_sel) {
                let count = tracks.len();
                for track in tracks {
                    let _ = daemon.send(&Request::AddToQueue((*track).clone())).await;
                }
                app.notify(format!("Added {count} tracks from '{name}' to queue"));
            }
        }
        KeyCode::Char('o') => open_external_current(app),
        _ => {}
    }
    Ok(())
}

// Returns (group_name, Vec<Track>) for the current library mode + filter
fn library_current_groups(app: &App) -> Vec<(String, Vec<mutui_common::Track>)> {
    use std::collections::BTreeMap;
    let filter = app.library_filter.to_lowercase();
    match app.library_mode {
        LibraryMode::AllTracks => vec![],
        LibraryMode::ByArtist => {
            let mut map: BTreeMap<String, Vec<mutui_common::Track>> = BTreeMap::new();
            for t in &app.library_tracks {
                map.entry(t.artist.clone()).or_default().push(t.clone());
            }
            map.into_iter()
                .filter(|(name, _)| filter.is_empty() || name.to_lowercase().contains(&filter))
                .collect()
        }
        LibraryMode::ByAlbum => {
            let mut map: BTreeMap<String, Vec<mutui_common::Track>> = BTreeMap::new();
            for t in &app.library_tracks {
                let album = t.album.as_deref().unwrap_or("Unknown Album").to_string();
                map.entry(album).or_default().push(t.clone());
            }
            map.into_iter()
                .filter(|(name, _)| filter.is_empty() || name.to_lowercase().contains(&filter))
                .collect()
        }
    }
}

fn library_filtered_track_count(app: &App) -> usize {
    if app.library_filter.is_empty() {
        return app.library_tracks.len();
    }
    let f = app.library_filter.to_lowercase();
    app.library_tracks
        .iter()
        .filter(|t| {
            t.title.to_lowercase().contains(&f)
                || t.artist.to_lowercase().contains(&f)
                || t.album.as_deref().unwrap_or("").to_lowercase().contains(&f)
        })
        .count()
}

fn library_get_filtered_track(
    app: &App,
    index: usize,
) -> Option<&mutui_common::Track> {
    if app.library_filter.is_empty() {
        return app.library_tracks.get(index);
    }
    let f = app.library_filter.to_lowercase();
    app.library_tracks
        .iter()
        .filter(|t| {
            t.title.to_lowercase().contains(&f)
                || t.artist.to_lowercase().contains(&f)
                || t.album.as_deref().unwrap_or("").to_lowercase().contains(&f)
        })
        .nth(index)
}

// --- Podcasts View ---

async fn handle_podcasts(
    app: &mut App,
    daemon: &mut DaemonClient,
    key: event::KeyEvent,
) -> Result<()> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        // Section jumps (mirrors search tab Ctrl+j/k)
        KeyCode::Char('j') if ctrl => {
            if !app.podcast_episode_focus {
                app.podcast_section = PodcastSection::Followed;
            }
        }
        KeyCode::Char('k') if ctrl => {
            if !app.podcast_episode_focus && !app.podcast_search_results.is_empty() {
                app.podcast_section = PodcastSection::Results;
            }
        }
        KeyCode::Char('/') => {
            app.podcast_input_mode = true;
            app.podcast_episode_focus = false;
        }
        KeyCode::Esc => {
            if app.podcast_episode_filter_mode {
                app.podcast_episode_filter_mode = false;
            } else if app.podcast_episode_focus {
                app.podcast_episode_focus = false;
            } else if !app.podcast_search_results.is_empty() {
                app.podcast_search_results.clear();
                app.podcast_result_selected = 0;
                app.podcast_section = PodcastSection::Followed;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.podcast_episode_focus {
                let len = filtered_episodes(app).len();
                if len > 0 {
                    app.podcast_episode_selected =
                        (app.podcast_episode_selected + 1).min(len - 1);
                }
            } else {
                match app.podcast_section {
                    PodcastSection::Results => {
                        let len = app.podcast_search_results.len();
                        if len == 0 || app.podcast_result_selected + 1 >= len {
                            if !app.podcast_followed.is_empty() {
                                app.podcast_section = PodcastSection::Followed;
                            }
                        } else {
                            app.podcast_result_selected += 1;
                        }
                    }
                    PodcastSection::Followed => {
                        let len = app.podcast_followed.len();
                        if len > 0 {
                            app.podcast_followed_selected =
                                (app.podcast_followed_selected + 1).min(len - 1);
                        }
                    }
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.podcast_episode_focus {
                app.podcast_episode_selected = app.podcast_episode_selected.saturating_sub(1);
            } else {
                match app.podcast_section {
                    PodcastSection::Results => {
                        app.podcast_result_selected =
                            app.podcast_result_selected.saturating_sub(1);
                    }
                    PodcastSection::Followed => {
                        if app.podcast_followed_selected == 0 {
                            if !app.podcast_search_results.is_empty() {
                                app.podcast_section = PodcastSection::Results;
                            }
                        } else {
                            app.podcast_followed_selected -= 1;
                        }
                    }
                }
            }
        }
        KeyCode::Right | KeyCode::Char('l') => {
            if !app.podcast_episode_focus && !app.podcast_episodes.is_empty() {
                app.podcast_episode_focus = true;
            }
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.podcast_episode_focus = false;
        }
        KeyCode::Enter => {
            if app.podcast_episode_focus {
                if let Some(ep) = filtered_episodes(app)
                    .get(app.podcast_episode_selected)
                    .cloned()
                    .cloned()
                {
                    let track = episode_to_track(&ep);
                    play_or_queue_now(app, daemon, track).await;
                    app.notify("Playing now!");
                }
            } else if let Some(ch) = selected_podcast_channel(app).cloned() {
                app.podcast_selected_feed = Some(ch.feed_url.clone());
                app.podcast_episodes.clear();
                app.podcast_episode_selected = 0;
                app.podcast_episode_filter.clear();
                app.podcast_episode_filter_cursor = 0;
                app.podcast_episode_focus = false;
                app.pending_podcast_episodes = Some(ch.feed_url);
            }
        }
        KeyCode::Char('a') => {
            if app.podcast_episode_focus {
                if let Some(ep) = filtered_episodes(app)
                    .get(app.podcast_episode_selected)
                    .cloned()
                    .cloned()
                {
                    let track = episode_to_track(&ep);
                    let title = track.title.clone();
                    let _ = daemon.send(&Request::AddToQueue(track)).await;
                    app.notify(format!("Added to queue: {title}"));
                }
            }
        }
        KeyCode::Char('f') => {
            if app.podcast_episode_focus {
                // Enter filter mode for the episode list
                app.podcast_episode_filter_mode = true;
            } else {
                // Follow / unfollow the selected channel
                if let Some(ch) = selected_podcast_channel(app).cloned() {
                    let already_followed = app
                        .podcast_followed
                        .iter()
                        .any(|f| f.feed_url == ch.feed_url);
                    if already_followed {
                        match daemon.send(&Request::UnfollowPodcast(ch.feed_url.clone())).await {
                            Ok(Response::PodcastChannels(followed)) => {
                                app.podcast_followed = followed;
                                app.notify(format!("Unfollowed: {}", ch.title));
                            }
                            Ok(Response::Error(e)) => app.notify(format!("Error: {e}")),
                            _ => {}
                        }
                    } else {
                        match daemon.send(&Request::FollowPodcast(ch.clone())).await {
                            Ok(Response::PodcastChannels(followed)) => {
                                app.podcast_followed = followed;
                                app.notify(format!("Following: {}", ch.title));
                            }
                            Ok(Response::Error(e)) => app.notify(format!("Error: {e}")),
                            _ => {}
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_podcast_episode_filter_input(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.podcast_episode_filter_mode = false;
        }
        KeyCode::Char(c) => {
            app.podcast_episode_filter
                .insert(app.podcast_episode_filter_cursor, c);
            app.podcast_episode_filter_cursor += 1;
            app.podcast_episode_selected = 0;
        }
        KeyCode::Backspace => {
            if app.podcast_episode_filter_cursor > 0 {
                app.podcast_episode_filter_cursor -= 1;
                app.podcast_episode_filter
                    .remove(app.podcast_episode_filter_cursor);
                app.podcast_episode_selected = 0;
            }
        }
        KeyCode::Left => {
            app.podcast_episode_filter_cursor =
                app.podcast_episode_filter_cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            app.podcast_episode_filter_cursor = (app.podcast_episode_filter_cursor + 1)
                .min(app.podcast_episode_filter.len());
        }
        _ => {}
    }
}

fn filtered_episodes(app: &App) -> Vec<&mutui_common::PodcastEpisode> {
    if app.podcast_episode_filter.is_empty() {
        return app.podcast_episodes.iter().collect();
    }
    let q = app.podcast_episode_filter.to_lowercase();
    app.podcast_episodes
        .iter()
        .filter(|ep| ep.title.to_lowercase().contains(&q))
        .collect()
}

fn handle_podcast_search_input(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.podcast_input_mode = false;
        }
        KeyCode::Enter => {
            if !app.podcast_search_input.is_empty() {
                app.pending_podcast_search = Some(app.podcast_search_input.clone());
            }
            app.podcast_input_mode = false;
        }
        KeyCode::Char(c) => {
            app.podcast_search_input
                .insert(app.podcast_search_cursor, c);
            app.podcast_search_cursor += 1;
        }
        KeyCode::Backspace => {
            if app.podcast_search_cursor > 0 {
                app.podcast_search_cursor -= 1;
                app.podcast_search_input.remove(app.podcast_search_cursor);
            }
        }
        KeyCode::Left => {
            app.podcast_search_cursor = app.podcast_search_cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            app.podcast_search_cursor =
                (app.podcast_search_cursor + 1).min(app.podcast_search_input.len());
        }
        KeyCode::Home => {
            app.podcast_search_cursor = 0;
        }
        KeyCode::End => {
            app.podcast_search_cursor = app.podcast_search_input.len();
        }
        _ => {}
    }
}

fn selected_podcast_channel(app: &App) -> Option<&PodcastChannel> {
    match app.podcast_section {
        PodcastSection::Results => app.podcast_search_results.get(app.podcast_result_selected),
        PodcastSection::Followed => app.podcast_followed.get(app.podcast_followed_selected),
    }
}

fn episode_to_track(ep: &mutui_common::PodcastEpisode) -> mutui_common::Track {
    mutui_common::Track {
        id: ep.guid.clone(),
        title: ep.title.clone(),
        artist: String::new(),
        album: None,
        duration: ep.duration,
        url: ep.url.clone(),
    }
}

async fn play_or_queue_now(
    app: &mut App,
    daemon: &mut DaemonClient,
    track: mutui_common::Track,
) {
    if app.status.queue.is_empty() {
        let _ = daemon.send(&Request::AddToQueue(track)).await;
    } else {
        let target = (app.status.queue_index + 1).min(app.status.queue.len());
        let _ = daemon.send(&Request::InsertNext(track)).await;
        let _ = daemon.send(&Request::PlayIndex(target)).await;
    }
}

fn handle_library_filter_input(app: &mut App, key: event::KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Enter => {
            app.input_mode = InputMode::Normal;
            // Reset group selection when filter changes
            app.library_group_selected = 0;
            app.library_group_track_selected = 0;
            app.library_group_focus = false;
            app.library_selected = 0;
        }
        KeyCode::Char(c) => {
            app.library_filter.insert(char_to_byte_idx(&app.library_filter, app.library_filter_cursor), c);
            app.library_filter_cursor += 1;
        }
        KeyCode::Backspace => {
            if app.library_filter_cursor > 0 {
                app.library_filter_cursor -= 1;
                app.library_filter.remove(char_to_byte_idx(&app.library_filter, app.library_filter_cursor));
            }
        }
        KeyCode::Delete => {
            if app.library_filter_cursor < app.library_filter.chars().count() {
                app.library_filter.remove(char_to_byte_idx(&app.library_filter, app.library_filter_cursor));
            }
        }
        KeyCode::Left => {
            app.library_filter_cursor = app.library_filter_cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            app.library_filter_cursor =
                (app.library_filter_cursor + 1).min(app.library_filter.chars().count());
        }
        _ => {}
    }
}

async fn handle_library_folder_input(
    app: &mut App,
    daemon: &mut DaemonClient,
    key: event::KeyEvent,
) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            if !app.library_folder_input.is_empty() {
                let folder = app.library_folder_input.clone();
                match daemon.send(&Request::AddLibraryFolder(folder.clone())).await {
                    Ok(Response::LibraryFolders(folders)) => {
                        app.library_folders = folders;
                        app.notify(format!("Added folder: {folder}"));
                        app.input_mode = InputMode::Normal;
                        // Rescan
                        if let Ok(Response::LibraryTracks(tracks)) =
                            daemon.send(&Request::ScanLibrary).await
                        {
                            app.library_tracks = tracks;
                            app.library_selected = 0;
                        }
                    }
                    Ok(Response::Error(e)) => {
                        app.notify(format!("Error: {e}"));
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Char(c) => {
            app.library_folder_input
                .insert(char_to_byte_idx(&app.library_folder_input, app.library_folder_cursor), c);
            app.library_folder_cursor += 1;
        }
        KeyCode::Backspace => {
            if app.library_folder_cursor > 0 {
                app.library_folder_cursor -= 1;
                app.library_folder_input
                    .remove(char_to_byte_idx(&app.library_folder_input, app.library_folder_cursor));
            }
        }
        KeyCode::Left => {
            app.library_folder_cursor = app.library_folder_cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            app.library_folder_cursor =
                (app.library_folder_cursor + 1).min(app.library_folder_input.chars().count());
        }
        _ => {}
    }
    Ok(())
}

// --- Open External ---

fn open_external_current(app: &mut App) {
    let track = if let Some(t) = &app.status.current_track {
        t.clone()
    } else {
        app.notify("No track playing");
        return;
    };

    open_track_external(app, &track);
}

fn open_track_external(app: &mut App, track: &mutui_common::Track) {
    let url = &track.url;

    // Determine what to open
    let target = if url.contains("youtube.com") || url.contains("youtu.be") {
        url.clone()
    } else if std::path::Path::new(url).exists() {
        url.clone()
    } else if url.starts_with("http") {
        url.clone()
    } else {
        app.notify("Cannot open: unknown source");
        return;
    };

    match std::process::Command::new("xdg-open")
        .arg(&target)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => app.notify("Opened externally"),
        Err(e) => app.notify(format!("Failed to open: {e}")),
    }
}
