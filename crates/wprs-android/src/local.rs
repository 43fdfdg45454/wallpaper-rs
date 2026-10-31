use crate::library::{now, with};
use crate::{Error, apply, depth, settings, store};
use std::collections::{BTreeMap, BTreeSet};
use wprs_sync::{
    DEVICE_KEYS, Ids, Kept, Local, Replica, View, edit_fields, item_view, keep, kind, link, object, preserve,
    settings as sections,
};

type Links = BTreeMap<String, String>;

fn device() -> Vec<&'static str> {
    DEVICE_KEYS.into_iter().chain(["playback.max_fps"]).collect()
}

pub(crate) fn snapshot(
    ids: &Ids,
    replica: &Replica,
    links: &mut Links,
    present: &BTreeSet<String>,
    kept: &mut Kept,
) -> Result<Local, Error> {
    let (hashed, playlists) = with(|lib| {
        let lists = lib.playlists()?.into_iter().map(|p| {
            let items = if p.query.is_none() { lib.playlist_items(&p.uid, now())? } else { Vec::new() };
            Ok::<_, Error>((p, items.into_iter().map(|i| i.uid).collect::<Vec<_>>()))
        });
        Ok((lib.hashed()?, lists.collect::<Result<Vec<_>, _>>()?))
    })?;
    let base = |key: &str| replica.bases.get(key).and_then(|(_, doc)| doc.view());
    let mut local = Local::default();
    for (item, hash) in hashed {
        let key = ids.blob(&hash);
        local.keys.insert(item.uid.clone(), key.clone());
        let uids = local.items.entry(key.clone()).or_default();
        uids.push(item.uid.clone());
        let record = format!("library/{key}");
        if wprs_sync::deleted_before(replica, &record, item.added_at) {
            continue;
        }
        match (uids.len(), base(&record)) {
            (1, Some(view)) if !present.contains(&key) => {
                local.fresh.insert(key);
                local.views.insert(record, view);
            }
            (1, old) => {
                let mut view = item_view(&item);
                view.extend(edit_fields(depth::shared(&item).as_ref(), old.as_ref()));
                local.views.insert(record, view);
            }
            _ => {}
        }
    }
    for (info, items) in playlists {
        let key = link(links, ids, "playlist", &info.uid);
        local.views.insert(key.clone(), local.playlist(&info, &items, base(&key)));
    }
    local.views.extend(sections(&settings::config().options(), replica, ids, &device()));
    for (key, (_, doc)) in &replica.bases {
        let shared = key.starts_with("repos/") || object(key, Some(doc));
        let Some(view) = doc.view().filter(|_| shared && !local.views.contains_key(key)) else { continue };
        let unlinked = key.starts_with("playlists/") && !links.contains_key(key);
        if unlinked || !key.starts_with("playlists/") {
            local.views.insert(key.clone(), view);
        }
        if unlinked {
            local.unlinked.insert(key.clone());
        }
    }
    preserve(&mut local.views, replica, kept, &BTreeSet::new());
    Ok(local)
}

pub(crate) fn apply(
    local: &Local,
    applied: Vec<(String, Option<View>)>,
    links: &mut Links,
    replica: &Replica,
    kept: &mut Kept,
) -> (usize, Vec<(String, String)>) {
    let rank = |key: &String| ["library/", "playlists/", "settings/"].iter().position(|p| key.starts_with(p));
    let mut applied = applied;
    applied.sort_by_key(|(key, _)| rank(key).unwrap_or(3));
    let (mut done, mut failed) = (0, Vec::new());
    for (key, view) in applied {
        let section = replica.bases.get(&key).is_none_or(|(_, doc)| kind(doc) == "section");
        let result = match (key.split_once('/').unwrap_or_default(), view) {
            (("library", item), Some(view)) => apply::item(local, &key, item, &view, kept),
            (("library", item), None) => delete(local, item),
            (("playlists", _), view) => apply::playlist(local, &key, view, links, kept),
            (("settings", _), Some(view)) if section => options(&key, &view, kept),
            _ => continue,
        };
        match result {
            Ok(()) => done += 1,
            Err(e) => failed.push((key, e.to_string())),
        }
    }
    if done > 0 {
        store::changed();
    }
    (done, failed)
}

fn delete(local: &Local, item: &str) -> Result<(), Error> {
    let uids: Vec<&str> = local.items.get(item).into_iter().flatten().map(String::as_str).collect();
    let shown = settings::config().resolve("").image;
    let items = with(|lib| Ok::<_, Error>(lib.remove_items(&uids)?))?;
    items.iter().try_for_each(store::forget)?;
    match items.iter().any(|item| shown.as_ref() == Some(&item.path)) {
        true => settings::update(|config| {
            config.default.image = None;
            Ok(())
        }),
        false => Ok(()),
    }
}

fn options(key: &str, view: &View, kept: &mut Kept) -> Result<(), Error> {
    let (options, device, mut held) = (settings::config().options(), device(), BTreeMap::new());
    for (option, value) in view.iter().filter(|(option, _)| !device.contains(&option.as_str())) {
        let current = options.get(option).and_then(|v| serde_json::to_value(v).ok());
        let Some(current) = current.filter(|current| current != value) else { continue };
        let text = value.as_str().map_or_else(|| value.to_string(), String::from);
        if settings::update(|config| config.set_option(option, &text)).is_err() {
            held.insert(option.clone(), current);
        }
    }
    keep(kept, key, held);
    Ok(())
}
