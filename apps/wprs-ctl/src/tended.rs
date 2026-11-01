use crate::steps::value;
use crate::swarming::Remap;
use crate::tending::{Boids, Collider, Keep, Operator, Rope, Trail};
use serde_json::{Value, json};
use wprs_editor::actions::{ParticleEdit as P, Response, Strand, TendEdit as T};

fn target(t: &str) -> Value {
    let pair = t
        .split_once(',')
        .and_then(|(x, y)| Some([x.trim().parse::<f64>().ok()?, y.trim().parse::<f64>().ok()?]));
    pair.map_or_else(|| json!(t), |p| json!(p))
}

fn some(pairs: &[(&str, Option<Value>)]) -> Option<Value> {
    Some(Value::Object(pairs.iter().filter_map(|(k, v)| Some((k.to_string(), v.clone()?))).collect()))
}

fn num<T: serde::Serialize>(v: Option<T>) -> Option<Value> {
    v.map(|v| json!(v))
}

pub(crate) fn collider(system: String, c: Collider) -> P {
    P::Tend(match c {
        Collider::Add { collider } => {
            T::Collider { system, index: None, collider: Some(value(&collider)), remove: false }
        }
        Collider::Set { index, collider } => {
            T::Collider { system, index: Some(index), collider: Some(value(&collider)), remove: false }
        }
        Collider::Remove { index } => {
            T::Collider { system, index: Some(index), collider: None, remove: true }
        }
        Collider::Response { index, response, bounce, friction } => {
            let response = serde_json::from_value(json!(response)).unwrap_or(Response::Bounce);
            T::Response { system, index, response, bounce, friction }
        }
    })
}

pub(crate) fn keep(system: String, k: Keep) -> P {
    let (index, force) = match k {
        Keep::Distance { target: t, distance, strength, index } => (
            index,
            json!({ "keep_distance": { "target": target(&t), "distance": distance, "strength": strength } }),
        ),
        Keep::Between { from, to, strength, index } => (
            index,
            json!({ "keep_between": { "from": target(&from), "to": target(&to), "strength": strength } }),
        ),
        Keep::Remove { index } => {
            return P::Tend(T::Force { system, index: Some(index), force: None, remove: true });
        }
    };
    P::Tend(T::Force { system, index, force: Some(force), remove: false })
}

pub(crate) fn operator(system: String, o: Operator) -> P {
    let Operator::Remap(r) = o;
    let (index, force, remove) = match r {
        Remap::Add { remap } => (None, Some(json!({ "remap": value(&remap) })), false),
        Remap::Set { index, remap } => (Some(index), Some(json!({ "remap": value(&remap) })), false),
        Remap::Remove { index } => (Some(index), None, true),
    };
    P::Tend(T::Force { system, index, force, remove })
}

pub(crate) fn boids(system: String, b: Boids) -> P {
    let boids = some(&[
        ("radius", num(b.radius)),
        ("separation", num(b.separation)),
        ("alignment", num(b.alignment)),
        ("cohesion", num(b.cohesion)),
        ("max_speed", num(b.max_speed)),
    ]);
    P::Tend(T::Boids { system, boids, off: b.off })
}

pub(crate) fn rope(system: String, r: Rope) -> P {
    let settings = some(&[
        ("subdivide", num(r.subdivide)),
        ("width", num(r.width)),
        ("uv_scale", num(r.uv_scale)),
        ("uv_scroll", num(r.uv_scroll)),
        ("smooth", r.smooth.map(Value::Bool)),
    ]);
    P::Tend(T::Strand { system, strand: Strand::Rope, layer: r.layer, settings, off: r.off })
}

pub(crate) fn trail(system: String, t: Trail) -> P {
    let settings = some(&[
        ("length", num(t.length)),
        ("width", num(t.width)),
        ("segments", num(t.segments)),
        ("subdivide", num(t.subdivide)),
        ("uv_scale", num(t.uv_scale)),
        ("uv_scroll", num(t.uv_scroll)),
        ("textured", t.textured.map(Value::Bool)),
    ]);
    P::Tend(T::Strand { system, strand: Strand::Trail, layer: t.layer, settings, off: t.off })
}

pub(crate) fn texture(system: String, texture: String, layer: Option<String>) -> P {
    P::Tend(T::Texture { system, layer, texture: Some(texture).filter(|t| t != "none") })
}
