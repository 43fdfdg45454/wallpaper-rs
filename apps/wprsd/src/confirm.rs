use crate::compositor::lock;
use crate::error::{Error, Result, denied};
use crate::inputs::{Grants, Hub};
use futures_util::StreamExt;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};
use wprs_core::dbus::error::UNCONFIRMED;
use wprs_core::model::Input;
use zbus::{Variant, fdo};

const WAIT: Duration = Duration::from_secs(120);
const QUIET: Duration = Duration::from_secs(600);
const APP: &str = "io.github.wallpaper_rs";

type Asked = (String, Input);

static ASKED: LazyLock<Mutex<BTreeMap<Asked, Option<Instant>>>> = LazyLock::new(Default::default);

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Variant<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    fn close_notification(&self, id: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn notification_closed(&self, id: u32, reason: u32) -> zbus::Result<()>;
}

pub(crate) async fn ask(
    bus: &zbus::Connection,
    (summary, body): (String, String),
) -> zbus::Result<Option<bool>> {
    let notes = NotificationsProxy::new(bus).await?;
    let (mut actions, mut closed) =
        (notes.receive_action_invoked().await?, notes.receive_notification_closed().await?);
    let (allow, deny) = crate::words::answers();
    let buttons = ["allow", allow.as_str(), "deny", deny.as_str()];
    let hints = HashMap::from([("desktop-entry", Variant::from(APP))]);
    let id = notes.notify("wallpaper-rs", 0, APP, &summary, &body, &buttons, hints, 0).await?;
    let answer = async {
        loop {
            tokio::select! {
                Some(signal) = actions.next() => match signal.args() {
                    Ok(args) if args.id == id => return Some(args.action_key == "allow"),
                    _ => {}
                },
                Some(signal) = closed.next() => if signal.args().is_ok_and(|args| args.id == id) {
                    return None;
                },
                else => return None,
            }
        }
    };
    let answer = tokio::time::timeout(WAIT, answer).await;
    if answer.is_err() {
        let _ = notes.close_notification(id).await;
    }
    Ok(answer.ok().flatten())
}

fn open(asked: &Asked, now: Instant) -> Result<bool> {
    let mut all = lock(&ASKED);
    all.retain(|_, quiet| quiet.is_none_or(|until| until > now));
    match all.get(asked) {
        Some(None) => Ok(false),
        Some(Some(_)) => {
            Err(denied("the user did not allow this input a moment ago; ask again in a few minutes"))
        }
        None => Ok(all.insert(asked.clone(), None).is_none()),
    }
}

pub(crate) fn questions() -> (Grants, Grants) {
    let (mut asking, mut quiet, now) = (Grants::new(), Grants::new(), Instant::now());
    for ((scene, input), until) in lock(&ASKED).iter() {
        let list = match until {
            None => &mut asking,
            Some(until) if *until > now => &mut quiet,
            Some(_) => continue,
        };
        list.entry(scene.clone()).or_default().insert(*input);
    }
    (asking, quiet)
}

pub(crate) async fn grant(
    bus: &zbus::Connection,
    hub: Arc<Hub>,
    path: PathBuf,
    (scene, key, title): (String, String, String),
    input: Input,
) -> Result<()> {
    if !notifications(bus).await {
        return Err(denied(
            "only the wallpaper-rs app, the tray and wprs-ctl grant inputs; nothing can ask the user",
        ));
    }
    let asked = (scene.clone(), input);
    if open(&asked, Instant::now())? {
        hub.changed();
        let bus = bus.clone();
        tokio::spawn(async move {
            let words = crate::words::request(&title, input == Input::Audio);
            let answer = ask(&bus, words).await.inspect_err(|e| tracing::warn!("cannot ask the user: {e}"));
            let quiet = matches!(answer, Ok(None | Some(false)));
            if let Ok(Some(allowed)) = answer {
                let (hub, path) = (hub.clone(), path.clone());
                let set = tokio::task::spawn_blocking(move || match allowed {
                    true => hub.grant(&path, &scene, &key, input),
                    false => hub.decline(&path, &scene, &key, input),
                });
                if let Ok(Err(e)) = set.await {
                    tracing::warn!("cannot keep the answer of the user: {e:#}");
                }
            }
            let until = quiet.then(|| Instant::now() + QUIET);
            match until {
                Some(until) => drop(lock(&ASKED).insert(asked.clone(), Some(until))),
                None => drop(lock(&ASKED).remove(&asked)),
            }
            hub.changed();
            let Some(until) = until else { return };
            tokio::time::sleep_until(until.into()).await;
            let mut all = lock(&ASKED);
            if all.get(&asked) == Some(&Some(until)) {
                all.remove(&asked);
                drop(all);
                hub.changed();
            }
        });
    }
    Err(Error::Named(
        UNCONFIRMED,
        "the user is asked in a notification; the input is granted once allowed".into(),
    ))
}

async fn notifications(bus: &zbus::Connection) -> bool {
    let name = "org.freedesktop.Notifications";
    let Ok(dbus) = fdo::DBusProxy::new(bus).await else { return false };
    let Ok(owned) = name.try_into() else { return false };
    let activatable =
        async { dbus.list_activatable_names().await.is_ok_and(|n| n.iter().any(|n| n.as_str() == name)) };
    dbus.name_has_owner(owned).await.unwrap_or(false) || activatable.await
}

#[cfg(test)]
pub(crate) mod tests;
