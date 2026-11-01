use crate::sceneprops::Props;
use serde_json::{Value as Json, json};

pub(super) fn color(text: Option<&str>) -> [u8; 3] {
    let hex = text.and_then(|t| t.strip_prefix('#')).and_then(|h| u32::from_str_radix(h, 16).ok());
    hex.map_or([0; 3], |n| [(n >> 16) as u8, (n >> 8) as u8, n as u8])
}

pub fn shape(props: &Props, languages: &[String]) -> Json {
    let kinds: serde_json::Map<String, Json> =
        props.properties.iter().map(|(id, p)| (id.clone(), json!(p.kind()))).collect();
    json!({"kinds": kinds, "labels": wprs_scene::labels(&props.properties, languages)})
}
