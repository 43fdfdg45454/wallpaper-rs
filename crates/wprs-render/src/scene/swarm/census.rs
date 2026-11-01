use super::{B, SceneView, buffer};
use crate::{RenderError, Renderer};
use wprs_scene::{Census, Particle};

pub type Herded = (String, Vec<Vec<Particle>>);

impl Renderer {
    pub fn census(&self, view: &SceneView) -> Result<Vec<(String, Census)>, RenderError> {
        let all = self.particles(view)?;
        Ok(all
            .into_iter()
            .map(|(id, parts)| (id, Census::join(&parts.iter().map(|p| Census::of(p)).collect::<Vec<_>>())))
            .collect())
    }

    pub fn particles(&self, view: &SceneView) -> Result<Vec<Herded>, RenderError> {
        let Some(swarms) = &view.swarms else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for (herd, flight) in swarms.herds.iter().zip(&swarms.flights) {
            let Some(flight) = flight else { continue };
            let system = herd.bake.systems[0];
            let id =
                swarms.kits.iter().find(|k| k.system == system).map(|k| k.id.clone()).unwrap_or_default();
            if !flight.clouds.is_empty() {
                out.push((id, flight.clouds.iter().map(|c| c.particles.clone()).collect()));
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
                parts.push(bytemuck::cast_slice(&data).to_vec());
            }
            out.push((id, parts));
        }
        Ok(out)
    }
}
