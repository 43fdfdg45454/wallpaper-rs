use super::{Look, Shade};
use crate::eye::Eye;
use crate::resolve::Context;
use crate::{Model, ModelLayer, Param, Quality, Value};
use std::borrow::Cow;

pub const FUR_LAYERS: f32 = 12.0;
pub const FUR_LENGTH: f32 = 10.0;
pub const CHROMA_KEY: [f32; 3] = [0.0, 1.0, 0.0];

pub(crate) fn eye(cx: &Context) -> (Eye, Option<super::paths::Flight>) {
    let set = cx.script.stage.and_then(|s| s.camera3d).unwrap_or_default();
    let base = cx.scene.camera3d.clone().unwrap_or_default();
    let flight = super::paths::flight(cx, &base);
    let fly = flight.as_ref().map(|f| f.eye);
    let eye = Eye {
        at: set.eye.or(fly.map(|e| e.at)).unwrap_or_else(|| base.eye.get(cx).0),
        center: set.center.or(fly.map(|e| e.center)).unwrap_or_else(|| base.center.get(cx).0),
        up: set.up.or(fly.map(|e| e.up)).unwrap_or(base.up.0),
        fov: set.fov.or(fly.map(|e| e.fov)).unwrap_or_else(|| base.fov.get(cx)).clamp(1.0, 179.0),
        near: base.near,
        far: base.far,
        aspect: 1.0,
    };
    (eye, flight)
}

fn number(cx: &Context, v: Option<&Value<Param>>) -> Option<f32> {
    match v?.get(cx) {
        Param::Number(n) => Some(n),
        _ => None,
    }
}

fn color(cx: &Context, v: Option<&Value<Param>>) -> Option<[f32; 3]> {
    match v?.get(cx) {
        Param::Color(c) => Some([c.0[0], c.0[1], c.0[2]]),
        _ => None,
    }
}

pub(super) fn looks(cx: &Context, model: &Model, m: &ModelLayer, quality: Quality, out: &mut Vec<Look>) {
    for (name, mat) in &m.materials {
        let Some(index) = model.material(name) else { continue };
        let param = |id: &str| number(cx, mat.params.get(id));
        let [r, g, b, _] = mat.emissive.as_ref().map_or([0.0; 4], |e| e.get(cx).0);
        let shader = mat.shader.as_deref().unwrap_or("pbr");
        let (mut extra, mut slots, mut package) = ([0.0; 8], [[0.0; 4]; 8], 0);
        let shade = match shader {
            "pbr" => Shade::Pbr,
            "unlit" => Shade::Unlit,
            "fur" => {
                let layers = param("layers").unwrap_or(FUR_LAYERS).clamp(1.0, 32.0) as u32;
                let shells = crate::model::fur_shells(layers, quality) as f32;
                let comb = ["comb_x", "comb_y", "comb_z"].map(|k| param(k).unwrap_or(0.0));
                let length = param("length").unwrap_or(FUR_LENGTH) * 0.01 * model.radius();
                extra =
                    [shells, length, param("density").unwrap_or(0.5), comb[0], comb[1], comb[2], 0.0, 0.0];
                Shade::Fur
            }
            "foliage" => {
                extra[..2]
                    .copy_from_slice(&[param("strength").unwrap_or(1.0), param("speed").unwrap_or(1.0)]);
                Shade::Foliage
            }
            "chroma" => {
                let key = color(cx, mat.params.get("key")).unwrap_or(CHROMA_KEY);
                let (t, s) = (param("threshold").unwrap_or(0.4), param("softness").unwrap_or(0.1));
                extra = [key[0], key[1], key[2], t, s, 0.0, 0.0, 0.0];
                Shade::Chroma
            }
            id => match cx.scene.effect(id).filter(|d| d.stage == crate::effects::Stage::Material) {
                Some(d) => {
                    let found = |k: &str| mat.params.get(k).map(|v| Cow::Owned(v.get(cx)));
                    let all = d.pack(|k| d.setting(quality, k, found(k)));
                    slots.iter_mut().zip(all).for_each(|(s, a)| *s = a);
                    package = d.key;
                    Shade::Package
                }
                None => Shade::Pbr,
            },
        };
        out.push(Look {
            material: index as u16,
            color: mat.color.as_ref().map(|c| c.get(cx).0),
            emissive: mat.emissive.as_ref().map(|_| [r, g, b]),
            unlit: shader == "unlit",
            lit: mat.lit,
            double_sided: mat.double_sided,
            cull: mat.cull,
            depth_test: mat.depth_test,
            depth_write: mat.depth_write,
            blend: mat.blend,
            numbers: [param("metallic"), param("roughness"), param("normal_scale"), param("occlusion")],
            shade,
            extra,
            slots,
            package,
            cast: mat.cast_shadows,
            receive: mat.receive_shadows,
        });
    }
}
