use super::shapes::Frame;
use serde_json::{Value, json};
use wprs_scene::{OffsetNoise, Turbulent, fractal, swirl};

const SAMPLES: usize = 12;
const GRID: usize = 7;

fn pair(v: &Value) -> Option<[f64; 2]> {
    let l = v.as_array().filter(|l| l.len() == 2)?;
    Some([l[0].as_f64()?, l[1].as_f64()?])
}

fn mean(list: &[Value]) -> Option<[f64; 2]> {
    let all: Vec<[f64; 2]> = list.iter().filter_map(pair).collect();
    let n = all.len() as f64;
    (n > 0.0).then(|| [all.iter().map(|p| p[0]).sum::<f64>() / n, all.iter().map(|p| p[1]).sum::<f64>() / n])
}

pub(super) fn middle(e: &Value) -> Option<[f64; 2]> {
    match e["kind"].as_str()? {
        "point" => pair(&e["at"]),
        "line" => mean(&[e["from"].clone(), e["to"].clone()]),
        "box" => mean(e["corners"].as_array()?),
        "circle" | "around" => pair(&e["center"]),
        "between" => pair(&e["bulge"]),
        "layer" => mean(e["outline"].as_array()?),
        _ => None,
    }
}

fn round(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn turbulent(t: Turbulent, f: &Frame, at: [f64; 2], time: f64) -> Value {
    let turn = f.turn();
    let forward = f64::from(t.forward) + turn;
    let spread = f64::from(t.scale) * 180.0;
    let x = |k: usize| {
        (f64::from(t.noise_speed) * time + f64::from(t.phase)) as f32 + t.offset * k as f32 / SAMPLES as f32
    };
    let samples: Vec<f64> =
        (0..SAMPLES).map(|k| round(f64::from(swirl(t.forward, t.scale, x(k))) + turn)).collect();
    let reach = (f64::from(t.speed[1]) * f.scale()).clamp(40.0, 240.0);
    let (s, c) = forward.to_radians().sin_cos();
    json!({ "turbulent": t, "at": at, "forward": round(forward), "fan": [round(forward - spread), round(forward + spread)],
        "reach": reach, "knob": [round(at[0] + c * reach), round(at[1] + s * reach)], "samples": samples })
}

fn offset(o: OffsetNoise, f: &Frame, at: [f64; 2], time: f64) -> Value {
    let back = Frame::new(f.inverse());
    let local = back.apply(at);
    let side = f64::from(o.distance) * 2.0 + 40.0;
    let fields = [o.distance, o.scale, o.noise_scale, (f64::from(o.noise_speed) * time) as f32];
    let g = [f32::from(o.sign[0]), f32::from(o.sign[1]), o.octaves.clamp(1, 6) as f32];
    let arrows: Vec<Value> = (0..GRID * GRID)
        .map(|k| {
            let u = [(k % GRID) as f64, (k / GRID) as f64].map(|v| v / (GRID - 1) as f64 * 2.0 - 1.0);
            let p = [local[0] + u[0] * side, local[1] + u[1] * side];
            let r = (k as f32 * 0.618_034).fract();
            let q = fractal(p.map(|v| v as f32), fields, g, r).map(f64::from);
            json!([f.round(f.apply(p)), f.round(f.apply(q))])
        })
        .collect();
    let ring = f64::from(o.distance) * f.scale();
    json!({ "offset": o, "at": at, "radius": ring, "knob": [round(at[0] + ring), round(at[1])], "arrows": arrows })
}

pub(super) fn fields(s: &Value, f: &Frame, emitters: &[Value], time: f64) -> Value {
    let at = emitters.iter().find_map(middle).unwrap_or_else(|| f.round(f.apply([0.0; 2])));
    let init = &s["init"];
    let t = serde_json::from_value::<Turbulent>(init["turbulent"].clone()).ok();
    let o = serde_json::from_value::<OffsetNoise>(init["offset_noise"].clone()).ok();
    json!({ "turbulent": t.map(|t| turbulent(t, f, at, time)), "offset": o.map(|o| offset(o, f, at, time)) })
}
