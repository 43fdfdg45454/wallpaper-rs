use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const PROVIDERS: [&str; 2] = ["pipewiredeviceprovider", "pulsedeviceprovider"];
const APPEARED: &str = "wprs-output";

pub(crate) struct Outputs {
    provider: Option<gst::DeviceProvider>,
    at: Instant,
    seen: Arc<AtomicBool>,
}

fn speaker(device: &gst::Device) -> bool {
    device.has_classes("Audio/Sink")
}

fn listen(provider: &gst::DeviceProvider, bus: gst::Bus, seen: Arc<AtomicBool>) {
    provider.bus().set_sync_handler(move |_, message| {
        if let gst::MessageView::DeviceAdded(added) = message.view()
            && speaker(&added.device())
        {
            seen.store(true, Ordering::Relaxed);
            let _ = bus.post(gst::message::Application::new(gst::Structure::new_empty(APPEARED)));
        }
        gst::BusSyncReply::Drop
    });
}

impl Outputs {
    pub(crate) fn watch(bus: &gst::Bus, at: Instant) -> Outputs {
        let seen = Arc::new(AtomicBool::new(false));
        let provider = PROVIDERS.iter().find_map(|name| {
            let provider = gst::DeviceProviderFactory::by_name(name)?;
            listen(&provider, bus.clone(), seen.clone());
            if provider.start().is_ok() {
                return Some(provider);
            }
            provider.bus().unset_sync_handler();
            None
        });
        if provider.as_ref().is_some_and(|p| p.devices().iter().any(speaker)) {
            seen.store(true, Ordering::Relaxed);
        }
        Outputs { provider, at, seen }
    }

    pub(crate) fn wait(&self) -> Option<Instant> {
        (self.provider.is_none() || self.seen.load(Ordering::Relaxed)).then_some(self.at)
    }

    pub(crate) fn ready(&self, now: Instant) -> bool {
        self.wait().is_some_and(|at| at <= now)
    }
}

impl Drop for Outputs {
    fn drop(&mut self) {
        if let Some(provider) = &self.provider {
            provider.stop();
            provider.bus().unset_sync_handler();
        }
    }
}
