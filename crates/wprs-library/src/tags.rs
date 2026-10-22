use crate::db::{label, rows};
use crate::{Library, Result};
use rusqlite::{Connection, params};

pub(crate) const TAG_LENGTH: usize = 64;

pub(crate) fn tag_item(db: &Connection, item: i64, tag: &str) -> Result<()> {
    let tag = label("tag", tag, TAG_LENGTH)?;
    db.execute("INSERT INTO tags(name) VALUES (?1) ON CONFLICT DO NOTHING", [tag])?;
    let sql = "INSERT OR IGNORE INTO item_tags(item_id, tag_id) SELECT ?1, id FROM tags WHERE name = ?2";
    db.execute(sql, params![item, tag])?;
    Ok(())
}

impl Library {
    pub fn add_tag(&mut self, uid: &str, tag: &str) -> Result<()> {
        self.retag(&[uid], &[tag], &[])
    }

    pub fn remove_tag(&mut self, uid: &str, tag: &str) -> Result<()> {
        self.retag(&[uid], &[], &[tag])
    }

    pub fn tags(&self) -> Result<Vec<(String, u32)>> {
        let sql = "SELECT t.name, count(*) FROM tags t JOIN item_tags it ON it.tag_id = t.id GROUP BY t.id ORDER BY t.name";
        rows(&self.db, sql, [], |row| Ok((row.get(0)?, row.get(1)?)))
    }
}
