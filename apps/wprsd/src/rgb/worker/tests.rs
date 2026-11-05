use super::*;
use wprs_core::rgb::Covered;

fn session(source: &str) -> Session {
    let status = Arc::new(Mutex::new(Status { source: Some(source.into()), ..Status::default() }));
    let strip = Strip::new(1, 1, vec![0, 255, 0]).unwrap();
    let last = Some((source.to_owned(), strip));
    Session { status, rgb: Rgb::default(), client: None, failed: None, last, test: None }
}

#[test]
fn a_forgotten_or_revoked_source_is_never_painted_again() {
    let mut s = session("TEST-RGB");
    s.forget(Some("OTHER"));
    assert!(s.last.is_some(), "another source forgot this one");
    s.forget(Some("TEST-RGB"));
    assert!(s.last.is_none() && lock(&s.status).source.is_none());
    let mut s = session("TEST-RGB");
    s.repaint();
    assert!(s.last.is_none(), "a source without the rgb grant kept its colors");
    let mut s = session("preview:editor");
    s.repaint();
    assert!(s.last.is_none());
    let mut s = session("TEST-RGB");
    s.forget(None);
    assert!(s.last.is_none());
}

#[test]
fn the_lights_idle_only_while_the_screen_they_follow_is_covered_and_they_pause() {
    let mut s = session("TEST-HELD");
    assert!(!s.held() && s.wait() == IDLE);
    super::super::cover("TEST-HELD", true);
    assert!(s.held(), "a covered source kept the lights listening");
    s.rgb.when_covered = Covered::Follow;
    assert!(!s.held(), "following lights idled");
    s.rgb.when_covered = Covered::Pause;
    s.test = Some((Instant::now(), None));
    assert_eq!(s.wait(), STEP, "a test pattern waited for the screen");
    super::super::cover("TEST-HELD", false);
    assert!(!s.held());
}

#[test]
fn covered_lights_that_follow_a_server_without_devices_retry_it_instead_of_drawing_for_nothing() {
    let device =
        wprs_rgb::Device { index: 0, name: "keys".into(), kind: "keyboard", leds: 1, zones: Vec::new() };
    let empty = Status { connected: true, ..Status::default() };
    assert!(!empty.receiving(), "a server without devices counted as receiving");
    assert!(Status { connected: true, devices: vec![device], ..Status::default() }.receiving());
    let mut s = session("TEST-RETRY");
    s.rgb = Rgb { enabled: true, server: "127.0.0.1:1".into(), when_covered: Covered::Follow };
    super::super::cover("TEST-RETRY", true);
    assert_eq!(s.wait(), RETRY, "covered following lights never tried the server again");
    s.retry();
    assert!(s.failed.is_some() && !lock(&s.status).receiving(), "an unreachable server counted as receiving");
    s.rgb.enabled = false;
    assert_eq!(s.wait(), IDLE, "lights turned off kept retrying");
    super::super::cover("TEST-RETRY", false);
}
