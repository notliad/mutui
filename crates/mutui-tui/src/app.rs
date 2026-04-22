use mutui_common::{DaemonStatus, PodcastChannel, PodcastEpisode, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Search,
    Playlists,
    Library,
    Podcasts,
}

impl View {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Search => "1 Search",
            Self::Playlists => "2 Playlists",
            Self::Library => "3 Library",
            Self::Podcasts => "4 Podcasts",
        }
    }

    pub fn all() -> &'static [View] {
        &[View::Search, View::Playlists, View::Library, View::Podcasts]
    }

    pub fn next(&self) -> View {
        match self {
            Self::Search => Self::Playlists,
            Self::Playlists => Self::Library,
            Self::Library => Self::Podcasts,
            Self::Podcasts => Self::Search,
        }
    }

    pub fn prev(&self) -> View {
        match self {
            Self::Search => Self::Podcasts,
            Self::Playlists => Self::Search,
            Self::Library => Self::Playlists,
            Self::Podcasts => Self::Library,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    PlaylistName,
    LibraryFolderPath,
    LibraryFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryMode {
    #[default]
    AllTracks,
    ByArtist,
    ByAlbum,
}

impl LibraryMode {
    pub fn next(self) -> Self {
        match self {
            Self::AllTracks => Self::ByArtist,
            Self::ByArtist => Self::ByAlbum,
            Self::ByAlbum => Self::AllTracks,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::AllTracks => "Tracks",
            Self::ByArtist => "Artists",
            Self::ByAlbum => "Albums",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistView {
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchSection {
    #[default]
    Tracks,
    Playlists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PodcastSection {
    Results,
    #[default]
    Followed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HelpPopupPage {
    #[default]
    Shortcuts,
    About,
}

impl HelpPopupPage {
    pub fn next(self) -> Self {
        match self {
            Self::Shortcuts => Self::About,
            Self::About => Self::Shortcuts,
        }
    }

    pub fn prev(self) -> Self {
        self.next()
    }
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
    pub search_section: SearchSection,
    pub search_results: Vec<Track>,
    pub search_selected: usize,
    pub search_playlist_results: Vec<Track>,
    pub search_playlist_selected: usize,
    pub search_playlist_expanded: bool,
    pub search_playlist_loading: bool,
    pub pending_search_playlist_url: Option<String>,
    pub pending_search_playlist_id: Option<String>,
    pub search_playlist_tracks: Vec<Track>,
    pub search_playlist_track_selected: usize,
    pub search_playlist_track_focus: bool,
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
    pub playlist_expanded: bool,

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
    pub help_popup_page: HelpPopupPage,

    // Confirm delete playlist popup
    pub playlist_delete_confirm_name: Option<String>,
    pub library_delete_confirm_selected: Option<usize>,

    // Library
    pub library_folders: Vec<String>,
    pub library_tracks: Vec<Track>,
    pub library_selected: usize,
    pub library_folder_input: String,
    pub library_folder_cursor: usize,
    pub library_mode: LibraryMode,
    pub library_filter: String,
    pub library_filter_cursor: usize,
    // Grouped view (Artists / Albums)
    pub library_group_selected: usize,
    pub library_group_track_selected: usize,
    pub library_group_focus: bool,

    // Podcasts
    pub podcast_search_input: String,
    pub podcast_search_cursor: usize,
    pub podcast_search_results: Vec<PodcastChannel>,
    pub podcast_searching: bool,
    pub pending_podcast_search: Option<String>,
    pub podcast_followed: Vec<PodcastChannel>,
    /// None = showing search/followed list; Some(feed_url) = showing episodes
    pub podcast_selected_feed: Option<String>,
    pub podcast_episodes: Vec<PodcastEpisode>,
    pub podcast_episode_selected: usize,
    pub podcast_episodes_loading: bool,
    pub pending_podcast_episodes: Option<String>,
    /// Which column has focus: false = channel panel, true = episode list
    pub podcast_episode_focus: bool,
    pub podcast_section: PodcastSection,
    pub podcast_result_selected: usize,
    pub podcast_followed_selected: usize,
    pub podcast_input_mode: bool,
    /// Last search/fetch error, shown persistently in the panel.
    pub podcast_last_error: Option<String>,
    pub podcast_episode_filter: String,
    pub podcast_episode_filter_cursor: usize,
    pub podcast_episode_filter_mode: bool,
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
            search_section: SearchSection::Tracks,
            search_results: Vec::new(),
            search_selected: 0,
            search_playlist_results: Vec::new(),
            search_playlist_selected: 0,
            search_playlist_expanded: false,
            search_playlist_loading: false,
            pending_search_playlist_url: None,
            pending_search_playlist_id: None,
            search_playlist_tracks: Vec::new(),
            search_playlist_track_selected: 0,
            search_playlist_track_focus: false,
            searching: false,
            pending_search_query: None,
            search_spinner_frame: 0,
            playlist_names: Vec::new(),
            playlist_selected: 0,
            playlist_view: PlaylistView::List,
            playlist_tracks: Vec::new(),
            playlist_track_selected: 0,
            playlist_track_focus: false,
            playlist_expanded: false,
            new_playlist_name: String::new(),
            new_playlist_cursor: 0,
            notification: None,
            notification_timer: 0,
            queue_selected: 0,
            show_shortcuts_popup: false,
            help_popup_page: HelpPopupPage::default(),
            playlist_delete_confirm_name: None,
            library_delete_confirm_selected: None,
            library_folders: Vec::new(),
            library_tracks: Vec::new(),
            library_selected: 0,
            library_folder_input: String::new(),
            library_folder_cursor: 0,
            library_mode: LibraryMode::ByArtist,
            library_filter: String::new(),
            library_filter_cursor: 0,
            library_group_selected: 0,
            library_group_track_selected: 0,
            library_group_focus: false,
            podcast_search_input: String::new(),
            podcast_search_cursor: 0,
            podcast_search_results: Vec::new(),
            podcast_searching: false,
            pending_podcast_search: None,
            podcast_followed: Vec::new(),
            podcast_selected_feed: None,
            podcast_episodes: Vec::new(),
            podcast_episode_selected: 0,
            podcast_episodes_loading: false,
            pending_podcast_episodes: None,
            podcast_episode_focus: false,
            podcast_section: PodcastSection::Followed,
            podcast_result_selected: 0,
            podcast_followed_selected: 0,
            podcast_input_mode: false,
            podcast_last_error: None,
            podcast_episode_filter: String::new(),
            podcast_episode_filter_cursor: 0,
            podcast_episode_filter_mode: false,
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

    pub fn selected_search_playlist(&self) -> Option<&Track> {
        self.search_playlist_results
            .get(self.search_playlist_selected)
    }

    pub fn selected_queue_track(&self) -> Option<usize> {
        if self.status.queue.is_empty() {
            None
        } else {
            Some(self.queue_selected.min(self.status.queue.len().saturating_sub(1)))
        }
    }

}
