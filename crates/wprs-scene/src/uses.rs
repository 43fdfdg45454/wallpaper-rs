use crate::{EffectUse, Kind, Layer, Quality, Scene, Use, Values, descriptor};
use serde_json::Value as Json;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Uses {
    pub audio: bool,
    pub cursor: bool,
    pub clicks: bool,
    pub scripts: bool,
}

pub(crate) fn of(scene: &Scene, props: &Values) -> [Uses; 4] {
    levels(scene, Some(props))
}

fn levels(scene: &Scene, props: Option<&Values>) -> [Uses; 4] {
    let mut json = serde_json::to_value(scene).unwrap_or_default();
    json["ext"] = Json::Null;
    let listens = scene.layers().any(|l| matches!(l.kind, Kind::Spectrum(_)))
        || scene.timelines.iter().any(|t| t.trigger.as_deref() == Some("beat"));
    let parallax = scene.layers().any(|l| l.parallax != 0.0);
    let heard = scene.particles.values().map(|s| s.listens());
    let (beat, pointer, clicks) =
        heard.fold((false, false, false), |a, b| (a.0 || b.0, a.1 || b.1, a.2 || b.2));
    let base = Uses {
        audio: listens || beat || has_key(&json, "audio"),
        cursor: parallax || pointer || has_key(&json, "cursor"),
        clicks,
        scripts: !scene.scripts.is_empty(),
    };
    [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3].map(|q| effects(scene, q, base, props))
}

pub fn needs(scene: &Scene) -> Uses {
    let levels = levels(scene, None);
    let any = |pick: fn(&Uses) -> bool| levels.iter().any(pick);
    let scripts = !scene.scripts.is_empty();
    Uses { audio: any(|u| u.audio), cursor: any(|u| u.cursor), clicks: any(|u| u.clicks), scripts }
}

pub fn source(scene: &Scene, pick: fn(&Uses) -> bool) -> Option<String> {
    let bare =
        Scene { layers: Vec::new(), particles: Default::default(), timelines: Vec::new(), ..scene.clone() };
    let uses = |s: Scene| pick(&needs(&s));
    let alone = |layer: Layer| uses(Scene { layers: vec![layer], ..bare.clone() });
    let layer = scene.layers.iter().enumerate().find_map(|(i, l)| {
        if alone(Layer { effects: Vec::new(), ..l.clone() }) {
            return Some(format!("/layers/{i}"));
        }
        let effect = |e: &EffectUse| alone(Layer { effects: vec![e.clone()], ..l.clone() });
        l.effects.iter().position(effect).map(|j| format!("/layers/{i}/effects/{j}"))
    });
    let particles = || {
        let one = |(id, s): &(&String, &crate::System)| {
            uses(Scene { particles: [((*id).clone(), (*s).clone())].into(), ..bare.clone() })
        };
        scene.particles.iter().find(one).map(|(id, _)| format!("/particles/{id}"))
    };
    let timelines = || {
        let one = |t: &crate::Timeline| uses(Scene { timelines: vec![t.clone()], ..bare.clone() });
        scene.timelines.iter().position(one).map(|i| format!("/timelines/{i}"))
    };
    layer.or_else(particles).or_else(timelines)
}

pub fn tells_time(scene: &Scene) -> bool {
    has_key(&serde_json::to_value(&scene.layers).unwrap_or_default(), "clock")
}

fn effects(scene: &Scene, quality: Quality, base: Uses, props: Option<&Values>) -> Uses {
    let shown = |e: &EffectUse| e.enabled.settled(props) != Some(false);
    let drawn = |e: &&EffectUse| {
        descriptor(&e.effect).filter(|d| shown(e) && quality.effect_scale(d.class(), e.optional).is_some())
    };
    let kept: Vec<_> =
        scene.layers().flat_map(|l| &l.effects).filter_map(|e| Some((drawn(&e)?, e))).collect();
    let binds = |e: &EffectUse, key: &str| {
        let values = e.params.values().filter_map(|v| serde_json::to_value(v).ok());
        values.chain(serde_json::to_value(&e.enabled).ok()).any(|j| has_key(&j, key))
    };
    let any = |u: Use, key: &str| kept.iter().any(|(d, e)| d.needs(u, e, props) || binds(e, key));
    let clicks = base.clicks || kept.iter().any(|(d, e)| d.needs(Use::Clicks, e, props));
    Uses {
        audio: base.audio || any(Use::Audio, "audio"),
        cursor: base.cursor || clicks || any(Use::Cursor, "cursor"),
        clicks,
        ..base
    }
}

fn has_key(json: &Json, key: &str) -> bool {
    match json {
        Json::Object(map) => {
            map.contains_key(key) || map.iter().any(|(k, v)| k != "effects" && has_key(v, key))
        }
        Json::Array(items) => items.iter().any(|v| has_key(v, key)),
        _ => false,
    }
}

fn mapped(value: Option<&crate::Value<String>>) -> Vec<&str> {
    match value {
        Some(crate::Value::Bound(b)) => b.map.iter().flatten().map(|(_, id)| id.as_str()).collect(),
        _ => Vec::new(),
    }
}

pub fn variants(scene: &Scene) -> Vec<(String, Vec<&str>)> {
    let mut found = Vec::new();
    for l in scene.layers() {
        let image = match &l.kind {
            Kind::Image { image, .. } => mapped(Some(image)),
            _ => Vec::new(),
        };
        let params = l.effects.iter().flat_map(|e| e.params.values()).filter_map(|v| match v {
            crate::Value::Bound(b) => b.map.as_ref(),
            _ => None,
        });
        let params = params.flat_map(|m| m.values()).filter_map(|p| match p {
            crate::Param::Text(id) => Some(id.as_str()),
            _ => None,
        });
        let masks = l.mask.iter().chain(l.effects.iter().filter_map(|e| e.mask.as_ref()));
        let masks = masks.flat_map(|m| mapped(m.texture.as_ref()));
        found.push((format!("layer {}", l.id), image));
        found.push((format!("the effects of layer {}", l.id), params.collect()));
        found.push((format!("the masks of layer {}", l.id), masks.collect()));
    }
    let sprites =
        scene.particles.iter().map(|(id, s)| (format!("particles {id}"), mapped(s.render.texture.as_ref())));
    found.extend(sprites);
    found.retain(|(_, ids)| ids.len() > 1);
    found
}
