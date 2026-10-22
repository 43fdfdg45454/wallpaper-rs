use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid};
use crate::items::{MAX_UIDS, limited, merged, reply};
use crate::jobs::now;
use crate::playback::stop_sections;
use crate::rotation::{Act, seed};
use serde_json::{Value, json};
use std::collections::HashMap;
use wprs_core::model::{MODEL, Playlist, PlaylistPage, PlaylistsReply};
use wprs_library::{EntryEdit, Library, PlaylistInfo, Rotation, Rules};
use wprs_schedule::PlaylistState;

const MAX_PAGE: u32 = 500;
const FLAGS: [&str; 4] = ["synced", "resume", "advance_while_paused", "skip_videos_on_battery"];

fn normalized(lib: &Library, query: &str) -> Result<Option<String>> {
    let query = query.trim();
    Ok((!query.is_empty()).then(|| lib.normalize_query(query, seed() % 2_147_483_646 + 1)).transpose()?)
}

fn model(lib: &Library, info: PlaylistInfo, showing: &HashMap<String, Vec<String>>) -> Result<Playlist> {
    let count = match &info.query {
        Some(query) => lib.count(query, now())?,
        None => lib.playlist_items(&info.uid, now())?.len() as u32,
    };
    let outputs = showing.get(&info.uid).cloned().unwrap_or_default();
    merged(info, json!({"count": count, "outputs": outputs}))
}

pub fn set_rule(rules: &Rules, key: &str, value: &str) -> Result<Rules> {
    let value = value.trim();
    let bad = || invalid(format!("invalid value {value:?} for the rule {key}"));
    let list = || Value::from(value.split(',').map(str::trim).filter(|v| !v.is_empty()).collect::<Vec<_>>());
    let text = match key {
        "order" | "interval" => json!(value),
        "at" | "days" => list(),
        "transition_ms" if value.is_empty() => Value::Null,
        "transition_ms" => json!(value.parse::<u32>().ok().filter(|ms| *ms <= 10_000).ok_or_else(bad)?),
        key if FLAGS.contains(&key) => json!(value.parse::<bool>().map_err(|_| bad())?),
        _ => return Err(invalid(format!("unknown rule {key:?}; the rules are listed in GetSchema().rules"))),
    };
    let mut json = serde_json::to_value(rules).map_err(|e| failed(e.to_string()))?;
    json[key] = text;
    let mut rules: Rules = serde_json::from_value(json).map_err(|_| bad())?;
    rules.at.sort();
    rules.at.dedup();
    rules.days.sort_by_key(|d| d.num_days_from_monday());
    rules.days.dedup();
    let seconds = rules.interval.as_secs();
    if (seconds != 0 && !(10..=30 * 86_400).contains(&seconds)) || rules.at.len() > 24 {
        return Err(bad());
    }
    Ok(rules)
}

impl Daemon {
    pub fn showing(&self) -> HashMap<String, Vec<String>> {
        let (mut showing, st) = (HashMap::<String, Vec<String>>::new(), self.state());
        for (output, p) in &st.playing {
            if st.config.resolve(output).playlist.as_ref() == Some(&p.playlist.uid) {
                showing.entry(p.playlist.uid.clone()).or_default().push(output.clone());
            }
        }
        showing.values_mut().for_each(|outputs| outputs.sort());
        showing
    }

    pub fn list_playlists(&self) -> Result<String> {
        let (serial, showing) = (self.changes.serial(), self.showing());
        let playlists =
            self.store.read(|lib| lib.playlists()?.into_iter().map(|p| model(lib, p, &showing)).collect())?;
        reply(PlaylistsReply { model: MODEL, serial, playlists })
    }

    pub fn get_playlist(&self, key: &str, offset: u32, limit: u32) -> Result<String> {
        if limit > MAX_PAGE {
            return Err(invalid(format!("limit must be at most {MAX_PAGE}")));
        }
        let (serial, showing) = (self.changes.serial(), self.showing());
        let (playlist, total, items) = self.store.read(|lib| {
            let info = lib.playlist(key)?;
            let (total, items) = match &info.query {
                Some(query) if limit == 0 => (lib.count(query, now())?, Vec::new()),
                Some(query) => (lib.count(query, now())?, lib.search(query, now(), offset, limit)?),
                None => {
                    let all = lib.playlist_items(&info.uid, now())?;
                    (all.len() as u32, all.into_iter().skip(offset as usize).take(limit as usize).collect())
                }
            };
            Ok::<_, Error>((model(lib, info, &showing)?, total, items))
        })?;
        let items = items.into_iter().map(|item| self.summary(item)).collect::<Result<_>>()?;
        reply(PlaylistPage { model: MODEL, serial, playlist, total, offset, items })
    }

    pub fn create_playlist(&self, name: &str, query: &str) -> Result<String> {
        let uid = self.store.write(|lib| {
            let query = normalized(lib, query)?;
            Ok::<_, Error>(lib.create_playlist(name, query.as_deref(), &Rules::default())?)
        })?;
        self.changes.changed("playlists", vec![uid.clone()]);
        Ok(uid)
    }

    pub fn edit_playlist(
        &self,
        key: &str,
        change: impl FnOnce(&mut Library, PlaylistInfo) -> Result<Vec<String>>,
    ) -> Result<()> {
        let (uid, keys) = self.store.write(|lib| {
            let info = lib.playlist(key)?;
            Ok::<_, Error>((info.uid.clone(), change(lib, info)?))
        })?;
        self.rotations.known(&keys);
        self.changes.changed("playlists", vec![uid]);
        self.show(Act::Check);
        Ok(())
    }

    pub fn rename_playlist(&self, key: &str, name: &str) -> Result<()> {
        self.edit_playlist(key, |lib, info| {
            lib.update_playlist(&PlaylistInfo { name: name.into(), ..info })?;
            Ok(Vec::new())
        })
    }

    pub fn set_query(&self, key: &str, query: &str) -> Result<()> {
        self.edit_playlist(key, |lib, info| {
            lib.update_playlist(&PlaylistInfo { query: normalized(lib, query)?, ..info })?;
            Ok(Vec::new())
        })
    }

    pub fn edit_entries(
        &self,
        key: &str,
        edit: impl FnOnce(&mut Library, &str) -> wprs_library::Result<()>,
    ) -> Result<()> {
        self.edit_playlist(key, |lib, info| {
            edit(lib, &info.uid)?;
            Ok(Vec::new())
        })
    }

    pub fn remove_entries(&self, key: &str, indexes: &[u32], expected: &[String]) -> Result<()> {
        limited(indexes, MAX_UIDS)?;
        if indexes.len() != expected.len() {
            return Err(invalid("indexes and expected must have the same length"));
        }
        let entries: Vec<(u32, &str)> =
            indexes.iter().copied().zip(expected.iter().map(String::as_str)).collect();
        self.edit_entries(key, |lib, uid| lib.edit_entries(uid, EntryEdit::Remove(&entries)))
    }

    pub fn set_playlist_rule(&self, key: &str, rule: &str, value: &str) -> Result<()> {
        let showing = self.showing();
        self.edit_playlist(key, |lib, mut info| {
            let rules = set_rule(&info.rules, rule, value)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_are_checked_against_the_library_rules() {
        let rules = set_rule(&Rules::default(), "at", "18:00, 07:30,18:00").unwrap();
        let rules = set_rule(&rules, "days", "sunday,Mon").unwrap();
        let json = serde_json::to_value(&rules).unwrap();
        assert_eq!((&json["at"], &json["days"]), (&json!(["07:30", "18:00"]), &json!(["Mon", "Sun"])));
        assert_eq!(set_rule(&rules, "interval", "1h30m").unwrap().interval.as_secs(), 5400);
        assert_eq!(set_rule(&rules, "transition_ms", "250").unwrap().transition_ms, Some(250));
        let bad = [("interval", "5s"), ("interval", "31d"), ("order", "random"), ("at", "25:00")];
        for (key, value) in
            bad.into_iter().chain([("synced", "yes"), ("transition_ms", "10001"), ("colour", "red")])
        {
            assert!(set_rule(&rules, key, value).is_err(), "{key}={value}");
        }
    }
}
