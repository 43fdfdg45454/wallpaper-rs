use crate::db::rows;
use crate::{Library, Result};

impl Library {
    pub fn recheck(&mut self, refused: &str) -> Result<Vec<String>> {
        let found = "SELECT uid FROM items WHERE kind = 'package' AND missing = 0 AND instr(error, ?1) > 0";
        let uids: Vec<String> = rows(&self.db, found, [refused], |r| r.get(0))?;
        if !uids.is_empty() {
            let unread = "UPDATE items SET pkg_read = NULL WHERE kind = 'package' AND instr(error, ?1) > 0";
            self.db.execute(unread, [refused])?;
            self.reread()?;
        }
        Ok(uids)
    }
}
