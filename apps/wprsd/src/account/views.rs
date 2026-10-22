use super::local::{DEVICE_KEYS, OBJECTS};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use wprs_library::{Item, PlaylistInfo};
use wprs_sync::{Ids, Replica, View, positions};

pub const REFERENCES: [&str; 3] = ["item", "playlist", "preset"];

type Lookup<'a> = &'a dyn Fn(&str, &str) -> Option<String>;

#[derive(Default)]
pub struct Local {
    pub views: BTreeMap<String, View>,
    pub items: HashMap<String, Vec<String>>,
    pub keys: HashMap<String, String>,
    pub fresh: BTreeSet<String>,
    pub unlinked: BTreeSet<String>,
}

pub fn link(links: &mut BTreeMap<String, String>, ids: &Ids, kind: &str, uid: &str) -> String {
    if let Some((key, _)) = links.iter().find(|(_, linked)| *linked == uid) {
        return key.clone();
    }
    let collection = if kind == "playlist" { "playlists/".into() } else { format!("settings/{kind}_") };
    let key = format!("{collection}{}", ids.named(kind, uid));
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

pub fn settings(options: &BTreeMap<String, toml::Value>, replica: &Replica) -> BTreeMap<String, View> {
    let mut sections: BTreeMap<String, View> = BTreeMap::new();
    for (option, value) in options.iter().filter(|(k, _)| !DEVICE_KEYS.contains(&k.as_str())) {
        let key = format!("settings/{}", option.split('.').next().unwrap_or_default());
        sections
            .entry(key)
            .or_default()
            .insert(option.clone(), serde_json::to_value(value).unwrap_or_default());
    }
    let synced = replica.bases.iter().filter(|(key, _)| key.starts_with("settings/"));
    for (key, (_, doc)) in synced.filter(|(key, _)| !OBJECTS.iter().any(|kind| key.starts_with(kind))) {
        let unknown = doc.view().into_iter().flatten().filter(|(option, _)| !options.contains_key(option));
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
