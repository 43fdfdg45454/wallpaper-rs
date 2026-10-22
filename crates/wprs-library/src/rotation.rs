use crate::db::{json, row_id, rows};
use crate::{Library, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use wprs_schedule::PlaylistState;

const HISTORY: i64 = 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Used {
    pub at: i64,
    pub item: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rotation {
    pub output: String,
    pub playlist: String,
    pub state: PlaylistState,
    pub next_at: Option<i64>,
    pub current: Option<String>,
}

impl Library {
    pub fn save_rotation(&mut self, rotation: &Rotation) -> Result<()> {
        let Rotation { output, playlist, state, next_at, current } = rotation;
        let id = row_id(&self.db, "playlist", playlist)?;
        let sql = "INSERT OR REPLACE INTO rotation(output, playlist_id, state, next_at, current) \
            VALUES (?1, ?2, ?3, ?4, ?5)";
        self.db.execute(sql, params![output, id, serde_json::to_string(state)?, next_at, current])?;
        Ok(())
    }

    pub fn rotations(&self) -> Result<Vec<Rotation>> {
        let sql = "SELECT r.output, p.uid, r.state, r.next_at, r.current FROM rotation r \
            JOIN playlists p ON p.id = r.playlist_id ORDER BY r.output";
        rows(&self.db, sql, [], |r| {
            let (output, playlist, next_at, current) = (r.get(0)?, r.get(1)?, r.get(3)?, r.get(4)?);
            Ok(Rotation { output, playlist, state: json(r, 2)?, next_at, current })
        })
    }

    pub fn rotation(&self, output: &str) -> Result<Option<Rotation>> {
        Ok(self.rotations()?.into_iter().find(|rotation| rotation.output == output))
    }

    pub fn clear_rotation(&mut self, output: &str) -> Result<()> {
        self.db.execute("DELETE FROM rotation WHERE output = ?1", [output])?;
        Ok(())
    }

    pub fn mark_used(&mut self, uid: &str, output: &str, now: i64) -> Result<()> {
        let tx = self.db.transaction()?;
        let id = row_id(&tx, "item", uid)?;
        tx.execute(
            "UPDATE items SET used_at = ?2, use_count = use_count + 1 WHERE id = ?1",
            params![id, now],
        )?;
        tx.execute("INSERT INTO history(at, output, item_id) VALUES (?1, ?2, ?3)", params![now, output, id])?;
        let trim = "DELETE FROM history WHERE rowid IN (SELECT rowid FROM history ORDER BY rowid DESC LIMIT -1 OFFSET ?1)";
        tx.execute(trim, [HISTORY])?;
        Ok(tx.commit()?)
    }

    pub fn history(&self, output: &str, limit: u32) -> Result<Vec<Used>> {
        let sql = "SELECT h.at, i.uid FROM history h LEFT JOIN items i ON i.id = h.item_id WHERE h.output = ?1 \
            ORDER BY h.rowid DESC LIMIT ?2";
        rows(&self.db, sql, params![output, limit], |row| Ok(Used { at: row.get(0)?, item: row.get(1)? }))
    }
}
