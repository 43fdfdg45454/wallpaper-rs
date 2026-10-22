use crate::db::{ensure, json, label, remove, row_id, rows, uid, unique};
use crate::items::{SELECT_ITEMS, item_row};
use crate::query::compile;
use crate::{Error, Item, Library, Result, Rules};
use rusqlite::{Row, params};
use serde::{Deserialize, Serialize};

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

    pub fn item_playlists(&self, uid: &str) -> Result<Vec<PlaylistInfo>> {
        let id = row_id(&self.db, "item", uid)?;
        let sql = format!(
            "{SELECT_PLAYLISTS} WHERE query IS NULL \
            AND id IN (SELECT playlist_id FROM playlist_items WHERE item_id = ?1) ORDER BY name"
        );
        rows(&self.db, &sql, [id], playlist_row)
    }
}
