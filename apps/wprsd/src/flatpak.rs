use std::collections::HashMap;
use zbus::{OwnedObjectPath, Variant};

#[zbus::proxy(
    interface = "org.freedesktop.portal.Background",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait Background {
    fn request_background(
        &self,
        parent_window: &str,
        options: HashMap<&str, Variant<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
}

pub fn app_id() -> Option<String> {
    std::env::var("FLATPAK_ID").ok().filter(|id| !id.is_empty())
}

pub async fn request_background(connection: zbus::Connection) {
    if app_id().is_none() {
        return;
    }
    let options = HashMap::from([("reason", Variant::from("Draws the desktop wallpaper"))]);
    let request = async { BackgroundProxy::new(&connection).await?.request_background("", options).await };
    if let Err(e) = request.await {
        tracing::warn!("cannot ask the background portal to keep wprsd running: {e}");
    }
}
