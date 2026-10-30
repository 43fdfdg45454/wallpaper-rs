use crate::{Binding, Param, Scene, Script, ScriptApi};
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

impl Script {
    pub fn declares(&self) -> bool {
        !self.uses.is_empty() || !self.events.is_empty() || !self.params.is_empty()
    }

    pub fn effective(&self, source: &str) -> Vec<ScriptApi> {
        if self.declares() { self.uses.clone() } else { imported(source) }
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

#[derive(Debug, PartialEq)]
enum Token<'a> {
    Word(&'a str),
    Text(String),
    Mark(char),
}

fn tokens(source: &str) -> Vec<Token<'_>> {
    let (bytes, mut out, mut i) = (source.as_bytes(), Vec::new(), 0);
    let until =
        |from: usize, end: &str| source[from..].find(end).map_or(source.len(), |at| from + at + end.len());
    while i < bytes.len() {
        let c = bytes[i];
        if source[i..].starts_with("//") {
            i = until(i, "\n");
        } else if source[i..].starts_with("/*") {
            i = until(i + 2, "*/");
        } else if matches!(c, b'"' | b'\'' | b'`') {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != c {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            out.push(Token::Text(source[i + 1..j.min(bytes.len())].to_owned()));
            i = j + 1;
        } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
            let end = source[i..].find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'));
            let end = end.map_or(source.len(), |e| i + e);
            out.push(Token::Word(&source[i..end]));
            i = end;
        } else {
            if !c.is_ascii_whitespace() {
                out.push(Token::Mark(c as char));
            }
            i += source[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

fn api(name: &str) -> Option<ScriptApi> {
    Some(match name {
        "audio" => ScriptApi::Audio,
        "input" => ScriptApi::Pointer,
        "particles" => ScriptApi::Particles,
        "setTimeout" | "setInterval" | "clearTimeout" | "clearInterval" => ScriptApi::Timers,
        "console" => ScriptApi::Console,
        "shared" => ScriptApi::Shared,
        "sound" => ScriptApi::Sounds,
        _ => return None,
    })
}

pub fn imported(source: &str) -> Vec<ScriptApi> {
    let (all, mut found) = (tokens(source), Vec::new());
    for (i, t) in all.iter().enumerate() {
        if *t != Token::Word("import") {
            continue;
        }
        let rest = &all[i + 1..];
        let Some(from) = rest.iter().take(256).position(|t| *t == Token::Word("from")) else { continue };
        let (names, module) = (&rest[..from], rest.get(from + 1));
        let apis: Vec<ScriptApi> = match module {
            Some(Token::Text(m)) if m == "wprs/math" => vec![ScriptApi::Math],
            Some(Token::Text(m)) if m == "wprs" && names.contains(&Token::Mark('*')) => {
                vec![ScriptApi::Audio, ScriptApi::Pointer]
            }
            Some(Token::Text(m)) if m == "wprs" => {
                let inner = names.iter().skip_while(|t| **t != Token::Mark('{'));
                let mut list = inner.take_while(|t| **t != Token::Mark('}')).peekable();
                let mut picked = Vec::new();
                while let Some(t) = list.next() {
                    if let Token::Word(w) = t
                        && *w != "as"
                    {
                        picked.extend(api(w));
                        if list.peek() == Some(&&Token::Word("as")) {
                            list.nth(1);
                        }
                    }
                }
                picked
            }
            _ => Vec::new(),
        };
        found.extend(apis);
    }
    found.sort();
    found.dedup();
    found
}
