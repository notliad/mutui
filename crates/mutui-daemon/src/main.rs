mod mpv;
mod library;
mod playlist;
mod queue;
mod search;
mod server;

use anyhow::Result;
use log::info;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    // Check if already running
    let socket = mutui_common::socket_path();
    if socket.exists() {
        if tokio::net::UnixStream::connect(&socket).await.is_ok() {
            eprintln!("mutui daemon is already running.");
            std::process::exit(1);
        }
        // Stale socket, remove it
        let _ = std::fs::remove_file(&socket);
    }

    info!("Starting mutui daemon...");

    let daemon = server::Daemon::new().await?;
    let daemon = Arc::new(Mutex::new(daemon));

    // Install signal handler for clean shutdown
    let daemon_sig = Arc::clone(&daemon);
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        info!("Received SIGINT, shutting down...");
        let mut d = daemon_sig.lock().await;
        d.mpv.shutdown().await;
        let _ = std::fs::remove_file(mutui_common::socket_path());
        std::process::exit(0);
    });

    server::run(daemon).await
}
