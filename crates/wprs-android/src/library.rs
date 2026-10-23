use crate::choice::{self, Choice};
use crate::{Error, props, settings, store};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Mutex, PoisonError};
use wprs_library::{Item, Library, PlaylistInfo, Rules, Turn};

const DATABASE: &str = "library.db";

static LIBRARY: Mutex<Option<Library>> = Mutex::new(None);

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Items { query: String },
    Item { uid: String },
    Thumb { uid: String },
    Favorite { uid: String, favorite: bool },
    Remove { uid: String },
    Playlists,
    Entries { playlist: String },
    Create { name: String },
    Save { playlist: PlaylistInfo },
    Delete { playlist: String },
    Add { playlist: String, item: String },
    Take { playlist: String, item: String },
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
    answer.unwrap_or_else(|e| json!({ "error": e.to_string() })).to_string()
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
        Request::Items { query } => listed(lib.search(&query, now, 0, u32::MAX)?)?,
        Request::Item { uid } => listed(vec![lib.item(&uid)?])?[0].take(),
        Request::Thumb { uid } => json!(store::thumb(lib, &lib.item(&uid)?)?),
        Request::Favorite { uid, favorite } => json!(lib.set_favorite(&uid, favorite)?),
        Request::Remove { uid } => {
            let item = lib.item(&uid)?;
            lib.remove_item(&uid)?;
            store::forget(&item)?;
            if shown.image.as_ref() == Some(&item.path) {
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
        Request::Add { playlist, item } => match lib.item_playlists(&item)?.iter().any(|p| p.uid == playlist)
        {
            true => Value::Null,
            false => json!(lib.add_to_playlist(&playlist, &[&item])?),
        },
        Request::Take { playlist, item } => json!(lib.remove_from_playlist(&playlist, &item)?),
        Request::Move { playlist, from, to } => json!(lib.move_in_playlist(&playlist, from, to)?),
        Request::Pick { choice } => json!(choice::pick(choice)),
        Request::Unpick { choice } => json!(choice::unpick(&choice)),
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
