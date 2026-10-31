use crate::meta::sync;
use crate::ops::apply;
use anyhow::{Result, bail, ensure};
use serde_json::{Value, json};
use wprs_core::message::{Message, said};
use wprs_format::valid_tag;

pub const STARTERS: [&str; 4] = ["empty", "particles", "parallax", "clock"];

fn sky(top: &str, bottom: &str) -> Value {
    json!({"id": "sky", "type": "shape", "shape": {"rect": {"size": [1920, 1080]}},
        "gradient": {"stops": [[0, top], [1, bottom]]}})
}

fn hill(id: &str, size: [u32; 2], at: [u32; 2], fill: &str, parallax: f64) -> Value {
    json!({"id": id, "type": "shape", "shape": {"ellipse": {"size": size}}, "fill": fill,
        "transform": {"position": at}, "parallax": parallax})
}

fn layers(starter: &str) -> Result<(Value, Value, Value)> {
    let none = json!({});
    Ok(match starter {
        "empty" => (json!([]), none.clone(), none),
        "particles" => {
            let system = json!({"capacity": 3000, "seed": 7, "prewarm": 4,
                "emitters": [{"shape": {"box": [[0, -40], [1920, 0]]}, "rate": {"prop": "density"}}],
                "init": {"life": [8, 14], "speed": [40, 90], "angle": [80, 100], "size": [3, 9],
                    "color": ["#ffffff", "#bcd4ff"]},
                "forces": [{"turbulence": {"strength": 24, "scale": 0.003}}, {"drag": 0.1}],
                "render": {"size_over_life": [[0, 0], [0.1, 1], [0.9, 1], [1, 0]]}});
            let props = json!({"density": {"type": "slider", "label": {"en": "Density", "es": "Densidad"},
                "min": 0, "max": 400, "step": 10, "default": 160}});
            let layers = json!([sky("#0b1830", "#2c4a70"), {"id": "flakes", "type": "particles",
                "system": "flakes", "blend": "add"}]);
            (layers, json!({"flakes": system}), props)
        }
        "parallax" => {
            let sun = json!({"id": "sun", "type": "shape", "shape": {"ellipse": {"size": [220, 220]}},
                "fill": "#ffe7b0", "transform": {"position": [1380, 330]}, "parallax": 0.005});
            let layers = json!([
                sky("#f3a96b", "#fbe3c4"),
                sun,
                hill("far", [2600, 700], [600, 1000], "#c97f6b", 0.01),
                hill("middle", [2400, 600], [1500, 1080], "#8f4f5a", 0.025),
                hill("near", [2800, 520], [700, 1180], "#4b2a3f", 0.05)
            ]);
            (layers, none.clone(), none)
        }
        "clock" => {
            let time = json!({"id": "time", "type": "text", "text": {"clock": "HH:mm"}, "size": 220,
                "color": {"prop": "color"}, "anchor": "center"});
            let date = json!({"id": "seconds", "type": "text", "text": {"clock": "ss"}, "size": 64,
                "color": {"prop": "color"}, "opacity": 0.6, "anchor": "center", "transform": {"position": [0, 170]}});
            let props = json!({"color": {"type": "color", "label": {"en": "Color", "es": "Color"},
                "default": "#f2f5ff"}});
            (json!([sky("#101828", "#283a5c"), time, date]), none, props)
        }
        _ => bail!(said(
            "editor.not_starting_point",
            "%1 is not a starting point; use one of %2",
            &[&starter, &(STARTERS.join(", "))]
        )),
    })
}

pub fn locale() -> String {
    let languages = wprs_core::locale::languages();
    let first = languages.first().and_then(|l| l.split(['.', '@', '_', '-']).next()).unwrap_or_default();
    Some(first.to_ascii_lowercase()).filter(|l| valid_tag(l)).unwrap_or_else(|| "en".into())
}

pub fn starter(name: &str, id: &str, title: &str, locale: &str) -> Result<Value> {
    ensure!(
        valid_tag(locale),
        said("editor.not_language_such", "%1 is not a language such as en or pt-BR", &[&locale])
    );
    let (layers, particles, mut properties) = layers(name)?;
    let labels = properties.as_object_mut().into_iter().flat_map(|p| p.values_mut());
    for label in labels.filter_map(|p| p["label"].as_object_mut()).filter(|l| !l.contains_key(locale)) {
        let english = label["en"].clone();
        label.insert(locale.to_owned(), english);
    }
    let manifest = json!({"schema": wprs_format::SCHEMA, "id": id, "version": "0.1.0", "title": title,
        "default_locale": locale, "license": "CC0-1.0", "origin": {"kind": "original"},
        "content": {"rating": "general"}, "photosensitive": false, "kind": "scene", "entry": "scene.json",
        "properties": properties});
    let mut scene = json!({"schema": wprs_scene::SCHEMA,
        "canvas": {"size": [1920, 1080], "fit": "fill", "background": "#000000"},
        "clock": {"max_fps": "display"}, "layers": layers});
    if particles.as_object().is_some_and(|p| !p.is_empty()) {
        scene["particles"] = particles;
    }
    let mut doc = json!({"manifest": manifest, "scene": scene});
    let ops = sync(&doc, &Default::default(), None);
    apply(&mut doc, &ops)?;
    Ok(doc)
}

const LATIN: [(&str, char); 15] = [
    ("áàâäãåāąă", 'a'),
    ("çćč", 'c'),
    ("ďđ", 'd'),
    ("éèêëēęě", 'e'),
    ("ğ", 'g'),
    ("íìîïīı", 'i'),
    ("łľĺ", 'l'),
    ("ñńň", 'n'),
    ("óòôöõøōő", 'o'),
    ("řŕ", 'r'),
    ("šśş", 's'),
    ("ťţ", 't'),
    ("úùûüūůű", 'u'),
    ("ýÿ", 'y'),
    ("žźż", 'z'),
];

pub(crate) fn fold(c: char) -> char {
    LATIN.iter().find(|(set, _)| set.contains(c)).map_or(c, |(_, base)| *base)
}

pub fn slug(name: &str) -> String {
    let words: String = name.to_lowercase().replace('ß', "ss").replace('æ', "ae").replace('œ', "oe");
    let words: String = words.chars().map(fold).collect();
    let parts: Vec<&str> =
        words.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let joined = parts.join("-");
    match joined.chars().next() {
        Some(c) if c.is_ascii_lowercase() => joined.chars().take(40).collect(),
        Some(_) => format!("s-{}", joined.chars().take(38).collect::<String>()),
        None => "scene".into(),
    }
}

pub fn default_id(folder: &str) -> String {
    format!("org.example.{}", slug(folder))
}

pub fn label(id: &str) -> Option<Message> {
    Some(match id {
        "sky" => said("editor.layer.sky", "Sky", &[]),
        "sun" => said("editor.layer.sun", "Sun", &[]),
        "far" => said("editor.layer.far", "Far hills", &[]),
        "middle" => said("editor.layer.middle", "Middle hills", &[]),
        "near" => said("editor.layer.near", "Near hills", &[]),
        "flakes" => said("editor.layer.flakes", "Snowflakes", &[]),
        "time" => said("editor.layer.time", "Time", &[]),
        "seconds" => said("editor.layer.seconds", "Seconds", &[]),
        _ => return None,
    })
}
