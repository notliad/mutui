use anyhow::{Context, Result, anyhow};
use hmac::{Hmac, Mac};
use log::{info, warn};
use mutui_common::{PodcastChannel, PodcastEpisode, podcasts_config_path};
use sha1::Sha1;
use std::time::{SystemTime, UNIX_EPOCH};

// --- Followed podcast storage ---

pub fn load_followed() -> Vec<PodcastChannel> {
    let path = podcasts_config_path();
    let Ok(data) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn save_followed(channels: &[PodcastChannel]) -> Result<()> {
    let path = podcasts_config_path();
    let data = serde_json::to_string_pretty(channels)?;
    std::fs::write(&path, data)?;
    Ok(())
}

pub fn follow(channel: PodcastChannel) -> Result<Vec<PodcastChannel>> {
    let mut followed = load_followed();
    if !followed.iter().any(|c| c.feed_url == channel.feed_url) {
        info!("Following podcast: {}", channel.title);
        followed.push(channel);
        save_followed(&followed)?;
    }
    Ok(followed)
}

pub fn unfollow(feed_url: &str) -> Result<Vec<PodcastChannel>> {
    let mut followed = load_followed();
    let before = followed.len();
    followed.retain(|c| c.feed_url != feed_url);
    if followed.len() < before {
        save_followed(&followed)?;
        info!("Unfollowed podcast with feed: {feed_url}");
    }
    Ok(followed)
}

// --- PodcastIndex API search ---

/// Search for podcasts via the PodcastIndex API.
/// Falls back to the iTunes Search API when PodcastIndex credentials are not set.
pub async fn search(query: &str) -> Result<Vec<PodcastChannel>> {
    match search_podcastindex(query).await {
        Ok(channels) if !channels.is_empty() => return Ok(channels),
        Ok(_) => {} // empty results — fall through to iTunes
        Err(e) => {
            // Only log; don't abort: missing keys or any API error triggers fallback
            log::info!("PodcastIndex search unavailable ({e}), falling back to iTunes");
        }
    }
    search_itunes(query).await
}

async fn search_podcastindex(query: &str) -> Result<Vec<PodcastChannel>> {
    let api_key = std::env::var("PODCASTINDEX_API_KEY")
        .map_err(|_| anyhow!("PODCASTINDEX_API_KEY not set"))?;
    let api_secret = std::env::var("PODCASTINDEX_API_SECRET")
        .map_err(|_| anyhow!("PODCASTINDEX_API_SECRET not set"))?;

    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System time error")?
        .as_secs();

    let hash_input = format!("{api_key}{api_secret}{epoch}");
    let mut mac = Hmac::<Sha1>::new_from_slice(api_secret.as_bytes())
        .context("HMAC init failed")?;
    mac.update(hash_input.as_bytes());
    let auth_hash = hex::encode(mac.finalize().into_bytes());

    let client = reqwest::Client::new();

    let resp = client
        .get("https://api.podcastindex.org/api/1.0/search/byterm")
        .query(&[("q", query), ("max", "20")])
        .header("User-Agent", "mutui/0.3")
        .header("X-Auth-Key", &api_key)
        .header("X-Auth-Date", epoch.to_string())
        .header("Authorization", &auth_hash)
        .send()
        .await
        .context("PodcastIndex API request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("PodcastIndex API error {status}: {body}"));
    }

    let json: serde_json::Value = resp.json().await.context("Failed to parse API response")?;
    let feeds = json["feeds"].as_array().cloned().unwrap_or_default();

    let channels = feeds
        .iter()
        .filter_map(|f| parse_channel_from_json(f))
        .collect();

    Ok(channels)
}

// --- iTunes Search API fallback ---

async fn search_itunes(query: &str) -> Result<Vec<PodcastChannel>> {
    let client = reqwest::Client::new();

    let resp = client
        .get("https://itunes.apple.com/search")
        .query(&[("term", query), ("media", "podcast"), ("limit", "20")])
        .header("User-Agent", "mutui/0.3")
        .send()
        .await
        .context("iTunes API request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        return Err(anyhow!("iTunes API error {status}"));
    }

    let json: serde_json::Value = resp.json().await.context("Failed to parse iTunes response")?;
    let results = json["results"].as_array().cloned().unwrap_or_default();

    let channels = results
        .iter()
        .filter_map(|r| parse_channel_from_itunes(r))
        .collect();

    Ok(channels)
}

fn parse_channel_from_itunes(r: &serde_json::Value) -> Option<PodcastChannel> {
    let id = r["collectionId"].as_u64()?.to_string();
    let title = r["collectionName"].as_str()?.to_string();
    let feed_url = r["feedUrl"].as_str()?.to_string();
    let author = r["artistName"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string();
    let description = String::new();
    let image_url = r["artworkUrl600"]
        .as_str()
        .or_else(|| r["artworkUrl100"].as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    Some(PodcastChannel {
        id,
        title,
        author,
        description,
        image_url,
        feed_url,
    })
}

fn parse_channel_from_json(f: &serde_json::Value) -> Option<PodcastChannel> {
    let id = f["id"].as_u64()?.to_string();
    let title = f["title"].as_str()?.to_string();
    let feed_url = f["url"].as_str()?.to_string();
    let author = f["author"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string();
    let description = f["description"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let image_url = f["image"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(String::from);

    Some(PodcastChannel {
        id,
        title,
        author,
        description,
        image_url,
        feed_url,
    })
}

// --- RSS fetch and parse ---

pub async fn fetch_episodes(feed_url: &str) -> Result<Vec<PodcastEpisode>> {
    let client = reqwest::Client::new();
    let body = client
        .get(feed_url)
        .header("User-Agent", "mutui/0.3")
        .send()
        .await
        .context("Failed to fetch RSS feed")?
        .text()
        .await
        .context("Failed to read RSS body")?;

    parse_rss(&body)
}

fn parse_rss(xml: &str) -> Result<Vec<PodcastEpisode>> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut episodes: Vec<PodcastEpisode> = Vec::new();

    // State for the current <item>
    let mut in_item = false;
    let mut guid = String::new();
    let mut title = String::new();
    let mut description = String::new();
    let mut audio_url = String::new();
    let mut duration_str = String::new();
    let mut pub_date = String::new();
    let mut current_tag = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if tag == "item" {
                    in_item = true;
                    guid.clear();
                    title.clear();
                    description.clear();
                    audio_url.clear();
                    duration_str.clear();
                    pub_date.clear();
                }
                current_tag = tag;

                // <enclosure> carries the audio URL as an attribute
                if in_item && current_tag == "enclosure" {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref())
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        let val = attr.unescape_value().unwrap_or_default().to_string();
                        if key == "url" && audio_url.is_empty() {
                            audio_url = val;
                        }
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if in_item && tag == "enclosure" {
                    for attr in e.attributes().flatten() {
                        let key = std::str::from_utf8(attr.key.as_ref())
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        let val = attr.unescape_value().unwrap_or_default().to_string();
                        if key == "url" && audio_url.is_empty() {
                            audio_url = val;
                        }
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if !in_item {
                    continue;
                }
                let text = e.unescape().unwrap_or_default().to_string();
                match current_tag.as_str() {
                    "title" => title = text,
                    "description" | "itunes:summary" => {
                        if description.is_empty() {
                            description = text;
                        }
                    }
                    "guid" => guid = text,
                    "pubdate" => pub_date = text,
                    "itunes:duration" => duration_str = text,
                    _ => {}
                }
            }
            Ok(Event::CData(e)) => {
                if !in_item {
                    continue;
                }
                let text = String::from_utf8_lossy(e.as_ref()).to_string();
                match current_tag.as_str() {
                    "title" => title = text,
                    "description" | "itunes:summary" => {
                        if description.is_empty() {
                            description = text;
                        }
                    }
                    "guid" => guid = text,
                    "pubdate" => pub_date = text,
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let tag = std::str::from_utf8(e.name().as_ref())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if tag == "item" && in_item {
                    in_item = false;
                    if audio_url.is_empty() {
                        warn!("Episode '{}' has no audio URL, skipping", title);
                        continue;
                    }
                    let duration = parse_duration(&duration_str);
                    episodes.push(PodcastEpisode {
                        guid: if guid.is_empty() { audio_url.clone() } else { guid.clone() },
                        title: title.clone(),
                        description: description.clone(),
                        url: audio_url.clone(),
                        duration,
                        pub_date: if pub_date.is_empty() { None } else { Some(pub_date.clone()) },
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                warn!("RSS parse warning: {e}");
                break;
            }
            _ => {}
        }
    }

    Ok(episodes)
}

/// Parse itunes:duration formats: "HH:MM:SS", "MM:SS", or plain seconds.
fn parse_duration(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => s.parse::<f64>().ok(),
        2 => {
            let m = parts[0].parse::<f64>().ok()?;
            let sec = parts[1].parse::<f64>().ok()?;
            Some(m * 60.0 + sec)
        }
        3 => {
            let h = parts[0].parse::<f64>().ok()?;
            let m = parts[1].parse::<f64>().ok()?;
            let sec = parts[2].parse::<f64>().ok()?;
            Some(h * 3600.0 + m * 60.0 + sec)
        }
        _ => None,
    }
}
