use crate::{EffectUse, Kind, Quality, Scene, Use, Values, descriptor};
use serde_json::Value as Json;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Uses {
    pub audio: bool,
    pub cursor: bool,
    pub scripts: bool,
}

pub(crate) fn of(scene: &Scene, props: &Values) -> [Uses; 4] {
    let mut json = serde_json::to_value(scene).unwrap_or_default();
    json["ext"] = Json::Null;
    let listens = scene.layers().any(|l| matches!(l.kind, Kind::Spectrum(_)))
        || scene.timelines.iter().any(|t| t.trigger.as_deref() == Some("beat"));
    let parallax = scene.layers().any(|l| l.parallax != 0.0);
    let (beat, pointer) =
        scene.particles.values().map(|s| s.listens()).fold((false, false), |a, b| (a.0 || b.0, a.1 || b.1));
    let base = Uses {
        audio: listens || beat || has_key(&json, "audio"),
        cursor: parallax || pointer || has_key(&json, "cursor"),
        scripts: !scene.scripts.is_empty(),
    };
    [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3].map(|q| effects(scene, q, base, props))
}

fn effects(scene: &Scene, quality: Quality, base: Uses, props: &Values) -> Uses {
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
    Uses {
        audio: base.audio || any(Use::Audio, "audio"),
        cursor: base.cursor || any(Use::Cursor, "cursor"),
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
