use anyhow::{Context, Result};
use log::{debug, info, warn};
use mutui_common::Track;
use serde::Deserialize;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct YtDlpEntry {
    id: Option<String>,
    #[serde(rename = "_type")]
    entry_type: Option<String>,
    title: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    artist: Option<String>,
    duration: Option<f64>,
    playlist_count: Option<u64>,
    url: Option<String>,
    webpage_url: Option<String>,
    original_url: Option<String>,
}

impl YtDlpEntry {
    fn into_track(self) -> Option<Track> {
        let id = self.id?;
        let title = self.title.unwrap_or_else(|| "Unknown".into());
        let artist = self
            .artist
            .or(self.channel)
            .or(self.uploader)
            .unwrap_or_else(|| "Unknown".into());
        let url = self
            .webpage_url
            .or(self.original_url)
            .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={id}"));

        Some(Track {
            id,
            title,
            artist,
            album: None,
            duration: self.duration,
            url,
        })
    }

    fn into_playlist_stub(self) -> Option<Track> {
        let raw_id = self.id?;
        let title = self.title.unwrap_or_else(|| "Unknown playlist".into());
        let owner = self
            .artist
            .or(self.channel)
            .or(self.uploader)
            .unwrap_or_else(|| "Unknown".into());

        let playlist_count = self.playlist_count;

        let mut playlist_id = self
            .webpage_url
            .as_deref()
            .and_then(extract_list_id)
            .or_else(|| self.original_url.as_deref().and_then(extract_list_id))
            .or_else(|| self.url.as_deref().and_then(extract_list_id));

        if playlist_id.is_none() && raw_id.len() != 11 {
            playlist_id = Some(raw_id.clone());
        }

        let playlist_id = playlist_id?;

        let url = format!("https://www.youtube.com/playlist?list={playlist_id}");

        Some(Track {
            id: playlist_id,
            title,
            artist: owner,
            // Encode playlist metadata for TUI rendering.
            album: Some(match playlist_count {
                Some(count) => format!("youtube-playlist:{count}"),
                None => "youtube-playlist".to_string(),
            }),
            duration: None,
            url,
        })
    }

    fn looks_like_playlist(&self) -> bool {
        self.entry_type.as_deref() == Some("playlist")
            || self.playlist_count.is_some()
            || self
                .webpage_url
                .as_deref()
                .map(|url| url.contains("/playlist") || url.contains("list="))
                .unwrap_or(false)
            || self
                .original_url
                .as_deref()
                .map(|url| url.contains("/playlist") || url.contains("list="))
                .unwrap_or(false)
            || self
                .url
                .as_deref()
                .map(|url| url.contains("/playlist") || url.contains("list="))
                .unwrap_or(false)
            || self.id.as_ref().map(|id| id.len() != 11).unwrap_or(false)
    }
}

fn extract_list_id(url: &str) -> Option<String> {
    let list_pos = url.find("list=")?;
    let list_value = &url[(list_pos + 5)..];
    let end = list_value.find('&').unwrap_or(list_value.len());
    let id = &list_value[..end];
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn url_encode_query(query: &str) -> String {
    let mut out = String::new();
    for b in query.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

async fn run_yt_dlp(args: &[String], operation: &str) -> Result<std::process::Output> {
    let mut cmd = Command::new("yt-dlp");
    cmd.args(args);

    let output = timeout(Duration::from_secs(20), cmd.output())
        .await
        .with_context(|| format!("yt-dlp timed out while trying to {operation}"))?
        .context("Failed to run yt-dlp. Is yt-dlp installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("yt-dlp exited with error during {operation}: {stderr}");
    }

    Ok(output)
}

fn parse_playlist_stubs(stdout: &str) -> Vec<Track> {
    let mut playlists = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match serde_json::from_str::<YtDlpEntry>(line) {
            Ok(entry) => {
                if !entry.looks_like_playlist() {
                    continue;
                }

                if let Some(playlist) = entry.into_playlist_stub() {
                    debug!("Found playlist candidate: {}", playlist.title);
                    playlists.push(playlist);
                }
            }
            Err(e) => {
                debug!("Failed to parse yt-dlp playlist entry: {e}");
            }
        }
    }

    playlists
}

pub async fn search(query: &str, max_results: usize) -> Result<Vec<Track>> {
    info!("Searching for: {query}");

    let search_query = format!("ytsearch{max_results}:{query}");

    let output = run_yt_dlp(
        &[
            search_query,
            "--dump-json".to_string(),
            "--flat-playlist".to_string(),
            "--no-download".to_string(),
            "--no-warnings".to_string(),
            "--ignore-errors".to_string(),
        ],
        "search tracks",
    )
    .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut tracks = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<YtDlpEntry>(line) {
            Ok(entry) => {
                if let Some(track) = entry.into_track() {
                    debug!("Found: {} - {}", track.title, track.artist);
                    tracks.push(track);
                }
            }
            Err(e) => {
                debug!("Failed to parse yt-dlp entry: {e}");
            }
        }
    }

    info!("Search returned {} results", tracks.len());
    Ok(tracks)
}

pub async fn search_playlists(query: &str, max_results: usize) -> Result<Vec<Track>> {
    info!("Searching playlists for: {query}");

    // YouTube `sp=EgIQAw%3D%3D` applies the "Playlist" filter in search.
    let encoded_query = url_encode_query(query);
    let search_url =
        format!("https://www.youtube.com/results?search_query={encoded_query}&sp=EgIQAw%3D%3D");

    let output = run_yt_dlp(
        &[
            search_url,
            "--dump-json".to_string(),
            "--flat-playlist".to_string(),
            "--playlist-end".to_string(),
            max_results.to_string(),
            "--no-download".to_string(),
            "--no-warnings".to_string(),
            "--ignore-errors".to_string(),
        ],
        "search playlists",
    )
    .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let playlists = parse_playlist_stubs(&stdout);

    info!("Playlist search returned {} results", playlists.len());
    Ok(playlists)
}

pub async fn load_youtube_playlist(
    playlist_url_or_id: &str,
    max_tracks: usize,
) -> Result<Vec<Track>> {
    let target = if playlist_url_or_id.starts_with("http://")
        || playlist_url_or_id.starts_with("https://")
    {
        playlist_url_or_id.to_string()
    } else {
        format!("https://www.youtube.com/playlist?list={playlist_url_or_id}")
    };

    info!("Loading YouTube playlist: {target}");

    let output = run_yt_dlp(
        &[
            target,
            "--dump-json".to_string(),
            "--flat-playlist".to_string(),
            "--no-download".to_string(),
            "--no-warnings".to_string(),
            "--ignore-errors".to_string(),
        ],
        "load YouTube playlist",
    )
    .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut tracks = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match serde_json::from_str::<YtDlpEntry>(line) {
            Ok(entry) => {
                if let Some(track) = entry.into_track() {
                    tracks.push(track);
                    if tracks.len() >= max_tracks {
                        break;
                    }
                }
            }
            Err(e) => {
                debug!("Failed to parse yt-dlp playlist track entry: {e}");
            }
        }
    }

    info!("Loaded {} tracks from YouTube playlist", tracks.len());
    Ok(tracks)
}
