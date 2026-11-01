use super::{Glb, Piece, Skin};
use anyhow::{Result, ensure};
use serde_json::Value;
use wprs_core::message::said;
use wprs_scene::mesh_math::{Mat, apply, mul};
use wprs_scene::{Rigged, VERTEX_FLOATS};

fn list(v: &Value) -> &[Value] {
    v.as_array().map_or(&[], Vec::as_slice)
}

fn blend(bent: &[Mat], f: &[f32]) -> Mat {
    let mut m = [0.0; 12];
    for k in 0..4 {
        let (slot, w) = (f[5 + k] as usize, f[9 + k]);
        if w > 0.0
            && let Some(b) = bent.get(slot)
        {
            m.iter_mut().zip(b).for_each(|(x, y)| *x += y * w);
        }
    }
    m
}

impl Skin {
    pub(super) fn from(glb: Glb, rig: &Rigged, size: [f32; 2]) -> Result<Skin> {
        let doc = &glb.json;
        let nodes = list(&doc["nodes"]);
        let mesh = |n: usize| nodes[n]["mesh"].as_u64().map(|m| m as usize);
        let drawn: Vec<usize> = (0..nodes.len()).filter_map(mesh).collect();
        let once = drawn.iter().enumerate().all(|(i, m)| !drawn[..i].contains(m));
        let primitives: usize = drawn.iter().map(|m| list(&doc["meshes"][*m]["primitives"]).len()).sum();
        ensure!(
            once && primitives == rig.pieces.len() && nodes.len() == rig.joints.len(),
            said(
                "editor.mesh_parts",
                "bones and weights are edited here on meshes whose parts are each drawn once, in at most 64 primitives",
                &[]
            )
        );
        let joints: Vec<u64> =
            list(&doc["skins"]).iter().flat_map(|s| list(&s["joints"])).filter_map(Value::as_u64).collect();
        let bones =
            (0..nodes.len()).filter(|n| joints.contains(&(*n as u64)) || mesh(*n).is_none()).collect();
        let auto = list(&doc["skins"]).first().is_none_or(|s| s["extras"]["weights"] == "auto");
        let mut skin = Skin {
            size,
            names: rig.joints.iter().map(|j| j.name.clone()).collect(),
            parents: rig.joints.iter().map(|j| j.parent.map(usize::from)).collect(),
            locals: rig.joints.iter().map(|j| j.rest).collect(),
            bones,
            points: Vec::with_capacity(rig.vertex_count()),
            weights: Vec::with_capacity(rig.vertex_count()),
            loose: Vec::with_capacity(rig.vertex_count()),
            triangles: rig.indices.as_chunks::<3>().0.to_vec(),
            pieces: Vec::with_capacity(rig.pieces.len()),
            auto,
            glb,
        };
        let worlds = skin.worlds();
        let bent: Vec<Mat> = rig.palette.iter().map(|(n, ibm)| mul(&worlds[*n as usize], ibm)).collect();
        let mut pieces = rig.pieces.iter();
        for n in 0..skin.names.len() {
            let Some(m) = skin.glb.json["nodes"][n]["mesh"].as_u64().map(|m| m as usize) else { continue };
            let count = list(&skin.glb.json["meshes"][m]["primitives"]).len();
            for primitive in 0..count {
                let Some(piece) = pieces.next() else { break };
                let first = skin.points.len();
                let mut linear = Vec::with_capacity(piece.vertices as usize);
                for v in piece.base..piece.base + piece.vertices {
                    let f = &rig.vertices[v as usize * VERTEX_FLOATS..][..VERTEX_FLOATS];
                    let m = blend(&bent, f);
                    skin.points.push(apply(&m, [f[0], f[1], f[2]]));
                    let w: Vec<(usize, f32)> = (0..4)
                        .filter(|k| f[9 + k] > 0.0)
                        .map(|k| (rig.palette[f[5 + k] as usize].0 as usize, f[9 + k]))
                        .collect();
                    skin.loose.push(w.iter().any(|(b, _)| !skin.bones.contains(b)));
                    skin.weights.push(w);
                    linear.push([m[0], m[1], m[2], 0.0, m[4], m[5], m[6], 0.0, m[8], m[9], m[10], 0.0]);
                }
                let json = list(&skin.glb.json["meshes"][m]["primitives"][primitive]["targets"]);
                let named = (0..json.len()).filter(|t| json[*t].get("POSITION").is_some());
                let targets = named
                    .zip(&piece.targets)
                    .map(|(t, (_, base))| {
                        let deltas = &rig.morphs[*base as usize..(*base + piece.vertices) as usize];
                        (t, deltas.iter().zip(&linear).map(|(d, l)| apply(l, [d[0], d[1], d[2]])).collect())
                    })
                    .collect();
                let count = piece.vertices as usize;
                skin.pieces.push(Piece { node: n, mesh: m, primitive, first, count, targets });
            }
        }
        Ok(skin)
    }
}
