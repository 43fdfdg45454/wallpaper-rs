use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};
use wprs_video::{Cut, Event, Options, Player};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/magenta-64x32.webm");

fn play(cut: Cut) -> Option<(Player, Receiver<Event>)> {
    let (tx, rx) = mpsc::channel();
    let options =
        Options { max_size: (64, 32), max_fps: 60, volume: 0.0, muted: true, cut, zero_copy: false };
    let player = Player::open(Path::new(FIXTURE), options, move |event| drop(tx.send(event)));
    if let Err(why) = &player {
        assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "video playback required: {why}");
    }
    let player = player.ok()?;
    player.play_once(true);
    Some((player, rx))
}

fn last_pts(rx: &Receiver<Event>, span: Duration) -> (usize, Option<Duration>) {
    let (start, mut seen, mut last) = (Instant::now(), 0, None);
    while start.elapsed() < span {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::Frame(f)) => (seen, last) = (seen + 1, f.pts.or(last)),
            Ok(Event::Error(e)) => panic!("playback error: {e}"),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    (seen, last)
}

#[test]
fn a_video_that_plays_once_stops_on_its_last_frame() {
    for cut in [Cut::default(), Cut { speed: 100, start: 200, end: 600 }] {
        let Some((_player, rx)) = play(cut) else { return };
        let (played, end) = last_pts(&rx, Duration::from_secs(4));
        assert!(played > 3, "only {played} frames before the end");
        let (after, _) = last_pts(&rx, Duration::from_secs(2));
        assert_eq!(after, 0, "{after} frames after the end of {cut:?}, last at {end:?}");
    }
}
