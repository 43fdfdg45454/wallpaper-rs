use crate::state::State;
use crate::{Cover, Event};
use handle::{State as Mode, ZwlrForeignToplevelHandleV1 as Handle};
use manager::ZwlrForeignToplevelManagerV1 as Manager;
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::reexports::client::{
    Connection, Dispatch, Proxy, QueueHandle, event_created_child,
};
use smithay_client_toolkit::reexports::protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1 as handle, zwlr_foreign_toplevel_manager_v1 as manager,
};

#[derive(Default)]
pub(crate) struct Toplevel {
    outputs: Vec<WlOutput>,
    covering: bool,
}

pub(crate) fn bind(globals: &GlobalList, qh: &QueueHandle<State>) {
    let bound = globals.bind::<Manager, _, _>(qh, 1..=3, ());
    if let Err(e) = bound {
        tracing::debug!("no foreign toplevel information: {e}");
    }
}

fn covering(states: &[u8]) -> bool {
    let has = |mode: Mode| states.as_chunks().0.iter().any(|c| u32::from_ne_bytes(*c) == mode as u32);
    (has(Mode::Fullscreen) || has(Mode::Maximized)) && has(Mode::Activated) && !has(Mode::Minimized)
}

impl State {
    fn cover_outputs(&mut self) {
        for screen in &mut self.screens {
            let covered = self.toplevels.values().any(|t| t.covering && t.outputs.contains(&screen.output));
            if std::mem::replace(&mut screen.toplevel, covered) != covered {
                (self.sink)(Event::Covered(screen.name.clone(), Cover::Toplevel, covered));
            }
        }
    }
}

impl Dispatch<Manager, ()> for State {
    fn event(
        state: &mut Self,
        _: &Manager,
        event: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let manager::Event::Toplevel { toplevel } = event {
            state.toplevels.insert(toplevel.id(), Toplevel::default());
        }
    }

    event_created_child!(State, Manager, [manager::EVT_TOPLEVEL_OPCODE => (Handle, ())]);
}

impl Dispatch<Handle, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &Handle,
        event: handle::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let toplevel = state.toplevels.get_mut(&proxy.id());
        match (event, toplevel) {
            (handle::Event::OutputEnter { output }, Some(t)) => t.outputs.push(output),
            (handle::Event::OutputLeave { output }, Some(t)) => t.outputs.retain(|o| *o != output),
            (handle::Event::State { state: modes }, Some(t)) => t.covering = covering(&modes),
            (handle::Event::Done, _) => state.cover_outputs(),
            (handle::Event::Closed, _) => {
                proxy.destroy();
                state.toplevels.remove(&proxy.id());
                state.cover_outputs();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modes(list: &[Mode]) -> Vec<u8> {
        list.iter().flat_map(|m| (*m as u32).to_ne_bytes()).collect()
    }

    #[test]
    fn only_focused_fullscreen_or_maximized_windows_cover_an_output() {
        assert!(covering(&modes(&[Mode::Activated, Mode::Fullscreen])));
        assert!(covering(&modes(&[Mode::Maximized, Mode::Activated])));
        assert!(!covering(&modes(&[Mode::Fullscreen])));
        assert!(!covering(&modes(&[Mode::Activated, Mode::Maximized, Mode::Minimized])));
        assert!(!covering(&modes(&[Mode::Activated])));
        assert!(!covering(&[]));
    }
}
