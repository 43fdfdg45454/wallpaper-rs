use crate::daemon::Daemon;
use crate::jobs::now;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn shipped() -> BTreeSet<PathBuf> {
    let system = std::env::var_os("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let dirs = dirs::data_dir().into_iter().chain(std::env::split_paths(&system));
    let entries = dirs.flat_map(|d| std::fs::read_dir(d.join("wallpaper-rs/examples")).into_iter().flatten());
    let files = entries.flatten().map(|e| e.path());
    files.filter(|p| p.extension().is_some_and(|e| e == "wpak") && p.is_file()).collect()
}

impl Daemon {
    pub fn offer_examples(&self) -> bool {
        let record = self.store.data.join("examples.json");
        let read = std::fs::read(&record).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let (offered, found): (BTreeSet<PathBuf>, _) = (read.unwrap_or_default(), shipped());
        if offered == found {
            return false;
        }
        let changed = self.store.write(|lib| {
            let gone: Vec<String> =
                offered.difference(&found).filter_map(|p| lib.resolve_item(p.to_str()?).ok()).collect();
            gone.iter().try_for_each(|uid| lib.remove_item(uid))?;
            let added: Vec<String> =
                found.difference(&offered).filter_map(|p| lib.import_file(p, now()).ok()).collect();
            Ok::<_, wprs_library::Error>((gone, added))
        });
        let Ok((gone, added)) = changed else { return false };
        if let Err(e) = crate::inputs::save(&record, &found) {
            tracing::warn!(error = %e.kind(), "cannot record the example scenes offered");
            tracing::debug!("{e}");
        }
        tracing::info!(added = added.len(), removed = gone.len(), "example scenes offered in the library");
        let any = !added.is_empty();
        self.library_changed(gone.into_iter().chain(added).collect());
        any
    }
}
