use super::set_rule;
use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::Result;
use crate::playback::stop_sections;
use crate::rotation::seed;
use wprs_library::Rotation;
use wprs_schedule::PlaylistState;

impl Daemon {
    pub fn set_playlist_rule(&self, key: &str, rule: &str, value: &str) -> Result<()> {
        let showing = self.showing();
        self.edit_playlist(key, |lib, mut info| {
            let rules = set_rule(&info.rules, rule, value)?;
            lock(&self.rotations.unsaved).retain(|r| r.playlist != info.uid);
            let (migrate, reorder) = (rules.synced != info.rules.synced, rules.order != info.rules.order);
            info.rules = rules;
            lib.update_playlist(&info)?;
            let rows: Vec<Rotation> =
                lib.rotations()?.into_iter().filter(|r| r.playlist == info.uid).collect();
            let (current, mut keys) = (rows.iter().find_map(|r| r.current.clone()), Vec::new());
            for mut row in rows {
                row.next_at = None;
                match migrate {
                    true => lib.clear_rotation(&row.output)?,
                    false => lib.save_rotation(&row)?,
                }
                keys.extend((!migrate && reorder).then_some(row.output));
            }
            let added = match (migrate, info.rules.synced) {
                (false, _) => Vec::new(),
                (true, true) => vec![format!("@{}", info.uid)],
                (true, false) => showing.get(&info.uid).cloned().unwrap_or_default(),
            };
            for key in &added {
                let (state, current) = (PlaylistState::new(seed()), current.clone());
                let row = Rotation {
                    output: key.clone(),
                    playlist: info.uid.clone(),
                    state,
                    next_at: None,
                    current,
                };
                lib.save_rotation(&row)?;
            }
            keys.extend(added);
            Ok(keys)
        })
    }

    pub fn delete_playlist(&self, key: &str) -> Result<()> {
        let uid = self.store.read(|lib| lib.playlist(key))?.uid;
        let playing = self.state().playing.clone();
        self.store.write(|lib| lib.remove_playlist(&uid))?;
        self.update_config(|c| {
            stop_sections(c, &playing, "", Some(&uid));
            Ok(())
        })?;
        self.configure_all();
        self.changes.changed("playlists", vec![uid]);
        Ok(())
    }
}
