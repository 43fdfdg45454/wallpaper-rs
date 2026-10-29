use crate::state::State;
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;
use smithay_client_toolkit::reexports::client::{Connection, Dispatch, QueueHandle, delegate_noop};
use smithay_client_toolkit::reexports::protocols::wp::presentation_time::client::wp_presentation::WpPresentation;
use smithay_client_toolkit::reexports::protocols::wp::presentation_time::client::wp_presentation_feedback::{
    Event, WpPresentationFeedback,
};
use smithay_client_toolkit::shell::WaylandSurface;

pub(crate) fn bind(globals: &GlobalList, qh: &QueueHandle<State>) -> Option<WpPresentation> {
    let bound = globals.bind(qh, 1..=1, ());
    bound.inspect_err(|e| tracing::debug!("no presentation times: {e}")).ok()
}

pub(crate) fn millihertz(refresh_ns: u32) -> Option<u32> {
    let rate = (refresh_ns > 0).then(|| 1_000_000_000_000 / u64::from(refresh_ns));
    rate.map(|r| r.min(u64::from(u32::MAX)) as u32)
}

impl Dispatch<WpPresentationFeedback, WlSurface> for State {
    fn event(
        state: &mut Self,
        _: &WpPresentationFeedback,
        event: Event,
        surface: &WlSurface,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Event::Presented { refresh, .. } = event else { return };
        let Some(rate) = millihertz(refresh) else {
            return tracing::debug!("presented without a refresh rate");
        };
        let on = |s: &crate::state::Screen| s.parts.layer.wl_surface() == surface;
        let Some(screen) = state.screens.iter_mut().find(|s| on(s)) else { return };
        if screen.rate.presented(rate) {
            let hz = rate.saturating_add(500) / 1000;
            tracing::debug!(output = screen.name, hz, "refresh rate presented");
            state.changed(on, |_| {});
        }
    }
}

delegate_noop!(State: ignore WpPresentation);

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Rate {
    mode: u32,
    measured: Option<u32>,
}

impl Rate {
    pub(crate) fn current(&mut self, mode: u32) -> u32 {
        if self.mode != mode {
            *self = Rate { mode, measured: None };
        }
        self.measured.unwrap_or(mode)
    }

    fn presented(&mut self, rate: u32) -> bool {
        let new = self.measured.is_none_or(|m| m.abs_diff(rate) * 200 > rate);
        self.measured = Some(self.measured.filter(|_| !new).unwrap_or(rate));
        new
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn presentation_times_give_the_refresh_rate() {
        assert_eq!(super::millihertz(16_666_667), Some(59_999));
        assert_eq!(super::millihertz(6_944_444), Some(144_000));
        assert_eq!(super::millihertz(0), None);
    }

    #[test]
    fn a_new_mode_drops_the_rate_measured_in_the_old_one() {
        let mut rate = super::Rate::default();
        assert_eq!(rate.current(60_000), 60_000);
        assert!(rate.presented(59_940) && !rate.presented(59_950));
        assert_eq!(rate.current(60_000), 59_940);
        assert_eq!(rate.current(144_000), 144_000, "the rate measured at 60 Hz was kept");
        assert!(rate.presented(143_900));
        assert_eq!(rate.current(144_000), 143_900);
    }
}
