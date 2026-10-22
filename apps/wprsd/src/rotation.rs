use crate::compositor::lock;
use crate::daemon::{Daemon, State};
use crate::error::Error;
use crate::timing::{Turn, rotate};
use chrono::{Local, Utc};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{BuildHasher, RandomState};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::Notify;
use wprs_core::model::{OutputPlaylist, OutputState};
use wprs_library::{Library, PlaylistInfo, Resolved, Rotation};
use wprs_schedule::PlaylistState;

#[derive(Debug, Clone, PartialEq)]
pub struct Playing {
    pub playlist: OutputPlaylist,
    pub item: Option<String>,
    pub path: Option<PathBuf>,
    pub error: Option<String>,
    pub transition: Option<Duration>,
}

#[derive(Default)]
pub struct Rotations {
    lists: Mutex<HashMap<String, Vec<String>>>,
    due: Mutex<Option<i64>>,
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
    revoked.iter().filter_map(|p| lib.resolve_item(p.to_str()?).ok()).collect()
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

impl Rotations {
    pub fn known(&self, keys: &[String]) {
        lock(&self.lists).extend(keys.iter().map(|key| (key.clone(), Vec::new())));
    }
}

impl State {
    pub fn assigned(&self) -> Vec<(String, String, bool)> {
        let playlist = |name: &String| self.config.resolve(name).playlist;
        let found = |(name, info): (&String, _)| Some((name.clone(), playlist(name)?, self.active(info)));
        self.outputs.iter().filter_map(found).collect()
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
        let (changed, due) = self.sync(act);
        changed.iter().for_each(|o| drop(self.configure_output(o)));
        due
    }

    fn read(
        &self,
        assigned: &[(String, String, bool)],
        on_battery: bool,
        now: i64,
    ) -> (Vec<Rotation>, Found) {
        let (revoked, uids) =
            (self.revoked(), assigned.iter().map(|(_, uid, _)| uid).collect::<HashSet<_>>());
        let read = self.store.read(|lib| {
            let exclude = excluded(lib, &revoked);
            let found = |uid: &str| {
                Ok::<_, Error>((lib.playlist(uid)?, lib.resolve(uid, now, on_battery, &exclude)?))
            };
            let found = uids.iter().map(|uid| (uid.to_string(), found(uid).map_err(|e| e.to_string())));
            Ok::<_, Error>((lib.rotations()?, found.collect()))
        });
        read.unwrap_or_else(|e| {
            (Vec::new(), uids.iter().map(|uid| (uid.to_string(), Err(e.to_string()))).collect())
        })
    }

    fn sync(&self, act: Act<'_>) -> (Vec<String>, Option<i64>) {
        let mut lists = lock(&self.rotations.lists);
        let now = Utc::now();
        let ((assigned, on_battery), before) = (self.fingerprint(), self.state().playing.clone());
        let (rows, found) = self.read(&assigned, on_battery, now.timestamp());
        let mut groups: BTreeMap<String, (String, Vec<(String, bool)>)> = BTreeMap::new();
        for (output, uid, active) in assigned {
            let synced = matches!(found.get(&uid), Some(Ok((info, _))) if info.rules.synced);
            let key = if synced { format!("@{uid}") } else { output.clone() };
            groups.entry(key).or_insert_with(|| (uid, Vec::new())).1.push((output, active));
        }
        let (mut playing, mut saves, mut used, mut due) = (HashMap::new(), Vec::new(), Vec::new(), None);
        for (key, (uid, members)) in groups {
            let (info, resolved) = match &found[&uid] {
                Ok(found) => found,
                Err(e) => {
                    playing.extend(members.into_iter().map(|(o, _)| (o, unresolved(&uid, e))));
                    continue;
                }
            };
            let stored = rows.iter().find(|r| r.output == key && r.playlist == uid).cloned();
            let mut row = stored.clone().unwrap_or_else(|| {
                let state = PlaylistState::new(seed());
                Rotation { output: key.clone(), playlist: uid.clone(), state, next_at: None, current: None }
            });
            let has = |outputs: &[String]| members.iter().any(|(o, _)| outputs.contains(o));
            let start = matches!(act, Act::Start(p, o) if p == uid && (o.is_empty() || has(&[o.to_owned()])));
            let step = match act {
                Act::Step(outputs, forward) if has(outputs) => Some(forward),
                _ => None,
            };
            let live = info.rules.advance_while_paused || members.iter().any(|(_, active)| *active);
            let turn = Turn { live, fresh: !lists.contains_key(&key), start, step };
            let last = lists.get(&key).map(Vec::as_slice);
            let switched = rotate(&mut row, last, resolved, &info.rules, turn, now, &Local);
            lists.insert(key, resolved.uids.clone());
            if stored.as_ref() != Some(&row) {
                saves.push(row.clone());
            }
            due = due.into_iter().chain(row.next_at.filter(|_| live && !resolved.uids.is_empty())).min();
            let path = row.state.current(&resolved.playlist).map(Path::to_path_buf);
            let transition = info.rules.transition_ms.map(|ms| Duration::from_millis(ms.into()));
            for (output, _) in members {
                let kept = before.get(&output).filter(|p| p.playlist.uid == uid && p.path.is_some());
                let (item, path, error) = match (&path, kept) {
                    (Some(path), _) => (row.current.clone(), Some(path.clone()), None),
                    (None, Some(p)) => (p.item.clone(), p.path.clone(), None),
                    (None, None) => (None, None, Some("the playlist has no items to show".to_owned())),
                };
                let previous = before.get(&output).map(|p| &p.item);
                if start || (previous != Some(&item) && (switched || previous.is_some())) {
                    used.extend(item.clone().map(|item| (item, output.clone())));
                }
                let (position, count) = (row.state.position(), resolved.uids.len() as u32);
                let playlist = OutputPlaylist {
                    uid: uid.clone(),
                    name: info.name.clone(),
                    position,
                    count,
                    next_at: row.next_at,
                };
                playing.insert(output, Playing { playlist, item, path, error, transition });
            }
        }
        if std::mem::replace(&mut *lock(&self.rotations.due), due) != due {
            self.rotations.wake.notify_one();
        }
        if !saves.is_empty() || !used.is_empty() {
            let saved = self.store.write(|lib| {
                saves.iter().try_for_each(|row| lib.save_rotation(row))?;
                used.iter().try_for_each(|(item, output)| lib.mark_used(item, output, now.timestamp()))
            });
            saved.unwrap_or_else(|e| tracing::warn!("cannot save the playlist position: {e}"));
        }
        let moved = |o: &String| {
            before.get(o).map(|p| (&p.path, p.transition)) != Some((&playing[o].path, playing[o].transition))
        };
        let changed: Vec<String> = playing.keys().filter(|o| moved(o)).cloned().collect();
        let outputs: HashSet<&String> = before.keys().chain(playing.keys()).collect();
        let touched: Vec<String> =
            outputs.into_iter().filter(|o| before.get(*o) != playing.get(*o)).cloned().collect();
        self.state().playing = playing;
        if !touched.is_empty() {
            self.changes.changed("state", touched);
        }
        if !used.is_empty() {
            self.changes.changed("library", used.into_iter().map(|(item, _)| item).collect());
        }
        (changed, due)
    }
}
