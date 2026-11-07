use smithay_client_toolkit::dmabuf::{DmabufFeedback, DmabufHandler, DmabufState};
use smithay_client_toolkit::reexports::client::globals::registry_queue_init;
use smithay_client_toolkit::reexports::client::protocol::wl_buffer::WlBuffer;
use smithay_client_toolkit::reexports::client::{Connection, QueueHandle};
use smithay_client_toolkit::reexports::protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
    zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1,
};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::{delegate_dispatch2, delegate_registry, registry_handlers};
use std::sync::mpsc::channel;
use std::time::Duration;

const WAIT: Duration = Duration::from_secs(2);

struct Feedback {
    registry: RegistryState,
    dmabuf: DmabufState,
    device: Option<u64>,
}

fn ask() -> anyhow::Result<Option<u64>> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<Feedback>(&conn)?;
    let qh = queue.handle();
    let (registry, dmabuf) = (RegistryState::new(&globals), DmabufState::new(&globals, &qh));
    let mut state = Feedback { registry, dmabuf, device: None };
    if state.dmabuf.get_default_feedback(&qh).is_err() {
        return Ok(None);
    }
    while state.device.is_none() {
        queue.blocking_dispatch(&mut state)?;
    }
    Ok(state.device)
}

pub fn compositor_device() -> Option<u64> {
    let (send, receive) = channel();
    std::thread::Builder::new().name("wprs-dmabuf".into()).spawn(move || drop(send.send(ask()))).ok()?;
    match receive.recv_timeout(WAIT) {
        Ok(Ok(device)) => device,
        Ok(Err(e)) => {
            tracing::debug!("no dmabuf feedback from the compositor: {e:#}");
            None
        }
        Err(_) => None,
    }
}

impl DmabufHandler for Feedback {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf
    }

    fn dmabuf_feedback(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &ZwpLinuxDmabufFeedbackV1,
        feedback: DmabufFeedback,
    ) {
        self.device = Some(feedback.main_device());
    }

    fn created(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ZwpLinuxBufferParamsV1, _: WlBuffer) {}

    fn failed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &ZwpLinuxBufferParamsV1) {}

    fn released(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlBuffer) {}
}

impl ProvidesRegistryState for Feedback {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }

    registry_handlers![];
}

delegate_registry!(Feedback);
delegate_dispatch2!(Feedback);
