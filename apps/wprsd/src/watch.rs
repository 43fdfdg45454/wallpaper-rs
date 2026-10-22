use crate::daemon::Daemon;
use crate::service::Service;
use futures_util::StreamExt;
use std::sync::Arc;
use wprs_core::dbus;
use zbus::fdo::NameOwnerChangedStream;
use zbus::{OwnedVariant, Variant};

#[zbus::proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
trait UPower {
    #[zbus(property)]
    fn on_battery(&self) -> zbus::Result<bool>;
}

#[zbus::proxy(
    interface = "org.freedesktop.ScreenSaver",
    default_service = "org.freedesktop.ScreenSaver",
    default_path = "/org/freedesktop/ScreenSaver"
)]
trait ScreenSaver {
    fn get_active(&self) -> zbus::Result<bool>;

    #[zbus(signal)]
    fn active_changed(&self, active: bool) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.portal.Settings",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait Portal {
    fn read_one(&self, namespace: &str, key: &str) -> zbus::Result<OwnedVariant>;

    #[zbus(signal)]
    fn setting_changed(&self, namespace: &str, key: &str, value: Variant<'_>) -> zbus::Result<()>;
}

const APPEARANCE: &str = "org.freedesktop.appearance";
const REDUCED_MOTION: &str = "reduced-motion";

fn reduced(value: &Variant<'_>) -> bool {
    let value = match value {
        Variant::Variant(inner) => inner,
        value => value,
    };
    matches!(value, Variant::U32(1))
}

pub async fn motion(connection: zbus::Connection, daemon: Arc<Daemon>) {
    let follow = async {
        let portal = PortalProxy::new(&connection).await?;
        let mut changes =
            portal.receive_setting_changed_with_args(&[(0, APPEARANCE), (1, REDUCED_MOTION)]).await?;
        if let Ok(value) = portal.read_one(APPEARANCE, REDUCED_MOTION).await {
            daemon.compositor.calm(reduced(&value));
        }
        while let Some(signal) = changes.next().await {
            signal.args().map(|a| daemon.compositor.calm(reduced(&a.value))).ok();
        }
        zbus::Result::Ok(())
    };
    if let Err(e) = follow.await {
        tracing::debug!("reduced motion unknown, parallax stays on: {e}");
    }
}

pub async fn battery(daemon: Arc<Daemon>) {
    let follow = async {
        let upower = UPowerProxy::new(&zbus::Connection::system().await?).await?;
        let mut changes = upower.receive_on_battery_changed().await;
        upower.on_battery().await.map(|v| daemon.set_on_battery(v)).ok();
        while let Some(change) = changes.next().await {
            change.get().await.map(|v| daemon.set_on_battery(v)).ok();
        }
        zbus::Result::Ok(())
    };
    if let Err(e) = follow.await {
        tracing::info!("power source unknown, battery pause disabled: {e}");
    }
}

pub async fn screen_lock(connection: zbus::Connection, daemon: Arc<Daemon>) {
    let follow = async {
        let screensaver = ScreenSaverProxy::new(&connection).await?;
        let mut changes = screensaver.receive_active_changed().await?;
        match screensaver.get_active().await {
            Ok(locked) => daemon.set_locked(locked),
            Err(e) => tracing::debug!("screen lock state unknown until it changes: {e}"),
        }
        while let Some(signal) = changes.next().await {
            signal.args().map(|a| daemon.set_locked(a.active)).ok();
        }
        zbus::Result::Ok(())
    };
    if let Err(e) = follow.await {
        tracing::info!("cannot follow the screen lock: {e}");
    }
}

pub async fn clients(mut vanished: NameOwnerChangedStream, daemon: Arc<Daemon>) {
    while let Some(signal) = vanished.next().await {
        let Ok(args) = signal.args() else { continue };
        if args.new_owner().is_none()
            && let Err(e) = daemon.drop_client(args.name())
        {
            tracing::warn!("cannot release the streams of a client that left: {e:#}");
        }
    }
}

pub async fn properties(connection: zbus::Connection, daemon: Arc<Daemon>) {
    let mut changes = daemon.props.subscribe();
    let follow = async {
        let iface = connection.object_server().interface::<_, Service>(dbus::OBJECT_PATH).await?;
        let mut last = *changes.borrow_and_update();
        while changes.changed().await.is_ok() {
            let now = *changes.borrow_and_update();
            let (service, emitter) = (iface.get().await, iface.signal_emitter());
            if now.paused != last.paused {
                service.paused_changed(emitter).await?;
            }
            if now.volume != last.volume {
                service.volume_changed(emitter).await?;
            }
            if now.muted != last.muted {
                service.muted_changed(emitter).await?;
            }
            last = now;
        }
        zbus::Result::Ok(())
    };
    if let Err(e) = follow.await {
        tracing::warn!("cannot announce property changes: {e}");
    }
}
