use crate::library::with;
use crate::pack::Pack;
use crate::props::{change, stored};
use crate::{Error, store, vault};
use serde_json::{Value, json};
use std::sync::{Mutex, PoisonError};
use wprs_format::values;
use wprs_scene::{Control, Values};

static PRESSED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

pub(crate) fn listed(pack: &Pack) -> Result<Vec<Value>, Error> {
    let mine = with(|lib| Ok(lib.settings(&pack.id)?))?;
    Ok(mine.into_iter().map(|p| json!({ "value": p.uid, "label": p.name })).collect())
}

pub(crate) fn filled(pack: &Pack) -> Vec<String> {
    let kept = vault::kept();
    let secret = |(key, p): (&String, &wprs_scene::Property)| {
        let hidden = matches!(p.control, Control::Text { secret: true, .. });
        (hidden && kept.contains_key(&format!("{}/{key}", pack.id))).then(|| key.clone())
    };
    pack.properties.iter().filter_map(secret).collect()
}

fn current(pack: &Pack) -> Values {
    values::portable(&pack.properties, &crate::props::values(pack))
}

fn keep(pack: &Pack, name: &str, chosen: &Values) -> Result<(), Error> {
    let chosen = serde_json::to_value(chosen)?;
    with(|lib| Ok(lib.keep_settings(&pack.id, name, &chosen).map(drop)?))?;
    store::changed();
    Ok(())
}

pub(crate) fn save(pack: &Pack, name: &str) -> Result<(), Error> {
    keep(pack, name, &current(pack))
}

pub(crate) fn chosen(pack: &Pack, key: &str) -> Result<Values, Error> {
    if let Some(preset) = pack.presets.iter().find(|p| p.id == key) {
        return Ok(preset.values.clone());
    }
    let mine = with(|lib| Ok(lib.settings(&pack.id)?))?;
    let found = mine.into_iter().find(|p| p.uid == key || p.name.eq_ignore_ascii_case(key.trim()));
    found.map(|p| values::saved(&p.value)).ok_or_else(|| Error::Scene(format!("no preset {key}")))
}

pub(crate) fn apply(pack: &Pack, given: &Values) -> Result<(), Error> {
    let fit = |(k, v): (&String, &Value)| {
        let prop = pack.properties.get(k).filter(|p| p.holds_value() && !p.personal())?;
        Some((k.clone(), stored(&prop.value(v).ok()?)?))
    };
    let kept: Vec<_> = given.iter().filter_map(fit).collect();
    change(pack, |values| values.extend(kept))
}

pub(crate) fn rename(preset: &str, name: &str) -> Result<(), Error> {
    with(|lib| Ok(lib.rename_preset(preset, name)?))?;
    store::changed();
    Ok(())
}

pub(crate) fn forget(preset: &str) -> Result<(), Error> {
    let settings = with(|lib| Ok(lib.preset(preset)?))?.package().is_some();
    if !settings {
        return Err(Error::Scene(format!("{preset} is not a preset of settings")));
    }
    with(|lib| Ok(lib.remove_preset(preset)?))?;
    store::changed();
    Ok(())
}

pub(crate) fn export(pack: &Pack, preset: &str) -> Result<String, Error> {
    let chosen = if preset.is_empty() {
        current(pack)
    } else {
        values::portable(&pack.properties, &chosen(pack, preset)?)
    };
    Ok(values::export(&pack.id, &pack.version, chosen))
}

pub(crate) fn import(pack: &Pack, text: &str, name: &str) -> Result<Vec<String>, Error> {
    let (kept, warnings) = values::import(text, &pack.id, &pack.properties).map_err(Error::Scene)?;
    match name.trim() {
        "" => apply(pack, &kept)?,
        name => keep(pack, name, &kept)?,
    }
    Ok(warnings)
}

pub(crate) fn press(pack: &Pack, key: &str) -> Result<(), Error> {
    let button = pack.properties.get(key).filter(|p| matches!(p.control, Control::Button {}));
    let button = button.ok_or_else(|| Error::Scene(format!("no button {key}")))?;
    if !button.met(&pack.properties, &crate::props::values(pack), |p| &p.enabled_if) {
        return Err(Error::Scene(format!("{key} cannot be pressed now")));
    }
    PRESSED.lock().unwrap_or_else(PoisonError::into_inner).push((pack.id.clone(), key.to_owned()));
    store::changed();
    Ok(())
}

pub(crate) fn pressed(package: &str) -> Vec<String> {
    let mut all = PRESSED.lock().unwrap_or_else(PoisonError::into_inner);
    let (mine, rest): (Vec<_>, Vec<_>) = all.drain(..).partition(|(p, _)| p == package);
    *all = rest;
    mine.into_iter().map(|(_, key)| key).collect()
}

pub(crate) fn secret(pack: &Pack, key: &str, text: &str) -> Result<(), Error> {
    let hidden =
        matches!(pack.properties.get(key).map(|p| &p.control), Some(Control::Text { secret: true, .. }));
    if !hidden || text.chars().count() > 1024 || text.chars().any(char::is_control) {
        return Err(Error::Scene(format!("{key} is no secret text of at most 1024 characters")));
    }
    vault::keep(&format!("{}/{key}", pack.id), text)?;
    store::changed();
    Ok(())
}
