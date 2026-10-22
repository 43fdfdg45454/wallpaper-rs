use crate::db::{ensure, json, label, remove, row_id, rows, uid, unique};
use crate::{Error, Library, Result};
use rusqlite::{Row, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SELECT_PROFILES: &str = "SELECT uid, name, outputs, options, auto, priority FROM profiles";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub uid: String,
    pub item: Option<String>,
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub uid: String,
    pub name: String,
    pub outputs: Value,
    pub options: Value,
    pub auto: Value,
    pub priority: i32,
}

fn profile_row(row: &Row<'_>) -> rusqlite::Result<Profile> {
    let (uid, name, priority) = (row.get(0)?, row.get(1)?, row.get(5)?);
    Ok(Profile { uid, name, outputs: json(row, 2)?, options: json(row, 3)?, auto: json(row, 4)?, priority })
}

impl Library {
    pub fn create_preset(&mut self, item: Option<&str>, name: &str, value: &Value) -> Result<String> {
        let name = label("name", name, 128)?;
        let item = item.map(|uid| row_id(&self.db, "item", uid)).transpose()?;
        let uid = uid();
        let sql = "INSERT INTO presets(uid, item_id, name, value) VALUES (?1, ?2, ?3, ?4)";
        unique(self.db.execute(sql, params![uid, item, name, value.to_string()]), "preset", name)?;
        Ok(uid)
    }

    pub fn presets(&self, item: Option<&str>) -> Result<Vec<Preset>> {
        let item = item.map(|uid| row_id(&self.db, "item", uid)).transpose()?;
        let sql = "SELECT p.uid, i.uid, p.name, p.value FROM presets p LEFT JOIN items i ON i.id = p.item_id \
            WHERE p.item_id IS ?1 ORDER BY p.name";
        rows(&self.db, sql, [item], |r| {
            Ok(Preset { uid: r.get(0)?, item: r.get(1)?, name: r.get(2)?, value: json(r, 3)? })
        })
    }

    pub fn preset(&self, uid: &str) -> Result<Preset> {
        let sql = "SELECT p.uid, i.uid, p.name, p.value FROM presets p LEFT JOIN items i ON i.id = p.item_id \
            WHERE p.id = ?1";
        let row =
            |r: &Row<'_>| Ok(Preset { uid: r.get(0)?, item: r.get(1)?, name: r.get(2)?, value: json(r, 3)? });
        Ok(self.db.query_row(sql, [row_id(&self.db, "preset", uid)?], row)?)
    }

    pub fn update_preset(&mut self, uid: &str, value: &Value) -> Result<()> {
        let changed = self
            .db
            .execute("UPDATE presets SET value = ?2 WHERE uid = ?1", params![uid, value.to_string()])?;
        ensure(changed == 1, || Error::NotFound("preset", uid.into()))
    }

    pub fn remove_preset(&mut self, uid: &str) -> Result<()> {
        remove(&self.db, "preset", uid)
    }

    pub fn create_profile(
        &mut self,
        name: &str,
        outputs: &Value,
        options: &Value,
        auto: &Value,
        priority: i32,
    ) -> Result<String> {
        let name = label("name", name, 128)?;
        let uid = uid();
        let sql = "INSERT INTO profiles(uid, name, outputs, options, auto, priority) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
        let values = params![uid, name, outputs.to_string(), options.to_string(), auto.to_string(), priority];
        unique(self.db.execute(sql, values), "profile", name)?;
        Ok(uid)
    }

    pub fn update_profile(&mut self, profile: &Profile) -> Result<()> {
        let Profile { uid, name, outputs, options, auto, priority } = profile;
        let name = label("name", name, 128)?;
        let sql = "UPDATE profiles SET name = ?2, outputs = ?3, options = ?4, auto = ?5, priority = ?6 WHERE uid = ?1";
        let values = params![uid, name, outputs.to_string(), options.to_string(), auto.to_string(), priority];
        let changed = unique(self.db.execute(sql, values), "profile", name)?;
        ensure(changed == 1, || Error::NotFound("profile", uid.clone()))
    }

    pub fn profiles(&self) -> Result<Vec<Profile>> {
        rows(&self.db, &format!("{SELECT_PROFILES} ORDER BY priority DESC, name"), [], profile_row)
    }

    pub fn profile(&self, key: &str) -> Result<Profile> {
        let id = row_id(&self.db, "profile", key)?;
        Ok(self.db.query_row(&format!("{SELECT_PROFILES} WHERE id = ?1"), [id], profile_row)?)
    }

    pub fn remove_profile(&mut self, key: &str) -> Result<()> {
        remove(&self.db, "profile", key)
    }
}
