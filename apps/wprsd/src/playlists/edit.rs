use super::normalized;
use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Error, Result, invalid};
use crate::items::{MAX_UIDS, limited};
use crate::rotation::Act;
use wprs_library::{EntryEdit, Library, PlaylistInfo};

impl Daemon {
    pub fn edit_playlist(
        &self,
        key: &str,
        change: impl FnOnce(&mut Library, PlaylistInfo) -> Result<Vec<String>>,
    ) -> Result<()> {
        let uid = {
            let mut lists = lock(&self.rotations.lists);
            let (uid, keys) = self.store.write(|lib| {
                let info = lib.playlist(key)?;
                Ok::<_, Error>((info.uid.clone(), change(lib, info)?))
            })?;
            lists.extend(keys.into_iter().map(|key| (key, Vec::new())));
            uid
        };
        self.changes.changed("playlists", vec![uid]);
        self.show(Act::Check);
        Ok(())
    }

    pub fn rename_playlist(&self, key: &str, name: &str) -> Result<()> {
        self.edit_playlist(key, |lib, info| {
            lib.update_playlist(&PlaylistInfo { name: name.into(), ..info })?;
            Ok(Vec::new())
        })
    }

    pub fn set_query(&self, key: &str, query: &str) -> Result<()> {
        self.edit_playlist(key, |lib, info| {
            lib.update_playlist(&PlaylistInfo { query: normalized(lib, query)?, ..info })?;
            Ok(Vec::new())
        })
    }

    pub fn edit_entries(
        &self,
        key: &str,
        edit: impl FnOnce(&mut Library, &str) -> wprs_library::Result<()>,
    ) -> Result<()> {
        self.edit_playlist(key, |lib, info| {
            edit(lib, &info.uid)?;
            Ok(Vec::new())
        })
    }

    pub fn remove_entries(&self, key: &str, indexes: &[u32], expected: &[String]) -> Result<()> {
        limited(indexes, MAX_UIDS)?;
        if indexes.len() != expected.len() {
            return Err(invalid("indexes and expected must have the same length"));
        }
        let entries: Vec<(u32, &str)> =
            indexes.iter().copied().zip(expected.iter().map(String::as_str)).collect();
        self.edit_entries(key, |lib, uid| lib.edit_entries(uid, EntryEdit::Remove(&entries)))
    }
}
