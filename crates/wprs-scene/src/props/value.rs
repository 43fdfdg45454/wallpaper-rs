use super::{Choice, Condition, Control, Property, Values};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

pub(super) const MAX_TEXT: usize = 1024;

impl Choice {
    pub fn value(&self) -> &Json {
        match self {
            Choice::Plain(value) | Choice::Labeled { value, .. } => value,
        }
    }

    pub fn icon(&self) -> Option<&str> {
        match self {
            Choice::Labeled { icon, .. } => icon.as_deref(),
            Choice::Plain(_) => None,
        }
    }
}

pub(super) fn same(a: &Json, b: &Json) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

impl Property {
    pub fn new(control: Control) -> Property {
        let (visible_if, enabled_if) = (Condition::default(), Condition::default());
        Property {
            label: None,
            help: None,
            group: None,
            visible_if,
            enabled_if,
            order: 0.0,
            icon: None,
            quality: BTreeMap::new(),
            control,
        }
    }

    pub fn default_value(&self) -> Json {
        match &self.control {
            Control::Color { default, .. } | Control::Text { default, .. } => json!(default),
            Control::Combo { default, .. } => default.clone(),
            Control::Slider(s) if s.integer => json!(s.default.round() as i64),
            Control::Slider(s) => json!(s.default),
            Control::Bool { default } => json!(default),
            Control::File { default_asset, .. } => default_asset.as_ref().map_or(Json::Null, |a| json!(a)),
            _ => Json::Null,
        }
    }

    pub fn value(&self, value: &Json) -> Result<Json, String> {
        let wrong = || format!("must be a {} value", self.kind());
        let path = |s: &str| s.starts_with('/') && !s.contains('\0');
        match (&self.control, value) {
            (Control::Color { alpha, .. }, Json::String(s)) => {
                let hex = s.strip_prefix('#').filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit()));
                let fits = hex.is_some_and(|h| h.len() == 6 || (*alpha && h.len() == 8));
                fits.then(|| json!(s.to_ascii_lowercase())).ok_or_else(|| "must be a #rrggbb color".into())
            }
            (Control::Slider(s), Json::Number(n)) => {
                let v = s.fit(n.as_f64().filter(|v| v.is_finite()).ok_or_else(wrong)?);
                Ok(if s.integer { json!(v.round() as i64) } else { json!(v) })
            }
            (Control::Bool { .. }, Json::Bool(b)) => Ok(json!(b)),
            (Control::Combo { options, .. }, given) => {
                let found = options.iter().find(|o| same(o.value(), given));
                found.map(|o| o.value().clone()).ok_or_else(|| format!("{given} is not one of the options"))
            }
            (Control::Text { max_len, multiline, .. }, Json::String(s)) => {
                let max = max_len.map_or(MAX_TEXT, |m| m as usize);
                let clean = s.chars().all(|c| !c.is_control() || (*multiline && c == '\n'));
                let ok = clean && s.chars().count() <= max;
                ok.then(|| json!(s))
                    .ok_or_else(|| format!("must have at most {max} characters and no control characters"))
            }
            (Control::File { .. } | Control::Folder { .. }, Json::Null) => Ok(Json::Null),
            (Control::File { .. } | Control::Folder { .. }, Json::String(s)) if path(s) => Ok(json!(s)),
            (Control::Shortcut {}, Json::Null) => Ok(Json::Null),
            (Control::Shortcut {}, Json::Object(map)) => {
                let target = map.iter().next().filter(|_| map.len() == 1);
                let app = |s: &str| {
                    (1..=255).contains(&s.len())
                        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                };
                let fits = target.is_some_and(|(kind, v)| match (kind.as_str(), v.as_str()) {
                    ("uri", Some(s)) => {
                        s.starts_with("https://") && s.len() <= 2048 && !s.chars().any(char::is_control)
                    }
                    ("file" | "folder", Some(s)) => path(s),
                    ("app", Some(s)) => app(s),
                    _ => false,
                });
                fits.then(|| value.clone()).ok_or_else(|| "must be {uri}, {file}, {folder} or {app}".into())
            }
            (Control::Button {} | Control::Group { .. } | Control::Label {} | Control::Divider {}, _) => {
                Err("takes no value".into())
            }
            _ => Err(wrong()),
        }
    }

    pub(crate) fn kept(&self, value: &Json) -> Option<Json> {
        let inside = match (&self.control, value.as_f64()) {
            (Control::Slider(s), Some(v)) => (s.min..=s.max).contains(&v),
            _ => true,
        };
        self.value(value).ok().filter(|_| inside)
    }

    pub fn kind(&self) -> &'static str {
        match self.control {
            Control::Color { .. } => "color",
            Control::Slider(_) => "slider",
            Control::Bool { .. } => "bool",
            Control::Combo { .. } => "combo",
            Control::Text { .. } => "text",
            Control::File { .. } => "file",
            Control::Folder { .. } => "folder",
            Control::Shortcut {} => "shortcut",
            Control::Button {} => "button",
            Control::Group { .. } => "group",
            Control::Label {} => "label",
            Control::Divider {} => "divider",
        }
    }

    pub fn holds_value(&self) -> bool {
        !matches!(
            self.control,
            Control::Button {} | Control::Group { .. } | Control::Label {} | Control::Divider {}
        )
    }

    pub fn personal(&self) -> bool {
        matches!(
            self.control,
            Control::File { .. }
                | Control::Folder { .. }
                | Control::Shortcut {}
                | Control::Text { secret: true, .. }
        )
    }

    pub fn met(
        &self,
        props: &BTreeMap<String, Property>,
        values: &Values,
        pick: fn(&Property) -> &Condition,
    ) -> bool {
        let group = self.group.as_ref().and_then(|g| props.get(g));
        pick(self).holds(props, values) && group.is_none_or(|g| pick(g).holds(props, values))
    }

    pub fn accepts(&self, what: &str) -> bool {
        match &self.control {
            Control::File { accept, .. } | Control::Folder { accept, .. } if !accept.is_empty() => {
                accept.iter().any(|a| a == what)
            }
            Control::File { .. } | Control::Folder { .. } => what == "image",
            _ => false,
        }
    }
}
