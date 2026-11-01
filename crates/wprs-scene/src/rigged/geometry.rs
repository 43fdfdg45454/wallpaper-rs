use super::build::column;
use super::doc::Primitive;
use super::math::IDENTITY;
use super::read::Bin;
use super::{
    MAX_MORPH_TEXELS, MAX_RIG_INDICES, MAX_RIG_PALETTE, MAX_RIG_PIECES, MAX_RIG_VERTICES, Piece, Rigged,
};

fn named(name: &Option<String>, kind: &str, i: usize) -> String {
    name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("{kind}_{i}"))
}

fn skin(bin: &Bin, rig: &mut Rigged, s: usize) -> Result<(u32, usize), String> {
    let skin = &bin.doc.skins[s];
    let base = rig.palette.len() as u32;
    let mats = match skin.inverse {
        Some(a) => bin.floats(a, 16)?,
        None => Vec::new(),
    };
    for (k, &j) in skin.joints.iter().enumerate() {
        if j >= rig.joints.len() {
            return Err(format!("skin {s} names the missing node {j}"));
        }
        let m = mats.get(k * 16..k * 16 + 16).map_or(IDENTITY, column);
        rig.palette.push((j as u16, m));
    }
    Ok((base, skin.joints.len()))
}

pub(super) fn pieces(bin: &Bin, rig: &mut Rigged) -> Result<(), String> {
    let doc = bin.doc;
    rig.parts = doc.meshes.iter().enumerate().map(|(i, m)| named(&m.name, "mesh", i)).collect();
    let (mut skins, mut skipped) = (vec![None; doc.skins.len()], 0);
    for (n, node) in doc.nodes.iter().enumerate() {
        let Some(m) = node.mesh else { continue };
        let mesh = doc.meshes.get(m).ok_or_else(|| format!("node {n} names the missing mesh {m}"))?;
        let names: Vec<String> = (0..mesh.primitives.iter().map(|p| p.targets.len()).max().unwrap_or(0))
            .map(|t| {
                mesh.extras.as_ref().and_then(|e| e.names.get(t)).cloned().unwrap_or(format!("shape_{t}"))
            })
            .collect();
        if let Some(twice) = names.iter().enumerate().find(|(t, n)| names[..*t].contains(n)).map(|x| x.1) {
            return Err(format!("mesh {m} names the blend shape {twice:?} twice"));
        }
        for (t, name) in names.iter().enumerate() {
            if rig.shape(name).is_none() {
                rig.shapes.push(name.clone());
                let weight = node.weights.get(t).or(mesh.weights.get(t)).copied().unwrap_or(0.0);
                rig.defaults.push(if weight.is_finite() { weight } else { 0.0 });
            }
        }
        for p in &mesh.primitives {
            let jointed = p.attributes.contains_key("JOINTS_0") && p.attributes.contains_key("WEIGHTS_0");
            let (base, joints) = match node.skin.filter(|_| jointed) {
                Some(s) if s < skins.len() => match skins[s] {
                    Some(found) => found,
                    None => *skins[s].insert(skin(bin, rig, s)?),
                },
                Some(s) => return Err(format!("node {n} names the missing skin {s}")),
                None => {
                    rig.palette.push((n as u16, IDENTITY));
                    (rig.palette.len() as u32 - 1, 1)
                }
            };
            if rig.palette.len() > MAX_RIG_PALETTE {
                return Err(format!("has more than {MAX_RIG_PALETTE} bones and parts"));
            }
            match rig.pieces.len() < MAX_RIG_PIECES {
                true => primitive(bin, rig, p, (m, base, joints), &names)?,
                false => skipped += 1,
            }
        }
    }
    let (total, pieces) = (rig.morphs.len(), rig.pieces.len() + skipped);
    let checks = [
        (rig.vertex_count() <= MAX_RIG_VERTICES, format!("more than {MAX_RIG_VERTICES} vertices")),
        (rig.palette.len() <= MAX_RIG_PALETTE, format!("more than {MAX_RIG_PALETTE} bones and parts")),
        (pieces <= MAX_RIG_PIECES, format!("more than {MAX_RIG_PIECES} drawn primitives")),
        (total <= MAX_MORPH_TEXELS, format!("more than {MAX_MORPH_TEXELS} blend shape vertices")),
    ];
    match checks.into_iter().find(|(ok, _)| !ok) {
        Some((_, what)) => Err(format!("has {what}")),
        None => Ok(()),
    }
}

fn primitive(
    bin: &Bin,
    rig: &mut Rigged,
    p: &Primitive,
    (mesh, base, joints): (usize, u32, usize),
    names: &[String],
) -> Result<(), String> {
    let at = |key: &str| p.attributes.get(key).copied();
    let position = at("POSITION").ok_or("has a primitive without positions")?;
    if rig.vertex_count() + bin.count(position)? > MAX_RIG_VERTICES {
        return Err(format!("has more than {MAX_RIG_VERTICES} vertices"));
    }
    let positions = bin.floats(position, 3)?;
    let n = positions.len() / 3;
    let listed = p.indices.map_or(Ok(n), |a| bin.count(a))?;
    let corners = if p.mode.unwrap_or(4) == 4 { listed / 3 * 3 } else { listed.saturating_sub(2) * 3 };
    if rig.indices.len() + corners > MAX_RIG_INDICES {
        return Err(format!("has more than {MAX_RIG_INDICES} triangle corners"));
    }
    let uvs = at("TEXCOORD_0").map_or(Ok(vec![0.0; n * 2]), |a| bin.floats(a, 2))?;
    let skinned = joints > 0 && at("JOINTS_0").is_some();
    let ids = at("JOINTS_0").filter(|_| skinned).map_or(Ok(vec![0; n * 4]), |a| bin.ints(a, 4))?;
    let weights = at("WEIGHTS_0").filter(|_| skinned);
    let mut weights =
        weights.map_or(Ok((0..n * 4).map(|k| f32::from(k % 4 == 0)).collect()), |a| bin.floats(a, 4))?;
    if uvs.len() != n * 2 || ids.len() != n * 4 || weights.len() != n * 4 {
        return Err("has a primitive whose attributes count different vertices".into());
    }
    let piece = rig.pieces.len() as f32;
    let first = rig.vertex_count() as u32;
    for v in 0..n {
        let w = &mut weights[v * 4..v * 4 + 4];
        let sum: f32 = w.iter().sum();
        if w.iter().any(|x| *x < 0.0) || (sum - 1.0).abs() > 0.02 {
            return Err(format!("has vertex {v} whose bone weights add up to {sum} instead of 1"));
        }
        w.iter_mut().for_each(|x| *x /= sum);
        let mut slots = [0.0; 4];
        for k in 0..4 {
            let j = ids[v * 4 + k] as usize;
            if w[k] > 0.0 && j >= joints {
                return Err(format!("has vertex {v} bound to the missing bone {j}"));
            }
            slots[k] = (base + if w[k] > 0.0 { j as u32 } else { 0 }) as f32;
        }
        rig.vertices.extend_from_slice(&positions[v * 3..v * 3 + 3]);
        rig.vertices.extend_from_slice(&uvs[v * 2..v * 2 + 2]);
        rig.vertices.extend_from_slice(&slots);
        rig.vertices.extend_from_slice(w);
        rig.vertices.push(piece);
    }
    let list = p.indices.map_or(Ok((0..n as u32).collect()), |a| bin.ints(a, 1))?;
    if list.iter().any(|i| *i as usize >= n) {
        return Err("has an index out of the range of its vertices".into());
    }
    let start = rig.indices.len() as u32;
    triangles(&list, p.mode.unwrap_or(4), first, &mut rig.indices)?;
    let mut targets = Vec::new();
    for (t, target) in p.targets.iter().enumerate() {
        let Some(&a) = target.get("POSITION") else { continue };
        let deltas = bin.floats(a, 3)?;
        if deltas.len() != n * 3 {
            return Err("has a blend shape with a different number of vertices".into());
        }
        let shape = names.get(t).and_then(|s| rig.shape(s)).unwrap_or(0) as u16;
        targets.push((shape, rig.morphs.len() as u32));
        rig.morphs.extend(deltas.chunks(3).map(|d| [d[0], d[1], d[2], 0.0]));
        if rig.morphs.len() > MAX_MORPH_TEXELS {
            return Err(format!("has more than {MAX_MORPH_TEXELS} blend shape vertices"));
        }
    }
    let color = p.material.and_then(|m| bin.doc.materials.get(m)?.pbr.as_ref()?.color).unwrap_or([1.0; 4]);
    let count = rig.indices.len() as u32 - start;
    let part = mesh as u16;
    rig.pieces.push(Piece { part, first: start, count, base: first, vertices: n as u32, targets, color });
    Ok(())
}

fn triangles(list: &[u32], mode: u32, base: u32, out: &mut Vec<u32>) -> Result<(), String> {
    let at = |k: usize| list[k] + base;
    match mode {
        4 => out.extend(list.iter().take(list.len() / 3 * 3).map(|i| i + base)),
        5 => (2..list.len()).for_each(|k| {
            let (a, b) = if k % 2 == 0 { (at(k - 2), at(k - 1)) } else { (at(k - 1), at(k - 2)) };
            out.extend([a, b, at(k)]);
        }),
        6 => (2..list.len()).for_each(|k| out.extend([at(0), at(k - 1), at(k)])),
        _ => return Err("draws primitives other than triangles".into()),
    }
    Ok(())
}

pub(super) fn picture(bin: &Bin) -> Option<Vec<u8>> {
    let doc = bin.doc;
    let chosen = doc.materials.iter().find_map(|m| m.pbr.as_ref()?.texture.as_ref()).map(|t| t.index);
    let image = chosen.and_then(|t| doc.textures.get(t)?.source).or((!doc.images.is_empty()).then_some(0))?;
    Some(bin.view(doc.images.get(image)?.view?).ok()?.to_vec())
}
