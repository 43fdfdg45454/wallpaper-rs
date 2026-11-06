use wprs_core::rgb::{Flow, Place, Rgb};
use wprs_rgb::{Area, Device};

fn flow(flow: Flow) -> wprs_rgb::Flow {
    match flow {
        Flow::Right => wprs_rgb::Flow::Right,
        Flow::Left => wprs_rgb::Flow::Left,
        Flow::Down => wprs_rgb::Flow::Down,
        Flow::Up => wprs_rgb::Flow::Up,
    }
}

fn placed(place: Place) -> Option<Area> {
    match place {
        Place::Off => None,
        Place::At { rect, flow: f } => Some(Area { rect, flow: flow(f) }),
    }
}

pub(super) fn area(rgb: &Rgb, device: &Device) -> Area {
    rgb.place(&device.key).and_then(placed).unwrap_or_default()
}

pub(super) fn zone(rgb: &Rgb, device: &Device, zone: usize) -> Option<Option<Area>> {
    rgb.zone_place(&device.key, zone).map(placed)
}
