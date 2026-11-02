use crate::rigs::round;
use serde_json::{Value, json};
use wprs_scene::{Model, ModelFile, Tier, model_caps, model_verdict};

pub fn info(model: &Model, file: &ModelFile) -> Value {
    let tiers: Vec<Value> = [Tier::T0, Tier::T1, Tier::T2, Tier::T3]
        .into_iter()
        .map(|t| {
            let v = model_verdict(&[model], file.bones.len(), t);
            let cap = model_caps(t, false);
            json!({ "tier": t, "fits": v.fits, "reasons": v.reasons, "triangles": cap.triangles, "texture": cap.texture,
                "memory": model.texture_bytes(cap.texture) })
        })
        .collect();
    let textures: Vec<Value> =
        model.textures.iter().map(|t| json!({ "size": t.size, "bytes": t.bytes.len() })).collect();
    let materials: Vec<Value> = model
        .materials
        .iter()
        .map(|m| json!({ "name": m.name, "color": m.color.map(round), "metallic": round(m.metallic), "roughness": round(m.roughness),
            "unlit": m.unlit, "double_sided": m.double_sided, "maps": m.maps.map(|s| s.map(usize::from)) }))
        .collect();
    let clips: Vec<Value> =
        file.animations.iter().map(|(n, t)| json!({ "name": n, "length": round(*t) })).collect();
    json!({
        "triangles": model.triangles,
        "vertices": model.vertex_count(),
        "materials": materials,
        "textures": textures,
        "bones": file.bones.len(),
        "clips": clips,
        "parts": file.parts,
        "shapes": file.shapes.len(),
        "size": (0..3).map(|i| round(model.bounds[1][i] - model.bounds[0][i])).collect::<Vec<_>>(),
        "middle": model.middle().map(round),
        "tiers": tiers,
        "phone": model_caps(Tier::T1, true),
        "notes": file.notes,
    })
}

pub(super) fn finish(m: &wprs_scene::Finish) -> Value {
    let alpha = match m.alpha {
        wprs_scene::Alpha::Opaque => json!("opaque"),
        wprs_scene::Alpha::Mask(c) => json!({ "mask": round(c) }),
        wprs_scene::Alpha::Blend => json!("blend"),
    };
    json!({ "color": m.color.map(round), "metallic": round(m.metallic), "roughness": round(m.roughness),
        "emissive": m.emissive.map(round), "normal_scale": round(m.normal_scale), "occlusion": round(m.occlusion),
        "alpha": alpha, "double_sided": m.double_sided, "unlit": m.unlit })
}
