use super::{EDIT, EDIT_BYTES, PARTS, SCREENS};
use crate::View;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use wprs_core::edit::{Edit, VERSION};

pub(super) fn edit_part(field: &str) -> bool {
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
