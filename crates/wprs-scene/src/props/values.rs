use super::{Control, Pick, Property, Values};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

pub fn effective(props: &BTreeMap<String, Property>, layers: &[&Values]) -> Values {
    let pick = |(id, prop): (&String, &Property)| {
        let set = layers.iter().find_map(|values| prop.kept(values.get(id)?));
        (id.clone(), set.unwrap_or_else(|| prop.default_value()))
    };
    props.iter().map(pick).collect()
}

pub fn exposed(props: &BTreeMap<String, Property>, values: &Values) -> Values {
    let token = |id: &String, value: &Json| {
        let user =
            props.get(id).is_some_and(|p| matches!(p.control, Control::File { .. } | Control::Folder { .. }));
        match value.as_str() {
            Some(path) if user && path.starts_with('/') => json!(format!("wpak-user://{id}")),
            _ if user => Json::Null,
            _ => value.clone(),
        }
    };
    values.iter().map(|(id, value)| (id.clone(), token(id, value))).collect()
}

pub fn stale(props: &BTreeMap<String, Property>, layers: &[&Values]) -> Vec<String> {
    let why = |(id, value): (&String, &Json)| match props.get(id) {
        None => Some(format!("{id} no longer exists")),
        Some(prop) if prop.kept(value).is_none() => Some(format!("{id} no longer accepts {value}")),
        _ => None,
    };
    let mut found: Vec<String> = layers.iter().flat_map(|values| values.iter()).filter_map(why).collect();
    found.sort();
    found.dedup();
    found.into_iter().map(|w| format!("{w}; the default is used")).collect()
}

fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^ (x >> 33)
}

impl Property {
    pub fn folder_turn(&self, now: u64, count: usize) -> Option<usize> {
        let Control::Folder { pick, every, .. } = self.control else { return None };
        let turn = now / u64::from(every.max(1));
        let at = match pick {
            Pick::Sequence => turn,
            Pick::Random => mix(turn ^ count as u64),
        };
        (count > 0).then(|| (at % count as u64) as usize)
    }
}
