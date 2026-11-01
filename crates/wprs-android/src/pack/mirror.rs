use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::SystemTime;
use wprs_format::Flip;
use wprs_library::{Item, Kind};

type Seen = (Option<SystemTime>, [bool; 2]);

static FLIPS: LazyLock<Mutex<HashMap<PathBuf, Seen>>> = LazyLock::new(Default::default);

pub(crate) fn flip(item: &Item) -> [bool; 2] {
    if item.kind != Kind::Package {
        return [false; 2];
    }
    let stamp = std::fs::metadata(&item.path).and_then(|m| m.modified()).ok();
    let mut flips = FLIPS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, flip)) = flips.get(&item.path).filter(|(at, _)| *at == stamp) {
        return *flip;
    }
    let display = wprs_format::peek(&item.path).ok().and_then(|m| m.display);
    let chosen = display.and_then(|d| d.flip).unwrap_or(Flip::None);
    let flip = [chosen == Flip::Horizontal, chosen == Flip::Vertical];
    flips.insert(item.path.clone(), (stamp, flip));
    flip
}
