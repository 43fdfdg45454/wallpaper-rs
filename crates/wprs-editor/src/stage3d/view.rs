use serde_json::{Value, json};
use wprs_scene::{Eye, unit3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: [f32; 3],
    pub through: bool,
}

fn vec3(v: &Value, or: [f32; 3]) -> [f32; 3] {
    match v {
        Value::Array(a) if a.len() == 3 => [0, 1, 2].map(|i| a[i].as_f64().map_or(or[i], |x| x as f32)),
        Value::Number(n) => [n.as_f64().unwrap_or(0.0) as f32; 3],
        _ => or,
    }
}

pub fn camera(scene: &Value) -> Eye {
    let c = &scene["camera3d"];
    let fov = c["fov"].as_f64().unwrap_or(60.0) as f32;
    Eye {
        at: vec3(&c["eye"], [0.0, 0.0, 5.0]),
        center: vec3(&c["center"], [0.0; 3]),
        up: vec3(&c["up"], [0.0, 1.0, 0.0]),
        fov,
        near: c["near"].as_f64().unwrap_or(0.1) as f32,
        far: c["far"].as_f64().unwrap_or(1000.0) as f32,
        aspect: 1.0,
    }
}

impl View {
    pub fn read(scene: &Value) -> Option<View> {
        let v = scene.pointer("/ext/editor/view3d")?;
        let n = |k: &str, or: f32| v[k].as_f64().map_or(or, |x| x as f32);
        Some(View {
            yaw: n("yaw", 0.0),
            pitch: n("pitch", 0.0),
            distance: n("distance", 5.0).max(1e-3),
            target: vec3(&v["target"], [0.0; 3]),
            through: v["through"] == true,
        })
    }

    pub fn from(eye: &Eye) -> View {
        let d: [f32; 3] = std::array::from_fn(|i| eye.at[i] - eye.center[i]);
        let distance = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-3);
        let [x, y, z] = unit3(d);
        View {
            yaw: x.atan2(z).to_degrees(),
            pitch: y.asin().to_degrees(),
            distance,
            target: eye.center,
            through: false,
        }
    }

    pub fn eye(&self, base: &Eye) -> Eye {
        let (yaw, pitch) = (self.yaw.to_radians(), self.pitch.clamp(-89.0, 89.0).to_radians());
        let d = [pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos()];
        let at = std::array::from_fn(|i| self.target[i] + d[i] * self.distance);
        let far = base.far.max(self.distance * 4.0);
        Eye { at, center: self.target, up: [0.0, 1.0, 0.0], far, ..*base }
    }

    pub fn json(&self) -> Value {
        let r = |v: f32| (f64::from(v) * 1000.0).round() / 1000.0;
        json!({ "yaw": r(self.yaw), "pitch": r(self.pitch), "distance": r(self.distance),
            "target": self.target.map(r), "through": self.through })
    }
}

pub fn preview(scene: &mut Value) {
    let Some(view) = View::read(scene).filter(|v| !v.through) else { return };
    let eye = view.eye(&camera(scene));
    let r = |v: [f32; 3]| v.map(|x| (f64::from(x) * 1000.0).round() / 1000.0);
    let c = &mut scene["camera3d"];
    if !c.is_object() {
        *c = json!({});
    }
    (c["eye"], c["center"], c["up"], c["far"]) =
        (json!(r(eye.at)), json!(r(eye.center)), json!([0, 1, 0]), json!(eye.far));
    if let Some(m) = c.as_object_mut() {
        m.remove("paths");
    }
}
