use crate::effects::{EXTRA, builtin};
use crate::{Descriptor, ParamDef};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    params: BTreeMap<String, Vec<ParamDef>>,
    effects: Vec<serde_json::Value>,
}

struct Known {
    params: BTreeMap<String, Vec<ParamDef>>,
    effects: Vec<Descriptor>,
    merged: Vec<Descriptor>,
}

fn known() -> &'static Known {
    static KNOWN: OnceLock<Known> = OnceLock::new();
    KNOWN.get_or_init(|| {
        let catalog: Catalog = serde_json::from_str(include_str!("catalog.json"))
            .unwrap_or_else(|e| panic!("catalog.json: {e}"));
        let parse = |v: &serde_json::Value| {
            Descriptor::parse(&v.to_string(), "").unwrap_or_else(|e| panic!("catalog.json: {e}"))
        };
        let effects: Vec<Descriptor> = catalog.effects.iter().map(parse).collect();
        let merged = builtin().iter().filter_map(|b| {
            let more = catalog.params.get(&b.id)?;
            let mut d = b.clone();
            d.params.extend(more.iter().filter(|p| b.params.iter().all(|o| o.id != p.id)).cloned());
            Some(d)
        });
        let merged = merged.collect();
        Known { params: catalog.params, effects, merged }
    })
}

pub fn described(id: &str) -> Option<&'static Descriptor> {
    let k = known();
    let extra = || EXTRA.read().unwrap_or_else(|e| e.into_inner()).iter().copied().find(|d| d.id == id);
    let found = k.merged.iter().chain(builtin()).chain(&k.effects).find(|d| d.id == id);
    found.or_else(extra)
}

pub fn undrawn() -> &'static [Descriptor] {
    &known().effects
}

pub fn newer_params(effect: &str) -> impl Iterator<Item = &'static ParamDef> {
    known().params.get(effect).into_iter().flatten()
}

pub fn drawn(effect: &str, param: Option<&str>) -> bool {
    let d = builtin().iter().find(|d| d.id == effect);
    d.is_some_and(|d| param.is_none_or(|p| d.params.iter().any(|o| o.id == p)))
}

pub fn reserved(id: &str) -> bool {
    builtin().iter().chain(undrawn()).any(|d| d.id == id)
}
