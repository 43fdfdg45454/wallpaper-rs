use super::{MAX_MODEL_VERTICES, Model, Primitive};
use crate::rigged::build::rest;
use crate::rigged::math::{Mat, mul};
use crate::rigged::read::Bin;

pub(crate) mod part;
mod tangents;

pub(super) fn roots(bin: &Bin) -> Result<Vec<(usize, Mat)>, String> {
    let nodes = &bin.doc.nodes;
    let mut child = vec![false; nodes.len()];
    for (i, n) in nodes.iter().enumerate() {
        for &c in &n.children {
            if c >= nodes.len() || c == i || std::mem::replace(&mut child[c], true) {
                return Err("has a node cycle, a missing child or a node with two parents".into());
            }
        }
    }
    let mut out: Vec<(usize, Mat)> = Vec::with_capacity(nodes.len());
    for (i, n) in nodes.iter().enumerate().filter(|(i, _)| !child[*i]) {
        out.push((i, rest(n)?.matrix()));
    }
    let mut at = 0;
    while let Some(&(node, world)) = out.get(at) {
        for &c in &nodes[node].children {
            out.push((c, mul(&world, &rest(&nodes[c])?.matrix())));
        }
        at += 1;
        if out.len() > nodes.len() {
            return Err("has a node cycle".into());
        }
    }
    Ok(out)
}

pub(super) fn append(model: &mut Model, p: &part::Part, material: Option<usize>) -> Primitive {
    let base = model.vertex_count() as u32;
    for v in 0..p.at.len() {
        model.vertices.extend_from_slice(&p.at[v]);
        model.vertices.extend_from_slice(&p.normals[v]);
        model.vertices.extend_from_slice(&p.uvs[v * 2..v * 2 + 2]);
        model.vertices.extend_from_slice(&p.tangents[v]);
    }
    match &p.paint {
        Some(paint) => model.paint.extend_from_slice(paint),
        None => model.paint.extend(std::iter::repeat_n(-1.0, p.at.len())),
    }
    let first = model.indices.len() as u32;
    model.indices.extend(p.list.iter().map(|i| i + base));
    let material = material.filter(|m| *m < model.materials.len()).map(|m| m as u16);
    let count = model.indices.len() as u32 - first;
    model.triangles += u64::from(count / 3);
    Primitive { first, count, material, base, vertices: p.at.len() as u32, piece: u16::MAX }
}

pub(super) fn geometry(bin: &Bin, model: &mut Model) -> Result<(), String> {
    for (n, world) in roots(bin)? {
        let Some(m) = bin.doc.nodes[n].mesh else { continue };
        let mesh = bin.doc.meshes.get(m).ok_or_else(|| format!("node {n} names the missing mesh {m}"))?;
        for p in &mesh.primitives {
            let part = part::part(bin, p, MAX_MODEL_VERTICES - model.vertex_count(), &world)?;
            let prim = append(model, &part, p.material);
            model.primitives.push(prim);
        }
    }
    model.bounds = bounds(model.vertices.chunks(super::MODEL_FLOATS).map(|v| [v[0], v[1], v[2]]));
    Ok(())
}

pub(super) fn bounds(points: impl Iterator<Item = [f32; 3]>) -> [[f32; 3]; 2] {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for at in points {
        (0..3).for_each(|k| (lo[k], hi[k]) = (lo[k].min(at[k]), hi[k].max(at[k])));
    }
    if lo[0] <= hi[0] { [lo, hi] } else { [[0.0; 3]; 2] }
}
