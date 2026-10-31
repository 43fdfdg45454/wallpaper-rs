use super::*;

#[test]
fn only_repeated_quick_loops_without_frames_count_as_an_empty_part() {
    let now = Instant::now();
    assert_eq!(spins(None, 1), 0);
    assert_eq!(spins(Some((1, now, 0)), 1), 1);
    assert_eq!(spins(Some((1, now, SPINS - 1)), 1), SPINS);
    assert_eq!(spins(Some((1, now, 2)), 2), 0);
    assert_eq!(spins(Some((1, now - SPIN * 3, 2)), 1), 0);
}

#[test]
fn a_stuck_pause_ends_on_the_exact_frame_after_its_key_frame_seek() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/magenta-64x32.webm");
    let uri = crate::init().and_then(|()| crate::local_uri(std::path::Path::new(fixture)));
    let (playbin, sink) = (crate::make("playbin"), crate::make("fakesink"));
    let (Ok(uri), Ok(playbin), Ok(sink)) = (uri, playbin, sink) else {
        return assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "video playback required");
    };
    playbin.set_property("uri", uri);
    playbin.set_property_from_str("flags", "video");
    playbin.set_property("video-sink", sink);
    let bus = playbin.bus().expect("playbin has a bus");
    let settled = || {
        let kinds = [gst::MessageType::AsyncDone, gst::MessageType::Error];
        let message = bus.timed_pop_filtered(gst::ClockTime::from_seconds(10), &kinds);
        message.is_some_and(|m| matches!(m.view(), gst::MessageView::AsyncDone(_)))
    };
    playbin.set_state(gst::State::Paused).unwrap();
    assert!(settled(), "the video did not open");
    let (mut audio, at) = (Audio::default(), gst::ClockTime::from_mseconds(500));
    audio.unstick(&playbin, at);
    assert!(settled() && audio.holding(), "the key frame seek did not settle");
    let early = playbin.query_position::<gst::ClockTime>();
    audio.settled(&playbin);
    assert!(settled() && !audio.holding(), "the exact seek did not settle");
    audio.settled(&playbin);
    let exact = playbin.query_position::<gst::ClockTime>();
    playbin.set_state(gst::State::Null).unwrap();
    assert_eq!((early, exact), (Some(gst::ClockTime::ZERO), Some(at)), "only key frame 0 is before it");
    assert_eq!(audio.shared.seeks.load(Ordering::Relaxed), 2);
}
