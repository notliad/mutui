use std::collections::HashMap;
use std::future::pending;
use std::sync::Arc;

use anyhow::{Context, Result};
use log::{error, info};
use mutui_common::{PlayerState, Request, Response};
use tokio::sync::Mutex;
use zbus::{Connection, interface};
use zvariant::{OwnedObjectPath, Value};

use crate::server::Daemon;

const MPRIS_BUS_NAME: &str = "org.mpris.MediaPlayer2.mutui";
const MPRIS_OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";

#[derive(Clone)]
struct PlayerIface {
    daemon: Arc<Mutex<Daemon>>,
}

impl PlayerIface {
    fn new(daemon: Arc<Mutex<Daemon>>) -> Self {
        Self { daemon }
    }

    async fn apply_request(&self, req: Request) {
        let mut d = self.daemon.lock().await;
        match d.handle_request(req) {
            Response::Error(err) => error!("MPRIS request failed: {err}"),
            _ => {}
        }
    }
}

#[derive(Clone)]
struct RootIface;

#[interface(name = "org.mpris.MediaPlayer2")]
impl RootIface {
    async fn raise(&self) {
        // Not supported by mutui daemon.
    }

    async fn quit(&self) {
        // Not exposed through MPRIS.
    }

    #[zbus(property)]
    async fn can_quit(&self) -> bool {
        false
    }

    #[zbus(property)]
    async fn can_raise(&self) -> bool {
        false
    }

    #[zbus(property)]
    async fn has_track_list(&self) -> bool {
        false
    }

    #[zbus(property)]
    async fn identity(&self) -> String {
        "mutui".to_string()
    }

    #[zbus(property)]
    async fn desktop_entry(&self) -> String {
        "mutui".to_string()
    }

    #[zbus(property)]
    async fn supported_uri_schemes(&self) -> Vec<String> {
        vec![]
    }

    #[zbus(property)]
    async fn supported_mime_types(&self) -> Vec<String> {
        vec![]
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl PlayerIface {
    async fn next(&self) {
        self.apply_request(Request::Next).await;
    }

    async fn previous(&self) {
        self.apply_request(Request::Previous).await;
    }

    async fn pause(&self) {
        self.apply_request(Request::Pause).await;
    }

    async fn play_pause(&self) {
        self.apply_request(Request::Toggle).await;
    }

    async fn stop(&self) {
        self.apply_request(Request::Stop).await;
    }

    async fn play(&self) {
        self.apply_request(Request::Play).await;
    }

    async fn seek(&self, _offset: i64) {
        // Offset seek not supported.
    }

    async fn set_position(&self, _track_id: OwnedObjectPath, _position: i64) {
        // Absolute seek not supported.
    }

    async fn open_uri(&self, _uri: &str) {
        // Queueing arbitrary URIs is not supported yet.
    }

    #[zbus(property)]
    async fn playback_status(&self) -> String {
        let d = self.daemon.lock().await;
        match d.get_status().state {
            PlayerState::Playing => "Playing".to_string(),
            PlayerState::Paused => "Paused".to_string(),
            PlayerState::Stopped => "Stopped".to_string(),
        }
    }

    #[zbus(property)]
    async fn metadata(&self) -> HashMap<String, Value<'static>> {
        let d = self.daemon.lock().await;
        let status = d.get_status();

        let mut meta = HashMap::new();
        let track_id = format!("/org/mpris/MediaPlayer2/track/{}", status.queue_index.max(1));
        if let Ok(track_id_path) = OwnedObjectPath::try_from(track_id) {
            meta.insert("mpris:trackid".to_string(), Value::from(track_id_path));
        }

        if let Some(track) = status.current_track {
            meta.insert("xesam:title".to_string(), Value::from(track.title));
            meta.insert("xesam:artist".to_string(), Value::from(vec![track.artist]));
            if let Some(album) = track.album {
                meta.insert("xesam:album".to_string(), Value::from(album));
            }
            if let Some(duration_secs) = track.duration {
                let us = (duration_secs * 1_000_000.0) as i64;
                meta.insert("mpris:length".to_string(), Value::from(us));
            }
            meta.insert("xesam:url".to_string(), Value::from(track.url));
        }

        meta
    }

    #[zbus(property)]
    async fn volume(&self) -> f64 {
        let d = self.daemon.lock().await;
        (d.get_status().volume as f64) / 100.0
    }

    #[zbus(property)]
    async fn set_volume(&self, volume: f64) {
        let vol = (volume.clamp(0.0, 1.5) * 100.0).round() as i64;
        self.apply_request(Request::SetVolume(vol)).await;
    }

    #[zbus(property)]
    async fn position(&self) -> i64 {
        let d = self.daemon.lock().await;
        (d.get_status().position * 1_000_000.0) as i64
    }

    #[zbus(property)]
    async fn can_go_next(&self) -> bool {
        let d = self.daemon.lock().await;
        !d.get_status().queue.is_empty()
    }

    #[zbus(property)]
    async fn can_go_previous(&self) -> bool {
        let d = self.daemon.lock().await;
        !d.get_status().queue.is_empty()
    }

    #[zbus(property)]
    async fn can_play(&self) -> bool {
        let d = self.daemon.lock().await;
        !d.get_status().queue.is_empty()
    }

    #[zbus(property)]
    async fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    async fn can_seek(&self) -> bool {
        false
    }

    #[zbus(property)]
    async fn can_control(&self) -> bool {
        true
    }
}

pub async fn run(daemon: Arc<Mutex<Daemon>>) -> Result<()> {
    let root_iface = RootIface;
    let player_iface = PlayerIface::new(daemon);

    let connection = Connection::session()
        .await
        .context("Failed to connect to DBus session bus")?;

    connection
        .request_name(MPRIS_BUS_NAME)
        .await
        .context("Failed to request MPRIS bus name")?;

    connection
        .object_server()
        .at(MPRIS_OBJECT_PATH, root_iface)
        .await
        .context("Failed to register MPRIS root interface")?;

    connection
        .object_server()
        .at(MPRIS_OBJECT_PATH, player_iface)
        .await
        .context("Failed to register MPRIS player interface")?;

    info!("MPRIS controls available on {MPRIS_BUS_NAME}");

    pending::<()>().await;
    Ok(())
}

pub fn spawn(daemon: Arc<Mutex<Daemon>>) {
    tokio::spawn(async move {
        if let Err(e) = run(daemon).await {
            error!("MPRIS service failed: {e:#}");
        }
    });
}
