use crate::db::{ensure, item_ids, row_id, rows};
use crate::{Error, Library, Result};
use rusqlite::{Connection, params};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryEdit<'a> {
    Remove(&'a [(u32, &'a str)]),
    Move { from: u32, to: u32, expected: &'a str },
}

fn within(len: usize, positions: &[usize]) -> Result<()> {
    match positions.iter().find(|position| **position >= len) {
        Some(position) => Err(Error::Invalid("position", format!("{position} is not below {len}"))),
        None => Ok(()),
    }
}

impl Library {
    pub fn add_to_playlist(&mut self, key: &str, items: &[&str]) -> Result<()> {
        self.edit(key, |db, ids| {
            ids.extend(item_ids(db, items)?);
            Ok(())
        })
    }

    pub fn remove_from_playlist(&mut self, key: &str, item: &str) -> Result<()> {
        self.remove_from_playlist_many(key, &[item])
    }

    pub fn remove_from_playlist_many(&mut self, key: &str, items: &[&str]) -> Result<()> {
        self.edit(key, |db, ids| {
            let removed = item_ids(db, items)?;
            ids.retain(|id| !removed.contains(id));
            Ok(())
        })
    }

    pub fn move_in_playlist(&mut self, key: &str, from: usize, to: usize) -> Result<()> {
        self.edit(key, |_, ids| {
            within(ids.len(), &[from, to])?;
            let id = ids.remove(from);
            ids.insert(to, id);
            Ok(())
        })
    }

    pub fn edit_entries(&mut self, playlist: &str, edit: EntryEdit<'_>) -> Result<()> {
        self.edit(playlist, |db, ids| {
            let holds = |index: usize, expected: &str| -> Result<()> {
                let sql = "SELECT uid = ?2 FROM items WHERE id = ?1";
                let same: bool = db.query_row(sql, params![ids[index], expected], |r| r.get(0))?;
                ensure(same, || Error::Conflict)
            };
            match edit {
                EntryEdit::Remove(entries) => {
                    let mut indexes: Vec<usize> = entries.iter().map(|(index, _)| *index as usize).collect();
                    indexes.sort_unstable();
                    indexes.dedup();
                    within(ids.len(), &indexes)?;
                    let repeated = || Error::Invalid("position", "a position is given more than once".into());
                    ensure(indexes.len() == entries.len(), repeated)?;
                    entries.iter().try_for_each(|(index, expected)| holds(*index as usize, expected))?;
                    for index in indexes.into_iter().rev() {
                        ids.remove(index);
                    }
                }
                EntryEdit::Move { from, to, expected } => {
                    let (from, to) = (from as usize, to as usize);
                    within(ids.len(), &[from, to])?;
                    holds(from, expected)?;
                    let id = ids.remove(from);
                    ids.insert(to, id);
                }
            }
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
