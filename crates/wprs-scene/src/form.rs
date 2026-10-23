use crate::props::{Choice, Control, Preset, Property, Text, Values};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use wprs_core::locale::names;
use wprs_core::model::{SceneField, SceneOption};

impl Text {
    pub fn pick(&self, languages: &[String]) -> String {
        let map = match self {
            Text::Plain(text) => return text.clone(),
            Text::Localized(map) => map,
        };
        let tags: Vec<(String, &String)> = map.iter().map(|(k, v)| (tag(k), v)).collect();
        let find = |name: String| {
            let exact = tags.iter().find(|(k, _)| *k == name);
            let base = || tags.iter().find(|(k, _)| k.split('-').next() == Some(name.as_str()));
            exact.or_else(base).map(|(_, v)| (*v).clone())
        };
        let found = names(languages).chain(["en"]).map(tag).find_map(find);
        found.or_else(|| map.values().next().cloned()).unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Text::Plain(text) => text.is_empty(),
            Text::Localized(map) => map.is_empty(),
        }
    }
}

impl Default for Text {
    fn default() -> Self {
        Text::Plain(String::new())
    }
}

impl From<&str> for Text {
    fn from(text: &str) -> Self {
        Text::Plain(text.into())
    }
}

pub fn tag(name: &str) -> String {
    let tag = name.replace('_', "-").to_ascii_lowercase();
    match tag.as_str() {
        "zh-chs" | "zh-cn" | "zh-sg" => "zh-hans".into(),
        "zh-cht" | "zh-tw" | "zh-hk" | "zh-mo" => "zh-hant".into(),
        _ => tag,
    }
}

pub fn form(
    props: &BTreeMap<String, Property>,
    presets: &[Preset],
    values: &Values,
    languages: &[String],
) -> (Vec<SceneField>, Vec<SceneOption>) {
    let text = |t: &Option<Text>, id: &str| t.as_ref().map_or_else(|| id.to_owned(), |t| t.pick(languages));
    let same = |id: &String, want: &Json| {
        let (want, have) = (props.get(id).and_then(|t| t.value(want).ok()), values.get(id));
        match (want.as_ref().and_then(Json::as_f64), have.and_then(Json::as_f64)) {
            (Some(a), Some(b)) => a == b,
            _ => want.is_some() && want.as_ref() == have,
        }
    };
    let met = |p: &Property| p.visible_if.iter().all(|(id, want)| same(id, want));
    let parent = |p: &Property| props.get_key_value(p.group.as_deref()?);
    let mut list: Vec<_> = props.iter().collect();
    list.sort_by_key(|(id, p)| match parent(p) {
        Some((g, group)) => (group.order, g.as_str(), true, p.order, id.as_str()),
        None => (p.order, id.as_str(), false, 0, id.as_str()),
    });
    let field = |(id, p): (&String, &Property)| {
        let visible = met(p) && parent(p).is_none_or(|(_, g)| met(g));
        let mut f = SceneField {
            key: id.clone(),
            kind: p.kind().into(),
            label: text(&p.label, id),
            help: p.help.as_ref().map(|h| h.pick(languages)).unwrap_or_default(),
            value: values.get(id).cloned().unwrap_or(Json::Null),
            visible,
            min: None,
            max: None,
            step: None,
            integer: false,
            unit: String::new(),
            options: Vec::new(),
            alpha: false,
            multiline: false,
            max_len: None,
            accept: Vec::new(),
        };
        match &p.control {
            Control::Color { alpha, .. } => f.alpha = *alpha,
            Control::Slider { min, max, step, integer, unit, .. } => {
                (f.min, f.max, f.step, f.integer) = (Some(*min), Some(*max), Some(*step), *integer);
                f.unit = unit.clone().unwrap_or_default();
            }
            Control::Combo { options, .. } => {
                f.options = options.iter().map(|o| option(o, languages)).collect()
            }
            Control::Text { max_len, multiline, .. } => (f.max_len, f.multiline) = (*max_len, *multiline),
            Control::File { accept } => f.accept = accept.clone(),
            Control::Bool { .. } | Control::Group {} | Control::Label {} => {}
        }
        f
    };
    let labels = presets.iter().map(|p| SceneOption { value: p.id.clone(), label: text(&p.label, &p.id) });
    (list.into_iter().map(field).collect(), labels.collect())
}

fn option(choice: &Choice, languages: &[String]) -> SceneOption {
    let value = choice.value().to_owned();
    let label = match choice {
        Choice::Plain(_) => value.clone(),
        Choice::Labeled { label, .. } => label.pick(languages),
    };
    SceneOption { value, label }
}
