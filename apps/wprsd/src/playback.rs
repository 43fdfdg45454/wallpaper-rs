use crate::apply::target_output;
use crate::daemon::Daemon;
use crate::error::{Error, Result, failed, invalid, not_found};
use crate::items::reply;
use crate::jobs::now;
use crate::rotation::{Act, Playing, excluded};
use std::collections::HashMap;
use wprs_core::Config;
use wprs_core::model::{HistoryEntry, HistoryReply, MODEL};

mod turns;

pub use turns::run;

pub fn stop_sections(
    c: &mut Config,
    playing: &HashMap<String, Playing>,
    output: &str,
    only: Option<&str>,
) -> Vec<String> {
    let hit = |p: Option<&String>| p.is_some_and(|p| only.is_none_or(|u| u == p));
    let (mut stopped, mut fallback) = (Vec::new(), None);
    for (name, p) in playing.iter().filter(|(o, _)| output.is_empty() || *o == output) {
        if !hit(Some(&p.playlist.uid)) || c.resolve(name).playlist.as_ref() != Some(&p.playlist.uid) {
            continue;
        }
        let own = c.outputs.get(name).is_some_and(|s| s.playlist.is_some() || s.image.is_some());
        fallback = fallback.or(p.path.clone().filter(|_| !own));
        let section = c.outputs.entry(name.clone()).or_default();
        (section.image, section.playlist) = (p.path.clone(), None);
        stopped.push(p.playlist.uid.clone());
    }
    let sections =
        std::iter::once(("", &mut c.default)).chain(c.outputs.iter_mut().map(|(n, s)| (n.as_str(), s)));
    for (name, section) in
        sections.filter(|(n, s)| hit(s.playlist.as_ref()) && (output.is_empty() || *n == output))
    {
        stopped.extend(section.playlist.take());
        if name.is_empty() {
            section.image = fallback.clone();
        }
    }
    stopped.sort();
    stopped.dedup();
    stopped
}

impl Daemon {
    pub fn start(&self, key: &str, output: &str) -> Result<()> {
        let (output, revoked, on_battery) = (target_output(output)?, self.revoked(), self.state().on_battery);
        let resolved =
            self.store.read(|lib| lib.resolve(key, now(), on_battery, &excluded(lib, &revoked)))?;
        if resolved.uids.is_empty() {
            return Err(failed("the playlist has no items to show"));
        }
        let uid = resolved.playlist.id;
        let assign = || {
            let assigned = self.update_config(|c| {
                c.set_playlist(&output, uid.clone());
                Ok(())
            });
            assigned.map_err(Error::from)
        };
        self.show_after(Act::Start(&uid, &output), assign)?;
        self.changes.changed("playlists", vec![uid]);
        Ok(())
    }

    pub fn stop(&self, output: &str) -> Result<()> {
        let (output, playing) = (target_output(output)?, self.state().playing.clone());
        let mut stopped = Vec::new();
        self.update_config(|c| {
            stopped = stop_sections(c, &playing, &output, None);
            Ok(())
        })?;
        self.configure_all();
        self.changes.changed("playlists", stopped);
        Ok(())
    }

    pub fn step(&self, output: &str, forward: bool) -> Result<()> {
        let output = target_output(output)?;
        let assigned = self.state().assigned().into_iter().map(|(o, ..)| o);
        let outputs: Vec<String> = assigned.filter(|o| output.is_empty() || *o == output).collect();
        if outputs.is_empty() && !output.is_empty() {
            return Err(not_found(format!("{output} has no playlist")));
        }
        self.show(Act::Step(&outputs, forward));
        Ok(())
    }

    pub fn history(&self, output: &str, limit: u32) -> Result<String> {
        let (output, serial) = (target_output(output)?, self.changes.serial());
        if output.is_empty() || !(1..=1000).contains(&limit) {
            return Err(invalid("give a connector name and a limit of 1 to 1000"));
        }
        let used = self.store.read(|lib| {
            let entries = lib.history(&output, limit)?.into_iter();
            Ok::<_, Error>(
                entries.map(|u| (u.at, u.item.and_then(|uid| lib.item(&uid).ok()))).collect::<Vec<_>>(),
            )
        })?;
        let entry = |(at, item): (i64, Option<_>)| {
            Ok(HistoryEntry { at, item: item.map(|i| self.summary(i)).transpose()? })
        };
        let entries = used.into_iter().map(entry).collect::<Result<_>>()?;
        reply(HistoryReply { model: MODEL, serial, output, entries })
    }
}

#[cfg(test)]
mod tests;
