use super::build::{append, bounds, part::part};
use super::{MAX_MODEL_VERTICES, MAX_POSED_MORPHS, Model};
use crate::rigged::math::{IDENTITY, Mat, apply, mul};
use crate::rigged::read::Bin;
use crate::rigged::{MAX_RIG_PALETTE, Piece, Rigged};

fn shapes(bin: &Bin, rig: &mut Rigged, m: usize, node: usize) -> Result<Vec<String>, String> {
    let mesh = &bin.doc.meshes[m];
    let count = mesh.primitives.iter().map(|p| p.targets.len()).max().unwrap_or(0);
    let names: Vec<String> = (0..count)
        .map(|t| mesh.extras.as_ref().and_then(|e| e.names.get(t)).cloned().unwrap_or(format!("shape_{t}")))
        .collect();
    if let Some(twice) = names.iter().enumerate().find(|(t, n)| names[..*t].contains(n)).map(|x| x.1) {
        return Err(format!("mesh {m} names the blend shape {twice:?} twice"));
    }
    let n = &bin.doc.nodes[node];
    for (t, name) in names.iter().enumerate() {
        if rig.shape(name).is_some() {
            continue;
        }
        rig.shapes.push(name.clone());
        let w = n.weights.get(t).or(mesh.weights.get(t)).copied().unwrap_or(0.0);
        rig.defaults.push(if w.is_finite() { w } else { 0.0 });
    }
    Ok(names)
}

fn skin(bin: &Bin, rig: &mut Rigged, s: usize) -> Result<u32, String> {
    let skin = &bin.doc.skins[s];
    let base = rig.palette.len() as u32;
    let mats = skin.inverse.map_or(Ok(Vec::new()), |a| bin.floats(a, 16))?;
    for (k, &j) in skin.joints.iter().enumerate() {
        if j >= rig.joints.len() {
            return Err(format!("skin {s} names the missing node {j}"));
        }
        let m = mats.get(k * 16..k * 16 + 16).map_or(IDENTITY, crate::rigged::build::column);
        rig.palette.push((j as u16, m));
    }
    Ok(base)
}

fn weights(
    bin: &Bin,
    p: &crate::rigged::doc::Primitive,
    n: usize,
    base: u32,
    joints: usize,
) -> Result<Vec<f32>, String> {
    let ids = bin.ints(p.attributes["JOINTS_0"], 4)?;
    let mut w = bin.floats(p.attributes["WEIGHTS_0"], 4)?;
    if ids.len() != n * 4 || w.len() != n * 4 {
        return Err("has a primitive whose attributes count different vertices".into());
    }
    let mut out = Vec::with_capacity(n * 8);
    for v in 0..n {
        let w = &mut w[v * 4..v * 4 + 4];
        let sum: f32 = w.iter().sum();
        if w.iter().any(|x| *x < 0.0) || (sum - 1.0).abs() > 0.02 {
            return Err(format!("has vertex {v} whose bone weights add up to {sum} instead of 1"));
        }
        let mut slots = [0.0; 4];
        for k in 0..4 {
            let j = ids[v * 4 + k] as usize;
            if w[k] > 0.0 && j >= joints {
                return Err(format!("has vertex {v} bound to the missing bone {j}"));
            }
            slots[k] = (base + if w[k] > 0.0 { j as u32 } else { 0 }) as f32;
        }
        out.extend_from_slice(&slots);
        out.extend(w.iter().map(|x| x / sum));
    }
    Ok(out)
}

pub(super) fn geometry(bin: &Bin, model: &mut Model, rig: &mut Rigged) -> Result<(), String> {
    let doc = bin.doc;
    rig.parts =
        doc.meshes.iter().enumerate().map(|(i, m)| m.name.clone().unwrap_or(format!("mesh_{i}"))).collect();
    let mut skins: Vec<Option<u32>> = vec![None; doc.skins.len()];
    for (n, node) in doc.nodes.iter().enumerate() {
        let Some(m) = node.mesh else { continue };
        if m >= doc.meshes.len() {
            return Err(format!("node {n} names the missing mesh {m}"));
        }
        let names = shapes(bin, rig, m, n)?;
        for p in &doc.meshes[m].primitives {
            let skinned = p.attributes.contains_key("JOINTS_0") && p.attributes.contains_key("WEIGHTS_0");
            let (base, joints) = match node.skin.filter(|_| skinned) {
                Some(s) if s < skins.len() => {
                    let b = match skins[s] {
                        Some(b) => b,
                        None => *skins[s].insert(skin(bin, rig, s)?),
                    };
                    (b, doc.skins[s].joints.len())
                }
                Some(s) => return Err(format!("node {n} names the missing skin {s}")),
                None => {
                    rig.palette.push((n as u16, IDENTITY));
                    (rig.palette.len() as u32 - 1, 0)
                }
            };
            if rig.palette.len() > MAX_RIG_PALETTE {
                return Err(format!("has more than {MAX_RIG_PALETTE} bones and parts"));
            }
            super::build::room(bin, p, model)?;
            let found = part(bin, p, MAX_MODEL_VERTICES - model.vertex_count(), &IDENTITY)?;
            let count = found.at.len();
            match joints {
                0 => (0..count)
                    .for_each(|_| model.skin.extend([base as f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0])),
                _ => model.skin.extend(weights(bin, p, count, base, joints)?),
            }
            let mut prim = append(model, &found, p.material);
            let mut targets = Vec::new();
            for (t, target) in p.targets.iter().enumerate() {
                let Some(&a) = target.get("POSITION") else { continue };
                let deltas = bin.floats(a, 3)?;
                let normals = target.get("NORMAL").map(|a| bin.floats(*a, 3)).transpose()?;
                if deltas.len() != count * 3 || normals.as_ref().is_some_and(|d| d.len() != count * 3) {
                    return Err("has a blend shape with a different number of vertices".into());
                }
                let shape = names.get(t).and_then(|s| rig.shape(s)).unwrap_or(0) as u16;
                targets.push((shape, model.morphs.len() as u32));
                for v in 0..count {
                    let d = &deltas[v * 3..v * 3 + 3];
                    let k = normals.as_ref().map_or([0.0; 3], |n| [n[v * 3], n[v * 3 + 1], n[v * 3 + 2]]);
                    model.morphs.extend([[d[0], d[1], d[2], 0.0], [k[0], k[1], k[2], 0.0]]);
                }
                if model.morphs.len() > MAX_POSED_MORPHS {
                    return Err(format!("has more than {} blend shape vertices", MAX_POSED_MORPHS / 2));
                }
            }
            prim.piece = rig.pieces.len() as u16;
            let (first, vertices) = (prim.base, prim.vertices);
            let piece = Piece {
                part: m as u16,
                first: prim.first,
                count: prim.count,
                base: first,
                vertices,
                targets,
                color: [1.0; 4],
            };
            rig.pieces.push(piece);
            model.primitives.push(prim);
        }
    }
    Ok(())
}

pub(super) fn rest(rig: &Rigged) -> Vec<Mat> {
    let mut worlds = vec![IDENTITY; rig.joints.len()];
    for &n in &rig.order {
        let (n, j) = (n as usize, &rig.joints[n as usize]);
        worlds[n] = j.parent.map_or(j.rest.matrix(), |p| mul(&worlds[p as usize], &j.rest.matrix()));
    }
    rig.palette.iter().map(|(n, inverse)| mul(&worlds[*n as usize], inverse)).collect()
}

pub(super) fn reach(model: &Model) -> [[f32; 3]; 2] {
    let at = |v: usize| {
        let p = [0, 1, 2].map(|k| model.vertices[v * super::MODEL_FLOATS + k]);
        let s = &model.skin[v * 8..v * 8 + 8];
        (0..4).filter(|k| s[4 + k] > 0.0).fold([0.0; 3], |acc, k| {
            let m = model.rest.get(s[k] as usize).unwrap_or(&IDENTITY);
            let q = apply(m, p);
            std::array::from_fn(|i| acc[i] + q[i] * s[4 + k])
        })
    };
    bounds((0..model.vertex_count()).map(at))
}

pub(super) fn skeleton(bin: &Bin, rig: &mut Rigged) -> Result<(), String> {
    let doc = bin.doc;
    let mut skins: Vec<bool> = vec![false; doc.skins.len()];
    for (n, node) in doc.nodes.iter().enumerate() {
        let Some(m) = node.mesh.filter(|m| *m < doc.meshes.len()) else { continue };
        shapes(bin, rig, m, n)?;
        match node.skin.filter(|s| *s < skins.len()) {
            Some(s) if !skins[s] => {
                skins[s] = true;
                skin(bin, rig, s)?;
            }
            Some(_) => {}
            None => rig.palette.push((n as u16, IDENTITY)),
        }
    }
    Ok(())
}
