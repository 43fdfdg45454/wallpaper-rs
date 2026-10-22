use crate::db::{ensure, json, label, remove, row_id, rows, uid, unique};
use crate::items::{SELECT_ITEMS, item_row};
use crate::query::compile;
use crate::{Error, Item, Kind, Library, Result, Rules};
use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};
use wprs_schedule::Playlist;

const SELECT_PLAYLISTS: &str = "SELECT uid, name, query, rules FROM playlists";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistInfo {
    pub uid: String,
    pub name: String,
    pub query: Option<String>,
    pub rules: Rules,
}

fn playlist_row(row: &Row<'_>) -> rusqlite::Result<PlaylistInfo> {
    Ok(PlaylistInfo { uid: row.get(0)?, name: row.get(1)?, query: row.get(2)?, rules: json(row, 3)? })
}

fn checked<'a>(name: &'a str, query: Option<&str>) -> Result<&'a str> {
    query.map(|query| compile(query, 0)).transpose()?;
    label("name", name, 128)
}

impl Library {
    pub fn create_playlist(&mut self, name: &str, query: Option<&str>, rules: &Rules) -> Result<String> {
        let (uid, name, rules) = (uid(), checked(name, query)?, serde_json::to_string(rules)?);
        let sql = "INSERT INTO playlists(uid, name, query, rules) VALUES (?1, ?2, ?3, ?4)";
        unique(self.db.execute(sql, params![uid, name, query, rules]), "playlist", name)?;
        Ok(uid)
    }

    pub fn update_playlist(&mut self, playlist: &PlaylistInfo) -> Result<()> {
        let PlaylistInfo { uid, name, query, rules } = playlist;
        let (name, rules) = (checked(name, query.as_deref())?, serde_json::to_string(rules)?);
        let sql = "UPDATE playlists SET name = ?2, query = ?3, rules = ?4 WHERE uid = ?1";
        let changed = unique(self.db.execute(sql, params![uid, name, query, rules]), "playlist", name)?;
        ensure(changed == 1, || Error::NotFound("playlist", uid.clone()))
    }

    pub fn playlists(&self) -> Result<Vec<PlaylistInfo>> {
        rows(&self.db, &format!("{SELECT_PLAYLISTS} ORDER BY name"), [], playlist_row)
    }

    pub fn playlist(&self, key: &str) -> Result<PlaylistInfo> {
        let id = row_id(&self.db, "playlist", key)?;
        Ok(self.db.query_row(&format!("{SELECT_PLAYLISTS} WHERE id = ?1"), [id], playlist_row)?)
    }

    pub fn remove_playlist(&mut self, key: &str) -> Result<()> {
        remove(&self.db, "playlist", key)
    }

    pub fn playlist_items(&self, key: &str, now: i64) -> Result<Vec<Item>> {
        let playlist = self.playlist(key)?;
        if let Some(query) = &playlist.query {
            return self.search(query, now, 0, u32::MAX);
        }
        let sql = format!(
            "{SELECT_ITEMS} JOIN playlist_items p ON p.item_id = i.id JOIN playlists l ON l.id = p.playlist_id \
            WHERE l.uid = ?1 ORDER BY p.pos"
        );
        rows(&self.db, &sql, [playlist.uid], item_row)
    }

    pub fn resolve(&self, key: &str, now: i64, on_battery: bool) -> Result<Playlist> {
        let PlaylistInfo { uid, rules, .. } = self.playlist(key)?;
        let skip_videos = on_battery && rules.skip_videos_on_battery;
        let video = |item: &Item| {
            item.kind == Kind::Video || item.package.as_ref().is_some_and(|p| p.kind == "video")
        };
        let items =
            self.playlist_items(&uid, now)?.into_iter().filter(|i| !i.missing && !(skip_videos && video(i)));
        let (order, interval, per_output) = (rules.order, rules.interval, !rules.synced);
        Ok(Playlist { id: uid, items: items.map(|item| item.path).collect(), order, interval, per_output })
    }

    pub fn add_to_playlist(&mut self, key: &str, items: &[&str]) -> Result<()> {
        self.edit(key, |db, ids| {
            for item in items {
                ids.push(row_id(db, "item", item)?);
            }
            Ok(())
        })
    }

    pub fn remove_from_playlist(&mut self, key: &str, item: &str) -> Result<()> {
        self.edit(key, |db, ids| {
            let id = row_id(db, "item", item)?;
            ids.retain(|other| *other != id);
            Ok(())
        })
    }

    pub fn move_in_playlist(&mut self, key: &str, from: usize, to: usize) -> Result<()> {
        self.edit(key, |_, ids| {
            let len = ids.len();
            let outside = || Error::Invalid("position", format!("{from} or {to} is not below {len}"));
            ensure(from < len && to < len, outside)?;
            let id = ids.remove(from);
            ids.insert(to, id);
            Ok(())
        })
    }

    fn edit(
        &mut self,
        key: &str,
        change: impl FnOnce(&Connection, &mut Vec<i64>) -> Result<()>,
    ) -> Result<()> {
        let tx = self.db.transaction()?;
        let id = row_id(&tx, "playlist", key)?;
        let sql = "SELECT query IS NOT NULL FROM playlists WHERE id = ?1";
        let smart: bool = tx.query_row(sql, [id], |r| r.get(0))?;
        let smart_error =
            || Error::Invalid("playlist", format!("{key:?} is smart, its items come from its query"));
        ensure(!smart, smart_error)?;
        let sql = "SELECT item_id FROM playlist_items WHERE playlist_id = ?1 ORDER BY pos";
        let mut ids = rows(&tx, sql, [id], |r| r.get(0))?;
        change(&tx, &mut ids)?;
        tx.execute("DELETE FROM playlist_items WHERE playlist_id = ?1", [id])?;
        let sql = "INSERT INTO playlist_items(playlist_id, pos, item_id) VALUES (?1, ?2, ?3)";
        for (pos, item) in (0_i64..).zip(ids) {
            tx.execute(sql, params![id, pos, item])?;
        }
        Ok(tx.commit()?)
    }
}
