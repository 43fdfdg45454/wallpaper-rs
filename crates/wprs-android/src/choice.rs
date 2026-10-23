use crate::library::{now, with};
use crate::{Error, settings, store};
use chrono::{Local, Utc};
use serde::Deserialize;
use std::collections::HashSet;
use std::hash::{BuildHasher, RandomState};
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use wprs_library::{Kind, Library, Rotation, Turn, rotate};
use wprs_schedule::PlaylistState;

const OUTPUT: &str = "home";

static PENDING: Mutex<Option<Choice>> = Mutex::new(None);
static LISTS: Mutex<Option<(String, Vec<String>)>> = Mutex::new(None);

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Choice {
    Item(String),
    Playlist(String),
}

pub(crate) struct Shown {
    pub(crate) uid: String,
    pub(crate) path: PathBuf,
    pub(crate) video: bool,
    pub(crate) scene: bool,
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

pub(crate) fn turn(lib: &mut Library, playlist: &str, turn: Turn, on_battery: bool) -> Result<bool, Error> {
    let now = Utc::now();
    let rules = lib.playlist(playlist)?.rules;
    let resolved = lib.resolve(playlist, now.timestamp(), on_battery, &HashSet::new())?;
    let id = resolved.playlist.id.clone();
    let stored = lib.rotation(OUTPUT)?.filter(|row| row.playlist == id);
    let mut row = stored.clone().unwrap_or_else(|| {
        let state = PlaylistState::new(seed());
        Rotation { output: OUTPUT.into(), playlist: id.clone(), state, next_at: None, current: None }
    });
    let mut lists = LISTS.lock().unwrap_or_else(PoisonError::into_inner);
    let last = lists.take().filter(|(key, _)| *key == id).map(|(_, uids)| uids);
    let turn = Turn { fresh: last.is_none(), ..turn };
    let switched = rotate(&mut row, last.as_deref(), &resolved, &rules, turn, now, &Local);
    *lists = Some((id, resolved.uids));
    if stored.as_ref() != Some(&row) {
        lib.save_rotation(&row)?;
    }
    if let Some(item) = row.current.as_ref().filter(|_| switched || turn.start) {
        lib.mark_used(item, OUTPUT, now.timestamp())?;
    }
    Ok(switched)
}

fn chosen(lib: &Library, preview: bool) -> Result<Option<String>, Error> {
    let pending = preview.then(|| PENDING.lock().unwrap_or_else(PoisonError::into_inner).clone()).flatten();
    let shown = settings::config().resolve("");
    let choice = pending.or_else(|| {
        let image = shown.image.and_then(|path| lib.resolve_item(path.to_str()?).ok()).map(Choice::Item);
        shown.playlist.map(Choice::Playlist).or(image)
    });
    Ok(match choice {
        None => None,
        Some(Choice::Item(uid)) => Some(uid),
        Some(Choice::Playlist(playlist)) => {
            match lib.rotation(OUTPUT)?.filter(|row| row.playlist == playlist) {
                Some(row) => row.current,
                None => lib.playlist_items(&playlist, now())?.into_iter().find(|i| !i.missing).map(|i| i.uid),
            }
        }
    })
}

pub(crate) fn shown(preview: bool) -> Option<Shown> {
    let item = with(|lib| chosen(lib, preview)?.map(|uid| Ok(lib.item(&uid)?)).transpose()).ok()??;
    let (video, scene) = (item.kind == Kind::Video, item.kind == Kind::Package);
    Some(Shown { video, scene, uid: item.uid, path: item.path })
}

pub(crate) fn colors(preview: bool) -> Result<Option<[i32; 4]>, Error> {
    with(|lib| match chosen(lib, preview)? {
        Some(uid) => store::colors(lib, &lib.item(&uid)?),
        None => Ok(None),
    })
}

pub(crate) fn commit(on_battery: bool) -> Result<bool, Error> {
    let Some(choice) = PENDING.lock().unwrap_or_else(PoisonError::into_inner).take() else {
        return Ok(false);
    };
    with(|lib| match choice {
        Choice::Item(uid) => {
            let path = lib.item(&uid)?.path;
            settings::update(|config| {
                config.set_image("", path, None);
                Ok(())
            })
        }
        Choice::Playlist(uid) => {
            settings::update(|config| {
                config.set_playlist("", uid.clone());
                Ok(())
            })?;
            turn(lib, &uid, Turn { start: true, live: true, ..Turn::default() }, on_battery).map(drop)
        }
    })?;
    store::changed();
    Ok(settings::chosen(settings::COLORS) != "never")
}

pub(crate) fn advance(on_battery: bool) -> Result<bool, Error> {
    let Some(playlist) = settings::config().resolve("").playlist else { return Ok(false) };
    let switched = with(|lib| turn(lib, &playlist, Turn { live: true, ..Turn::default() }, on_battery))?;
    if switched {
        store::changed();
    }
    Ok(switched)
}
