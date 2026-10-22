use crate::Event;
use crate::state::{Screen, State};
use smithay_client_toolkit::compositor::CompositorHandler;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::protocol::{
    wl_output, wl_output::WlOutput, wl_surface::WlSurface,
};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::SeatState;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure};
use smithay_client_toolkit::{delegate_dispatch2, delegate_registry, registry_handlers};

type Queue = QueueHandle<State>;

fn on(surface: &WlSurface) -> impl Fn(&Screen) -> bool {
    move |s| s.parts.layer.wl_surface() == surface
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }

    fn new_output(&mut self, _: &Connection, _: &Queue, output: WlOutput) {
        self.add(output)
    }

    fn update_output(&mut self, _: &Connection, _: &Queue, output: WlOutput) {
        self.changed(|s| s.output == output, |_| {})
    }

    fn output_destroyed(&mut self, _: &Connection, _: &Queue, output: WlOutput) {
        self.remove(|s| s.output == output)
    }
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &Queue, layer: &LayerSurface) {
        self.remove(|s| s.parts.layer == *layer)
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &Queue,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        let output = self.screens.iter().find(|s| s.parts.layer == *layer).map(|s| s.output.clone());
        let fallback = output.and_then(|o| self.outputs.info(&o)?.logical_size).unwrap_or((1, 1));
        let pick = |given: u32, fallback: i32| if given > 0 { given } else { fallback.max(1) as u32 };
        let (width, height) = configure.new_size;
        let logical = (pick(width, fallback.0), pick(height, fallback.1));
        self.changed(|s| s.parts.layer == *layer, |s| s.logical = Some(logical))
    }
}

impl CompositorHandler for State {
    fn scale_factor_changed(&mut self, _: &Connection, _: &Queue, surface: &WlSurface, factor: i32) {
        self.changed(on(surface), |s| s.integer = factor)
    }

    fn transform_changed(&mut self, _: &Connection, _: &Queue, _: &WlSurface, _: wl_output::Transform) {}

    fn frame(&mut self, _: &Connection, _: &Queue, surface: &WlSurface, _: u32) {
        let screen = self.screens.iter().find(|s| on(surface)(s));
        screen.into_iter().for_each(|s| (self.sink)(Event::Frame(s.name.clone())))
    }

    fn surface_enter(&mut self, _: &Connection, _: &Queue, _: &WlSurface, _: &WlOutput) {}

    fn surface_leave(&mut self, _: &Connection, _: &Queue, _: &WlSurface, _: &WlOutput) {}
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }

    registry_handlers![OutputState, SeatState];
}

delegate_registry!(State);
delegate_dispatch2!(State);
