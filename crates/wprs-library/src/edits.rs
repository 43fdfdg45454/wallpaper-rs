use crate::db::{ensure, item_ids, label, remove, row_id, rows};
use crate::items::{Item, SELECT_ITEMS, item_in, item_row};
use crate::tags::{TAG_LENGTH, tag_item, untag_item};
use crate::{Error, Library, Result};
use rusqlite::types::ToSql;
use rusqlite::{OptionalExtension, params};
use std::collections::HashSet;

fn tags<'a>(tags: &[&'a str]) -> Result<Vec<&'a str>> {
    tags.iter().map(|tag| label("tag", tag, TAG_LENGTH)).collect()
}

impl Library {
    pub fn set_title(&mut self, uid: &str, title: &str) -> Result<()> {
        self.update(&[uid], "title = ?2", &label("title", title, 256)?)
    }

    pub fn set_rating(&mut self, uid: &str, rating: u8) -> Result<()> {
        self.set_rating_many(&[uid], rating)
    }

    pub fn set_favorite(&mut self, uid: &str, favorite: bool) -> Result<()> {
        self.set_favorite_many(&[uid], favorite)
    }

    pub fn set_rating_many(&mut self, uids: &[&str], rating: u8) -> Result<()> {
        ensure(rating <= 5, || Error::Invalid("rating", format!("{rating} is not between 0 and 5")))?;
        self.update(uids, "rating = ?2", &rating)
    }

    pub fn set_favorite_many(&mut self, uids: &[&str], favorite: bool) -> Result<()> {
        self.update(uids, "favorite = ?2", &favorite)
    }

    fn update(&mut self, uids: &[&str], set: &'static str, value: &dyn ToSql) -> Result<()> {
        let tx = self.db.transaction()?;
        for id in item_ids(&tx, uids)? {
            tx.execute(&format!("UPDATE items SET {set} WHERE id = ?1"), params![id, value])?;
        }
        Ok(tx.commit()?)
    }

    pub fn retag(&mut self, uids: &[&str], add: &[&str], remove: &[&str]) -> Result<()> {
        let (add, remove) = (tags(add)?, tags(remove)?);
        let tx = self.db.transaction()?;
        for id in item_ids(&tx, uids)? {
            for tag in &add {
                tag_item(&tx, id, tag)?;
            }
            for tag in &remove {
                untag_item(&tx, id, tag)?;
            }
        }
        Ok(tx.commit()?)
    }

    pub fn rename_tag(&mut self, from: &str, to: &str) -> Result<u32> {
        let to = label("tag", to, TAG_LENGTH)?;
        let tx = self.db.transaction()?;
        let find = |name: &str| {
            tx.query_row("SELECT id FROM tags WHERE name = ?1", [name], |r| r.get::<_, i64>(0)).optional()
        };
        let id = find(from.trim())?.ok_or_else(|| Error::NotFound("tag", from.into()))?;
        let found = find(to)?;
        let target = found.filter(|other| *other != id);
        let items: Vec<i64> =
            rows(&tx, "SELECT item_id FROM item_tags WHERE tag_id = ?1", [id], |r| r.get(0))?;
        if let Some(target) = target {
            tx.execute("UPDATE OR IGNORE item_tags SET tag_id = ?2 WHERE tag_id = ?1", [id, target])?;
            tx.execute("DELETE FROM tags WHERE id = ?1", [id])?;
        } else {
            tx.execute("UPDATE tags SET name = ?2 WHERE id = ?1", params![id, to])?;
        }
        let refused = "INSERT OR IGNORE INTO untagged(item_id, name) VALUES (?1, ?2)";
        for item in &items {
            tx.execute("UPDATE items SET title = title WHERE id = ?1", [item])?;
            tx.execute("DELETE FROM untagged WHERE item_id = ?1 AND name = ?2", params![item, to])?;
            if found != Some(id) {
                tx.execute(refused, params![item, from.trim()])?;
            }
        }
        tx.commit()?;
        Ok(items.len() as u32)
    }

    pub fn remove_items(&mut self, uids: &[&str]) -> Result<Vec<Item>> {
        let tx = self.db.transaction()?;
        let mut items = uids.iter().map(|uid| item_in(&tx, uid)).collect::<Result<Vec<_>>>()?;
        let mut seen = HashSet::new();
        items.retain(|item| seen.insert(item.uid.clone()));
        for item in &items {
            tx.execute("DELETE FROM items WHERE uid = ?1", [&item.uid])?;
        }
        tx.commit()?;
        Ok(items)
    }

    pub fn remove_item(&mut self, uid: &str) -> Result<()> {
        remove(&self.db, "item", uid)
    }

    pub fn remove_source(&mut self, uid: &str) -> Result<Vec<Item>> {
        let tx = self.db.transaction()?;
        let id = row_id(&tx, "source", uid)?;
        let items = rows(&tx, &format!("{SELECT_ITEMS} WHERE i.source_id = ?1"), [id], item_row)?;
        tx.execute("DELETE FROM sources WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(items)
    }
}
