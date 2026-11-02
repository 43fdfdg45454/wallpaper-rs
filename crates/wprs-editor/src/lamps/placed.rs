use crate::keys::value_at;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use wprs_scene::{Affine, Beam, Quality, Runtime, Scene, Values};

#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub id: String,
    pub beam: Beam,
    pub world: Affine,
    pub parent: Affine,
    pub heading: [f32; 3],
    pub color: [f32; 4],
    pub intensity: f32,
    pub radius: f32,
    pub z: f32,
    pub cone: [f32; 2],
    pub length: f32,
    pub tilt: f32,
}

fn shown(list: &mut Value) {
    for layer in list.as_array_mut().into_iter().flatten() {
        if layer["type"] == "light" {
            layer["visible"] = Value::Bool(true);
        }
        if let Some(inner) = layer.get_mut("layers") {
            shown(inner);
        }
    }
}

fn number(v: &Value, t: f64, fallback: f64) -> f32 {
    value_at(v, t).as_f64().unwrap_or(fallback) as f32
}

fn pair(v: &Value, t: f64, fallback: f32) -> [f32; 2] {
    match value_at(v, t) {
        Value::Array(l) if l.len() == 2 => [0, 1].map(|i| l[i].as_f64().map_or(fallback, |v| v as f32)),
        Value::Number(n) => [n.as_f64().unwrap_or(0.0) as f32; 2],
        _ => [fallback; 2],
    }
}

pub fn local(layer: &Value, t: f64) -> Affine {
    let tr = &layer["transform"];
    let at = pair(&tr["position"], t, 0.0);
    let (turn, scale) = (number(&tr["rotation"], t, 0.0), pair(&tr["scale"], t, 1.0));
    Affine::translate(at).then(Affine::rotate(turn)).then(Affine::scale(scale))
}

pub fn placed(dir: &Path, doc: &Value, t: f64) -> Vec<Placed> {
    let _ = dir;
    let mut scene = doc["scene"].clone();
    if let Some(list) = scene.get_mut("layers") {
        shown(list);
    }
    let Ok(scene) = Scene::from_json(&scene.to_string()) else { return Vec::new() };
    let view = [0.0, 0.0, scene.canvas.size[0] as f32, scene.canvas.size[1] as f32];
    let mut rt = Runtime::new(Arc::new(scene), Values::new());
    rt.advance(t.max(0.0));
    let frame = rt.frame(view, Quality::Q3);
    let camera = frame.camera.inverse();
    frame
        .lamps
        .iter()
        .map(|l| {
            let world = camera.then(l.world);
            let json = crate::actions::layer(doc, &l.layer.id).map(|(_, v)| v.clone()).unwrap_or_default();
            Placed {
                id: l.layer.id.clone(),
                beam: l.beam,
                world,
                parent: world.then(local(&json, t).inverse()).inverse(),
                heading: l.heading(),
                color: l.color.0,
                intensity: l.intensity,
                radius: l.radius,
                z: l.z,
                cone: l.cone,
                length: l.length,
                tilt: l.tilt,
            }
        })
        .collect()
}

impl Placed {
    pub fn at(&self) -> [f32; 2] {
        [self.world.0[4], self.world.0[5]]
    }

    pub fn power(&self, d: f32) -> f32 {
        let k = (1.0 - (d / self.radius.max(1e-3)).powi(2)).clamp(0.0, 1.0);
        self.intensity * k * k
    }

    pub fn base(&self) -> f32 {
        if self.beam == Beam::Tube { 0.0 } else { 90.0 }
    }

    pub fn angle(&self) -> f32 {
        let [a, b, c, d, ..] = self.world.0;
        let (x, y) = if self.beam == Beam::Tube { (a, b) } else { (c, d) };
        y.atan2(x).to_degrees()
    }
}
