use crate::wire::{Cursor, Wire};
use rquickjs::{Array, Function, Object, Result, TypedArray};
use std::sync::Arc;
use wprs_scene::{MESH_FLOATS, Mesh, ScriptOutputs};

fn bytes<'a, T>(array: &'a TypedArray<'_, T>) -> &'a [u8] {
    array.as_raw().map_or(&[], |raw| unsafe { raw.as_ref() })
}

pub(crate) fn take<'js>(state: &Object<'js>, blob: &mut Vec<u8>) -> Result<()> {
    blob.clear();
    let Some(pools) = state.get::<_, Function>("shapes")?.call::<_, Option<Array>>(())? else {
        return Ok(());
    };
    let w = &mut Wire(std::mem::take(blob));
    w.u16(pools.len());
    for pool in pools.iter::<Array>() {
        let pool = pool?;
        let shapes: Array = pool.get(1)?;
        w.str(&pool.get::<String>(0)?).u16(shapes.len());
        for shape in shapes.iter::<Array>() {
            let shape = shape?;
            let (vertices, indices): (TypedArray<f32>, TypedArray<u32>) = (shape.get(1)?, shape.get(2)?);
            let (v, i) = (bytes(&vertices), bytes(&indices));
            w.u8(shape.get(0)?).u32(v.len() as u32);
            w.0.extend_from_slice(v);
            w.u32(i.len() as u32);
            w.0.extend_from_slice(i);
        }
    }
    *blob = std::mem::take(&mut w.0);
    Ok(())
}

fn floats(raw: &[u8]) -> impl Iterator<Item = f32> + '_ {
    raw.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b))
}

fn widen(raw: &[u8], stride: usize, out: &mut Vec<f32>) {
    let values: Vec<f32> = floats(raw).collect();
    for v in values.chunks_exact(stride) {
        let (uv, rgba) = match stride {
            2 => ([0.0; 2], [1.0; 4]),
            4 => ([v[2], v[3]], [1.0; 4]),
            6 => ([0.0; 2], [v[2], v[3], v[4], v[5]]),
            _ => ([v[2], v[3]], [v[4], v[5], v[6], v[7]]),
        };
        out.extend([v[0], v[1], uv[0], uv[1], rgba[0], rgba[1], rgba[2], rgba[3]]);
    }
}

pub(crate) fn read(blob: &[u8], outputs: &mut ScriptOutputs, generation: &mut u64) -> std::io::Result<()> {
    if blob.is_empty() {
        return Ok(());
    }
    let c = &mut Cursor(blob);
    for _ in 0..c.u16()? {
        let pool = c.str()?.to_owned();
        let shapes = c.u16()?;
        *generation += 1;
        let mut mesh = Mesh { generation: *generation, shapes: shapes as u32, ..Mesh::default() };
        let mut named = true;
        for _ in 0..shapes {
            let stride = usize::from(c.u8()?);
            let (n, base) = (c_len(c)?, mesh.count() as u32);
            let vertices = c.take(n)?;
            let n = c_len(c)?;
            let indices = c.take(n)?;
            if ![2, 4, 6, 8].contains(&stride) || vertices.len() % (4 * stride) != 0 {
                return Err(std::io::Error::other("a shape of the wrong format"));
            }
            widen(vertices, stride, &mut mesh.vertices);
            let own = (vertices.len() / (4 * stride)) as u32;
            let found = indices.as_chunks::<4>().0.iter().map(|b| u32::from_le_bytes(*b));
            named &= found.clone().all(|i| i < own);
            mesh.indices.extend(found.map(|i| i.saturating_add(base)));
        }
        debug_assert_eq!(mesh.vertices.len() % MESH_FLOATS, 0);
        match named {
            true => drop(outputs.meshes.insert(pool, Arc::new(mesh))),
            false => drop(outputs.meshes.remove(&pool)),
        }
    }
    Ok(())
}

fn c_len(c: &mut Cursor<'_>) -> std::io::Result<usize> {
    c.u32().map(|n| n as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(shapes: &[(&[f32], &[u32])]) -> Vec<u8> {
        let w = &mut Wire(Vec::new());
        w.u16(1).str("bars").u16(shapes.len());
        for (vertices, indices) in shapes {
            w.u8(2).u32(4 * vertices.len() as u32);
            vertices.iter().for_each(|v| w.0.extend_from_slice(&v.to_le_bytes()));
            w.u32(4 * indices.len() as u32);
            indices.iter().for_each(|i| w.0.extend_from_slice(&i.to_le_bytes()));
        }
        std::mem::take(&mut w.0)
    }

    #[test]
    fn a_shape_may_only_name_its_own_vertices() {
        let (mut outputs, mut generation) = (ScriptOutputs::default(), 0);
        let triangle: &[f32] = &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        read(&pool(&[(triangle, &[0, 1, 2]), (triangle, &[0, 1, 2])]), &mut outputs, &mut generation)
            .unwrap();
        assert_eq!(outputs.meshes["bars"].indices, [0, 1, 2, 3, 4, 5]);
        read(&pool(&[(triangle, &[0, 1, 5]), (triangle, &[0, 1, 2])]), &mut outputs, &mut generation)
            .unwrap();
        assert!(!outputs.meshes.contains_key("bars"), "a shape reached into the next one");
    }
}
