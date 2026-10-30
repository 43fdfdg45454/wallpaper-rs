use crate::{Binding, Param, Scene, ScriptApi};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Site {
    pub key: String,
    pub path: String,
    pub script: String,
    pub call: String,
    pub params: BTreeMap<String, Param>,
    pub layer: Option<String>,
}

pub fn script_key(script: &str, call: Option<&str>, params: Option<&BTreeMap<String, Param>>) -> String {
    let params = params.map(|p| serde_json::to_string(p).unwrap_or_default()).unwrap_or_default();
    format!("{script}#{}{params}", call.unwrap_or("update"))
}

impl<T> Binding<T> {
    pub fn per_binding(&self) -> bool {
        self.script.is_some() && (self.call.is_some() || self.params.is_some())
    }
}

pub fn sites(scene: &Scene) -> Vec<Site> {
    let mut found = Vec::new();
    walk(&serde_json::to_value(scene).unwrap_or_default(), "", None, &mut found);
    let mut seen = std::collections::BTreeSet::new();
    found.retain(|s: &Site| seen.insert(s.key.clone()));
    found
}

fn walk(json: &Json, at: &str, layer: Option<&str>, out: &mut Vec<Site>) {
    match json {
        Json::Object(map) => {
            let own = map.get("type").and(map.get("id")).and_then(Json::as_str);
            let layer = own.or(layer);
            let bound = (map.get("fn").is_some() || map.get("params").is_some())
                .then(|| map.get("script")?.as_str())
                .flatten();
            if let Some(script) = bound {
                let call = map.get("fn").and_then(Json::as_str);
                let params: Option<BTreeMap<String, Param>> =
                    map.get("params").and_then(|p| serde_json::from_value(p.clone()).ok());
                let key = script_key(script, call, params.as_ref());
                let (script, call) = (script.to_owned(), call.unwrap_or("update").to_owned());
                let (params, layer) = (params.unwrap_or_default(), layer.map(String::from));
                return out.push(Site { key, path: at.to_owned(), script, call, params, layer });
            }
            let escaped = |k: &str| k.replace('~', "~0").replace('/', "~1");
            map.iter().for_each(|(k, v)| walk(v, &format!("{at}/{}", escaped(k)), layer, out));
        }
        Json::Array(items) => {
            items.iter().enumerate().for_each(|(i, v)| walk(v, &format!("{at}/{i}"), layer, out))
        }
        _ => {}
    }
}

pub fn script_inputs<'a>(
    scene: &Scene,
    sources: impl IntoIterator<Item = &'a (String, String)>,
) -> [bool; 2] {
    let sources: Vec<&(String, String)> = sources.into_iter().collect();
    let text = |path: &str| sources.iter().find(|(p, _)| p == path).map_or("", |(_, s)| s.as_str());
    let apis: Vec<ScriptApi> = scene.scripts.iter().flat_map(|s| s.effective(text(&s.path))).collect();
    [ScriptApi::Audio, ScriptApi::Pointer].map(|api| apis.contains(&api))
}
