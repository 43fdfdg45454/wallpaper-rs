use super::{MAX_UIDS, limited};
use crate::daemon::Daemon;
use crate::error::{Result, invalid, not_found};
use wprs_core::dbus::uri_scheme;
use wprs_library::Library;

impl Daemon {
    pub fn resolve(&self, refs: &[String]) -> Result<Vec<String>> {
        limited(refs, MAX_UIDS)?;
        let found = |lib: &Library, r: &String| match lib.resolve_item(r) {
            _ if uri_scheme(r).is_some() => Err(invalid(format!("{r} is a URI; give a local path"))),
            Err(wprs_library::Error::NotFound(..)) => Err(not_found(format!("no item matches {r:?}"))),
            Err(wprs_library::Error::Invalid(_, message)) => Err(invalid(message)),
            other => Ok(other?),
        };
        self.store.read(|lib| refs.iter().map(|r| found(lib, r)).collect::<Result<Vec<_>>>())
    }
}
