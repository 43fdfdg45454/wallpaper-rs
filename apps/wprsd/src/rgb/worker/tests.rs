use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver as Heard, channel};
use wprs_core::rgb::Covered;

static SERIAL: Mutex<()> = Mutex::new(());

fn server(during: impl FnOnce() + Send + 'static) -> (String, Heard<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let (closed, heard) = channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut during = Some(during);
        let mut head = [0u8; 16];
        while stream.read_exact(&mut head).is_ok() {
            let word = |at: usize| u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]]);
            let mut data = vec![0; word(12) as usize];
            stream.read_exact(&mut data).ok();
            during.take().into_iter().for_each(|f| f());
            if matches!(word(8), 0 | 40) {
                stream.write_all(&[&wprs_rgb::header(0, word(8), 4)[..], &[0; 4]].concat()).ok();
            }
        }
        closed.send(()).ok();
    });
    (address, heard)
}

fn follower(source: &str, address: String) -> Session {
    let mut s = session(source);
    (s.last, s.rgb) =
        (None, Rgb { enabled: true, server: address, when_covered: Covered::Follow, ..Rgb::default() });
    s
}

fn released(s: &Session, heard: &Heard<()>) -> bool {
    let closed = heard.recv_timeout(Duration::from_secs(5)).is_ok();
    closed && s.client.is_none() && s.wait() == IDLE && !lock(&s.status).connected
}

pub(super) fn session(source: &str) -> Session {
    let status = Arc::new(Mutex::new(Status { source: Some(source.into()), ..Status::default() }));
    let strip = Strip::new(1, 1, vec![0, 255, 0]).unwrap();
    let last = Some((source.to_owned(), strip));
    Session { status, last, ..Session::default() }
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
    let _serial = lock(&SERIAL);
    let device = wprs_rgb::Device { name: "keys".into(), kind: "keyboard", leds: 1, ..Default::default() };
    let empty = Status { connected: true, ..Status::default() };
    assert!(!empty.receiving(), "a server without devices counted as receiving");
    assert!(Status { connected: true, devices: vec![device], ..Status::default() }.receiving());
    let mut s = session("TEST-RETRY");
    s.rgb =
        Rgb { enabled: true, server: "127.0.0.1:1".into(), when_covered: Covered::Follow, ..Rgb::default() };
    super::super::cover("TEST-RETRY", true);
    s.retry();
    assert_eq!(s.wait(), IDLE, "a covered screen that sends no lights kept retrying the server");
    assert!(s.failed.is_none(), "a covered screen that sends no lights connected to the server");
    super::super::want("TEST-RETRY", true);
    assert_eq!(s.wait(), RETRY, "covered following lights never tried the server again");
    s.retry();
    assert!(s.failed.is_some() && !lock(&s.status).receiving(), "an unreachable server counted as receiving");
    s.rgb.enabled = false;
    assert_eq!(s.wait(), IDLE, "lights turned off kept retrying");
    s.rgb.enabled = true;
    super::super::want("TEST-RETRY", false);
    assert_eq!(s.wait(), IDLE, "lights kept retrying once the covered scene stopped wanting them");
    super::super::cover("TEST-RETRY", false);
}

#[test]
fn a_server_that_finds_its_first_device_wakes_the_covered_scene() {
    let device = wprs_rgb::Device { name: "keys".into(), kind: "keyboard", leds: 1, ..Default::default() };
    let mut status = Status { connected: true, ..Status::default() };
    assert!(status.show(true, vec![device.clone()], None), "a new device list never woke the scene");
    assert!(!status.show(true, vec![device.clone()], None), "the same devices woke the scene again");
    assert!(status.show(false, vec![device], None) && status.devices.is_empty());
    assert!(!status.show(true, Vec::new(), None), "a server without devices woke the scene");
}

#[test]
fn covered_following_lights_let_the_devices_go_once_nothing_wants_them() {
    let _serial = lock(&SERIAL);
    let (address, heard) = server(|| {});
    let mut s = follower("TEST-LET-GO", address);
    super::super::cover("TEST-LET-GO", true);
    super::super::want("TEST-LET-GO", true);
    s.retry();
    assert!(s.client.is_some() && s.wait() == LISTEN, "covered following lights never connected");
    super::super::want("TEST-LET-GO", false);
    s.retry();
    super::super::cover("TEST-LET-GO", false);
    assert!(released(&s, &heard), "a locked, paused or battery session kept the devices and kept listening");
}

#[test]
fn a_slow_server_is_let_go_when_the_covered_scene_stopped_wanting_lights_meanwhile() {
    let _serial = lock(&SERIAL);
    let (address, heard) = server(|| super::super::want("TEST-SLOW", false));
    let mut s = follower("TEST-SLOW", address);
    super::super::cover("TEST-SLOW", true);
    super::super::want("TEST-SLOW", true);
    s.retry();
    super::super::cover("TEST-SLOW", false);
    assert!(
        released(&s, &heard),
        "a connection that finished after the lights were unwanted kept the devices"
    );
}

#[test]
fn lights_an_uncovered_screen_paints_stay_connected_when_covered_ones_stop_wanting_them() {
    let _serial = lock(&SERIAL);
    let (address, _heard) = server(|| {});
    let mut s = follower("TEST-KEEP", address);
    super::super::cover("TEST-KEEP", true);
    super::super::want("TEST-KEEP", true);
    s.retry();
    s.last = Some(("TEST-SHOWN".into(), Strip::new(1, 1, vec![0, 0, 255]).unwrap()));
    super::super::want("TEST-KEEP", false);
    s.retry();
    assert!(s.unwanted(Some("TEST-KEEP")) && !s.unwanted(Some("TEST-SHOWN")));
    super::super::cover("TEST-KEEP", false);
    assert!(s.client.is_some() && s.wait() == LISTEN, "lights an uncovered screen paints were let go");
}
