use super::{MAX_MODEL_VERTICES, Model, Primitive};
use crate::rigged::build::rest;
use crate::rigged::geometry::triangles;
use crate::rigged::math::{Mat, apply, mul};
use crate::rigged::read::Bin;

mod tangents;

fn roots(bin: &Bin) -> Result<Vec<(usize, Mat)>, String> {
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

fn normal_matrix(m: &Mat) -> ([f32; 9], bool) {
    let a = |r: usize, c: usize| m[r * 4 + c];
    let cof = |r: usize, c: usize| {
        let (r1, r2, c1, c2) = ((r + 1) % 3, (r + 2) % 3, (c + 1) % 3, (c + 2) % 3);
        a(r1, c1) * a(r2, c2) - a(r1, c2) * a(r2, c1)
    };
    let det: f32 = (0..3).map(|c| a(0, c) * cof(0, c)).sum();
    let s = if det < 0.0 { -1.0 } else { 1.0 };
    (std::array::from_fn(|k| cof(k / 3, k % 3) * s), det < 0.0)
}

fn turn(n: &[f32; 9], v: [f32; 3]) -> [f32; 3] {
    let o: [f32; 3] = std::array::from_fn(|r| n[r * 3] * v[0] + n[r * 3 + 1] * v[1] + n[r * 3 + 2] * v[2]);
    let l = o.iter().map(|x| x * x).sum::<f32>().sqrt();
    if l > 1e-12 { o.map(|x| x / l) } else { [0.0, 0.0, 1.0] }
}

pub(super) fn geometry(bin: &Bin, model: &mut Model) -> Result<(), String> {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for (n, world) in roots(bin)? {
        let Some(m) = bin.doc.nodes[n].mesh else { continue };
        let mesh = bin.doc.meshes.get(m).ok_or_else(|| format!("node {n} names the missing mesh {m}"))?;
        let (normals, flipped) = normal_matrix(&world);
        for p in &mesh.primitives {
            let at = |key: &str| p.attributes.get(key).copied();
            let position = at("POSITION").ok_or("has a primitive without positions")?;
            if model.vertex_count() + bin.count(position)? > MAX_MODEL_VERTICES {
                return Err(format!("has more than {MAX_MODEL_VERTICES} vertices"));
            }
            let positions = bin.floats(position, 3)?;
            let count = positions.len() / 3;
            let read = |key: &str, width: usize| at(key).map(|a| bin.floats(a, width)).transpose();
            let uvs = read("TEXCOORD_0", 2)?.unwrap_or_else(|| vec![0.0; count * 2]);
            let list = p.indices.map_or(Ok((0..count as u32).collect()), |a| bin.ints(a, 1))?;
            if list.iter().any(|i| *i as usize >= count) {
                return Err("has an index out of the range of its vertices".into());
            }
            let mut local = Vec::with_capacity(list.len());
            triangles(&list, p.mode.unwrap_or(4), 0, &mut local)?;
            if flipped {
                local.chunks_mut(3).for_each(|t| t.swap(1, 2));
            }
            let placed: Vec<[f32; 3]> =
                positions.chunks(3).map(|v| apply(&world, [v[0], v[1], v[2]])).collect();
            let given = read("NORMAL", 3)?.filter(|v| v.len() == count * 3);
            let normals: Vec<[f32; 3]> = match given {
                Some(v) => v.chunks(3).map(|n| turn(&normals, [n[0], n[1], n[2]])).collect(),
                None => tangents::smooth(&placed, &local),
            };
            let given = read("TANGENT", 4)?.filter(|v| v.len() == count * 4);
            let tangents = match given {
                Some(t) => t.chunks(4).map(|t| tangents::along(&world, t, flipped)).collect(),
                None => tangents::tangents(&placed, &normals, &uvs, &local),
            };
            if uvs.len() != count * 2 {
                return Err("has a primitive whose attributes count different vertices".into());
            }
            let base = model.vertex_count() as u32;
            for v in 0..count {
                let at = placed[v];
                (0..3).for_each(|k| (lo[k], hi[k]) = (lo[k].min(at[k]), hi[k].max(at[k])));
                model.vertices.extend_from_slice(&at);
                model.vertices.extend_from_slice(&normals[v]);
                model.vertices.extend_from_slice(&uvs[v * 2..v * 2 + 2]);
                model.vertices.extend_from_slice(&tangents[v]);
            }
            let first = model.indices.len() as u32;
            model.indices.extend(local.iter().map(|i| i + base));
            let material = p.material.filter(|m| *m < model.materials.len()).map(|m| m as u16);
            let count = model.indices.len() as u32 - first;
            model.triangles += u64::from(count / 3);
            model.primitives.push(Primitive { first, count, material });
        }
    }
    model.bounds = if lo[0] <= hi[0] { [lo, hi] } else { [[0.0; 3]; 2] };
    Ok(())
}
