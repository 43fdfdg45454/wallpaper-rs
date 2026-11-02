use serde_json::Value;
use wprs_scene::{Color, Finish};

fn linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn encode(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round() as u8
}

fn picked(v: &Value) -> Option<[f32; 4]> {
    serde_json::from_value::<Color>(v.clone()).ok().map(|c| c.0)
}

pub(super) fn sphere(doc: &Value, m: &Finish, set: &Value) -> Value {
    let lit = |name| crate::lamps::color(doc, name).map(|c| c.map(linear));
    let ambient = lit("ambient").unwrap_or([0.12, 0.12, 0.12, 1.0]);
    let sky = lit("sky").unwrap_or([0.18, 0.18, 0.2, 1.0]);
    let tint = picked(&set["color"]).unwrap_or([1.0; 4]);
    let base: [f32; 3] = std::array::from_fn(|i| linear(m.color[i]) * linear(tint[i]));
    let glow = picked(&set["emissive"]).map_or(m.emissive, |e| [linear(e[0]), linear(e[1]), linear(e[2])]);
    let number = |k: &str, or: f32| set["params"][k].as_f64().map_or(or, |v| v as f32);
    let (metal, rough) =
        (number("metallic", m.metallic).clamp(0.0, 1.0), number("roughness", m.roughness).clamp(0.04, 1.0));
    let flat = set["shader"] == "unlit" || set["lit"] == false || m.unlit;
    let side = 96u32;
    let r = side as f32 / 2.0;
    let key = {
        let k = [-0.45f32, 0.6, 0.65];
        let l = (k[0] * k[0] + k[1] * k[1] + k[2] * k[2]).sqrt();
        k.map(|v| v / l)
    };
    let ball = image::RgbaImage::from_fn(side, side, |x, y| {
        let (u, v) = ((x as f32 + 0.5 - r) / r, (r - y as f32 - 0.5) / r);
        let d = u * u + v * v;
        if d > 1.0 {
            return image::Rgba([0, 0, 0, 0]);
        }
        let n = [u, v, (1.0 - d).sqrt()];
        let shade = |i: usize| {
            if flat {
                return base[i] + glow[i];
            }
            let up = 0.5 + 0.5 * n[1];
            let around = (ambient[i] + sky[i] * up) * base[i] * (1.0 - metal);
            let nl = (n[0] * key[0] + n[1] * key[1] + n[2] * key[2]).max(0.0);
            let h = [key[0], key[1], key[2] + 1.0];
            let hl = (h[0] * h[0] + h[1] * h[1] + h[2] * h[2]).sqrt();
            let nh = ((n[0] * h[0] + n[1] * h[1] + n[2] * h[2]) / hl).max(0.0);
            let shine =
                nh.powf(2.0 / (rough * rough * rough * rough).max(1e-4)) * (0.04 + 0.96 * metal * base[i]);
            around
                + 2.2 * nl * (base[i] * (1.0 - metal) / std::f32::consts::PI * std::f32::consts::PI + shine)
                + glow[i]
        };
        image::Rgba([encode(shade(0)), encode(shade(1)), encode(shade(2)), 255])
    });
    Value::String(crate::rigs::url(&ball))
}
