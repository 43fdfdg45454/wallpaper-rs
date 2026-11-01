use std::collections::HashMap;
use std::path::Path;
use zbus::{Fd, OwnedObjectPath, Variant as Value};

#[zbus::proxy(
    interface = "org.freedesktop.portal.OpenURI",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait OpenUri {
    #[zbus(name = "OpenURI")]
    fn open_uri(
        &self,
        parent: &str,
        uri: &str,
        options: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
    fn open_file(
        &self,
        parent: &str,
        fd: Fd<'_>,
        options: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
}

pub(super) async fn uri(connection: &zbus::Connection, uri: &str) -> anyhow::Result<()> {
    OpenUriProxy::new(connection).await?.open_uri("", uri, HashMap::new()).await?;
    Ok(())
}

pub(super) async fn file(connection: &zbus::Connection, path: &Path) -> anyhow::Result<()> {
    let file = std::fs::File::open(path)?;
    OpenUriProxy::new(connection).await?.open_file("", Fd::from(&file), HashMap::new()).await?;
    Ok(())
}
