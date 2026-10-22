mod account;
mod apply;
mod bus;
mod changes;
mod clock;
mod compositor;
mod controls;
mod daemon;
mod decode;
mod edits;
mod error;
mod examples;
mod flatpak;
mod folders;
mod import;
mod inputs;
mod inputs1;
mod items;
mod jobs;
mod keyring;
mod kinds;
mod library1;
mod packages;
mod playback;
mod playlists;
mod playlists1;
mod pool;
mod present;
mod presets;
mod previews;
mod profiles;
mod profiles1;
mod providers;
mod providers1;
mod relief;
mod remote;
mod repos;
mod repos1;
mod rotation;
mod scan;
mod sceneprops;
mod scenes;
mod schema;
mod service;
#[cfg(test)]
mod spec;
mod store;
mod streams;
mod sync;
mod sync1;
mod thumbs;
mod timing;
mod wallpaper;
mod watch;
mod words;
mod worker;

use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal::unix::{SignalKind, signal};
use tracing_subscriber::EnvFilter;
use wprs_core::{Config, dbus};
use zbus::fdo::{DBusProxy, RequestNameFlags};

#[derive(Parser)]
#[command(
    version = wprs_core::VERSION,
    about = "Renders wallpapers and shows them through PipeWire streams, layer-shell surfaces or X11 windows"
)]
struct Args {
    #[arg(long, help = "Configuration file to use instead of the default one")]
    config: Option<PathBuf>,
    #[arg(long, hide = true)]
    worker: bool,
    #[arg(long, hide = true)]
    script_host: bool,
    #[arg(long, value_enum, default_value = "auto", help = "How wallpapers reach the desktop")]
    backend: present::Backend,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.worker {
        return worker::run();
    }
    if args.script_host {
        return Ok(wprs_script::serve()?);
    }
    skip_vulkan_device_selection_layer();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let quiet =
        ["zbus", "hyper", "hyper_util", "h2", "reqwest"].map(|target| format!("{target}=warn").parse());
    let filter = quiet.into_iter().flatten().fold(filter, EnvFilter::add_directive);
    tracing_subscriber::fmt().with_env_filter(filter).init();
    std::panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        fatal(&format!("thread {} {info}", thread.name().unwrap_or("unnamed")))
    }));
    run(args)
}

fn skip_vulkan_device_selection_layer() {
    if std::env::var_os("NODEVICE_SELECT").is_none() {
        unsafe { std::env::set_var("NODEVICE_SELECT", "1") };
    }
}

pub(crate) fn fatal(reason: &str) -> ! {
    tracing::error!("{reason}; exiting so that the service manager restarts the daemon");
    std::process::exit(70)
}

#[tokio::main]
async fn run(args: Args) -> anyhow::Result<()> {
    let config_path = args.config.map_or_else(Config::default_path, Ok)?;
    let connection = zbus::Connection::session().await.context("cannot connect to the session bus")?;
    let bus = DBusProxy::new(&connection).await?;
    let taken = || anyhow::anyhow!("another wprsd already owns {} on the session bus", dbus::BUS_NAME);
    if bus.name_has_owner(dbus::BUS_NAME.try_into()?).await? {
        return Err(taken());
    }
    let session = present::select(args.backend, &bus).await?;
    let display = session.as_ref().map(|s| Box::new(s.display()) as wprs_render::Display);
    let daemon =
        Arc::new(tokio::task::spawn_blocking(move || daemon::Daemon::new(config_path, display)).await??);
    let vanished = bus.receive_name_owner_changed_with_args(&[(2, "")]).await?;
    tokio::spawn(watch::clients(vanished, daemon.clone()));
    let server = connection.object_server();
    let weak = Arc::downgrade(&daemon);
    server.at(dbus::OBJECT_PATH, service::Service(weak.clone())).await?;
    server.at(dbus::OBJECT_PATH, library1::Library(weak.clone())).await?;
    server.at(dbus::OBJECT_PATH, playlists1::Playlists(weak.clone())).await?;
    server.at(dbus::OBJECT_PATH, profiles1::Profiles(weak.clone())).await?;
    server.at(dbus::OBJECT_PATH, providers1::Providers(weak.clone())).await?;
    server.at(dbus::OBJECT_PATH, inputs1::Inputs(weak.clone())).await?;
    server.at(dbus::OBJECT_PATH, repos1::Repos(weak.clone())).await?;
    server.at(dbus::OBJECT_PATH, sync1::Sync(weak)).await?;
    tokio::spawn(changes::emit(connection.clone(), daemon.clone()));
    inputs1::start(&daemon, connection.clone());
    pool::start(&daemon);
    folders::start(&daemon);
    remote::start(&daemon, connection.clone());
    account::start(&daemon, connection.clone());
    edits::start(&daemon);
    tokio::task::spawn_blocking({
        let daemon = daemon.clone();
        move || daemon.tidy()
    });
    let flags = RequestNameFlags::DO_NOT_QUEUE;
    connection.request_name_with_flags(dbus::BUS_NAME, flags).await.map_err(|e| match e {
        zbus::Error::NameTaken => taken(),
        e => e.into(),
    })?;
    tracing::info!("serving {} on the session bus", dbus::BUS_NAME);
    session.map(|s| present::start(s, daemon.clone())).transpose()?;
    tokio::spawn(flatpak::request_background(connection.clone()));
    tokio::spawn(watch::battery(daemon.clone()));
    tokio::spawn(watch::screen_lock(connection.clone(), daemon.clone()));
    tokio::spawn(watch::properties(connection.clone(), daemon.clone()));
    tokio::spawn(watch::motion(connection.clone(), daemon.clone()));
    tokio::spawn(bus::announce_streams(connection.clone(), daemon.clone()));
    tokio::spawn(playback::run(daemon.clone()));
    tokio::spawn(profiles1::follow(daemon.clone()));
    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
    tracing::info!("shutting down");
    inputs::stop(&connection).await;
    Ok(())
}
