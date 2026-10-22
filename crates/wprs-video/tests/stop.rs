use std::path::Path;
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};
use wprs_video::{Options, Player};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/magenta-64x32.webm");

#[test]
fn dropping_a_stuck_player_returns_in_time() {
    let (tx, rx) = mpsc::channel();
    let (release, stuck) = mpsc::channel::<()>();
    let stuck = Mutex::new(stuck);
    let options = Options { max_size: (64, 32), max_fps: 60, volume: 0.0, muted: true };
    let opened = Player::open(Path::new(FIXTURE), options, move |_| {
        let _ = tx.send(());
        let _ = stuck.lock().map(|stuck| stuck.recv());
    });
    let Some(player) = opened.ok().filter(|_| rx.recv_timeout(Duration::from_secs(10)).is_ok()) else {
        assert!(std::env::var_os("WPRS_REQUIRE_VIDEO").is_none(), "video playback required");
        return;
    };
    let start = Instant::now();
    drop(player);
    let took = start.elapsed();
    drop(release);
    assert!(took < Duration::from_secs(5), "dropping a stuck player took {took:?}");
}
