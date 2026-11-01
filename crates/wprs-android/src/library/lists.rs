use super::known;
use crate::{Error, settings, store};
use serde_json::{Value, json};
use std::collections::HashSet;
use wprs_library::{Item, Library};

pub(super) fn remove(lib: &mut Library, uids: &[String], now: i64) -> Result<Value, Error> {
    let refs = known(lib, uids);
    let hashes = lib.hashed()?.into_iter().filter(|(i, _)| refs.contains(&i.uid.as_str()));
    let removed: Vec<String> = hashes.map(|(_, hash)| wprs_sync::removed_blob(&hash)).collect();
    let items = lib.remove_items(&refs)?;
    crate::keeping::forget(lib, &items)?;
    crate::state::forget(removed, now * 1000)?;
    items.iter().try_for_each(store::forget)?;
    let gone =
        |o: &wprs_core::config::OutputConfig| items.iter().any(|item| o.image.as_ref() == Some(&item.path));
    let config = settings::config();
    if gone(&config.default) || config.outputs.values().any(gone) {
        settings::update(|config| {
            let sections = std::iter::once(&mut config.default).chain(config.outputs.values_mut());
            sections.filter(|o| gone(o)).for_each(|o| o.image = None);
            config.outputs.retain(|_, o| *o != Default::default());
            Ok(())
        })?;
    }
    Ok(Value::Null)
}

pub(super) fn playlists(lib: &Library, now: i64) -> Result<Value, Error> {
    let config = settings::config();
    let on = |uid: &str, screen: &str| config.resolve(screen).playlist.as_deref() == Some(uid);
    let listed = lib.playlists()?.into_iter().map(|p| {
        let count = lib.playlist_items(&p.uid, now)?.len();
        let (home, lock) = (on(&p.uid, ""), on(&p.uid, crate::choice::LOCK));
        Ok(json!({ "count": count, "current": home || lock, "home": home, "lock": lock, "playlist": p }))
    });
    Ok(Value::Array(listed.collect::<Result<_, Error>>()?))
}

pub(super) fn add(lib: &mut Library, playlist: &str, items: &[String]) -> Result<Value, Error> {
    let listed = |item: &str| lib.item_playlists(item).is_ok_and(|p| p.iter().any(|p| p.uid == playlist));
    let mut seen = HashSet::new();
    let fresh: Vec<&str> = known(lib, items).into_iter().filter(|i| seen.insert(*i) && !listed(i)).collect();
    Ok(json!(lib.add_to_playlist(playlist, &fresh)?))
}

pub(super) fn listed(items: Vec<Item>) -> Result<Value, Error> {
    let listed = items.into_iter().map(|item| {
        let exposes = item.package.as_ref().map(crate::page::exposes);
        let thumb = store::thumb_file(&item.uid, "jpg").ok().filter(|path| path.is_file());
        let mut value = serde_json::to_value(item)?;
        if let Some(exposes) = exposes {
            value["package"]["exposes"] = json!(exposes);
        }
        value["thumb"] = json!(thumb);
        Ok(value)
    });
    listed.collect::<Result<_, Error>>().map(Value::Array)
}
