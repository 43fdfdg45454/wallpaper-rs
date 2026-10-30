use super::{B, SceneView, buffer};
use crate::{RenderError, Renderer};
use wprs_scene::Census;

impl Renderer {
    pub fn census(&self, view: &SceneView) -> Result<Vec<(String, Census)>, RenderError> {
        let Some(swarms) = &view.swarms else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for (herd, flight) in swarms.herds.iter().zip(&swarms.flights) {
            let Some(flight) = flight else { continue };
            let system = herd.bake.systems[0];
            let id =
                swarms.kits.iter().find(|k| k.system == system).map(|k| k.id.clone()).unwrap_or_default();
            if !flight.clouds.is_empty() {
                let parts: Vec<Census> = flight.clouds.iter().map(|c| Census::of(&c.particles)).collect();
                out.push((id, Census::join(&parts)));
                continue;
            }
            let mut parts = Vec::new();
            for store in &flight.stores {
                let size = store.particles.size();
                let staging = buffer(self, size, B::MAP_READ | B::COPY_DST);
                let mut encoder = self.device.create_command_encoder(&Default::default());
                encoder.copy_buffer_to_buffer(&store.particles, 0, &staging, 0, size);
                let done = self.queue.submit([encoder.finish()]);
                self.map(&staging.slice(..), &Default::default(), done)?;
                let data =
                    staging.slice(..).get_mapped_range().map_err(|e| RenderError::Readback(e.to_string()))?;
                parts.push(Census::of(bytemuck::cast_slice(&data)));
            }
            out.push((id, Census::join(&parts)));
        }
        Ok(out)
    }
}
