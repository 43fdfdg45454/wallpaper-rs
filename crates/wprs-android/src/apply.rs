use crate::Error;
use crate::library::{now, with};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use wprs_library::PlaylistInfo;
use wprs_sync::{Ids, Kept, Local, View, item_view, keep, keep_name, playlist_rules, renamed, text};

type Links = BTreeMap<String, String>;

pub(crate) fn item(local: &Local, key: &str, item: &str, view: &View, kept: &mut Kept) -> Result<(), Error> {
    let tags: Vec<&str> = view.keys().filter_map(|key| key.strip_prefix("t:")).collect();
    let rating = view.get("rating").map_or(Some(0), |r| r.as_u64().filter(|r| *r <= 5));
    let favorite = view.get("favorite").map_or(Some(false), Value::as_bool);
    for uid in local.items.get(item).into_iter().flatten() {
        let held = with(|lib| {
            let item = lib.item(uid)?;
            let (shown, mut held) = (item_view(&item), BTreeMap::new());
            let mut hold = |field: &str| {
                drop(held.insert(field.to_owned(), shown.get(field).cloned().unwrap_or_default()))
            };
            match rating {
                Some(rating) if item.rating != rating as u8 => lib.set_rating(uid, rating as u8)?,
                Some(_) => {}
                None => hold("rating"),
            }
            match favorite {
                Some(favorite) if item.favorite != favorite => lib.set_favorite(uid, favorite)?,
                Some(_) => {}
                None => hold("favorite"),
            }
            if let Some(title) = view.get("title").and_then(Value::as_str).filter(|t| *t != item.title) {
                lib.set_title(uid, title)?;
            }
            let add: Vec<&str> =
                tags.iter().copied().filter(|t| !item.tags.iter().any(|have| have == t)).collect();
            let gone: Vec<&str> =
                item.tags.iter().map(String::as_str).filter(|t| !tags.contains(t)).collect();
            if !add.is_empty() || !gone.is_empty() {
                lib.retag(&[uid], &add, &gone)?;
            }
            Ok(held)
        })?;
        keep(kept, key, held);
    }
    Ok(())
}

pub(crate) fn playlist(
    local: &Local,
    key: &str,
    view: Option<View>,
    links: &mut Links,
    kept: &mut Kept,
) -> Result<(), Error> {
    let uid =
        links.get(key).filter(|uid| with(|lib| Ok(lib.playlist(uid).is_ok())).unwrap_or(false)).cloned();
    let Some(view) = view else {
        links.remove(key);
        return uid.map_or(Ok(()), |uid| with(|lib| crate::library::drop_playlist(lib, &uid)));
    };
    let (name, query) =
        (text(&view, "name", "Playlist"), view.get("query").and_then(Value::as_str).map(String::from));
    let current = uid.as_ref().and_then(|uid| with(|lib| Ok(lib.playlist(uid)?)).ok());
    let (rules, held) = playlist_rules(&view, &current.map(|p| p.rules).unwrap_or_default());
    keep(kept, key, held);
    let mut order: Vec<(f64, &String, &str)> = view
        .iter()
        .filter(|(k, _)| k.starts_with("e:"))
        .filter_map(|(id, e)| Some((e["p"].as_f64()?, id, e["i"].as_str()?)))
        .collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(b.1)));
    let wanted: Vec<&str> =
        order.iter().filter_map(|(.., item)| local.items.get(*item)?.first().map(String::as_str)).collect();
    let shown = with(|lib| {
        let uid = match &uid {
            Some(uid) => uid.clone(),
            None => renamed(&name, |name| lib.create_playlist(name, query.as_deref(), &rules))?,
        };
        links.insert(key.to_owned(), uid.clone());
        let info = lib.playlist(&uid)?;
        let updated = PlaylistInfo { name: name.clone(), query: query.clone(), rules, ..info.clone() };
        let mut shown = name.clone();
        if updated != info && lib.update_playlist(&updated).is_err() {
            shown.clone_from(&info.name);
            lib.update_playlist(&PlaylistInfo { name: info.name.clone(), ..updated })?;
        }
        let current: Vec<String> = lib.playlist_items(&uid, now())?.into_iter().map(|i| i.uid).collect();
        if query.is_none() && current.iter().map(String::as_str).ne(wanted.iter().copied()) {
            lib.remove_from_playlist_many(&uid, &current.iter().map(String::as_str).collect::<Vec<_>>())?;
            lib.add_to_playlist(&uid, &wanted)?;
        }
        Ok(shown)
    })?;
    keep_name(kept, key, &shown, &name);
    Ok(())
}

pub(crate) fn copies(
    local: &Local,
    copies: Vec<(String, View)>,
    links: &mut Links,
    ids: &Ids,
    conflict: &str,
) -> usize {
    let mut made = 0;
    for (_, mut view) in copies.into_iter().filter(|(key, _)| key.starts_with("playlists/")) {
        let name = text(&view, "name", "Playlist");
        let named = conflict.contains("%1").then(|| conflict.replace("%1", &name));
        view.insert("name".into(), json!(named.unwrap_or_else(|| format!("{name} (conflict)"))));
        let draft = "playlists/copy".to_owned();
        let result = playlist(local, &draft, Some(view), links, &mut Kept::new());
        match (result, links.remove(&draft)) {
            (Ok(()), Some(uid)) => {
                links.insert(format!("playlists/{}", ids.named("playlist", &uid)), uid);
                made += 1;
            }
            _ => log::warn!("the conflict copy of a playlist could not be made"),
        }
    }
    made
}
