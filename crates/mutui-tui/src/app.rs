use mutui_common::{DaemonStatus, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Search,
    Playlists,
}

impl View {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Search => "Search",
            Self::Playlists => "Playlists",
        }
    }

    pub fn all() -> &'static [View] {
        &[View::Search, View::Playlists]
    }

    pub fn next(&self) -> View {
        match self {
            Self::Search => Self::Playlists,
            Self::Playlists => Self::Search,
        }
    }

    pub fn prev(&self) -> View {
        match self {
            Self::Search => Self::Playlists,
            Self::Playlists => Self::Search,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    PlaylistName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistView {
    List,
}

pub struct App {
    pub should_quit: bool,
    pub view: View,
    pub input_mode: InputMode,

    // Status from daemon
    pub status: DaemonStatus,

    // Search
    pub search_input: String,
    pub search_cursor: usize,
    pub search_selection_anchor: Option<usize>,
    pub search_results: Vec<Track>,
    pub search_selected: usize,
    pub searching: bool,
    pub pending_search_query: Option<String>,
    pub search_spinner_frame: u8,

    // Playlists
    pub playlist_names: Vec<String>,
    pub playlist_selected: usize,
    pub playlist_view: PlaylistView,
    pub playlist_tracks: Vec<Track>,
    pub playlist_track_selected: usize,
    pub playlist_track_focus: bool,

    // Playlist name input (for saving)
    pub new_playlist_name: String,
    pub new_playlist_cursor: usize,

    // Notification
    pub notification: Option<String>,
    pub notification_timer: u8,

    // Side queue selection
    pub queue_selected: usize,

    // Help popup
    pub show_shortcuts_popup: bool,

    // Confirm delete playlist popup
    pub playlist_delete_confirm_name: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            view: View::Search,
            input_mode: InputMode::Normal,
            status: DaemonStatus::default(),
            search_input: String::new(),
            search_cursor: 0,
            search_selection_anchor: None,
            search_results: Vec::new(),
            search_selected: 0,
            searching: false,
            pending_search_query: None,
            search_spinner_frame: 0,
            playlist_names: Vec::new(),
            playlist_selected: 0,
            playlist_view: PlaylistView::List,
            playlist_tracks: Vec::new(),
            playlist_track_selected: 0,
            playlist_track_focus: false,
            new_playlist_name: String::new(),
            new_playlist_cursor: 0,
            notification: None,
            notification_timer: 0,
            queue_selected: 0,
            show_shortcuts_popup: false,
            playlist_delete_confirm_name: None,
        }
    }

    pub fn notify(&mut self, msg: impl Into<String>) {
        self.notification = Some(msg.into());
        self.notification_timer = 6; // ~3 seconds at 500ms tick
    }

    pub fn tick_notification(&mut self) {
        if self.notification_timer > 0 {
            self.notification_timer -= 1;
            if self.notification_timer == 0 {
                self.notification = None;
            }
        }
    }

    pub fn selected_search_track(&self) -> Option<&Track> {
        self.search_results.get(self.search_selected)
    }

    pub fn selected_queue_track(&self) -> Option<usize> {
        if self.status.queue.is_empty() {
            None
        } else {
            Some(self.queue_selected.min(self.status.queue.len().saturating_sub(1)))
        }
    }

}
