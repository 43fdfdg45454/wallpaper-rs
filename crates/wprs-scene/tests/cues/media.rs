use super::*;

fn track(track: u64, cover: u64, playing: bool) -> MediaState {
    let title = format!("Track {track}");
    MediaState {
        enabled: true,
        track,
        cover,
        title,
        playing,
        rate: 1.0,
        duration: 100.0,
        ..MediaState::default()
    }
}

fn cued() -> Runtime {
    let tls = ["play", "pause", "track", "cover"]
        .map(|n| json!({ "id": n, "duration": 1, "autoplay": false, "trigger": format!("media:{n}") }));
    let layers = ["play", "pause", "track", "cover"].map(|n| {
        json!({ "id": n, "type": "solid", "color": "#ffffff", "size": [1, 1],
            "transform": { "position": { "timeline": n, "keys": [[0, [0, 0]], [1, [100, 0]]] } } })
    });
    let sound =
        json!({ "id": "ding", "type": "sound", "sound": "a", "loop": false, "play_on": "media:track" });
    let mut layers = layers.to_vec();
    layers.push(sound);
    runtime(json!(tls), json!(layers))
}

fn moved(rt: &mut Runtime) -> Vec<bool> {
    rt.advance(0.5);
    let draws = rt.frame([0.0, 0.0, 100.0, 50.0], Quality::Q3).draws;
    draws.iter().take(4).map(|d| (1.0..99.0).contains(&d.world.apply([0.0, 0.0])[0])).collect()
}

#[test]
fn media_triggers_start_timelines_and_sounds_without_scripts() {
    let mut rt = cued();
    rt.set_lengths(BTreeMap::from([("a".into(), 1.0)]));
    assert!(rt.uses().media);
    assert_eq!(moved(&mut rt), [false; 4], "nothing plays, so nothing starts");
    assert!(!rt.voices().0[0].play);
    rt.set_media(track(1, 0, true));
    assert_eq!(moved(&mut rt), [true, false, true, false], "a track that starts playing");
    assert!(rt.voices().0[0].play, "the sound plays on the new track");
    let mut rt = cued();
    rt.set_media(track(1, 0, true));
    rt.advance(2.0);
    rt.set_media(track(1, 0, false));
    assert_eq!(moved(&mut rt), [false, true, false, false], "pausing fires only pause");
    rt.advance(2.0);
    rt.set_media(track(2, 7, false));
    assert_eq!(moved(&mut rt), [false, false, true, true], "a new paused track with a cover");
    rt.advance(2.0);
    rt.set_media(track(2, 7, false));
    assert_eq!(moved(&mut rt), [false; 4], "the same reading fires nothing");
    rt.set_media(track(2, 8, false));
    assert_eq!(moved(&mut rt), [false, false, false, true], "a new cover of the same track");
}

#[test]
fn media_triggers_stay_quiet_without_the_grant_or_a_player() {
    let mut rt = cued();
    rt.set_media(MediaState::default());
    rt.set_media(MediaState { enabled: false, ..track(1, 3, true) });
    assert_eq!(moved(&mut rt), [false; 4]);
    assert_eq!(rt.frame([0.0, 0.0, 100.0, 50.0], Quality::Q3).wake, Wake::Idle, "nothing wakes");
    rt.set_media(MediaState { enabled: true, ..MediaState::default() });
    assert_eq!(moved(&mut rt), [false; 4], "a grant without a player starts nothing");
    let mut rt = cued();
    rt.set_media(track(1, 0, true));
    rt.advance(2.0);
    rt.set_media(MediaState { enabled: true, ..MediaState::default() });
    assert_eq!(moved(&mut rt), [false, true, false, false], "the player leaving pauses");
}
