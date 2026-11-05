use super::Frame;
use super::players::{Player, Status, chosen};
use super::track::Track;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use wprs_scene::MediaState;
use zbus::{OwnedVariant, Variant};

fn owned(v: Variant<'_>) -> OwnedVariant {
    v.try_to_owned().unwrap()
}

fn metadata(title: &str, artists: &[&str], length: i64) -> OwnedVariant {
    let mut data: HashMap<String, OwnedVariant> = HashMap::new();
    data.insert("xesam:title".into(), owned(Variant::from(title)));
    data.insert("xesam:artist".into(), owned(Variant::from(artists.to_vec())));
    data.insert("xesam:album".into(), owned(Variant::from("Example Album")));
    data.insert("xesam:genre".into(), owned(Variant::from(vec!["Ambient", "Drone"])));
    data.insert("mpris:length".into(), owned(Variant::from(length)));
    data.insert("mpris:artUrl".into(), owned(Variant::from("file:///covers/example.png")));
    owned(Variant::from(data))
}

fn props(pairs: Vec<(&str, OwnedVariant)>) -> HashMap<String, OwnedVariant> {
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

#[test]
fn metadata_becomes_track_fields_joined_and_capped() {
    let data = super::track::dict(&metadata("Night Drive", &["First", "Second"], 180_000_000));
    let track = Track::read(&data);
    let mut state = MediaState::default();
    track.fill(&mut state);
    assert_eq!((state.title.as_str(), state.artist.as_str()), ("Night Drive", "First, Second"));
    assert_eq!(
        (state.album.as_str(), state.genres.as_str(), state.kind.as_str()),
        ("Example Album", "Ambient, Drone", "music")
    );
    assert_eq!((state.subtitle.as_str(), state.duration), ("", 180.0));
    assert_eq!(track.art, "file:///covers/example.png");
    let long = "x".repeat(5000);
    let data = super::track::dict(&metadata(&long, &[], -5));
    let track = Track::read(&data);
    assert_eq!((track.title.chars().count(), track.length), (512, 0.0));
    assert!(!Track::default().known());
}

#[test]
fn the_playing_player_is_chosen_and_positions_follow_rate_and_seeks() {
    let start = Instant::now();
    let mut players = HashMap::new();
    let mut paused = Player::new(start);
    paused.apply(
        &props(vec![
            ("PlaybackStatus", owned(Variant::from("Paused"))),
            ("Metadata", metadata("A", &["B"], 60_000_000)),
        ]),
        start,
    );
    players.insert(":1.5".to_owned(), paused);
    assert_eq!(chosen(&players).unwrap().0, ":1.5");
    let mut playing = Player::new(start);
    let later = start + Duration::from_secs(1);
    playing.apply(
        &props(vec![
            ("PlaybackStatus", owned(Variant::from("Playing"))),
            ("Metadata", metadata("C", &["D"], 60_000_000)),
        ]),
        later,
    );
    playing.seek(10_000_000.0, later);
    players.insert(":1.9".to_owned(), playing);
    let (name, p) = chosen(&players).unwrap();
    assert_eq!((name.as_str(), p.status), (":1.9", Status::Playing));
    assert!((p.now(later + Duration::from_secs(5)) - 15.0).abs() < 1e-6);
    let mut p = p.clone();
    p.apply(&props(vec![("Rate", owned(Variant::from(2.0)))]), later + Duration::from_secs(5));
    assert!((p.now(later + Duration::from_secs(7)) - 19.0).abs() < 1e-6);
    assert_eq!(p.now(later + Duration::from_secs(600)), 60.0);
    let changed = p.apply(
        &props(vec![("PlaybackStatus", owned(Variant::from("Paused")))]),
        later + Duration::from_secs(8),
    );
    assert!(changed && p.now(later + Duration::from_secs(100)) == 21.0);
}

#[test]
fn frames_extrapolate_only_while_playing() {
    let state = MediaState {
        enabled: true,
        track: 1,
        playing: true,
        position: 3.0,
        duration: 4.0,
        rate: 1.0,
        ..Default::default()
    };
    let frame = Frame::new(state.clone());
    let read = frame.read;
    assert!((frame.at(read + Duration::from_millis(500)).position - 3.5).abs() < 1e-4);
    assert_eq!(frame.at(read + Duration::from_secs(9)).position, 4.0);
    let paused = Frame::new(MediaState { playing: false, ..state });
    assert_eq!(paused.at(read + Duration::from_secs(9)).position, 3.0);
    assert!(Frame::new(MediaState::default()).serial > frame.serial);
}
