use super::{Act, Found, Lists, Playing, excluded, newest, seed, unresolved};
use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::Error;
use crate::timing::{Turn, rotate};
use crate::zone::Zone;
use chrono::Utc;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::Duration;
use wprs_core::model::OutputPlaylist;
use wprs_library::Rotation;
use wprs_schedule::PlaylistState;

impl Daemon {
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

    pub(super) fn sync(&self, act: Act<'_>) -> (Vec<String>, Option<i64>) {
        self.synced(&mut lock(&self.rotations.lists), act)
    }

    pub(super) fn synced(&self, lists: &mut Lists, act: Act<'_>) -> (Vec<String>, Option<i64>) {
        let (now, retime) = (Utc::now(), self.rotations.retime.swap(false, Ordering::Relaxed));
        let ((assigned, on_battery), before) = (self.fingerprint(), self.state().playing.clone());
        let (stored, found) = self.read(&assigned, on_battery, now.timestamp());
        let rows = newest(stored.clone(), &lock(&self.rotations.unsaved));
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
            let ours = |r: &&Rotation| r.output == key && r.playlist == uid;
            let (saved, kept) = (stored.iter().find(ours), rows.iter().find(ours).cloned());
            let mut row = kept.unwrap_or_else(|| {
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
            let turn = Turn { live, fresh: !lists.contains_key(&key), start, step, retime };
            let last = lists.get(&key).map(Vec::as_slice);
            let switched = rotate(&mut row, last, resolved, &info.rules, turn, now, &Zone);
            lists.insert(key, resolved.uids.clone());
            if saved != Some(&row) {
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
            if let Err(e) = &saved {
                tracing::warn!("cannot save the playlist position; it is kept until it can be saved: {e}");
            }
            *lock(&self.rotations.unsaved) = if saved.is_ok() { Vec::new() } else { saves };
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
