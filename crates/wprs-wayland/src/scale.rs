use crate::state::State;
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::{Connection, Dispatch, QueueHandle, delegate_noop};
use smithay_client_toolkit::reexports::protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1, wp_fractional_scale_v1,
};
use smithay_client_toolkit::reexports::protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wp_fractional_scale_v1::WpFractionalScaleV1;
use wp_viewport::WpViewport;

pub(crate) const UNIT: u32 = 120;

pub(crate) struct Scaling(Option<wp_viewporter::WpViewporter>, Option<WpFractionalScaleManagerV1>);

impl Scaling {
    pub(crate) fn bind(globals: &GlobalList, qh: &QueueHandle<State>) -> Self {
        Self(globals.bind(qh, 1..=1, ()).ok(), globals.bind(qh, 1..=1, ()).ok())
    }

    pub(crate) fn attach(
        &self,
        surface: &WlSurface,
        qh: &QueueHandle<State>,
    ) -> (Option<WpViewport>, Option<WpFractionalScaleV1>) {
        let viewport = self.0.as_ref().map(|v| v.get_viewport(surface, qh, ()));
        (viewport, self.1.as_ref().map(|m| m.get_fractional_scale(surface, qh, ())))
    }
}

pub(crate) fn buffer_size((width, height): (u32, u32), scale: u32) -> (u32, u32) {
    let scaled = |v: u32| ((u64::from(v) * u64::from(scale) + u64::from(UNIT / 2)) / u64::from(UNIT)).max(1);
    (scaled(width) as u32, scaled(height) as u32)
}

pub(crate) fn effective(fractional: Option<u32>, integer: i32, viewport: bool) -> u32 {
    match fractional.filter(|s| viewport && *s > 0) {
        Some(scale) => scale,
        None => integer.max(1) as u32 * UNIT,
    }
}

impl Dispatch<WpFractionalScaleV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            state.changed(|s| s.parts.fractional.as_ref() == Some(proxy), |s| s.fractional = Some(scale));
        }
    }
}

delegate_noop!(State: WpFractionalScaleManagerV1);
delegate_noop!(State: wp_viewporter::WpViewporter);
delegate_noop!(State: WpViewport);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffers_follow_the_fractional_scale() {
        assert_eq!(buffer_size((1280, 720), 120), (1280, 720));
        assert_eq!(buffer_size((1280, 720), 180), (1920, 1080));
        assert_eq!(buffer_size((1707, 960), 150), (2134, 1200));
        assert_eq!(buffer_size((427, 240), 180), (641, 360));
        assert_eq!(buffer_size((0, 0), 240), (1, 1));
    }

    #[test]
    fn without_a_viewport_only_integer_scales_apply() {
        assert_eq!(effective(Some(180), 2, true), 180);
        assert_eq!(effective(Some(180), 2, false), 240);
        assert_eq!(effective(None, 3, true), 360);
        assert_eq!(effective(None, 0, false), 120);
        assert_eq!(effective(Some(0), 1, true), 120);
    }
}
