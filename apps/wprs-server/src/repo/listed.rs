use std::sync::PoisonError;

impl super::Repo {
    pub fn list(&self, id: &str, version: Option<&str>, listed: bool) -> wprs_format::Result<()> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        wprs_format::list(&self.dir, id, version, listed)
    }

    pub fn feature(&self, id: &str, note: Option<&str>, on: bool) -> wprs_format::Result<()> {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        wprs_format::feature(&self.dir, id, note, on)
    }
}
