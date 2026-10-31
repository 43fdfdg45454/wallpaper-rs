use crate::daemon::Daemon;
use crate::error::{invalid, not_found};
use image::RgbaImage;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};
use wprs_core::Edit;

const PREVIEW: Duration = Duration::from_secs(60);
pub(crate) const MAX_PREVIEWS: usize = 4;

pub(crate) struct Preview {
    pub(crate) edit: Arc<Edit>,
    at: Instant,
    image: Option<Arc<RgbaImage>>,
}

pub(crate) fn keep(previews: &mut HashMap<PathBuf, Preview>, path: &Path, edit: Edit, at: Instant) -> bool {
    let edit = Arc::new(edit);
    if let Some(preview) = previews.get_mut(path) {
        (preview.edit, preview.at) = (edit, at);
        return false;
    }
    while previews.len() >= MAX_PREVIEWS {
        let oldest = previews.iter().min_by_key(|(_, p)| p.at).map(|(path, _)| path.clone());
        oldest.map(|path| previews.remove(&path));
    }
    previews.insert(path.to_owned(), Preview { edit, at, image: None });
    true
}

impl Daemon {
    pub fn preview_edit(self: &Arc<Self>, path: &Path, json: &str) -> anyhow::Result<()> {
        let path =
            std::fs::canonicalize(path).map_err(|_| not_found(format!("cannot find {}", path.display())))?;
        if json.len() > wprs_core::edit::MAX_BYTES {
            return Err(invalid("the edit is too large").into());
        }
        let edit =
            Some(json.trim()).filter(|j| !j.is_empty()).map(Edit::parse).transpose().map_err(invalid)?;
        let started = {
            let previews = &mut self.state().sidecars.previews;
            match edit {
                Some(edit) => keep(previews, &path, edit, Instant::now()),
                None => {
                    previews.remove(&path);
                    false
                }
            }
        };
        if started {
            tokio::spawn(expire(Arc::downgrade(self), path));
        }
        self.configure_all();
        Ok(())
    }

    pub(crate) fn keep_decoded(&self, file: &Path, image: &Arc<RgbaImage>) {
        if let Some(preview) = self.state().sidecars.previews.get_mut(file) {
            preview.image = Some(image.clone());
        }
    }
}

async fn expire(daemon: Weak<Daemon>, path: PathBuf) {
    while let Some(d) = daemon.upgrade() {
        let now = Instant::now();
        let due = {
            let mut st = d.state();
            let due = st.sidecars.previews.get(&path).map(|p| p.at + PREVIEW);
            due.filter(|due| *due > now).or_else(|| st.sidecars.previews.remove(&path).map(|_| now))
        };
        match due {
            None => return,
            Some(due) if due <= now => {
                tracing::debug!("an edit preview expired");
                return drop(tokio::task::spawn_blocking(move || d.configure_all()));
            }
            Some(due) => {
                drop(d);
                tokio::time::sleep_until(due.into()).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_PREVIEWS, keep};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    #[test]
    fn previews_are_bounded_and_the_oldest_goes_first() {
        let (mut previews, start) = (std::collections::HashMap::new(), Instant::now());
        let path = |i: usize| PathBuf::from(format!("/pictures/{i}.jpg"));
        for i in 0..MAX_PREVIEWS + 3 {
            assert!(keep(&mut previews, &path(i), Default::default(), start + Duration::from_secs(i as u64)));
        }
        assert!(!keep(&mut previews, &path(MAX_PREVIEWS + 2), Default::default(), start));
        assert_eq!(previews.len(), MAX_PREVIEWS);
        assert!((0..3).all(|i| !previews.contains_key(&path(i))));
    }
}
