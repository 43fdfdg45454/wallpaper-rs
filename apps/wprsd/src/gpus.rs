use wprs_core::GpuChoice;
use wprs_core::model::GpuAdapter;
use wprs_render::{Chosen, Fallback, Renderer};

#[derive(Default)]
pub(crate) struct Gpus {
    pub(crate) adapters: Vec<GpuAdapter>,
    pub(crate) warning: Option<String>,
    started: GpuChoice,
}

impl Gpus {
    pub(crate) fn start(chosen: Chosen, choice: &GpuChoice) -> (Gpus, Renderer) {
        let warning = chosen.fallback.map(|why| match why {
            Fallback::Missing => crate::words::gpu_missing(choice.as_str()),
            Fallback::Unusable => crate::words::gpu_unusable(choice.as_str()),
        });
        let gpus = Gpus { adapters: chosen.adapters, warning, started: choice.clone() };
        (gpus, chosen.renderer)
    }

    pub(crate) fn restart(&self, configured: &GpuChoice) -> bool {
        !configured.same(&self.started)
    }
}
