use anyhow::Result;
use log::{debug, info};
use lofty::prelude::{Accessor, AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use mutui_common::Track;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "opus", "wav", "m4a", "aac", "wma", "alac", "aiff", "ape", "wv",
];

// --- Cache ---

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct CacheEntry {
    mtime: u64,
    track: Track,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
struct LibraryCache {
    entries: HashMap<String, CacheEntry>,
}

fn cache_path() -> PathBuf {
    mutui_common::data_dir().join("library_cache.bin")
}

fn load_cache() -> LibraryCache {
    let path = cache_path();
    if !path.exists() {
        return LibraryCache::default();
    }
    match std::fs::read(&path) {
        Ok(data) => bincode::deserialize(&data).unwrap_or_default(),
        Err(_) => LibraryCache::default(),
    }
}

fn save_cache(cache: &LibraryCache) {
    if let Ok(data) = bincode::serialize(cache) {
        let _ = std::fs::write(cache_path(), data);
    }
}

fn file_mtime(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
        .unwrap_or(0)
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct LibraryConfig {
    pub folders: Vec<String>,
}

pub fn config_path() -> PathBuf {
    mutui_common::library_config_path()
}

pub fn load_config() -> LibraryConfig {
    let path = config_path();
    if !path.exists() {
        return LibraryConfig::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => LibraryConfig::default(),
    }
}

pub fn save_config(config: &LibraryConfig) -> Result<()> {
    let path = config_path();
    let data = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, data)?;
    Ok(())
}

pub fn add_folder(folder: &str) -> Result<Vec<String>> {
    let mut config = load_config();
    let canonical = std::fs::canonicalize(folder)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| folder.to_string());

    if !config.folders.contains(&canonical) {
        config.folders.push(canonical);
        save_config(&config)?;
        info!("Added library folder: {folder}");
    }
    Ok(config.folders)
}

pub fn remove_folder(folder: &str) -> Result<Vec<String>> {
    let mut config = load_config();
    config.folders.retain(|f| f != folder);
    save_config(&config)?;
    info!("Removed library folder: {folder}");
    Ok(config.folders)
}

pub fn list_folders() -> Vec<String> {
    load_config().folders
}

pub fn scan() -> Vec<Track> {
    let config = load_config();
    let mut cache = load_cache();
    let mut cache_dirty = false;
    let mut visited = std::collections::HashSet::new();
    let mut tracks = Vec::new();

    for folder in &config.folders {
        let path = Path::new(folder);
        if path.is_dir() {
            scan_dir(path, &mut tracks, &mut cache, &mut cache_dirty, &mut visited);
        }
    }

    // Evict cache entries for files that no longer exist on disk.
    let before = cache.entries.len();
    cache.entries.retain(|p, _| visited.contains(p.as_str()));
    if cache.entries.len() != before {
        cache_dirty = true;
    }

    if cache_dirty {
        save_cache(&cache);
    }

    tracks.sort_by(|a, b| a.title.cmp(&b.title));
    info!("Library scan: {} tracks found", tracks.len());
    tracks
}

fn scan_dir(
    dir: &Path,
    tracks: &mut Vec<Track>,
    cache: &mut LibraryCache,
    cache_dirty: &mut bool,
    visited: &mut std::collections::HashSet<String>,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            debug!("Cannot read directory {}: {e}", dir.display());
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, tracks, cache, cache_dirty, visited);
        } else if is_audio_file(&path) {
            let path_str = path.to_string_lossy().to_string();
            visited.insert(path_str.clone());
            let mtime = file_mtime(&path);

            // Return the cached track if the file hasn't changed since last scan.
            if let Some(entry) = cache.entries.get(&path_str) {
                if entry.mtime == mtime {
                    tracks.push(entry.track.clone());
                    continue;
                }
            }

            // Cache miss or stale — read tags from disk and update cache.
            if let Some(track) = file_to_track(&path) {
                cache.entries.insert(path_str, CacheEntry { mtime, track: track.clone() });
                *cache_dirty = true;
                tracks.push(track);
            }
        }
    }
}

fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn file_to_track(path: &Path) -> Option<Track> {
    let path_str = path.to_string_lossy().to_string();

    let mut hasher = DefaultHasher::new();
    path_str.hash(&mut hasher);
    let id = format!("local_{:x}", hasher.finish());

    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown");

    // Filename-based fallback values
    let (fallback_artist, fallback_title) = if let Some((a, t)) = file_stem.split_once(" - ") {
        (a.trim().to_string(), t.trim().to_string())
    } else {
        ("Unknown Artist".to_string(), file_stem.to_string())
    };
    let fallback_album = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string());

    // Try to read real audio tags via lofty
    match Probe::open(path).and_then(|p| p.read()) {
        Ok(tagged_file) => {
            let duration = Some(
                tagged_file
                    .properties()
                    .duration()
                    .as_secs_f64(),
            );

            // lofty tries tags in priority order; take the first populated tag
            let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

            let (title, artist, album) = if let Some(tag) = tag {
                let title = tag
                    .title()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or(fallback_title);
                let artist = tag
                    .artist()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .unwrap_or(fallback_artist);
                let album = tag
                    .album()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .or(fallback_album);
                (title, artist, album)
            } else {
                (fallback_title, fallback_artist, fallback_album)
            };

            Some(Track {
                id,
                title,
                artist,
                album,
                duration,
                url: path_str,
            })
        }
        Err(e) => {
            debug!("Could not read tags for {}: {e}", path.display());
            // Fall back to filename heuristics with no duration
            Some(Track {
                id,
                title: fallback_title,
                artist: fallback_artist,
                album: fallback_album,
                duration: None,
                url: path_str,
            })
        }
    }
}
