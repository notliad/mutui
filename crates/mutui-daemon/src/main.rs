mod mpv;
mod mpris;
mod library;
mod playlist;
mod podcasts;
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

    let daemon = server::Daemon::new()?;
    let daemon = Arc::new(Mutex::new(daemon));

    // Expose MPRIS controls for Linux media keys and desktop integrations.
    mpris::spawn(Arc::clone(&daemon));

    // Install signal handler for clean shutdown
    let daemon_sig = Arc::clone(&daemon);
    tokio::spawn(async move {
        let signal_name = wait_for_shutdown_signal().await;
        info!("Received {signal_name}, shutting down...");
        let d = daemon_sig.lock().await;
        d.mpv.shutdown();
        let _ = std::fs::remove_file(mutui_common::socket_path());
        std::process::exit(0);
    });

    server::run(daemon).await
}

async fn wait_for_shutdown_signal() -> &'static str {
    #[cfg(unix)]
    {
        use std::future::pending;
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigterm = signal(SignalKind::terminate()).ok();
        let mut sigquit = signal(SignalKind::quit()).ok();

        tokio::select! {
            _ = tokio::signal::ctrl_c() => "SIGINT",
            _ = async {
                if let Some(sig) = sigterm.as_mut() {
                    let _ = sig.recv().await;
                } else {
                    pending::<()>().await;
                }
            } => "SIGTERM",
            _ = async {
                if let Some(sig) = sigquit.as_mut() {
                    let _ = sig.recv().await;
                } else {
                    pending::<()>().await;
                }
            } => "SIGQUIT",
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        "SIGINT"
    }
}
