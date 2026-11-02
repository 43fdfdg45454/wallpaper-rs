use super::SceneState;
use wprs_render::Renderer;
use wprs_rgb::Strip;

fn allowed(output: &str) -> bool {
    crate::rgb::enabled() && crate::inputs::lights(output)
}

impl SceneState {
    pub(in crate::compositor) fn want_rgb(&mut self, output: &str) -> bool {
        let Some(view) = self.view.as_mut() else { return false };
        let lit = wprs_render::lit(&self.spec.pack.scene);
        view.want_rgb(lit && (output.starts_with("preview:") || allowed(output)))
    }

    pub(in crate::compositor) fn send_rgb(&mut self, output: &str, renderer: &Renderer) {
        if self.want_rgb(output) {
            self.due = Some(std::time::Instant::now());
        }
        let Some(view) = self.view.as_mut() else { return };
        if let Some(strip) = view.rgb(renderer).and_then(|(w, h, rgb)| Strip::new(w, h, rgb)) {
            crate::rgb::frame(output, strip, allowed(output) && !output.starts_with("preview:"));
        }
        if view.rgb_pending() {
            self.due = Some(std::time::Instant::now() + std::time::Duration::from_millis(30));
        }
    }
}
