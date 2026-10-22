use super::output::Sink;
use super::state::State;
use std::time::Instant;
use wprs_render::{Layer, RenderError, Renderer, Surface};
use wprs_wayland::{Cover, Pacer, Window};

pub(super) struct Screen {
    surface: Option<Surface>,
    window: Window,
    pacer: Pacer,
    fresh: bool,
}

impl Screen {
    pub(super) fn new(window: Window) -> Self {
        Self { surface: None, window, pacer: Pacer::default(), fresh: true }
    }

    pub(super) fn ready(&self) -> bool {
        self.pacer.ready()
    }

    pub(super) fn present(
        &mut self,
        renderer: &Renderer,
        layers: &[Layer<'_>],
        name: &str,
    ) -> Result<bool, RenderError> {
        let (window, size, fresh) = (&self.window, self.window.size(), &mut self.fresh);
        let surface = match &mut self.surface {
            Some(surface) => surface,
            empty => {
                let surface = renderer.create_surface(window.clone(), size)?;
                tracing::debug!(output = name, mode = ?surface.present_mode(), "surface ready");
                empty.insert(surface)
            }
        };
        let presented = renderer.present_layers(surface, size, layers, || {
            if std::mem::take(fresh) {
                window.apply();
            }
            window.frame()
        });
        let presented = presented.inspect_err(|_| self.surface = None)?;
        if !presented {
            window.frame();
            window.commit();
        }
        self.pacer.requested(Instant::now());
        Ok(presented)
    }
}

impl State {
    pub(super) fn attach(&mut self, name: String, window: Window) {
        let Some(output) = self.outputs.get_mut(&name) else {
            self.windows.insert(name, window);
            return;
        };
        match &mut output.sink {
            Sink::Surface(screen) if screen.window.same_surface(&window) => {
                let moved = screen.window.geometry() != window.geometry();
                (screen.window, screen.fresh) = (window, screen.fresh || moved);
                output.dirty |= moved;
            }
            sink => (*sink, output.dirty) = (Sink::Surface(Screen::new(window)), true),
        }
    }

    pub(super) fn frame_done(&mut self, name: &str) {
        let Some(Sink::Surface(screen)) = self.outputs.get_mut(name).map(|o| &mut o.sink) else { return };
        if screen.pacer.done() {
            screen.window.report(Cover::Starved, false);
        }
    }

    pub(super) fn starvation(&self) -> impl Iterator<Item = Instant> + '_ {
        self.outputs.values().filter(|o| o.active && o.animated()).filter_map(|o| match &o.sink {
            Sink::Surface(screen) => screen.pacer.deadline(screen.window.refresh()),
            Sink::Stream(_) => None,
        })
    }

    pub(super) fn check_starvation(&mut self) {
        let now = Instant::now();
        for output in self.outputs.values_mut().filter(|o| o.active && o.animated()) {
            if let Sink::Surface(screen) = &mut output.sink
                && screen.pacer.starve(screen.window.refresh(), now)
            {
                screen.window.report(Cover::Starved, true);
            }
        }
    }
}
