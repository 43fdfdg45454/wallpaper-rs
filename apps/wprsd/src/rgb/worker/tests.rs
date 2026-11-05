use super::*;

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
