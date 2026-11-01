use super::round;
use crate::skin::Skin;
use serde_json::{Value, json};
use std::path::Path;
use wprs_scene::mesh_math::Mat;

struct Frame<'a> {
    skin: &'a Skin,
    worlds: Vec<Mat>,
}

impl Frame<'_> {
    fn node(&self, name: &str) -> Option<usize> {
        self.skin.names.iter().position(|n| n == name)
    }

    fn head(&self, n: usize) -> [f32; 2] {
        self.skin.picture(self.skin.head(&self.worlds, n))
    }

    fn tail(&self, n: usize) -> [f32; 2] {
        let s = self.skin;
        let child = (0..s.names.len()).find(|c| s.parents[*c] == Some(n) && s.bones.contains(c));
        let [hx, hy] = self.head(n);
        match (child, s.up(n)) {
            (Some(c), _) => self.head(c),
            (None, Some(p)) => {
                let [px, py] = self.head(p);
                [2.0 * hx - px, 2.0 * hy - py]
            }
            (None, None) => [hx + 0.1, hy],
        }
    }

    fn angle(&self, n: usize) -> f64 {
        let ([hx, hy], [tx, ty]) = (self.head(n), self.tail(n));
        let [w, h] = self.skin.size;
        round(((ty - hy) * h).atan2((tx - hx) * w).to_degrees())
    }

    fn at(&self, n: usize) -> [f64; 2] {
        self.head(n).map(round)
    }

    fn target(&self, t: &Value) -> (Value, &'static str) {
        let [w, h] = self.skin.size.map(f64::from);
        match t {
            Value::Array(p) => {
                let (x, y) = (p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0));
                (json!([(x / w * 1e4).round() / 1e4, (y / h * 1e4).round() / 1e4]), "point")
            }
            Value::String(s) if s.starts_with("bone:") => {
                (json!(self.node(&s[5..]).map(|n| self.at(n))), "bone")
            }
            Value::String(s) if s.starts_with("layer:") => (Value::Null, "layer"),
            Value::String(_) => (Value::Null, "cursor"),
            _ => (Value::Null, "bound"),
        }
    }
}

pub fn handles(dir: &Path, doc: &Value, id: &str) -> Value {
    let Ok((_, layer)) = crate::actions::layer(doc, id) else { return Value::Null };
    let Ok((_, skin)) = Skin::load(dir, doc, layer) else { return Value::Null };
    let f = Frame { worlds: skin.worlds(), skin: &skin };
    let list = |key: &str| layer[key].as_array().cloned().unwrap_or_default();
    let ik = list("ik").into_iter().map(|c| {
        let nodes: Vec<usize> =
            c["chain"].as_array().into_iter().flatten().filter_map(|b| f.node(b.as_str()?)).collect();
        let (target, follows) = f.target(&c["target"]);
        let limits = c["limits"].as_object().into_iter().flatten().filter_map(|(bone, v)| {
            let n = f.node(bone)?;
            Some(json!({ "bone": bone, "at": f.at(n), "rest": f.angle(n), "min": v[0], "max": v[1] }))
        });
        json!({ "chain": c["chain"], "at": nodes.iter().map(|n| f.at(*n)).collect::<Vec<_>>(),
            "end": nodes.last().map(|n| f.tail(*n).map(round)), "target": target, "follows": follows,
            "bend": c.get("bend").unwrap_or(&json!("clockwise")), "limits": limits.collect::<Vec<_>>() })
    });
    let bodies = list("physics").into_iter().filter_map(|p| {
        let n = f.node(p["bone"].as_str()?)?;
        let gravity = p["gravity"].as_f64().unwrap_or(0.0);
        Some(json!({ "bone": p["bone"], "mode": p.get("mode").unwrap_or(&json!("spring")), "at": f.at(n),
            "tip": f.tail(n).map(round), "gravity": gravity }))
    });
    let [w, h] = skin.size;
    json!({ "size": [w, h], "ik": ik.collect::<Vec<_>>(), "bodies": bodies.collect::<Vec<_>>() })
}

pub(crate) fn rest_end(dir: &Path, doc: &Value, id: &str, bone: &str) -> Option<[f32; 2]> {
    let (_, layer) = crate::actions::layer(doc, id).ok()?;
    let (_, skin) = Skin::load(dir, doc, layer).ok()?;
    let f = Frame { worlds: skin.worlds(), skin: &skin };
    let [x, y] = f.tail(f.node(bone)?);
    Some([x * skin.size[0], y * skin.size[1]])
}

pub fn wind_arrow(doc: &Value) -> Value {
    let canvas = &doc["scene"]["canvas"];
    let gusts = canvas["gusts"].as_f64().unwrap_or(0.35);
    let Some([x, y]) = canvas["wind"]
        .as_array()
        .filter(|w| w.len() == 2)
        .map(|w| [0, 1].map(|i| w[i].as_f64().unwrap_or(0.0)))
    else {
        return json!({ "angle": 0, "strength": 0, "gusts": gusts, "fixed": canvas["wind"].is_null() });
    };
    json!({ "angle": round(y.atan2(x).to_degrees() as f32), "strength": round(x.hypot(y) as f32), "gusts": gusts, "fixed": true })
}
