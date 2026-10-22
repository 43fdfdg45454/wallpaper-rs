use gstreamer as gst;
use gstreamer::prelude::*;

pub(crate) const SINK: &str = "wprs-audio";
const FIXED_POOL: (u32, u32) = (1, 2);

pub(crate) fn sink() -> Option<gst::Element> {
    let factory = gst::ElementFactory::find("pipewiresink")?;
    let version = factory.plugin().map(|p| p.version()).unwrap_or_default();
    if !fixed_pool(&version) {
        tracing::info!(%version, "the PipeWire GStreamer plugin is older than 1.2, using the default audio output");
        return None;
    }
    let props = gst::Structure::builder("props").field("media.role", "Movie").build();
    let sink = factory.create().name(SINK).build().ok()?;
    sink.set_property("client-name", "wallpaper-rs");
    sink.set_property("stream-properties", props);
    Some(sink)
}

fn fixed_pool(version: &str) -> bool {
    let mut parts = version.split('.').map(|part| part.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0)) >= FIXED_POOL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plugins_from_1_2_send_the_sound_to_pipewire() {
        let versions = ["0.3.80", "1.0.5", "1.1.82", "", "1.2.0", "1.4.7", "2.0"];
        let fixed: Vec<_> = versions.iter().map(|v| fixed_pool(v)).collect();
        assert_eq!(fixed, [false, false, false, false, true, true, true]);
    }
}
