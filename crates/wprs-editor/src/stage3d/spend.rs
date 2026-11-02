use wprs_scene::{Kind, Light, Model, Quality, Scene, Tier, fur_shells, shadow_side};

pub(super) fn shadow_texels(scene: &Scene, q: Quality) -> u64 {
    let lights = scene.layers().filter_map(|l| match &l.kind {
        Kind::Light(x) if x.shadows => Some(x.as_ref()),
        _ => None,
    });
    let texels = |l: &Light| {
        let side = u64::from(shadow_side(l.light, l.shadow_resolution, (Tier::T3, false), q));
        side * side * if l.light == wprs_scene::Beam::Point { 6 } else { 1 }
    };
    lights.take(2).map(texels).sum()
}

fn triangles(model: &Model, material: usize) -> u64 {
    let mine = model.primitives.iter().filter(|p| p.material.map(usize::from) == Some(material));
    mine.map(|p| u64::from(p.count / 3)).sum()
}

pub(super) fn fur(scene: &Scene, q: Quality) -> (u64, u64) {
    let (mut shells, mut drawn) = (0, 0);
    for l in scene.layers() {
        let Kind::Model(m) = &l.kind else { continue };
        let Some(model) = scene.solids.get(&m.model) else { continue };
        for (name, mat) in m.materials.iter().filter(|(_, x)| x.shader.as_deref() == Some("fur")) {
            let layers = match mat.params.get("layers") {
                Some(wprs_scene::Value::Fixed(wprs_scene::Param::Number(n))) => *n as u32,
                _ => wprs_scene::FUR_LAYERS as u32,
            };
            let n = u64::from(fur_shells(layers, q));
            shells += n;
            drawn += n * model.material(name).map_or(0, |i| triangles(model, i));
        }
    }
    (shells, drawn)
}

pub(super) fn skinned(scene: &Scene) -> usize {
    let posed =
        |l: &wprs_scene::Layer| matches!(&l.kind, Kind::Model(m) if !m.rig.is_empty() || m.root_motion);
    let models = scene.layers().filter(|l| posed(l)).filter_map(|l| scene.solids.get(l.figure()?));
    models.map(|m| m.vertex_count()).sum()
}
