use crate::{Binding, EffectUse, Kind, Layer, Quality, Scene, ScriptApi, ScriptEvent, Use, Values};
use serde_json::Value as Json;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Uses {
    pub audio: bool,
    pub cursor: bool,
    pub clicks: bool,
    pub scripts: bool,
    pub media: bool,
}

fn declared(scene: &Scene) -> Uses {
    let wants = |api| scene.scripts.iter().any(|s| s.wants(api));
    let handles =
        |pick: fn(ScriptEvent) -> bool| scene.scripts.iter().flat_map(|s| &s.events).any(|e| pick(*e));
    let sounds = scene.sounds().filter_map(|s| s.play_on.as_deref());
    let triggers: Vec<&str> =
        scene.timelines.iter().filter_map(|t| t.trigger.as_deref()).chain(sounds).collect();
    let fired =
        |kinds: &[&str]| triggers.iter().any(|t| t.split_once(':').is_some_and(|(k, _)| kinds.contains(&k)));
    let mut reaches = scene.layers().filter_map(|l| l.rig()).flat_map(|r| &r.ik);
    let reaches = reaches.any(|ik| matches!(&ik.target, crate::Reach::Named(n) if n == "cursor"));
    let clicks = wants(ScriptApi::Clicks) || handles(ScriptEvent::clicks) || fired(&["click"]);
    Uses {
        audio: wants(ScriptApi::Audio),
        cursor: clicks
            || reaches
            || wants(ScriptApi::Pointer)
            || handles(ScriptEvent::pointer)
            || fired(&["enter", "leave"]),
        clicks,
        scripts: false,
        media: wants(ScriptApi::Media) || handles(ScriptEvent::media) || fired(&["media"]),
    }
}

pub(crate) fn of(scene: &Scene, props: &Values) -> [Uses; 4] {
    levels(scene, Some(props))
}

fn levels(scene: &Scene, props: Option<&Values>) -> [Uses; 4] {
    let mut json = serde_json::to_value(scene).unwrap_or_default();
    if let Some(root) = json.as_object_mut() {
        root.remove("audio");
    }
    let listens = scene.layers().any(|l| matches!(l.kind, Kind::Spectrum(_)))
        || scene.timelines.iter().any(|t| t.trigger.as_deref() == Some("beat"));
    let follows = scene.canvas.parallax.as_ref().is_none_or(|f| f.pointer > 0.0);
    let parallax = follows && scene.layers().any(|l| !l.parallax.is_zero());
    let heard = scene.particles.values().map(|s| s.listens());
    let (beat, pointer, clicks) =
        heard.fold((false, false, false), |a, b| (a.0 || b.0, a.1 || b.1, a.2 || b.2));
    let said = declared(scene);
    let base = Uses {
        audio: listens || beat || said.audio || reads(&json, "audio"),
        cursor: parallax || pointer || said.cursor || reads(&json, "cursor"),
        clicks: clicks || said.clicks,
        scripts: !scene.scripts.is_empty(),
        media: said.media || scene.assets.values().any(|a| a.media.is_some()) || reads(&json, "media"),
    };
    [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3].map(|q| effects(scene, q, base, props))
}

pub fn needs(scene: &Scene) -> Uses {
    let levels = levels(scene, None);
    let any = |pick: fn(&Uses) -> bool| levels.iter().any(pick);
    let scripts = !scene.scripts.is_empty();
    let (audio, cursor, clicks, media) =
        (any(|u| u.audio), any(|u| u.cursor), any(|u| u.clicks), any(|u| u.media));
    Uses { audio, cursor, clicks, scripts, media }
}

pub fn source(scene: &Scene, pick: fn(&Uses) -> bool) -> Option<String> {
    let (layers, particles, timelines, scripts, assets) = Default::default();
    let bare = Scene { layers, particles, timelines, scripts, assets, ..scene.clone() };
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
    let scripts = || {
        let one = |s: &crate::Script| uses(Scene { scripts: vec![s.clone()], ..bare.clone() });
        scene.scripts.iter().position(one).map(|i| format!("/scripts/{i}"))
    };
    let assets = || {
        let one = |(id, a): &(&String, &crate::Asset)| {
            uses(Scene { assets: [((*id).clone(), (*a).clone())].into(), ..bare.clone() })
        };
        scene.assets.iter().find(one).map(|(id, _)| format!("/assets/{id}"))
    };
    layer.or_else(particles).or_else(timelines).or_else(scripts).or_else(assets)
}

pub fn tells_time(scene: &Scene) -> bool {
    let layers = serde_json::to_value(&scene.layers).unwrap_or_default();
    reads(&layers, "clock") || reads(&layers, "date")
}

fn effects(scene: &Scene, quality: Quality, base: Uses, props: Option<&Values>) -> Uses {
    let shown = |e: &EffectUse| e.enabled.settled(props) != Some(false);
    let drawn = |e: &&EffectUse| {
        scene.effect(&e.effect).filter(|d| shown(e) && quality.effect_scale(d.class(), e.optional).is_some())
    };
    let kept: Vec<_> =
        scene.layers().flat_map(|l| &l.effects).filter_map(|e| Some((drawn(&e)?, e))).collect();
    let binds = |e: &EffectUse, key: &str| {
        let values = e.params.values().filter_map(|v| serde_json::to_value(v).ok());
        values.chain(serde_json::to_value(&e.enabled).ok()).any(|j| reads(&j, key))
    };
    let any = |u: Use, key: &str| kept.iter().any(|(d, e)| d.needs(u, e, props) || binds(e, key));
    let clicks = base.clicks || kept.iter().any(|(d, e)| d.needs(Use::Clicks, e, props));
    Uses {
        audio: base.audio || any(Use::Audio, "audio"),
        cursor: base.cursor || clicks || any(Use::Cursor, "cursor"),
        clicks,
        media: base.media || kept.iter().any(|(_, e)| binds(e, "media")),
        ..base
    }
}

fn reads(json: &Json, key: &str) -> bool {
    let values = |v: &Json| v.as_object().is_some_and(|m| m.values().any(|v| reads(v, key)));
    match json {
        Json::Object(map) => {
            let bound =
                map.contains_key(key) && serde_json::from_value::<Binding<Json>>(json.clone()).is_ok();
            bound
                || map.iter().any(|(k, v)| match k.as_str() {
                    "effects" | "map" | "ext" | "expressions" | "limits" => false,
                    "params" | "shapes" => values(v),
                    _ => reads(v, key),
                })
        }
        Json::Array(items) => items.iter().any(|v| reads(v, key)),
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
