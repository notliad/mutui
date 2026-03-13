use anyhow::{Context, Result};
use mutui_common::{encode_message, Request, Response};
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
    // Find the daemon binary next to ourselves
    let exe = std::env::current_exe()?;
    let daemon_exe = exe.parent().unwrap().join("mutuid");

    if !daemon_exe.exists() {
        anyhow::bail!(
            "Daemon binary not found at {}. Build with `cargo build`.",
            daemon_exe.display()
        );
    }

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

/// Ensure the daemon is running (start it if not) and return a connected client.
pub async fn ensure_daemon() -> Result<DaemonClient> {
    // Try connecting first
    if let Ok(client) = DaemonClient::connect().await {
        return Ok(client);
    }

    // Not running — start daemon
    start_daemon()?;

    // Wait for it to be ready
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if let Ok(client) = DaemonClient::connect().await {
            return Ok(client);
        }
    }

    anyhow::bail!("Daemon started but could not connect")
}
