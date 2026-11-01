mod steps;

use image::codecs::gif::GifEncoder;
use image::{Delay, Frame, Rgba, RgbaImage};
use serde_json::{Value, json};
use std::fs;
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::actions::{Action, plan};
use wprs_editor::check::{check, check_files};
use wprs_editor::inspect::fields;
use wprs_editor::meta;
use wprs_editor::ops::apply;
use wprs_editor::package::build;
use wprs_editor::starters::starter;
use wprs_editor::{Problem, Project};

pub fn state() {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
}

pub fn action(value: Value) -> Action {
    serde_json::from_value(value).unwrap()
}

pub fn run(doc: &mut Value, value: Value) {
    let ops = plan(doc, &action(value)).unwrap();
    apply(doc, &ops).unwrap();
}

pub fn at(problems: &[Problem], pointer: &str) -> bool {
    problems.iter().any(|p| p.pointer == pointer && !p.warning)
}

#[test]
fn texts_are_translated_where_they_live() {
    let mut doc = starter("empty", "org.example.dusk", "Dusk", "en").unwrap();
    let title =
        |text: Value| json!({"translate": {"pointer": "/manifest/title", "language": "es", "text": text}});
    run(&mut doc, title(json!("Atardecer")));
    assert_eq!(doc["manifest"]["title"], json!({"en": "Dusk", "es": "Atardecer"}));
    let named =
        |languages: &[&str]| meta::title(&doc, &languages.iter().map(|l| l.to_string()).collect::<Vec<_>>());
    assert_eq!((named(&["es_ES.UTF-8"]), named(&[])), ("Atardecer".into(), "Dusk".into()));
    run(&mut doc, json!({"translate": {"pointer": "/manifest/title", "language": "", "text": "Evening"}}));
    run(&mut doc, title(Value::Null));
    assert_eq!(doc["manifest"]["title"], json!({"en": "Evening"}));
    let last = action(json!({"translate": {"pointer": "/manifest/title", "language": "en"}}));
    assert!(plan(&doc, &last).is_err(), "the title keeps a text");
    let description = |language: &str, text: &str| json!({"translate": {"pointer": "/manifest/description", "language": language, "text": text}});
    run(&mut doc, description("", "Hills at **dusk**."));
    assert_eq!(doc["manifest"]["description"], "Hills at **dusk**.");
    run(&mut doc, description("", ""));
    assert!(doc["manifest"].get("description").is_none());
    let changes = json!({"translate": {"pointer": "/manifest/changes", "language": "de", "text": "Neu"}});
    assert!(plan(&doc, &action(changes)).is_err(), "a text starts in the default language");
    doc["manifest"]["changes"] = json!({"de": "Neu"});
    assert!(at(&check(&doc), "/manifest/changes"), "a text needs the default language");
    run(&mut doc, json!({"remove": {"pointer": "/manifest/changes"}}));
    doc["manifest"]["strings"] = json!({"en": {"about": "Hills"}});
    doc["manifest"]["description"] = json!("@about");
    run(&mut doc, description("es", "Colinas"));
    assert_eq!(doc["manifest"]["strings"]["es"]["about"], "Colinas");
    let shown = fields(&doc, "/manifest");
    let rows = &shown.iter().find(|f| f.key == "description").unwrap().options;
    assert_eq!(
        rows[1],
        json!({"language": "es", "text": "Colinas", "pointer": "/manifest/strings/es/about"})
    );
    assert_eq!(check(&doc), []);
}

#[test]
fn adding_a_language_never_erases_a_text() {
    let mut doc = starter("empty", "org.example.mi", "Mi escena", "es").unwrap();
    assert_eq!(doc["manifest"]["default_locale"], "es");
    let add = |pointer: &str, language: &str| json!({"translate": {"pointer": pointer, "language": language, "add": true}});
    let put = |pointer: &str, language: &str, text: &str| json!({"translate": {"pointer": pointer, "language": language, "text": text}});
    run(&mut doc, add("/manifest/title", "es"));
    assert_eq!(doc["manifest"]["title"], "Mi escena");
    run(&mut doc, add("/manifest/title", "en"));
    assert_eq!(doc["manifest"]["title"], json!({"es": "Mi escena", "en": "Mi escena"}));
    run(&mut doc, put("/manifest/title", "en", "My scene"));
    run(&mut doc, add("/manifest/title", "en"));
    assert_eq!(doc["manifest"]["title"], json!({"es": "Mi escena", "en": "My scene"}));
    run(&mut doc, put("/manifest/description", "", "Colinas"));
    run(&mut doc, add("/manifest/description", "es"));
    assert_eq!(doc["manifest"]["description"], "Colinas");
    let shown = fields(&doc, "/manifest");
    let rows = &shown.iter().find(|f| f.key == "description").unwrap().options;
    assert_eq!((&rows[0]["language"], rows.len()), (&json!("es"), 1));
    run(&mut doc, put("/manifest/title", "en", ""));
    assert_eq!(doc["manifest"]["title"], json!({"es": "Mi escena"}));
    assert!(plan(&doc, &action(put("/manifest/title", "es", " "))).is_err(), "the title keeps a text");
    assert!(plan(&doc, &action(add("/manifest/changes", "en"))).is_err(), "nothing to translate yet");
    for pointer in ["/manifest/version", "/manifest/inputs", "/manifest/requires/features", "/scene/canvas"] {
        assert!(plan(&doc, &action(put(pointer, "es", "x"))).is_err(), "{pointer} is not a text");
    }
    assert_eq!(check(&doc), []);
    let french = starter("particles", "org.example.fr", "Ma scène", "fr").unwrap();
    assert_eq!(french["manifest"]["default_locale"], "fr", "the title is in French");
    assert_eq!(french["manifest"]["title"], "Ma scène");
    let density = &french["manifest"]["properties"]["density"]["label"];
    assert_eq!((density["fr"].as_str(), density["es"].as_str()), (Some("Density"), Some("Densidad")));
    assert_eq!(check(&french), []);
    assert!(starter("empty", "org.example.x", "X", "not a language").is_err());
}

#[test]
fn the_inspector_shows_the_manifest_from_its_schema() {
    let mut doc = starter("parallax", "org.example.dawn", "Dawn", "en").unwrap();
    run(&mut doc, json!({"set": {"pointer": "/manifest/previews", "value": {}}}));
    run(&mut doc, json!({"translate": {"pointer": "/manifest/title", "language": "es", "text": "Alba"}}));
    let shown = fields(&doc, "/manifest");
    let top: Vec<&str> = shown.iter().filter(|f| f.depth == 0).map(|f| f.key.as_str()).collect();
    assert_eq!(top[..5], ["title", "description", "changes", "version", "id"]);
    assert!(!top.iter().any(|k| ["schema", "kind", "entry", "properties", "strings", "preview"].contains(k)));
    let field = |key: &str| shown.iter().find(|f| f.key == key).unwrap_or_else(|| panic!("{key}"));
    let widgets =
        ["title", "description", "authors", "inputs", "descriptors", "rating", "features", "thumbnail"]
            .map(|key| field(key).widget.as_str());
    assert_eq!(widgets, ["texts", "markup", "lines", "flags", "flags", "enum", "computed", "ref:thumbnail"]);
    assert_eq!((field("inputs").options.len(), &field("inputs").fixed), (5, &vec![json!("pointer")]));
    assert!(field("photosensitive").required && field("content").required && !field("links").required);
    assert_eq!(field("features").value, json!(["inputs.pointer"]));
    assert_eq!((field("kind").widget.as_str(), field("derived_from").widget.as_str()), ("enum", "list"));
    let locale = field("default_locale");
    assert_eq!((locale.widget.as_str(), locale.required), ("ref:language", true));
    assert!(locale.options.contains(&json!("en")) && locale.options.contains(&json!("es")));
    let display =
        |doc: &Value| fields(doc, "/manifest/display").into_iter().map(|f| f.key).collect::<Vec<_>>();
    doc["manifest"]["display"] = json!({});
    assert_eq!(display(&doc), ["accent", "span"], "scenes are not framed");
    doc["manifest"]["display"] = json!({"fit": "tile"});
    assert_eq!(display(&doc), ["accent", "fit", "span"], "a value set by hand can still be removed");
    let playback =
        |doc: &Value| fields(doc, "/manifest/playback").into_iter().map(|f| f.key).collect::<Vec<_>>();
    doc["manifest"]["playback"] = json!({});
    assert_eq!(playback(&doc), ["max_fps", "muted", "volume"], "scenes only limit their rate and sound");
    doc["manifest"]["kind"] = json!("video");
    assert_eq!(playback(&doc).len(), 7, "{:?}", playback(&doc));
}
