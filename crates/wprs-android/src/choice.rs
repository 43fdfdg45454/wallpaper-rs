use crate::library::{now, with};
use crate::{Error, settings, store};
use chrono::{Local, Utc};
use serde::Deserialize;
use std::collections::HashSet;
use std::hash::{BuildHasher, RandomState};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use wprs_library::{Library, Rotation, Turn, rotate};
use wprs_schedule::PlaylistState;

const HOME: &str = "home";
pub(crate) const LOCK: &str = "lock";

mod screens;

#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use screens::{Target, commit};

static PENDING: Mutex<Option<Choice>> = Mutex::new(None);
type Listed = Option<(String, Vec<String>)>;

static LISTS: Mutex<[Listed; 2]> = Mutex::new([None, None]);

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Choice {
    Item(String),
    Playlist(String),
}

pub(crate) fn screen(lock: bool) -> &'static str {
    if lock { LOCK } else { "" }
}

fn row(lock: bool) -> &'static str {
    if lock { LOCK } else { HOME }
}

pub(crate) struct Shown {
    pub(crate) uid: String,
    pub(crate) path: PathBuf,
    pub(crate) video: bool,
    pub(crate) scene: bool,
    pub(crate) web: bool,
    pub(crate) flip: [bool; 2],
}

pub(crate) fn pick(choice: Option<Choice>) {
    *PENDING.lock().unwrap_or_else(PoisonError::into_inner) = choice;
}

pub(crate) fn unpick(choice: &Choice) {
    PENDING.lock().unwrap_or_else(PoisonError::into_inner).take_if(|pending| pending == choice);
}

fn seed() -> u32 {
    RandomState::new().hash_one(std::time::SystemTime::now()) as u32
}

pub(crate) fn turn(
    lib: &mut Library,
    (playlist, lock): (&str, bool),
    turn: Turn,
    on_battery: bool,
) -> Result<bool, Error> {
    let now = Utc::now();
    let rules = lib.playlist(playlist)?.rules;
    let output = if rules.synced { HOME } else { row(lock) };
    let flashing: HashSet<String> = lib.unconfirmed_flashing()?.into_iter().collect();
    let resolved = lib.resolve(playlist, now.timestamp(), on_battery, &flashing)?;
    let id = resolved.playlist.id.clone();
    let stored = lib.rotation(output)?.filter(|row| row.playlist == id);
    let mut row = stored.clone().unwrap_or_else(|| {
        let state = PlaylistState::new(seed());
        Rotation { output: output.into(), playlist: id.clone(), state, next_at: None, current: None }
    });
    let mut lists = LISTS.lock().unwrap_or_else(PoisonError::into_inner);
    let slot = &mut lists[usize::from(lock)];
    let last = slot.take().filter(|(key, _)| *key == id).map(|(_, uids)| uids);
    let turn = Turn { fresh: last.is_none(), ..turn };
    let switched = rotate(&mut row, last.as_deref(), &resolved, &rules, turn, now, &Local);
    *slot = Some((id, resolved.uids));
    if stored.as_ref() != Some(&row) {
        lib.save_rotation(&row)?;
    }
    if let Some(item) = row.current.as_ref().filter(|_| switched || turn.start) {
        lib.mark_used(item, output, now.timestamp())?;
    }
    Ok(switched)
}

fn chosen(lib: &Library, preview: bool, lock: bool) -> Result<Option<String>, Error> {
    let pending = preview.then(|| PENDING.lock().unwrap_or_else(PoisonError::into_inner).clone()).flatten();
    let shown = settings::config().resolve(screen(lock));
    let choice = pending.or_else(|| {
        let image = shown.image.and_then(|path| lib.resolve_item(path.to_str()?).ok()).map(Choice::Item);
        shown.playlist.map(Choice::Playlist).or(image)
    });
    Ok(match choice {
        None => None,
        Some(Choice::Item(uid)) => Some(uid),
        Some(Choice::Playlist(playlist)) => {
            let synced = lib.playlist(&playlist).is_ok_and(|list| list.rules.synced);
            match lib.rotation(if synced { HOME } else { row(lock) })?.filter(|row| row.playlist == playlist)
            {
                Some(row) => row.current,
                None => {
                    let shown = |i: &wprs_library::Item| {
                        !i.package.as_ref().is_some_and(|p| p.asks("photosensitive"))
                    };
                    lib.playlist_items(&playlist, now())?
                        .into_iter()
                        .find(|i| !i.missing && shown(i))
                        .map(|i| i.uid)
                }
            }
        }
    })
}

#[cfg(test)]
pub(crate) fn shown(preview: bool) -> Option<Shown> {
    shown_on(preview, false)
}

pub(crate) fn shown_on(preview: bool, lock: bool) -> Option<Shown> {
    let item = with(|lib| chosen(lib, preview, lock)?.map(|uid| Ok(lib.item(&uid)?)).transpose()).ok()??;
    let (path, kind) = crate::pack::media(&item).inspect_err(|e| log::warn!("{e}")).ok()?;
    let (video, scene, web) = (kind == "video", kind == "scene", kind == "web");
    let flip = if video || kind == "image" { crate::pack::flip(&item) } else { [false; 2] };
    Some(Shown { video, scene, web, flip, uid: item.uid, path })
}

pub(crate) fn colors(preview: bool, lock: bool) -> Result<Option<[i32; 4]>, Error> {
    with(|lib| match chosen(lib, preview, lock)? {
        Some(uid) => {
            let item = lib.item(&uid)?;
            let mut colors = store::colors(lib, &item)?;
            let accent = colors.as_ref().and_then(|_| crate::accent::accent(&item));
            colors.iter_mut().zip(accent).for_each(|(c, accent)| c[0] = accent);
            Ok(colors)
        }
        None => Ok(None),
    })
}

pub(crate) fn advance(on_battery: bool, lock: bool) -> Result<bool, Error> {
    let Some(playlist) = settings::config().resolve(screen(lock)).playlist else { return Ok(false) };
    let live = Turn { live: true, ..Turn::default() };
    let switched = with(|lib| turn(lib, (&playlist, lock), live, on_battery))?;
    if switched {
        store::changed();
    }
    Ok(switched)
}

pub(crate) fn retime(lib: &mut Library, on_battery: bool) {
    let config = settings::config();
    for lock in [false, true] {
        if let Some(playlist) = config.resolve(screen(lock)).playlist {
            let _ = turn(lib, (&playlist, lock), Turn { retime: true, ..Turn::default() }, on_battery);
        }
    }
}
