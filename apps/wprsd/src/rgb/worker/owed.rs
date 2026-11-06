use super::restored::{server, told};
use super::tests::session;
use super::*;

fn reopened(disk: &std::path::Path, address: &str) -> Session {
    let mut s = session("TEST-AGAIN");
    let saved = disk::load(disk);
    (s.owed, s.disk, s.last) = (!saved.is_empty(), Some(disk.to_owned()), None);
    lock(&s.status).saved = saved;
    s.rgb = Rgb { enabled: true, server: address.into(), ..Rgb::default() };
    s
}

#[test]
fn own_modes_outlive_a_killed_daemon_and_come_back_when_the_server_answers() {
    let dir = tempfile::tempdir().unwrap();
    let disk = dir.path().join("rgb-modes.json");
    let (address, heard) = server();
    let mut s = session("TEST-KILLED");
    (s.rgb, s.disk) = (Rgb { enabled: true, server: address.clone(), ..Rgb::default() }, Some(disk.clone()));
    s.paint(&Strip::new(1, 1, vec![0, 255, 0]).unwrap(), None);
    assert_eq!(told(&heard).first(), Some(&(1100, -1)));
    let kept = disk::load(&disk);
    assert_eq!(
        kept.iter().map(|d| d.mode.as_ref().map(|m| m.name.clone())).collect::<Vec<_>>(),
        [Some("Breathing".into())]
    );
    drop(s);
    let mut again = reopened(&disk, &address);
    let strip = Strip::new(1, 1, vec![0, 0, 255]).unwrap();
    again.last = Some(("TEST-AGAIN".into(), strip.clone()));
    again.paint(&strip, None);
    assert_eq!(told(&heard).first(), Some(&(1100, -1)), "the device was not taken again");
    assert_eq!(
        disk::load(&disk)[0].mode.as_ref().map(|m| m.name.as_str()),
        Some("Breathing"),
        "the direct mode was kept as the own mode"
    );
    drop(again);
    let mut idle = reopened(&disk, &address);
    idle.ready().unwrap();
    assert_eq!(told(&heard), [(1101, 0)], "the own mode left by a killed daemon was not given back");
    assert!(!disk.exists() && !idle.owed, "given back modes stayed on disk");
}

#[test]
fn a_mode_that_cannot_be_given_back_is_tried_again_on_the_next_connection() {
    let dir = tempfile::tempdir().unwrap();
    let disk = dir.path().join("rgb-modes.json");
    let (address, heard) = server();
    let mut s = session("TEST-LOST");
    (s.rgb, s.disk) = (Rgb { enabled: true, server: address, ..Rgb::default() }, Some(disk.clone()));
    s.paint(&Strip::new(1, 1, vec![0, 255, 0]).unwrap(), None);
    told(&heard);
    (s.client, s.last) = (None, None);
    s.restore();
    assert!(s.owed && disk.exists(), "a mode not given back was forgotten");
    s.ready().unwrap();
    assert_eq!(told(&heard), [(1101, 0)]);
    assert!(!s.owed && !disk.exists());
}

#[test]
fn broken_or_oversized_kept_modes_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let disk = dir.path().join("rgb-modes.json");
    std::fs::write(&disk, b"not json").unwrap();
    assert!(disk::load(&disk).is_empty());
    let row = |index: u32, key: &str, data: usize| serde_json::json!({ "index": index, "key": key, "name": "n", "leds": 1, "mode": [0, "m", vec![0u8; data], false], "colors": [] });
    let rows = serde_json::json!([
        row(0, "fine", 4),
        row(999, "far", 4),
        row(1, "", 4),
        row(2, "huge", 1 << 20),
        row(3, "bad\u{7}", 4)
    ]);
    std::fs::write(&disk, rows.to_string()).unwrap();
    let kept = disk::load(&disk);
    assert_eq!(kept.iter().map(|d| d.key.as_str()).collect::<Vec<_>>(), ["fine"]);
    assert!(disk::load(&dir.path().join("missing.json")).is_empty());
}
