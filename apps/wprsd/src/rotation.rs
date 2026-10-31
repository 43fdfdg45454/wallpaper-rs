use crate::compositor::lock;
use crate::daemon::{Daemon, State};
use crate::error::Result;
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, RandomState};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Notify;
use wprs_core::model::{OutputPlaylist, OutputState};
use wprs_library::{Library, PlaylistInfo, Resolved, Rotation};

mod sync;

#[derive(Debug, Clone, PartialEq)]
pub struct Playing {
    pub playlist: OutputPlaylist,
    pub item: Option<String>,
    pub path: Option<PathBuf>,
    pub error: Option<String>,
    pub transition: Option<Duration>,
}

type Lists = HashMap<String, Vec<String>>;

#[derive(Default)]
pub struct Rotations {
    pub(crate) lists: Mutex<Lists>,
    pub(crate) unsaved: Mutex<Vec<Rotation>>,
    due: Mutex<Option<i64>>,
    retime: AtomicBool,
    pub wake: Notify,
}

#[derive(Clone, Copy)]
pub enum Act<'a> {
    Check,
    Start(&'a str, &'a str),
    Step(&'a [String], bool),
}

type Found = HashMap<String, Result<(PlaylistInfo, Resolved), String>>;

pub fn seed() -> u32 {
    RandomState::new().hash_one(std::time::SystemTime::now()) as u32
}

pub fn excluded(lib: &Library, revoked: &[PathBuf]) -> HashSet<String> {
    let flashing = lib.unconfirmed_flashing().unwrap_or_default();
    revoked.iter().filter_map(|p| lib.resolve_item(p.to_str()?).ok()).chain(flashing).collect()
}

pub fn overlay(st: &State, output: &str, shown: &mut OutputState) {
    let playlist = st.config.resolve(output).playlist;
    let Some(p) = st.playing.get(output).filter(|p| playlist.as_ref() == Some(&p.playlist.uid)) else {
        return;
    };
    (shown.image, shown.item, shown.playlist) = (p.path.clone(), p.item.clone(), Some(p.playlist.clone()));
    shown.error = shown.error.take().or_else(|| p.error.clone());
}

fn unresolved(uid: &str, error: &str) -> Playing {
    let playlist =
        OutputPlaylist { uid: uid.into(), name: String::new(), position: 0, count: 0, next_at: None };
    Playing { playlist, item: None, path: None, error: Some(error.into()), transition: None }
}

pub fn newest(stored: Vec<Rotation>, unsaved: &[Rotation]) -> Vec<Rotation> {
    let same = |a: &Rotation, b: &Rotation| a.output == b.output && a.playlist == b.playlist;
    let kept = stored.into_iter().filter(|r| !unsaved.iter().any(|u| same(r, u)));
    kept.chain(unsaved.iter().cloned()).collect()
}

impl Rotations {
    pub fn retime(&self) {
        self.retime.store(true, Ordering::Relaxed);
        self.wake.notify_one();
    }
}

impl State {
    pub fn assigned(&self) -> Vec<(String, String, bool)> {
        let playlist = |name: &String| self.config.resolve(name).playlist;
        let found = |(name, info): (&String, _)| Some((name.clone(), playlist(name)?, self.active(info)));
        self.outputs
            .iter()
            .filter(|(name, _)| !crate::drafts::virtual_output(name))
            .filter_map(found)
            .collect()
    }
}

impl Daemon {
    pub fn revoked(&self) -> Vec<PathBuf> {
        let st = self.state();
        st.installed.iter().filter(|i| i.revoked.is_some()).map(|i| i.path.clone()).collect()
    }

    pub fn fingerprint(&self) -> (Vec<(String, String, bool)>, bool) {
        let st = self.state();
        (st.assigned(), st.on_battery)
    }

    pub fn playlist_media(
        &self,
        output: &str,
        playlist: Option<String>,
    ) -> Option<(Option<PathBuf>, Option<Duration>)> {
        let Some(uid) = playlist else {
            self.state().playing.remove(output);
            return None;
        };
        let find = || {
            let st = self.state();
            st.playing.get(output).filter(|p| p.playlist.uid == uid).map(|p| (p.path.clone(), p.transition))
        };
        if let Some(found) = find() {
            return Some(found);
        }
        let (changed, _) = self.sync(Act::Check);
        changed.iter().filter(|o| *o != output).for_each(|o| drop(self.configure_output(o)));
        Some(find().unwrap_or_default())
    }

    pub fn show(&self, act: Act<'_>) -> Option<i64> {
        self.show_after(act, || Ok(())).unwrap_or_default()
    }

    pub fn show_after(&self, act: Act<'_>, before: impl FnOnce() -> Result<()>) -> Result<Option<i64>> {
        let (changed, due) = {
            let mut lists = lock(&self.rotations.lists);
            before()?;
            self.synced(&mut lists, act)
        };
        changed.iter().for_each(|o| drop(self.configure_output(o)));
        Ok(due)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wprs_schedule::PlaylistState;

    #[test]
    fn a_position_that_could_not_be_saved_is_used_instead_of_the_stored_one() {
        let row = |output: &str, next_at: i64| Rotation {
            output: output.into(),
            playlist: "P".into(),
            state: PlaylistState::new(1),
            next_at: Some(next_at),
            current: None,
        };
        let stored = vec![row("DP-1", 100), row("DP-2", 100)];
        let merged = newest(stored, &[row("DP-1", 200), row("HDMI-A-1", 300)]);
        let seen: Vec<(&str, Option<i64>)> = merged.iter().map(|r| (r.output.as_str(), r.next_at)).collect();
        assert_eq!(seen, [("DP-2", Some(100)), ("DP-1", Some(200)), ("HDMI-A-1", Some(300))]);
    }
}
