mod clash;
mod crypto;
mod engine;
mod files;
mod merge;
mod protocol;

use crate::{Doc, Hlc, View};
use serde_json::{Value, json};

pub fn view(pairs: &[(&str, Value)]) -> View {
    pairs.iter().map(|(key, value)| (key.to_string(), value.clone())).collect()
}

pub fn at(ms: u64, device: &str) -> Hlc {
    Hlc(ms, 0, device.into())
}

pub fn doc(pairs: &[(&str, Value, u64)]) -> Doc {
    let fields = pairs.iter().map(|(key, value, ms)| (key.to_string(), (value.clone(), at(*ms, "a"))));
    Doc { fields: fields.collect(), gone: None }
}

pub fn title(text: &str) -> View {
    view(&[("title", json!(text))])
}
