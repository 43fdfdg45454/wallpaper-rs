use super::{B, SceneView, buffer};
use crate::{RenderError, Renderer};
use wprs_scene::{ROPE_POINTS, Strand};

pub type Strands = (Vec<Strand>, Vec<[u32; 4]>);

impl Renderer {
    pub fn strands(&self, view: &SceneView) -> Result<Vec<Strands>, RenderError> {
        let Some(swarms) = &view.swarms else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for (herd, flight) in swarms.herds.iter().zip(&swarms.flights) {
            let Some(flight) = flight else { continue };
            for (p, store) in flight.stores.iter().enumerate() {
                let marks = u64::from(herd.bake.allocs[p]) * u64::from(herd.bake.trails[p]) * 16;
                if !flight.clouds.is_empty() {
                    let cloud = &flight.clouds[p];
                    out.push((cloud.rope.clone(), cloud.trail.clone()));
                    continue;
                }
                let rope = match &store.rope {
                    Some(b) => self.fetch(b, super::flight::ROPE_AT, ROPE_POINTS as u64 * 48)?,
                    None => Vec::new(),
                };
                let trail = match (&store.gpu, marks) {
                    (Some((state, ..)), 1..) => {
                        self.fetch(state, (4 + u64::from(herd.bake.marks[p])) * 4, marks)?
                    }
                    _ => Vec::new(),
                };
                out.push((bytemuck::pod_collect_to_vec(&rope), bytemuck::pod_collect_to_vec(&trail)));
            }
        }
        Ok(out)
    }

    fn fetch(&self, from: &wgpu::Buffer, at: u64, size: u64) -> Result<Vec<u8>, RenderError> {
        let staging = buffer(self, size, B::MAP_READ | B::COPY_DST);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(from, at, &staging, 0, size);
        let done = self.queue.submit([encoder.finish()]);
        self.map(&staging.slice(..), &Default::default(), done)?;
        let data = staging.slice(..).get_mapped_range().map_err(|e| RenderError::Readback(e.to_string()))?;
        Ok(data.to_vec())
    }
}
