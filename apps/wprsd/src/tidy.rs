use crate::daemon::Daemon;
use crate::error::Result;
use crate::jobs::now;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use wprs_library::Library;

impl Daemon {
    pub fn tidy(self: &Arc<Self>) {
        self.offer_examples();
        crate::compositor::stops::recheck(self);
        if self.store.connect().and_then(|mut lib| Ok(lib.reread()?)).is_ok_and(|n| n > 0) {
            self.library_changed(Vec::new());
        }
        self.thumbs.prune(u64::from(self.state().config.library.thumb_cache_mb) << 20);
        let packages = self.store.read(|lib| lib.search("kind:package", now(), 0, u32::MAX));
        let Ok(packages) = packages else { return };
        let kept: HashSet<PathBuf> = packages.iter().map(|i| self.blob(&i.path, i.mtime, i.size)).collect();
        let blobs = std::fs::read_dir(self.store.cache.join("blobs")).into_iter().flatten().flatten();
        let base = |e: &std::fs::DirEntry| {
            e.path().with_file_name(e.file_name().to_string_lossy().split('.').next().unwrap_or_default())
        };
        blobs.filter(|e| !kept.contains(&base(e))).for_each(|e| drop(std::fs::remove_file(e.path())));
        if self.store.read(|lib| lib.unanalyzed(1)).is_ok_and(|found| !found.is_empty()) {
            self.analyze_job();
        }
    }

    pub fn refresh_items(&self) {
        let paths: Vec<PathBuf> = {
            let st = self.state();
            let sections = std::iter::once(&st.config.default).chain(st.config.outputs.values());
            sections.filter_map(|s| s.image.clone()).collect()
        };
        let find = |lib: &Library| -> Result<HashMap<PathBuf, String>> {
            let uid = |p: &PathBuf| Some((p.clone(), lib.resolve_item(p.to_str()?).ok()?));
            Ok(paths.iter().filter_map(uid).collect())
        };
        let found = self.store.read(find).unwrap_or_default();
        let changed = {
            let mut st = self.state();
            std::mem::replace(&mut st.items, found) != st.items
        };
        if changed {
            self.changes.changed("state", Vec::new());
        }
    }
}
