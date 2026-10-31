use crate::cosmic::{Notify, Overlaps};
use crate::presented::Rate;
use crate::scale::Scaling;
use crate::toplevel::Toplevel;
use crate::window::Parts;
use crate::{Cover, Event, Sink};
use smithay_client_toolkit::compositor::{CompositorState, Region};
use smithay_client_toolkit::output::{OutputInfo, OutputState};
use smithay_client_toolkit::reexports::client::backend::ObjectId;
use smithay_client_toolkit::reexports::client::globals::{Global, GlobalList};
use smithay_client_toolkit::reexports::client::protocol::wl_output::{Transform, WlOutput};
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::reexports::protocols::wp::presentation_time::client::wp_presentation::WpPresentation;
use smithay_client_toolkit::registry::RegistryState;
use smithay_client_toolkit::seat::SeatState;
use smithay_client_toolkit::seat::pointer::ThemedPointer;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell};
use smithay_client_toolkit::shm::Shm;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

mod screens;

const REOPENED: usize = 3;
const REOPEN_WINDOW: Duration = Duration::from_secs(60);

pub(crate) fn turned(info: &OutputInfo, (w, h): (i32, i32)) -> (u32, u32) {
    let (w, h) = (w.max(0) as u32, h.max(0) as u32);
    let turned = matches!(
        info.transform,
        Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270
    );
    if turned { (h, w) } else { (w, h) }
}

pub(crate) fn is_layer_shell(global: &Global) -> bool {
    global.interface == "zwlr_layer_shell_v1"
}

pub(crate) struct Screen {
    pub(crate) output: WlOutput,
    pub(crate) name: String,
    pub(crate) parts: Arc<Parts>,
    pub(crate) logical: Option<(u32, u32)>,
    pub(crate) integer: i32,
    pub(crate) fractional: Option<u32>,
    pub(crate) toplevel: bool,
    pub(crate) rate: Rate,
    pub(crate) overlaps: Overlaps,
}

pub(crate) struct State {
    pub(crate) registry: RegistryState,
    pub(crate) outputs: OutputState,
    pub(crate) seat: SeatState,
    pub(crate) pointer: Option<ThemedPointer>,
    pub(crate) buttons: u8,
    pub(crate) touch: Option<smithay_client_toolkit::reexports::client::protocol::wl_touch::WlTouch>,
    pub(crate) touching: Option<(i32, String, [f32; 2])>,
    pub(crate) shm: Shm,
    pub(crate) compositor: CompositorState,
    layers: LayerShell,
    scaling: Scaling,
    presentation: Option<WpPresentation>,
    overlap: Option<Notify>,
    pub(crate) screens: Vec<Screen>,
    pub(crate) toplevels: HashMap<ObjectId, Toplevel>,
    pub(crate) reopened: HashMap<String, Vec<Instant>>,
    pub(crate) reopen: Vec<WlOutput>,
    pub(crate) sink: Sink,
    conn: Connection,
    qh: QueueHandle<State>,
}

impl State {
    pub(crate) fn new(
        conn: Connection,
        globals: &GlobalList,
        qh: &QueueHandle<Self>,
        sink: Sink,
    ) -> anyhow::Result<Self> {
        let state = Self {
            registry: RegistryState::new(globals),
            outputs: OutputState::new(globals, qh),
            seat: SeatState::new(globals, qh),
            pointer: None,
            buttons: 0,
            touch: None,
            touching: None,
            shm: Shm::bind(globals, qh)?,
            compositor: CompositorState::bind(globals, qh)?,
            layers: LayerShell::bind(globals, qh)?,
            scaling: Scaling::bind(globals, qh),
            presentation: crate::presented::bind(globals, qh),
            overlap: crate::cosmic::bind(globals, qh),
            screens: Vec::new(),
            toplevels: HashMap::new(),
            reopened: HashMap::new(),
            reopen: Vec::new(),
            sink,
            conn,
            qh: qh.clone(),
        };
        crate::toplevel::bind(globals, qh);
        Ok(state)
    }

    pub(crate) fn add(&mut self, output: WlOutput) {
        let Some(info) = self.outputs.info(&output) else { return };
        let name = info.name.clone().unwrap_or_else(|| format!("wayland-{}", info.id));
        let (qh, surface) = (&self.qh, self.compositor.create_surface(&self.qh));
        let layer = self.layers.create_layer_surface(
            qh,
            surface,
            Layer::Background,
            Some("wallpaper"),
            Some(&output),
        );
        layer.set_anchor(Anchor::all());
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        let (viewport, fractional) = self.scaling.attach(layer.wl_surface(), qh);
        let empty = Region::new(&self.compositor).ok();
        layer.wl_surface().set_input_region(empty.as_ref().map(Region::wl_region));
        layer.commit();
        tracing::info!(output = name, "layer surface created");
        let overlaps = crate::cosmic::watch(self.overlap.as_ref(), &layer, qh);
        let presentation = self.presentation.clone();
        let wanted = Default::default();
        let parts = Arc::new(Parts { layer, viewport, fractional, empty, presentation, wanted });
        let (name, integer, fractional, rate) = (name.clone(), info.scale_factor, None, Rate::default());
        let screen = Screen {
            output,
            name: name.clone(),
            parts,
            logical: None,
            integer,
            fractional,
            toplevel: false,
            rate,
            overlaps,
        };
        self.screens.push(screen);
        for cover in [Cover::Toplevel, Cover::Overlap] {
            (self.sink)(Event::Covered(name.clone(), cover, false));
        }
        self.cover_outputs();
    }
}
