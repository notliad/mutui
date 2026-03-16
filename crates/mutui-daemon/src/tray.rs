use anyhow::{Context, Result};
use ksni::menu::StandardItem;
use ksni::TrayMethods;
use mutui_common::{encode_message, PlayerState, Request, Response};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
enum TrayAction {
    OpenTui,
    Toggle,
    Next,
    Previous,
    Stop,
    ShutdownDaemon,
    QuitTray,
}

struct MutuiTray {
    tx: mpsc::Sender<TrayAction>,
    title: String,
    tooltip: String,
}

impl MutuiTray {
    fn new(tx: mpsc::Sender<TrayAction>) -> Self {
        Self {
            tx,
            title: "mutui".to_string(),
            tooltip: "mutui offline".to_string(),
        }
    }

    fn dispatch(&self, action: TrayAction) {
        let _ = self.tx.try_send(action);
    }
}

impl ksni::Tray for MutuiTray {
    fn id(&self) -> String {
        "mutui-tray".into()
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    fn icon_name(&self) -> String {
        "audio-x-generic".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        // 22x22 ARGB32 (network byte order) cyan circle — guarantees the
        // icon is visible even when the name cannot be resolved by the
        // active icon theme (common on Hyprland + waybar).
        let size: i32 = 22;
        let mut data = Vec::with_capacity((size * size * 4) as usize);
        let c = size as f64 / 2.0;
        let r = c - 1.5;
        for y in 0..size {
            for x in 0..size {
                let dx = x as f64 - c + 0.5;
                let dy = y as f64 - c + 0.5;
                if dx * dx + dy * dy <= r * r {
                    data.extend_from_slice(&[255, 0, 190, 220]); // ARGB cyan
                } else {
                    data.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        }
        vec![ksni::Icon { width: size, height: size, data }]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "mutui".into(),
            description: self.tooltip.clone(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.dispatch(TrayAction::OpenTui);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Open mutui".into(),
                activate: Box::new(|this: &mut Self| this.dispatch(TrayAction::OpenTui)),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Play/Pause".into(),
                activate: Box::new(|this: &mut Self| this.dispatch(TrayAction::Toggle)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next".into(),
                activate: Box::new(|this: &mut Self| this.dispatch(TrayAction::Next)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Previous".into(),
                activate: Box::new(|this: &mut Self| this.dispatch(TrayAction::Previous)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Stop".into(),
                activate: Box::new(|this: &mut Self| this.dispatch(TrayAction::Stop)),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Shutdown daemon".into(),
                activate: Box::new(|this: &mut Self| this.dispatch(TrayAction::ShutdownDaemon)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Close tray".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| this.dispatch(TrayAction::QuitTray)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = match std::fs::metadata(path) {
            Ok(meta) => meta.permissions().mode(),
            Err(_) => return false,
        };
        return mode & 0o111 != 0;
    }
    #[allow(unreachable_code)]
    true
}

fn find_binary(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(parent) = exe.parent() {
        candidates.push(parent.join(name));
    }
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(home).join(".local/bin").join(name));
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            candidates.push(dir.join(name));
        }
    }

    candidates.into_iter().find(|p| is_executable(p))
}

fn send_request(req: Request) -> Result<Response> {
    let socket = mutui_common::socket_path();
    let mut stream = UnixStream::connect(&socket)
        .with_context(|| format!("Could not connect to daemon at {}", socket.display()))?;

    let frame = encode_message(&req)?;
    stream.write_all(&frame)?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);

    let mut len_line = String::new();
    reader.read_line(&mut len_line)?;
    let _expected_len: usize = len_line.trim().parse().context("Invalid daemon frame")?;

    let mut json_line = String::new();
    reader.read_line(&mut json_line)?;

    let resp: Response = serde_json::from_str(json_line.trim())?;
    Ok(resp)
}

fn open_tui() -> Result<()> {
    let mutui = find_binary("mutui").context("mutui binary not found")?;

    let runners: [(&str, &[&str]); 8] = [
        ("x-terminal-emulator", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("kitty", &[]),
        ("alacritty", &["-e"]),
        ("wezterm", &["start", "--"]),
        ("xfce4-terminal", &["-x"]),
        ("foot", &["-e"]),
    ];

    for (cmd, base_args) in runners {
        let Some(bin) = find_binary(cmd) else {
            continue;
        };

        let mut command = std::process::Command::new(bin);
        for arg in base_args {
            command.arg(arg);
        }
        command.arg(&mutui);

        if command.spawn().is_ok() {
            return Ok(());
        }
    }

    anyhow::bail!("No supported terminal emulator found to launch mutui")
}

fn describe_status() -> String {
    match send_request(Request::GetStatus) {
        Ok(Response::Status(status)) => match status.state {
            PlayerState::Playing => {
                if let Some(track) = status.current_track {
                    format!("Playing: {} - {}", track.artist, track.title)
                } else {
                    "Playing".to_string()
                }
            }
            PlayerState::Paused => {
                if let Some(track) = status.current_track {
                    format!("Paused: {} - {}", track.artist, track.title)
                } else {
                    "Paused".to_string()
                }
            }
            PlayerState::Stopped => "mutui running (stopped)".to_string(),
        },
        _ => "mutui offline".to_string(),
    }
}

struct TrayLock {
    path: PathBuf,
}

impl Drop for TrayLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn acquire_lock() -> Option<TrayLock> {
    let lock_path = mutui_common::socket_path().with_file_name("mutui-tray.lock");

    // Try creating the lock file exclusively.
    match std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&lock_path)
    {
        Ok(mut file) => {
            let _ = writeln!(file, "{}", std::process::id());
            return Some(TrayLock { path: lock_path });
        }
        Err(_) => {}
    }

    // Lock file exists — check if the owning process is still alive.
    if let Ok(contents) = std::fs::read_to_string(&lock_path) {
        if let Ok(pid) = contents.trim().parse::<u32>() {
            let alive = std::path::Path::new(&format!("/proc/{pid}")).exists();
            if alive {
                return None; // another tray is genuinely running
            }
        }
    }

    // Stale lock — remove and retry.
    let _ = std::fs::remove_file(&lock_path);
    match std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&lock_path)
    {
        Ok(mut file) => {
            let _ = writeln!(file, "{}", std::process::id());
            Some(TrayLock { path: lock_path })
        }
        Err(_) => None,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let _lock = match acquire_lock() {
        Some(lock) => lock,
        None => return Ok(()),
    };

    let (tx, mut rx) = mpsc::channel::<TrayAction>(32);
    let tray = MutuiTray::new(tx.clone());
    let handle = Arc::new(tray.spawn().await?);

    let updater = Arc::clone(&handle);
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            tick.tick().await;
            let status_text = match tokio::task::spawn_blocking(describe_status).await {
                Ok(text) => text,
                Err(_) => "mutui offline".to_string(),
            };
            let _ = updater
                .update(move |tray: &mut MutuiTray| {
                    tray.tooltip = status_text.clone();
                    tray.title = if status_text.contains("offline") {
                        "mutui".to_string()
                    } else {
                        "mutui aberto".to_string()
                    };
                })
                .await;
        }
    });

    while let Some(action) = rx.recv().await {
        match action {
            TrayAction::OpenTui => {
                let _ = tokio::task::spawn_blocking(open_tui).await;
            }
            TrayAction::Toggle => {
                let _ = tokio::task::spawn_blocking(|| send_request(Request::Toggle)).await;
            }
            TrayAction::Next => {
                let _ = tokio::task::spawn_blocking(|| send_request(Request::Next)).await;
            }
            TrayAction::Previous => {
                let _ = tokio::task::spawn_blocking(|| send_request(Request::Previous)).await;
            }
            TrayAction::Stop => {
                let _ = tokio::task::spawn_blocking(|| send_request(Request::Stop)).await;
            }
            TrayAction::ShutdownDaemon => {
                let _ = tokio::task::spawn_blocking(|| send_request(Request::Shutdown)).await;
                let _ = handle.shutdown().await;
                break;
            }
            TrayAction::QuitTray => {
                let _ = handle.shutdown().await;
                break;
            }
        }
    }

    Ok(())
}
