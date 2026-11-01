use crate::compositor::lock;
use crate::daemon::Daemon;
use serde_json::Value as Json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use wprs_scene::Control;

mod launch;
mod portal;
#[cfg(test)]
mod tests;

const MAX_WAITING: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ask {
    pub(crate) output: String,
    pub(crate) package: String,
    pub(crate) prop: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    Uri(String),
    File(PathBuf),
    Folder(PathBuf),
    App(String),
}

static WAITING: Mutex<Vec<Ask>> = Mutex::new(Vec::new());
static WAKE: Notify = Notify::const_new();

pub(crate) fn ask(output: &str, package: &str, prop: &str) {
    let mut waiting = lock(&WAITING);
    if waiting.len() < MAX_WAITING {
        waiting.push(Ask { output: output.into(), package: package.into(), prop: prop.into() });
        WAKE.notify_one();
    }
}

pub(crate) fn target(value: &Json) -> Option<Target> {
    let map = value.as_object().filter(|m| m.len() == 1)?;
    let (key, text) = map.iter().next().and_then(|(k, v)| Some((k.as_str(), v.as_str()?)))?;
    let absolute = || text.starts_with('/') && !text.split('/').any(|p| p == "..");
    match key {
        "uri" if text.starts_with("https://") && text.len() <= 2048 => Some(Target::Uri(text.into())),
        "file" if absolute() => Some(Target::File(text.into())),
        "folder" if absolute() => Some(Target::Folder(text.into())),
        "app" if launch::valid_id(text) => Some(Target::App(text.into())),
        _ => None,
    }
}

pub(crate) async fn serve(connection: zbus::Connection, daemon: Arc<Daemon>) {
    loop {
        WAKE.notified().await;
        let asks = std::mem::take(&mut *lock(&WAITING));
        for ask in asks {
            let Some(target) = daemon.shortcut(&ask).as_ref().and_then(target) else {
                tracing::debug!(
                    package = ask.package,
                    prop = ask.prop,
                    "a shortcut without a target was asked"
                );
                continue;
            };
            let opened = match &target {
                Target::Uri(uri) => portal::uri(&connection, uri).await,
                Target::File(path) | Target::Folder(path) => portal::file(&connection, path).await,
                Target::App(id) => launch::app(&connection, id).await,
            };
            if let Err(e) = opened {
                tracing::warn!(package = ask.package, "cannot open the shortcut the user chose: {e:#}");
            }
        }
    }
}

impl Daemon {
    pub(crate) fn shortcut(&self, ask: &Ask) -> Option<Json> {
        let st = self.state();
        let info = st.outputs.get(&ask.output)?;
        let props = info.scene.as_ref().map(|s| s.props.clone()).or_else(|| info.video.clone())?;
        let kind = props.properties.get(&ask.prop).map(|p| &p.control);
        if props.id != ask.package || !matches!(kind, Some(Control::Shortcut {})) {
            return None;
        }
        props.values(&st.config.scene_values(&ask.output, &props.id)).remove(&ask.prop)
    }
}
