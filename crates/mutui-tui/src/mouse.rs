use crate::app::{App, HelpPopupPage, InputMode, LibraryMode, PodcastSection, SearchSection, View};
use crate::client::DaemonClient;
use crossterm::event::{self, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use mutui_common::{Request, Response};
use ratatui::layout::{Position, Rect};
use std::time::{Duration, Instant};

/// Window in which two clicks on the same target count as a double click.
const DOUBLE_CLICK_TIMEOUT: Duration = Duration::from_millis(350);

// --- Identifiers for interactive regions -----------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListId {
    SearchTracks,
    SearchPlaylists,
    PlaylistList,
    LibraryTracks,
    LibraryGroups,
    LibraryGroupTracks,
    PodcastResults,
    PodcastFollowed,
    PodcastEpisodes,
    Queue,
    LibraryFolderDelete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputId {
    Search,
    PodcastSearch,
    PodcastEpisodeFilter,
    LibraryFilter,
    PlaylistName,
    LibraryFolder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAction {
    TogglePlay,
    Search,
    ToggleAutoplay,
    Help,
    Quit,
    Confirm,
    RemoveFolder,
    Cancel,
    SwitchLibraryMode(LibraryMode),
}

// --- Hit regions -----------------------------------------------------------

pub enum HitRegion {
    /// A scrollable list. `area` is the *content* area (rows start at `area.y`),
    /// `offset` is the first visible row, `count` the total number of rows.
    List(ListId, Rect, usize, usize),
    /// A text input box.
    Input(InputId, Rect),
    /// The now-playing progress bar (click = seek).
    Seek(Rect),
    /// A clickable control (player bar, popup buttons, library mode tabs).
    Button(ButtonAction, Rect),
    /// A view tab in the top bar.
    Tab(Rect, View),
    /// A modal popup area (used to detect clicks outside it).
    Popup(Rect),
}

impl HitRegion {
    fn rect(&self) -> &Rect {
        match self {
            HitRegion::List(_, r, _, _)
            | HitRegion::Input(_, r)
            | HitRegion::Seek(r)
            | HitRegion::Button(_, r)
            | HitRegion::Tab(r, _)
            | HitRegion::Popup(r) => r,
        }
    }

    fn contains(&self, x: u16, y: u16) -> bool {
        self.rect().contains(Position::new(x, y))
    }
}

/// Regions registered during rendering, so mouse events can be mapped back to
/// the widget that was drawn. Popups are registered last and win on overlap.
pub struct MouseMap {
    regions: Vec<HitRegion>,
}

impl MouseMap {
    pub fn new() -> Self {
        Self { regions: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.regions.clear();
    }

    pub fn push(&mut self, region: HitRegion) {
        self.regions.push(region);
    }

    /// Register a `List` widget that is wrapped in a bordered block. The
    /// content rows live between the top and bottom borders.
    pub fn push_list(&mut self, id: ListId, area: Rect, offset: usize, count: usize) {
        let content = Rect::new(
            area.x,
            area.y.saturating_add(1),
            area.width,
            area.height.saturating_sub(2),
        );
        self.regions.push(HitRegion::List(id, content, offset, count));
    }

    /// Register a list whose rows are drawn directly in `area` (no borders).
    pub fn push_list_content(&mut self, id: ListId, area: Rect, offset: usize, count: usize) {
        self.regions.push(HitRegion::List(id, area, offset, count));
    }

    fn find(&self, x: u16, y: u16) -> Option<&HitRegion> {
        self.regions.iter().rev().find(|r| r.contains(x, y))
    }

    fn popup_contains(&self, x: u16, y: u16) -> bool {
        self.regions
            .iter()
            .rev()
            .any(|r| matches!(r, HitRegion::Popup(a) if a.contains(Position::new(x, y))))
    }
}

// --- Double click detection ------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClickKey {
    List(ListId, usize),
}

struct LastClick {
    key: ClickKey,
    x: u16,
    y: u16,
    at: Instant,
}

pub struct MouseState {
    last: Option<LastClick>,
}

impl MouseState {
    pub fn new() -> Self {
        Self { last: None }
    }

    fn is_double_click(&mut self, key: ClickKey, x: u16, y: u16) -> bool {
        let now = Instant::now();
        let double = matches!(
            &self.last,
            Some(l)
                if l.key == key
                    && now.duration_since(l.at) <= DOUBLE_CLICK_TIMEOUT
                    && l.x.abs_diff(x) <= 1
                    && l.y.abs_diff(y) <= 1
        );
        self.last = Some(LastClick { key, x, y, at: now });
        double
    }
}

// --- Entry point -----------------------------------------------------------

pub async fn handle_mouse(
    app: &mut App,
    daemon: &mut DaemonClient,
    map: &MouseMap,
    state: &mut MouseState,
    event: MouseEvent,
) -> anyhow::Result<()> {
    let MouseEvent { kind, column, row, .. } = event;
    match kind {
        MouseEventKind::Down(MouseButton::Left) => click(app, daemon, map, state, column, row).await,
        MouseEventKind::Down(MouseButton::Right) => right_click(app, daemon, map, column, row).await,
        MouseEventKind::ScrollDown => scroll(app, daemon, map, column, row, true).await,
        MouseEventKind::ScrollUp => scroll(app, daemon, map, column, row, false).await,
        _ => Ok(()),
    }
}

// --- Popup helpers ---------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum PopupKind {
    DeletePlaylist,
    DeleteFolder,
    ClearQueueConfirm,
    Shortcuts,
    PlaylistName,
    FolderPath,
}

fn current_popup(app: &App) -> Option<PopupKind> {
    if app.playlist_delete_confirm_name.is_some() {
        return Some(PopupKind::DeletePlaylist);
    }
    if app.clear_queue_confirm {
        return Some(PopupKind::ClearQueueConfirm);
    }
    if app.library_delete_confirm_selected.is_some() {
        return Some(PopupKind::DeleteFolder);
    }
    if app.show_shortcuts_popup {
        return Some(PopupKind::Shortcuts);
    }
    if app.input_mode == InputMode::PlaylistName {
        return Some(PopupKind::PlaylistName);
    }
    if app.input_mode == InputMode::LibraryFolderPath {
        return Some(PopupKind::FolderPath);
    }
    None
}

fn dismiss_popup(app: &mut App) {
    app.playlist_delete_confirm_name = None;
    app.clear_queue_confirm = false;
    app.library_delete_confirm_selected = None;
    app.show_shortcuts_popup = false;
    app.help_popup_page = HelpPopupPage::Shortcuts;
    if app.input_mode == InputMode::PlaylistName || app.input_mode == InputMode::LibraryFolderPath {
        app.input_mode = InputMode::Normal;
    }
}

/// Leave text-entry modes (used before a mouse action on the main UI).
fn exit_input_modes(app: &mut App) {
    app.input_mode = InputMode::Normal;
    app.podcast_input_mode = false;
    app.podcast_episode_filter_mode = false;
}

// --- Click handling --------------------------------------------------------

async fn click(
    app: &mut App,
    daemon: &mut DaemonClient,
    map: &MouseMap,
    state: &mut MouseState,
    x: u16,
    y: u16,
) -> anyhow::Result<()> {
    if let Some(popup) = current_popup(app) {
        if !map.popup_contains(x, y) {
            dismiss_popup(app);
            return Ok(());
        }
        match popup {
            PopupKind::Shortcuts => {
                app.help_popup_page = app.help_popup_page.next();
                return Ok(());
            }
            PopupKind::PlaylistName | PopupKind::FolderPath => {
                if let Some(HitRegion::Input(id, area)) = map.find(x, y) {
                    focus_input(app, *id, *area, x);
                }
                return Ok(());
            }
            PopupKind::DeletePlaylist | PopupKind::DeleteFolder | PopupKind::ClearQueueConfirm => {}
        }
    }

    let Some(region) = map.find(x, y) else {
        return Ok(());
    };

    match region {
        HitRegion::Popup(_) => {}
        HitRegion::Button(action, _) => button_action(app, daemon, *action).await?,
        HitRegion::Tab(_, view) => {
            exit_input_modes(app);
            crate::switch_view(app, daemon, *view).await;
        }
        HitRegion::Seek(area) => seek(app, daemon, area, x).await?,
        HitRegion::Input(id, area) => focus_input(app, *id, *area, x),
        HitRegion::List(id, area, offset, count) => {
            let region = ListRegion {
                id: *id,
                area: *area,
                offset: *offset,
                count: *count,
            };
            list_click(app, daemon, state, region, x, y).await?
        }
    }
    Ok(())
}

async fn button_action(
    app: &mut App,
    daemon: &mut DaemonClient,
    action: ButtonAction,
) -> anyhow::Result<()> {
    match action {
        ButtonAction::TogglePlay => {
            let _ = daemon.send(&Request::Toggle).await;
        }
        ButtonAction::Search => {
            exit_input_modes(app);
            app.input_mode = InputMode::Search;
            app.search_selection_anchor = None;
        }
        ButtonAction::ToggleAutoplay => {
            let _ = daemon.send(&Request::ToggleAutoplay).await;
            app.status.autoplay_enabled = !app.status.autoplay_enabled;
            let mode = if app.status.autoplay_enabled { "ON" } else { "OFF" };
            app.notify(format!("Auto-play: {mode}"));
        }
        ButtonAction::Help => {
            app.show_shortcuts_popup = true;
            app.help_popup_page = HelpPopupPage::Shortcuts;
        }
        ButtonAction::Quit => {
            app.should_quit = true;
        }
        ButtonAction::Confirm => {
            if app.clear_queue_confirm {
                app.clear_queue_confirm = false;
                let _ = daemon.send(&Request::ClearQueue).await;
                app.queue_selected = 0;
                app.notify("Queue cleared");
            } else {
                confirm_delete_playlist(app, daemon).await?;
            }
        }
        ButtonAction::RemoveFolder => {
            if let Some(index) = app.library_delete_confirm_selected {
                remove_folder(app, daemon, index).await?;
            }
        }
        ButtonAction::Cancel => dismiss_popup(app),
        ButtonAction::SwitchLibraryMode(mode) => {
            app.library_mode = mode;
            app.library_group_selected = 0;
            app.library_group_track_selected = 0;
            app.library_group_focus = false;
        }
    }
    Ok(())
}

async fn confirm_delete_playlist(app: &mut App, daemon: &mut DaemonClient) -> anyhow::Result<()> {
    if let Some(name) = app.playlist_delete_confirm_name.take() {
        let _ = daemon.send(&Request::DeletePlaylist(name.clone())).await;
        app.playlist_names.retain(|n| *n != name);
        app.playlist_selected = app
            .playlist_selected
            .min(app.playlist_names.len().saturating_sub(1));
        app.playlist_track_focus = false;
        app.playlist_expanded = false;
        crate::refresh_selected_playlist(app, daemon).await;
        app.notify(format!("Playlist '{name}' deleted"));
    }
    Ok(())
}

async fn remove_folder(app: &mut App, daemon: &mut DaemonClient, index: usize) -> anyhow::Result<()> {
    if let Some(folder) = app.library_folders.get(index).cloned() {
        if let Ok(Response::LibraryFolders(folders)) =
            daemon.send(&Request::RemoveLibraryFolder(folder.clone())).await
        {
            app.library_folders = folders;
            app.library_delete_confirm_selected = None;
            app.notify(format!("Removed folder: {folder}"));
            crate::refresh_library(app, daemon).await;
        }
    }
    Ok(())
}

async fn seek(app: &mut App, daemon: &mut DaemonClient, area: &Rect, x: u16) -> anyhow::Result<()> {
    if app.status.duration <= 0.0 || area.width == 0 {
        return Ok(());
    }
    let ratio = (x.saturating_sub(area.x)) as f64 / area.width as f64;
    let pos = ratio.clamp(0.0, 1.0) * app.status.duration;
    let _ = daemon.send(&Request::Seek(pos)).await;
    Ok(())
}

fn focus_input(app: &mut App, id: InputId, area: Rect, x: u16) {
    app.podcast_input_mode = false;
    app.podcast_episode_filter_mode = false;
    match id {
        InputId::Search => {
            app.input_mode = InputMode::Search;
            app.search_selection_anchor = None;
            app.search_cursor = clamp_cursor(x, area.x, 3, app.search_input.chars().count());
        }
        InputId::PodcastSearch => {
            app.input_mode = InputMode::Normal;
            app.podcast_input_mode = true;
            app.podcast_search_cursor =
                clamp_cursor(x, area.x, 3, app.podcast_search_input.chars().count());
        }
        InputId::PodcastEpisodeFilter => {
            app.input_mode = InputMode::Normal;
            app.podcast_episode_filter_mode = true;
            app.podcast_episode_filter_cursor = clamp_cursor(
                x,
                area.x,
                3,
                app.podcast_episode_filter.chars().count(),
            );
        }
        InputId::LibraryFilter => {
            app.input_mode = InputMode::LibraryFilter;
            app.library_filter_cursor =
                clamp_cursor(x, area.x, 0, app.library_filter.chars().count());
        }
        InputId::PlaylistName => {
            app.input_mode = InputMode::PlaylistName;
            app.new_playlist_cursor =
                clamp_cursor(x, area.x, 3, app.new_playlist_name.chars().count());
        }
        InputId::LibraryFolder => {
            app.input_mode = InputMode::LibraryFolderPath;
            app.library_folder_cursor =
                clamp_cursor(x, area.x, 3, app.library_folder_input.chars().count());
        }
    }
}

fn clamp_cursor(x: u16, area_x: u16, offset: i32, max: usize) -> usize {
    let col = (x as i32) - (area_x as i32) - offset;
    col.clamp(0, max as i32) as usize
}

// --- List handling ---------------------------------------------------------

/// Flattened row -> playlist index / track index inside an expanded playlist.
enum RowTarget {
    Playlist(usize),
    Track(usize),
}

fn search_playlist_row_target(app: &App, index: usize) -> Option<RowTarget> {
    let mut cursor = 0usize;
    for (i, _) in app.search_playlist_results.iter().enumerate() {
        if cursor == index {
            return Some(RowTarget::Playlist(i));
        }
        cursor += 1;
        if i == app.search_playlist_selected && app.search_playlist_expanded {
            if app.search_playlist_loading {
                if cursor == index {
                    return None;
                }
                cursor += 1;
            }
            for t in 0..app.search_playlist_tracks.len() {
                if cursor == index {
                    return Some(RowTarget::Track(t));
                }
                cursor += 1;
            }
        }
    }
    None
}

fn playlist_row_target(app: &App, index: usize) -> Option<RowTarget> {
    let mut cursor = 0usize;
    for (i, _) in app.playlist_names.iter().enumerate() {
        if cursor == index {
            return Some(RowTarget::Playlist(i));
        }
        cursor += 1;
        if i == app.playlist_selected && app.playlist_expanded {
            for t in 0..app.playlist_tracks.len() {
                if cursor == index {
                    return Some(RowTarget::Track(t));
                }
                cursor += 1;
            }
        }
    }
    None
}

/// Apply a single click on a list row. Returns `false` if the row has no
/// actionable target (e.g. the "loading tracks..." row).
fn select_at(app: &mut App, id: ListId, index: usize) -> bool {
    match id {
        ListId::SearchTracks => {
            app.search_section = SearchSection::Tracks;
            app.search_selected = index.min(app.search_results.len().saturating_sub(1));
            app.search_playlist_track_focus = false;
        }
        ListId::SearchPlaylists => match search_playlist_row_target(app, index) {
            Some(RowTarget::Playlist(i)) => {
                app.search_section = SearchSection::Playlists;
                app.search_playlist_selected = i;
                app.search_playlist_track_focus = false;
            }
            Some(RowTarget::Track(t)) => {
                app.search_section = SearchSection::Playlists;
                app.search_playlist_track_focus = true;
                app.search_playlist_track_selected =
                    t.min(app.search_playlist_tracks.len().saturating_sub(1));
            }
            None => return false,
        },
        ListId::PlaylistList => match playlist_row_target(app, index) {
            Some(RowTarget::Playlist(i)) => {
                app.playlist_selected = i;
                app.playlist_track_focus = false;
            }
            Some(RowTarget::Track(t)) => {
                app.playlist_track_focus = true;
                app.playlist_track_selected = t.min(app.playlist_tracks.len().saturating_sub(1));
            }
            None => return false,
        },
        ListId::LibraryTracks => {
            app.library_selected = index;
        }
        ListId::LibraryGroups => {
            app.library_group_selected = index;
            app.library_group_track_selected = 0;
            app.library_group_focus = false;
        }
        ListId::LibraryGroupTracks => {
            app.library_group_track_selected = index;
            app.library_group_focus = true;
        }
        ListId::PodcastResults => {
            app.podcast_section = PodcastSection::Results;
            app.podcast_result_selected = index;
            app.podcast_episode_focus = false;
        }
        ListId::PodcastFollowed => {
            app.podcast_section = PodcastSection::Followed;
            app.podcast_followed_selected = index;
            app.podcast_episode_focus = false;
        }
        ListId::PodcastEpisodes => {
            app.podcast_episode_selected = index;
            app.podcast_episode_focus = true;
        }
        ListId::Queue => {
            app.queue_selected = index;
        }
        ListId::LibraryFolderDelete => {
            app.library_delete_confirm_selected = Some(index);
        }
    }
    true
}

/// A scrollable list region: its id, the content area (rows start at `area.y`),
/// the first visible row and the total number of rows.
#[derive(Clone, Copy)]
struct ListRegion {
    id: ListId,
    area: Rect,
    offset: usize,
    count: usize,
}

impl ListRegion {
    fn row_index(self, y: u16) -> Option<usize> {
        if y < self.area.y {
            return None;
        }
        let index = self.offset + (y - self.area.y) as usize;
        (index < self.count).then_some(index)
    }
}

async fn list_click(
    app: &mut App,
    daemon: &mut DaemonClient,
    state: &mut MouseState,
    region: ListRegion,
    x: u16,
    y: u16,
) -> anyhow::Result<()> {
    let Some(index) = region.row_index(y) else {
        return Ok(());
    };
    exit_input_modes(app);
    if !select_at(app, region.id, index) {
        return Ok(());
    }
    if state.is_double_click(ClickKey::List(region.id, index), x, y) {
        activate(app, daemon, region.id, index).await?;
    }
    Ok(())
}

/// Double click: run the primary action for the row (mirrors `Enter`/`T`).
async fn activate(
    app: &mut App,
    daemon: &mut DaemonClient,
    id: ListId,
    index: usize,
) -> anyhow::Result<()> {
    match id {
        ListId::SearchTracks => {
            if let Some(track) = app.search_results.get(index).cloned() {
                crate::play_or_queue_now(app, daemon, track).await;
                app.notify("Playing now!");
            }
        }
        ListId::SearchPlaylists => match search_playlist_row_target(app, index) {
            Some(RowTarget::Playlist(i)) => {
                if app.search_playlist_selected == i {
                    app.search_playlist_expanded = !app.search_playlist_expanded;
                    app.search_playlist_track_focus = false;
                    app.search_playlist_track_selected = 0;
                    crate::refresh_selected_search_playlist(app);
                }
            }
            Some(RowTarget::Track(t)) => {
                if let Some(track) = app.search_playlist_tracks.get(t).cloned() {
                    crate::play_or_queue_now(app, daemon, track).await;
                    app.notify("Playing now!");
                }
            }
            None => {}
        },
        ListId::PlaylistList => match playlist_row_target(app, index) {
            Some(RowTarget::Playlist(i)) => {
                if app.playlist_selected == i {
                    app.playlist_expanded = !app.playlist_expanded;
                    app.playlist_track_focus = false;
                    app.playlist_track_selected = 0;
                    crate::refresh_selected_playlist(app, daemon).await;
                }
            }
            Some(RowTarget::Track(t)) => {
                if let Some(track) = app.playlist_tracks.get(t).cloned() {
                    crate::play_or_queue_now(app, daemon, track).await;
                    app.notify("Playing now!");
                }
            }
            None => {}
        },
        ListId::LibraryTracks => {
            if let Some(track) = crate::library_get_filtered_track(app, index).cloned() {
                crate::play_or_queue_now(app, daemon, track).await;
                app.notify("Playing now!");
            }
        }
        ListId::LibraryGroups => {
            app.library_group_focus = true;
            app.library_group_track_selected = 0;
        }
        ListId::LibraryGroupTracks => {
            let groups = crate::library_current_groups(app);
            let group_sel = app.library_group_selected.min(groups.len().saturating_sub(1));
            if let Some((_, tracks)) = groups.get(group_sel) {
                if let Some(track) = tracks.get(index) {
                    crate::play_or_queue_now(app, daemon, track.clone()).await;
                    app.notify("Playing now!");
                }
            }
        }
        ListId::PodcastResults | ListId::PodcastFollowed => {
            if let Some(ch) = crate::selected_podcast_channel(app).cloned() {
                app.podcast_selected_feed = Some(ch.feed_url.clone());
                app.podcast_episodes.clear();
                app.podcast_episode_selected = 0;
                app.podcast_episode_filter.clear();
                app.podcast_episode_filter_cursor = 0;
                app.podcast_episode_focus = false;
                app.pending_podcast_episodes = Some(ch.feed_url);
            }
        }
        ListId::PodcastEpisodes => {
            if let Some(ep) = crate::filtered_episodes(app).get(index).cloned() {
                let track = crate::episode_to_track(ep);
                crate::play_or_queue_now(app, daemon, track).await;
                app.notify("Playing now!");
            }
        }
        ListId::Queue => {
            let _ = daemon.send(&Request::PlayIndex(index)).await;
            app.notify("Playing selected queue track");
        }
        ListId::LibraryFolderDelete => {
            remove_folder(app, daemon, index).await?;
        }
    }
    Ok(())
}

// --- Right click (add to queue) --------------------------------------------

async fn add_track_to_queue(app: &mut App, daemon: &mut DaemonClient, track: mutui_common::Track) {
    let name = track.title.clone();
    let _ = daemon.send(&Request::AddToQueue(track)).await;
    app.notify(format!("Added to queue: {name}"));
}

async fn right_click(
    app: &mut App,
    daemon: &mut DaemonClient,
    map: &MouseMap,
    x: u16,
    y: u16,
) -> anyhow::Result<()> {
    let Some(HitRegion::List(id, area, offset, count)) = map.find(x, y) else {
        return Ok(());
    };
    let region = ListRegion {
        id: *id,
        area: *area,
        offset: *offset,
        count: *count,
    };
    let Some(index) = region.row_index(y) else {
        return Ok(());
    };
    match region.id {
        ListId::SearchTracks => {
            if let Some(track) = app.search_results.get(index).cloned() {
                add_track_to_queue(app, daemon, track).await;
            }
        }
        ListId::SearchPlaylists => {
            match search_playlist_row_target(app, index) {
                Some(RowTarget::Track(t)) => {
                    if let Some(track) = app.search_playlist_tracks.get(t).cloned() {
                        add_track_to_queue(app, daemon, track).await;
                    }
                }
                Some(RowTarget::Playlist(i)) => {
                    if let Some(playlist) = app.search_playlist_results.get(i).cloned() {
                        let playlist_name = playlist.title.clone();
                        match daemon
                            .send(&Request::AddYoutubePlaylistToQueue(playlist.url.clone()))
                            .await
                        {
                            Ok(Response::Ok) => {
                                app.notify(format!("Playlist added to queue: {playlist_name}"));
                            }
                            Ok(Response::Error(e)) => {
                                app.notify(format!("Failed to add playlist: {e}"))
                            }
                            _ => {}
                        }
                    }
                }
                None => {}
            }
        }
        ListId::PlaylistList => {
            match playlist_row_target(app, index) {
                Some(RowTarget::Track(t)) => {
                    if let Some(track) = app.playlist_tracks.get(t).cloned() {
                        add_track_to_queue(app, daemon, track).await;
                    }
                }
                Some(RowTarget::Playlist(i)) => {
                    if let Some(name) = app.playlist_names.get(i).cloned() {
                        let _ = daemon.send(&Request::LoadPlaylist(name.clone())).await;
                        app.notify(format!("Playlist '{name}' loaded into queue"));
                    }
                }
                None => {}
            }
        }
        ListId::LibraryTracks => {
            if let Some(track) = crate::library_get_filtered_track(app, index).cloned() {
                add_track_to_queue(app, daemon, track).await;
            }
        }
        ListId::LibraryGroupTracks => {
            let groups = crate::library_current_groups(app);
            let group_sel = app.library_group_selected.min(groups.len().saturating_sub(1));
            if let Some((_, tracks)) = groups.get(group_sel) {
                if let Some(track) = tracks.get(index) {
                    add_track_to_queue(app, daemon, track.clone()).await;
                }
            }
        }
        ListId::PodcastEpisodes => {
            if let Some(ep) = crate::filtered_episodes(app).get(index).cloned() {
                let track = crate::episode_to_track(ep);
                add_track_to_queue(app, daemon, track).await;
            }
        }
        _ => {}
    }
    Ok(())
}

// --- Scroll wheel ----------------------------------------------------------

async fn scroll(
    app: &mut App,
    daemon: &mut DaemonClient,
    map: &MouseMap,
    x: u16,
    y: u16,
    down: bool,
) -> anyhow::Result<()> {
    if current_popup(app).is_some() {
        let in_folder_list = map
            .find(x, y)
            .is_some_and(|r| matches!(r, HitRegion::List(ListId::LibraryFolderDelete, ..)));
        if !in_folder_list {
            return Ok(());
        }
    }

    let Some(region) = map.find(x, y) else {
        return Ok(());
    };
    let HitRegion::List(id, ..) = region else {
        return Ok(());
    };

    let key = match id {
        ListId::Queue => {
            if down {
                KeyCode::Char('J')
            } else {
                KeyCode::Char('K')
            }
        }
        _ => {
            exit_input_modes(app);
            prepare_scroll_focus(app, *id);
            if down {
                KeyCode::Down
            } else {
                KeyCode::Up
            }
        }
    };
    crate::handle_key(app, daemon, event::KeyEvent::new(key, KeyModifiers::NONE)).await?;
    Ok(())
}

/// Before scrolling, point focus at the panel the cursor is hovering.
fn prepare_scroll_focus(app: &mut App, id: ListId) {
    match id {
        ListId::SearchTracks => app.search_section = SearchSection::Tracks,
        ListId::SearchPlaylists => app.search_section = SearchSection::Playlists,
        ListId::LibraryGroups => app.library_group_focus = false,
        ListId::LibraryGroupTracks => app.library_group_focus = true,
        ListId::PodcastResults => {
            app.podcast_episode_focus = false;
            app.podcast_section = PodcastSection::Results;
        }
        ListId::PodcastFollowed => {
            app.podcast_episode_focus = false;
            app.podcast_section = PodcastSection::Followed;
        }
        ListId::PodcastEpisodes => app.podcast_episode_focus = true,
        _ => {}
    }
}
