use crate::*;
use anyhow::Context;
use std::sync::Arc;
use tokio::signal::unix::{SignalKind, signal};
use wprs_core::{Config, dbus};
use zbus::fdo::{DBusProxy, RequestNameFlags};

#[tokio::main]
pub(crate) async fn run(args: Args) -> anyhow::Result<()> {
    let config_path = args.config.map_or_else(Config::default_path, Ok)?;
    let connection = zbus::Connection::session().await.context("cannot connect to the session bus")?;
    let bus = DBusProxy::new(&connection).await?;
    let taken = || anyhow::anyhow!("another wprsd already owns {} on the session bus", dbus::BUS_NAME);
    if bus.name_has_owner(dbus::BUS_NAME.try_into()?).await? {
        return Err(taken());
    }
    compositor::stops::restore();
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
    match session {
        Some(session) => surfaces::start(session, daemon.clone())?,
        None if std::env::var_os("WAYLAND_DISPLAY").is_some() => pixels::follow_modes(&daemon),
        None => {}
    }
    tokio::spawn(flatpak::request_background(connection.clone()));
    let (power, known) = tokio::sync::oneshot::channel();
    tokio::spawn(watch::battery(daemon.clone(), power));
    tokio::spawn(watch::screen_lock(connection.clone(), daemon.clone()));
    tokio::spawn(watch::properties(connection.clone(), daemon.clone()));
    tokio::spawn(watch::motion(connection.clone(), daemon.clone()));
    tokio::spawn(watch::saver(daemon.clone()));
    tokio::spawn(bus::announce_streams(connection.clone(), daemon.clone()));
    tokio::spawn(playback::run(daemon.clone()));
    tokio::spawn(picks::run(daemon.clone()));
    tokio::spawn(crate::choice::follow(daemon.clone(), known));
    if cfg!(debug_assertions) && std::env::var_os("WPRS_RENDER_STOP").is_some() {
        let (daemon, mut stop) = (daemon.clone(), signal(SignalKind::user_defined2())?);
        tokio::spawn(async move { stop.recv().await.map(|()| daemon.compositor.stop()) });
    }
    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
    tracing::info!("shutting down");
    wprs_scene::end_trials();
    inputs::stop(&connection).await;
    wprs_render::finish_builds(0);
    Ok(())
}
