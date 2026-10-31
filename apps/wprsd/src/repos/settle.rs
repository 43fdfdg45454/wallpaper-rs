use crate::error::{Error, Result};
use std::collections::HashSet;
use std::path::Path;

pub(super) enum Settled<T> {
    Done(T),
    Stopped,
    Gone,
    Failed(Error),
}

pub(super) fn settle<T>(updated: Result<T>, gone: impl FnOnce() -> bool) -> Settled<T> {
    match updated {
        Ok(done) => Settled::Done(done),
        Err(Error::Cancelled) => Settled::Stopped,
        Err(_) if gone() => Settled::Gone,
        Err(e) => Settled::Failed(e),
    }
}

pub(super) fn vanished(removing: &HashSet<String>, data: &Path, id: &str) -> bool {
    removing.contains(id) || !data.join("repos").join(id).is_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn lost() -> Result<()> {
        Err(wprs_format::Error::NotFound("repository".into()).into())
    }

    #[test]
    fn a_repository_removed_during_its_refresh_leaves_no_error_behind() {
        let (dir, id) = (tempfile::tempdir().unwrap(), "a".repeat(64));
        let (data, mut removing) = (dir.path(), HashSet::new());
        fs::create_dir_all(data.join("repos").join(&id)).unwrap();
        let gone = |removing: &HashSet<String>| settle(lost(), || vanished(removing, data, &id));
        assert!(matches!(gone(&removing), Settled::Failed(Error::Named(..))));
        removing.insert(id.clone());
        assert!(matches!(gone(&removing), Settled::Gone));
        removing.clear();
        fs::remove_dir_all(data.join("repos").join(&id)).unwrap();
        assert!(matches!(gone(&removing), Settled::Gone));
        assert!(matches!(settle(Err::<(), _>(Error::Cancelled), || true), Settled::Stopped));
        assert!(matches!(settle(Ok(7), || unreachable!("asked whether it is gone")), Settled::Done(7)));
    }
}
