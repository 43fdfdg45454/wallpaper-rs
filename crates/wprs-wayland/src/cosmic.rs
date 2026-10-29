use crate::cover::covers;
use crate::state::State;
use crate::x11::Rect;
use crate::{Cover, Event};
use list::{ext_foreign_toplevel_handle_v1 as handle, ext_foreign_toplevel_list_v1 as manager};
use overlap::zcosmic_overlap_notification_v1::{
    Event as Overlap, ZcosmicOverlapNotificationV1 as Notification,
};
pub(crate) use overlap::zcosmic_overlap_notify_v1::ZcosmicOverlapNotifyV1 as Notify;
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::{
    Connection, Dispatch, Proxy, QueueHandle, event_created_child,
};
use smithay_client_toolkit::reexports::protocols::ext::foreign_toplevel_list::v1::client as list;
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{LayerSurface, SurfaceKind};
use std::collections::HashMap;

#[allow(dead_code, non_camel_case_types, unused_imports, non_upper_case_globals, clippy::all)]
mod overlap {
    use smithay_client_toolkit::reexports::client as wayland_client;
    use smithay_client_toolkit::reexports::protocols::ext::foreign_toplevel_list::v1::client::*;
    use smithay_client_toolkit::reexports::protocols_wlr::layer_shell::v1::client::*;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use smithay_client_toolkit::reexports::client::protocol::__interfaces::*;
        use smithay_client_toolkit::reexports::protocols::ext::foreign_toplevel_list::v1::client::__interfaces::*;
        use smithay_client_toolkit::reexports::protocols_wlr::layer_shell::v1::client::__interfaces::*;
        wayland_scanner::generate_interfaces!("cosmic-overlap.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("cosmic-overlap.xml");
}

#[derive(Default)]
pub(crate) struct Overlaps {
    notification: Option<Notification>,
    toplevels: HashMap<u32, Rect>,
    panels: HashMap<String, Rect>,
    covered: bool,
}

impl Drop for Overlaps {
    fn drop(&mut self) {
        self.notification.iter().for_each(Notification::destroy);
    }
}

pub(crate) fn bind(globals: &GlobalList, qh: &QueueHandle<State>) -> Option<Notify> {
    let notify = globals.bind::<Notify, _, _>(qh, 1..=1, ()).ok()?;
    let listed = globals.bind::<manager::ExtForeignToplevelListV1, _, _>(qh, 1..=1, ());
    listed.inspect_err(|e| tracing::debug!("no toplevel list for overlap notices: {e}")).ok()?;
    Some(notify)
}

pub(crate) fn watch(notify: Option<&Notify>, layer: &LayerSurface, qh: &QueueHandle<State>) -> Overlaps {
    let wlr = match layer.kind() {
        SurfaceKind::Wlr(wlr) => Some(wlr),
        _ => None,
    };
    let surface = layer.wl_surface().clone();
    let notification = notify.zip(wlr).map(|(n, wlr)| n.notify_on_overlap(wlr, qh, surface));
    let (toplevels, panels, covered) = Default::default();
    Overlaps { notification, toplevels, panels, covered }
}

pub(crate) fn covering(area: Rect, overlaps: &Overlaps) -> bool {
    let rects: Vec<Rect> = overlaps.toplevels.values().chain(overlaps.panels.values()).copied().collect();
    !overlaps.toplevels.is_empty() && covers(area, &rects)
}

fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
    Rect { x, y, width: width.max(0) as u32, height: height.max(0) as u32 }
}

impl Dispatch<Notification, WlSurface> for State {
    fn event(
        state: &mut Self,
        _: &Notification,
        event: Overlap,
        surface: &WlSurface,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(screen) = state.screens.iter_mut().find(|s| s.parts.layer.wl_surface() == surface) else {
            return;
        };
        let overlaps = &mut screen.overlaps;
        match event {
            Overlap::ToplevelEnter { toplevel, x, y, width, height } => {
                overlaps.toplevels.insert(toplevel.id().protocol_id(), rect(x, y, width, height));
            }
            Overlap::ToplevelLeave { toplevel } => {
                overlaps.toplevels.remove(&toplevel.id().protocol_id());
            }
            Overlap::LayerEnter { identifier, exclusive, x, y, width, height, .. } if exclusive != 0 => {
                overlaps.panels.insert(identifier, rect(x, y, width, height));
            }
            Overlap::LayerEnter { identifier, .. } | Overlap::LayerLeave { identifier } => {
                overlaps.panels.remove(&identifier);
            }
        }
        let (width, height) = screen.logical.unwrap_or_default();
        let now = covering(Rect { x: 0, y: 0, width, height }, &screen.overlaps);
        if std::mem::replace(&mut screen.overlaps.covered, now) != now {
            (state.sink)(Event::Covered(screen.name.clone(), Cover::Overlap, now));
        }
    }
}

impl Dispatch<manager::ExtForeignToplevelListV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &manager::ExtForeignToplevelListV1,
        _: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(State, manager::ExtForeignToplevelListV1, [manager::EVT_TOPLEVEL_OPCODE => (handle::ExtForeignToplevelHandleV1, ())]);
}

impl Dispatch<handle::ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        _: &mut Self,
        proxy: &handle::ExtForeignToplevelHandleV1,
        event: handle::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let handle::Event::Closed = event {
            proxy.destroy();
        }
    }
}

impl Dispatch<Notify, ()> for State {
    fn event(
        _: &mut Self,
        _: &Notify,
        _: overlap::zcosmic_overlap_notify_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_and_panels_that_fill_the_background_cover_it() {
        let area = Rect { x: 0, y: 0, width: 1920, height: 1080 };
        let mut overlaps = Overlaps::default();
        overlaps.panels.insert("panel".into(), rect(0, 0, 1920, 32));
        assert!(!covering(area, &overlaps), "panels alone never cover");
        overlaps.toplevels.insert(1, rect(0, 32, 1920, 1048));
        assert!(covering(area, &overlaps));
        overlaps.toplevels.insert(1, rect(0, 32, 1900, 1048));
        assert!(!covering(area, &overlaps));
    }
}
