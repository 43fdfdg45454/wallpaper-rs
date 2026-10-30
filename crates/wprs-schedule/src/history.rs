use crate::playlist::{Order, Playlist};
use crate::state::PlaylistState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const KEPT: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Shown {
    path: PathBuf,
    cycle: u32,
    position: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anchor: Option<(u32, usize)>,
}

impl PlaylistState {
    pub(crate) fn travel<'a>(
        &mut self,
        playlist: &'a Playlist,
        forward: bool,
        now: DateTime<Utc>,
    ) -> Option<Option<&'a Path>> {
        (playlist.order == Order::Shuffle).then_some(())?;
        let here = self.current(playlist).map(|path| Shown {
            path: path.to_path_buf(),
            cycle: self.cycle,
            position: self.position,
            anchor: self.anchor,
        });
        let (from, to) =
            if forward { (&mut self.ahead, &mut self.behind) } else { (&mut self.behind, &mut self.ahead) };
        let found = recall(from, playlist, here.as_ref().map(|h| h.path.as_path()));
        if found.is_some() || forward {
            to.extend(here);
            to.drain(..to.len().saturating_sub(KEPT));
        }
        let to = found?;
        let was = (self.cycle, self.position, self.anchor);
        (self.cycle, self.position, self.anchor) = (to.cycle, to.position, to.anchor);
        if self.current(playlist) != Some(to.path.as_path()) {
            (self.cycle, self.position, self.anchor) = was;
            let index = playlist.items.iter().position(|p| *p == to.path)?;
            self.seek(playlist, index);
        }
        self.since = Some(now);
        Some(self.current(playlist))
    }
}

fn recall(list: &mut Vec<Shown>, playlist: &Playlist, shown: Option<&Path>) -> Option<Shown> {
    while let Some(entry) = list.pop() {
        if Some(entry.path.as_path()) != shown && playlist.items.contains(&entry.path) {
            return Some(entry);
        }
    }
    None
}
