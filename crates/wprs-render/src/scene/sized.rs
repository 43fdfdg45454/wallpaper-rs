use super::SceneView;
use crate::Renderer;
use wprs_scene::Quality;

impl SceneView {
    pub fn inner(&self, r: &Renderer, size: (u32, u32), quality: Quality) -> (u32, u32) {
        let scale = quality.render_scale() * r.render_scale(size) * self.shrink.0;
        let side = |v: u32| (v as f32 * scale).round().max(1.0) as u32;
        (side(size.0), side(size.1))
    }

    pub fn shrunk(&self) -> f32 {
        self.shrink.0
    }

    pub(super) fn pace(&mut self, r: &Renderer) {
        if std::mem::take(&mut self.clock.fresh) {
            self.shrink = crate::timeout::shrink(self.shrink, self.clock.gpu_ms, r.gpu_timeout());
        }
    }
}
