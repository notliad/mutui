use anyhow::{Context, Result};
use log::{debug, info, warn};
use mutui_common::Track;
use serde::Deserialize;
use std::sync::Arc;
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
            album: None,
            duration: self.duration,
            url,
        })
    }
}

#[derive(Clone)]
struct ScoreConfig {
    positive_noise_words: Vec<&'static str>,
    official_channel_terms: Vec<&'static str>,
    medium_positive_terms: Vec<(&'static str, i32)>,
    soft_negative_terms: Vec<(&'static str, i32)>,
    score_title_match: i32,
    score_official_channel: i32,
    score_duration_ideal: i32,
    score_duration_good: i32,
    penalty_duration_short: i32,
    penalty_duration_long: i32,
    min_duration_secs: u64,
    max_duration_secs: u64,
    matcher: Arc<dyn Fn(&str, &str) -> bool + Send + Sync>,
}

impl Default for ScoreConfig {
    fn default() -> Self {
        Self {
            positive_noise_words: vec!["official", "video", "audio", "hd", "lyrics"],
            official_channel_terms: vec!["topic", "official", "vevo"],
            medium_positive_terms: vec![
                ("official audio", 25),
                ("official video", 20),
            ],
            soft_negative_terms: vec![
                ("live", -20),
                ("cover", -15),
                ("remix", -10),
                ("playlist", -10),
                ("mix", -10),
                ("radio", -10),
            ],
            score_title_match: 60,
            score_official_channel: 50,
            score_duration_ideal: 40,
            score_duration_good: 10,
            penalty_duration_short: -10,
            penalty_duration_long: -20,
            min_duration_secs: 20,
            max_duration_secs: 3600,
            matcher: Arc::new(default_query_matcher),
        }
    }
}

#[derive(Debug)]
struct ScoredTrack {
    idx: usize,
    score: i32,
    track: Track,
}

fn sanitize_for_matching(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;

    for ch in input.chars() {
        match ch {
            '(' => {
                paren_depth += 1;
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
            }
            '[' => {
                bracket_depth += 1;
            }
            ']' => {
                bracket_depth = bracket_depth.saturating_sub(1);
            }
            _ => {
                if paren_depth == 0 && bracket_depth == 0 {
                    let ch = ch.to_ascii_lowercase();
                    if ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace() {
                        out.push(ch);
                    } else {
                        out.push(' ');
                    }
                }
            }
        }
    }

    out
}

fn normalize_text(input: &str, noise_words: &[&str]) -> String {
    let sanitized = sanitize_for_matching(input);
    let mut words = Vec::new();
    for word in sanitized.split_whitespace() {
        if !noise_words.iter().any(|noise| *noise == word) {
            words.push(word);
        }
    }
    words.join(" ")
}

fn default_query_matcher(normalized_query: &str, normalized_title: &str) -> bool {
    if normalized_query.is_empty() || normalized_title.is_empty() {
        return false;
    }

    if normalized_title.contains(normalized_query) {
        return true;
    }

    let query_words: Vec<&str> = normalized_query.split_whitespace().collect();
    let title_words: Vec<&str> = normalized_title.split_whitespace().collect();
    if query_words.is_empty() || title_words.is_empty() {
        return false;
    }

    let overlap = query_words
        .iter()
        .filter(|q| title_words.iter().any(|t| t.contains(**q) || q.contains(*t)))
        .count();

    let overlap_ratio = overlap as f32 / query_words.len() as f32;
    overlap_ratio >= 0.7
}

fn is_official_channel(channel: &str, normalized_query: &str, cfg: &ScoreConfig) -> bool {
    let normalized_channel = sanitize_for_matching(channel);
    if normalized_channel.contains(" topic") || normalized_channel.ends_with(" topic") {
        return true;
    }
    if cfg
        .official_channel_terms
        .iter()
        .any(|term| normalized_channel.contains(term))
    {
        return true;
    }

    if normalized_query.is_empty() {
        return false;
    }

    // Heuristic: channels starting with query terms are likely artist channels.
    normalized_channel.starts_with(normalized_query)
        || normalized_query
            .split_whitespace()
            .all(|w| normalized_channel.contains(w))
}

fn score_track(query: &str, track: &Track, cfg: &ScoreConfig) -> i32 {
    let normalized_query = normalize_text(query, &cfg.positive_noise_words);
    let normalized_title = normalize_text(&track.title, &cfg.positive_noise_words);
    let normalized_artist = sanitize_for_matching(&track.artist);
    let normalized_title_full = sanitize_for_matching(&track.title);

    let mut score = 0;

    if (cfg.matcher)(&normalized_query, &normalized_title) {
        score += cfg.score_title_match;
    }

    if is_official_channel(&normalized_artist, &normalized_query, cfg) {
        score += cfg.score_official_channel;
    }

    if cfg
        .medium_positive_terms
        .iter()
        .any(|(phrase, _)| normalized_title_full.contains(phrase))
    {
        for (phrase, phrase_score) in &cfg.medium_positive_terms {
            if normalized_title_full.contains(phrase) {
                score += *phrase_score;
            }
        }
    }

    for (term, penalty) in &cfg.soft_negative_terms {
        if normalized_title_full.contains(term) {
            score += *penalty;
        }
    }

    if let Some(duration) = track.duration {
        let secs = duration.max(0.0) as u64;
        if (120..=330).contains(&secs) {
            score += cfg.score_duration_ideal;
        }
        if (60..=480).contains(&secs) {
            score += cfg.score_duration_good;
        }
        if secs > 600 {
            score += cfg.penalty_duration_long;
        }
        if secs < 60 {
            score += cfg.penalty_duration_short;
        }
    }

    score
}

fn should_keep_track(track: &Track, cfg: &ScoreConfig) -> bool {
    if let Some(duration) = track.duration {
        let secs = duration.max(0.0) as u64;
        return secs >= cfg.min_duration_secs && secs <= cfg.max_duration_secs;
    }
    true
}

fn rank_tracks(query: &str, tracks: Vec<Track>, cfg: &ScoreConfig) -> Vec<Track> {
    let mut scored = Vec::new();

    for (idx, track) in tracks.into_iter().enumerate() {
        if !should_keep_track(&track, cfg) {
            continue;
        }
        let score = score_track(query, &track, cfg);
        debug!(
            "Scored candidate idx={} score={} title='{}' artist='{}'",
            idx, score, track.title, track.artist
        );
        scored.push(ScoredTrack { idx, score, track });
    }

    scored.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.idx.cmp(&b.idx))
    });

    scored.into_iter().map(|s| s.track).collect()
}

fn improve_query(query: &str) -> String {
    let trimmed = query.trim();
    let lowered = trimmed.to_ascii_lowercase();
    if lowered.is_empty()
        || lowered.contains("playlist")
        || lowered.contains("mix")
        || lowered.contains("live")
        || lowered.contains("remix")
        || lowered.contains("cover")
        || lowered.contains("-playlist")
    {
        return trimmed.to_string();
    }

    // Keep broad candidate set while nudging yt-dlp away from giant playlists.
    format!("{trimmed} -playlist")
}

pub async fn search(query: &str, max_results: usize) -> Result<Vec<Track>> {
    info!("Searching for: {query}");

    let improved_query = improve_query(query);
    let search_query = format!("ytsearch{max_results}:{improved_query}");

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

    let cfg = ScoreConfig::default();
    let ranked = rank_tracks(query, tracks, &cfg);

    info!("Search returned {} ranked results", ranked.len());
    Ok(ranked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_track(id: &str, title: &str, artist: &str, duration: Option<f64>) -> Track {
        Track {
            id: id.to_string(),
            title: title.to_string(),
            artist: artist.to_string(),
            album: None,
            duration,
            url: format!("https://www.youtube.com/watch?v={id}"),
        }
    }

    #[test]
    fn normalize_strips_noise_and_brackets() {
        let cfg = ScoreConfig::default();
        let normalized = normalize_text(
            "Artist - Song (Official Video) [HD]",
            &cfg.positive_noise_words,
        );
        assert_eq!(normalized, "artist song");
    }

    #[test]
    fn hard_filters_remove_only_extreme_duration_outliers() {
        let cfg = ScoreConfig::default();
        let query = "artist song";
        let tracks = vec![
            mk_track("ok1", "Artist - Song", "Artist - Topic", Some(210.0)),
            mk_track("short", "Artist - Snippet", "Artist", Some(10.0)),
            mk_track("long", "Artist - Mega Stream", "Artist", Some(3700.0)),
            mk_track("ok2", "Artist - Song Live", "Artist", Some(900.0)),
        ];

        let ranked = rank_tracks(query, tracks, &cfg);
        let ids: Vec<&str> = ranked.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["ok1", "ok2"]);
    }

    #[test]
    fn ranks_better_match_first_but_keeps_full_candidate_list() {
        let cfg = ScoreConfig::default();
        let query = "artist song";
        let tracks = vec![
            mk_track(
                "best",
                "Artist - Song (Official Audio)",
                "Artist - Topic",
                Some(220.0),
            ),
            mk_track(
                "good",
                "Artist - Song (Official Video)",
                "Artist Official",
                Some(230.0),
            ),
            mk_track("live", "Artist - Song Live", "Artist", Some(210.0)),
            mk_track("remix", "Artist - Song Remix", "DJ Somebody", Some(245.0)),
        ];

        let ranked = rank_tracks(query, tracks, &cfg);
        let ids: Vec<&str> = ranked.iter().map(|t| t.id.as_str()).collect();

        assert_eq!(ranked.len(), 4);
        assert_eq!(ids.first().copied(), Some("best"));
        assert!(ids.contains(&"live"));
        assert!(ids.contains(&"remix"));
    }

    #[test]
    fn improve_query_adds_playlist_exclusion_lightly() {
        assert_eq!(improve_query("lofi hip hop"), "lofi hip hop -playlist");
        assert_eq!(improve_query("lofi hip hop -playlist"), "lofi hip hop -playlist");
        assert_eq!(improve_query("chill playlist"), "chill playlist");
    }
}
