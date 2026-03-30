use anyhow::Result;
use libmpv2::Mpv;
use log::{info, warn};

const MUTUI_SINK: &str = "mutui_sink";

#[derive(Debug, Clone, Copy)]
struct AudioRouting {
    sink_module_id: u32,
    loopback_module_id: u32,
}

pub struct MpvHandle {
    mpv: Mpv,
    audio_routing: Option<AudioRouting>,
}

impl MpvHandle {
    pub fn start() -> Result<Self> {
        let audio_routing = if audio_routing_requested() {
            setup_audio_routing()
        } else {
            info!("Pulse loopback routing disabled (set MUTUI_ENABLE_AUDIO_ROUTING=1 to enable)");
            None
        };

        let audio_device = audio_routing
            .as_ref()
            .map(|_| format!("pulse/{MUTUI_SINK}"));

        let mpv = Mpv::with_initializer(|init| {
            init.set_option("vo", "null")?;
            init.set_option("video", "no")?;
            init.set_option("ytdl", "yes")?;
            init.set_option("ytdl-format", "bestaudio/best")?;
            if let Some(ref dev) = audio_device {
                init.set_option("audio-device", dev.as_str())?;
            }
            Ok(())
        })
        .map_err(|e| anyhow::anyhow!("Failed to create libmpv instance: {e}"))?;

        info!("libmpv initialized (embedded)");

        Ok(Self { mpv, audio_routing })
    }

    pub fn loadfile(&self, url: &str) -> Result<()> {
        self.mpv
            .command("loadfile", &[url, "replace"])
            .map_err(|e| anyhow::anyhow!("loadfile failed: {e}"))
    }

    pub fn play(&self) -> Result<()> {
        self.mpv
            .set_property("pause", false)
            .map_err(|e| anyhow::anyhow!("play failed: {e}"))
    }

    pub fn pause(&self) -> Result<()> {
        self.mpv
            .set_property("pause", true)
            .map_err(|e| anyhow::anyhow!("pause failed: {e}"))
    }

    pub fn toggle_pause(&self) -> Result<()> {
        let paused: bool = self.mpv.get_property("pause").unwrap_or(false);
        self.mpv
            .set_property("pause", !paused)
            .map_err(|e| anyhow::anyhow!("toggle_pause failed: {e}"))
    }

    pub fn stop(&self) -> Result<()> {
        self.mpv
            .command("stop", &[])
            .map_err(|e| anyhow::anyhow!("stop failed: {e}"))
    }

    pub fn seek(&self, seconds: f64) -> Result<()> {
        self.mpv
            .command("seek", &[&seconds.to_string(), "absolute"])
            .map_err(|e| anyhow::anyhow!("seek failed: {e}"))
    }

    pub fn set_volume(&self, volume: i64) -> Result<()> {
        self.mpv
            .set_property("volume", volume)
            .map_err(|e| anyhow::anyhow!("set_volume failed: {e}"))
    }

    pub fn get_time_pos(&self) -> f64 {
        self.mpv.get_property::<f64>("time-pos").unwrap_or(0.0)
    }

    pub fn get_duration(&self) -> f64 {
        self.mpv.get_property::<f64>("duration").unwrap_or(0.0)
    }

    pub fn get_volume(&self) -> i64 {
        self.mpv.get_property::<i64>("volume").unwrap_or(80)
    }

    pub fn is_paused(&self) -> bool {
        self.mpv.get_property::<bool>("pause").unwrap_or(true)
    }

    pub fn is_idle(&self) -> bool {
        self.mpv.get_property::<bool>("idle-active").unwrap_or(true)
    }

    pub fn shutdown(&self) {
        let _ = self.mpv.command("quit", &[]);
        teardown_audio_routing(self.audio_routing);
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
    #[cfg(not(target_os = "linux"))]
    {
        info!("Custom audio routing is only available on Linux (PulseAudio/PipeWire pactl)");
        return None;
    }

    #[cfg(target_os = "linux")]
    {
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
}

fn teardown_audio_routing(routing: Option<AudioRouting>) {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = routing;
        return;
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(r) = routing {
            let _ = run_pactl(&["unload-module", &r.loopback_module_id.to_string()]);
            let _ = run_pactl(&["unload-module", &r.sink_module_id.to_string()]);
        }

        // Also clear out any stale mutui modules left from a previous unclean exit.
        cleanup_stale_audio_routing();
    }
}

fn cleanup_stale_audio_routing() {
    #[cfg(not(target_os = "linux"))]
    {
        return;
    }

    #[cfg(target_os = "linux")]
    {
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
}

fn list_mutui_modules() -> Vec<(u32, bool)> {
    #[cfg(not(target_os = "linux"))]
    {
        return Vec::new();
    }

    #[cfg(target_os = "linux")]
    {
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
}

fn run_pactl(args: &[&str]) -> Option<String> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = args;
        return None;
    }

    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("pactl")
            .args(args)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
