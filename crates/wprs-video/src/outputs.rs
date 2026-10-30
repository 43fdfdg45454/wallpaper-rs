use crate::lock;
use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

const PROVIDERS: [&str; 2] = ["pipewiredeviceprovider", "pulsedeviceprovider"];
const APPEARED: &str = "wprs-output";

static HOOKED: Mutex<Vec<gst::Bus>> = Mutex::new(Vec::new());
static WAITERS: Mutex<Vec<(Weak<AtomicBool>, gst::Bus)>> = Mutex::new(Vec::new());

pub(crate) struct Outputs {
    provider: Option<gst::DeviceProvider>,
    at: Instant,
    seen: Arc<AtomicBool>,
}

fn speaker(device: &gst::Device) -> bool {
    device.has_classes("Audio/Sink")
}

fn hook(source: &gst::Bus) {
    let mut hooked = lock(&HOOKED);
    if hooked.contains(source) {
        return;
    }
    source.set_sync_handler(|_, message| {
        if let gst::MessageView::DeviceAdded(added) = message.view()
            && speaker(&added.device())
        {
            wake();
        }
        gst::BusSyncReply::Drop
    });
    hooked.push(source.clone());
}

fn wake() {
    let waiters = lock(&WAITERS)
        .iter()
        .filter_map(|(seen, bus)| Some((seen.upgrade()?, bus.clone())))
        .collect::<Vec<_>>();
    for (seen, bus) in waiters {
        seen.store(true, Ordering::Relaxed);
        let _ = bus.post(gst::message::Application::new(gst::Structure::new_empty(APPEARED)));
    }
}

impl Outputs {
    fn enlist(bus: &gst::Bus, at: Instant) -> Outputs {
        let seen = Arc::new(AtomicBool::new(false));
        lock(&WAITERS).push((Arc::downgrade(&seen), bus.clone()));
        Outputs { provider: None, at, seen }
    }

    pub(crate) fn watch(bus: &gst::Bus, at: Instant) -> Outputs {
        let mut outputs = Self::enlist(bus, at);
        let mut found = PROVIDERS.iter().filter_map(|name| gst::DeviceProviderFactory::by_name(name));
        outputs.provider = found.find(|p| {
            hook(&p.bus());
            p.start().is_ok()
        });
        if outputs.provider.as_ref().is_some_and(|p| p.devices().iter().any(speaker)) {
            outputs.seen.store(true, Ordering::Relaxed);
        }
        outputs
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
        }
        lock(&WAITERS).retain(|(seen, _)| seen.as_ptr() != Arc::as_ptr(&self.seen));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gst::glib;
    use gst::subclass::prelude::*;

    mod imp {
        use super::*;

        #[derive(Default)]
        pub struct Speaker;

        #[glib::object_subclass]
        impl ObjectSubclass for Speaker {
            const NAME: &'static str = "WprsTestSpeaker";
            type Type = super::Speaker;
            type ParentType = gst::Device;
        }

        impl ObjectImpl for Speaker {}
        impl GstObjectImpl for Speaker {}
        impl DeviceImpl for Speaker {}
    }

    glib::wrapper! {
        pub struct Speaker(ObjectSubclass<imp::Speaker>) @extends gst::Device, gst::Object;
    }

    #[test]
    fn every_waiting_player_hears_a_new_output_and_dropping_one_keeps_the_rest() {
        crate::init().expect("GStreamer starts");
        let (source, first_bus, second_bus) = (gst::Bus::new(), gst::Bus::new(), gst::Bus::new());
        let speaker = glib::Object::builder::<Speaker>()
            .property("device-class", "Audio/Sink")
            .build()
            .upcast::<gst::Device>();
        let added =
            || source.post(gst::message::DeviceAdded::new(&speaker)).expect("the source bus takes it");
        let heard = |bus: &gst::Bus| {
            std::iter::from_fn(|| bus.pop()).any(|m| m.structure().is_some_and(|s| s.name() == APPEARED))
        };
        hook(&source);
        let (first, second) =
            (Outputs::enlist(&first_bus, Instant::now()), Outputs::enlist(&second_bus, Instant::now()));
        hook(&source);
        added();
        assert!(first.seen.load(Ordering::Relaxed) && second.seen.load(Ordering::Relaxed));
        assert!(heard(&first_bus) && heard(&second_bus), "both players are told");
        drop(first);
        second.seen.store(false, Ordering::Relaxed);
        added();
        assert!(second.seen.load(Ordering::Relaxed) && heard(&second_bus), "the other player still hears it");
        assert!(!heard(&first_bus), "a dropped player is no longer told");
    }
}
