//! wallpaper-rs daemon.
//!
//! Renders the configured wallpaper for every output that requests a stream
//! and exports it over PipeWire. Clients (the Plasma wallpaper plugin, the
//! CLI) talk to it over the session bus.

mod daemon;
mod service;

use anyhow::Context;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use wprs_core::{Config, dbus};

fn usage() -> ! {
    eprintln!(
        "usage: wprsd [--config PATH]\n\n\
         Runs the wallpaper-rs daemon on the session bus as {}.\n\
         Logging is controlled with RUST_LOG (default: info).",
        dbus::BUS_NAME
    );
    std::process::exit(2)
}

fn parse_args() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    let mut config = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => config = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--version" => {
                println!("wprsd {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0)
            }
            _ => usage(),
        }
    }
    config
}

fn main() -> anyhow::Result<()> {
    // Mesa's Vulkan device selection layer inspects the Wayland session to
    // pick the compositor's GPU, and crashes when that device has no Vulkan
    // driver (virtual machines, software rendering). The daemon only renders
    // offscreen and picks its adapter through wgpu, so it skips the layer
    // unless the user configured it explicitly.
    if std::env::var_os("NODEVICE_SELECT").is_none() {
        // SAFETY: no other thread exists yet; the runtime starts below.
        unsafe { std::env::set_var("NODEVICE_SELECT", "1") };
    }
    run()
}

#[tokio::main]
async fn run() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config_path = match parse_args() {
        Some(p) => p,
        None => Config::default_path()?,
    };
    let config = Config::load(&config_path)?;
    tracing::info!(path = %config_path.display(), "configuration loaded");

    let daemon = tokio::task::spawn_blocking(move || daemon::Daemon::new(config, config_path))
        .await
        .context("startup task panicked")??;
    let daemon = Arc::new(daemon);

    let _connection = zbus::connection::Builder::session()
        .name(dbus::BUS_NAME)
        .serve_at(dbus::OBJECT_PATH, service::Service::new(daemon.clone()))
        .build()
        .await
        .with_context(|| {
            format!(
                "cannot own {} on the session bus (is another wprsd running?)",
                dbus::BUS_NAME
            )
        })?;
    tracing::info!("serving {} on the session bus", dbus::BUS_NAME);

    wait_for_shutdown().await;
    tracing::info!("shutting down");
    Ok(())
}

async fn wait_for_shutdown() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}
