use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::actions::{Action, plan};
use wprs_editor::check::check;
use wprs_editor::inspect::{Field, fields};
use wprs_editor::listing::rows;
use wprs_editor::ops::apply;
use wprs_editor::props::PROPERTY_KINDS;
use wprs_editor::settings::condition;
use wprs_editor::starters::starter;
use wprs_editor::{Problem, Project};

mod fields;
mod kinds;

fn state() {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
}

fn action(value: Value) -> Action {
    serde_json::from_value(value).unwrap()
}

fn run(doc: &mut Value, value: Value) {
    let ops = plan(doc, &action(value)).unwrap();
    apply(doc, &ops).unwrap();
}

fn refused(doc: &Value, value: Value) -> String {
    plan(doc, &action(value)).unwrap_err().to_string()
}

fn field(doc: &Value, pointer: &str, key: &str) -> Field {
    fields(doc, pointer).into_iter().find(|f| f.key == key).unwrap_or_else(|| panic!("{pointer} {key}"))
}

fn errors(doc: &Value) -> Vec<Problem> {
    check(doc).into_iter().filter(|p| !p.warning).collect()
}

fn sample() -> Value {
    let mut doc = starter("empty", "org.example.kinds", "Kinds", "en").unwrap();
    doc["manifest"]["properties"] = json!({
        "mood": {"type": "combo", "label": "Mood", "options": [{"value": "calm", "label": "Calm"}, {"value": "storm", "label": "Storm"}],
            "default": "calm", "order": 1},
        "speed": {"type": "slider", "min": 0.5, "max": 4, "step": 0.5, "default": 1, "unit": "x", "order": 2,
            "visible_if": "mood == 'storm' && tint != '#000000'"},
        "go": {"type": "button", "enabled_if": {"mood": "storm"}, "group": "tools", "order": 1},
        "tools": {"type": "group", "order": 3},
        "tint": {"type": "color", "default": "#ffffff"},
        "photo": {"type": "file", "accept": ["image"]},
        "secret": {"type": "text", "secret": true, "visible_if": "speed > 1"}
    });
    doc["manifest"]["presets"] = json!([{"id": "wild", "label": "Wild", "values": {"mood": "storm"}}]);
    doc["scene"]["assets"] =
        json!({"calm": {"path": "assets/calm.png"}, "storm": {"path": "assets/storm.png"}});
    doc["scene"]["timelines"] = json!([{"id": "pulse", "duration": 1, "trigger": "prop:go"}]);
    doc["scene"]["layers"] =
        json!([{"id": "sky", "type": "image", "image": {"prop": "mood", "map": {"calm": "calm"}}}]);
    doc
}
