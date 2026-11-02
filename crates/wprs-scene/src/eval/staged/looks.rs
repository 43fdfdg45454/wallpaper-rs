use super::Look;
use crate::eye::Eye;
use crate::resolve::Context;
use crate::{Model, ModelLayer, Param, Value};

pub(crate) fn eye(cx: &Context) -> Eye {
    let set = cx.script.stage.and_then(|s| s.camera3d).unwrap_or_default();
    let base = cx.scene.camera3d.clone().unwrap_or_default();
    Eye {
        at: set.eye.unwrap_or_else(|| base.eye.get(cx).0),
        center: set.center.unwrap_or_else(|| base.center.get(cx).0),
        up: set.up.unwrap_or(base.up.0),
        fov: set.fov.unwrap_or_else(|| base.fov.get(cx)).clamp(1.0, 179.0),
        near: base.near,
        far: base.far,
        aspect: 1.0,
    }
}

fn number(cx: &Context, v: Option<&Value<Param>>) -> Option<f32> {
    match v?.get(cx) {
        Param::Number(n) => Some(n),
        _ => None,
    }
}

pub(super) fn looks(cx: &Context, model: &Model, m: &ModelLayer, out: &mut Vec<Look>) {
    for (name, mat) in &m.materials {
        let Some(index) = model.material(name) else { continue };
        let param = |id: &str| number(cx, mat.params.get(id));
        let [r, g, b, _] = mat.emissive.as_ref().map_or([0.0; 4], |e| e.get(cx).0);
        out.push(Look {
            material: index as u16,
            color: mat.color.as_ref().map(|c| c.get(cx).0),
            emissive: mat.emissive.as_ref().map(|_| [r, g, b]),
            unlit: mat.shader.as_deref() == Some("unlit"),
            lit: mat.lit,
            double_sided: mat.double_sided,
            cull: mat.cull,
            depth_test: mat.depth_test,
            depth_write: mat.depth_write,
            blend: mat.blend,
            numbers: [param("metallic"), param("roughness"), param("normal_scale"), param("occlusion")],
        });
    }
}
