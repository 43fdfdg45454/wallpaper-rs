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
    state.seek(&p, 4);
    let mut from_toml: PlaylistState = toml::from_str(&toml::to_string(&state).unwrap()).unwrap();
    let mut from_json: PlaylistState = serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    let expected = sequence(&mut state, &p, 20);
    assert_eq!(sequence(&mut from_toml, &p, 20), expected);
    assert_eq!(sequence(&mut from_json, &p, 20), expected);
}

#[test]
fn back_retraces_advances_across_cycles_and_stops_at_the_start() {
    for order in [Order::Sequential, Order::Shuffle] {
        for len in 1..=6 {
            let p = playlist(len, order);
            let mut state = PlaylistState::new(3);
            let forward = sequence(&mut state, &p, len * 3 + 2);
            assert_eq!(
                (state.cycle() as usize, state.position() as usize),
                ((len * 3 + 1) / len, (len * 3 + 1) % len)
            );
            let mut backward: Vec<usize> = (1..forward.len())
                .map(|_| {
                    state.back(&p, t0());
                    state.index(&p).unwrap()
                })
                .collect();
            backward.reverse();
            assert_eq!(backward, forward[..forward.len() - 1], "{order:?} {len}");
            let start = state.clone();
            assert_eq!(state.back(&p, t0() + TimeDelta::hours(1)), Some(p.items[forward[0]].as_path()));
            assert_eq!(state, start);
        }
    }
}

#[test]
fn seeking_keeps_the_item_and_a_shuffle_cycle_starts_from_it_without_repeats() {
    for len in 1..=8 {
        let p = playlist(len, Order::Shuffle);
        for index in 0..len {
            let mut state = PlaylistState::new(11);
            sequence(&mut state, &p, 5);
            let cycle = state.cycle();
            state.seek(&p, index);
            assert_eq!((state.cycle(), state.position(), state.index(&p)), (cycle + 1, 0, Some(index)));
            let seq = sequence(&mut state, &p, len * 3);
            let mut first = seq[..len].to_vec();
            first.sort_unstable();
            assert_eq!(first, (0..len).collect::<Vec<_>>());
            assert!(len == 1 || seq.windows(2).all(|w| w[0] != w[1]), "{len} {index}: {seq:?}");
        }
    }
    let p = playlist(5, Order::Sequential);
    let mut state = PlaylistState::new(11);
    state.seek(&p, 3);
    assert_eq!((state.current(&p), state.position(), state.cycle()), (Some(Path::new("/w/3.png")), 3, 0));
}

#[test]
fn states_saved_without_newer_fields_still_load() {
    let state: PlaylistState = serde_json::from_str(r#"{"seed": 5, "cycle": 2, "position": 1}"#).unwrap();
    assert_eq!((state.cycle(), state.position()), (2, 1));
}

#[test]
fn going_back_in_a_shuffle_replays_the_exact_order_both_ways_across_cycles_and_list_changes() {
    let (p, mut state) = (playlist(5, Order::Shuffle), PlaylistState::new(9));
    let shown = sequence(&mut state, &p, 12);
    let step = |state: &mut PlaylistState, p: &Playlist, forward: bool| {
        if forward {
            state.advance(p, t0())
        } else {
            state.back(p, t0())
        };
        state.index(p).unwrap()
    };
    let back: Vec<usize> = (0..11).map(|_| step(&mut state, &p, false)).collect();
    assert_eq!(back, shown[..11].iter().rev().copied().collect::<Vec<_>>());
    let again: Vec<usize> = (0..11).map(|_| step(&mut state, &p, true)).collect();
    assert_eq!(again, shown[1..]);
    let mut grown = playlist(7, Order::Shuffle);
    grown.items.rotate_left(2);
    let at = grown.items.iter().position(|i| Some(i.as_path()) == state.current(&p)).unwrap();
    state.seek(&grown, at);
    let mut state: PlaylistState = serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
    let path = |state: &PlaylistState, p: &Playlist| state.current(p).unwrap().to_path_buf();
    let earlier: Vec<PathBuf> =
        (0..3).map(|_| (step(&mut state, &grown, false), path(&state, &grown)).1).collect();
    assert_eq!(earlier, shown[8..11].iter().rev().map(|i| p.items[*i].clone()).collect::<Vec<_>>());
    for _ in 0..40 {
        state.advance(&grown, t0());
    }
    let kept = serde_json::to_value(&state).unwrap();
    assert_eq!((kept["behind"].as_array().unwrap().len(), kept.get("ahead")), (32, None));
}
