use anyhow::{Context, Result};
use log::{debug, info, warn};
use mutui_common::Track;
use serde::Deserialize;
use tokio::process::Command;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct YtDlpEntry {
    id: Option<String>,
    title: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    artist: Option<String>,
    duration: Option<f64>,
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
            duration: self.duration,
            url,
        })
    }
}

pub async fn search(query: &str, max_results: usize) -> Result<Vec<Track>> {
    info!("Searching for: {query}");

    let search_query = format!("ytsearch{max_results}:{query}");

    let output = Command::new("yt-dlp")
        .arg(&search_query)
        .arg("--dump-json")
        .arg("--flat-playlist")
        .arg("--no-download")
        .arg("--no-warnings")
        .arg("--ignore-errors")
        .output()
        .await
        .context("Failed to run yt-dlp. Is yt-dlp installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("yt-dlp exited with error: {stderr}");
    }

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
