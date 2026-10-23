use crate::bus::client;
use crate::changes::Signal;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid};
use crate::items::limited;
use crate::thumbs::{Req, SIZES};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Weak};
use std::time::Duration;
use zbus::message::Header;
use zbus::object_server::SignalEmitter;

pub struct Library(pub Weak<Daemon>);

fn text(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

fn strs(list: &[String]) -> Vec<&str> {
    list.iter().map(String::as_str).collect()
}

fn thumb_size(size: u32) -> Result<u32> {
    if SIZES.contains(&size) { Ok(size) } else { Err(invalid(format!("size must be one of {SIZES:?}"))) }
}

impl Library {
    fn d(&self) -> Result<Arc<Daemon>> {
        let daemon = self.0.upgrade().ok_or_else(|| failed("the daemon is stopping"))?;
        daemon.store.error.as_ref().map_or(Ok(daemon.clone()), |e| Err(failed(e.clone())))
    }
}

#[zbus::interface(name = "io.github.wallpaper_rs.Library1")]
impl Library {
    #[zbus(out_args("source", "job"))]
    async fn add_source(&self, path: String, recursive: bool) -> Result<(String, u64)> {
        self.d()?.run(move |d| d.add_source(&path, recursive)).await
    }

    async fn remove_source(&self, source: String) -> Result<()> {
        self.d()?.run(move |d| d.remove_source(&source)).await
    }

    #[zbus(out_args("json"))]
    async fn list_sources(&self) -> Result<String> {
        self.d()?.run(|d| d.list_sources()).await
    }

    #[zbus(out_args("job"))]
    async fn rescan(&self, source: String) -> Result<u64> {
        self.d()?.run(move |d| d.rescan(&source)).await
    }

    #[zbus(out_args("job"))]
    async fn import(&self, paths: Vec<String>, copy: bool) -> Result<u64> {
        self.d()?.run(move |d| d.import(paths, copy)).await
    }

    #[zbus(out_args("uids"))]
    async fn resolve(&self, refs: Vec<String>) -> Result<Vec<String>> {
        self.d()?.run(move |d| d.resolve(&refs)).await
    }

    async fn remove(&self, uids: Vec<String>, delete_files: bool) -> Result<()> {
        self.d()?.run(move |d| d.remove_items(&uids, delete_files)).await
    }

    #[zbus(out_args("json"))]
    async fn search(&self, query: String, offset: u32, limit: u32) -> Result<String> {
        self.d()?.run(move |d| d.search(&query, offset, limit)).await
    }

    #[zbus(out_args("json"))]
    async fn get_item(&self, uid: String) -> Result<String> {
        self.d()?.run(move |d| d.get_item(&uid)).await
    }

    async fn set_title(&self, uid: String, title: String) -> Result<()> {
        self.d()?.run(move |d| d.edit_items(&[uid], |lib, uids| lib.set_title(uids[0], &title))).await
    }

    async fn set_rating(&self, uids: Vec<String>, rating: u32) -> Result<()> {
        let rating =
            u8::try_from(rating).ok().filter(|r| *r <= 5).ok_or_else(|| invalid("rating must be 0 to 5"))?;
        self.d()?.run(move |d| d.edit_items(&uids, |lib, uids| lib.set_rating_many(uids, rating))).await
    }

    async fn set_favorite(&self, uids: Vec<String>, favorite: bool) -> Result<()> {
        self.d()?.run(move |d| d.edit_items(&uids, |lib, uids| lib.set_favorite_many(uids, favorite))).await
    }

    async fn set_tags(&self, uids: Vec<String>, add: Vec<String>, remove: Vec<String>) -> Result<()> {
        self.d()?
            .run(move |d| {
                limited(&add, crate::items::MAX_UIDS).and(limited(&remove, crate::items::MAX_UIDS))?;
                d.edit_items(&uids, |lib, uids| lib.retag(uids, &strs(&add), &strs(&remove)))?;
                d.changes.changed("library", Vec::new());
                Ok::<_, Error>(())
            })
            .await
    }

    async fn confirm(&self, uids: Vec<String>, what: String) -> Result<()> {
        self.d()?
            .run(move |d| {
                limited(&uids, crate::items::MAX_UIDS)?;
                let changed = d.store.write(|lib| lib.confirm(&strs(&uids), &what))?;
                d.changes.changed("library", changed);
                Ok::<_, Error>(())
            })
            .await
    }

    #[zbus(out_args("json"))]
    async fn list_tags(&self) -> Result<String> {
        self.d()?.run(|d| d.list_tags()).await
    }

    async fn rename_tag(&self, from: String, to: String) -> Result<()> {
        self.d()?
            .run(move |d| {
                d.store.write(|lib| lib.rename_tag(&from, &to))?;
                d.library_changed(Vec::new());
                Ok::<_, Error>(())
            })
            .await
    }

    #[zbus(out_args("path"))]
    async fn thumbnail(&self, uid: String, size: u32) -> Result<String> {
        let (d, size) = (self.d()?, thumb_size(size)?);
        let cached = move |d: &Arc<Daemon>| {
            let item = d.store.read(|lib| lib.item(&uid))?;
            Ok::<_, Error>((d.thumbs.cached(&item, size), item))
        };
        let item = match d.run(cached).await? {
            (Some(path), _) => return Ok(text(path)),
            (None, item) if item.missing => {
                return Err(failed(format!("the file of {} is missing", item.uid)));
            }
            (None, item) => item,
        };
        let (tx, rx) = tokio::sync::oneshot::channel();
        d.thumbs.queue(item, Req::Wait(size, tx));
        match tokio::time::timeout(Duration::from_secs(20), rx).await {
            Ok(Ok(Ok(path))) => Ok(text(path)),
            Ok(Ok(Err(reason))) => Err(failed(reason)),
            _ => Err(failed("the thumbnail was not ready within 20 s")),
        }
    }

    #[zbus(out_args("paths"))]
    async fn request_thumbnails(
        &self,
        uids: Vec<String>,
        size: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<HashMap<String, String>> {
        let (d, size, sender) = (self.d()?, thumb_size(size)?, client(&header));
        limited(&uids, 500)?;
        d.run(move |d| {
            d.thumbs.forget(&sender);
            let items = d.store.read(|lib| {
                Ok::<_, Error>(uids.iter().filter_map(|u| lib.item(u).ok()).collect::<Vec<_>>())
            })?;
            let mut cached = HashMap::new();
            for item in items {
                match d.thumbs.cached(&item, size) {
                    Some(path) => drop(cached.insert(item.uid, text(path))),
                    None if item.missing => d.changes.send(Signal::Thumbnail(item.uid, size, String::new())),
                    None => d.thumbs.queue(item, Req::Signal(sender.clone(), size)),
                }
            }
            Ok::<_, Error>(cached)
        })
        .await
    }

    async fn apply(&self, uid: String, output: String, fit: String, preset: String) -> Result<()> {
        self.d()?.render(move |d| d.apply(&uid, &output, &fit, &preset)).await
    }

    #[zbus(signal)]
    pub async fn thumbnail_ready(
        emitter: &SignalEmitter<'_>,
        uid: &str,
        size: u32,
        path: &str,
    ) -> zbus::Result<()>;
}
