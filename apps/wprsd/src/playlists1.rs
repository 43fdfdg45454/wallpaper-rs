use crate::daemon::Daemon;
use crate::error::{Result, failed};
use crate::items::{MAX_UIDS, limited};
use std::sync::{Arc, Weak};
use wprs_library::EntryEdit;

pub struct Playlists(pub Weak<Daemon>);

pub fn library(daemon: &Weak<Daemon>) -> Result<Arc<Daemon>> {
    let daemon = daemon.upgrade().ok_or_else(|| failed("the daemon is stopping"))?;
    daemon.store.error.as_ref().map_or(Ok(daemon.clone()), |e| Err(failed(e.clone())))
}

fn refs(uids: &[String]) -> Result<Vec<&str>> {
    limited(uids, MAX_UIDS)?;
    Ok(uids.iter().map(String::as_str).collect())
}

#[zbus::interface(name = "io.github.wallpaper_rs.Playlists1")]
impl Playlists {
    #[zbus(out_args("json"))]
    async fn list(&self) -> Result<String> {
        library(&self.0)?.run(|d| d.list_playlists()).await
    }

    #[zbus(out_args("json"))]
    async fn get(&self, playlist: String, offset: u32, limit: u32) -> Result<String> {
        library(&self.0)?.run(move |d| d.get_playlist(&playlist, offset, limit)).await
    }

    #[zbus(out_args("uid"))]
    async fn create(&self, name: String, query: String) -> Result<String> {
        library(&self.0)?.run(move |d| d.create_playlist(&name, &query)).await
    }

    async fn rename(&self, playlist: String, name: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.rename_playlist(&playlist, &name)).await
    }

    async fn set_query(&self, playlist: String, query: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.set_query(&playlist, &query)).await
    }

    async fn delete(&self, playlist: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.delete_playlist(&playlist)).await
    }

    async fn add_items(&self, playlist: String, uids: Vec<String>) -> Result<()> {
        let edit = move |d: &Arc<Daemon>| {
            let uids = refs(&uids)?;
            d.edit_entries(&playlist, |lib, uid| lib.add_to_playlist(uid, &uids))
        };
        library(&self.0)?.run(edit).await
    }

    async fn remove_items(&self, playlist: String, uids: Vec<String>) -> Result<()> {
        let edit = move |d: &Arc<Daemon>| {
            let uids = refs(&uids)?;
            d.edit_entries(&playlist, |lib, uid| lib.remove_from_playlist_many(uid, &uids))
        };
        library(&self.0)?.run(edit).await
    }

    async fn remove_entries(&self, playlist: String, indexes: Vec<u32>, expected: Vec<String>) -> Result<()> {
        library(&self.0)?.run(move |d| d.remove_entries(&playlist, &indexes, &expected)).await
    }

    async fn move_item(&self, playlist: String, from: u32, to: u32, expected: String) -> Result<()> {
        let edit = move |d: &Arc<Daemon>| {
            let edit = EntryEdit::Move { from, to, expected: &expected };
            d.edit_entries(&playlist, |lib, uid| lib.edit_entries(uid, edit))
        };
        library(&self.0)?.run(edit).await
    }

    async fn set_rule(&self, playlist: String, key: String, value: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.set_playlist_rule(&playlist, &key, &value)).await
    }

    async fn start(&self, playlist: String, output: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.start(&playlist, &output)).await
    }

    async fn stop(&self, output: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.stop(&output)).await
    }

    async fn next(&self, output: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.step(&output, true)).await
    }

    async fn previous(&self, output: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.step(&output, false)).await
    }

    #[zbus(out_args("json"))]
    async fn history(&self, output: String, limit: u32) -> Result<String> {
        library(&self.0)?.run(move |d| d.history(&output, limit)).await
    }
}
