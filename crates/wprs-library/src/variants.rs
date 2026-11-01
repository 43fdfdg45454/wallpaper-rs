use crate::db::rows;
use crate::items::{Item, SELECT_ITEMS, item_row};
use crate::{Library, Result};

impl Library {
    pub fn variants(&self, base: &str) -> Result<Vec<Item>> {
        let sql = format!(
            "{SELECT_ITEMS} WHERE i.base = ?1 AND i.pkg_type = 'preset' AND i.missing = 0 ORDER BY i.title, i.id"
        );
        rows(&self.db, &sql, [base], item_row)
    }

    pub fn packages_of(&self, pkg_id: &str) -> Result<Vec<Item>> {
        let sql =
            format!("{SELECT_ITEMS} WHERE i.pkg_id = ?1 AND i.missing = 0 AND i.error IS NULL ORDER BY i.id");
        rows(&self.db, &sql, [pkg_id], item_row)
    }
}
