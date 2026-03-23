use anyhow::Result;
use log::{debug, error, info};
use mutui_common::{
    encode_message, DaemonStatus, PlayerState, Request, Response, Track,
};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

use crate::library;
use crate::mpv::MpvHandle;
use crate::playlist;
use crate::queue::Queue;
use crate::search;

pub struct Daemon {
    pub mpv: MpvHandle,
    pub queue: Queue,
    pub volume: i64,
    pub autoplay_enabled: bool,
    pub autoplay_results: Vec<Track>,
    pub autoplay_next_index: usize,
}

impl Daemon {
    pub fn new() -> Result<Self> {
        let mpv = MpvHandle::start()?;
        let volume = 80;
        let _ = mpv.set_volume(volume);

        Ok(Self {
            mpv,
            queue: Queue::new(),
            volume,
            autoplay_enabled: false,
            autoplay_results: Vec::new(),
            autoplay_next_index: 0,
        })
    }

    fn store_search_results(&mut self, tracks: &[Track]) {
        self.autoplay_results = tracks.to_vec();
        self.autoplay_next_index = 0;
    }

    fn append_next_autoplay_track(&mut self) -> bool {
        if !self.autoplay_enabled {
            return false;
        }

        let Some(track) = self.autoplay_results.get(self.autoplay_next_index).cloned() else {
            return false;
        };

        self.autoplay_next_index += 1;
        self.queue.add_autoplay(track);
        true
    }

    pub fn get_status(&self) -> DaemonStatus {
        let idle = self.mpv.is_idle();
        let paused = self.mpv.is_paused();

        let state = if idle || self.queue.is_empty() {
            PlayerState::Stopped
        } else if paused {
            PlayerState::Paused
        } else {
            PlayerState::Playing
        };

        DaemonStatus {
            state,
            current_track: self.queue.current_track().cloned(),
            position: self.mpv.get_time_pos(),
            duration: self.mpv.get_duration(),
            volume: self.mpv.get_volume(),
            queue: self.queue.tracks(),
            queue_index: self.queue.current,
            autoplay_enabled: self.autoplay_enabled,
            autoplay_queue_indices: self.queue.autoplay_indices(),
        }
    }

    fn play_current(&self) -> Result<()> {
        if let Some(track) = self.queue.current_track() {
            info!("Playing: {} - {}", track.title, track.artist);
            self.mpv.loadfile(&track.url)?;
            self.mpv.play()?;
        }
        Ok(())
    }

    pub fn handle_request(&mut self, req: Request) -> Response {
        match req {
            Request::Play => {
                if self.queue.current_track().is_some() {
                    if let Err(e) = self.mpv.play() {
                        return Response::Error(e.to_string());
                    }
                }
                Response::Ok
            }
            Request::Pause => {
                if let Err(e) = self.mpv.pause() {
                    return Response::Error(e.to_string());
                }
                Response::Ok
            }
            Request::Toggle => {
                if self.mpv.is_idle() {
                    if let Err(e) = self.play_current() {
                        return Response::Error(e.to_string());
                    }
                } else if let Err(e) = self.mpv.toggle_pause() {
                    return Response::Error(e.to_string());
                }
                Response::Ok
            }
            Request::Stop => {
                if let Err(e) = self.mpv.stop() {
                    return Response::Error(e.to_string());
                }
                Response::Ok
            }
            Request::Next => {
                if self.queue.next() {
                    if let Err(e) = self.play_current() {
                        return Response::Error(e.to_string());
                    }
                } else if self.append_next_autoplay_track() && self.queue.next() {
                    if let Err(e) = self.play_current() {
                        return Response::Error(e.to_string());
                    }
                } else {
                    let _ = self.mpv.stop();
                }
                Response::Ok
            }
            Request::Previous => {
                if self.queue.previous() {
                    if let Err(e) = self.play_current() {
                        return Response::Error(e.to_string());
                    }
                }
                Response::Ok
            }
            Request::Seek(pos) => {
                if let Err(e) = self.mpv.seek(pos) {
                    return Response::Error(e.to_string());
                }
                Response::Ok
            }
            Request::SetVolume(vol) => {
                let vol = vol.clamp(0, 150);
                self.volume = vol;
                if let Err(e) = self.mpv.set_volume(vol) {
                    return Response::Error(e.to_string());
                }
                Response::Ok
            }
            Request::AddToQueue(track) => {
                let was_empty = self.queue.is_empty();
                self.queue.add(track);
                if was_empty {
                    let _ = self.play_current();
                }
                Response::Ok
            }
            Request::InsertNext(track) => {
                let was_empty = self.queue.is_empty();
                self.queue.insert_next(track);
                if was_empty {
                    let _ = self.play_current();
                }
                Response::Ok
            }
            Request::RemoveFromQueue(index) => {
                let was_current = index == self.queue.current;
                self.queue.remove(index);
                if was_current {
                    if self.queue.is_empty() {
                        let _ = self.mpv.stop();
                    } else {
                        let _ = self.play_current();
                    }
                }
                Response::Ok
            }
            Request::ClearQueue => {
                self.queue.clear();
                self.autoplay_next_index = 0;
                let _ = self.mpv.stop();
                Response::Ok
            }
            Request::MoveInQueue { from, to } => {
                self.queue.move_track(from, to);
                Response::Ok
            }
            Request::PlayIndex(idx) => {
                if self.queue.set_index(idx) {
                    if let Err(e) = self.play_current() {
                        return Response::Error(e.to_string());
                    }
                }
                Response::Ok
            }
            Request::Search(_)
            | Request::SearchPlaylists(_)
            | Request::GetYoutubePlaylistTracks(_)
            | Request::LoadYoutubePlaylist(_)
            | Request::AddYoutubePlaylistToQueue(_) => {
                // Handled in handle_client to avoid blocking the daemon lock
                Response::Error("Search should be handled by handle_client".into())
            }
            Request::ToggleAutoplay => {
                self.autoplay_enabled = !self.autoplay_enabled;
                Response::Ok
            }
            Request::ListPlaylists => match playlist::list() {
                Ok(names) => Response::Playlists(names),
                Err(e) => Response::Error(e.to_string()),
            },
            Request::GetPlaylist(name) => match playlist::load(&name) {
                Ok(pl) => Response::Playlist(pl),
                Err(e) => Response::Error(e.to_string()),
            },
            Request::SavePlaylist(pl) => match playlist::save(&pl) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e.to_string()),
            },
            Request::DeletePlaylist(name) => match playlist::delete(&name) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e.to_string()),
            },
            Request::LoadPlaylist(name) => match playlist::load(&name) {
                Ok(pl) => {
                    self.queue.clear();
                    self.autoplay_next_index = 0;
                    let _ = self.mpv.stop();
                    for track in pl.tracks {
                        self.queue.add(track);
                    }
                    if !self.queue.is_empty() {
                        let _ = self.play_current();
                    }
                    Response::Ok
                }
                Err(e) => Response::Error(e.to_string()),
            },
            Request::AddLibraryFolder(folder) => match library::add_folder(&folder) {
                Ok(folders) => Response::LibraryFolders(folders),
                Err(e) => Response::Error(e.to_string()),
            },
            Request::RemoveLibraryFolder(folder) => match library::remove_folder(&folder) {
                Ok(folders) => Response::LibraryFolders(folders),
                Err(e) => Response::Error(e.to_string()),
            },
            Request::ListLibraryFolders => Response::LibraryFolders(library::list_folders()),
            Request::ScanLibrary => {
                let tracks = library::scan();
                Response::LibraryTracks(tracks)
            }
            Request::GetStatus => {
                let status = self.get_status();
                Response::Status(Box::new(status))
            }
            Request::Shutdown => Response::Ok,
        }
    }
}

/// Check if the mpv playback has ended and auto-advance to the next track.
pub async fn check_track_ended(daemon: &Arc<Mutex<Daemon>>) {
    let mut d = daemon.lock().await;
    if d.queue.is_empty() {
        return;
    }

    let idle = d.mpv.is_idle();
    if idle && d.queue.current_track().is_some() {
        if d.queue.next() {
            let _ = d.play_current();
            return;
        }

        if d.append_next_autoplay_track() && d.queue.next() {
            let _ = d.play_current();
        }
    }
}

async fn handle_client(stream: UnixStream, daemon: Arc<Mutex<Daemon>>) {
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let mut len_line = String::new();
    let mut json_line = String::new();

    loop {
        len_line.clear();
        json_line.clear();

        match reader.read_line(&mut len_line).await {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                debug!("Client read error: {e}");
                break;
            }
        }

        let _expected_len: usize = match len_line.trim().parse() {
            Ok(n) => n,
            Err(_) => {
                debug!("Invalid frame length: {:?}", len_line.trim());
                break;
            }
        };

        match reader.read_line(&mut json_line).await {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                debug!("Client read error: {e}");
                break;
            }
        }

        let req: Request = match serde_json::from_str(json_line.trim()) {
            Ok(r) => r,
            Err(e) => {
                error!("Invalid request: {e}");
                let resp = Response::Error(format!("Invalid request: {e}"));
                if let Ok(frame) = encode_message(&resp) {
                    let _ = writer.write_all(&frame).await;
                }
                continue;
            }
        };

        let is_shutdown = matches!(&req, Request::Shutdown);

        let resp = match req {
            // Handle slow/search and disk-heavy requests without holding the
            // global daemon lock, so status/playback requests stay responsive.
            Request::Search(query) => match search::search(&query, 15).await {
                Ok(tracks) => {
                    let mut d = daemon.lock().await;
                    d.store_search_results(&tracks);
                    Response::SearchResults(tracks)
                }
                Err(e) => Response::Error(e.to_string()),
            },
            Request::SearchPlaylists(query) => match search::search_playlists(&query, 15).await {
                Ok(playlists) => Response::SearchResults(playlists),
                Err(e) => Response::Error(e.to_string()),
            },
            Request::GetYoutubePlaylistTracks(url_or_id) => {
                match search::load_youtube_playlist(&url_or_id, 500).await {
                    Ok(tracks) => Response::SearchResults(tracks),
                    Err(e) => Response::Error(e.to_string()),
                }
            }
            Request::LoadYoutubePlaylist(url_or_id) => {
                match search::load_youtube_playlist(&url_or_id, 500).await {
                    Ok(tracks) => {
                        let mut d = daemon.lock().await;
                        d.queue.clear();
                        d.autoplay_next_index = 0;
                        let _ = d.mpv.stop();
                        for track in tracks {
                            d.queue.add(track);
                        }
                        if !d.queue.is_empty() {
                            let _ = d.play_current();
                        }
                        Response::Ok
                    }
                    Err(e) => Response::Error(e.to_string()),
                }
            }
            Request::AddYoutubePlaylistToQueue(url_or_id) => {
                match search::load_youtube_playlist(&url_or_id, 500).await {
                    Ok(tracks) => {
                        if tracks.is_empty() {
                            Response::Error("Playlist has no tracks".to_string())
                        } else {
                            let mut d = daemon.lock().await;
                            let was_empty = d.queue.is_empty();
                            for track in tracks {
                                d.queue.add(track);
                            }
                            if was_empty {
                                let _ = d.play_current();
                            }
                            Response::Ok
                        }
                    }
                    Err(e) => Response::Error(e.to_string()),
                }
            }
            Request::ListPlaylists => match playlist::list() {
                Ok(names) => Response::Playlists(names),
                Err(e) => Response::Error(e.to_string()),
            },
            Request::GetPlaylist(name) => match playlist::load(&name) {
                Ok(pl) => Response::Playlist(pl),
                Err(e) => Response::Error(e.to_string()),
            },
            Request::SavePlaylist(pl) => match playlist::save(&pl) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e.to_string()),
            },
            Request::DeletePlaylist(name) => match playlist::delete(&name) {
                Ok(()) => Response::Ok,
                Err(e) => Response::Error(e.to_string()),
            },
            Request::AddLibraryFolder(folder) => match library::add_folder(&folder) {
                Ok(folders) => Response::LibraryFolders(folders),
                Err(e) => Response::Error(e.to_string()),
            },
            Request::RemoveLibraryFolder(folder) => match library::remove_folder(&folder) {
                Ok(folders) => Response::LibraryFolders(folders),
                Err(e) => Response::Error(e.to_string()),
            },
            Request::ListLibraryFolders => Response::LibraryFolders(library::list_folders()),
            Request::ScanLibrary => {
                let tracks = library::scan();
                Response::LibraryTracks(tracks)
            }
            Request::LoadPlaylist(name) => match playlist::load(&name) {
                Ok(pl) => {
                    let mut d = daemon.lock().await;
                    d.queue.clear();
                    d.autoplay_next_index = 0;
                    let _ = d.mpv.stop();
                    for track in pl.tracks {
                        d.queue.add(track);
                    }
                    if !d.queue.is_empty() {
                        let _ = d.play_current();
                    }
                    Response::Ok
                }
                Err(e) => Response::Error(e.to_string()),
            },
            other => {
                let mut d = daemon.lock().await;
                d.handle_request(other)
            }
        };

        if let Ok(frame) = encode_message(&resp) {
            if writer.write_all(&frame).await.is_err() {
                break;
            }
            let _ = writer.flush().await;
        }

        if is_shutdown {
            info!("Shutdown requested, stopping mpv and exiting...");
            {
                let d = daemon.lock().await;
                d.mpv.shutdown();
            }
            let _ = std::fs::remove_file(mutui_common::socket_path());
            std::process::exit(0);
        }
    }
}

pub async fn run(daemon: Arc<Mutex<Daemon>>) -> Result<()> {
    let socket_path = mutui_common::socket_path();

    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path)?;
    info!("Daemon listening on {}", socket_path.display());

    let daemon_bg = Arc::clone(&daemon);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            check_track_ended(&daemon_bg).await;
        }
    });

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                debug!("New client connected");
                let d = Arc::clone(&daemon);
                tokio::spawn(handle_client(stream, d));
            }
            Err(e) => {
                error!("Accept error: {e}");
            }
        }
    }
}
