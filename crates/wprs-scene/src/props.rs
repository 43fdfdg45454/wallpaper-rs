use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

pub type Values = BTreeMap<String, Json>;

const MAX_TEXT: usize = 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Text {
    Plain(String),
    Localized(BTreeMap<String, String>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Property {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub visible_if: Values,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub order: i32,
    #[serde(flatten)]
    pub control: Control,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Control {
    Color {
        default: String,
        #[serde(default)]
        alpha: bool,
    },
    Slider {
        min: f64,
        max: f64,
        step: f64,
        default: f64,
        #[serde(default)]
        integer: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit: Option<String>,
    },
    Bool {
        default: bool,
    },
    Combo {
        options: Vec<Choice>,
        default: String,
    },
    Text {
        default: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_len: Option<u32>,
        #[serde(default)]
        multiline: bool,
    },
    File {
        accept: Vec<String>,
    },
    Group {},
    Label {},
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Choice {
    Plain(String),
    Labeled { value: String, label: Text },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<Text>,
    pub values: Values,
}

impl Choice {
    pub fn value(&self) -> &str {
        match self {
            Choice::Plain(value) | Choice::Labeled { value, .. } => value,
        }
    }
}

impl Property {
    pub fn new(control: Control) -> Property {
        Property { label: None, help: None, group: None, visible_if: Values::new(), order: 0, control }
    }

    pub fn default_value(&self) -> Json {
        match &self.control {
            Control::Color { default, .. }
            | Control::Text { default, .. }
            | Control::Combo { default, .. } => {
                json!(default)
            }
            Control::Slider { default, .. } => json!(default),
            Control::Bool { default } => json!(default),
            Control::File { .. } | Control::Group {} | Control::Label {} => Json::Null,
        }
    }

    pub fn check(&self) -> Result<(), String> {
        match &self.control {
            Control::Slider { min, max, step, default, .. } => {
                let finite = [min, max, step, default].iter().all(|v| v.is_finite());
                let range = finite && min < max && *step > 0.0 && *step <= max - min;
                range
                    .then_some(())
                    .ok_or("expected finite values with min < max and 0 < step <= max - min")?;
                let steps = (default - min) / step;
                let aligned = (steps - steps.round()).abs() <= 1e-9 * steps.max(1.0);
                ((min..=max).contains(&default) && aligned)
                    .then_some(())
                    .ok_or("default must lie on a step in range")?;
            }
            Control::Combo { options, .. } => {
                let unique = options.iter().enumerate().all(|(i, o)| {
                    !o.value().is_empty() && !options[..i].iter().any(|p| p.value() == o.value())
                });
                (unique && !options.is_empty())
                    .then_some(())
                    .ok_or("options must be non-empty and unique")?;
            }
            Control::Text { max_len: Some(max), .. } if *max as usize > MAX_TEXT => {
                return Err(format!("max_len must be at most {MAX_TEXT}"));
            }
            _ => {}
        }
        match &self.control {
            Control::File { .. } | Control::Group {} | Control::Label {} => Ok(()),
            _ => self.value(&self.default_value()).map(drop).map_err(|e| format!("default {e}")),
        }
    }

    pub fn value(&self, value: &Json) -> Result<Json, String> {
        let wrong = || format!("must be a {} value", self.kind());
        match (&self.control, value) {
            (Control::Color { alpha, .. }, Json::String(s)) => {
                let hex = s.strip_prefix('#').filter(|h| h.bytes().all(|b| b.is_ascii_hexdigit()));
                let fits = hex.is_some_and(|h| h.len() == 6 || (*alpha && h.len() == 8));
                fits.then(|| json!(s.to_ascii_lowercase())).ok_or_else(|| "must be a #rrggbb color".into())
            }
            (Control::Slider { min, max, integer, .. }, Json::Number(n)) => {
                let v = n.as_f64().filter(|v| v.is_finite()).ok_or_else(wrong)?.clamp(*min, *max);
                Ok(if *integer { json!(v.round() as i64) } else { json!(v) })
            }
            (Control::Bool { .. }, Json::Bool(b)) => Ok(json!(b)),
            (Control::Combo { options, .. }, Json::String(s)) => {
                let known = options.iter().any(|o| o.value() == s);
                known.then(|| json!(s)).ok_or_else(|| format!("{s:?} is not one of the options"))
            }
            (Control::Text { max_len, multiline, .. }, Json::String(s)) => {
                let max = max_len.map_or(MAX_TEXT, |m| m as usize);
                let clean = s.chars().all(|c| !c.is_control() || (*multiline && c == '\n'));
                let ok = clean && s.chars().count() <= max;
                ok.then(|| json!(s))
                    .ok_or_else(|| format!("must have at most {max} characters and no control characters"))
            }
            (Control::File { .. }, Json::Null) => Ok(Json::Null),
            (Control::File { .. }, Json::String(s)) if s.starts_with('/') && !s.contains('\0') => {
                Ok(json!(s))
            }
            _ => Err(wrong()),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self.control {
            Control::Color { .. } => "color",
            Control::Slider { .. } => "slider",
            Control::Bool { .. } => "bool",
            Control::Combo { .. } => "combo",
            Control::Text { .. } => "text",
            Control::File { .. } => "file",
            Control::Group {} => "group",
            Control::Label {} => "label",
        }
    }
}

pub fn check_presets(props: &BTreeMap<String, Property>, presets: &[Preset]) -> Result<(), String> {
    for (id, prop) in props {
        let group = prop.group.as_ref().map(|g| (g, props.get(g).map(|p| &p.control)));
        if let Some((g, _)) = group.filter(|(_, c)| !matches!(c, Some(Control::Group {}))) {
            return Err(format!("{id} names {g}, which is not a group"));
        }
        for (other, want) in &prop.visible_if {
            let target = props.get(other).filter(|_| other != id);
            let target = target.ok_or_else(|| format!("{id} is shown by the unknown property {other}"))?;
            target.value(want).map_err(|e| format!("{id} is shown when {other} {e}"))?;
        }
    }
    for (i, preset) in presets.iter().enumerate() {
        if presets[..i].iter().any(|p| p.id == preset.id) {
            return Err(format!("preset {} is defined twice", preset.id));
        }
        for (id, value) in &preset.values {
            let prop = props
                .get(id)
                .ok_or_else(|| format!("preset {} sets the unknown property {id}", preset.id))?;
            if matches!(prop.control, Control::File { .. }) {
                return Err(format!("preset {} cannot set the file property {id}", preset.id));
            }
            prop.value(value).map_err(|e| format!("preset {}: {id} {e}", preset.id))?;
        }
    }
    Ok(())
}

pub fn effective(props: &BTreeMap<String, Property>, layers: &[&Values]) -> Values {
    let pick = |(id, prop): (&String, &Property)| {
        let set = layers.iter().find_map(|values| prop.value(values.get(id)?).ok());
        (id.clone(), set.unwrap_or_else(|| prop.default_value()))
    };
    props.iter().map(pick).collect()
}

fn is_zero(v: &i32) -> bool {
    *v == 0
}
