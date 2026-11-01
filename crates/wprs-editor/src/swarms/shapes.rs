use crate::keys::value_at;
use serde_json::{Value, json};
use std::path::Path;
use wprs_scene::{taper, turn as sweep};

pub(super) struct Frame {
    pub m: [f64; 6],
}

impl Frame {
    pub fn new(m: [f64; 6]) -> Frame {
        Frame { m }
    }

    pub fn apply(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        let m = self.m;
        [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
    }

    pub fn scale(&self) -> f64 {
        (self.m[0] * self.m[3] - self.m[1] * self.m[2]).abs().sqrt()
    }

    pub fn turn(&self) -> f64 {
        self.m[1].atan2(self.m[0]).to_degrees()
    }

    pub fn inverse(&self) -> [f64; 6] {
        let [a, b, c, d, x, y] = self.m;
        let det = (a * d - b * c).abs().max(1e-12).copysign(a * d - b * c);
        let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
        [ia, ib, ic, id, -(ia * x + ic * y), -(ib * x + id * y)]
    }

    pub fn round(&self, p: [f64; 2]) -> [f64; 2] {
        p.map(|v| (v * 100.0).round() / 100.0)
    }
}

fn pair(v: &Value) -> [f64; 2] {
    let list = v.as_array();
    [0, 1].map(|i| list.and_then(|l| l.get(i)?.as_f64()).unwrap_or(0.0))
}

fn num(v: &Value, t: f64, or: f64) -> f64 {
    value_at(v, t).as_f64().unwrap_or(or)
}

fn rot([x, y]: [f64; 2], deg: f64) -> [f64; 2] {
    let (s, c) = deg.to_radians().sin_cos();
    [x * c - y * s, x * s + y * c]
}

fn target(v: &Value, f: &Frame, spots: &[Value]) -> (Option<[f64; 2]>, f64) {
    match v {
        Value::Array(_) => (Some(f.apply(pair(v))), f.turn()),
        Value::String(p) if p.starts_with('p') => {
            let k: usize = p[1..].parse().unwrap_or(0);
            let s = spots.get(k).or(spots.first());
            (
                s.and_then(|s| s["at"].as_array().map(|_| pair(&s["at"]))),
                s.map_or(0.0, |s| s["angle"].as_f64().unwrap_or(0.0)),
            )
        }
        _ => (None, 0.0),
    }
}

fn anchored(v: &Value, f: &Frame, spots: &[Value]) -> impl Fn([f64; 2]) -> [f64; 2] {
    let k = v.as_str().and_then(|p| p[1..].parse::<usize>().ok());
    let spot = k.and_then(|k| spots.get(k));
    let origin = spot.map_or([0.0; 2], |s| pair(&s["local"]));
    let angle = spot.map_or(0.0, |s| s["angle"].as_f64().unwrap_or(0.0) - f.turn());
    let m = f.m;
    move |p: [f64; 2]| {
        let [x, y] = rot(p, angle);
        let q = [origin[0] + x, origin[1] + y];
        [m[0] * q[0] + m[2] * q[1] + m[4], m[1] * q[0] + m[3] * q[1] + m[5]]
    }
}

fn around(a: &Value, f: &Frame, spots: &[Value], t: f64) -> Value {
    let (center, angle) = target(a.get("at").unwrap_or(&json!("p0")), f, spots);
    let (count, arc) = (a["count"].as_u64().unwrap_or(1).max(1), num(&a["arc"], t, 360.0));
    let radius = num(&a["radius"], t, 0.0) * f.scale();
    let steps = if arc >= 360.0 { count } else { count.saturating_sub(1) }.max(1) as f64;
    let at = |i: f64| {
        center.map(|c| {
            let (s, k) = (angle + i / steps * arc).to_radians().sin_cos();
            f.round([c[0] + radius * k, c[1] + radius * s])
        })
    };
    let places: Vec<_> = (0..count.min(64)).map(|i| at(i as f64)).collect();
    json!({ "kind": "around", "at": a.get("at").unwrap_or(&json!("p0")), "center": center.map(|c| f.round(c)), "radius": radius,
        "arc": arc, "start": angle, "count": count, "mode": a.get("mode").unwrap_or(&json!("repeat")), "places": places,
        "ends": [at(0.0), at(count.saturating_sub(1) as f64)] })
}

fn between(b: &Value, f: &Frame, spots: &[Value], t: f64) -> Value {
    let ((from, _), (to, _)) = (target(&b["from"], f, spots), target(&b["to"], f, spots));
    let count = b["count"].as_u64().unwrap_or(1).max(1);
    let (span, amount) = (num(&b["span"], t, 1.0), b["arc"]["amount"].as_f64().unwrap_or(0.0));
    let side = b["arc"]["side"].as_f64().filter(|s| *s != 0.0).unwrap_or(1.0);
    let taped = b.get("taper").is_some();
    let (pos, size) =
        (b["taper"]["position"].as_f64().unwrap_or(0.5), b["taper"]["size"].as_f64().unwrap_or(0.5));
    let point = |u: f64| {
        from.zip(to).map(|(p, q)| {
            let (d, bow) = ([q[0] - p[0], q[1] - p[1]], amount * (std::f64::consts::PI * u).sin() * side);
            f.round([p[0] + d[0] * u - d[1] * bow, p[1] + d[1] * u + d[0] * bow])
        })
    };
    let places: Vec<_> = (0..count.min(64))
        .map(|i| {
            let (k, _) = sweep(count as f32, false, i as u32);
            let u = if count > 1 { f64::from(k) / (count - 1) as f64 } else { 0.0 } * span;
            let grow = if taped { f64::from(taper(u as f32, pos as f32, size as f32)) } else { 1.0 };
            let grow = (grow * 1e4).round() / 1e4;
            json!({ "at": point(u), "size": grow })
        })
        .collect();
    json!({ "kind": "between", "from": from.map(|p| f.round(p)), "to": to.map(|p| f.round(p)), "from_ref": b["from"], "to_ref": b["to"],
        "count": count, "span": span, "bulge": point(0.5), "amount": amount, "side": b["arc"]["side"].as_f64().unwrap_or(0.0),
        "taper": taped.then(|| json!({ "at": point(pos * span), "position": pos, "size": size })), "places": places })
}

pub(super) fn shapes(
    dir: &Path,
    doc: &Value,
    s: &Value,
    (f, spots): (&Frame, &[Value]),
    t: f64,
) -> Vec<Value> {
    let list = s["emitters"].as_array().into_iter().flatten().enumerate();
    list.map(|(e, em)| {
        let shape = &em["shape"];
        let (kind, v) = shape.as_object().and_then(|o| o.iter().next()).map_or((shape.as_str().unwrap_or("edge"), &Value::Null), |(k, v)| (k.as_str(), v));
        let mut out = match kind {
            "point" => json!({ "kind": kind, "at": f.round(f.apply(pair(v))) }),
            "line" => json!({ "kind": kind, "from": f.round(f.apply(pair(&v[0]))), "to": f.round(f.apply(pair(&v[1]))) }),
            "box" => {
                let (from, to) = if v.is_array() { (pair(&v[0]), pair(&v[1])) } else { (pair(&v["from"]), pair(&v["to"])) };
                let place = anchored(&v["at"], f, spots);
                let corners = [from, [to[0], from[1]], to, [from[0], to[1]]].map(|p| f.round(place(p)));
                json!({ "kind": kind, "corners": corners, "from": from, "to": to, "inner": v["inner"], "anchor": v["at"] })
            }
            "circle" => {
                let place = anchored(&v["at"], f, spots);
                let center = pair(&v["center"]);
                let radius = num(&v["radius"], t, 0.0);
                let edge = f.round(place([center[0] + radius, center[1]]));
                json!({ "kind": kind, "center": f.round(place(center)), "local": center, "radius": radius * f.scale(), "edge": edge,
                    "inner": num(&v["inner"], t, 0.0) * f.scale(), "anchor": v["at"] })
            }
            "around" => around(v, f, spots, t),
            "between" => between(v, f, spots, t),
            "layer" => super::samples::samples(dir, doc, v, t),
            _ => json!({ "kind": kind }),
        };
        out["index"] = json!(e);
        out
    }).collect()
}
