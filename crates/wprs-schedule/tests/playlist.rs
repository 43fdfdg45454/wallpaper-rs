use chrono::{DateTime, TimeDelta, Utc};
use std::path::{Path, PathBuf};
use std::time::Duration;
use wprs_schedule::{Order, Playlist, PlaylistState, Positions};

fn playlist(len: usize, order: Order) -> Playlist {
    let items = (0..len).map(|i| PathBuf::from(format!("/w/{i}.png"))).collect();
    Playlist { id: "p".into(), items, order, interval: Duration::from_secs(600), per_output: false }
}

fn t0() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2024-06-21T12:00:00Z").unwrap().to_utc()
}

fn sequence(state: &mut PlaylistState, p: &Playlist, steps: usize) -> Vec<usize> {
    let mut out = vec![state.index(p).unwrap()];
    out.extend((1..steps).map(|_| {
        state.advance(p, t0());
        state.index(p).unwrap()
    }));
    out
}

#[test]
fn sequential_wraps_around() {
    let p = playlist(3, Order::Sequential);
    assert_eq!(sequence(&mut PlaylistState::new(7), &p, 7), [0, 1, 2, 0, 1, 2, 0]);
}

#[test]
fn shuffle_is_reproducible() {
    let p = playlist(10, Order::Shuffle);
    let a = sequence(&mut PlaylistState::new(42), &p, 60);
    assert_eq!(a, sequence(&mut PlaylistState::new(42), &p, 60));
    assert_ne!(a, sequence(&mut PlaylistState::new(43), &p, 60));
    assert_ne!(a[..10], a[10..20]);
}

#[test]
fn shuffle_cycles_are_permutations_without_immediate_repeats() {
    for len in 2..=9 {
        let p = playlist(len, Order::Shuffle);
        for seed in 0..50 {
            let seq = sequence(&mut PlaylistState::new(seed), &p, len * 20);
            assert!(seq.windows(2).all(|w| w[0] != w[1]), "len {len} seed {seed}: {seq:?}");
            for cycle in seq.chunks(len) {
                let mut sorted = cycle.to_vec();
                sorted.sort_unstable();
                assert_eq!(sorted, (0..len).collect::<Vec<_>>());
            }
        }
    }
}

#[test]
fn poll_switches_after_the_interval() {
    let p = playlist(3, Order::Sequential);
    let mut state = PlaylistState::new(1);
    assert_eq!(state.next_switch(&p), None);
    assert_eq!(state.poll(&p, t0()), Some(Path::new("/w/0.png")));
    assert_eq!(state.next_switch(&p), Some(t0() + TimeDelta::minutes(10)));
    assert_eq!(state.poll(&p, t0() + TimeDelta::minutes(9)), Some(Path::new("/w/0.png")));
    assert_eq!(state.poll(&p, t0() + TimeDelta::minutes(10)), Some(Path::new("/w/1.png")));
    assert_eq!(state.next_switch(&p), Some(t0() + TimeDelta::minutes(20)));
    let still = Playlist { interval: Duration::ZERO, ..p.clone() };
    assert_eq!(state.poll(&still, t0() + TimeDelta::days(1)), Some(Path::new("/w/1.png")));
}

#[test]
fn per_output_playlists_keep_separate_positions() {
    let mut shared = playlist(5, Order::Shuffle);
    let mut positions = Positions::default();
    positions.state(&shared, "DP-1", 9).advance(&shared, t0());
    assert_eq!(
        positions.state(&shared, "HDMI-A-1", 9).index(&shared),
        positions.state(&shared, "DP-1", 9).index(&shared)
    );
    shared.per_output = true;
    let one = positions.state(&shared, "DP-1", 9).clone();
    let two = positions.state(&shared, "HDMI-A-1", 9).clone();
    assert_ne!(one, two);
    assert_eq!(positions.0.keys().collect::<Vec<_>>(), ["p", "p/DP-1", "p/HDMI-A-1"]);
}

#[test]
fn persisted_state_continues_the_same_sequence() {
    let p = playlist(6, Order::Shuffle);
    let mut state = PlaylistState::new(5);
    sequence(&mut state, &p, 9);
    let mut from_toml: PlaylistState = toml::from_str(&toml::to_string(&state).unwrap()).unwrap();
    let mut from_json: PlaylistState = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    let expected = sequence(&mut state, &p, 20);
    assert_eq!(sequence(&mut from_toml, &p, 20), expected);
    assert_eq!(sequence(&mut from_json, &p, 20), expected);
}
