use crate::state::turned;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput;
use smithay_client_toolkit::reexports::client::{Connection, EventQueue, QueueHandle};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::{delegate_dispatch2, delegate_registry, registry_handlers};
use std::sync::Arc;
use std::time::Duration;

const RETRY: Duration = Duration::from_secs(1);

type Mode = Option<((u32, u32), u32)>;
type Report = Arc<dyn Fn(String, Mode) + Send + Sync>;

struct Modes {
    registry: RegistryState,
    outputs: OutputState,
    names: Vec<(WlOutput, String)>,
    report: Report,
}

fn open(report: &Report) -> anyhow::Result<(EventQueue<Modes>, Modes)> {
    let conn = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<Modes>(&conn)?;
    let (registry, outputs) = (RegistryState::new(&globals), OutputState::new(&globals, &queue.handle()));
    Ok((queue, Modes { registry, outputs, names: Vec::new(), report: report.clone() }))
}

pub fn watch_modes(report: impl Fn(String, Mode) + Send + Sync + 'static) -> anyhow::Result<()> {
    let report: Report = Arc::new(report);
    let mut opened = Some(open(&report)?);
    let follow = move || {
        let mut wait = RETRY;
        loop {
            if let Some((mut queue, mut modes)) = opened.take() {
                wait = RETRY;
                while queue.blocking_dispatch(&mut modes).is_ok() {}
                tracing::debug!("lost the display server, the screen modes are unknown until it is back");
                modes.names.drain(..).for_each(|(_, name)| report(name, None));
            }
            std::thread::sleep(wait);
            (wait, opened) = ((wait * 2).min(RETRY * 16), open(&report).ok());
        }
    };
    std::thread::Builder::new().name("wprs-modes".into()).spawn(follow)?;
    Ok(())
}

impl Modes {
    fn seen(&mut self, output: WlOutput) {
        let Some(info) = self.outputs.info(&output) else { return };
        let (Some(name), Some(mode)) = (info.name.clone(), info.modes.iter().find(|m| m.current)) else {
            return;
        };
        let size = Some(turned(&info, mode.dimensions)).filter(|(w, h)| *w > 0 && *h > 0);
        let hz = (mode.refresh_rate.max(0) as u32).saturating_add(500) / 1000;
        self.names.retain(|(o, _)| *o != output);
        self.names.push((output, name.clone()));
        (self.report)(name, size.map(|size| (size, hz)));
    }
}

impl OutputHandler for Modes {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        self.seen(output)
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        self.seen(output)
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        let gone: Vec<_> = self.names.extract_if(.., |(o, _)| *o == output).collect();
        gone.into_iter().for_each(|(_, name)| (self.report)(name, None));
    }
}

impl ProvidesRegistryState for Modes {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }

    registry_handlers![OutputState];
}

delegate_registry!(Modes);
delegate_dispatch2!(Modes);
