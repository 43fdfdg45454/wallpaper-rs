use crate::cosmic::{Notify, Overlaps};
use crate::cover::{allowed_at, rationed};
use crate::presented::Rate;
use crate::scale::{Scaling, effective};
use crate::toplevel::Toplevel;
use crate::window::{Display, Native, Parts, Window};
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

    pub(crate) fn closed(&mut self, gone: impl Fn(&Screen) -> bool) {
        let Some(screen) = self.screens.iter().find(|s| gone(s)) else { return };
        let history = self.reopened.get(&screen.name).map_or(&[][..], Vec::as_slice);
        let later = allowed_at(history, REOPENED, REOPEN_WINDOW).filter(|at| *at > Instant::now());
        if let Some(wait) = later.map(|at| at.saturating_duration_since(Instant::now()).as_secs()) {
            tracing::info!(
                output = screen.name,
                wait,
                "the background surface was closed too often; waiting"
            );
        }
        self.reopen.push(screen.output.clone());
        self.remove(gone);
    }

    pub(crate) fn reopen(&mut self) {
        let mut waiting = Vec::new();
        for output in std::mem::take(&mut self.reopen) {
            let name = self
                .outputs
                .info(&output)
                .and_then(|i| i.name)
                .filter(|_| self.outputs.outputs().any(|o| o == output));
            let Some(name) = name.filter(|n| self.screens.iter().all(|s| s.name != *n)) else { continue };
            let history = self.reopened.entry(name.clone()).or_default();
            if !rationed(history, Instant::now(), REOPENED, REOPEN_WINDOW) {
                waiting.push(output);
                continue;
            }
            tracing::info!(output = name, "the compositor closed the background surface; creating it again");
            self.add(output);
        }
        self.reopen = waiting;
    }

    pub(crate) fn retry_at(&self) -> Option<Instant> {
        let names = self.reopen.iter().filter_map(|o| self.outputs.info(o)?.name);
        names.filter_map(|n| allowed_at(self.reopened.get(&n)?, REOPENED, REOPEN_WINDOW)).min()
    }

    pub(crate) fn remove(&mut self, gone: impl Fn(&Screen) -> bool) {
        let removed: Vec<_> = self.screens.extract_if(.., |s| gone(s)).collect();
        for screen in removed {
            match self.screens.iter().rev().find(|s| s.name == screen.name).map(|s| s.parts.layer.clone()) {
                Some(twin) => self.changed(|s| s.parts.layer == twin, |_| {}),
                None => (self.sink)(Event::Gone(screen.name)),
            }
        }
    }

    pub(crate) fn changed(&mut self, matches: impl Fn(&Screen) -> bool, change: impl FnOnce(&mut Screen)) {
        let Some(screen) = self.screens.iter_mut().find(|s| matches(s)) else { return };
        change(screen);
        let Some(logical) = screen.logical else { return };
        let info = self.outputs.info(&screen.output);
        let mode = info.as_ref().and_then(|i| i.modes.iter().find(|m| m.current));
        let window = Window {
            native: Native::Layer(screen.parts.clone(), self.qh.clone()),
            server: Display::Wayland(self.conn.clone()),
            sink: self.sink.clone(),
            name: screen.name.clone(),
            logical,
            scale: effective(screen.fractional, screen.integer, screen.parts.viewport.is_some()),
            refresh: screen.rate.current(mode.map_or(0, |m| m.refresh_rate.max(0) as u32)),
            position: info.as_ref().map_or((0, 0), |i| i.logical_position.unwrap_or(i.location)),
            mm: info.as_ref().map_or((0, 0), |i| turned(i, i.physical_size)),
            identity: info
                .as_ref()
                .map_or(String::new(), |i| format!("{} {}", i.make, i.model).trim().to_owned()),
        };
        (self.sink)(Event::Output(screen.name.clone(), window));
    }
}
