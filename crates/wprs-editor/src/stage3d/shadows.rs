use super::gizmo::{Seen, px};
use crate::action::ShadowEdit;
use crate::ops::{Op, set};
use crate::rigs::round;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use wprs_core::message::said;
use wprs_scene::{Beam, Eye, Lamp, MAX_SHADOWS, Quality, Runtime, Stage3d, Tier, Values, shadow_side, unit3};

const TIERS: [Tier; 4] = [Tier::T0, Tier::T1, Tier::T2, Tier::T3];

pub(crate) fn edit(doc: &Value, e: &ShadowEdit) -> Result<Vec<Op>> {
    let (path, l) = crate::actions::layer(doc, &e.layer)?;
    let kind = l["light"].as_str().unwrap_or("point");
    let fits = l["type"] == "light" && ["point", "spot", "directional"].contains(&kind);
    ensure!(fits, said("editor.shadow_light", "only point, spot and directional lights cast shadows", &[]));
    let mut ops = Vec::new();
    if let Some(on) = e.on {
        let others =
            crate::layers::walk(doc).iter().filter(|(p, x)| *p != path && x["shadows"] == true).count();
        ensure!(
            !on || others < MAX_SHADOWS,
            said("editor.shadows_most", "a scene has at most %1 lights with shadows", &[&MAX_SHADOWS])
        );
        let at = format!("{path}/shadows");
        match on {
            true => ops.extend(set(doc, &at, json!(true))),
            false => ops.extend(["shadows", "shadow_resolution", "shadow_bias"].iter().filter_map(|k| {
                let at = format!("{path}/{k}");
                doc.pointer(&at).map(|_| Op::Remove { path: at })
            })),
        }
    }
    if let Some(r) = e.resolution {
        ensure!(
            (256..=4096).contains(&r),
            said("editor.shadow_size", "a shadow map has 256 to 4096 pixels a side", &[])
        );
        ops.extend(set(doc, &format!("{path}/shadow_resolution"), json!(r)));
    }
    if e.automatic && doc.pointer(&format!("{path}/shadow_resolution")).is_some() {
        ops.push(Op::Remove { path: format!("{path}/shadow_resolution") });
    }
    if let Some(b) = e.bias {
        ensure!(
            b.is_finite() && (0.0..=1.0).contains(&b),
            said("editor.shadow_bias", "the bias goes from 0 to 1", &[])
        );
        ops.extend(set(doc, &format!("{path}/shadow_bias"), crate::clips::num(b as f32)));
    }
    Ok(ops)
}

fn turn(eye: &Eye, [x, y, z]: [f32; 3]) -> [f32; 3] {
    let (r, u, f) = eye.basis();
    unit3(std::array::from_fn(|i| r[i] * x - u[i] * y - f[i] * z))
}

fn lines(seen: &Seen, stage: &Stage3d, lamp: &Lamp, (mid, r): ([f32; 3], f32)) -> Vec<Value> {
    let eye = &stage.eye;
    let local = stage.world.inverse().apply(lamp.at());
    let p = [local[0] + stage.origin[0] * stage.size[0], local[1] + stage.origin[1] * stage.size[1]];
    let at = eye.place(p, lamp.z, stage.size);
    let dir = turn(eye, lamp.heading());
    let side = unit3(if dir[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [-dir[2], 0.0, dir[0]] });
    let up = unit3([
        dir[1] * side[2] - dir[2] * side[1],
        dir[2] * side[0] - dir[0] * side[2],
        dir[0] * side[1] - dir[1] * side[0],
    ]);
    let add = |o: [f32; 3], v: [[f32; 3]; 3], k: [f32; 3]| -> [f32; 3] {
        std::array::from_fn(|i| o[i] + v[0][i] * k[0] + v[1][i] * k[1] + v[2][i] * k[2])
    };
    let gap = (0..3).map(|i| (mid[i] - at[i]).powi(2)).sum::<f32>().sqrt();
    let far = (lamp.radius * eye.unit(stage.size[1])).min(gap + r);
    let (apex, ring): ([f32; 3], Vec<[f32; 3]>) = match lamp.beam {
        Beam::Directional => {
            let back = add(mid, [dir, side, up], [-2.0 * r, 0.0, 0.0]);
            let quad = |d: f32| {
                [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                    .map(|(a, b)| add(back, [dir, side, up], [d, a * r, b * r]))
            };
            let (near, back4) = (quad(0.0), quad(4.0 * r));
            let mut out: Vec<([f32; 3], [f32; 3])> = (0..4).map(|k| (near[k], back4[k])).collect();
            out.extend((0..4).flat_map(|k| [(near[k], near[(k + 1) % 4]), (back4[k], back4[(k + 1) % 4])]));
            return out
                .into_iter()
                .filter_map(|(a, b)| Some(json!([px(seen.at(a)?), px(seen.at(b)?)])))
                .collect();
        }
        Beam::Spot => {
            let half = (lamp.cone[1] * 0.5).to_radians().tan() * far;
            (
                at,
                (0..16)
                    .map(|k| {
                        let a = k as f32 / 16.0 * std::f32::consts::TAU;
                        add(at, [dir, side, up], [far, a.cos() * half, a.sin() * half])
                    })
                    .collect(),
            )
        }
        _ => (
            at,
            (0..24)
                .map(|k| {
                    let a = k as f32 / 24.0 * std::f32::consts::TAU;
                    add(at, [dir, side, up], [0.0, a.cos() * far, a.sin() * far])
                })
                .collect(),
        ),
    };
    let mut out: Vec<([f32; 3], [f32; 3])> =
        (0..ring.len()).map(|k| (ring[k], ring[(k + 1) % ring.len()])).collect();
    if lamp.beam == Beam::Spot {
        out.extend([0, 4, 8, 12].map(|k| (apex, ring[k])));
    }
    out.into_iter().filter_map(|(a, b)| Some(json!([px(seen.at(a)?), px(seen.at(b)?)]))).collect()
}

pub fn shadows(dir: &Path, doc: &Value, t: f64) -> Value {
    let Some(scene) = super::store::scene(dir, doc) else { return Value::Null };
    let size = scene.canvas.size.map(|v| v as f32);
    let mut rt = Runtime::new(Arc::new(scene), Values::new());
    rt.advance(t.max(0.0));
    let frame = rt.frame([0.0, 0.0, size[0], size[1]], Quality::Q3);
    let seen = super::gizmo::seen_for(doc, &frame);
    let reach = super::gizmo::reach(&frame);
    let mut count = 0;
    let lights: Vec<Value> = frame
        .lamps
        .iter()
        .filter(|l| l.shadow.is_some())
        .map(|l| {
            count += 1;
            let set = l.shadow.and_then(|s| s.0);
            let sides: Vec<Value> = TIERS.iter().map(|t| {
                let cap = wprs_scene::model_caps(*t, false).shadows;
                json!({ "tier": t, "side": shadow_side(l.beam, set, (*t, false), Quality::Q3), "phone": shadow_side(l.beam, set, (*t, true), Quality::Q3),
                    "low": shadow_side(l.beam, set, (*t, false), Quality::Q0), "shown": count <= cap })
            }).collect();
            let frusta = match (&seen, frame.stages.first()) {
                (Some(s), Some(stage)) => lines(s, stage, l, reach),
                _ => Vec::new(),
            };
            json!({ "id": l.layer.id, "light": l.beam, "resolution": set, "bias": round(l.shadow.map_or(0.0, |s| s.1)),
                "tiers": sides, "frustum": frusta })
        })
        .collect();
    let caps: Vec<Value> = TIERS
        .iter()
        .map(|t| json!({ "tier": t, "lights": wprs_scene::model_caps(*t, false).shadows }))
        .collect();
    json!({ "lights": lights, "caps": caps, "most": MAX_SHADOWS, "debug": doc.pointer("/scene/ext/editor/debug3d") })
}
