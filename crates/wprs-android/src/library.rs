use crate::choice::{self, Choice};
use crate::{Error, framing, pack, props, settings, store};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Mutex, PoisonError};
use wprs_library::{Item, Library, PlaylistInfo, Rules, Turn};

const DATABASE: &str = "library.db";

static LIBRARY: Mutex<Option<Library>> = Mutex::new(None);
static LANGUAGES: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub(crate) fn languages() -> Vec<String> {
    LANGUAGES.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

fn heard(languages: &Option<Vec<String>>) {
    if let Some(languages) = languages.as_ref().filter(|l| !l.is_empty()) {
        *LANGUAGES.lock().unwrap_or_else(PoisonError::into_inner) = languages.clone();
    }
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Items { query: String, languages: Option<Vec<String>> },
    Item { uid: String, languages: Option<Vec<String>> },
    Confirm { uid: String, what: String },
    Thumb { uid: String },
    Favorite { uids: Vec<String>, favorite: bool },
    Remove { uids: Vec<String> },
    Playlists,
    Entries { playlist: String },
    Create { name: String },
    Save { playlist: PlaylistInfo },
    Delete { playlist: String },
    Add { playlist: String, items: Vec<String> },
    Take { playlist: String, items: Vec<String> },
    Move { playlist: String, from: usize, to: usize },
    Pick { choice: Option<Choice> },
    Unpick { choice: Choice },
    Current,
}

pub(crate) fn now() -> i64 {
    Utc::now().timestamp()
}

pub(crate) fn with<T>(work: impl FnOnce(&mut Library) -> Result<T, Error>) -> Result<T, Error> {
    let mut library = LIBRARY.lock().unwrap_or_else(PoisonError::into_inner);
    match library.as_mut() {
        Some(library) => work(library),
        None => work(library.insert(Library::open(store::dir()?.join(DATABASE))?)),
    }
}

pub(crate) fn request(text: &str, on_battery: bool) -> String {
    let answer = match serde_json::from_str(text) {
        Ok(scene) => props::request(scene),
        Err(_) => serde_json::from_str(text)
            .map_err(Error::from)
            .and_then(|r| with(|lib| answer(lib, r, on_battery))),
    };
    answer.unwrap_or_else(|e| json!({ "error": e.told() })).to_string()
}

fn answer(lib: &mut Library, request: Request, on_battery: bool) -> Result<Value, Error> {
    let (now, shown) = (now(), settings::config().resolve(""));
    let reads = matches!(
        request,
        Request::Items { .. }
            | Request::Item { .. }
            | Request::Thumb { .. }
            | Request::Playlists
            | Request::Entries { .. }
            | Request::Current
    );
    let value = match request {
        Request::Items { query, languages } => {
            heard(&languages);
            let mut items = lib.search(&query, now, 0, u32::MAX)?;
            lib.localize(&mut items, &languages.unwrap_or_default())?;
            listed(items)?
        }
        Request::Item { uid, languages } => {
            heard(&languages);
            let (mut items, languages) = (vec![lib.item(&uid)?], languages.unwrap_or_default());
            lib.localize(&mut items, &languages)?;
            let about = items[0].package.as_ref().and_then(|_| pack::about(&items[0].path, &languages));
            let animated = about.as_ref().and_then(|_| pack::animated(&uid, &items[0].path));
            let about = about.map(|about| wprs_core::model::About { animated, ..about });
            let lacks = items[0].package.as_ref().map(|p| pack::lacks(&items[0].path, &p.facets));
            let mut value = listed(items)?[0].take();
            (value["about"], value["lacks"]) = (json!(about), json!(lacks.unwrap_or_default()));
            value
        }
        Request::Confirm { uid, what } => lib.confirm(&[&uid], &what).map(|_| Value::Null)?,
        Request::Thumb { uid } => json!(store::thumb(lib, &lib.item(&uid)?)?),
        Request::Favorite { uids, favorite } => json!(lib.set_favorite_many(&known(lib, &uids), favorite)?),
        Request::Remove { uids } => {
            let items = lib.remove_items(&known(lib, &uids))?;
            items.iter().try_for_each(store::forget)?;
            if items.iter().any(|item| shown.image.as_ref() == Some(&item.path)) {
                settings::update(|config| {
                    config.default.image = None;
                    Ok(())
                })?;
            }
            Value::Null
        }
        Request::Playlists => Value::Array(
            (lib.playlists()?.into_iter())
                .map(|p| {
                    let (count, current) =
                        (lib.playlist_items(&p.uid, now)?.len(), shown.playlist == Some(p.uid.clone()));
                    Ok(json!({ "count": count, "current": current, "playlist": p }))
                })
                .collect::<Result<_, Error>>()?,
        ),
        Request::Entries { playlist } => listed(lib.playlist_items(&playlist, now)?)?,
        Request::Create { name } => json!(lib.create_playlist(&name, None, &Rules::default())?),
        Request::Save { playlist } => json!(lib.update_playlist(&playlist)?),
        Request::Delete { playlist } => {
            drop_playlist(lib, &playlist)?;
            Value::Null
        }
        Request::Add { playlist, items } => {
            let listed =
                |item: &str| lib.item_playlists(item).is_ok_and(|p| p.iter().any(|p| p.uid == playlist));
            let mut seen = std::collections::HashSet::new();
            let fresh: Vec<&str> =
                known(lib, &items).into_iter().filter(|i| seen.insert(*i) && !listed(i)).collect();
            json!(lib.add_to_playlist(&playlist, &fresh)?)
        }
        Request::Take { playlist, items } => {
            json!(lib.remove_from_playlist_many(&playlist, &known(lib, &items))?)
        }
        Request::Move { playlist, from, to } => json!(lib.move_in_playlist(&playlist, from, to)?),
        Request::Pick { choice } => {
            if let Some(Choice::Item(uid)) = &choice
                && lib.item(uid)?.package.is_some_and(|p| p.asks("photosensitive"))
            {
                return Err(Error::Scene("confirm this photosensitive wallpaper first".into()));
            }
            json!(choice::pick(choice))
        }
        Request::Unpick { choice } => {
            if let Choice::Item(uid) = &choice {
                framing::discard(uid);
            }
            json!(choice::unpick(&choice))
        }
        Request::Current => {
            let image = shown.image.and_then(|path| lib.resolve_item(path.to_str()?).ok());
            json!({ "image": image, "playlist": shown.playlist })
        }
    };
    if !reads {
        if let Some(playlist) = settings::config().resolve("").playlist {
            let _ = choice::turn(lib, &playlist, Turn { retime: true, ..Turn::default() }, on_battery);
        }
        store::changed();
    }
    Ok(value)
}

fn known<'a>(lib: &Library, uids: &'a [String]) -> Vec<&'a str> {
    uids.iter().map(String::as_str).filter(|uid| lib.item(uid).is_ok()).collect()
}

pub(crate) fn drop_playlist(lib: &mut Library, playlist: &str) -> Result<(), Error> {
    lib.remove_playlist(playlist)?;
    if settings::config().resolve("").playlist.as_deref() == Some(playlist) {
        settings::update(|config| {
            config.default.playlist = None;
            Ok(())
        })?;
    }
    Ok(())
}

fn listed(items: Vec<Item>) -> Result<Value, Error> {
    let listed = items.into_iter().map(|item| {
        let thumb = store::thumb_file(&item.uid, "jpg").ok().filter(|path| path.is_file());
        let mut value = serde_json::to_value(item)?;
        value["thumb"] = json!(thumb);
        Ok(value)
    });
    listed.collect::<Result<_, Error>>().map(Value::Array)
}
