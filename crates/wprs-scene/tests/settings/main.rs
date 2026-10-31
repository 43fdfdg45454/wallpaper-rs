use serde_json::{Value, json};
use std::collections::BTreeMap;
use wprs_scene::*;

mod bindings;
mod conditions;
mod forms;

fn props(value: Value) -> BTreeMap<String, Property> {
    serde_json::from_value(value).unwrap()
}

fn values(value: Value) -> Values {
    serde_json::from_value(value).unwrap()
}

fn sample() -> BTreeMap<String, Property> {
    props(json!({
        "mood": { "type": "combo", "default": 1, "icon": "weather-storm",
                  "options": [{ "value": 1, "label": "Calm", "icon": "weather-clear" }, { "value": 2, "label": "Storm", "icon": "x-nope" }] },
        "speed": { "type": "slider", "min": 0.05, "max": 4, "decimals": 2, "editable": true, "log": true, "default": 1 },
        "glow": { "type": "bool", "default": true, "visible_if": "mood == 2 || speed > 1.5", "group": "more" },
        "more": { "type": "group", "collapsed": true, "order": 2.5 },
        "lines": { "type": "divider", "label": "Lines", "order": 3 },
        "again": { "type": "button", "enabled_if": "!glow && mood != 1", "order": 4 },
        "key": { "type": "text", "secret": true },
        "photo": { "type": "file", "accept": ["image", "video"], "default_asset": "sky" },
        "photos": { "type": "folder", "accept": ["image"], "pick": "sequence", "every": 30 }
    }))
}

fn holds(text: &str, given: Value) -> Result<bool, String> {
    Ok(Expr::parse(text)?.eval(&values(given)) == json!(true))
}
