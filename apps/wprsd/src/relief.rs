use crate::compositor::Depth;
use crate::daemon::Daemon;
use std::path::Path;
use std::sync::{Arc, Weak};
use std::time::SystemTime;
use wprs_core::edit::{self, Edit, Source};
use wprs_depth::Places;
use wprs_render::DepthMap;

const MAX_EDGE: u32 = 1024;

pub(crate) type Cached = (Option<SystemTime>, edit::Depth, Weak<DepthMap>);

impl Daemon {
    pub(crate) fn depth_for(&self, file: &Path, edit: &Edit) -> Option<Depth> {
        let settings = edit.depth.clone().unwrap_or_default();
        let sha = match edit.source.sha256.as_str() {
            "" => Source::read(file, [0; 2]).ok()?.sha256,
            sha => sha.to_owned(),
        };
        let places = Places { data: self.store.data.clone(), cache: self.store.cache.clone() };
        let found = places.find(file, &sha, &settings)?;
        let modified = std::fs::metadata(&found).and_then(|m| m.modified()).ok();
        let cached = self.state().depths.get(&found).and_then(|(time, used, map)| {
            (*time == modified && *used == settings).then(|| map.upgrade()).flatten()
        });
        let map = match cached {
            Some(map) => map,
            None => {
                let loaded = places.map(file, &sha, &settings, MAX_EDGE).inspect_err(|e| {
                    tracing::warn!("cannot use a depth map");
                    tracing::debug!("{e:#}")
                });
                let map = Arc::new(loaded.ok()??);
                let mut st = self.state();
                st.depths.retain(|_, (.., weak)| weak.strong_count() > 0);
                st.depths.insert(found.clone(), (modified, settings, Arc::downgrade(&map)));
                map
            }
        };
        Some(Depth { file: found, modified, map })
    }
}
