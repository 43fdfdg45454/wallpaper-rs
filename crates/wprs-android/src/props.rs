use crate::library::with;
use crate::pack::{self, Pack};
use crate::{Error, settings, store};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use wprs_core::model::Input;
use wprs_scene::{Runtime, VOLUME, Values};

const GRANTS: &str = "inputs.json";

type Grants = BTreeMap<String, BTreeSet<Input>>;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum Request {
    Scene { uid: String, languages: Vec<String> },
    Property { uid: String, key: String, value: Value },
    Preset { uid: String, preset: String },
    Reset { uid: String },
    Grant { uid: String, input: String, on: bool },
    Examples { dir: String },
}

pub(crate) fn request(request: Request) -> Result<Value, Error> {
    let done = |result: Result<(), Error>| result.map(|()| Value::Null);
    match request {
        Request::Scene { uid, languages } => form(&*scene(&uid)?, &languages),
        Request::Property { uid, key, value } => done(set(&*scene(&uid)?, &key, &value)),
        Request::Preset { uid, preset } => done(choose(&*scene(&uid)?, &preset)),
        Request::Reset { uid } => done(change(&*scene(&uid)?, BTreeMap::clear)),
        Request::Grant { uid, input, on } => done(grant(&*scene(&uid)?, &input, on)),
        Request::Examples { dir } => Ok(json!(pack::examples(Path::new(&dir))?)),
    }
}

fn scene(uid: &str) -> Result<Arc<Pack>, Error> {
    pack::load(&with(|lib| Ok(lib.item(uid)?.path))?)
}

pub(crate) fn values(pack: &Pack) -> Values {
    let config = settings::config();
    let json = |table: &&BTreeMap<String, toml::Value>| -> Values {
        table.iter().filter_map(|(k, v)| Some((k.clone(), serde_json::to_value(v).ok()?))).collect()
    };
    let layers: Vec<Values> = config.scene_values("", &pack.id).iter().map(json).collect();
    wprs_scene::effective(&pack.properties, &layers.iter().collect::<Vec<_>>())
}

fn grants() -> Grants {
    let read = store::dir().ok().and_then(|dir| fs::read(dir.join(GRANTS)).ok());
    read.and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

pub(crate) fn granted(id: &str) -> [bool; 2] {
    let grants = grants();
    [Input::Audio, Input::Pointer].map(|input| grants.get(id).is_some_and(|g| g.contains(&input)))
}

fn form(pack: &Pack, languages: &[String]) -> Result<Value, Error> {
    let values = values(pack);
    let sounds = pack.scene.sounds().next().is_some();
    let mut props = pack.properties.clone();
    props.retain(|key, _| !sounds || key != VOLUME);
    let (fields, presets) = wprs_scene::form(&props, &pack.presets, &values, languages);
    let uses = Runtime::new(Arc::clone(&pack.scene), values).uses();
    let [audio, touch] = granted(&pack.id);
    Ok(json!({
        "fields": fields, "presets": presets, "scripts": uses.scripts, "sounds": sounds,
        "uses": { "audio": uses.audio, "touch": uses.cursor }, "granted": { "audio": audio, "touch": touch },
    }))
}

fn change(pack: &Pack, edit: impl FnOnce(&mut BTreeMap<String, toml::Value>)) -> Result<(), Error> {
    settings::update(|config| {
        let section = config.scene_section("");
        let values = section.entry(pack.id.clone()).or_default();
        edit(values);
        if values.is_empty() {
            section.remove(&pack.id);
        }
        Ok(())
    })
}

fn stored(value: &Value) -> Option<toml::Value> {
    (!value.is_null()).then(|| toml::Value::try_from(value).ok()).flatten()
}

fn set(pack: &Pack, key: &str, value: &Value) -> Result<(), Error> {
    let property = pack.properties.get(key).ok_or_else(|| Error::Scene(format!("no property {key}")))?;
    let checked = property.value(value).map_err(|e| Error::Scene(format!("{key} {e}")))?;
    change(pack, |values| match stored(&checked) {
        Some(value) => drop(values.insert(key.to_owned(), value)),
        None => drop(values.remove(key)),
    })
}

fn choose(pack: &Pack, id: &str) -> Result<(), Error> {
    let preset =
        pack.presets.iter().find(|p| p.id == id).ok_or_else(|| Error::Scene(format!("no preset {id}")))?;
    let chosen: Vec<_> = preset.values.iter().filter_map(|(k, v)| Some((k.clone(), stored(v)?))).collect();
    change(pack, |values| values.extend(chosen))
}

fn grant(pack: &Pack, input: &str, on: bool) -> Result<(), Error> {
    let input = match input {
        "audio" => Input::Audio,
        "touch" => Input::Pointer,
        _ => return Err(Error::Scene(format!("{input} is not audio or touch"))),
    };
    let mut grants = grants();
    let inputs = grants.entry(pack.id.clone()).or_default();
    if on {
        inputs.insert(input)
    } else {
        inputs.remove(&input)
    };
    grants.retain(|_, inputs| !inputs.is_empty());
    store::write(&store::dir()?.join(GRANTS), &serde_json::to_vec_pretty(&grants)?)?;
    store::changed();
    Ok(())
}
