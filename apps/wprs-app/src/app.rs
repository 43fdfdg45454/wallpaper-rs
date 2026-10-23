use crate::bus::{self, Daemon, Event, publish, set_link, spawn};
use futures_util::StreamExt;
use percent_encoding::percent_decode_str;
use serde_json::json;
use std::collections::HashMap;
use std::process::ExitCode;
use wprs_client::{Client, Daemon1Proxy, Library1Proxy, Repos1Proxy, explain};
use wprs_core::dbus::{APP_ID, APP_PATH, BUS_NAME};
use zbus::Connection;
use zbus::fdo::{DBusProxy, RequestNameFlags};
use zbus::proxy::CacheProperties;
use zbus::{OwnedVariant, Variant};

const APPLICATION: &str = "org.freedesktop.Application";
pub const PAGES: [&str; 7] =
    ["library", "playlists", "profiles", "providers", "repos", "settings", "account"];

pub fn page(name: Option<&str>) -> Option<&'static str> {
    PAGES.into_iter().find(|page| name == Some(page))
}

pub fn start(uris: Vec<String>, page: Option<&str>) -> Option<ExitCode> {
    if let Some(page) = page {
        bus::request(page);
    }
    if bus::checking() {
        set_link(Ok(Daemon::Fixtures));
        return None;
    }
    bus::block_on(async move {
        let conn = match Connection::session().await {
            Ok(conn) => conn,
            Err(e) => {
                set_link(Err(format!("cannot connect to the session bus: {e}")));
                return None;
            }
        };
        if conn.object_server().at(APP_PATH, Application).await.is_ok()
            && let Err(zbus::Error::NameTaken) =
                conn.request_name_with_flags(APP_ID, RequestNameFlags::DO_NOT_QUEUE).await
        {
            let Err(e) = pass_on(&conn, uris, page).await else { return Some(ExitCode::SUCCESS) };
            eprintln!("wprs-app: {}", explain(e));
            return Some(ExitCode::FAILURE);
        }
        spawn(watch(conn));
        spawn(open(uris));
        None
    })
}

pub async fn pass_on(conn: &Connection, uris: Vec<String>, page: Option<&str>) -> zbus::Result<()> {
    let (data, to) = (HashMap::<&str, Variant>::new(), (Some(APP_ID), APP_PATH, Some(APPLICATION)));
    if !uris.is_empty() {
        conn.call_method(to.0, to.1, to.2, "Open", &(&uris, &data)).await?;
    }
    if let Some(page) = page {
        conn.call_method(to.0, to.1, to.2, "ActivateAction", &(page, Vec::<Variant>::new(), &data)).await?;
    } else if uris.is_empty() {
        conn.call_method(to.0, to.1, to.2, "Activate", &(&data,)).await?;
    }
    Ok(())
}

async fn watch(conn: Connection) {
    let Ok(owners) = DBusProxy::new(&conn).await else { return };
    let Ok(mut owners) = owners.receive_name_owner_changed_with_args(&[(0, BUS_NAME)]).await else { return };
    let signals = conn.clone();
    spawn(async move {
        forward(signals).await.ok();
    });
    connect(&conn).await;
    while let Some(change) = owners.next().await {
        if change.args().is_ok_and(|a| a.new_owner.is_some()) {
            connect(&conn).await;
        }
    }
}

async fn connect(conn: &Connection) {
    set_link(Client::new(conn.clone()).await.map(Daemon::Bus).map_err(|e| e.to_string()));
}

async fn forward(conn: Connection) -> zbus::Result<()> {
    let daemon = Daemon1Proxy::builder(&conn).cache_properties(CacheProperties::No).build().await?;
    let library = Library1Proxy::builder(&conn).cache_properties(CacheProperties::No).build().await?;
    let repos = Repos1Proxy::builder(&conn).cache_properties(CacheProperties::No).build().await?;
    let (mut changed, mut progress) = (daemon.receive_changed().await?, daemon.receive_job_progress().await?);
    let (mut finished, mut thumbs) =
        (daemon.receive_job_finished().await?, library.receive_thumbnail_ready().await?);
    let mut previews = repos.receive_preview_ready().await?;
    loop {
        let event = tokio::select! {
            Some(s) = changed.next() => s.args().map(|a| {
                bus::saw(a.topic, a.serial);
                Event::Changed(a.topic.into(), a.ids, a.serial)
            }),
            Some(s) = progress.next() => s.args().map(|a| Event::Progress(a.job, a.done, a.total)),
            Some(s) = finished.next() => {
                s.args().map(|a| Event::Finished(a.job, a.state.into(), a.error_name.into(), a.message.into()))
            }
            Some(s) = thumbs.next() => s.args().map(|a| Event::Thumb(a.uid.into(), a.size, a.path.into())),
            Some(s) = previews.next() => s.args().map(|a| Event::Preview(a.spec.into(), a.path.into())),
            else => return Ok(()),
        };
        event.map(publish).ok();
    }
}

async fn open(uris: Vec<String>) {
    let paths: Vec<String> = uris.iter().filter_map(|uri| path(uri)).collect();
    if !paths.is_empty()
        && let Err(e) = bus::call("Library1", "Import", vec![json!(paths), json!(false)]).await
    {
        publish(Event::Error(e.message));
    }
}

pub fn path(uri: &str) -> Option<String> {
    let path = match uri.strip_prefix("file://") {
        Some(rest) => percent_decode_str(rest).decode_utf8().ok()?.into_owned(),
        None => uri.to_owned(),
    };
    path.starts_with('/').then_some(path)
}

pub struct Application;

#[zbus::interface(name = "org.freedesktop.Application")]
impl Application {
    fn activate(&self, _platform_data: HashMap<String, OwnedVariant>) {
        publish(Event::Activate(String::new()));
    }

    fn open(&self, uris: Vec<String>, _platform_data: HashMap<String, OwnedVariant>) {
        spawn(open(uris));
        publish(Event::Activate(String::new()));
    }

    fn activate_action(
        &self,
        name: String,
        _parameter: Vec<OwnedVariant>,
        _platform_data: HashMap<String, OwnedVariant>,
    ) {
        match page(Some(&name)) {
            Some(page) => bus::request(page),
            None => publish(Event::Activate(String::new())),
        }
    }
}
