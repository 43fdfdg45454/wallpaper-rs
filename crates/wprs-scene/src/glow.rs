use crate::look::Bloom;
use crate::value::{Param, Value};
use crate::{Canvas, EffectUse, Kind, Layer, Quality};
use serde_json::{Value as Json, json};

pub(crate) type Glows = [Option<Box<Layer>>; 2];

fn param(v: Json) -> Option<Value<Param>> {
    serde_json::from_value(v).ok()
}

fn layer(b: &Bloom, threshold: Json, strength: Json) -> Box<Layer> {
    let radius = serde_json::to_value(&b.radius).unwrap_or(json!(16));
    let tint = serde_json::to_value(&b.tint).unwrap_or(json!("#ffffff"));
    let params = [("threshold", threshold), ("intensity", strength), ("radius", radius), ("tint", tint)];
    let params = params.into_iter().filter_map(|(k, v)| Some((k.to_owned(), param(v)?))).collect();
    let (effect, name, enabled, mask, optional) = ("bloom".into(), None, Value::Fixed(true), None, false);
    let mut layer = Layer::new("", Kind::Adjustment { size: None, redraw: false, rgb: false });
    layer.effects = vec![EffectUse { effect, name, enabled, mask, params, optional }];
    Box::new(layer)
}

pub(crate) fn glows(canvas: &Canvas) -> Glows {
    let Some(b) = &canvas.bloom else { return [None, None] };
    let json = |v: &Value<f32>| serde_json::to_value(v).unwrap_or(Json::Null);
    let hdr = b.hdr.map(|h| layer(b, json!(h.threshold), json!(h.strength)));
    [Some(layer(b, json(&b.threshold), json(&b.strength))), hdr]
}

pub(crate) fn pick(glows: &Glows, quality: Quality, battery: bool) -> Option<&Layer> {
    let hdr = quality == Quality::Q3 || (quality == Quality::Q2 && !battery);
    let [base, high] = glows;
    high.as_deref().filter(|_| hdr).or(base.as_deref())
}
