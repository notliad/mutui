use anyhow::{Context, Result};
use mutui_common::{encode_message, Request, Response};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

pub struct DaemonClient {
    reader: BufReader<tokio::io::ReadHalf<UnixStream>>,
    writer: tokio::io::WriteHalf<UnixStream>,
}

pub async fn send_once(req: Request) -> Result<Response> {
    let mut client = DaemonClient::connect().await?;
    client.send(&req).await
}

impl DaemonClient {
    pub async fn connect() -> Result<Self> {
        let socket = mutui_common::socket_path();
        let stream = UnixStream::connect(&socket)
            .await
            .context("Could not connect to mutui daemon. Is it running?")?;
        let (reader, writer) = tokio::io::split(stream);
        Ok(Self {
            reader: BufReader::new(reader),
            writer,
        })
    }

    pub async fn send(&mut self, req: &Request) -> Result<Response> {
        let frame = encode_message(req)?;
        self.writer
            .write_all(&frame)
            .await
            .context("Failed to send to daemon")?;
        self.writer.flush().await?;

        // Read length line
        let mut len_line = String::new();
        self.reader.read_line(&mut len_line).await?;
        let _expected_len: usize = len_line
            .trim()
            .parse()
            .context("Invalid response frame")?;

        // Read JSON line
        let mut json_line = String::new();
        self.reader.read_line(&mut json_line).await?;

        let resp: Response = serde_json::from_str(json_line.trim())?;
        Ok(resp)
    }
}

/// Start the daemon as a detached background process.
pub fn start_daemon() -> Result<()> {
    let daemon_exe = find_binary("mutuid")
        .context("Daemon binary not found. Install mutuid or build with `cargo build --release`.")?;

    use std::process::{Command, Stdio};

    // Start in a new session so the daemon survives TUI/session teardown
    // (practical equivalent of shell disown for this process tree).
    let spawned = Command::new("setsid")
        .arg(&daemon_exe)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    if spawned.is_err() {
        // Fallback when `setsid` is unavailable.
        Command::new(&daemon_exe)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start daemon")?;
    }

    Ok(())
}

/// Start the tray process (best-effort).
pub fn start_tray() {
    let Some(tray_exe) = find_binary("mutui-tray") else {
        return;
    };

    let _ = std::process::Command::new("setsid")
        .arg(&tray_exe)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .or_else(|_| {
            std::process::Command::new(&tray_exe)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
        });
}

/// Stop the tray process if it is running (best-effort).
pub fn stop_tray() {
    let lock_path = mutui_common::socket_path().with_file_name("mutui-tray.lock");
    let Ok(pid_text) = std::fs::read_to_string(&lock_path) else {
        return;
    };

    let Ok(pid) = pid_text.trim().parse::<i32>() else {
        let _ = std::fs::remove_file(lock_path);
        return;
    };

    let _ = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();
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

/// Ensure the daemon is running (start it if not) and return a connected client.
pub async fn ensure_daemon() -> Result<DaemonClient> {
    // Try connecting first
    if let Ok(client) = DaemonClient::connect().await {
        // Keep tray in sync with daemon lifecycle.
        start_tray();
        return Ok(client);
    }

    // Not running — start daemon
    start_daemon()?;
    start_tray();

    // Wait for it to be ready
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if let Ok(client) = DaemonClient::connect().await {
            return Ok(client);
        }
    }

    anyhow::bail!("Daemon started but could not connect")
}
