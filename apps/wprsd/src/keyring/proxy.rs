use super::{Paths, Secret};
use std::collections::HashMap;
use zbus::{ObjectPath, OwnedObjectPath, OwnedVariant, Variant};

#[zbus::proxy(
    interface = "org.freedesktop.Secret.Service",
    default_service = "org.freedesktop.secrets",
    default_path = "/org/freedesktop/secrets"
)]
pub trait Service {
    fn open_session(
        &self,
        algorithm: &str,
        input: &Variant<'_>,
    ) -> zbus::Result<(OwnedVariant, OwnedObjectPath)>;
    fn search_items(&self, attributes: HashMap<&str, &str>) -> zbus::Result<(Paths, Paths)>;
    fn unlock(&self, objects: &[OwnedObjectPath]) -> zbus::Result<(Paths, OwnedObjectPath)>;
    fn get_secrets(
        &self,
        items: &[OwnedObjectPath],
        session: &ObjectPath<'_>,
    ) -> zbus::Result<HashMap<OwnedObjectPath, Secret>>;
}

#[zbus::proxy(interface = "org.freedesktop.Secret.Collection", default_service = "org.freedesktop.secrets")]
pub trait Collection {
    fn create_item(
        &self,
        properties: HashMap<&str, Variant<'_>>,
        secret: &Secret,
        replace: bool,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;
    #[zbus(property)]
    fn locked(&self) -> zbus::Result<bool>;
}

#[zbus::proxy(interface = "org.freedesktop.Secret.Item", default_service = "org.freedesktop.secrets")]
pub trait Item {
    fn delete(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(interface = "org.freedesktop.Secret.Prompt", default_service = "org.freedesktop.secrets")]
pub trait Prompt {
    fn prompt(&self, window_id: &str) -> zbus::Result<()>;
    fn dismiss(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn completed(&self, dismissed: bool, result: Variant<'_>) -> zbus::Result<()>;
}
