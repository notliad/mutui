use anyhow::{Context, Result};
use log::{info, warn};
use mutui_common::Playlist;
use std::path::PathBuf;

fn playlists_dir() -> PathBuf {
    mutui_common::playlists_dir()
}

fn playlist_path(name: &str) -> PathBuf {
    // Sanitize name to avoid path traversal
    let safe_name: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    playlists_dir().join(format!("{safe_name}.json"))
}

pub fn list() -> Result<Vec<String>> {
    let dir = playlists_dir();
    let mut names = Vec::new();

    if !dir.exists() {
        return Ok(names);
    }

    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }

    names.sort();
    Ok(names)
}

pub fn load(name: &str) -> Result<Playlist> {
    let path = playlist_path(name);
    let data =
        std::fs::read_to_string(&path).with_context(|| format!("Playlist '{}' not found", name))?;
    let playlist: Playlist = serde_json::from_str(&data)?;
    Ok(playlist)
}

pub fn save(playlist: &Playlist) -> Result<()> {
    let path = playlist_path(&playlist.name);
    let data = serde_json::to_string_pretty(playlist)?;
    std::fs::write(&path, data)?;
    info!(
        "Saved playlist '{}' with {} tracks",
        playlist.name,
        playlist.tracks.len()
    );
    Ok(())
}

pub fn delete(name: &str) -> Result<()> {
    let path = playlist_path(name);
    if path.exists() {
        std::fs::remove_file(&path)?;
        info!("Deleted playlist '{}'", name);
    } else {
        warn!("Playlist '{}' not found for deletion", name);
    }
    Ok(())
}
