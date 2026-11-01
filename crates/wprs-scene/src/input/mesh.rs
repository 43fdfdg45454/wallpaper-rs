pub const MESH_FLOATS: usize = 8;
pub const MAX_MESH_BYTES: usize = 1 << 20;

#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub generation: u64,
    pub shapes: u32,
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
}

impl PartialEq for Mesh {
    fn eq(&self, other: &Mesh) -> bool {
        self.generation == other.generation
    }
}

impl Mesh {
    pub fn count(&self) -> usize {
        self.vertices.len() / MESH_FLOATS
    }

    pub fn fits(&self, pool: &crate::Pool) -> bool {
        let count = self.count();
        self.shapes <= pool.max
            && count <= pool.max_vertices as usize
            && self.indices.len().is_multiple_of(3)
            && self.indices.iter().all(|i| (*i as usize) < count)
            && self.vertices.iter().all(|v| v.is_finite())
    }
}
