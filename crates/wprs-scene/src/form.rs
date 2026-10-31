use crate::props::{Choice, Control, ICONS, Preset, Property, Values};
use serde_json::Value as Json;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use wprs_core::model::{SceneField, SceneOption};

type Props = BTreeMap<String, Property>;

fn icon(name: Option<&str>) -> String {
    name.filter(|n| ICONS.split_whitespace().any(|i| i == *n)).unwrap_or_default().to_owned()
}

pub fn order(props: &Props, (a, p): (&String, &Property), (b, q): (&String, &Property)) -> Ordering {
    let key = |id: &String, p: &Property| match p.group.as_ref().and_then(|g| props.get_key_value(g)) {
        Some((g, group)) => (group.order, g.clone(), true, p.order, id.clone()),
        None => (p.order, id.clone(), false, 0.0, id.clone()),
    };
    let (x, y) = (key(a, p), key(b, q));
    let first = x.0.total_cmp(&y.0).then_with(|| x.1.cmp(&y.1)).then(x.2.cmp(&y.2));
    first.then(x.3.total_cmp(&y.3)).then_with(|| x.4.cmp(&y.4))
}

pub fn form(
    props: &Props,
    presets: &[Preset],
    values: &Values,
    languages: &[String],
) -> (Vec<SceneField>, Vec<SceneOption>) {
    let text =
        |t: &Option<crate::Text>, id: &str| t.as_ref().map_or_else(|| id.to_owned(), |t| t.pick(languages));
    let mut list: Vec<_> = props.iter().collect();
    list.sort_by(|a, b| order(props, *a, *b));
    let field = |(id, p): (&String, &Property)| {
        let value = values.get(id).cloned().unwrap_or(Json::Null);
        let chosen = |v: Json| if v.as_str().is_some_and(|s| s.starts_with('/')) { v } else { Json::Null };
        let mut f = SceneField {
            key: id.clone(),
            kind: p.kind().into(),
            label: match (&p.control, &p.label) {
                (Control::Divider {}, None) => String::new(),
                _ => text(&p.label, id),
            },
            help: p.help.as_ref().map(|h| h.pick(languages)).unwrap_or_default(),
            value,
            visible: p.met(props, values, |p| &p.visible_if),
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
            enabled: p.met(props, values, |p| &p.enabled_if),
            group: p.group.clone().unwrap_or_default(),
            icon: icon(p.icon.as_deref()),
            decimals: None,
            editable: false,
            log: false,
            collapsed: false,
            secret: false,
            filled: false,
        };
        match &p.control {
            Control::Color { alpha, .. } => f.alpha = *alpha,
            Control::Slider(s) => {
                (f.min, f.max, f.step, f.integer) = (Some(s.min), Some(s.max), Some(s.stride()), s.integer);
                (f.decimals, f.editable, f.log) = (Some(s.places()), s.editable, s.log);
                f.unit = s.unit.clone().unwrap_or_default();
            }
            Control::Combo { options, .. } => {
                f.options = options.iter().map(|o| option(o, languages)).collect()
            }
            Control::Text { max_len, multiline, secret, .. } => {
                (f.max_len, f.multiline, f.secret) = (*max_len, *multiline, *secret);
                if *secret {
                    f.value = Json::Null;
                }
            }
            Control::File { .. } | Control::Folder { .. } => {
                f.accept =
                    ["image", "video"].into_iter().filter(|a| p.accepts(a)).map(String::from).collect();
                f.value = chosen(f.value);
            }
            Control::Group { collapsed } => f.collapsed = *collapsed,
            _ => {}
        }
        f
    };
    let labels = presets.iter().map(|p| SceneOption {
        value: Json::String(p.id.clone()),
        label: text(&p.label, &p.id),
        icon: String::new(),
    });
    (list.into_iter().map(field).collect(), labels.collect())
}

pub fn labels(props: &Props, languages: &[String]) -> Json {
    let named = |options: &Vec<Choice>| {
        let pairs = options
            .iter()
            .map(|o| option(o, languages))
            .map(|o| (crate::resolve::binding::key(&o.value), Json::String(o.label)));
        Json::Object(pairs.collect())
    };
    let combos = props.iter().filter_map(|(id, p)| match &p.control {
        Control::Combo { options, .. } => Some((id.clone(), named(options))),
        _ => None,
    });
    Json::Object(combos.collect())
}

fn option(choice: &Choice, languages: &[String]) -> SceneOption {
    let value = choice.value().clone();
    let label = match choice {
        Choice::Labeled { label, .. } => label.pick(languages),
        Choice::Plain(_) => String::new(),
    };
    let label = if label.is_empty() { crate::resolve::binding::key(&value) } else { label };
    SceneOption { value, label, icon: icon(choice.icon()) }
}
