use crate::{Doc, Ids, Replica, View, positions};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use wprs_core::edit::{Edit, VERSION};
use wprs_library::{Item, PlaylistInfo};

pub const REFERENCES: [&str; 3] = ["item", "playlist", "preset"];
pub const DOWNLOADS: &str = "models.allow_download";
const EDIT: &str = "e:";
const SCREENS: &str = "e:outputs/";
const PARTS: [&str; 7] = ["geometry", "ops", "masks", "span", "depth", "parallax", "fit"];
const EDIT_BYTES: usize = 64 << 10;
pub const DEVICE_KEYS: [&str; 5] =
    ["playback.volume", "playback.muted", "playback.software_parallax", "library.thumb_cache_mb", DOWNLOADS];

type Lookup<'a> = &'a dyn Fn(&str, &str) -> Option<String>;
pub type Kept = BTreeMap<String, BTreeMap<String, Value>>;

#[derive(Default)]
pub struct Local {
    pub views: BTreeMap<String, View>,
    pub items: HashMap<String, Vec<String>>,
    pub keys: HashMap<String, String>,
    pub fresh: BTreeSet<String>,
    pub unlinked: BTreeSet<String>,
    pub repos: BTreeSet<String>,
}

pub fn kind(doc: &Doc) -> &str {
    doc.fields.get("kind").and_then(|(value, _)| value.as_str()).unwrap_or("section")
}

pub fn object(key: &str, doc: Option<&Doc>) -> bool {
    key.starts_with("playlists/") || doc.is_some_and(|doc| kind(doc) != "section")
}

fn known(key: &str, kind: &str, field: &str) -> bool {
    let has = |names: &[&str]| names.contains(&field);
    match key.split('/').next().unwrap_or_default() {
        "library" => has(&["title", "rating", "favorite"]) || field.starts_with("t:") || edit_part(field),
        "playlists" => has(&["name", "query"]) || field.starts_with("r:") || field.starts_with("e:"),
        "repos" => has(&["url", "repo_id", "name", "fingerprints"]),
        "secrets" => has(&["provider", "kind", "bind", "secret"]),
        "settings" if kind == "preset" => has(&["kind", "name", "value", "item"]),
        "settings" if kind == "profile" => has(&["kind", "name", "outputs", "options", "auto", "priority"]),
        _ => true,
    }
}

pub fn preserve(
    views: &mut BTreeMap<String, View>,
    replica: &Replica,
    kept: &mut Kept,
    dropped: &BTreeSet<String>,
) {
    for (key, view) in views.iter_mut() {
        let Some((base, doc)) = replica.bases.get(key).and_then(|(_, doc)| Some((doc.view()?, doc))) else {
            continue;
        };
        let gone = |field: &str| {
            field == "blob" && key.strip_prefix("library/").is_some_and(|k| dropped.contains(k))
        };
        let unknown =
            base.iter().filter(|(f, _)| !view.contains_key(*f) && !known(key, kind(doc), f) && !gone(f));
        let unknown: Vec<(String, Value)> = unknown.map(|(f, v)| (f.clone(), v.clone())).collect();
        view.extend(unknown);
        if let Some(fields) = kept.get_mut(key) {
            fields.retain(|field, old| view.get(field).unwrap_or(&Value::Null) == old);
            for field in fields.keys() {
                match base.get(field) {
                    Some(value) => drop(view.insert(field.clone(), value.clone())),
                    None => drop(view.remove(field)),
                }
            }
        }
    }
    kept.retain(|key, fields| !fields.is_empty() && views.contains_key(key));
}

pub fn link(links: &mut BTreeMap<String, String>, ids: &Ids, kind: &str, uid: &str) -> String {
    if let Some((key, _)) = links.iter().find(|(_, linked)| *linked == uid) {
        return key.clone();
    }
    let collection = if kind == "playlist" { "playlists" } else { "settings" };
    let key = format!("{collection}/{}", ids.named(kind, uid));
    links.insert(key.clone(), uid.to_owned());
    key
}

pub fn references(
    value: &Value,
    base: Option<&Value>,
    to: Lookup,
    known: &dyn Fn(&str, &str) -> bool,
) -> Value {
    let mut out = value.clone();
    for (name, section) in out.as_object_mut().into_iter().flatten() {
        let Some(section) = section.as_object_mut() else { continue };
        for field in REFERENCES {
            let mapped = section.get(field).and_then(Value::as_str).and_then(|text| to(field, text));
            let kept =
                || base?.get(name)?.get(field)?.as_str().filter(|key| !known(field, key)).map(String::from);
            section.insert(field.into(), mapped.or_else(kept).map_or(Value::Null, Value::String));
        }
    }
    out
}

pub fn settings(
    options: &BTreeMap<String, toml::Value>,
    replica: &Replica,
    ids: &Ids,
    device: &[&str],
) -> BTreeMap<String, View> {
    let mut sections: BTreeMap<String, View> = BTreeMap::new();
    for (option, value) in options.iter().filter(|(k, _)| !device.contains(&k.as_str())) {
        let key = format!("settings/{}", ids.named("section", option.split('.').next().unwrap_or_default()));
        sections
            .entry(key)
            .or_default()
            .insert(option.clone(), serde_json::to_value(value).unwrap_or_default());
    }
    let synced = replica.bases.iter().filter(|(key, _)| key.starts_with("settings/"));
    for (key, (_, doc)) in synced.filter(|(key, (_, doc))| !object(key, Some(doc))) {
        let unknown = doc.view().into_iter().flatten();
        let unknown =
            unknown.filter(|(option, _)| !options.contains_key(option) || device.contains(&option.as_str()));
        sections.entry(key.clone()).or_default().extend(unknown);
    }
    sections
}

pub fn item_view(item: &Item) -> View {
    let mut view = View::from([("title".to_owned(), json!(item.title))]);
    view.extend((item.rating > 0).then(|| ("rating".to_owned(), json!(item.rating))));
    view.extend(item.favorite.then(|| ("favorite".to_owned(), json!(true))));
    view.extend(item.tags.iter().map(|tag| (format!("t:{tag}"), json!(true))));
    view
}

fn edit_part(field: &str) -> bool {
    field.starts_with(SCREENS) || field.strip_prefix(EDIT).is_some_and(|part| PARTS.contains(&part))
}

pub fn edit_fields(edit: Option<&Edit>, base: Option<&View>) -> View {
    let (Some(edit), Ok(Value::Object(blank))) = (edit, serde_json::to_value(Edit::default())) else {
        return View::new();
    };
    let Ok(Value::Object(parts)) = serde_json::to_value(edit) else { return View::new() };
    let parts = parts
        .into_iter()
        .filter(|(part, value)| PARTS.contains(&part.as_str()) && blank.get(part) != Some(value));
    let screens = edit
        .outputs
        .iter()
        .filter_map(|(screen, crop)| Some((format!("{SCREENS}{screen}"), serde_json::to_value(crop).ok()?)));
    let fields = parts.map(|(part, value)| (format!("{EDIT}{part}"), value)).chain(screens);
    fields
        .filter_map(|(field, value)| match value.to_string().len() <= EDIT_BYTES {
            true => Some((field, value)),
            false => base?.get(&field).map(|sent| (field, sent.clone())),
        })
        .collect()
}

pub fn edit_of(view: &View) -> Option<Value> {
    let mut edit = serde_json::Map::new();
    for (field, value) in view.iter().filter(|(field, _)| edit_part(field)) {
        match field.strip_prefix(SCREENS) {
            Some(screen) => edit.entry("outputs").or_insert_with(|| json!({}))[screen] = value.clone(),
            None => drop(edit.insert(field[EDIT.len()..].to_owned(), value.clone())),
        }
    }
    (!edit.is_empty()).then(|| {
        edit.insert("version".into(), json!(VERSION));
        Value::Object(edit)
    })
}

pub fn edit_held(view: &View, own: View) -> BTreeMap<String, Value> {
    let synced = view.keys().filter(|field| edit_part(field)).cloned();
    let fields: BTreeSet<String> = synced.chain(own.keys().cloned()).collect();
    fields.into_iter().map(|field| (field.clone(), own.get(&field).cloned().unwrap_or_default())).collect()
}

impl Local {
    pub fn known(&self, key: &str) -> bool {
        self.items.contains_key(key) && !self.fresh.contains(key)
    }

    pub fn playlist(&self, info: &PlaylistInfo, items: &[String], base: Option<View>) -> View {
        let mut view = View::from([("name".to_owned(), json!(info.name))]);
        view.extend(info.query.as_ref().map(|q| ("query".to_owned(), json!(q))));
        let rules = serde_json::to_value(&info.rules).ok().and_then(|r| r.as_object().cloned());
        view.extend(rules.unwrap_or_default().into_iter().map(|(key, value)| (format!("r:{key}"), value)));
        let (base, mut seen) = (base.unwrap_or_default(), HashMap::<String, u32>::new());
        let unknown = base.iter().filter(|(key, _)| key.starts_with("r:") && !view.contains_key(*key));
        view.extend(unknown.map(|(key, value)| (key.clone(), value.clone())).collect::<Vec<_>>());
        let keys: Vec<&String> = items.iter().filter_map(|uid| self.keys.get(uid)).collect();
        let mut entry = |key: &String| {
            let n = seen.entry(key.clone()).and_modify(|n| *n += 1).or_default();
            format!("e:{}-{n}", &key[..16])
        };
        let ids: Vec<String> = keys.iter().map(|key| entry(key)).collect();
        let entries: Vec<(&String, &Value)> = base.iter().filter(|(key, _)| key.starts_with("e:")).collect();
        let at: BTreeMap<String, f64> =
            entries.iter().filter_map(|(id, e)| Some(((*id).clone(), e["p"].as_f64()?))).collect();
        for ((id, key), pos) in ids.iter().zip(keys).zip(positions(&ids, &at)) {
            view.insert(id.clone(), json!({"i": key, "p": pos}));
        }
        for (id, e) in entries {
            if !view.contains_key(id) && e["i"].as_str().is_some_and(|item| !self.known(item)) {
                view.insert(id.clone(), e.clone());
            }
        }
        view
    }
}
