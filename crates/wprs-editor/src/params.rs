use crate::inspect::{Field, SOURCES, candidates};
use crate::layers::walk;
use crate::ops::escape;
use serde_json::{Value, json};
use wprs_scene::{Entry, ParamDef, ParamKind, Text, Variant};

fn source(current: Option<&Value>) -> String {
    SOURCES.iter().find(|s| current.is_some_and(|c| c.get(**s).is_some())).map_or("", |s| *s).into()
}

fn local(texts: &std::collections::BTreeMap<String, String>) -> String {
    match texts.is_empty() {
        true => String::new(),
        false => Text::Localized(texts.clone()).pick(&wprs_core::locale::languages()),
    }
}

pub(crate) fn param(doc: &Value, p: &ParamDef, value: &Value, base: &str, depth: usize) -> Field {
    let current = value["params"].get(&p.id);
    let (widget, reference) = match p.kind {
        ParamKind::Float => ("number", None),
        ParamKind::Vec2 | ParamKind::Position => ("vec2", None),
        ParamKind::Color => ("color", None),
        ParamKind::Bool => ("bool", None),
        ParamKind::Texture | ParamKind::Mask => ("ref:image", Some("image")),
        ParamKind::Lut => ("ref:lut", Some("lut")),
        ParamKind::Int => ("integer", None),
        ParamKind::Enum | ParamKind::Blend => ("enum", None),
    };
    let listed = match p.kind {
        ParamKind::Blend => wprs_scene::BLENDS.iter().filter_map(|b| serde_json::to_value(b).ok()).collect(),
        _ => p.values.iter().map(|e| json!(e.value)).collect(),
    };
    let mut options = reference.map(|r| candidates(doc, r, base)).unwrap_or(listed);
    if reference == Some("image") {
        let own = base.split("/effects/").next().and_then(|l| doc.pointer(l)).and_then(|l| l["id"].as_str());
        let layers = walk(doc).into_iter().filter_map(|(_, l)| l["id"].as_str().map(String::from));
        options.extend(layers.filter(|id| Some(id.as_str()) != own).map(|id| json!({ "layer": id })));
    }
    let labeled = p.kind == ParamKind::Enum && p.values.iter().all(|e| !e.label.is_empty());
    let names = if labeled { p.values.iter().map(|e: &Entry| local(&e.label)).collect() } else { Vec::new() };
    Field {
        pointer: format!("{base}/{}", escape(&p.id)),
        key: p.id.clone(),
        widget: widget.into(),
        value: current.cloned().unwrap_or_default(),
        default: serde_json::to_value(&p.default).unwrap_or_default(),
        set: current.is_some(),
        options,
        range: p.min.zip(p.max).map_or(Value::Null, |(a, b)| json!([a, b])),
        bindable: reference.is_none(),
        source: source(current),
        depth,
        names,
        title: local(&p.label),
        help: local(&p.help),
        ..Field::default()
    }
}

pub(crate) fn option(o: &Variant, value: &Value, base: &str, depth: usize) -> Field {
    let current = value["params"].get(&o.id);
    Field {
        pointer: format!("{base}/{}", escape(&o.id)),
        key: o.id.clone(),
        widget: "enum".into(),
        value: current.cloned().unwrap_or_default(),
        default: serde_json::to_value(&o.default).unwrap_or_default(),
        set: current.is_some(),
        options: o.values.iter().filter_map(|v| serde_json::to_value(v).ok()).collect(),
        bindable: true,
        source: source(current),
        depth,
        ..Field::default()
    }
}
