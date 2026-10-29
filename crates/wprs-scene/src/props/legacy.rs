use super::{Condition, Control, Mode, Property};
use serde_json::Value as Json;

const COMMON: [&str; 6] = ["type", "label", "help", "group", "visible_if", "order"];

fn fields(kind: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    Some(match kind {
        "color" => (&["default"], &["alpha"]),
        "slider" => (&["min", "max", "step", "default"], &["integer", "unit"]),
        "bool" => (&["default"], &[]),
        "combo" => (&["options", "default"], &[]),
        "text" => (&["default"], &["max_len", "multiline"]),
        "file" => (&["accept"], &[]),
        "group" | "label" => (&[], &[]),
        _ => return None,
    })
}

pub fn legacy(raw: &Json) -> bool {
    let Some(map) = raw.as_object() else { return false };
    let Some((needed, optional)) = map.get("type").and_then(Json::as_str).and_then(fields) else {
        return false;
    };
    let known = |k: &String| [&COMMON[..], needed, optional].iter().any(|set| set.contains(&k.as_str()));
    let order = map.get("order").is_none_or(|o| o.as_i64().is_some_and(|o| i32::try_from(o).is_ok()));
    let option = |o: &Json| o.is_string() || o.get("value").is_some_and(Json::is_string);
    let options = map.get("options").and_then(Json::as_array).is_none_or(|all| all.iter().all(option));
    let combo =
        map.get("type") != Some(&Json::from("combo")) || map.get("default").is_some_and(Json::is_string);
    map.keys().all(known)
        && needed.iter().all(|k| map.contains_key(*k))
        && order
        && map.get("visible_if").is_none_or(Json::is_object)
        && options
        && combo
}

impl Property {
    pub fn features(&self) -> Vec<&'static str> {
        let control = match &self.control {
            Control::Slider(s) => s.step.is_none() || s.decimals.is_some() || s.editable || s.log,
            Control::Combo { options, default } => {
                !default.is_string() || options.iter().any(|o| !o.value().is_string() || o.icon().is_some())
            }
            Control::Text { secret, .. } => *secret,
            Control::File { default_asset, .. } => default_asset.is_some(),
            Control::Group { collapsed } => *collapsed,
            Control::Button {} | Control::Divider {} | Control::Shortcut {} | Control::Folder { .. } => true,
            _ => false,
        };
        let shown = !matches!(self.visible_if, Condition::Equal(_)) || !self.enabled_if.is_empty();
        let newer = control || shown || self.icon.is_some() || self.order.fract() != 0.0;
        let folder = matches!(self.control, Control::Folder { .. });
        let (lists, deeper) = match &self.control {
            Control::Folder { mode, max_files, recursive, .. } => {
                (*mode == Mode::All, *max_files != 1000 || *recursive > 0)
            }
            _ => (false, false),
        };
        let found = [
            (newer, "properties2"),
            (folder, "folder"),
            (self.accepts("video"), "user_videos"),
            (deeper, "folder2"),
            (lists, "folder_lists"),
            (matches!(self.control, Control::Shortcut {}), "shortcut"),
            (!self.quality.is_empty(), "quality_settings"),
        ];
        found.into_iter().filter(|(on, _)| *on).map(|(_, name)| name).collect()
    }
}
