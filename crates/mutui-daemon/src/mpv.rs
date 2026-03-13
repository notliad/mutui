use anyhow::{Context, Result};
use log::{debug, info};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

const MUTUI_SINK: &str = "mutui_sink";

#[derive(Debug, Clone, Copy)]
struct AudioRouting {
    sink_module_id: u32,
    loopback_module_id: u32,
}

pub struct MpvHandle {
    process: Child,
    socket_path: PathBuf,
    writer: Mutex<tokio::io::WriteHalf<UnixStream>>,
    reader: Mutex<BufReader<tokio::io::ReadHalf<UnixStream>>>,
    audio_routing: Option<AudioRouting>,
}

impl MpvHandle {
    pub async fn start() -> Result<Self> {
        let socket_path = mutui_common::mpv_socket_path();

        // Clean up stale socket
        let _ = std::fs::remove_file(&socket_path);

        let audio_routing = setup_audio_routing();

        info!("Starting mpv with IPC at {}", socket_path.display());

        let mut cmd = Command::new("mpv");
        cmd.arg("--idle=yes")
            .arg("--no-video")
            .arg("--no-terminal")
            .arg(format!("--input-ipc-server={}", socket_path.display()))
            .arg("--ytdl=yes")
            .arg("--ytdl-format=bestaudio/best")
            .kill_on_drop(false);

        if audio_routing.is_some() {
            cmd.arg(format!("--audio-device=pulse/{MUTUI_SINK}"));
        }

        let process = cmd
            .spawn()
            .context("Failed to start mpv. Is mpv installed?")?;

        // Wait for IPC socket
        for i in 0..100 {
            if socket_path.exists() {
                break;
            }
            if i == 99 {
                anyhow::bail!("mpv IPC socket did not appear");
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        let stream = UnixStream::connect(&socket_path)
            .await
            .context("Failed to connect to mpv IPC socket")?;

        let (reader, writer) = tokio::io::split(stream);
        let reader = BufReader::new(reader);

        info!("Connected to mpv IPC");

        Ok(Self {
            process,
            socket_path,
            writer: Mutex::new(writer),
            reader: Mutex::new(reader),
            audio_routing,
        })
    }

    /// Send a command to mpv and return the response.
    pub async fn command(&self, args: &[&str]) -> Result<Value> {
        let id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let cmd = json!({
            "command": args,
            "request_id": id,
        });

        let mut line = serde_json::to_string(&cmd)? + "\n";

        {
            let mut writer = self.writer.lock().await;
            writer
                .write_all(line.as_bytes())
                .await
                .context("Failed to write to mpv")?;
            writer.flush().await?;
        }

        // Read lines until we find our response
        let mut reader = self.reader.lock().await;
        line.clear();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                anyhow::bail!("mpv IPC connection closed");
            }
            if let Ok(val) = serde_json::from_str::<Value>(line.trim()) {
                if val.get("request_id").and_then(|v| v.as_u64()) == Some(id) {
                    let error = val
                        .get("error")
                        .and_then(|e| e.as_str())
                        .unwrap_or("unknown");
                    if error != "success" {
                        debug!("mpv error for {:?}: {}", args, error);
                    }
                    return Ok(val);
                }
                // else it's an event or response to a different command, skip
            }
        }
    }

    pub async fn loadfile(&self, url: &str) -> Result<()> {
        self.command(&["loadfile", url, "replace"]).await?;
        Ok(())
    }

    pub async fn play(&self) -> Result<()> {
        self.set_property("pause", json!(false)).await
    }

    pub async fn pause(&self) -> Result<()> {
        self.set_property("pause", json!(true)).await
    }

    pub async fn toggle_pause(&self) -> Result<()> {
        let paused = self.get_property("pause").await?;
        let is_paused = paused.as_bool().unwrap_or(false);
        self.set_property("pause", json!(!is_paused)).await
    }

    pub async fn stop(&self) -> Result<()> {
        self.command(&["stop"]).await?;
        Ok(())
    }

    pub async fn seek(&self, seconds: f64) -> Result<()> {
        self.command(&["seek", &seconds.to_string(), "absolute"])
            .await?;
        Ok(())
    }

    pub async fn set_volume(&self, volume: i64) -> Result<()> {
        self.set_property("volume", json!(volume)).await
    }

    pub async fn get_property(&self, name: &str) -> Result<Value> {
        let resp = self.command(&["get_property", name]).await?;
        Ok(resp.get("data").cloned().unwrap_or(Value::Null))
    }

    pub async fn set_property(&self, name: &str, value: Value) -> Result<()> {
        let id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
        let cmd = json!({
            "command": ["set_property", name, value],
            "request_id": id,
        });

        let line = serde_json::to_string(&cmd)? + "\n";

        {
            let mut writer = self.writer.lock().await;
            writer.write_all(line.as_bytes()).await?;
            writer.flush().await?;
        }

        let mut reader = self.reader.lock().await;
        let mut buf = String::new();
        loop {
            buf.clear();
            let n = reader.read_line(&mut buf).await?;
            if n == 0 {
                anyhow::bail!("mpv IPC connection closed");
            }
            if let Ok(val) = serde_json::from_str::<Value>(buf.trim()) {
                if val.get("request_id").and_then(|v| v.as_u64()) == Some(id) {
                    return Ok(());
                }
            }
        }
    }

    /// Get current playback time in seconds
    pub async fn get_time_pos(&self) -> f64 {
        self.get_property("time-pos")
            .await
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
    }

    /// Get current track duration in seconds
    pub async fn get_duration(&self) -> f64 {
        self.get_property("duration")
            .await
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
    }

    pub async fn get_volume(&self) -> i64 {
        self.get_property("volume")
            .await
            .ok()
            .and_then(|v| v.as_f64())
            .map(|v| v as i64)
            .unwrap_or(80)
    }

    pub async fn is_paused(&self) -> bool {
        self.get_property("pause")
            .await
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    pub async fn is_idle(&self) -> bool {
        self.get_property("idle-active")
            .await
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    pub async fn shutdown(&mut self) {
        let _ = self.command(&["quit"]).await;
        let _ = self.process.kill().await;
        teardown_audio_routing(self.audio_routing);
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

fn setup_audio_routing() -> Option<AudioRouting> {
    let sink_module_id = run_pactl(&[
        "load-module",
        "module-null-sink",
        &format!("sink_name={MUTUI_SINK}"),
        "sink_properties=device.description=mutui_sink",
    ])
    .and_then(|v| v.parse::<u32>().ok())?;

    let loopback_module_id = run_pactl(&[
        "load-module",
        "module-loopback",
        &format!("source={MUTUI_SINK}.monitor"),
        "sink=@DEFAULT_SINK@",
        "latency_msec=20",
    ])
    .and_then(|v| v.parse::<u32>().ok());

    if let Some(loopback_module_id) = loopback_module_id {
        info!("Audio routing enabled on sink '{MUTUI_SINK}'");
        Some(AudioRouting {
            sink_module_id,
            loopback_module_id,
        })
    } else {
        let _ = run_pactl(&["unload-module", &sink_module_id.to_string()]);
        info!("Pulse loopback unavailable; using default audio routing");
        None
    }
}

fn teardown_audio_routing(routing: Option<AudioRouting>) {
    if let Some(r) = routing {
        let _ = run_pactl(&["unload-module", &r.loopback_module_id.to_string()]);
        let _ = run_pactl(&["unload-module", &r.sink_module_id.to_string()]);
    }
}

fn run_pactl(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("pactl").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
