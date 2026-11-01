use crate::action::{Action, WeightStroke};
use crate::ops::Op;
use crate::rig::{Boning, Cutting};
use crate::skin::{Brush, Skin};
use anyhow::{Result, ensure};
use serde_json::Value;
use std::path::Path;
use wprs_core::message::said;

fn weights(dir: &Path, doc: &Value, w: &WeightStroke) -> Result<Vec<Op>> {
    let (_, found) = crate::actions::layer(doc, &w.layer)?;
    let (id, mut skin) = Skin::load(dir, doc, found)?;
    let bone = skin.bone(&w.bone)?;
    let locked = w.lock.iter().map(|l| skin.bone(l)).collect::<Result<Vec<usize>>>()?;
    ensure!(!locked.contains(&bone), said("editor.bone_locked", "the bone being painted is locked", &[]));
    let stroke = crate::sculpt::stroke(&skin, &w.points, w.radius)?;
    let fits = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
    ensure!(
        fits(w.by) && fits(w.falloff),
        said("editor.weight_by", "a weight stroke has a strength and a falloff from 0 to 1", &[])
    );
    let [x, y] = skin.size;
    let brush = Brush { bone, mode: w.mode, by: w.by, radius: w.radius * x.max(y), falloff: w.falloff };
    skin.paint(&brush, &stroke, &locked)?;
    skin.save(dir, doc, &id)
}

pub(crate) fn act(dir: &Path, doc: &Value, action: &Action) -> Option<Result<Vec<Op>>> {
    Some(match action {
        Action::MeshFromPicture(c) => {
            let (image, outline, cells, id) = (&c.image, c.outline, c.cells, c.id.as_deref());
            crate::rig::mesh_from_picture(dir, doc, &Cutting { image, outline, cells, id })
        }
        Action::Bone(b) => {
            let (parent, to) = (b.parent.as_deref(), b.to.as_deref());
            let (layer, name, at, remove) = (&b.layer, &b.name, b.at, b.remove);
            crate::rig::bone(dir, doc, &Boning { layer, name, parent, at, remove, to })
        }
        Action::MeshDepth(d) => crate::sculpt::depth(dir, doc, &d.layer, &d.points, (d.radius, d.by)),
        Action::Weights(w) => weights(dir, doc, w),
        Action::AutoWeights(a) => (|| {
            let (_, found) = crate::actions::layer(doc, &a.layer)?;
            let (id, mut skin) = Skin::load(dir, doc, found)?;
            ensure!(!skin.bones.is_empty(), said("editor.no_bones", "the mesh has no bones yet", &[]));
            skin.auto = true;
            skin.save(dir, doc, &id)
        })(),
        Action::Clip(c) => crate::clips::clip(dir, doc, c),
        Action::Key(k) => crate::clips::key(dir, doc, k),
        Action::Expression(e) => crate::shapes::expression(dir, doc, e),
        _ => return None,
    })
}
