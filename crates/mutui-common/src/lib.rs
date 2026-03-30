use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// --- Data Types ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub state: PlayerState,
    pub current_track: Option<Track>,
    pub position: f64,
    pub duration: f64,
    pub volume: i64,
    pub queue: Vec<Track>,
    pub queue_index: usize,
    #[serde(default)]
    pub autoplay_enabled: bool,
    #[serde(default)]
    pub autoplay_queue_indices: Vec<usize>,
}

impl Default for DaemonStatus {
    fn default() -> Self {
        Self {
            state: PlayerState::Stopped,
            current_track: None,
            position: 0.0,
            duration: 0.0,
            volume: 80,
            queue: Vec::new(),
            queue_index: 0,
            autoplay_enabled: false,
            autoplay_queue_indices: Vec::new(),
        }
    }
}

// --- IPC Protocol ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    // Playback
    Play,
    Pause,
    Toggle,
    Stop,
    Next,
    Previous,
    Seek(f64),
    SetVolume(i64),

    // Queue
    AddToQueue(Track),
    InsertNext(Track),
    RemoveFromQueue(usize),
    ClearQueue,
    MoveInQueue { from: usize, to: usize },
    PlayIndex(usize),

    // Search
    Search(String),
    SearchPlaylists(String),
    GetYoutubePlaylistTracks(String),
    LoadYoutubePlaylist(String),
    AddYoutubePlaylistToQueue(String),
    ToggleAutoplay,

    // Playlists
    ListPlaylists,
    GetPlaylist(String),
    SavePlaylist(Playlist),
    DeletePlaylist(String),
    LoadPlaylist(String),

    // Library
    AddLibraryFolder(String),
    RemoveLibraryFolder(String),
    ListLibraryFolders,
    ScanLibrary,

    // Status
    GetStatus,

    // Daemon control
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Status(Box<DaemonStatus>),
    SearchResults(Vec<Track>),
    Playlists(Vec<String>),
    Playlist(Playlist),
    LibraryFolders(Vec<String>),
    LibraryTracks(Vec<Track>),
    Error(String),
}

// --- Paths ---

pub fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir).join("mutui.sock")
}

pub fn mpv_socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir).join("mutui-mpv.sock")
}

pub fn data_dir() -> PathBuf {
    let data_dir = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{home}/.local/share")
    });
    let path = PathBuf::from(data_dir).join("mutui");
    std::fs::create_dir_all(&path).ok();
    path
}

pub fn playlists_dir() -> PathBuf {
    let path = data_dir().join("playlists");
    std::fs::create_dir_all(&path).ok();
    path
}

pub fn library_config_path() -> PathBuf {
    data_dir().join("library.json")
}

// --- IPC Framing helpers ---

/// Encode a message as a length-prefixed JSON frame: `<len>\n<json>\n`
pub fn encode_message<T: Serialize>(msg: &T) -> Result<Vec<u8>, serde_json::Error> {
    let json = serde_json::to_string(msg)?;
    let frame = format!("{}\n{}\n", json.len(), json);
    Ok(frame.into_bytes())
}
