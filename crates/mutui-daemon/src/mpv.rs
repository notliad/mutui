use anyhow::{Context, Result};
use log::{debug, info, warn};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
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
    #[cfg(unix)]
    writer: Mutex<tokio::io::WriteHalf<UnixStream>>,
    #[cfg(unix)]
    reader: Mutex<BufReader<tokio::io::ReadHalf<UnixStream>>>,
    #[cfg(windows)]
    writer: Mutex<tokio::io::WriteHalf<NamedPipeClient>>,
    #[cfg(windows)]
    reader: Mutex<BufReader<tokio::io::ReadHalf<NamedPipeClient>>>,
    audio_routing: Option<AudioRouting>,
}

impl MpvHandle {
    pub async fn start() -> Result<Self> {
        let socket_path = mutui_common::mpv_socket_path();

        #[cfg(unix)]
        {
            // Clean up stale socket.
            let _ = std::fs::remove_file(&socket_path);
        }

        let audio_routing = if audio_routing_requested() {
            setup_audio_routing()
        } else {
            info!(
                "Pulse loopback routing disabled (set MUTUI_ENABLE_AUDIO_ROUTING=1 to enable)"
            );
            None
        };

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

        #[cfg(unix)]
        let stream = {
            // Wait for IPC socket file to appear on Unix.
            for i in 0..100 {
                if socket_path.exists() {
                    break;
                }
                if i == 99 {
                    anyhow::bail!("mpv IPC socket did not appear");
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }

            UnixStream::connect(&socket_path)
                .await
                .context("Failed to connect to mpv IPC socket")?
        };

        #[cfg(windows)]
        let stream = {
            let pipe_name = socket_path.to_string_lossy().to_string();
            let mut connected = None;

            for _ in 0..100 {
                if let Ok(client) = ClientOptions::new().open(&pipe_name) {
                    connected = Some(client);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }

            connected.context("Failed to connect to mpv IPC named pipe")?
        };

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
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

fn audio_routing_requested() -> bool {
    let Ok(value) = std::env::var("MUTUI_ENABLE_AUDIO_ROUTING") else {
        return false;
    };

    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn setup_audio_routing() -> Option<AudioRouting> {
    cleanup_stale_audio_routing();

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

    // Also clear out any stale mutui modules left from a previous unclean exit.
    cleanup_stale_audio_routing();
}

fn cleanup_stale_audio_routing() {
    let modules = list_mutui_modules();
    if modules.is_empty() {
        return;
    }

    let mut removed = 0usize;

    // Unload loopbacks first, then sinks.
    for id in modules
        .iter()
        .filter(|(_, is_loopback)| *is_loopback)
        .map(|(id, _)| *id)
        .chain(
            modules
                .iter()
                .filter(|(_, is_loopback)| !*is_loopback)
                .map(|(id, _)| *id),
        )
    {
        if run_pactl(&["unload-module", &id.to_string()]).is_some() {
            removed += 1;
        }
    }

    if removed > 0 {
        info!("Cleaned up {removed} stale mutui audio module(s)");
    }
}

fn list_mutui_modules() -> Vec<(u32, bool)> {
    let output = match std::process::Command::new("pactl")
        .args(["list", "short", "modules"])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => {
            warn!("Could not query pactl modules for stale mutui cleanup");
            return Vec::new();
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let id = parts.next()?.parse::<u32>().ok()?;

            let is_mutui_loopback =
                line.contains("module-loopback") && line.contains("source=mutui_sink.monitor");
            let is_mutui_sink =
                line.contains("module-null-sink") && line.contains("sink_name=mutui_sink");

            if is_mutui_loopback {
                Some((id, true))
            } else if is_mutui_sink {
                Some((id, false))
            } else {
                None
            }
        })
        .collect()
}

fn run_pactl(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("pactl").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
